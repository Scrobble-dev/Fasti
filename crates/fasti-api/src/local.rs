use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, State},
    http::{header, HeaderMap},
    routing::post,
    Json, Router,
};
use chrono::Utc;
use fasti_application::{
    ApplicationAccessContext, ApplicationResult, AuthenticateCredentialQuery,
    BrowserRequestBoundaryPolicy, BrowserSessionAccessContext, BrowserSessionMutationCommand,
    BrowserSessionQuery, CapabilityKey, EnrollFirstClientCommand, FastiProblem,
    InitializeNodeCommand, LocalKernel, ProblemCode, RequestAccessContext, SecretMaterial,
    ValidatedBrowserReadBoundary,
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
    pub(crate) browser_boundary: Option<BrowserRequestBoundaryPolicy>,
}

pub(crate) enum RequestAuthentication {
    Bearer(SecretMaterial),
}

pub(crate) enum ApplicationRequestAuthentication {
    Bearer(SecretMaterial),
    BrowserSession(BrowserSessionAccessContext),
}

pub(crate) const SESSION_COOKIE: &str = "__Host-fasti_session";
pub(crate) const CSRF_COOKIE: &str = "__Host-fasti_csrf";
pub(crate) const CSRF_HEADER: &str = "X-CSRF-Token";

type HttpResult<T> = Result<Json<T>, HttpProblem>;

/// Bootstrap secret header, checked before `initialize_node` runs. Loopback
/// reachability alone is not proof of authorization: any local process can
/// reach this port, and without this check a second process could race the
/// legitimate first client for the one-time bootstrap credential. Presenting
/// this value proves the caller can read a file this data root's OS user
/// owns -- see `AccessAdministrationPort::ensure_bootstrap_secret`.
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

/// Bearer-credential extraction shared by every capability-scoped route
/// (records, observations, integration webhooks). Returns
/// `AuthenticationFailed`, distinct from `bootstrap_secret`'s `Forbidden`
/// above -- a missing/malformed application credential and a missing/wrong
/// bootstrap secret are different failure classes with different
/// registry-declared problem codes.
pub(crate) fn bearer_secret(
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<SecretMaterial, HttpProblem> {
    if cookie_value(headers, SESSION_COOKIE, capability, correlation_id)?.is_some() {
        return Err(authentication_failed(capability, correlation_id));
    }
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

pub(crate) fn authentication_failed(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    application_problem(Box::new(FastiProblem::authentication_failed(
        capability,
        correlation_id,
    )))
}

fn browser_authentication_problem(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    mutation: bool,
) -> HttpProblem {
    let code = if mutation {
        ProblemCode::Forbidden
    } else {
        ProblemCode::BrowserSessionRevoked
    };
    application_problem(Box::new(FastiProblem::from_code(
        code,
        capability,
        correlation_id,
    )))
}

fn raw_cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
    let mut found = None;
    for header_value in headers.get_all(header::COOKIE) {
        let value = header_value.to_str().map_err(|_| ())?;
        for pair in value.split(';') {
            let Some((cookie_name, cookie_value)) = pair.trim().split_once('=') else {
                continue;
            };
            if cookie_name == name && found.replace(cookie_value).is_some() {
                return Err(());
            }
        }
    }
    Ok(found)
}

pub(crate) fn cookie_value<'a>(
    headers: &'a HeaderMap,
    name: &str,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<Option<&'a str>, HttpProblem> {
    raw_cookie_value(headers, name).map_err(|()| authentication_failed(capability, correlation_id))
}

fn browser_cookie_value<'a>(
    headers: &'a HeaderMap,
    name: &str,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    mutation: bool,
) -> Result<Option<&'a str>, HttpProblem> {
    raw_cookie_value(headers, name)
        .map_err(|()| browser_authentication_problem(capability, correlation_id, mutation))
}

pub(crate) fn browser_session_query(
    headers: &HeaderMap,
    boundary: &BrowserRequestBoundaryPolicy,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<(BrowserSessionQuery, ValidatedBrowserReadBoundary), HttpProblem> {
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(authentication_failed(capability, correlation_id));
    }
    let session = browser_cookie_value(headers, SESSION_COOKIE, capability, correlation_id, false)?
        .ok_or_else(|| browser_authentication_problem(capability, correlation_id, false))?;
    let session_secret = SecretMaterial::try_from_hex(session)
        .map_err(|_| browser_authentication_problem(capability, correlation_id, false))?;
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let validated = boundary
        .validate_read(host)
        .map_err(|_| browser_authentication_problem(capability, correlation_id, false))?;
    Ok((
        BrowserSessionQuery::new(correlation_id, session_secret, Utc::now()),
        validated,
    ))
}

pub(crate) fn browser_session_mutation_command(
    headers: &HeaderMap,
    boundary: &BrowserRequestBoundaryPolicy,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<BrowserSessionMutationCommand, HttpProblem> {
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(authentication_failed(capability, correlation_id));
    }
    let session = browser_cookie_value(headers, SESSION_COOKIE, capability, correlation_id, true)?
        .ok_or_else(|| browser_authentication_problem(capability, correlation_id, true))?;
    let session_secret = SecretMaterial::try_from_hex(session)
        .map_err(|_| browser_authentication_problem(capability, correlation_id, true))?;
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok());
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());
    let validated = boundary
        .validate(origin, host)
        .map_err(|_| browser_authentication_problem(capability, correlation_id, true))?;
    let csrf_cookie = browser_cookie_value(headers, CSRF_COOKIE, capability, correlation_id, true)?
        .ok_or_else(|| browser_authentication_problem(capability, correlation_id, true))?;
    let csrf_header = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| browser_authentication_problem(capability, correlation_id, true))?;
    let csrf_cookie = SecretMaterial::try_from_hex(csrf_cookie)
        .map_err(|_| browser_authentication_problem(capability, correlation_id, true))?;
    let csrf_header = SecretMaterial::try_from_hex(csrf_header)
        .map_err(|_| browser_authentication_problem(capability, correlation_id, true))?;
    if !csrf_cookie.constant_time_eq(&csrf_header) {
        return Err(browser_authentication_problem(
            capability,
            correlation_id,
            true,
        ));
    }
    Ok(BrowserSessionMutationCommand::new(
        correlation_id,
        session_secret,
        csrf_header,
        validated,
        Utc::now(),
    ))
}

pub(crate) fn application_request_authentication(
    headers: &HeaderMap,
    browser_boundary: Option<&BrowserRequestBoundaryPolicy>,
    mutation: bool,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<ApplicationRequestAuthentication, HttpProblem> {
    let session = cookie_value(headers, SESSION_COOKIE, capability, correlation_id)?;
    let bearer_present = headers.contains_key(header::AUTHORIZATION);
    if bearer_present && session.is_some() {
        return Err(authentication_failed(capability, correlation_id));
    }
    if bearer_present {
        return bearer_secret(headers, capability, correlation_id)
            .map(ApplicationRequestAuthentication::Bearer);
    }
    let boundary =
        browser_boundary.ok_or_else(|| authentication_failed(capability, correlation_id))?;
    let access = if mutation {
        BrowserSessionAccessContext::mutation(browser_session_mutation_command(
            headers,
            boundary,
            capability,
            correlation_id,
        )?)
    } else {
        let (query, validated) =
            browser_session_query(headers, boundary, capability, correlation_id)?;
        BrowserSessionAccessContext::read(query, validated)
    };
    Ok(ApplicationRequestAuthentication::BrowserSession(access))
}
pub(crate) fn request_authentication(
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<RequestAuthentication, HttpProblem> {
    bearer_secret(headers, capability, correlation_id).map(RequestAuthentication::Bearer)
}

pub(crate) fn authenticate_request(
    kernel: &dyn LocalKernel,
    authentication: RequestAuthentication,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<RequestAccessContext> {
    match authentication {
        RequestAuthentication::Bearer(secret) => kernel.authenticate_credential(
            AuthenticateCredentialQuery::new(correlation_id, capability, secret),
        ),
    }
}

pub(crate) fn authenticate_application_request(
    kernel: &dyn LocalKernel,
    authentication: ApplicationRequestAuthentication,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<ApplicationAccessContext> {
    match authentication {
        ApplicationRequestAuthentication::Bearer(secret) => kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                correlation_id,
                capability,
                secret,
            ))
            .map(Into::into),
        ApplicationRequestAuthentication::BrowserSession(access) => Ok(access.into()),
    }
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
    browser_boundary: Option<BrowserRequestBoundaryPolicy>,
) -> Router {
    let state = LocalApiState {
        kernel,
        browser_boundary,
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
