use crate::{
    local::{cookie_secret, run_kernel, LocalApiState},
    problem::{application_problem, json_rejection, HttpProblem},
};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use fasti_application::{
    AuthenticateBrowserSessionQuery, BrowserPassword, BrowserUserView, BrowserUsername,
    CapabilityKey, CreateBrowserSessionCommand, DeleteBrowserUserCommand, EndBrowserSessionCommand,
    FastiProblem, ListBrowserUsersQuery, UpdateBrowserUserCommand, Violation,
};
use fasti_contracts::{
    BrowserSessionResponse, BrowserUserDto, CreateBrowserSessionRequest, DeleteBrowserUserRequest,
    ListBrowserUsersResponse, ProblemDetails, UpdateBrowserUserRequest,
};
use fasti_domain::{BrowserUserId, RequestCorrelationId};

const MAX_BROWSER_AUTH_JSON_BODY_BYTES: usize = 4 * 1024;

fn validation_problem(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    let violation = Violation::try_new(
        "invalid_browser_auth_input",
        "/",
        "browser authentication input is invalid",
        "the documented username, password, session lifetime, and user identifier bounds",
    )
    .expect("adapter-owned validation violation is valid");
    application_problem(Box::new(
        FastiProblem::validation_failed(capability, correlation_id, vec![violation])
            .expect("one validation violation is within bounds"),
    ))
}

fn user_dto(user: &BrowserUserView) -> BrowserUserDto {
    BrowserUserDto {
        user_id: user.user_id().to_string(),
        username: user.username().to_owned(),
        is_admin: user.is_admin(),
        is_test_account: user.is_test_account(),
        active: user.active(),
        created_at: user.created_at().to_rfc3339(),
        updated_at: user.updated_at().to_rfc3339(),
    }
}

fn no_store_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(header::VARY, HeaderValue::from_static("Cookie"));
    headers
}

fn append_cookie(headers: &mut HeaderMap, cookie: String) {
    headers.append(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("hex cookie attributes are valid"),
    );
}

fn session_cookie(value: &str, max_age_seconds: u32, secure: bool) -> String {
    format!(
        "fasti_session={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age_seconds}{}",
        if secure { "; Secure" } else { "" }
    )
}

fn csrf_cookie(value: &str, max_age_seconds: u32, secure: bool) -> String {
    format!(
        "fasti_csrf={value}; Path=/; SameSite=Strict; Max-Age={max_age_seconds}{}",
        if secure { "; Secure" } else { "" }
    )
}

fn clear_cookies(secure: bool) -> HeaderMap {
    let mut headers = no_store_headers();
    append_cookie(&mut headers, session_cookie("", 0, secure));
    append_cookie(&mut headers, csrf_cookie("", 0, secure));
    headers
}

fn csrf_from_request(
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<fasti_application::SecretMaterial, HttpProblem> {
    let cookie = cookie_secret(headers, "fasti_csrf", capability, correlation_id)?;
    let presented = headers
        .get("x-fasti-csrf")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| fasti_application::SecretMaterial::try_from_hex(value).ok())
        .ok_or_else(|| {
            application_problem(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )))
        })?;
    if !cookie.constant_time_eq(&presented) {
        return Err(application_problem(Box::new(FastiProblem::forbidden(
            capability,
            correlation_id,
        ))));
    }
    Ok(presented)
}

#[utoipa::path(
    post,
    path = "/api/v1/browser/session",
    tag = "browser authentication",
    request_body = CreateBrowserSessionRequest,
    responses(
        (status = 200, description = "An authenticated browser session", body = BrowserSessionResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Username or password is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Input does not satisfy the browser authentication contract", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn create_session(
    State(state): State<LocalApiState>,
    request: Result<Json<CreateBrowserSessionRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::CreateBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let username = BrowserUsername::try_new(request.username)
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let password = BrowserPassword::try_new(request.password)
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let command = CreateBrowserSessionCommand::try_new(
        correlation_id,
        username,
        password,
        request.session_timeout_minutes,
    )
    .map_err(|_| validation_problem(capability, correlation_id))?;
    let kernel = state.kernel;
    let secure_cookies = state.secure_cookies;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.create_browser_session(command)
    })
    .await?;
    let max_age_seconds = request.session_timeout_minutes.saturating_mul(60);
    let mut headers = no_store_headers();
    append_cookie(
        &mut headers,
        session_cookie(
            &outcome.session().expose_hex(),
            max_age_seconds,
            secure_cookies,
        ),
    );
    append_cookie(
        &mut headers,
        csrf_cookie(
            &outcome.csrf().expose_hex(),
            max_age_seconds,
            secure_cookies,
        ),
    );
    Ok((
        headers,
        Json(BrowserSessionResponse {
            user: user_dto(outcome.user()),
            expires_at: outcome.expires_at().to_rfc3339(),
        }),
    )
        .into_response())
}

#[utoipa::path(
    get,
    path = "/api/v1/browser/session",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 200, description = "The current authenticated browser session", body = BrowserSessionResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Session no longer has active access", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_session(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.authenticate_browser_session(AuthenticateBrowserSessionQuery::new(
            correlation_id,
            capability,
            session,
            None,
            false,
        ))
    })
    .await?;
    Ok((
        no_store_headers(),
        Json(BrowserSessionResponse {
            user: user_dto(outcome.user()),
            expires_at: outcome.expires_at().to_rfc3339(),
        }),
    )
        .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/browser/session",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 204, description = "Browser session ended"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn end_session(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::EndBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    let secure_cookies = state.secure_cookies;
    run_kernel(capability, correlation_id, move || {
        kernel.end_browser_session(EndBrowserSessionCommand::new(correlation_id, session, csrf))
    })
    .await?;
    Ok((clear_cookies(secure_cookies), StatusCode::NO_CONTENT).into_response())
}

#[utoipa::path(
    get,
    path = "/api/v1/browser/users",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 200, description = "Browser users visible to an administrator", body = ListBrowserUsersResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Browser session lacks user-management access", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn list_users(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ListBrowserUsers;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let kernel = state.kernel;
    let users = run_kernel(capability, correlation_id, move || {
        kernel.list_browser_users(ListBrowserUsersQuery::new(correlation_id, session))
    })
    .await?;
    Ok((
        no_store_headers(),
        Json(ListBrowserUsersResponse {
            users: users.iter().map(user_dto).collect(),
        }),
    )
        .into_response())
}

#[utoipa::path(
    patch,
    path = "/api/v1/browser/users/{user_id}",
    tag = "browser authentication",
    security(("browser_session" = [])),
    params(("user_id" = String, Path, description = "Browser user identifier")),
    request_body = UpdateBrowserUserRequest,
    responses(
        (status = 200, description = "Updated browser user", body = BrowserUserDto),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Administration, CSRF, or password re-authentication failed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Input or target user is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn update_user(
    State(state): State<LocalApiState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<UpdateBrowserUserRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::UpdateBrowserUser;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let target_user_id = user_id
        .parse::<BrowserUserId>()
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let current_password = BrowserPassword::try_new(request.current_password)
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let username = request
        .username
        .map(BrowserUsername::try_new)
        .transpose()
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let password = request
        .password
        .map(BrowserPassword::try_new)
        .transpose()
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let command = UpdateBrowserUserCommand::try_new(
        correlation_id,
        session,
        csrf,
        target_user_id,
        current_password,
        username,
        password,
        request.active,
    )
    .map_err(|_| validation_problem(capability, correlation_id))?;
    let kernel = state.kernel;
    let user = run_kernel(capability, correlation_id, move || {
        kernel.update_browser_user(command)
    })
    .await?;
    Ok((no_store_headers(), Json(user_dto(&user))).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/browser/users/{user_id}",
    tag = "browser authentication",
    security(("browser_session" = [])),
    params(("user_id" = String, Path, description = "Browser user identifier")),
    request_body = DeleteBrowserUserRequest,
    responses(
        (status = 204, description = "Browser user deleted"),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Administration, CSRF, or password re-authentication failed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Target user is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn delete_user(
    State(state): State<LocalApiState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<DeleteBrowserUserRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::DeleteBrowserUser;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let target_user_id = user_id
        .parse::<BrowserUserId>()
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let current_password = BrowserPassword::try_new(request.current_password)
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.delete_browser_user(DeleteBrowserUserCommand::new(
            correlation_id,
            session,
            csrf,
            target_user_id,
            current_password,
        ))
    })
    .await?;
    Ok((clear_cookies(state.secure_cookies), StatusCode::NO_CONTENT).into_response())
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new()
        .route(
            "/api/v1/browser/session",
            post(create_session).get(read_session).delete(end_session),
        )
        .route("/api/v1/browser/users", get(list_users))
        .route(
            "/api/v1/browser/users/{user_id}",
            patch(update_user).delete(delete_user),
        )
        .layer(axum::extract::DefaultBodyLimit::max(
            MAX_BROWSER_AUTH_JSON_BODY_BYTES,
        ))
}
