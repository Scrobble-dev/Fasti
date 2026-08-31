use crate::local::{
    browser_session_mutation_command, browser_session_query, run_kernel, CSRF_COOKIE,
    SESSION_COOKIE,
};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use crate::trailbase::{TrailBaseOrchestrationError, TrailBaseOrchestrator};
use crate::{FASTI_ACCESS_BINDING_COOKIE, FASTI_ACCESS_CALLBACK_PATH, FASTI_ACCESS_HOST};
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, RawQuery, State},
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{any, delete, get, post, put},
    Json, Router,
};
use fasti_application::{
    AccessBrowserSessionSummary, AccessCeremonyEvidence, AccessEvidenceKind, AccessEvidenceState,
    AccessFirstRunStep, AccessFirstRunStepKey, AccessMembershipSummary, AccessProfileGrantSummary,
    AccessProjection, AccessSessionAuthenticationSummary, AccessSubjectSummary,
    AccessTrailBaseActivationSummary, BrowserSessionSummary, CapabilityKey, CreatedBrowserSession,
    FastiProblem, LocalKernel, ProblemCode, SelectBrowserSessionProfileCommand,
    TargetBrowserSessionCommand, Violation, C1_AUTH_CEREMONY_LIFETIME,
};
use fasti_contracts::{
    AccessAuthenticationMethodDto, AccessCeremonyFailureDto, AccessCeremonyStateDto,
    AccessEvidenceDto, AccessEvidenceKindDto, AccessEvidenceStateDto, AccessFirstRunStepDto,
    AccessFirstRunStepKeyDto, AccessMembershipDto, AccessMembershipLifecycleDto,
    AccessProfileGrantDto, AccessProjectionResponse, AccessSessionAuthenticationDto,
    AccessSubjectDto, AccessSubjectLifecycleDto, AccessWorkspaceRoleDto, BrowserSessionDto,
    BrowserSessionPolicyDto, CompleteTrailBaseAuthenticationQuery, ListBrowserSessionsResponse,
    ProblemDetails, ReadBrowserSessionResponse, RecentAuthenticationDto,
    RevokeBrowserSessionsResponse, RotateBrowserSessionResponse,
    SelectBrowserSessionProfileRequest, SelectBrowserSessionProfileResponse,
    StartTrailBaseSignInRequest, StartTrailBaseSignInResponse, TrailBaseActivationBlockerDto,
    TrailBaseActivationDto, TrailBaseActivationStateDto,
};
use fasti_domain::{
    AuthCeremonyFailure, AuthCeremonyPurpose, AuthCeremonySelection, AuthCeremonyState,
    AuthReturnTarget, AuthSubjectLifecycle, AuthenticationMethod, BrowserSessionId,
    FastiBrowserSession, MembershipId, MembershipLifecycle, ProfileGrantId, RequestCorrelationId,
    TrailBaseActivationBlocker, TrailBaseActivationState, WorkspaceId, WorkspaceRole,
};
use std::{str::FromStr, sync::Arc};

const MAX_ACCESS_JSON_BODY_BYTES: usize = 4 * 1024;

type HttpResponse = Result<Response, HttpProblem>;

#[derive(Clone)]
pub(crate) struct AccessApiState {
    kernel: Arc<dyn LocalKernel>,
    boundary: fasti_application::BrowserRequestBoundaryPolicy,
    trailbase: Option<Arc<TrailBaseOrchestrator>>,
}

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

fn invalid_request(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    pointer: &'static str,
    expected: &'static str,
) -> HttpProblem {
    let violation = Violation::try_new(
        "invalid_access_request",
        pointer,
        "value does not satisfy the Access contract",
        expected,
    )
    .expect("static Access violation is valid");
    application_problem(Box::new(
        FastiProblem::validation_failed(capability, correlation_id, vec![violation])
            .expect("one Access violation is within bounds"),
    ))
}

fn trailbase_start_problem(
    error: TrailBaseOrchestrationError,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    let code = match error {
        TrailBaseOrchestrationError::ApplicationProblem(code) => code,
        TrailBaseOrchestrationError::InvalidInput => ProblemCode::ValidationFailed,
        TrailBaseOrchestrationError::LocalState => ProblemCode::StorageUnavailable,
        TrailBaseOrchestrationError::ExchangeFailed
        | TrailBaseOrchestrationError::ExchangeOutcomeUncertain
        | TrailBaseOrchestrationError::StatusRejected => ProblemCode::IdentityServiceUnavailable,
        TrailBaseOrchestrationError::LogoutUncertain => ProblemCode::TrailBaseSessionCleanupFailed,
        TrailBaseOrchestrationError::LocalAuthorizationDenied => {
            ProblemCode::AuthSubjectUnaffiliated
        }
    };
    application_problem(Box::new(FastiProblem::from_code(
        code,
        CapabilityKey::CreateBrowserSession,
        correlation_id,
    )))
}

fn session_dto(session: FastiBrowserSession, is_current: bool) -> BrowserSessionDto {
    BrowserSessionDto {
        browser_session_id: session.id().to_string(),
        workspace_id: session.workspace_id().to_string(),
        selected_profile_grant_id: session.selected_profile_grant_id().to_string(),
        is_current,
        created_at: session.created_at().to_rfc3339(),
        last_seen_at: session.last_seen_at().to_rfc3339(),
        idle_expires_at: session.idle_expires_at().to_rfc3339(),
        absolute_expires_at: session.absolute_expires_at().to_rfc3339(),
        rotation_generation: session.rotation_generation(),
    }
}

fn access_session_dto(session: AccessBrowserSessionSummary) -> BrowserSessionDto {
    session_dto(session.session(), session.is_current())
}

fn listed_session_dto(session: BrowserSessionSummary) -> BrowserSessionDto {
    session_dto(session.session(), session.is_current())
}

fn evidence_state_dto(state: AccessEvidenceState) -> AccessEvidenceStateDto {
    match state {
        AccessEvidenceState::Loading => AccessEvidenceStateDto::Loading,
        AccessEvidenceState::Empty => AccessEvidenceStateDto::Empty,
        AccessEvidenceState::Unavailable => AccessEvidenceStateDto::Unavailable,
        AccessEvidenceState::NeedsAttention => AccessEvidenceStateDto::NeedsAttention,
        AccessEvidenceState::FailedSafely => AccessEvidenceStateDto::FailedSafely,
        AccessEvidenceState::Verified => AccessEvidenceStateDto::Verified,
    }
}

fn subject_dto(subject: AccessSubjectSummary) -> AccessSubjectDto {
    AccessSubjectDto {
        auth_subject_id: subject.id().to_string(),
        lifecycle: match subject.lifecycle() {
            AuthSubjectLifecycle::Active => AccessSubjectLifecycleDto::Active,
            AuthSubjectLifecycle::Disabled => AccessSubjectLifecycleDto::Disabled,
            AuthSubjectLifecycle::Deleted => AccessSubjectLifecycleDto::Deleted,
            AuthSubjectLifecycle::RecoveryPending => AccessSubjectLifecycleDto::RecoveryPending,
        },
        created_at: subject.created_at().to_rfc3339(),
        updated_at: subject.updated_at().to_rfc3339(),
    }
}

fn membership_dto(membership: AccessMembershipSummary) -> AccessMembershipDto {
    AccessMembershipDto {
        membership_id: membership.id().to_string(),
        workspace_id: membership.workspace_id().to_string(),
        lifecycle: match membership.lifecycle() {
            MembershipLifecycle::Invited => AccessMembershipLifecycleDto::Invited,
            MembershipLifecycle::PendingApproval => AccessMembershipLifecycleDto::PendingApproval,
            MembershipLifecycle::Active => AccessMembershipLifecycleDto::Active,
            MembershipLifecycle::Suspended => AccessMembershipLifecycleDto::Suspended,
            MembershipLifecycle::Removed => AccessMembershipLifecycleDto::Removed,
        },
        role: match membership.role() {
            WorkspaceRole::Member => AccessWorkspaceRoleDto::Member,
            WorkspaceRole::Administrator => AccessWorkspaceRoleDto::Administrator,
        },
        created_at: membership.created_at().to_rfc3339(),
        updated_at: membership.updated_at().to_rfc3339(),
    }
}

fn profile_grant_dto(grant: AccessProfileGrantSummary) -> AccessProfileGrantDto {
    AccessProfileGrantDto {
        profile_grant_id: grant.grant_id().to_string(),
        profile_id: grant.profile_id().to_string(),
        owner_client_id: grant.owner_client_id().to_string(),
        selected: grant.is_selected(),
    }
}

fn authentication_dto(
    authentication: AccessSessionAuthenticationSummary,
) -> AccessSessionAuthenticationDto {
    let recent = authentication.recent_authentication();
    AccessSessionAuthenticationDto {
        method: match authentication.method() {
            AuthenticationMethod::TrailBasePassword => {
                AccessAuthenticationMethodDto::TrailBasePassword
            }
            AuthenticationMethod::TrailBaseSocial => AccessAuthenticationMethodDto::TrailBaseSocial,
        },
        verified_at: authentication.verified_at().to_rfc3339(),
        activation_generation: authentication.activation_generation(),
        recent_authentication: RecentAuthenticationDto {
            state: evidence_state_dto(recent.state()),
            expires_at: recent.expires_at().map(|value| value.to_rfc3339()),
        },
    }
}

fn trailbase_dto(trailbase: AccessTrailBaseActivationSummary) -> TrailBaseActivationDto {
    let (state, blocker) = match trailbase.state() {
        TrailBaseActivationState::Inactive => (TrailBaseActivationStateDto::Inactive, None),
        TrailBaseActivationState::Active => (TrailBaseActivationStateDto::Active, None),
        TrailBaseActivationState::Blocked(blocker) => (
            TrailBaseActivationStateDto::Blocked,
            Some(match blocker {
                TrailBaseActivationBlocker::ReleaseMismatch => {
                    TrailBaseActivationBlockerDto::ReleaseMismatch
                }
                TrailBaseActivationBlocker::PhysicalRootIdentityMismatch => {
                    TrailBaseActivationBlockerDto::PhysicalRootIdentityMismatch
                }
                TrailBaseActivationBlocker::DeclaredRestore => {
                    TrailBaseActivationBlockerDto::DeclaredRestore
                }
            }),
        ),
    };
    TrailBaseActivationDto {
        state,
        blocker,
        trailbase_instance_id: trailbase.instance_id().to_string(),
        generation: trailbase.generation(),
        session_generation_current: trailbase.session_generation_is_current(),
        updated_at: trailbase.updated_at().to_rfc3339(),
    }
}

fn first_run_step_dto(step: AccessFirstRunStep) -> AccessFirstRunStepDto {
    AccessFirstRunStepDto {
        key: match step.key() {
            AccessFirstRunStepKey::AccountConfirmed => AccessFirstRunStepKeyDto::AccountConfirmed,
            AccessFirstRunStepKey::StrongSignIn => AccessFirstRunStepKeyDto::StrongSignIn,
            AccessFirstRunStepKey::Recovery => AccessFirstRunStepKeyDto::Recovery,
            AccessFirstRunStepKey::DevicesAndClients => AccessFirstRunStepKeyDto::DevicesAndClients,
            AccessFirstRunStepKey::ExternalIdentity => AccessFirstRunStepKeyDto::ExternalIdentity,
        },
        state: evidence_state_dto(step.state()),
    }
}

fn evidence_dto(evidence: AccessCeremonyEvidence) -> AccessEvidenceDto {
    AccessEvidenceDto {
        kind: match evidence.kind() {
            AccessEvidenceKind::CurrentSessionIssued => AccessEvidenceKindDto::CurrentSessionIssued,
            AccessEvidenceKind::FirstAdministratorBootstrap => {
                AccessEvidenceKindDto::FirstAdministratorBootstrap
            }
        },
        state: evidence_state_dto(evidence.state()),
        operation_id: evidence.operation_id().to_string(),
        correlation_id: evidence.correlation_id().to_string(),
        ceremony_state: evidence.ceremony_state().map(|state| match state {
            AuthCeremonyState::Pending => AccessCeremonyStateDto::Pending,
            AuthCeremonyState::Claimed => AccessCeremonyStateDto::Claimed,
            AuthCeremonyState::Completed => AccessCeremonyStateDto::Completed,
            AuthCeremonyState::Cancelled => AccessCeremonyStateDto::Cancelled,
            AuthCeremonyState::Failed => AccessCeremonyStateDto::Failed,
            AuthCeremonyState::CleanupUncertain => AccessCeremonyStateDto::CleanupUncertain,
            AuthCeremonyState::Expired => AccessCeremonyStateDto::Expired,
        }),
        failure: evidence.failure().map(|failure| match failure {
            AuthCeremonyFailure::VerifierLostOnRestart => {
                AccessCeremonyFailureDto::VerifierLostOnRestart
            }
            AuthCeremonyFailure::ExchangeOutcomeUncertain => {
                AccessCeremonyFailureDto::ExchangeOutcomeUncertain
            }
            AuthCeremonyFailure::ExchangeFailed => AccessCeremonyFailureDto::ExchangeFailed,
            AuthCeremonyFailure::StatusRejected => AccessCeremonyFailureDto::StatusRejected,
            AuthCeremonyFailure::LogoutUncertain => AccessCeremonyFailureDto::LogoutUncertain,
            AuthCeremonyFailure::LocalAuthorizationDenied => {
                AccessCeremonyFailureDto::LocalAuthorizationDenied
            }
        }),
        occurred_at: evidence.occurred_at().to_rfc3339(),
    }
}

fn projection_dto(projection: AccessProjection) -> AccessProjectionResponse {
    let policy = projection.session_policy();
    let first_run_steps: [AccessFirstRunStepDto; 5] = projection
        .first_run_steps()
        .iter()
        .copied()
        .map(first_run_step_dto)
        .collect::<Vec<_>>()
        .try_into()
        .expect("application projection always owns exactly five first-run steps");
    AccessProjectionResponse {
        generated_at: projection.generated_at().to_rfc3339(),
        subject: subject_dto(projection.subject()),
        membership: membership_dto(projection.membership()),
        current_session: access_session_dto(projection.current_session()),
        sessions: projection
            .sessions()
            .iter()
            .copied()
            .map(access_session_dto)
            .collect(),
        sessions_truncated: projection.sessions_truncated(),
        profile_grants: projection
            .profile_grants()
            .iter()
            .copied()
            .map(profile_grant_dto)
            .collect(),
        profile_grants_truncated: projection.profile_grants_truncated(),
        session_policy: BrowserSessionPolicyDto {
            idle_timeout_seconds: policy.browser_idle_timeout().as_secs(),
            browser_lifetime_seconds: policy.browser_absolute_lifetime().as_secs(),
            remembered_browser_lifetime_seconds: policy.remembered_browser_lifetime().as_secs(),
            last_seen_write_interval_seconds: policy.last_seen_write_interval().as_secs(),
        },
        authentication: authentication_dto(projection.authentication()),
        trailbase: trailbase_dto(projection.trailbase()),
        first_run_steps,
        evidence: projection
            .evidence()
            .iter()
            .copied()
            .map(evidence_dto)
            .collect(),
        evidence_truncated: projection.evidence_truncated(),
    }
}

fn append_cookie(response: &mut Response, value: String) {
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).expect("hex session cookies produce valid header values"),
    );
}

fn set_session_cookies(response: &mut Response, created: &CreatedBrowserSession) {
    let max_age = (created.session().absolute_expires_at() - chrono::Utc::now())
        .num_seconds()
        .max(0);
    append_cookie(
        response,
        format!(
            "{SESSION_COOKIE}={}; Path=/; Max-Age={max_age}; Secure; HttpOnly; SameSite=Strict",
            created.session_secret().expose_hex()
        ),
    );
    append_cookie(
        response,
        format!(
            "{CSRF_COOKIE}={}; Path=/; Max-Age={max_age}; Secure; SameSite=Strict",
            created.csrf_secret().expose_hex()
        ),
    );
}

fn clear_session_cookies(response: &mut Response) {
    append_cookie(
        response,
        format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; Secure; HttpOnly; SameSite=Strict"),
    );
    append_cookie(
        response,
        format!("{CSRF_COOKIE}=; Path=/; Max-Age=0; Secure; SameSite=Strict"),
    );
}

fn clear_binding_cookie(response: &mut Response) {
    append_cookie(
        response,
        format!(
            "{FASTI_ACCESS_BINDING_COOKIE}=; Domain=127.0.0.1; Path={FASTI_ACCESS_CALLBACK_PATH}; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Secure; HttpOnly; SameSite=Lax"
        ),
    );
}

fn exact_callback_code(raw_query: Option<&str>) -> Option<String> {
    let code = raw_query?.strip_prefix("code=")?;
    if code.len() == 48 && code.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        Some(code.to_owned())
    } else {
        None
    }
}

fn exact_host(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(header::HOST).iter();
    matches!(values.next(), Some(value) if value.as_bytes() == FASTI_ACCESS_HOST.as_bytes())
        && values.next().is_none()
}

fn callback_binding(headers: &HeaderMap) -> Option<fasti_application::SecretMaterial> {
    let mut binding = None;
    for header_value in headers.get_all(header::COOKIE) {
        let value = header_value.to_str().ok()?;
        for pair in value.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name == FASTI_ACCESS_BINDING_COOKIE && binding.replace(value).is_some() {
                return None;
            }
        }
    }
    fasti_application::SecretMaterial::try_from_hex(binding?).ok()
}

fn return_path(target: AuthReturnTarget) -> &'static str {
    match target {
        AuthReturnTarget::ApplicationHome => "/",
        AuthReturnTarget::AccountSecurity => "/settings/account",
        AuthReturnTarget::FirstRun => "/first-run",
    }
}

fn callback_redirect(
    target: AuthReturnTarget,
    correlation_id: RequestCorrelationId,
    created: Option<&CreatedBrowserSession>,
) -> Response {
    let location = if created.is_some() {
        return_path(target).to_owned()
    } else {
        format!(
            "{}?auth=failed&correlation_id={correlation_id}",
            return_path(target)
        )
    };
    let mut response = StatusCode::SEE_OTHER.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location)
            .expect("fixed return path and typed correlation are valid"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    clear_binding_cookie(&mut response);
    if let Some(created) = created {
        set_session_cookies(&mut response, created);
    }
    no_store(response)
}

#[utoipa::path(
    post,
    path = "/api/access/v1/trailbase/sign-in",
    operation_id = "start_trailbase_sign_in",
    tag = "access",
    request_body = StartTrailBaseSignInRequest,
    responses(
        (status = 200, description = "TrailBase authorization URL and bounded Fasti ceremony", body = StartTrailBaseSignInResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Origin or Host boundary rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request payload is too large", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Request media type is unsupported", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Sign-in selection is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored Access state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "TrailBase trust or local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded authentication capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn start_trailbase_sign_in(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
    request: Result<Json<StartTrailBaseSignInRequest>, JsonRejection>,
) -> HttpResponse {
    let capability = CapabilityKey::CreateBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    if headers.contains_key(header::AUTHORIZATION) {
        return Err(application_problem(Box::new(FastiProblem::forbidden(
            capability,
            correlation_id,
        ))));
    }
    state
        .boundary
        .validate(
            headers
                .get(header::ORIGIN)
                .and_then(|value| value.to_str().ok()),
            headers
                .get(header::HOST)
                .and_then(|value| value.to_str().ok()),
        )
        .map_err(|_| {
            application_problem(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )))
        })?;
    let workspace_id = WorkspaceId::from_str(&request.workspace_id).map_err(|_| {
        invalid_request(
            capability,
            correlation_id,
            "/workspace_id",
            "a canonical Fasti workspace identifier",
        )
    })?;
    let profile_grant_id = ProfileGrantId::from_str(&request.profile_grant_id).map_err(|_| {
        invalid_request(
            capability,
            correlation_id,
            "/profile_grant_id",
            "a canonical Fasti profile-grant identifier",
        )
    })?;
    let invited_membership_id = request
        .invited_membership_id
        .as_deref()
        .map(MembershipId::from_str)
        .transpose()
        .map_err(|_| {
            invalid_request(
                capability,
                correlation_id,
                "/invited_membership_id",
                "a canonical Fasti membership identifier",
            )
        })?;
    let selection = AuthCeremonySelection::try_new(
        AuthCeremonyPurpose::SignIn,
        workspace_id,
        profile_grant_id,
        None,
        invited_membership_id,
        request.remembered,
    )
    .map_err(|_| {
        invalid_request(
            capability,
            correlation_id,
            "/",
            "a valid TrailBase sign-in selection",
        )
    })?;
    let orchestrator = state.trailbase.ok_or_else(|| {
        application_problem(Box::new(FastiProblem::from_code(
            ProblemCode::TrailBaseTrustUnavailable,
            capability,
            correlation_id,
        )))
    })?;
    let created_at = chrono::Utc::now();
    let expires_at = created_at
        + chrono::Duration::from_std(C1_AUTH_CEREMONY_LIFETIME)
            .expect("C1 ceremony lifetime fits chrono");
    let started = tokio::task::spawn_blocking(move || {
        orchestrator.start_sign_in(selection, correlation_id, created_at, expires_at)
    })
    .await
    .map_err(|_| {
        application_problem(Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        )))
    })?
    .map_err(|error| trailbase_start_problem(error, correlation_id))?;
    let max_age = (started.expires_at - chrono::Utc::now())
        .num_seconds()
        .max(0);
    let mut response = Json(StartTrailBaseSignInResponse {
        authorization_url: started.authorization_url,
        ceremony_id: started.operation_id.to_string(),
        expires_at: started.expires_at.to_rfc3339(),
    })
    .into_response();
    append_cookie(
        &mut response,
        format!(
            "__Secure-fasti_auth_binding={}; Domain=127.0.0.1; Path=/api/access/v1/trailbase/callback; Max-Age={max_age}; Secure; HttpOnly; SameSite=Lax",
            started.browser_binding.expose_hex()
        ),
    );
    Ok(no_store(response))
}

#[utoipa::path(
    get,
    path = "/api/access/v1/trailbase/callback",
    operation_id = "complete_trailbase_authentication",
    tag = "access",
    security(("auth_binding_cookie" = [])),
    params(CompleteTrailBaseAuthenticationQuery),
    responses(
        (status = 303, description = "Fixed application redirect after success or safe failure")
    )
)]
pub(crate) async fn complete_trailbase_authentication(
    State(state): State<AccessApiState>,
    method: Method,
    RawQuery(raw_query): RawQuery,
    headers: HeaderMap,
) -> Response {
    let correlation_id = RequestCorrelationId::new_v7();
    let fallback_target = AuthReturnTarget::ApplicationHome;
    if method != Method::GET || !exact_host(&headers) || headers.contains_key(header::AUTHORIZATION)
    {
        return callback_redirect(fallback_target, correlation_id, None);
    }
    let Some(code) = exact_callback_code(raw_query.as_deref()) else {
        return callback_redirect(fallback_target, correlation_id, None);
    };
    let Some(binding) = callback_binding(&headers) else {
        return callback_redirect(fallback_target, correlation_id, None);
    };
    let Some(orchestrator) = state.trailbase else {
        return callback_redirect(fallback_target, correlation_id, None);
    };
    let at = chrono::Utc::now();
    match orchestrator
        .callback_for_browser(code, binding, correlation_id, at)
        .await
    {
        Ok(outcome) => callback_redirect(
            outcome.return_target,
            correlation_id,
            Some(&outcome.created),
        ),
        Err(failure) => callback_redirect(
            failure.return_target.unwrap_or(fallback_target),
            correlation_id,
            None,
        ),
    }
}

#[utoipa::path(
    get,
    path = "/api/access/v1/projection",
    operation_id = "read_access_projection",
    tag = "access",
    security(("browser_session_cookie" = [])),
    responses(
        (status = 200, description = "Bounded Account and security projection", body = AccessProjectionResponse),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored Access state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_access_projection(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::ReadAccessProjection;
    let correlation_id = RequestCorrelationId::new_v7();
    let (query, _) = browser_session_query(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    let projection = run_kernel(capability, correlation_id, move || {
        kernel.read_access_projection(query)
    })
    .await?;
    Ok(no_store(Json(projection_dto(projection)).into_response()))
}

#[utoipa::path(
    get,
    path = "/api/access/v1/browser-session",
    operation_id = "read_browser_session",
    tag = "access",
    security(("browser_session_cookie" = [])),
    responses(
        (status = 200, description = "Current Fasti browser session", body = ReadBrowserSessionResponse),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_browser_session(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::ReadBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let (query, _) = browser_session_query(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    let authenticated = run_kernel(capability, correlation_id, move || {
        kernel.authenticate_browser_session(query)
    })
    .await?;
    Ok(no_store(
        Json(ReadBrowserSessionResponse {
            session: session_dto(authenticated.session(), true),
        })
        .into_response(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/access/v1/browser-sessions",
    operation_id = "list_browser_sessions",
    tag = "access",
    security(("browser_session_cookie" = [])),
    responses(
        (status = 200, description = "Bounded active browser-session inventory", body = ListBrowserSessionsResponse),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn list_browser_sessions(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::ListBrowserSessions;
    let correlation_id = RequestCorrelationId::new_v7();
    let (query, _) = browser_session_query(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    let inventory = run_kernel(capability, correlation_id, move || {
        kernel.list_browser_sessions(query)
    })
    .await?;
    Ok(no_store(
        Json(ListBrowserSessionsResponse {
            sessions: inventory
                .sessions()
                .iter()
                .copied()
                .map(listed_session_dto)
                .collect(),
            truncated: inventory.truncated(),
        })
        .into_response(),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/access/v1/browser-session",
    operation_id = "end_browser_session",
    tag = "access",
    security(("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    responses(
        (status = 204, description = "Current browser session ended"),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Origin, Host, or CSRF proof rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn end_browser_session(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::EndBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let command =
        browser_session_mutation_command(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    run_kernel(capability, correlation_id, move || {
        kernel.revoke_current_browser_session(command)
    })
    .await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    clear_session_cookies(&mut response);
    Ok(no_store(response))
}

#[utoipa::path(
    delete,
    path = "/api/access/v1/browser-sessions/{browser_session_id}",
    operation_id = "revoke_browser_session",
    tag = "access",
    security(("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    params(("browser_session_id" = String, Path, description = "Fasti browser-session identifier")),
    responses(
        (status = 200, description = "Exact revocation outcome", body = RevokeBrowserSessionsResponse),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Origin, Host, or CSRF proof rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Target session identifier is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn revoke_browser_session(
    State(state): State<AccessApiState>,
    Path(browser_session_id): Path<String>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::RevokeBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let target = BrowserSessionId::from_str(&browser_session_id).map_err(|_| {
        invalid_request(
            capability,
            correlation_id,
            "/browser_session_id",
            "a canonical Fasti browser-session identifier",
        )
    })?;
    let proof =
        browser_session_mutation_command(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        kernel.revoke_browser_session(TargetBrowserSessionCommand::new(proof, target))
    })
    .await?;
    let mut response = Json(RevokeBrowserSessionsResponse {
        revoked_count: u64::from(outcome.revoked()),
    })
    .into_response();
    if outcome.current_session_revoked() {
        clear_session_cookies(&mut response);
    }
    Ok(no_store(response))
}

#[utoipa::path(
    delete,
    path = "/api/access/v1/browser-sessions/others",
    operation_id = "revoke_other_browser_sessions",
    tag = "access",
    security(("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    responses(
        (status = 200, description = "Other-session revocation count", body = RevokeBrowserSessionsResponse),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Origin, Host, or CSRF proof rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn revoke_other_browser_sessions(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    revoke_many(
        state,
        headers,
        CapabilityKey::RevokeOtherBrowserSessions,
        false,
    )
    .await
}

#[utoipa::path(
    delete,
    path = "/api/access/v1/browser-sessions",
    operation_id = "revoke_all_browser_sessions",
    tag = "access",
    security(("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    responses(
        (status = 200, description = "All-session revocation count", body = RevokeBrowserSessionsResponse),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Origin, Host, or CSRF proof rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn revoke_all_browser_sessions(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    revoke_many(
        state,
        headers,
        CapabilityKey::RevokeAllBrowserSessions,
        true,
    )
    .await
}

async fn revoke_many(
    state: AccessApiState,
    headers: HeaderMap,
    capability: CapabilityKey,
    clear_current: bool,
) -> HttpResponse {
    let correlation_id = RequestCorrelationId::new_v7();
    let command =
        browser_session_mutation_command(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    let revoked_count = run_kernel(capability, correlation_id, move || match capability {
        CapabilityKey::RevokeOtherBrowserSessions => kernel.revoke_other_browser_sessions(command),
        CapabilityKey::RevokeAllBrowserSessions => kernel.revoke_all_browser_sessions(command),
        _ => unreachable!("revoke_many is called only for aggregate revocation capabilities"),
    })
    .await?;
    let mut response = Json(RevokeBrowserSessionsResponse { revoked_count }).into_response();
    if clear_current {
        clear_session_cookies(&mut response);
    }
    Ok(no_store(response))
}

#[utoipa::path(
    post,
    path = "/api/access/v1/browser-session/rotation",
    operation_id = "rotate_browser_session",
    tag = "access",
    security(("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    responses(
        (status = 200, description = "Rotated browser session", body = RotateBrowserSessionResponse),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Origin, Host, or CSRF proof rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn rotate_browser_session(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::RotateBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let command =
        browser_session_mutation_command(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    let created = run_kernel(capability, correlation_id, move || {
        kernel.rotate_browser_session(command)
    })
    .await?;
    let mut response = Json(RotateBrowserSessionResponse {
        session: session_dto(created.session(), true),
    })
    .into_response();
    set_session_cookies(&mut response, &created);
    Ok(no_store(response))
}

#[utoipa::path(
    put,
    path = "/api/access/v1/browser-session/profile",
    operation_id = "select_browser_session_profile",
    tag = "access",
    security(("browser_session_cookie" = [], "csrf_cookie" = [], "csrf_header" = [])),
    request_body = SelectBrowserSessionProfileRequest,
    responses(
        (status = 200, description = "Browser session with selected profile", body = SelectBrowserSessionProfileResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Browser session is inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Origin, Host, or CSRF proof rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request payload is too large", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Request media type is unsupported", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Profile grant is invalid or unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored session failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn select_browser_session_profile(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
    request: Result<Json<SelectBrowserSessionProfileRequest>, JsonRejection>,
) -> HttpResponse {
    let capability = CapabilityKey::SelectBrowserSessionProfile;
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let profile_grant_id = ProfileGrantId::from_str(&request.profile_grant_id).map_err(|_| {
        invalid_request(
            capability,
            correlation_id,
            "/profile_grant_id",
            "a canonical Fasti profile-grant identifier",
        )
    })?;
    let proof =
        browser_session_mutation_command(&headers, &state.boundary, capability, correlation_id)?;
    let kernel = state.kernel;
    let created = run_kernel(capability, correlation_id, move || {
        kernel.select_browser_session_profile(SelectBrowserSessionProfileCommand::new(
            proof,
            profile_grant_id,
        ))
    })
    .await?;
    let mut response = Json(SelectBrowserSessionProfileResponse {
        session: session_dto(created.session(), true),
    })
    .into_response();
    set_session_cookies(&mut response, &created);
    Ok(no_store(response))
}

pub(crate) fn router(
    kernel: Arc<dyn LocalKernel>,
    boundary: fasti_application::BrowserRequestBoundaryPolicy,
    trailbase: Option<Arc<TrailBaseOrchestrator>>,
) -> Router {
    Router::new()
        .route(
            "/api/access/v1/trailbase/sign-in",
            post(start_trailbase_sign_in),
        )
        .route(
            FASTI_ACCESS_CALLBACK_PATH,
            any(complete_trailbase_authentication),
        )
        .route("/api/access/v1/projection", get(read_access_projection))
        .route(
            "/api/access/v1/browser-session",
            get(read_browser_session).delete(end_browser_session),
        )
        .route(
            "/api/access/v1/browser-sessions",
            get(list_browser_sessions).delete(revoke_all_browser_sessions),
        )
        .route(
            "/api/access/v1/browser-sessions/others",
            delete(revoke_other_browser_sessions),
        )
        .route(
            "/api/access/v1/browser-sessions/{browser_session_id}",
            delete(revoke_browser_session),
        )
        .route(
            "/api/access/v1/browser-session/rotation",
            post(rotate_browser_session),
        )
        .route(
            "/api/access/v1/browser-session/profile",
            put(select_browser_session_profile),
        )
        .layer(DefaultBodyLimit::max(MAX_ACCESS_JSON_BODY_BYTES))
        .with_state(AccessApiState {
            kernel,
            boundary,
            trailbase,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cookie_attributes_keep_credentials_out_of_script_and_cross_site_requests() {
        let mut response = StatusCode::NO_CONTENT.into_response();
        clear_session_cookies(&mut response);
        let values: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().expect("cookie header"))
            .collect();
        assert_eq!(values.len(), 2);
        assert!(values[0].contains("Secure; HttpOnly; SameSite=Strict"));
        assert!(values[1].contains("Secure; SameSite=Strict"));
        assert!(values.iter().all(|value| !value.contains("Domain=")));
    }

    #[test]
    fn callback_query_parser_accepts_only_the_exact_unencoded_code() {
        let valid = format!("code={}", "aB3".repeat(16));
        assert_eq!(exact_callback_code(Some(&valid)), Some("aB3".repeat(16)));
        for invalid in [
            None,
            Some(""),
            Some("code=short"),
            Some("code=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&extra=1"),
            Some("code=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa%61"),
            Some("code=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa+"),
        ] {
            assert!(exact_callback_code(invalid).is_none());
        }
    }

    #[test]
    fn callback_failure_is_fixed_and_always_clears_the_binding() {
        let correlation_id = RequestCorrelationId::new_v7();
        let response = callback_redirect(AuthReturnTarget::FirstRun, correlation_id, None);
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some(format!("/first-run?auth=failed&correlation_id={correlation_id}").as_str())
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("private, no-store"))
        );
        let cookies: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .collect();
        assert_eq!(cookies.len(), 1);
        assert!(cookies[0]
            .to_str()
            .expect("cookie")
            .starts_with("__Secure-fasti_auth_binding=;"));
    }

    #[tokio::test]
    async fn trailbase_start_preserves_safe_application_problem_semantics() {
        let correlation_id = RequestCorrelationId::new_v7();
        for (code, status) in [
            (
                ProblemCode::CapacityExceeded,
                StatusCode::INSUFFICIENT_STORAGE,
            ),
            (
                ProblemCode::IntegrityFailed,
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                ProblemCode::StorageUnavailable,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
        ] {
            let response = trailbase_start_problem(
                TrailBaseOrchestrationError::ApplicationProblem(code),
                correlation_id,
            )
            .into_response();
            assert_eq!(response.status(), status);
            let body = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .expect("bounded problem body");
            let problem: ProblemDetails = serde_json::from_slice(&body).expect("problem details");
            assert_eq!(problem.code, code.as_str());
            assert_eq!(problem.capability_id, "browser.session.create");
            assert_eq!(problem.correlation_id, correlation_id.to_string());
            let public_body = std::str::from_utf8(&body).expect("UTF-8 problem body");
            assert!(!public_body.contains("vault") && !public_body.contains("sqlite"));
        }
    }
}
