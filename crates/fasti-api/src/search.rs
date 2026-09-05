//! Finite provider Search pages. Acquisition is a mutation, not Record creation.

use crate::local::{application_request_authentication, authenticate_application_request};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use crate::ProviderOperationLocks;
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection},
        DefaultBodyLimit, FromRequestParts, Path, State,
    },
    http::{header, request::Parts},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use fasti_application::{
    ApplicationAccessContext, BrowserRequestBoundaryPolicy, CapabilityKey, FastiProblem,
    LocalKernel, OutboundAccessPolicy, ProviderId, ProviderOperationLease, SearchPageRequest,
    SearchPersistencePort, SearchProviderQuery, Violation,
};
use fasti_contracts::{ProblemDetails, SearchProviderPageRequest, SearchProviderPageResponse};
use fasti_domain::{Grain, MetadataLocale, MetadataRegion, RequestCorrelationId, SearchQuery};
use fasti_provider_runtime::{ProviderSearchOutcome, ProviderSearchService};
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

pub(crate) struct SearchPageAccess {
    id: RequestCorrelationId,
    access: ApplicationAccessContext,
}

impl FromRequestParts<SearchApiState> for SearchPageAccess {
    type Rejection = HttpProblem;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SearchApiState,
    ) -> Result<Self, HttpProblem> {
        let id = RequestCorrelationId::new_v7();
        let authentication = application_request_authentication(
            &parts.headers,
            state.browser_boundary.as_ref(),
            true,
            CAPABILITY,
            id,
        )?;
        let kernel = Arc::clone(&state.kernel);
        let persistence = Arc::clone(&state.persistence);
        let access = tokio::task::spawn_blocking(move || {
            let access =
                authenticate_application_request(kernel.as_ref(), authentication, CAPABILITY, id)?;
            persistence.authorize_search_page_request(id, &access)?;
            Ok(access)
        })
        .await
        .map_err(|_| {
            application_problem(Box::new(FastiProblem::storage_unavailable(CAPABILITY, id)))
        })?
        .map_err(application_problem)?;
        Ok(Self { id, access })
    }
}

fn invalid(id: RequestCorrelationId, pointer: &'static str) -> HttpProblem {
    application_problem(Box::new(
        FastiProblem::validation_failed(
            CAPABILITY,
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
    SearchPageAccess { id, access }: SearchPageAccess,
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

pub(crate) fn router() -> Router<SearchApiState> {
    Router::new()
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
