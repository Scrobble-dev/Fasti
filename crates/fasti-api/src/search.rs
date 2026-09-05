//! Finite provider Search pages. Acquisition is a mutation, not Record creation.

use crate::local::{application_request_authentication, authenticate_application_request};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use crate::ProviderOperationLocks;
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection, QueryRejection},
        DefaultBodyLimit, FromRequestParts, Path, Query, State,
    },
    http::{header, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use fasti_application::{
    ApplicationAccessContext, BrowserRequestBoundaryPolicy, CapabilityKey, FastiProblem,
    LocalKernel, OutboundAccessPolicy, ProviderId, ProviderOperationLease,
    ReadSearchCandidateRequest, SearchPageRequest, SearchPersistencePort, SearchProviderQuery,
    Violation,
};
use fasti_contracts::{
    ProblemDetails, SearchCandidateDetailsQueryParameters, SearchCandidateDetailsResponse,
    SearchProviderPageRequest, SearchProviderPageResponse,
};
use fasti_domain::{Grain, MetadataLocale, MetadataRegion, RequestCorrelationId, SearchQuery};
use fasti_provider_runtime::{
    ProviderCandidateDetailsOutcome, ProviderSearchOutcome, ProviderSearchService,
};
use std::sync::Arc;

const CAPABILITY: CapabilityKey = CapabilityKey::SearchMetadata;
const MAX_SEARCH_JSON_BODY_BYTES: usize = 8 * 1024;

#[derive(Clone)]
pub(crate) struct SearchApiState {
    pub(crate) kernel: Arc<dyn LocalKernel>,
    pub(crate) persistence: Arc<dyn SearchPersistencePort>,
    pub(crate) service: Arc<ProviderSearchService>,
    pub(crate) locks: ProviderOperationLocks,
    pub(crate) browser_boundary: Option<BrowserRequestBoundaryPolicy>,
}

pub(crate) struct SearchAccess<const MUTATION: bool> {
    id: RequestCorrelationId,
    access: ApplicationAccessContext,
}

impl<const MUTATION: bool> FromRequestParts<SearchApiState> for SearchAccess<MUTATION> {
    type Rejection = HttpProblem;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SearchApiState,
    ) -> Result<Self, HttpProblem> {
        let (id, access) = authorize(parts, state, CAPABILITY, MUTATION).await?;
        Ok(Self { id, access })
    }
}

pub(crate) struct SearchActionAccess {
    id: RequestCorrelationId,
    access: ApplicationAccessContext,
}

impl FromRequestParts<SearchApiState> for SearchActionAccess {
    type Rejection = HttpProblem;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &SearchApiState,
    ) -> Result<Self, HttpProblem> {
        let (id, access) = authorize(parts, state, CapabilityKey::AttachIdentifier, true).await?;
        Ok(Self { id, access })
    }
}

async fn authorize(
    parts: &Parts,
    state: &SearchApiState,
    capability: CapabilityKey,
    mutation: bool,
) -> Result<(RequestCorrelationId, ApplicationAccessContext), HttpProblem> {
    let id = RequestCorrelationId::new_v7();
    let authentication = application_request_authentication(
        &parts.headers,
        state.browser_boundary.as_ref(),
        mutation,
        capability,
        id,
    )?;
    let kernel = Arc::clone(&state.kernel);
    let persistence = Arc::clone(&state.persistence);
    let access = tokio::task::spawn_blocking(move || {
        let access =
            authenticate_application_request(kernel.as_ref(), authentication, capability, id)?;
        if capability == CapabilityKey::AttachIdentifier {
            persistence.authorize_search_candidate_action_request(id, &access)?;
        } else if mutation {
            persistence.authorize_search_page_request(id, &access)?;
        } else {
            persistence.authorize_search_candidate_read_request(id, &access)?;
        }
        Ok(access)
    })
    .await
    .map_err(|_| application_problem(Box::new(FastiProblem::storage_unavailable(capability, id))))?
    .map_err(application_problem)?;
    Ok((id, access))
}

fn invalid(id: RequestCorrelationId, pointer: &'static str) -> HttpProblem {
    invalid_for(CAPABILITY, id, pointer)
}

fn invalid_for(
    capability: CapabilityKey,
    id: RequestCorrelationId,
    pointer: &'static str,
) -> HttpProblem {
    application_problem(Box::new(
        FastiProblem::validation_failed(
            capability,
            id,
            vec![Violation::try_new(
                "invalid_search_input",
                pointer,
                "Search input is invalid",
                "a bounded canonical Search value",
            )
            .expect("static Search violation")],
        )
        .expect("one bounded Search violation"),
    ))
}

fn query(
    provider: String,
    request: SearchProviderPageRequest,
    id: RequestCorrelationId,
) -> Result<SearchProviderQuery, HttpProblem> {
    let text = SearchQuery::try_new(request.query).map_err(|_| invalid(id, "/query"))?;
    let provider = ProviderId::try_new(provider).map_err(|_| invalid(id, "/provider_id"))?;
    let locale = request
        .locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| invalid(id, "/locale"))?;
    let region = request
        .region
        .map(MetadataRegion::try_new)
        .transpose()
        .map_err(|_| invalid(id, "/region"))?;
    if request.grains.len() > 32 {
        return Err(invalid(id, "/grains"));
    }
    let grains = request
        .grains
        .into_iter()
        .map(|value| value.parse::<Grain>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid(id, "/grains"))?;
    SearchProviderQuery::try_new(text, provider, request.page, locale, region, grains)
        .map_err(|_| invalid(id, "/page"))
}

#[utoipa::path(
    post,
    path = "/api/v1/search/providers/{provider_id}",
    operation_id = "search_provider_page",
    tag = "search",
    params(("provider_id" = String, Path, description = "Registered provider identity")),
    security(("credential_bearer" = []), ("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    request_body(content = SearchProviderPageRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Provider page or explicit source-unavailable outcome; no Record was created", body = SearchProviderPageResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Authentication is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Search authority or browser boundary denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Current session or receipt authority changed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request exceeds the Search body limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Search input", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Receipt state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Provider capability is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Receipt capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn search_provider_page(
    State(state): State<SearchApiState>,
    SearchAccess { id, access }: SearchAccess<true>,
    provider: Result<Path<String>, PathRejection>,
    body: Result<Json<SearchProviderPageRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let provider = provider.map_err(|_| invalid(id, "/provider_id"))?.0;
    let request = body
        .map_err(|error| json_rejection(CAPABILITY, id, error))?
        .0;
    let offline = request.offline;
    let page_number = request.page;
    let query = query(provider.clone(), request, id)?;
    let gate = state
        .locks
        .get(&provider)
        .ok_or_else(|| invalid(id, "/provider_id"))?;
    let lease = ProviderOperationLease::new(gate.lock_owned().await);
    let outcome = state
        .service
        .search_page(
            SearchPageRequest {
                correlation_id: id,
                access,
                query,
                outbound_policy: OutboundAccessPolicy::default(),
                // The service replaces this with its trusted provider cache-policy revision.
                terms_revision: String::new(),
            },
            offline,
            lease,
        )
        .await
        .map_err(application_problem)?;
    let response = match outcome {
        ProviderSearchOutcome::Page {
            page,
            upstream_problem,
        } => SearchProviderPageResponse::Page {
            provider_id: provider,
            page: page_number,
            candidates: page.candidates.iter().map(Into::into).collect(),
            next_page: page.next_page,
            cache_state: page.cache_state.into(),
            lifetime: (&page.lifetime).into(),
            upstream_problem: upstream_problem.map(|code| code.as_str().to_owned()),
        },
        ProviderSearchOutcome::Unavailable { problem } => SearchProviderPageResponse::Unavailable {
            provider_id: provider,
            problem_code: problem.as_str().to_owned(),
        },
    };
    Ok((
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(response),
    )
        .into_response())
}

#[utoipa::path(
    get,
    path = "/api/v1/search/candidates/{provider_id}/{grain}/{candidate_receipt_id}",
    operation_id = "read_search_candidate",
    tag = "search",
    params(
        ("provider_id" = String, Path, description = "Registered provider identity"),
        ("grain" = String, Path, description = "Canonical candidate grain"),
        ("candidate_receipt_id" = String, Path, description = "Opaque Search candidate receipt identity"),
        SearchCandidateDetailsQueryParameters
    ),
    security(("credential_bearer" = []), ("browser_session_cookie" = [])),
    responses(
        (status = 200, description = "Original snapshot, refetched details, source failure with snapshot, or non-enumerating missing outcome", body = SearchCandidateDetailsResponse),
        (status = 400, description = "Malformed request", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request exceeds the Search body limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Unsupported request content type", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Authentication is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Search authority or browser boundary denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Current session or receipt authority changed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Invalid candidate locator or query", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Receipt state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Provider capability is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Receipt capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_search_candidate(
    State(state): State<SearchApiState>,
    SearchAccess { id, access }: SearchAccess<false>,
    locator: Result<Path<(String, String, String)>, PathRejection>,
    query: Result<Query<SearchCandidateDetailsQueryParameters>, QueryRejection>,
) -> Result<Response, HttpProblem> {
    let (provider, grain, receipt) = locator.map_err(|_| invalid(id, "/candidate_receipt_id"))?.0;
    let query = query.map_err(|_| invalid(id, "/query"))?.0;
    let request = ReadSearchCandidateRequest {
        correlation_id: id,
        access,
        candidate_receipt_id: receipt
            .parse()
            .map_err(|_| invalid(id, "/candidate_receipt_id"))?,
        provider: ProviderId::try_new(provider.clone()).map_err(|_| invalid(id, "/provider_id"))?,
        grain: grain.parse().map_err(|_| invalid(id, "/grain"))?,
        outbound_policy: OutboundAccessPolicy::default(),
        // Replaced by the service's trusted descriptor, never caller input.
        terms_revision: String::new(),
    };
    let gate = state
        .locks
        .get(&provider)
        .ok_or_else(|| invalid(id, "/provider_id"))?;
    let lease = ProviderOperationLease::new(gate.lock_owned().await);
    let outcome = state
        .service
        .candidate_details(request, query.offline, lease)
        .await
        .map_err(application_problem)?;
    let response = match outcome {
        None => SearchCandidateDetailsResponse::Missing {},
        Some(ProviderCandidateDetailsOutcome::Snapshot(snapshot)) => {
            SearchCandidateDetailsResponse::Snapshot {
                snapshot: (&snapshot).into(),
            }
        }
        Some(ProviderCandidateDetailsOutcome::Unavailable { snapshot, problem }) => {
            SearchCandidateDetailsResponse::Unavailable {
                snapshot: (&snapshot).into(),
                problem_code: problem.as_str().to_owned(),
            }
        }
        Some(ProviderCandidateDetailsOutcome::Refetched {
            snapshot,
            details,
            locale,
        }) => {
            let evidence = details.search_evidence().map_err(|_| {
                application_problem(Box::new(FastiProblem::from_code(
                    fasti_application::ProblemCode::ProviderResponseInvalid,
                    CAPABILITY,
                    id,
                )))
            })?;
            SearchCandidateDetailsResponse::Refetched {
                snapshot: (&snapshot).into(),
                details: (&evidence).into(),
                locale: locale.map(|value| value.as_str().to_owned()),
            }
        }
    };
    Ok((
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(response),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/search/candidates/{provider_id}/{grain}/{candidate_receipt_id}/actions",
    operation_id = "save_search_candidate",
    tag = "search",
    params(
        ("provider_id" = String, Path, description = "Registered provider identity"),
        ("grain" = String, Path, description = "Canonical candidate grain"),
        ("candidate_receipt_id" = String, Path, description = "Opaque Search candidate receipt identity")
    ),
    security(("credential_bearer" = []), ("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    request_body(content = fasti_contracts::SearchCandidateActionRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Atomic Record action acceptance or explicit source failure; saved receipt is historical, including on replay", body = fasti_contracts::SearchCandidateActionResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Authentication is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Current identity-write authority, new-save Search authority or browser boundary denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "Target Record is not accessible", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Operation intent, identity or session state conflicts", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request exceeds the Search body limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Invalid candidate locator or action intent", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Receipt state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Provider capability is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn save_search_candidate(
    State(state): State<SearchApiState>,
    SearchActionAccess { id, access }: SearchActionAccess,
    locator: Result<Path<(String, String, String)>, PathRejection>,
    body: Result<Json<fasti_contracts::SearchCandidateActionRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::AttachIdentifier;
    let invalid = |pointer| invalid_for(capability, id, pointer);
    let (provider, grain, receipt) = locator.map_err(|_| invalid("/candidate_receipt_id"))?.0;
    let body = body
        .map_err(|error| json_rejection(capability, id, error))?
        .0;
    let action = match body.action {
        fasti_contracts::SearchRecordActionDto::Create {} => {
            fasti_application::SearchRecordAction::Create
        }
        fasti_contracts::SearchRecordActionDto::Attach { record_id } => {
            fasti_application::SearchRecordAction::Attach(
                record_id
                    .parse()
                    .map_err(|_| invalid("/action/record_id"))?,
            )
        }
    };
    let command = fasti_application::SearchCandidateActionCommand {
        request: ReadSearchCandidateRequest {
            correlation_id: id,
            access,
            candidate_receipt_id: receipt
                .parse()
                .map_err(|_| invalid("/candidate_receipt_id"))?,
            provider: ProviderId::try_new(provider.clone()).map_err(|_| invalid("/provider_id"))?,
            grain: grain.parse().map_err(|_| invalid("/grain"))?,
            outbound_policy: OutboundAccessPolicy::default(),
            terms_revision: String::new(),
        },
        operation_id: body
            .operation_id
            .parse()
            .map_err(|_| invalid("/operation_id"))?,
        action,
        evidence_mode: body.evidence_mode.into(),
    };
    let gate = state
        .locks
        .get(&provider)
        .ok_or_else(|| invalid("/provider_id"))?;
    let lease = ProviderOperationLease::new(gate.lock_owned().await);
    let outcome = state
        .service
        .save_candidate(command, lease)
        .await
        .map_err(application_problem)?;
    let response = match outcome {
        fasti_provider_runtime::ProviderSearchActionOutcome::Saved(receipt) => {
            fasti_contracts::SearchCandidateActionResponse::Saved {
                receipt: receipt.as_ref().try_into().map_err(|_| {
                    application_problem(Box::new(FastiProblem::from_code(
                        fasti_application::ProblemCode::IntegrityFailed,
                        capability,
                        id,
                    )))
                })?,
            }
        }
        fasti_provider_runtime::ProviderSearchActionOutcome::Unavailable { problem } => {
            fasti_contracts::SearchCandidateActionResponse::Unavailable {
                problem_code: problem.as_str().to_owned(),
            }
        }
    };
    Ok((
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(response),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/search/records",
    operation_id = "search_local_records",
    tag = "search",
    security(("credential_bearer" = []), ("browser_session_cookie" = [])),
    request_body(content = fasti_contracts::LocalSearchRequestDto, content_type = "application/json"),
    responses(
        (status = 200, description = "Complete local Record projections and inspected-position continuation; no provider access", body = fasti_contracts::LocalSearchResponseDto),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Authentication is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Search authority or browser read boundary denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Session authority changed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request exceeds the Search body limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Invalid Search query, grain or continuation", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Local state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Search capability is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "One complete Record exceeds the bounded page capacity; no evidence was omitted", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn search_local_records(
    State(state): State<SearchApiState>,
    SearchAccess { id, access }: SearchAccess<false>,
    body: Result<Json<fasti_contracts::LocalSearchRequestDto>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let body = body
        .map_err(|error| json_rejection(CAPABILITY, id, error))?
        .0;
    if body.grains.len() > Grain::ALL.len() {
        return Err(invalid(id, "/grains"));
    }
    let request = fasti_application::LocalSearchRequest {
        correlation_id: id,
        access,
        query: SearchQuery::try_new(body.query).map_err(|_| invalid(id, "/query"))?,
        grains: body
            .grains
            .into_iter()
            .map(|grain| grain.parse::<Grain>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| invalid(id, "/grains"))?,
        after: body
            .after
            .map(|cursor| {
                Ok::<_, HttpProblem>(fasti_application::LocalSearchCursor {
                    last_record_id: cursor
                        .last_record_id
                        .parse()
                        .map_err(|_| invalid(id, "/after/last_record_id"))?,
                    context_digest: cursor
                        .context_digest
                        .parse()
                        .map_err(|_| invalid(id, "/after/context_digest"))?,
                })
            })
            .transpose()?,
    };
    let persistence = Arc::clone(&state.persistence);
    // Only the existing local index/metadata owner runs. No provider gate or
    // credential access is needed, including while other sources are offline.
    let bytes = tokio::task::spawn_blocking(move || {
        let page = persistence.search_local_records(&request)?;
        let response = fasti_contracts::LocalSearchResponseDto {
            records: page
                .records
                .into_iter()
                .map(crate::records::record_summary_dto)
                .collect(),
            next: page.next.map(Into::into),
        };
        let mut buffer = vec![0; fasti_application::MAX_LOCAL_SEARCH_RESPONSE_BYTES];
        let mut remaining = buffer.as_mut_slice();
        serde_json::to_writer(&mut remaining, &response).map_err(|_| {
            Box::new(FastiProblem::from_code(
                fasti_application::ProblemCode::CapacityExceeded,
                CAPABILITY,
                id,
            ))
        })?;
        let written = fasti_application::MAX_LOCAL_SEARCH_RESPONSE_BYTES - remaining.len();
        buffer.truncate(written);
        buffer.shrink_to_fit();
        Ok::<_, Box<FastiProblem>>(buffer)
    })
    .await
    .map_err(|_| application_problem(Box::new(FastiProblem::storage_unavailable(CAPABILITY, id))))?
    .map_err(application_problem)?;
    Ok((
        [
            (header::CACHE_CONTROL, "private, no-store"),
            (header::CONTENT_TYPE, "application/json"),
        ],
        bytes,
    )
        .into_response())
}

pub(crate) fn router() -> Router<SearchApiState> {
    Router::new()
        .route("/api/v1/search/records", post(search_local_records))
        .route(
            "/api/v1/search/candidates/{provider_id}/{grain}/{candidate_receipt_id}/actions",
            post(save_search_candidate),
        )
        .route(
            "/api/v1/search/candidates/{provider_id}/{grain}/{candidate_receipt_id}",
            get(read_search_candidate),
        )
        .route(
            "/api/v1/search/providers/{provider_id}",
            post(search_provider_page),
        )
        .layer(DefaultBodyLimit::max(MAX_SEARCH_JSON_BODY_BYTES))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_query_reuses_byte_bounds_and_domain_grains() {
        let request = |text: String, page| SearchProviderPageRequest {
            query: text,
            page,
            locale: None,
            region: None,
            grains: vec![],
            offline: true,
        };
        let id = RequestCorrelationId::new_v7();
        assert!(query("tmdb".into(), request("海".repeat(85), 1), id).is_ok());
        assert!(query("tmdb".into(), request("海".repeat(86), 1), id).is_err());
        assert!(query("tmdb".into(), request("Dune".into(), 0), id).is_err());
    }
}
