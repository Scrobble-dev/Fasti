use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, State},
    routing::post,
    Json, Router,
};
use fasti_application::{
    ApplicationResult, CapabilityKey, EnrollFirstClientCommand, FastiProblem,
    InitializeNodeCommand, LocalKernel, SecretMaterial,
};
use fasti_contracts::{
    ClientEnrollmentResponse, CredentialSchemeDto, EnrollFirstClientRequest, InitializeNodeRequest,
    NodeInitializationResponse, ProblemDetails,
};
use fasti_domain::RequestCorrelationId;
use std::sync::Arc;

const MAX_BOOTSTRAP_JSON_BODY_BYTES: usize = 4 * 1024;
const MAX_OBSERVATION_JSON_BODY_BYTES: usize = 64 * 1024;
const MAX_RECORDS_JSON_BODY_BYTES: usize = 8 * 1024;

#[derive(Clone)]
pub(crate) struct LocalApiState {
    pub(crate) kernel: Arc<dyn LocalKernel>,
}

type HttpResult<T> = Result<Json<T>, HttpProblem>;

#[utoipa::path(
    post,
    path = "/api/v1/node/initialization",
    tag = "node",
    request_body = InitializeNodeRequest,
    responses(
        (status = 200, description = "One-time durable initialization proof", body = NodeInitializationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Bootstrap authorization was denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Node was already initialized", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the local API bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "JSON does not match the request schema", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn initialize_node(
    State(state): State<LocalApiState>,
    request: Result<Json<InitializeNodeRequest>, JsonRejection>,
) -> HttpResult<NodeInitializationResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(_request) = request.map_err(|rejection| {
        json_rejection(CapabilityKey::InitializeNode, correlation_id, rejection)
    })?;
    let kernel = state.kernel;
    let outcome = run_kernel(CapabilityKey::InitializeNode, correlation_id, move || {
        kernel.initialize_node(InitializeNodeCommand::new(correlation_id))
    })
    .await?;

    Ok(Json(NodeInitializationResponse {
        initialization_proof: outcome.initialization_proof().expose_hex(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/client-enrollments",
    tag = "client",
    request_body = EnrollFirstClientRequest,
    responses(
        (status = 200, description = "One-time durable bearer credential", body = ClientEnrollmentResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Enrollment authorization was denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Initialization proof is invalid, expired, or consumed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the local API bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "JSON does not match the request schema", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn enroll_first_client(
    State(state): State<LocalApiState>,
    request: Result<Json<EnrollFirstClientRequest>, JsonRejection>,
) -> HttpResult<ClientEnrollmentResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) = request.map_err(|rejection| {
        json_rejection(CapabilityKey::EnrollFirstClient, correlation_id, rejection)
    })?;
    let proof = SecretMaterial::try_from_hex(&request.initialization_proof).map_err(|_| {
        application_problem(Box::new(FastiProblem::bootstrap_closed(correlation_id)))
    })?;
    let kernel = state.kernel;
    let outcome = run_kernel(
        CapabilityKey::EnrollFirstClient,
        correlation_id,
        move || kernel.enroll_first_client(EnrollFirstClientCommand::new(correlation_id, proof)),
    )
    .await?;

    Ok(Json(ClientEnrollmentResponse {
        credential_scheme: CredentialSchemeDto::Bearer,
        credential: outcome.credential().expose_hex(),
    }))
}

pub(crate) async fn run_kernel<T>(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    operation: impl FnOnce() -> ApplicationResult<T> + Send + 'static,
) -> Result<T, HttpProblem>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| {
            application_problem(Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            )))
        })?
        .map_err(application_problem)
}

pub(crate) fn router(kernel: Arc<dyn LocalKernel>) -> Router {
    let state = LocalApiState { kernel };
    let bootstrap = Router::new()
        .route("/api/v1/node/initialization", post(initialize_node))
        .route("/api/v1/client-enrollments", post(enroll_first_client))
        .layer(DefaultBodyLimit::max(MAX_BOOTSTRAP_JSON_BODY_BYTES));
    let observation =
        crate::observation::router().layer(DefaultBodyLimit::max(MAX_OBSERVATION_JSON_BODY_BYTES));
    let records =
        crate::records::router().layer(DefaultBodyLimit::max(MAX_RECORDS_JSON_BODY_BYTES));

    bootstrap
        .merge(observation)
        .merge(records)
        .with_state(state)
}
