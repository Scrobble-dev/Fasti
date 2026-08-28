use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, State},
    http::{header, HeaderMap},
    routing::post,
    Json, Router,
};
use fasti_application::{
    ApplicationResult, AuthenticateBrowserSessionQuery, AuthenticateCredentialQuery, CapabilityKey,
    EnrollFirstClientCommand, FastiProblem, InitializeNodeCommand, LocalKernel,
    RequestAccessContext, SecretMaterial,
};
use fasti_contracts::{
    ClientEnrollmentResponse, CredentialSchemeDto, EnrollFirstClientRequest, InitializeNodeRequest,
    NodeInitializationResponse, ProblemDetails,
};
use fasti_domain::RequestCorrelationId;
use std::sync::Arc;

const MAX_BOOTSTRAP_JSON_BODY_BYTES: usize = 4 * 1024;
const MAX_OBSERVATION_JSON_BODY_BYTES: usize = 64 * 1024;
const MAX_NUVIO_COLLECTIONS_JSON_BODY_BYTES: usize =
    fasti_application::MAX_NUVIO_COLLECTIONS_JSON_BYTES;
const MAX_RECORDS_JSON_BODY_BYTES: usize = 8 * 1024;

#[derive(Clone)]
pub(crate) struct LocalApiState {
    pub(crate) kernel: Arc<dyn LocalKernel>,
    pub(crate) secure_cookies: bool,
}

pub(crate) enum RequestAuthentication {
    Bearer(SecretMaterial),
    Browser {
        session: SecretMaterial,
        csrf: Option<SecretMaterial>,
    },
}

type HttpResult<T> = Result<Json<T>, HttpProblem>;

/// Bootstrap secret header, checked before `initialize_node` runs. Loopback
/// reachability alone is not proof of authorization: any local process can
/// reach this port, and without this check a second process could race the
/// legitimate first client for the one-time bootstrap credential. Presenting
/// this value proves the caller can read a file this data root's OS user
/// owns -- see `AccessAdministrationPort::ensure_bootstrap_secret`.
/// Bearer-credential extraction shared by every capability-scoped route
/// (records, observations). Returns `AuthenticationFailed`, distinct from
/// `bootstrap_secret`'s `Forbidden` below -- a missing/malformed application
/// credential and a missing/wrong bootstrap secret are different failure
/// classes with different registry-declared problem codes.
pub(crate) fn bearer_secret(
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<SecretMaterial, HttpProblem> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty() && !value.chars().any(char::is_whitespace))
        .ok_or_else(|| {
            application_problem(Box::new(FastiProblem::authentication_failed(
                capability,
                correlation_id,
            )))
        })?;
    SecretMaterial::try_from_hex(token).map_err(|_| {
        application_problem(Box::new(FastiProblem::authentication_failed(
            capability,
            correlation_id,
        )))
    })
}

pub(crate) fn cookie_secret(
    headers: &HeaderMap,
    name: &str,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<SecretMaterial, HttpProblem> {
    let mut values = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .filter_map(|(key, value)| (key == name).then_some(value));
    let value = values
        .next()
        .filter(|_| values.next().is_none())
        .ok_or_else(|| {
            application_problem(Box::new(FastiProblem::authentication_failed(
                capability,
                correlation_id,
            )))
        })?;
    SecretMaterial::try_from_hex(value).map_err(|_| {
        application_problem(Box::new(FastiProblem::authentication_failed(
            capability,
            correlation_id,
        )))
    })
}

fn csrf_secret(
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<SecretMaterial, HttpProblem> {
    let cookie = cookie_secret(headers, "fasti_csrf", capability, correlation_id)?;
    let header = headers
        .get("x-fasti-csrf")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| SecretMaterial::try_from_hex(value).ok())
        .ok_or_else(|| {
            application_problem(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )))
        })?;
    if !cookie.constant_time_eq(&header) {
        return Err(application_problem(Box::new(FastiProblem::forbidden(
            capability,
            correlation_id,
        ))));
    }
    Ok(header)
}

pub(crate) fn request_authentication(
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    mutation: bool,
) -> Result<RequestAuthentication, HttpProblem> {
    if headers.contains_key(header::AUTHORIZATION) {
        return bearer_secret(headers, capability, correlation_id)
            .map(RequestAuthentication::Bearer);
    }
    Ok(RequestAuthentication::Browser {
        session: cookie_secret(headers, "fasti_session", capability, correlation_id)?,
        csrf: mutation
            .then(|| csrf_secret(headers, capability, correlation_id))
            .transpose()?,
    })
}

pub(crate) fn authenticate_request(
    kernel: &dyn LocalKernel,
    authentication: RequestAuthentication,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    mutation: bool,
) -> ApplicationResult<RequestAccessContext> {
    match authentication {
        RequestAuthentication::Bearer(secret) => kernel.authenticate_credential(
            AuthenticateCredentialQuery::new(correlation_id, capability, secret),
        ),
        RequestAuthentication::Browser { session, csrf } => kernel
            .authenticate_browser_session(AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session,
                csrf,
                mutation,
            ))
            .map(|authenticated| *authenticated.access()),
    }
}

fn bootstrap_secret(
    headers: &HeaderMap,
    correlation_id: RequestCorrelationId,
) -> Result<SecretMaterial, HttpProblem> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty() && !value.chars().any(char::is_whitespace))
        .ok_or_else(|| {
            application_problem(Box::new(FastiProblem::forbidden(
                CapabilityKey::InitializeNode,
                correlation_id,
            )))
        })?;
    SecretMaterial::try_from_hex(token).map_err(|_| {
        application_problem(Box::new(FastiProblem::forbidden(
            CapabilityKey::InitializeNode,
            correlation_id,
        )))
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/node/initialization",
    tag = "node",
    security(("bootstrap_bearer" = [])),
    request_body = InitializeNodeRequest,
    responses(
        (status = 200, description = "One-time durable initialization proof", body = NodeInitializationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Bootstrap authorization was denied -- the bootstrap secret header is missing, malformed, or does not match the secret at <data_root>/bootstrap.secret", body = ProblemDetails, content_type = "application/problem+json"),
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
    headers: HeaderMap,
    request: Result<Json<InitializeNodeRequest>, JsonRejection>,
) -> HttpResult<NodeInitializationResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(_request) = request.map_err(|rejection| {
        json_rejection(CapabilityKey::InitializeNode, correlation_id, rejection)
    })?;
    let presented = bootstrap_secret(&headers, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(CapabilityKey::InitializeNode, correlation_id, move || {
        let expected = kernel.ensure_bootstrap_secret()?;
        if !expected.constant_time_eq(&presented) {
            return Err(Box::new(FastiProblem::forbidden(
                CapabilityKey::InitializeNode,
                correlation_id,
            )));
        }
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

pub(crate) fn router(
    kernel: Arc<dyn LocalKernel>,
    include_bootstrap: bool,
    secure_cookies: bool,
) -> Router {
    let state = LocalApiState {
        kernel,
        secure_cookies,
    };
    let bootstrap = Router::new()
        .route("/api/v1/node/initialization", post(initialize_node))
        .route("/api/v1/client-enrollments", post(enroll_first_client))
        .layer(DefaultBodyLimit::max(MAX_BOOTSTRAP_JSON_BODY_BYTES));
    let observation =
        crate::observation::router().layer(DefaultBodyLimit::max(MAX_OBSERVATION_JSON_BODY_BYTES));
    let records =
        crate::records::router().layer(DefaultBodyLimit::max(MAX_RECORDS_JSON_BODY_BYTES));
    let profile_state =
        crate::profile_state::router().layer(DefaultBodyLimit::max(MAX_RECORDS_JSON_BODY_BYTES));
    let nuvio_collections = crate::nuvio_collections::router()
        .layer(DefaultBodyLimit::max(MAX_NUVIO_COLLECTIONS_JSON_BODY_BYTES));

    let routes = Router::new()
        .merge(crate::browser_auth::router())
        .merge(observation)
        .merge(records)
        .merge(profile_state)
        .merge(nuvio_collections);
    if include_bootstrap {
        bootstrap.merge(routes).with_state(state)
    } else {
        routes.with_state(state)
    }
}
