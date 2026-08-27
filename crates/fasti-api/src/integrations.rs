use crate::{
    local::LocalApiState,
    observation::{accept_observation_request, bearer_secret},
    problem::{application_problem, HttpProblem},
};
use axum::{
    body::Bytes,
    extract::State,
    http::{header, HeaderMap},
    routing::{get, post},
    Json, Router,
};
use chrono::{TimeZone, Utc};
use fasti_application::{
    derive_deterministic_operation_id, CapabilityKey, FastiProblem, ProblemCode, Violation,
};
use fasti_contracts::{
    IntegrationObservationRequest, IntegrationStatusDto, IntegrationStatusListResponse,
    ObservationIdentifierInput, ObservationIngressKind, ProblemDetails, SubmitObservationRequest,
    SubmitObservationResponse,
};
use fasti_domain::RequestCorrelationId;
use serde_json::Value;
use std::collections::BTreeMap;

const MAX_PROVIDER_JSON_BYTES: usize = 64 * 1024;
const MAX_PLEX_MULTIPART_BYTES: usize = 512 * 1024;
const MAX_MULTIPART_BOUNDARY_BYTES: usize = 200;

fn invalid_integration(
    correlation_id: RequestCorrelationId,
    pointer: &str,
    reason: &str,
    expected: &str,
) -> HttpProblem {
    let violation = Violation::try_new("invalid_observation", pointer, reason, expected)
        .expect("adapter-owned integration violation is valid");
    let problem = FastiProblem::invalid_observation(correlation_id, vec![violation])
        .expect("one integration violation is within bounds");
    application_problem(Box::new(problem))
}

fn representation_problem(code: ProblemCode, correlation_id: RequestCorrelationId) -> HttpProblem {
    application_problem(Box::new(FastiProblem::from_code(
        code,
        CapabilityKey::AcceptObservation,
        correlation_id,
    )))
}

fn content_type_is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

fn grain_for_item_type(item_type: &str) -> Option<&'static str> {
    match item_type.trim().to_ascii_lowercase().as_str() {
        "movie" | "film" => Some("film"),
        "episode" => Some("episode"),
        "track" | "audio" | "song" => Some("recording"),
        "series" | "show" => Some("series"),
        _ => None,
    }
}

fn namespace_for(provider: &str, grain: &str) -> Option<String> {
    let provider = provider.trim().to_ascii_lowercase();
    if provider.is_empty()
        || provider.len() > 40
        || !provider
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return None;
    }
    let namespace = match (provider.as_str(), grain) {
        ("imdb", _) => "imdb.title".to_owned(),
        ("tmdb", "film") => "tmdb.movie".to_owned(),
        ("tmdb", "series") => "tmdb.series".to_owned(),
        ("tmdb", "episode") => "tmdb.episode".to_owned(),
        ("tvdb", "series") => "tvdb.series".to_owned(),
        ("tvdb", "episode") => "tvdb.episode".to_owned(),
        ("musicbrainz", "recording") => "musicbrainz.recording".to_owned(),
        _ => format!("{provider}.{grain}"),
    };
    (namespace.len() <= 64).then_some(namespace)
}

fn push_ids(
    output: &mut Vec<ObservationIdentifierInput>,
    values: &BTreeMap<String, String>,
    grain: &str,
    correlation_id: RequestCorrelationId,
) -> Result<(), HttpProblem> {
    for (provider, value) in values {
        if value.is_empty() || value.len() > 512 {
            return Err(invalid_integration(
                correlation_id,
                "/provider_ids",
                "provider identifier is empty or exceeds its bound",
                "a non-empty identifier no longer than 512 bytes",
            ));
        }
        let namespace = namespace_for(provider, grain).ok_or_else(|| {
            invalid_integration(
                correlation_id,
                "/provider_ids",
                "provider namespace is not a safe bounded identifier",
                "an ASCII provider key using letters, numbers, dot, dash, or underscore",
            )
        })?;
        output.push(ObservationIdentifierInput {
            namespace,
            grain: grain.to_owned(),
            value: value.clone(),
        });
        if output.len() > 16 {
            return Err(invalid_integration(
                correlation_id,
                "/provider_ids",
                "too many provider identifiers were supplied",
                "at most 16 item and series identifiers combined",
            ));
        }
    }
    Ok(())
}

fn normalize_template_request(
    source: &str,
    request: IntegrationObservationRequest,
    correlation_id: RequestCorrelationId,
) -> Result<SubmitObservationRequest, HttpProblem> {
    if !request.completed {
        return Err(invalid_integration(
            correlation_id,
            "/completed",
            "this Chronicle adapter accepts complete consumption occurrences only",
            "true; use the separate progress capability for partial playback state",
        ));
    }
    let grain = grain_for_item_type(&request.item_type).ok_or_else(|| {
        invalid_integration(
            correlation_id,
            "/item_type",
            "this media kind is not supported by the playback webhook adapter",
            "movie, episode, track, audio, song, series, or show",
        )
    })?;
    let mut identifiers = Vec::new();
    push_ids(
        &mut identifiers,
        &request.provider_ids,
        grain,
        correlation_id,
    )?;
    if grain == "episode" {
        push_ids(
            &mut identifiers,
            &request.series_provider_ids,
            "series",
            correlation_id,
        )?;
    }

    let title = match (request.series_title.as_deref(), request.title.as_deref()) {
        (Some(series), Some(title)) if grain == "episode" => Some(format!("{series} — {title}")),
        (_, Some(title)) => Some(title.to_owned()),
        (Some(series), None) => Some(series.to_owned()),
        _ => None,
    };

    Ok(SubmitObservationRequest {
        kind: ObservationIngressKind::ConsumptionOccurrence,
        source: source.to_owned(),
        source_event_id: request.source_event_id,
        observed_at: request.observed_at,
        occurred_at: request.occurred_at,
        target_grain: Some(grain.to_owned()),
        identifiers,
        title,
        progress_percent: Some(100.0),
        position_seconds: request.position_seconds.or(request.duration_seconds),
        duration_seconds: request.duration_seconds,
    })
}

async fn template_webhook(
    source: &'static str,
    state: LocalApiState,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let secret = bearer_secret(&headers, correlation_id)?;
    if !content_type_is_json(&headers) {
        return Err(representation_problem(
            ProblemCode::UnsupportedMediaType,
            correlation_id,
        ));
    }
    if body.len() > MAX_PROVIDER_JSON_BYTES {
        return Err(application_problem(Box::new(
            FastiProblem::payload_too_large(
                CapabilityKey::AcceptObservation,
                correlation_id,
                vec![Violation::try_new(
                    "invalid_representation",
                    "/",
                    "integration webhook body exceeds the ingress bound",
                    "at most 65536 bytes",
                )
                .expect("adapter-owned representation violation is valid")],
            )
            .expect("one violation is within bounds"),
        )));
    }
    let request: IntegrationObservationRequest = serde_json::from_slice(&body)
        .map_err(|_| representation_problem(ProblemCode::ValidationFailed, correlation_id))?;
    let normalized = normalize_template_request(source, request, correlation_id)?;
    accept_observation_request(state, secret, normalized, body.to_vec(), correlation_id).await
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/tautulli/webhook",
    tag = "integrations",
    request_body = IntegrationObservationRequest,
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails),
        (status = 422, description = "Webhook does not describe a complete supported occurrence", body = ProblemDetails)
    )
)]
pub(crate) async fn tautulli_webhook(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    template_webhook("tautulli", state, headers, body).await
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/jellyfin/webhook",
    tag = "integrations",
    request_body = IntegrationObservationRequest,
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails),
        (status = 422, description = "Webhook does not describe a complete supported occurrence", body = ProblemDetails)
    )
)]
pub(crate) async fn jellyfin_webhook(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    template_webhook("jellyfin", state, headers, body).await
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/nuvio/webhook",
    tag = "integrations",
    request_body = IntegrationObservationRequest,
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails),
        (status = 422, description = "Webhook does not describe a complete supported occurrence", body = ProblemDetails)
    )
)]
pub(crate) async fn nuvio_webhook(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    template_webhook("nuvio", state, headers, body).await
}

fn emby_string<'a>(value: &'a Value, pointers: &[&str]) -> Option<&'a str> {
    pointers
        .iter()
        .find_map(|pointer| value.pointer(pointer).and_then(Value::as_str))
}

fn ticks_to_seconds(value: Option<u64>) -> Option<u64> {
    value.map(|ticks| ticks / 10_000_000)
}

fn emby_request(
    value: &Value,
    raw: &[u8],
    correlation_id: RequestCorrelationId,
) -> Result<SubmitObservationRequest, HttpProblem> {
    let event = emby_string(value, &["/Event", "/NotificationType", "/notificationType"])
        .unwrap_or_default();
    let normalized_event = event.to_ascii_lowercase();
    let complete = value
        .pointer("/PlayedToCompletion")
        .and_then(Value::as_bool)
        .or_else(|| {
            value
                .pointer("/Item/UserData/Played")
                .and_then(Value::as_bool)
        })
        .unwrap_or_else(|| {
            normalized_event == "item.markplayed" || normalized_event == "itemmarkedplayed"
        });
    if !complete {
        return Err(invalid_integration(
            correlation_id,
            "/Event",
            "Emby event is not an explicit completed occurrence",
            "item.markplayed or a playback.stop event with PlayedToCompletion/UserData.Played true",
        ));
    }
    if !matches!(
        normalized_event.as_str(),
        "playback.stop" | "playbackstop" | "item.markplayed" | "itemmarkedplayed"
    ) {
        return Err(invalid_integration(
            correlation_id,
            "/Event",
            "Emby event is not a supported Chronicle trigger",
            "playback.stop or item.markplayed",
        ));
    }

    let item_type = emby_string(value, &["/Item/Type"]).unwrap_or("custom");
    let grain = grain_for_item_type(item_type).ok_or_else(|| {
        invalid_integration(
            correlation_id,
            "/Item/Type",
            "Emby item type is not supported by this adapter",
            "Movie, Episode, Audio, or Series",
        )
    })?;
    let mut identifiers = Vec::new();
    if let Some(item_id) = emby_string(value, &["/Item/Id"]) {
        identifiers.push(ObservationIdentifierInput {
            namespace: format!("emby.{grain}"),
            grain: grain.to_owned(),
            value: item_id.to_owned(),
        });
    }
    if let Some(provider_ids) = value
        .pointer("/Item/ProviderIds")
        .and_then(Value::as_object)
    {
        let mut ids = BTreeMap::new();
        for (provider, id) in provider_ids {
            if let Some(id) = id.as_str() {
                ids.insert(provider.to_ascii_lowercase(), id.to_owned());
            }
        }
        push_ids(&mut identifiers, &ids, grain, correlation_id)?;
    }
    if grain == "episode" {
        if let Some(series_id) = emby_string(value, &["/Item/SeriesId"]) {
            identifiers.push(ObservationIdentifierInput {
                namespace: "emby.series".to_owned(),
                grain: "series".to_owned(),
                value: series_id.to_owned(),
            });
        }
    }
    if identifiers.len() > 16 {
        return Err(invalid_integration(
            correlation_id,
            "/Item/ProviderIds",
            "too many Emby identifiers were supplied",
            "at most 16 identity clues",
        ));
    }

    let digest_identity =
        derive_deterministic_operation_id(&String::from_utf8_lossy(raw)).to_string();
    let observed_at = emby_string(value, &["/UtcTimestamp", "/Timestamp"])
        .map(str::to_owned)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let title = emby_string(value, &["/Item/Name"]).map(str::to_owned);
    let runtime_ticks = value.pointer("/Item/RunTimeTicks").and_then(Value::as_u64);
    let position_ticks = value
        .pointer("/Session/PlayState/PositionTicks")
        .and_then(Value::as_u64)
        .or_else(|| {
            value
                .pointer("/PlaybackPositionTicks")
                .and_then(Value::as_u64)
        });

    Ok(SubmitObservationRequest {
        kind: ObservationIngressKind::ConsumptionOccurrence,
        source: "emby".to_owned(),
        source_event_id: digest_identity,
        observed_at,
        occurred_at: None,
        target_grain: Some(grain.to_owned()),
        identifiers,
        title,
        progress_percent: Some(100.0),
        position_seconds: ticks_to_seconds(position_ticks)
            .or_else(|| ticks_to_seconds(runtime_ticks)),
        duration_seconds: ticks_to_seconds(runtime_ticks).filter(|seconds| *seconds > 0),
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/emby/webhook",
    tag = "integrations",
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails),
        (status = 422, description = "Webhook does not describe a completed supported occurrence", body = ProblemDetails)
    )
)]
pub(crate) async fn emby_webhook(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let secret = bearer_secret(&headers, correlation_id)?;
    if !content_type_is_json(&headers) {
        return Err(representation_problem(
            ProblemCode::UnsupportedMediaType,
            correlation_id,
        ));
    }
    if body.len() > MAX_PROVIDER_JSON_BYTES {
        return Err(invalid_integration(
            correlation_id,
            "/",
            "Emby webhook body exceeds its bound",
            "at most 65536 bytes",
        ));
    }
    let value: Value = serde_json::from_slice(&body)
        .map_err(|_| representation_problem(ProblemCode::ValidationFailed, correlation_id))?;
    let normalized = emby_request(&value, &body, correlation_id)?;
    accept_observation_request(state, secret, normalized, body.to_vec(), correlation_id).await
}

fn multipart_boundary(content_type: &str) -> Option<&str> {
    content_type.split(';').skip(1).find_map(|part| {
        let (name, value) = part.trim().split_once('=')?;
        if !name.eq_ignore_ascii_case("boundary") {
            return None;
        }
        Some(value.trim().trim_matches('"'))
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    if needle.is_empty() || start > haystack.len() {
        return None;
    }
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}

fn plex_payload_part<'a>(body: &'a [u8], boundary: &str) -> Option<&'a [u8]> {
    if boundary.is_empty() || boundary.len() > MAX_MULTIPART_BOUNDARY_BYTES {
        return None;
    }
    let marker = format!("--{boundary}").into_bytes();
    let separator = b"\r\n\r\n";
    let mut cursor = 0;
    while let Some(marker_start) = find_bytes(body, &marker, cursor) {
        let part_start = marker_start + marker.len();
        let next_marker = find_bytes(body, &marker, part_start).unwrap_or(body.len());
        let part = &body[part_start..next_marker];
        let Some(header_end) = find_bytes(part, separator, 0) else {
            cursor = next_marker;
            if cursor >= body.len() {
                break;
            }
            continue;
        };
        let headers = std::str::from_utf8(&part[..header_end]).ok()?;
        if headers.contains("name=\"payload\"") || headers.contains("name=payload") {
            let mut content = &part[header_end + separator.len()..];
            while content.ends_with(b"\r\n") {
                content = &content[..content.len() - 2];
            }
            return Some(content);
        }
        cursor = next_marker;
        if cursor >= body.len() {
            break;
        }
    }
    None
}

fn plex_request(
    value: &Value,
    raw: &[u8],
    correlation_id: RequestCorrelationId,
) -> Result<SubmitObservationRequest, HttpProblem> {
    let event = value
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if event != "media.scrobble" {
        return Err(invalid_integration(
            correlation_id,
            "/event",
            "Plex event is not a completed media occurrence",
            "media.scrobble",
        ));
    }
    let media_type = value
        .pointer("/Metadata/type")
        .and_then(Value::as_str)
        .unwrap_or("custom");
    let grain = grain_for_item_type(media_type).ok_or_else(|| {
        invalid_integration(
            correlation_id,
            "/Metadata/type",
            "Plex media type is not supported by this adapter",
            "movie, episode, or track",
        )
    })?;
    let mut identifiers = Vec::new();
    if let Some(rating_key) = value.pointer("/Metadata/ratingKey").and_then(Value::as_str) {
        identifiers.push(ObservationIdentifierInput {
            namespace: format!("plex.{grain}"),
            grain: grain.to_owned(),
            value: rating_key.to_owned(),
        });
    }
    if grain == "episode" {
        if let Some(series_key) = value
            .pointer("/Metadata/grandparentRatingKey")
            .and_then(Value::as_str)
        {
            identifiers.push(ObservationIdentifierInput {
                namespace: "plex.series".to_owned(),
                grain: "series".to_owned(),
                value: series_key.to_owned(),
            });
        }
    }
    if let Some(guids) = value.pointer("/Metadata/Guid").and_then(Value::as_array) {
        for guid in guids.iter().take(12) {
            let Some(id) = guid.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some((provider, provider_id)) = id.split_once("://") else {
                continue;
            };
            let Some(namespace) = namespace_for(provider, grain) else {
                continue;
            };
            identifiers.push(ObservationIdentifierInput {
                namespace,
                grain: grain.to_owned(),
                value: provider_id.to_owned(),
            });
            if identifiers.len() >= 16 {
                break;
            }
        }
    }

    let source_event_id =
        derive_deterministic_operation_id(&String::from_utf8_lossy(raw)).to_string();
    let duration_ms = value.pointer("/Metadata/duration").and_then(Value::as_u64);
    let view_offset_ms = value
        .pointer("/Metadata/viewOffset")
        .and_then(Value::as_u64);
    let occurred_at = value
        .pointer("/Metadata/lastViewedAt")
        .and_then(Value::as_i64)
        .and_then(|seconds| Utc.timestamp_opt(seconds, 0).single())
        .map(|value| value.to_rfc3339());

    Ok(SubmitObservationRequest {
        kind: ObservationIngressKind::ConsumptionOccurrence,
        source: "plex".to_owned(),
        source_event_id,
        observed_at: Utc::now().to_rfc3339(),
        occurred_at,
        target_grain: Some(grain.to_owned()),
        identifiers,
        title: value
            .pointer("/Metadata/title")
            .and_then(Value::as_str)
            .map(str::to_owned),
        progress_percent: Some(100.0),
        position_seconds: view_offset_ms
            .map(|value| value / 1000)
            .or_else(|| duration_ms.map(|value| value / 1000)),
        duration_seconds: duration_ms
            .map(|value| value / 1000)
            .filter(|value| *value > 0),
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/plex/webhook",
    tag = "integrations",
    responses(
        (status = 200, description = "Durable Plex scrobble receipt", body = SubmitObservationResponse),
        (status = 401, description = "A trusted proxy did not inject the scoped Fasti bearer credential", body = ProblemDetails),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails),
        (status = 409, description = "Plex event identity was reused with changed evidence", body = ProblemDetails),
        (status = 413, description = "Multipart body exceeds its bound", body = ProblemDetails),
        (status = 415, description = "Request is not Plex multipart/form-data", body = ProblemDetails),
        (status = 422, description = "Payload is not a supported completed Plex event", body = ProblemDetails)
    )
)]
pub(crate) async fn plex_webhook(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let secret = bearer_secret(&headers, correlation_id)?;
    if body.len() > MAX_PLEX_MULTIPART_BYTES {
        return Err(invalid_integration(
            correlation_id,
            "/",
            "Plex multipart body exceeds its bound",
            "at most 524288 bytes including optional artwork",
        ));
    }
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| representation_problem(ProblemCode::UnsupportedMediaType, correlation_id))?;
    if !content_type
        .to_ascii_lowercase()
        .starts_with("multipart/form-data")
    {
        return Err(representation_problem(
            ProblemCode::UnsupportedMediaType,
            correlation_id,
        ));
    }
    let boundary = multipart_boundary(content_type).ok_or_else(|| {
        invalid_integration(
            correlation_id,
            "/",
            "Plex multipart boundary is missing or invalid",
            "a bounded multipart/form-data boundary",
        )
    })?;
    let payload = plex_payload_part(&body, boundary).ok_or_else(|| {
        invalid_integration(
            correlation_id,
            "/payload",
            "Plex multipart request does not contain one payload JSON part",
            "a form-data part named payload",
        )
    })?;
    if payload.len() > MAX_PROVIDER_JSON_BYTES {
        return Err(invalid_integration(
            correlation_id,
            "/payload",
            "Plex JSON payload exceeds its bound",
            "at most 65536 bytes",
        ));
    }
    let value: Value = serde_json::from_slice(payload)
        .map_err(|_| representation_problem(ProblemCode::ValidationFailed, correlation_id))?;
    let normalized = plex_request(&value, payload, correlation_id)?;
    accept_observation_request(state, secret, normalized, payload.to_vec(), correlation_id).await
}

#[utoipa::path(
    get,
    path = "/api/v1/integrations",
    tag = "integrations",
    responses((status = 200, description = "Production integration runtime status", body = IntegrationStatusListResponse))
)]
pub async fn integration_status() -> Json<IntegrationStatusListResponse> {
    Json(IntegrationStatusListResponse {
        integrations: vec![
            IntegrationStatusDto {
                id: "nuvio".to_owned(),
                label: "NuvioTV".to_owned(),
                state: "setup_required".to_owned(),
                available: true,
                endpoint_ready: true,
                setup_action: "Create an observation client and configure the Nuvio Fasti provider."
                    .to_owned(),
                detail: "Authenticated occurrence ingress is mounted. Nuvio must keep its own durable retry outbox."
                    .to_owned(),
            },
            IntegrationStatusDto {
                id: "plex".to_owned(),
                label: "Plex".to_owned(),
                state: "setup_required".to_owned(),
                available: true,
                endpoint_ready: true,
                setup_action: "Use Tautulli, or place the Plex webhook behind a trusted proxy that injects a scoped Fasti bearer header."
                    .to_owned(),
                detail: "Plex webhooks do not carry a Fasti credential. Fasti never places bearer secrets in webhook URLs."
                    .to_owned(),
            },
            IntegrationStatusDto {
                id: "tautulli".to_owned(),
                label: "Tautulli".to_owned(),
                state: "setup_required".to_owned(),
                available: true,
                endpoint_ready: true,
                setup_action: "Configure the documented JSON template and Authorization header."
                    .to_owned(),
                detail: "Tautulli can submit watched events through a bounded authenticated webhook."
                    .to_owned(),
            },
            IntegrationStatusDto {
                id: "jellyfin".to_owned(),
                label: "Jellyfin".to_owned(),
                state: "setup_required".to_owned(),
                available: true,
                endpoint_ready: true,
                setup_action: "Configure the Jellyfin Webhook plugin with the Fasti template and Authorization header."
                    .to_owned(),
                detail: "Completed playback notifications are normalized into durable Fasti occurrences."
                    .to_owned(),
            },
            IntegrationStatusDto {
                id: "emby".to_owned(),
                label: "Emby".to_owned(),
                state: "setup_required".to_owned(),
                available: true,
                endpoint_ready: true,
                setup_action: "Configure Emby playback-stop/mark-played webhooks with a scoped Authorization header."
                    .to_owned(),
                detail: "Native Emby event payloads are accepted only when completion is explicit."
                    .to_owned(),
            },
            IntegrationStatusDto {
                id: "mpris".to_owned(),
                label: "Desktop MPRIS observer".to_owned(),
                state: if cfg!(target_os = "linux") {
                    "setup_required"
                } else {
                    "unsupported"
                }
                .to_owned(),
                available: cfg!(target_os = "linux"),
                endpoint_ready: false,
                setup_action: if cfg!(target_os = "linux") {
                    "Enable the desktop MPRIS observer."
                } else {
                    "Use a supported Linux desktop or another observation adapter."
                }
                .to_owned(),
                detail: "MPRIS support is platform-scoped and never claimed on unsupported operating systems."
                    .to_owned(),
            },
        ],
    })
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new()
        .route("/api/v1/integrations", get(integration_status))
        .route("/api/v1/integrations/nuvio/webhook", post(nuvio_webhook))
        .route(
            "/api/v1/integrations/tautulli/webhook",
            post(tautulli_webhook),
        )
        .route(
            "/api/v1/integrations/jellyfin/webhook",
            post(jellyfin_webhook),
        )
        .route("/api/v1/integrations/emby/webhook", post(emby_webhook))
        .route("/api/v1/integrations/plex/webhook", post(plex_webhook))
}
