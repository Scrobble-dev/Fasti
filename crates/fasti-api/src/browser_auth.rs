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
    AuthenticateBrowserSessionQuery, BeginPasskeyRegistrationQuery, BrowserPassword,
    BrowserUserView, BrowserUsername, CapabilityKey, CompletePasskeyRegistrationCommand,
    CreateBrowserSessionCommand, DeleteBrowserUserCommand, DeleteOidcConfigCommand,
    DeletePasskeyCommand, DisableTotpCommand, DiscoverOidcQuery, EndAllOtherBrowserSessionsCommand,
    EndBrowserSessionCommand, EndSpecificBrowserSessionCommand, EnrollTotpBeginCommand,
    EnrollTotpConfirmCommand, FastiProblem, GetOidcConfigQuery, ListBrowserSessionsQuery,
    ListBrowserUsersQuery, ListPasskeysQuery, SaveOidcConfigCommand,
    SwitchBrowserSessionProfileCommand, UpdateBrowserUserCommand, Violation,
};
use fasti_contracts::{
    BeginPasskeyRegistrationResponse, BrowserSessionItemDto, BrowserSessionResponse,
    BrowserUserDto, CompletePasskeyRegistrationRequest, ConfirmTotpRequest,
    CreateBrowserSessionRequest, DeleteBrowserUserRequest, DisableTotpRequest, EnrollTotpResponse,
    ListBrowserSessionsResponse, ListBrowserUsersResponse, ListPasskeysResponse, OidcConfigDto,
    OidcDiscoveryRequest, OidcDiscoveryResponse, PasskeyDto, ProblemDetails, SaveOidcConfigRequest,
    SwitchProfileRequest, UpdateBrowserUserRequest,
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
        state.max_session_minutes,
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
    let deleted_self = run_kernel(capability, correlation_id, move || {
        kernel.delete_browser_user(DeleteBrowserUserCommand::new(
            correlation_id,
            session,
            csrf,
            target_user_id,
            current_password,
        ))
    })
    .await?;
    let headers = if deleted_self {
        clear_cookies(state.secure_cookies)
    } else {
        no_store_headers()
    };
    Ok((headers, StatusCode::NO_CONTENT).into_response())
}

#[utoipa::path(
    get,
    path = "/api/v1/browser/sessions",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 200, description = "Active browser sessions for the current user", body = ListBrowserSessionsResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Session no longer has active access", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn list_sessions(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let kernel = state.kernel;
    let summaries = run_kernel(capability, correlation_id, move || {
        kernel.list_browser_sessions(ListBrowserSessionsQuery::new(correlation_id, session))
    })
    .await?;
    let dts = summaries
        .into_iter()
        .map(|s| BrowserSessionItemDto {
            session_id: s.session_id().to_owned(),
            created_at: s.created_at().to_rfc3339(),
            expires_at: s.expires_at().to_rfc3339(),
            last_seen_at: s.last_seen_at().to_rfc3339(),
            location: s.location().to_owned(),
            device_type: s.device_type().to_owned(),
            is_current: s.is_current(),
        })
        .collect();
    Ok((
        no_store_headers(),
        Json(ListBrowserSessionsResponse { sessions: dts }),
    )
        .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/browser/sessions/{session_id}",
    tag = "browser authentication",
    security(("browser_session" = [])),
    params(("session_id" = String, Path, description = "Session identifier to terminate")),
    responses(
        (status = 204, description = "Specific browser session terminated"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn end_specific_session(
    State(state): State<LocalApiState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::EndBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.end_specific_browser_session(EndSpecificBrowserSessionCommand::new(
            correlation_id,
            session,
            csrf,
            session_id,
        ))
    })
    .await?;
    Ok((no_store_headers(), StatusCode::NO_CONTENT).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/browser/sessions",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 204, description = "All other browser sessions terminated"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn end_other_sessions(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::EndBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.end_all_other_browser_sessions(EndAllOtherBrowserSessionsCommand::new(
            correlation_id,
            session,
            csrf,
        ))
    })
    .await?;
    Ok((no_store_headers(), StatusCode::NO_CONTENT).into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/browser/session/switch-profile",
    tag = "browser authentication",
    security(("browser_session" = [])),
    request_body = SwitchProfileRequest,
    responses(
        (status = 200, description = "Switched active profile for session", body = BrowserSessionResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn switch_profile(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    payload: Result<Json<SwitchProfileRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        payload.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let target_profile_id = request
        .profile_id
        .parse::<fasti_domain::ProfileId>()
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.switch_browser_session_profile(SwitchBrowserSessionProfileCommand::new(
            correlation_id,
            session,
            csrf,
            target_profile_id,
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
    get,
    path = "/api/v1/browser/auth/passkeys",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 200, description = "List of registered passkeys", body = ListPasskeysResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Session lacks authorization", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn list_passkeys(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.list_passkeys(ListPasskeysQuery::new(correlation_id, session))
    })
    .await?;
    let passkeys = outcome
        .into_iter()
        .map(|p| PasskeyDto {
            passkey_id: p.passkey_id().to_string(),
            name: p.name().to_string(),
            created_at: p.created_at().to_rfc3339(),
            last_used_at: p.last_used_at().map(|t| t.to_rfc3339()),
        })
        .collect();
    Ok((no_store_headers(), Json(ListPasskeysResponse { passkeys })).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/browser/auth/passkeys/{passkey_id}",
    tag = "browser authentication",
    security(("browser_session" = [])),
    params(
        ("passkey_id" = String, Path, description = "Identifier of the passkey to delete")
    ),
    responses(
        (status = 204, description = "Passkey deleted"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn delete_passkey(
    State(state): State<LocalApiState>,
    Path(passkey_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.delete_passkey(DeletePasskeyCommand::new(
            correlation_id,
            session,
            csrf,
            passkey_id,
        ))
    })
    .await?;
    Ok((no_store_headers(), StatusCode::NO_CONTENT).into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/browser/auth/passkey/register/begin",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 200, description = "WebAuthn challenge generated", body = BeginPasskeyRegistrationResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Session lacks authorization", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn begin_passkey_registration(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel
            .begin_passkey_registration(BeginPasskeyRegistrationQuery::new(correlation_id, session))
    })
    .await?;
    Ok((
        no_store_headers(),
        Json(BeginPasskeyRegistrationResponse {
            challenge: outcome.challenge().to_string(),
            rp_name: outcome.rp_name().to_string(),
            rp_id: outcome.rp_id().to_string(),
            user_id: outcome.user_id().to_string(),
            user_name: outcome.user_name().to_string(),
        }),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/browser/auth/passkey/register/complete",
    tag = "browser authentication",
    security(("browser_session" = [])),
    request_body = CompletePasskeyRegistrationRequest,
    responses(
        (status = 200, description = "Passkey registered", body = PasskeyDto),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn complete_passkey_registration(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    payload: Result<Json<CompletePasskeyRegistrationRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        payload.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.complete_passkey_registration(CompletePasskeyRegistrationCommand::new(
            correlation_id,
            session,
            csrf,
            request.name,
            request.credential_id,
            request.client_data_json,
            request.attestation_object,
        ))
    })
    .await?;
    Ok((
        no_store_headers(),
        Json(PasskeyDto {
            passkey_id: outcome.passkey_id().to_string(),
            name: outcome.name().to_string(),
            created_at: outcome.created_at().to_rfc3339(),
            last_used_at: outcome.last_used_at().map(|t| t.to_rfc3339()),
        }),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/browser/auth/totp/enroll/begin",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 200, description = "TOTP enrollment begun with secret, otpauth URI, and backup codes", body = EnrollTotpResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn enroll_totp_begin(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.enroll_totp_begin(EnrollTotpBeginCommand::new(correlation_id, session, csrf))
    })
    .await?;
    Ok((
        no_store_headers(),
        Json(EnrollTotpResponse {
            secret: outcome.secret().to_string(),
            otpauth_uri: outcome.otpauth_uri().to_string(),
            backup_codes: outcome.backup_codes().to_vec(),
        }),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/browser/auth/totp/enroll/confirm",
    tag = "browser authentication",
    security(("browser_session" = [])),
    request_body = ConfirmTotpRequest,
    responses(
        (status = 200, description = "TOTP enrollment confirmed"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "CSRF proof is missing or invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn enroll_totp_confirm(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    payload: Result<Json<ConfirmTotpRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        payload.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.enroll_totp_confirm(EnrollTotpConfirmCommand::new(
            correlation_id,
            session,
            csrf,
            request.code,
        ))
    })
    .await?;
    Ok((no_store_headers(), StatusCode::OK).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/browser/auth/totp",
    tag = "browser authentication",
    security(("browser_session" = [])),
    request_body = DisableTotpRequest,
    responses(
        (status = 204, description = "TOTP 2FA disabled"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Password verification failed or CSRF proof is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn disable_totp(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    payload: Result<Json<DisableTotpRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        payload.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let current_password = BrowserPassword::try_new(request.current_password)
        .map_err(|_| validation_problem(capability, correlation_id))?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.disable_totp(DisableTotpCommand::new(
            correlation_id,
            session,
            csrf,
            current_password,
        ))
    })
    .await?;
    Ok((no_store_headers(), StatusCode::NO_CONTENT).into_response())
}

#[utoipa::path(
    get,
    path = "/api/v1/browser/auth/oidc/config",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 200, description = "OIDC SSO configuration", body = OidcConfigDto),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Session lacks authorization", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn get_oidc_config(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.get_oidc_config(GetOidcConfigQuery::new(correlation_id, session))
    })
    .await?;
    let dto = outcome
        .map(|c| OidcConfigDto {
            issuer_url: c.issuer_url().to_string(),
            client_id: c.client_id().to_string(),
            pkce_enabled: c.pkce_enabled(),
            scopes: c.scopes().to_vec(),
            enabled: c.enabled(),
        })
        .unwrap_or(OidcConfigDto {
            issuer_url: String::new(),
            client_id: String::new(),
            pkce_enabled: true,
            scopes: vec![
                "openid".to_string(),
                "profile".to_string(),
                "email".to_string(),
            ],
            enabled: false,
        });
    Ok((no_store_headers(), Json(dto)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/v1/browser/auth/oidc/config",
    tag = "browser authentication",
    security(("browser_session" = [])),
    request_body = SaveOidcConfigRequest,
    responses(
        (status = 200, description = "OIDC SSO configuration saved", body = OidcConfigDto),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Forbidden for non-administrators or invalid CSRF proof", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn save_oidc_config(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    payload: Result<Json<SaveOidcConfigRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        payload.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.save_oidc_config(SaveOidcConfigCommand::new(
            correlation_id,
            session,
            csrf,
            request.issuer_url,
            request.client_id,
            request.client_secret,
            request.pkce_enabled,
            request.scopes,
            request.enabled,
        ))
    })
    .await?;
    Ok((
        no_store_headers(),
        Json(OidcConfigDto {
            issuer_url: outcome.issuer_url().to_string(),
            client_id: outcome.client_id().to_string(),
            pkce_enabled: outcome.pkce_enabled(),
            scopes: outcome.scopes().to_vec(),
            enabled: outcome.enabled(),
        }),
    )
        .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/browser/auth/oidc/config",
    tag = "browser authentication",
    security(("browser_session" = [])),
    responses(
        (status = 204, description = "OIDC SSO configuration removed"),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Forbidden for non-administrators or invalid CSRF proof", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn delete_oidc_config(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let csrf = csrf_from_request(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.delete_oidc_config(DeleteOidcConfigCommand::new(correlation_id, session, csrf))
    })
    .await?;
    Ok((no_store_headers(), StatusCode::NO_CONTENT).into_response())
}

#[utoipa::path(
    post,
    path = "/api/v1/browser/auth/oidc/discover",
    tag = "browser authentication",
    security(("browser_session" = [])),
    request_body = OidcDiscoveryRequest,
    responses(
        (status = 200, description = "Discovered OIDC IdP endpoints", body = OidcDiscoveryResponse),
        (status = 401, description = "Browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Session lacks authorization", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn discover_oidc(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    payload: Result<Json<OidcDiscoveryRequest>, JsonRejection>,
) -> Result<Response, HttpProblem> {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let _session = cookie_secret(&headers, "fasti_session", capability, correlation_id)?;
    let Json(request) =
        payload.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.discover_oidc(DiscoverOidcQuery::new(correlation_id, request.issuer_url))
    })
    .await?;
    Ok((
        no_store_headers(),
        Json(OidcDiscoveryResponse {
            authorization_endpoint: outcome.authorization_endpoint().to_string(),
            token_endpoint: outcome.token_endpoint().to_string(),
            userinfo_endpoint: outcome.userinfo_endpoint().map(|s| s.to_string()),
            jwks_uri: outcome.jwks_uri().to_string(),
        }),
    )
        .into_response())
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new()
        .route(
            "/api/v1/browser/session",
            post(create_session).get(read_session).delete(end_session),
        )
        .route(
            "/api/v1/browser/session/switch-profile",
            post(switch_profile),
        )
        .route(
            "/api/v1/browser/sessions",
            get(list_sessions).delete(end_other_sessions),
        )
        .route(
            "/api/v1/browser/sessions/{session_id}",
            axum::routing::delete(end_specific_session),
        )
        .route("/api/v1/browser/users", get(list_users))
        .route(
            "/api/v1/browser/users/{user_id}",
            patch(update_user).delete(delete_user),
        )
        .route("/api/v1/browser/auth/passkeys", get(list_passkeys))
        .route(
            "/api/v1/browser/auth/passkeys/{passkey_id}",
            axum::routing::delete(delete_passkey),
        )
        .route(
            "/api/v1/browser/auth/passkey/register/begin",
            post(begin_passkey_registration),
        )
        .route(
            "/api/v1/browser/auth/passkey/register/complete",
            post(complete_passkey_registration),
        )
        .route(
            "/api/v1/browser/auth/totp/enroll/begin",
            post(enroll_totp_begin),
        )
        .route(
            "/api/v1/browser/auth/totp/enroll/confirm",
            post(enroll_totp_confirm),
        )
        .route(
            "/api/v1/browser/auth/totp",
            axum::routing::delete(disable_totp),
        )
        .route(
            "/api/v1/browser/auth/oidc/config",
            get(get_oidc_config)
                .put(save_oidc_config)
                .delete(delete_oidc_config),
        )
        .route("/api/v1/browser/auth/oidc/discover", post(discover_oidc))
        .layer(axum::extract::DefaultBodyLimit::max(
            MAX_BROWSER_AUTH_JSON_BODY_BYTES,
        ))
}
