use crate::{
    local::{bearer_secret, LocalApiState, RequestAuthentication},
    observation::accept_observation_request,
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
    let secret = bearer_secret(&headers, CapabilityKey::AcceptObservation, correlation_id)?;
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
        .map_err(|_| representation_problem(ProblemCode::MalformedJson, correlation_id))?;
    let normalized = normalize_template_request(source, request, correlation_id)?;
    accept_observation_request(
        state,
        RequestAuthentication::Bearer(secret),
        normalized,
        body.to_vec(),
        correlation_id,
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/tautulli/webhook",
    tag = "integrations",
    security(("credential_bearer" = [])),
    request_body = IntegrationObservationRequest,
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Webhook does not describe a complete supported occurrence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded evidence or observation capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
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
    security(("credential_bearer" = [])),
    request_body = IntegrationObservationRequest,
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Webhook does not describe a complete supported occurrence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded evidence or observation capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
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
    security(("credential_bearer" = [])),
    request_body = IntegrationObservationRequest,
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Webhook does not describe a complete supported occurrence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded evidence or observation capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
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

// Clamp to duration rather than passing a raw provider-reported position
// through: provider session and item metadata can disagree slightly
// (rounding, a still-buffering duration), and an unclamped position past
// duration would make validate_request reject an otherwise-genuine completion.
fn clamp_position(position: Option<u64>, duration: Option<u64>) -> Option<u64> {
    match (position, duration) {
        (Some(position), Some(duration)) => Some(position.min(duration)),
        (Some(position), None) => Some(position),
        (None, duration) => duration,
    }
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
    // Known gap, not fixed here: this identity has no per-user component, so
    // two different Emby accounts on the same server completing the same
    // item within one workspace synthesize the same lexeme and collide into
    // one occurrence. Emby's webhook plugin payload shape for the acting
    // user's id isn't confirmed enough here to add without risking a wrong
    // field guess (unlike `ingest.rs`'s desktop-side JellyfinWebhookPayload,
    // which already fails closed on a missing UserId for this exact reason).
    let item_id = emby_string(value, &["/Item/Id"]);
    let mut identifiers = Vec::new();
    if let Some(item_id) = item_id {
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

    let explicit_observed_at = emby_string(value, &["/UtcTimestamp", "/Timestamp"]);
    let observed_at = explicit_observed_at
        .map(str::to_owned)
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    // Keyed on stable fields (item, event kind, completion time), not the
    // raw evidence bytes: byte-level noise in a re-delivery of the same real
    // event (whitespace, field reordering, an added optional field) must not
    // change the identity, or a legitimate retry looks like a brand new
    // occurrence and 409 idempotency_conflict ("changed evidence, same
    // identity") becomes unreachable. Falls back to the raw digest when
    // Item/Id or an explicit timestamp is absent: `observed_at` would
    // otherwise fall back to Utc::now() above, which -- like the documented
    // Plex observed_at gap -- differs on every retry of the exact same
    // delivery and would turn a legitimate replay into a fabricated new
    // occurrence.
    let event_lexeme = match (item_id, explicit_observed_at) {
        (Some(item_id), Some(explicit_observed_at)) => {
            format!("emby:{item_id}:{normalized_event}:{explicit_observed_at}")
        }
        _ => String::from_utf8_lossy(raw).into_owned(),
    };
    let digest_identity = derive_deterministic_operation_id(&event_lexeme).to_string();
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

    let duration_seconds = ticks_to_seconds(runtime_ticks).filter(|seconds| *seconds > 0);
    let position_seconds = clamp_position(ticks_to_seconds(position_ticks), duration_seconds);

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
        position_seconds,
        duration_seconds,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/emby/webhook",
    tag = "integrations",
    security(("credential_bearer" = [])),
    responses(
        (status = 200, description = "Durable occurrence receipt", body = SubmitObservationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Provider event identity was reused with changed evidence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Webhook body exceeds its bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Webhook is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Webhook does not describe a completed supported occurrence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded evidence or observation capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn emby_webhook(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let secret = bearer_secret(&headers, CapabilityKey::AcceptObservation, correlation_id)?;
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
        .map_err(|_| representation_problem(ProblemCode::MalformedJson, correlation_id))?;
    let normalized = emby_request(&value, &body, correlation_id)?;
    accept_observation_request(
        state,
        RequestAuthentication::Bearer(secret),
        normalized,
        body.to_vec(),
        correlation_id,
    )
    .await
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
        let headers = std::str::from_utf8(&part[..header_end]).unwrap_or_default();
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
    let rating_key = value.pointer("/Metadata/ratingKey").and_then(Value::as_str);
    let mut identifiers = Vec::new();
    if let Some(rating_key) = rating_key {
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

    let duration_ms = value.pointer("/Metadata/duration").and_then(Value::as_u64);
    let view_offset_ms = value
        .pointer("/Metadata/viewOffset")
        .and_then(Value::as_u64);
    let last_viewed_at = value
        .pointer("/Metadata/lastViewedAt")
        .and_then(Value::as_i64);
    let occurred_at = last_viewed_at
        .and_then(|seconds| Utc.timestamp_opt(seconds, 0).single())
        .map(|value| value.to_rfc3339());
    let server_uuid = value.pointer("/Server/uuid").and_then(Value::as_str);
    // Known gap, not fixed here: this identity has no per-account component,
    // so two different Plex accounts on the same server scrobbling the same
    // item concurrently within one workspace synthesize the same lexeme and
    // collide into one occurrence. `/Account/id` is present in Plex's
    // webhook payload but wasn't confirmed stable/present across every event
    // type this adapter accepts here, so it's flagged rather than added
    // without that verification.
    // Keyed on stable fields (item, server, view time), not the raw payload
    // part bytes: byte-level noise in a re-delivery of the same real event
    // must not change the identity, or a legitimate retry looks like a
    // brand new occurrence and 409 idempotency_conflict becomes unreachable.
    // Server/account identity distinguishes the same ratingKey scrobbled on
    // two different Plex servers. Falls back to the raw digest when
    // ratingKey is absent, since there's no other stable identity to key on.
    let event_lexeme = match rating_key {
        Some(rating_key) => format!(
            "plex:{}:{rating_key}:{}",
            server_uuid.unwrap_or_default(),
            last_viewed_at.unwrap_or_default(),
        ),
        None => String::from_utf8_lossy(raw).into_owned(),
    };
    let source_event_id = derive_deterministic_operation_id(&event_lexeme).to_string();

    let duration_seconds = duration_ms
        .map(|value| value / 1000)
        .filter(|value| *value > 0);
    let position_seconds =
        clamp_position(view_offset_ms.map(|value| value / 1000), duration_seconds);

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
        position_seconds,
        duration_seconds,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations/plex/webhook",
    tag = "integrations",
    security(("credential_bearer" = [])),
    responses(
        (status = 200, description = "Durable Plex scrobble receipt", body = SubmitObservationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "A trusted proxy did not inject the scoped Fasti bearer credential", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks observation acceptance scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Plex event identity was reused with changed evidence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Multipart body exceeds its bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Request is not Plex multipart/form-data", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Payload is not a supported completed Plex event", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded evidence or observation capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn plex_webhook(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SubmitObservationResponse>, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let secret = bearer_secret(&headers, CapabilityKey::AcceptObservation, correlation_id)?;
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
        .map_err(|_| representation_problem(ProblemCode::MalformedJson, correlation_id))?;
    let normalized = plex_request(&value, payload, correlation_id)?;
    accept_observation_request(
        state,
        RequestAuthentication::Bearer(secret),
        normalized,
        payload.to_vec(),
        correlation_id,
    )
    .await
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

#[cfg(test)]
mod event_identity_tests {
    use super::*;

    /// Two deliveries of the same real Emby event that differ only in
    /// incidental JSON field order must derive the same identity and every
    /// other semantic field, except `observed_at` (Fasti's own ingestion
    /// timestamp -- see `emby_request`'s fallback to `Utc::now()` when
    /// `UtcTimestamp`/`Timestamp` is absent; both fixtures below supply an
    /// explicit `UtcTimestamp`, so it's stable here too).
    #[test]
    fn emby_identity_and_fields_are_stable_across_field_reordering() {
        let correlation_id = RequestCorrelationId::new_v7();
        let a: Value = serde_json::from_str(
            r#"{
                "Event": "playback.stop",
                "PlayedToCompletion": true,
                "UtcTimestamp": "2026-08-27T12:00:00Z",
                "Item": {
                    "Id": "emby-item-9",
                    "Type": "Movie",
                    "Name": "Fixture movie",
                    "RunTimeTicks": 72000000000,
                    "ProviderIds": {"Imdb": "tt9999999", "Tmdb": "9999"}
                },
                "Session": {"PlayState": {"PositionTicks": 72000000000}}
            }"#,
        )
        .expect("fixture a");
        let b: Value = serde_json::from_str(
            r#"{
                "Session": {"PlayState": {"PositionTicks": 72000000000}},
                "Item": {
                    "ProviderIds": {"Tmdb": "9999", "Imdb": "tt9999999"},
                    "RunTimeTicks": 72000000000,
                    "Name": "Fixture movie",
                    "Id": "emby-item-9",
                    "Type": "Movie"
                },
                "UtcTimestamp": "2026-08-27T12:00:00Z",
                "PlayedToCompletion": true,
                "Event": "playback.stop"
            }"#,
        )
        .expect("fixture b (reordered)");

        let ra = emby_request(&a, b"raw-a", correlation_id)
            .unwrap_or_else(|_| panic!("fixture a is valid"));
        let rb = emby_request(&b, b"raw-b", correlation_id)
            .unwrap_or_else(|_| panic!("fixture b is valid"));
        assert_eq!(ra.source_event_id, rb.source_event_id);
        assert_eq!(ra.observed_at, rb.observed_at);
        assert_eq!(ra.title, rb.title);
        assert_eq!(ra.position_seconds, rb.position_seconds);
        assert_eq!(ra.duration_seconds, rb.duration_seconds);
        assert_eq!(ra.target_grain, rb.target_grain);
        assert_eq!(claim_tuples(&ra.identifiers), claim_tuples(&rb.identifiers));
    }

    /// When Emby omits both UtcTimestamp and Timestamp, `emby_request` has no
    /// stable source timestamp and must not key identity on `Utc::now()`
    /// (which would differ on every retry of the exact same delivery and
    /// turn a legitimate replay into a fabricated new occurrence -- the same
    /// class of bug documented for Plex below). It should fall back to the
    /// raw body bytes instead, exactly like the Item/Id-missing case.
    #[test]
    fn emby_identity_falls_back_to_raw_bytes_when_no_explicit_timestamp_is_present() {
        let correlation_id = RequestCorrelationId::new_v7();
        let payload: Value = serde_json::from_str(
            r#"{
                "Event": "playback.stop",
                "PlayedToCompletion": true,
                "Item": {"Id": "emby-item-9", "Type": "Movie", "Name": "Fixture movie"}
            }"#,
        )
        .expect("fixture without an explicit timestamp");
        let raw = b"identical-raw-bytes";

        let first = emby_request(&payload, raw, correlation_id)
            .unwrap_or_else(|_| panic!("fixture is otherwise valid"));
        let second = emby_request(&payload, raw, correlation_id)
            .unwrap_or_else(|_| panic!("fixture is otherwise valid"));
        assert_eq!(
            first.source_event_id, second.source_event_id,
            "identical raw bytes must derive the same identity regardless of \
             wall-clock time between the two calls"
        );
    }

    /// Same as above for Plex, whose identity now derives from ratingKey,
    /// Server.uuid, and lastViewedAt rather than the raw multipart bytes.
    /// `observed_at` is intentionally excluded from this comparison: Plex
    /// webhooks carry no observation timestamp of their own, so
    /// `plex_request` always stamps it with `Utc::now()`. That value feeds
    /// `fasti-store`'s idempotency semantic_digest (see
    /// `crates/fasti-store/src/observation.rs::semantic_digest`), which
    /// means two genuinely separate Plex deliveries -- even byte-identical
    /// ones -- can never satisfy that digest check today. That's a
    /// pre-existing gap in the Plex adapter's timestamp handling, separate
    /// from the identity-derivation fix this test covers; flagged, not
    /// fixed here.
    #[test]
    fn plex_identity_and_fields_are_stable_across_field_reordering() {
        let correlation_id = RequestCorrelationId::new_v7();
        let a: Value = serde_json::from_str(
            r#"{
                "event": "media.scrobble",
                "Server": {"uuid": "plex-server"},
                "Metadata": {
                    "type": "movie",
                    "ratingKey": "200",
                    "title": "Fixture Plex movie",
                    "duration": 7200000,
                    "viewOffset": 7200000,
                    "lastViewedAt": 1787832000,
                    "Guid": [{"id": "imdb://tt2222333"}]
                }
            }"#,
        )
        .expect("fixture a");
        let b: Value = serde_json::from_str(
            r#"{
                "Server": {"uuid": "plex-server"},
                "event": "media.scrobble",
                "Metadata": {
                    "lastViewedAt": 1787832000,
                    "ratingKey": "200",
                    "type": "movie",
                    "title": "Fixture Plex movie",
                    "viewOffset": 7200000,
                    "duration": 7200000,
                    "Guid": [{"id": "imdb://tt2222333"}]
                }
            }"#,
        )
        .expect("fixture b (reordered)");

        let ra = plex_request(&a, b"raw-a", correlation_id)
            .unwrap_or_else(|_| panic!("fixture a is valid"));
        let rb = plex_request(&b, b"raw-b", correlation_id)
            .unwrap_or_else(|_| panic!("fixture b is valid"));
        assert_eq!(ra.source_event_id, rb.source_event_id);
        assert_eq!(ra.occurred_at, rb.occurred_at);
        assert_eq!(ra.title, rb.title);
        assert_eq!(ra.position_seconds, rb.position_seconds);
        assert_eq!(ra.duration_seconds, rb.duration_seconds);
        assert_eq!(ra.target_grain, rb.target_grain);
        assert_eq!(claim_tuples(&ra.identifiers), claim_tuples(&rb.identifiers));
    }

    /// A genuinely different Plex server publishing the same ratingKey must
    /// not collide: the fix folds Server.uuid into the identity precisely so
    /// two servers can't shadow each other's scrobbles.
    #[test]
    fn plex_identity_distinguishes_the_same_rating_key_on_different_servers() {
        let correlation_id = RequestCorrelationId::new_v7();
        let make = |server_uuid: &str| -> Value {
            serde_json::json!({
                "event": "media.scrobble",
                "Server": {"uuid": server_uuid},
                "Metadata": {
                    "type": "movie",
                    "ratingKey": "200",
                    "duration": 7200000,
                    "viewOffset": 7200000,
                    "lastViewedAt": 1787832000_i64,
                }
            })
        };
        let a = make("server-a");
        let b = make("server-b");
        let ra = plex_request(&a, b"raw", correlation_id)
            .unwrap_or_else(|_| panic!("fixture a is valid"));
        let rb = plex_request(&b, b"raw", correlation_id)
            .unwrap_or_else(|_| panic!("fixture b is valid"));
        assert_ne!(ra.source_event_id, rb.source_event_id);
    }

    fn claim_tuples(identifiers: &[ObservationIdentifierInput]) -> Vec<(String, String, String)> {
        identifiers
            .iter()
            .map(|claim| {
                (
                    claim.namespace.clone(),
                    claim.grain.clone(),
                    claim.value.clone(),
                )
            })
            .collect()
    }
}
