use crate::local::{authenticate_request, request_authentication};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
    Json, Router,
};
use fasti_application::{
    CapabilityKey, ConfigureMetadataProjectionCommand, FastiProblem, MetadataClaimRefreshService,
    MetadataOverrideMutation, MetadataProjectionPort, ReadMetadataProjectionQuery,
    RefreshMetadataClaimsCommand, RequestAccessContext, Violation,
};
use fasti_contracts::{
    metadata_field_group, metadata_projection_configuration_response, metadata_projection_response,
    metadata_refresh_mode, refresh_metadata_claims_response, ConfigureMetadataProjectionRequest,
    LastKnownGoodPolicyDto, MetadataFieldGroupDto, MetadataOverrideMutationDto,
    MetadataProjectionConfigurationResponse, MetadataProjectionQueryParameters,
    MetadataProjectionResponse, ProblemDetails, RefreshMetadataClaimsRequest,
    RefreshMetadataClaimsResponse,
};
use fasti_domain::{
    FieldKey, LastKnownGoodPolicy, MetadataFieldGroup, MetadataLocale, MetadataProjectionPolicy,
    MetadataProviderId, MetadataRegion, ProfileId, RecordId, RequestCorrelationId,
    MAX_FIELD_VALUE_BYTES,
};
use std::{collections::HashSet, sync::Arc};

const MAX_METADATA_JSON_BODY_BYTES: usize = 64 * 1024;
const MAX_METADATA_OVERRIDE_MUTATIONS: usize = 64;
type HttpResult<T> = Result<Json<T>, HttpProblem>;

#[derive(Clone)]
pub(crate) struct MetadataApiState {
    pub(crate) kernel: Arc<dyn fasti_application::LocalKernel>,
    pub(crate) refresh_service: Arc<dyn MetadataClaimRefreshService>,
    pub(crate) projection_port: Arc<dyn MetadataProjectionPort>,
    pub(crate) credential_operation_lock: Arc<tokio::sync::Mutex<()>>,
}

async fn authorize(
    state: &MetadataApiState,
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<RequestAccessContext, HttpProblem> {
    let authentication = request_authentication(headers, capability, correlation_id)?;
    let kernel = Arc::clone(&state.kernel);
    tokio::task::spawn_blocking(move || {
        authenticate_request(kernel.as_ref(), authentication, capability, correlation_id)
    })
    .await
    .map_err(|_| storage_problem(capability, correlation_id))?
    .map_err(application_problem)
}

fn storage_problem(capability: CapabilityKey, correlation_id: RequestCorrelationId) -> HttpProblem {
    application_problem(Box::new(FastiProblem::storage_unavailable(
        capability,
        correlation_id,
    )))
}

fn integrity_problem(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    application_problem(Box::new(FastiProblem::integrity_failed(
        capability,
        correlation_id,
    )))
}

fn invalid_input(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    pointer: &'static str,
    reason: &'static str,
    expected: &'static str,
) -> HttpProblem {
    let violation = Violation::try_new("invalid_metadata_input", pointer, reason, expected)
        .expect("adapter-owned metadata violation is valid");
    application_problem(Box::new(
        FastiProblem::validation_failed(capability, correlation_id, vec![violation])
            .expect("one metadata violation is within bounds"),
    ))
}

fn field_group(value: MetadataFieldGroupDto) -> MetadataFieldGroup {
    metadata_field_group(value)
}

fn parse_field_groups(
    values: Vec<MetadataFieldGroupDto>,
    allow_empty: bool,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<Vec<MetadataFieldGroup>, HttpProblem> {
    if (!allow_empty && values.is_empty()) || values.len() > 32 {
        return Err(invalid_input(
            capability,
            correlation_id,
            "/field_groups",
            "metadata field groups must be bounded",
            if allow_empty {
                "0 to 32 field groups"
            } else {
                "1 to 32 field groups"
            },
        ));
    }
    let groups = values.into_iter().map(field_group).collect::<Vec<_>>();
    let unique = groups.iter().copied().collect::<HashSet<_>>();
    if unique.len() != groups.len() {
        return Err(invalid_input(
            capability,
            correlation_id,
            "/field_groups",
            "metadata field groups must be unique",
            "a list without duplicate field groups",
        ));
    }
    Ok(groups)
}

#[utoipa::path(
    post,
    path = "/api/v1/metadata/claims/refresh",
    operation_id = "refresh_metadata_claims",
    tag = "metadata",
    security(("credential_bearer" = [])),
    request_body(content = RefreshMetadataClaimsRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Immutable metadata and rating claims were refreshed", body = RefreshMetadataClaimsResponse),
        (status = 400, description = "Request JSON is malformed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks metadata refresh scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "Record is not available in the authenticated workspace", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Provider refresh failed and the last-known-good projection was retained", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded metadata transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Request media type is unsupported", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Refresh request or provider route is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 429, description = "Provider rate limit was reached", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Metadata state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 502, description = "Provider returned an invalid response", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Provider runtime or storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn refresh_metadata_claims(
    State(state): State<MetadataApiState>,
    headers: HeaderMap,
    body: Result<Json<RefreshMetadataClaimsRequest>, JsonRejection>,
) -> HttpResult<RefreshMetadataClaimsResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::RefreshMetadataClaims;
    let request = body
        .map_err(|error| json_rejection(capability, correlation_id, error))?
        .0;
    let access = authorize(&state, &headers, capability, correlation_id).await?;
    let record_id = request.record_id.parse::<RecordId>().map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/record_id",
            "record ID is invalid",
            "a canonical rec_ UUIDv7",
        )
    })?;
    let provider_id = MetadataProviderId::try_new(request.provider_id).map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/provider_id",
            "provider ID is invalid",
            "a canonical lowercase provider ID",
        )
    })?;
    let field_groups = parse_field_groups(request.field_groups, false, capability, correlation_id)?;
    let locale = request
        .locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/locale",
                "locale is invalid",
                "a bounded BCP-47-shaped locale",
            )
        })?;
    let region = request
        .region
        .map(MetadataRegion::try_new)
        .transpose()
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/region",
                "region is invalid",
                "a 2 to 8 character region",
            )
        })?;
    let mode = metadata_refresh_mode(request.mode);
    let provider_id_text = provider_id.as_str().to_owned();
    let _credential_guard = state.credential_operation_lock.lock().await;
    let outcome = state
        .refresh_service
        .authorize_and_refresh(RefreshMetadataClaimsCommand::new(
            correlation_id,
            access,
            record_id,
            provider_id,
            field_groups,
            locale,
            region,
            mode,
        ))
        .await
        .map_err(application_problem)?;
    Ok(Json(refresh_metadata_claims_response(
        record_id,
        &provider_id_text,
        &outcome,
    )))
}

#[utoipa::path(
    get,
    path = "/api/v1/records/{record_id}/metadata-projection",
    operation_id = "read_metadata_projection",
    tag = "metadata",
    security(("credential_bearer" = [])),
    params(
        ("record_id" = String, Path, description = "Canonical Fasti Record ID"),
        MetadataProjectionQueryParameters
    ),
    responses(
        (status = 200, description = "Profile-selected metadata projection with provenance and offline cache state", body = MetadataProjectionResponse),
        (status = 401, description = "Credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks metadata projection read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "Record is not available in the authenticated workspace", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Record ID or query is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Metadata state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Metadata storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_metadata_projection(
    State(state): State<MetadataApiState>,
    Path(record_id): Path<String>,
    query: Result<
        Query<MetadataProjectionQueryParameters>,
        axum::extract::rejection::QueryRejection,
    >,
    headers: HeaderMap,
) -> HttpResult<MetadataProjectionResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ReadMetadataProjection;
    let access = authorize(&state, &headers, capability, correlation_id).await?;
    let record_id = record_id.parse::<RecordId>().map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/record_id",
            "record ID is invalid",
            "a canonical rec_ UUIDv7",
        )
    })?;
    let offline = query
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/offline",
                "offline query value is invalid",
                "true or false",
            )
        })?
        .offline;
    let profile_id = access.profile_id();
    let port = Arc::clone(&state.projection_port);
    let view = tokio::task::spawn_blocking(move || {
        port.authorize_and_read_projection(ReadMetadataProjectionQuery::new(
            correlation_id,
            access,
            record_id,
            offline,
        ))
    })
    .await
    .map_err(|_| storage_problem(capability, correlation_id))?
    .map_err(application_problem)?;
    if view.profile_id() != profile_id || view.record_id() != record_id {
        return Err(integrity_problem(capability, correlation_id));
    }
    Ok(Json(metadata_projection_response(&view)))
}

fn parse_overrides(
    values: Vec<MetadataOverrideMutationDto>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<Vec<MetadataOverrideMutation>, HttpProblem> {
    if values.len() > MAX_METADATA_OVERRIDE_MUTATIONS {
        return Err(invalid_input(
            capability,
            correlation_id,
            "/overrides",
            "too many override mutations",
            "at most 64 override mutations",
        ));
    }
    let mut targets = HashSet::with_capacity(values.len());
    let mut parsed = Vec::with_capacity(values.len());
    for value in values {
        let (record_text, field_text, set_value) = match value {
            MetadataOverrideMutationDto::Set {
                record_id,
                field_key,
                value,
            } => (record_id, field_key, Some(value)),
            MetadataOverrideMutationDto::Clear {
                record_id,
                field_key,
            } => (record_id, field_key, None),
        };
        let record_id = record_text.parse::<RecordId>().map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/overrides",
                "override record ID is invalid",
                "canonical rec_ UUIDv7 values",
            )
        })?;
        let field_key = FieldKey::try_new(field_text).map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/overrides",
                "override field key is invalid",
                "a canonical dotted lowercase field key",
            )
        })?;
        if !targets.insert((record_id, field_key.clone())) {
            return Err(invalid_input(
                capability,
                correlation_id,
                "/overrides",
                "override targets must be unique",
                "at most one mutation per Record and field",
            ));
        }
        parsed.push(match set_value {
            Some(value) => {
                if value.is_empty()
                    || value.len() > MAX_FIELD_VALUE_BYTES
                    || value.chars().any(char::is_control)
                {
                    return Err(invalid_input(
                        capability,
                        correlation_id,
                        "/overrides",
                        "override value is invalid",
                        "a non-empty value of at most 4096 bytes without control characters",
                    ));
                }
                MetadataOverrideMutation::Set {
                    record_id,
                    field_key,
                    value,
                }
            }
            None => MetadataOverrideMutation::Clear {
                record_id,
                field_key,
            },
        });
    }
    Ok(parsed)
}

#[utoipa::path(
    put,
    path = "/api/v1/profile/metadata-projection",
    operation_id = "configure_metadata_projection",
    tag = "metadata",
    security(("credential_bearer" = [])),
    request_body(content = ConfigureMetadataProjectionRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Profile metadata policy and override mutations were applied atomically", body = MetadataProjectionConfigurationResponse),
        (status = 400, description = "Request JSON is malformed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks metadata projection configuration scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded metadata transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Request media type is unsupported", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Projection policy or override mutation is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Metadata state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Metadata storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn configure_metadata_projection(
    State(state): State<MetadataApiState>,
    headers: HeaderMap,
    body: Result<Json<ConfigureMetadataProjectionRequest>, JsonRejection>,
) -> HttpResult<MetadataProjectionConfigurationResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ConfigureMetadataProjection;
    let request = body
        .map_err(|error| json_rejection(capability, correlation_id, error))?
        .0;
    let access = authorize(&state, &headers, capability, correlation_id).await?;
    let profile_id: ProfileId = access.profile_id();
    let preferred_provider_id = request
        .preferred_provider_id
        .map(MetadataProviderId::try_new)
        .transpose()
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/preferred_provider_id",
                "preferred provider ID is invalid",
                "a canonical lowercase provider ID",
            )
        })?;
    let preferred_locale = request
        .preferred_locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/preferred_locale",
                "preferred locale is invalid",
                "a bounded BCP-47-shaped locale",
            )
        })?;
    let original_locale = request
        .original_locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/original_locale",
                "original locale is invalid",
                "a bounded BCP-47-shaped locale",
            )
        })?;
    let region = request
        .region
        .map(MetadataRegion::try_new)
        .transpose()
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/region",
                "region is invalid",
                "a 2 to 8 character region",
            )
        })?;
    let field_groups = parse_field_groups(
        request.enabled_field_groups,
        true,
        capability,
        correlation_id,
    )?;
    let overrides = parse_overrides(request.overrides, capability, correlation_id)?;
    let policy = MetadataProjectionPolicy::new(
        profile_id,
        preferred_provider_id,
        preferred_locale,
        original_locale,
        request.allow_english_fallback,
        match request.last_known_good {
            LastKnownGoodPolicyDto::Allow => LastKnownGoodPolicy::Allow,
            LastKnownGoodPolicyDto::Deny => LastKnownGoodPolicy::Deny,
        },
    );
    let port = Arc::clone(&state.projection_port);
    let outcome = tokio::task::spawn_blocking(move || {
        port.authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
            correlation_id,
            access,
            policy,
            region,
            field_groups,
            overrides,
        ))
    })
    .await
    .map_err(|_| storage_problem(capability, correlation_id))?
    .map_err(application_problem)?;
    if outcome.enrichment_policy().profile_id() != profile_id {
        return Err(integrity_problem(capability, correlation_id));
    }
    Ok(Json(metadata_projection_configuration_response(&outcome)))
}

pub(crate) fn router() -> Router<MetadataApiState> {
    Router::new()
        .route(
            "/api/v1/metadata/claims/refresh",
            post(refresh_metadata_claims),
        )
        .route(
            "/api/v1/records/{record_id}/metadata-projection",
            get(read_metadata_projection),
        )
        .route(
            "/api/v1/profile/metadata-projection",
            put(configure_metadata_projection),
        )
        .layer(DefaultBodyLimit::max(MAX_METADATA_JSON_BODY_BYTES))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_refresh_groups_are_rejected_before_provider_io() {
        let result = parse_field_groups(
            vec![
                MetadataFieldGroupDto::BasicInfo,
                MetadataFieldGroupDto::BasicInfo,
            ],
            false,
            CapabilityKey::RefreshMetadataClaims,
            RequestCorrelationId::new_v7(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn duplicate_override_targets_are_rejected_before_persistence() {
        let record_id = RecordId::new_v7().to_string();
        let result = parse_overrides(
            vec![
                MetadataOverrideMutationDto::Clear {
                    record_id: record_id.clone(),
                    field_key: "core.title".to_owned(),
                },
                MetadataOverrideMutationDto::Set {
                    record_id,
                    field_key: "core.title".to_owned(),
                    value: "Replacement".to_owned(),
                },
            ],
            CapabilityKey::ConfigureMetadataProjection,
            RequestCorrelationId::new_v7(),
        );
        assert!(result.is_err());
    }
}
