use crate::local::{authenticate_request, request_authentication, run_kernel, LocalApiState};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use fasti_application::{
    CapabilityKey, ClearNuvioCollectionsCommand, FastiProblem, GetNuvioCollectionsQuery,
    ReplaceNuvioCollectionsCommand, Violation,
};
use fasti_contracts::{NuvioCollectionsDocumentDto, NuvioCollectionsStateDto, ProblemDetails};
use fasti_domain::RequestCorrelationId;

type HttpResult<T> = Result<Json<T>, HttpProblem>;

fn invalid_document(
    correlation_id: RequestCorrelationId,
    error: fasti_application::NuvioCollectionsError,
) -> HttpProblem {
    let capability = CapabilityKey::ReplaceNuvioCollections;
    let violation = Violation::try_new(
        "invalid_nuvio_collections",
        error.pointer(),
        error.reason(),
        "a bounded Nuvio custom Collections array compatible with NuvioTV commit 3f44c404",
    )
    .expect("adapter-owned Nuvio Collections violation is valid");
    let problem = FastiProblem::validation_failed(capability, correlation_id, vec![violation])
        .expect("one Nuvio Collections violation is within bounds");
    application_problem(Box::new(problem))
}

fn state_dto(
    document: Option<&fasti_application::NuvioCollectionsDocument>,
) -> NuvioCollectionsStateDto {
    NuvioCollectionsStateDto {
        document: document.map(NuvioCollectionsDocumentDto::from_application),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/profile/nuvio-collections",
    tag = "profile",
    security(("credential_bearer" = []), ("browser_session" = [])),
    responses(
        (status = 200, description = "The authenticated profile's Nuvio custom Collections document", body = NuvioCollectionsStateDto),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn get_nuvio_collections(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> HttpResult<NuvioCollectionsStateDto> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::GetNuvioCollections;
    let authentication = request_authentication(&headers, capability, correlation_id, false)?;
    let kernel = state.kernel;
    let document = run_kernel(capability, correlation_id, move || {
        let access = authenticate_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
            false,
        )?;
        kernel.get_nuvio_collections(GetNuvioCollectionsQuery::new(correlation_id, access))
    })
    .await?;
    Ok(Json(state_dto(document.as_ref())))
}

#[utoipa::path(
    put,
    path = "/api/v1/profile/nuvio-collections",
    tag = "profile",
    security(("credential_bearer" = []), ("browser_session" = [])),
    request_body = NuvioCollectionsDocumentDto,
    responses(
        (status = 200, description = "The authenticated profile's normalized Nuvio custom Collections document", body = NuvioCollectionsStateDto),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state write scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Document does not satisfy the Nuvio Collections contract", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn replace_nuvio_collections(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    request: Result<Json<NuvioCollectionsDocumentDto>, JsonRejection>,
) -> HttpResult<NuvioCollectionsStateDto> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ReplaceNuvioCollections;
    let authentication = request_authentication(&headers, capability, correlation_id, true)?;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let document = request
        .into_application()
        .map_err(|error| invalid_document(correlation_id, error))?;
    let kernel = state.kernel;
    let document = run_kernel(capability, correlation_id, move || {
        let access = authenticate_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
            true,
        )?;
        kernel.replace_nuvio_collections(ReplaceNuvioCollectionsCommand::new(
            correlation_id,
            access,
            document,
        ))
    })
    .await?;
    Ok(Json(state_dto(Some(&document))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/profile/nuvio-collections",
    tag = "profile",
    security(("credential_bearer" = []), ("browser_session" = [])),
    responses(
        (status = 200, description = "The authenticated profile no longer has a Nuvio custom Collections document", body = NuvioCollectionsStateDto),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state write scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn clear_nuvio_collections(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> HttpResult<NuvioCollectionsStateDto> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ClearNuvioCollections;
    let authentication = request_authentication(&headers, capability, correlation_id, true)?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        let access = authenticate_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
            true,
        )?;
        kernel.clear_nuvio_collections(ClearNuvioCollectionsCommand::new(correlation_id, access))
    })
    .await?;
    Ok(Json(state_dto(None)))
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new().route(
        "/api/v1/profile/nuvio-collections",
        get(get_nuvio_collections)
            .put(replace_nuvio_collections)
            .delete(clear_nuvio_collections),
    )
}
