use crate::local::{
    application_request_authentication, authenticate_application_request, run_kernel, LocalApiState,
};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::HeaderMap,
    routing::{get, post},
    Json, Router,
};
use fasti_application::{
    AnimeGroupingPolicyChange, AnimeGroupingPolicyScope, ApplyAnimeGroupingPolicyChangeCommand,
    CapabilityKey, FastiProblem, IdentityImpactPageLimit, PreviewAnimeGroupingPolicyChangeQuery,
    ProviderId, ReadAnimeGroupingPolicyQuery, ResolveIdentityRouteQuery, Violation,
};
use fasti_contracts::{
    anime_grouping_policy_impact_response, anime_grouping_preference,
    apply_anime_grouping_policy_change_response, read_anime_grouping_policy_response,
    resolution_intent, resolve_identity_route_response, AnimeGroupingPolicyChangeDto,
    AnimeGroupingPolicyImpactResponse, AnimeGroupingPolicyScopeDto,
    AnimeGroupingPolicyScopeKindDto, ApplyAnimeGroupingPolicyChangeRequest,
    ApplyAnimeGroupingPolicyChangeResponse, PreviewAnimeGroupingPolicyChangeRequest,
    ProblemDetails, ReadAnimeGroupingPolicyParameters, ReadAnimeGroupingPolicyResponse,
    ResolveIdentityRouteParameters, ResolveIdentityRouteResponse,
};
use fasti_domain::{ClientId, OperationId, RecordId, RequestCorrelationId, Sha256Digest};
use sha2::{Digest, Sha256};

type HttpResult<T> = Result<Json<T>, HttpProblem>;

fn invalid_input(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    pointer: &'static str,
    reason: &'static str,
    expected: &'static str,
) -> HttpProblem {
    let violation = Violation::try_new("invalid_identity_routing_input", pointer, reason, expected)
        .expect("adapter-owned identity routing violation is valid");
    application_problem(Box::new(
        FastiProblem::validation_failed(capability, correlation_id, vec![violation])
            .expect("one identity routing violation is within bounds"),
    ))
}

fn application_validation_problem(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    let violation = Violation::try_new(
        "invalid_identity_routing_input",
        "/change",
        "policy change does not match its scope or operation",
        "a valid set, inherit_profile, or rollback change",
    )
    .expect("adapter-owned identity routing violation is valid");
    Box::new(
        FastiProblem::validation_failed(capability, correlation_id, vec![violation])
            .expect("one identity routing violation is within bounds"),
    )
}

fn policy_scope(
    value: AnimeGroupingPolicyScopeDto,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<AnimeGroupingPolicyScope, HttpProblem> {
    match (value.kind, value.client_id) {
        (AnimeGroupingPolicyScopeKindDto::Profile, None) => Ok(AnimeGroupingPolicyScope::Profile),
        (AnimeGroupingPolicyScopeKindDto::Client, Some(client_id)) => client_id
            .parse::<ClientId>()
            .map(AnimeGroupingPolicyScope::Client)
            .map_err(|_| {
                invalid_input(
                    capability,
                    correlation_id,
                    "/scope/client_id",
                    "application client ID is invalid",
                    "a canonical cli_ UUIDv7",
                )
            }),
        _ => Err(invalid_input(
            capability,
            correlation_id,
            "/scope",
            "anime policy scope and application client ID disagree",
            "profile without client_id, or client with client_id",
        )),
    }
}

fn policy_change(
    value: AnimeGroupingPolicyChangeDto,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<AnimeGroupingPolicyChange, HttpProblem> {
    match value {
        AnimeGroupingPolicyChangeDto::Set { preference } => Ok(AnimeGroupingPolicyChange::Set(
            anime_grouping_preference(preference),
        )),
        AnimeGroupingPolicyChangeDto::InheritProfile => {
            Ok(AnimeGroupingPolicyChange::InheritProfile)
        }
        AnimeGroupingPolicyChangeDto::Rollback {
            applied_operation_id,
        } => applied_operation_id
            .parse::<OperationId>()
            .map(|applied_operation_id| AnimeGroupingPolicyChange::Rollback {
                applied_operation_id,
            })
            .map_err(|_| {
                invalid_input(
                    capability,
                    correlation_id,
                    "/change/applied_operation_id",
                    "rollback operation ID is invalid",
                    "a canonical op_ UUIDv7",
                )
            }),
    }
}

fn policy_semantic_digest(
    scope: AnimeGroupingPolicyScope,
    expected_revision: u64,
    change: AnimeGroupingPolicyChange,
) -> Sha256Digest {
    let mut hasher = Sha256::new();
    hasher.update(b"fasti:anime-grouping-policy:v1\0");
    match scope {
        AnimeGroupingPolicyScope::Profile => hasher.update(b"profile\0"),
        AnimeGroupingPolicyScope::Client(client_id) => {
            hasher.update(b"client\0");
            hasher.update(client_id.to_string());
            hasher.update([0]);
        }
    }
    hasher.update(expected_revision.to_be_bytes());
    match change {
        AnimeGroupingPolicyChange::Set(preference) => {
            hasher.update(b"set\0");
            hasher.update(preference.as_str());
        }
        AnimeGroupingPolicyChange::InheritProfile => hasher.update(b"inherit_profile"),
        AnimeGroupingPolicyChange::Rollback {
            applied_operation_id,
        } => {
            hasher.update(b"rollback\0");
            hasher.update(applied_operation_id.to_string());
        }
    }
    Sha256Digest::from_bytes((&hasher.finalize()).into())
}

#[utoipa::path(
    get,
    path = "/api/v1/records/{record_id}/identity-route",
    operation_id = "resolve_identity_route",
    tag = "identity",
    security(("credential_bearer" = []), ("browser_session_cookie" = [])),
    params(
        ("record_id" = String, Path, description = "Canonical Fasti Record ID"),
        ResolveIdentityRouteParameters
    ),
    responses(
        (status = 200, description = "Purpose-specific provider route with evidence", body = ResolveIdentityRouteResponse),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks identity read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "Record does not exist", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Route request is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Identity evidence failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Identity evidence exceeds a bounded local limit", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn resolve_identity_route(
    State(state): State<LocalApiState>,
    Path(record_id): Path<String>,
    query: Result<Query<ResolveIdentityRouteParameters>, axum::extract::rejection::QueryRejection>,
    headers: HeaderMap,
) -> HttpResult<ResolveIdentityRouteResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ResolveIdentityRoute;
    let query = query.map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/query",
            "identity route query is invalid",
            "intent and target_provider query parameters",
        )
    })?;
    let record_id = record_id.parse::<RecordId>().map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/record_id",
            "Record ID is invalid",
            "a canonical rec_ UUIDv7",
        )
    })?;
    let target_provider = ProviderId::try_new(&query.target_provider).map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/target_provider",
            "target provider is invalid",
            "a canonical lowercase provider ID",
        )
    })?;
    let intent = resolution_intent(query.intent);
    let authentication = application_request_authentication(
        &headers,
        state.browser_boundary.as_ref(),
        false,
        capability,
        correlation_id,
    )?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        let access = authenticate_application_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
        )?;
        kernel.authorize_and_resolve_identity(ResolveIdentityRouteQuery::new(
            correlation_id,
            access,
            record_id,
            intent,
            target_provider,
        ))
    })
    .await?;
    Ok(Json(resolve_identity_route_response(outcome.plan())))
}

#[utoipa::path(
    get,
    path = "/api/v1/profile/anime-grouping-policy",
    operation_id = "read_anime_grouping_policy",
    tag = "profile",
    security(("credential_bearer" = []), ("browser_session_cookie" = [])),
    params(ReadAnimeGroupingPolicyParameters),
    responses(
        (status = 200, description = "Current profile or application-client anime projection policy", body = ReadAnimeGroupingPolicyResponse),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Policy scope is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Policy state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_anime_grouping_policy(
    State(state): State<LocalApiState>,
    query: Result<
        Query<ReadAnimeGroupingPolicyParameters>,
        axum::extract::rejection::QueryRejection,
    >,
    headers: HeaderMap,
) -> HttpResult<ReadAnimeGroupingPolicyResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ReadAnimeGroupingPolicy;
    let query = query.map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/query",
            "anime policy query is invalid",
            "scope and optional client_id query parameters",
        )
    })?;
    let scope = policy_scope(
        AnimeGroupingPolicyScopeDto {
            kind: query.scope,
            client_id: query.client_id.clone(),
        },
        capability,
        correlation_id,
    )?;
    let authentication = application_request_authentication(
        &headers,
        state.browser_boundary.as_ref(),
        false,
        capability,
        correlation_id,
    )?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        let access = authenticate_application_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
        )?;
        kernel.authorize_and_read_anime_grouping_policy(ReadAnimeGroupingPolicyQuery::new(
            correlation_id,
            access,
            scope,
        ))
    })
    .await?;
    Ok(Json(read_anime_grouping_policy_response(outcome.policy())))
}

#[utoipa::path(
    post,
    path = "/api/v1/profile/anime-grouping-policy/preview",
    operation_id = "preview_anime_grouping_policy_change",
    tag = "profile",
    security(("credential_bearer" = []), ("browser_session_cookie" = [])),
    request_body = PreviewAnimeGroupingPolicyChangeRequest,
    responses(
        (status = 200, description = "Bounded impact preview without mutation", body = AnimeGroupingPolicyImpactResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Policy preview request is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Policy state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Identity evidence exceeds a bounded local limit", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn preview_anime_grouping_policy_change(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    request: Result<Json<PreviewAnimeGroupingPolicyChangeRequest>, JsonRejection>,
) -> HttpResult<AnimeGroupingPolicyImpactResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::PreviewAnimeGroupingPolicyChange;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let scope = policy_scope(request.scope, capability, correlation_id)?;
    let change = policy_change(request.change, capability, correlation_id)?;
    let after_record_id = request
        .after_record_id
        .map(|value| value.parse::<RecordId>())
        .transpose()
        .map_err(|_| {
            invalid_input(
                capability,
                correlation_id,
                "/after_record_id",
                "preview cursor is invalid",
                "a canonical rec_ UUIDv7",
            )
        })?;
    let limit = IdentityImpactPageLimit::try_new(request.limit).ok_or_else(|| {
        invalid_input(
            capability,
            correlation_id,
            "/limit",
            "preview page limit is invalid",
            "an integer from 1 through 100",
        )
    })?;
    let authentication = application_request_authentication(
        &headers,
        state.browser_boundary.as_ref(),
        false,
        capability,
        correlation_id,
    )?;
    let kernel = state.kernel;
    let impact = run_kernel(capability, correlation_id, move || {
        let access = authenticate_application_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
        )?;
        let query = PreviewAnimeGroupingPolicyChangeQuery::try_new(
            correlation_id,
            access,
            scope,
            change,
            after_record_id,
            limit,
        )
        .map_err(|_| application_validation_problem(capability, correlation_id))?;
        kernel.authorize_and_preview_anime_grouping_policy_change(query)
    })
    .await?;
    Ok(Json(anime_grouping_policy_impact_response(&impact)))
}

#[utoipa::path(
    put,
    path = "/api/v1/profile/anime-grouping-policy",
    operation_id = "apply_anime_grouping_policy_change",
    tag = "profile",
    security(("credential_bearer" = []), ("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    request_body = ApplyAnimeGroupingPolicyChangeRequest,
    responses(
        (status = 200, description = "Anime projection policy change committed with an immutable receipt", body = ApplyAnimeGroupingPolicyChangeResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state write scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Expected revision or operation replay conflicts", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Policy change is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Policy state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Identity evidence exceeds a bounded local limit", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn apply_anime_grouping_policy_change(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    request: Result<Json<ApplyAnimeGroupingPolicyChangeRequest>, JsonRejection>,
) -> HttpResult<ApplyAnimeGroupingPolicyChangeResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ApplyAnimeGroupingPolicyChange;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let operation_id = request.operation_id.parse::<OperationId>().map_err(|_| {
        invalid_input(
            capability,
            correlation_id,
            "/operation_id",
            "operation ID is invalid",
            "a canonical op_ UUIDv7",
        )
    })?;
    let scope = policy_scope(request.scope, capability, correlation_id)?;
    let change = policy_change(request.change, capability, correlation_id)?;
    let semantic_digest = policy_semantic_digest(scope, request.expected_revision, change);
    let authentication = application_request_authentication(
        &headers,
        state.browser_boundary.as_ref(),
        true,
        capability,
        correlation_id,
    )?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        let access = authenticate_application_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
        )?;
        let command = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            correlation_id,
            access,
            scope,
            operation_id,
            semantic_digest,
            request.expected_revision,
            change,
        )
        .map_err(|_| application_validation_problem(capability, correlation_id))?;
        kernel.authorize_and_apply_anime_grouping_policy_change(command)
    })
    .await?;
    Ok(Json(apply_anime_grouping_policy_change_response(&outcome)))
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new()
        .route(
            "/api/v1/records/{record_id}/identity-route",
            get(resolve_identity_route),
        )
        .route(
            "/api/v1/profile/anime-grouping-policy",
            get(read_anime_grouping_policy).put(apply_anime_grouping_policy_change),
        )
        .route(
            "/api/v1/profile/anime-grouping-policy/preview",
            post(preview_anime_grouping_policy_change),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_digest_is_stable_and_binds_scope_revision_and_change() {
        let operation = OperationId::new_v7();
        let set = AnimeGroupingPolicyChange::Set(
            fasti_domain::AnimeGroupingPreference::KeepMalReleasesSeparate,
        );
        let first = policy_semantic_digest(AnimeGroupingPolicyScope::Profile, 3, set);
        assert_eq!(
            first,
            policy_semantic_digest(AnimeGroupingPolicyScope::Profile, 3, set)
        );
        assert_ne!(
            first,
            policy_semantic_digest(AnimeGroupingPolicyScope::Profile, 4, set)
        );
        assert_ne!(
            first,
            policy_semantic_digest(
                AnimeGroupingPolicyScope::Profile,
                3,
                AnimeGroupingPolicyChange::Rollback {
                    applied_operation_id: operation,
                },
            )
        );
    }
}
