use crate::local::{
    browser_session_mutation_command, browser_session_query, run_kernel, CSRF_COOKIE,
    SESSION_COOKIE,
};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use crate::trailbase::{
    sha256_digest, TrailBaseCallbackOutcome, TrailBaseOrchestrationError, TrailBaseOrchestrator,
};
use crate::{
    FASTI_ACCESS_BINDING_COOKIE, FASTI_ACCESS_CALLBACK_PATH, FASTI_ACCESS_CALLBACK_URL,
    FASTI_ACCESS_CONTINUATION_COOKIE, FASTI_ACCESS_CONTINUATION_PATH, FASTI_ACCESS_HOST,
};
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
    AccessTrailBaseActivationSummary, AuthSelectionChoice, BrowserSessionSummary,
    CancelTrailBaseSignInContinuationCommand, CapabilityKey,
    CompleteTrailBaseSignInContinuationCommand, CreatedBrowserSession, FastiProblem, LocalKernel,
    ProblemCode, ReadTrailBaseSignInContinuationQuery, SelectBrowserSessionProfileCommand,
    TargetBrowserSessionCommand, Violation, C1_AUTH_CEREMONY_LIFETIME,
};
use fasti_contracts::{
    AccessAuthenticationMethodDto, AccessCeremonyFailureDto, AccessCeremonyStateDto,
    AccessEvidenceDto, AccessEvidenceKindDto, AccessEvidenceStateDto, AccessFirstRunStepDto,
    AccessFirstRunStepKeyDto, AccessMembershipDto, AccessMembershipLifecycleDto,
    AccessProfileGrantDto, AccessProjectionResponse, AccessSessionAuthenticationDto,
    AccessSubjectDto, AccessSubjectLifecycleDto, AccessWorkspaceRoleDto, BrowserSessionDto,
    BrowserSessionPolicyDto, CompleteTrailBaseAuthenticationQuery,
    CompleteTrailBaseContinuationRequest, ListBrowserSessionsResponse, ProblemDetails,
    ReadBrowserSessionResponse, ReadTrailBaseContinuationResponse, RecentAuthenticationDto,
    RevokeBrowserSessionsResponse, RotateBrowserSessionResponse,
    SelectBrowserSessionProfileRequest, SelectBrowserSessionProfileResponse,
    StartTrailBaseSignInRequest, StartTrailBaseSignInResponse, TrailBaseActivationBlockerDto,
    TrailBaseActivationDto, TrailBaseActivationStateDto, TrailBaseContinuationChoiceDto,
};
use fasti_domain::{
    AuthCeremonyFailure, AuthCeremonyState, AuthReturnTarget, AuthSubjectLifecycle,
    AuthenticationMethod, BrowserSessionId, FastiBrowserSession, MembershipLifecycle,
    ProfileGrantId, RequestCorrelationId, Sha256Digest, TrailBaseActivationBlocker,
    TrailBaseActivationState, WorkspaceRole,
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

fn membership_lifecycle_dto(value: MembershipLifecycle) -> AccessMembershipLifecycleDto {
    match value {
        MembershipLifecycle::Invited => AccessMembershipLifecycleDto::Invited,
        MembershipLifecycle::PendingApproval => AccessMembershipLifecycleDto::PendingApproval,
        MembershipLifecycle::Active => AccessMembershipLifecycleDto::Active,
        MembershipLifecycle::Suspended => AccessMembershipLifecycleDto::Suspended,
        MembershipLifecycle::Removed => AccessMembershipLifecycleDto::Removed,
    }
}

fn workspace_role_dto(value: WorkspaceRole) -> AccessWorkspaceRoleDto {
    match value {
        WorkspaceRole::Member => AccessWorkspaceRoleDto::Member,
        WorkspaceRole::Administrator => AccessWorkspaceRoleDto::Administrator,
    }
}

fn membership_dto(membership: AccessMembershipSummary) -> AccessMembershipDto {
    AccessMembershipDto {
        membership_id: membership.id().to_string(),
        workspace_id: membership.workspace_id().to_string(),
        lifecycle: membership_lifecycle_dto(membership.lifecycle()),
        role: workspace_role_dto(membership.role()),
        created_at: membership.created_at().to_rfc3339(),
        updated_at: membership.updated_at().to_rfc3339(),
    }
}

fn continuation_choice_dto(choice: AuthSelectionChoice) -> TrailBaseContinuationChoiceDto {
    TrailBaseContinuationChoiceDto {
        choice_ordinal: choice.ordinal(),
        workspace_ordinal: choice.workspace_ordinal(),
        profile_ordinal: choice.profile_ordinal(),
        workspace_created_at: choice.workspace_created_at().to_rfc3339(),
        profile_created_at: choice.profile_created_at().to_rfc3339(),
        membership_state: membership_lifecycle_dto(choice.membership_state()),
        role: workspace_role_dto(choice.role()),
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
            AuthCeremonyState::SelectionRequired => AccessCeremonyStateDto::SelectionRequired,
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
            AuthCeremonyFailure::LocalPersistenceFailed => {
                AccessCeremonyFailureDto::LocalPersistenceFailed
            }
            AuthCeremonyFailure::TrustUnavailable => AccessCeremonyFailureDto::TrustUnavailable,
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

fn clear_continuation_cookie(response: &mut Response) {
    append_cookie(
        response,
        format!(
            "{FASTI_ACCESS_CONTINUATION_COOKIE}=; Domain=127.0.0.1; Path={FASTI_ACCESS_CONTINUATION_PATH}; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Secure; HttpOnly; SameSite=Strict"
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

pub(crate) fn exact_callback_url_code(value: &str) -> Option<String> {
    exact_callback_code(
        value
            .strip_prefix(FASTI_ACCESS_CALLBACK_URL)?
            .strip_prefix('?'),
    )
}

fn exact_host(headers: &HeaderMap) -> bool {
    exact_header(headers, header::HOST) == Some(FASTI_ACCESS_HOST)
}

fn exact_header(headers: &HeaderMap, name: header::HeaderName) -> Option<&str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}

fn secret_cookie(
    headers: &HeaderMap,
    expected_name: &str,
) -> Option<fasti_application::SecretMaterial> {
    let mut binding = None;
    for header_value in headers.get_all(header::COOKIE) {
        let value = header_value.to_str().ok()?;
        for pair in value.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name == expected_name && binding.replace(value).is_some() {
                return None;
            }
        }
    }
    fasti_application::SecretMaterial::try_from_hex(binding?).ok()
}

fn callback_binding(headers: &HeaderMap) -> Option<fasti_application::SecretMaterial> {
    secret_cookie(headers, FASTI_ACCESS_BINDING_COOKIE)
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

fn callback_selection_redirect(
    target: AuthReturnTarget,
    binding: &fasti_application::SecretMaterial,
    expires_at: chrono::DateTime<chrono::Utc>,
    correlation_id: RequestCorrelationId,
    failed: bool,
) -> Response {
    let max_age = (expires_at - chrono::Utc::now()).num_seconds();
    if max_age <= 0 {
        return callback_redirect(target, correlation_id, None);
    }
    let mut response = StatusCode::SEE_OTHER.into_response();
    let location = if failed {
        format!(
            "{}?auth=failed&correlation_id={correlation_id}",
            return_path(target)
        )
    } else {
        format!("{}?auth=continue", return_path(target))
    };
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(&location).expect("fixed return path is valid"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    clear_binding_cookie(&mut response);
    append_cookie(
        &mut response,
        format!(
            "{FASTI_ACCESS_CONTINUATION_COOKIE}={}; Domain=127.0.0.1; Path={FASTI_ACCESS_CONTINUATION_PATH}; Max-Age={max_age}; Secure; HttpOnly; SameSite=Strict",
            binding.expose_hex()
        ),
    );
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
            exact_header(&headers, header::ORIGIN),
            exact_header(&headers, header::HOST),
        )
        .map_err(|_| {
            application_problem(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )))
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
        orchestrator.start_sign_in(request.remembered, correlation_id, created_at, expires_at)
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
        .callback_for_browser(code, &binding, correlation_id, at)
        .await
    {
        Ok(TrailBaseCallbackOutcome::SessionCreated {
            created,
            return_target,
        }) => callback_redirect(return_target, correlation_id, Some(created.as_ref())),
        Ok(TrailBaseCallbackOutcome::SelectionRequired {
            expires_at,
            return_target,
        }) => {
            callback_selection_redirect(return_target, &binding, expires_at, correlation_id, false)
        }
        Err(failure) => match (failure.return_target, failure.continuation_expires_at) {
            (Some(return_target), Some(expires_at)) => callback_selection_redirect(
                return_target,
                &binding,
                expires_at,
                correlation_id,
                true,
            ),
            (return_target, _) => callback_redirect(
                return_target.unwrap_or(fallback_target),
                correlation_id,
                None,
            ),
        },
    }
}

fn continuation_binding_digest(
    headers: &HeaderMap,
    boundary: &fasti_application::BrowserRequestBoundaryPolicy,
    require_origin: bool,
    correlation_id: RequestCorrelationId,
) -> Result<Sha256Digest, HttpProblem> {
    let capability = CapabilityKey::CreateBrowserSession;
    if headers.contains_key(header::AUTHORIZATION) || !exact_host(headers) {
        return Err(application_problem(Box::new(FastiProblem::forbidden(
            capability,
            correlation_id,
        ))));
    }
    let boundary_result = if require_origin {
        boundary
            .validate(
                exact_header(headers, header::ORIGIN),
                exact_header(headers, header::HOST),
            )
            .map(|_| ())
    } else {
        boundary
            .validate_read(exact_header(headers, header::HOST))
            .map(|_| ())
    };
    boundary_result.map_err(|_| {
        application_problem(Box::new(FastiProblem::forbidden(
            capability,
            correlation_id,
        )))
    })?;
    let binding = secret_cookie(headers, FASTI_ACCESS_CONTINUATION_COOKIE).ok_or_else(|| {
        application_problem(Box::new(FastiProblem::from_code(
            ProblemCode::AuthBrowserBindingInvalid,
            capability,
            correlation_id,
        )))
    })?;
    Ok(sha256_digest(binding.expose_bytes()))
}

fn continuation_problem(problem: HttpProblem) -> HttpResponse {
    if matches!(
        problem.code(),
        "auth_browser_binding_invalid" | "trailbase_proof_invalid"
    ) {
        let mut response = problem.into_response();
        clear_continuation_cookie(&mut response);
        Ok(no_store(response))
    } else {
        Err(problem)
    }
}

#[utoipa::path(
    get,
    path = "/api/access/v1/trailbase/continuation",
    operation_id = "read_trailbase_continuation",
    tag = "access",
    security(("auth_continuation_cookie" = [])),
    responses(
        (status = 200, description = "Bounded identifier-free sign-in choices", body = ReadTrailBaseContinuationResponse),
        (status = 401, description = "Continuation binding or proof is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "No selectable Fasti affiliation", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Continuation input is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 502, description = "TrailBase session cleanup was not confirmed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored continuation state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "TrailBase identity, trust, or local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "The bounded selection capacity was exceeded", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_trailbase_continuation(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::CreateBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let digest = match continuation_binding_digest(&headers, &state.boundary, false, correlation_id)
    {
        Ok(digest) => digest,
        Err(problem) => return continuation_problem(problem),
    };
    let kernel = state.kernel;
    let result = run_kernel(capability, correlation_id, move || {
        kernel.read_trailbase_sign_in_continuation(ReadTrailBaseSignInContinuationQuery::new(
            digest,
            correlation_id,
            chrono::Utc::now(),
        ))
    })
    .await;
    match result {
        Ok(projection) => Ok(no_store(
            Json(ReadTrailBaseContinuationResponse {
                expires_at: projection.expires_at().to_rfc3339(),
                remembered: projection.remembered(),
                candidate_revision: projection.candidate_revision().to_string(),
                choices: projection
                    .choices()
                    .iter()
                    .copied()
                    .map(continuation_choice_dto)
                    .collect(),
            })
            .into_response(),
        )),
        Err(problem) => continuation_problem(problem),
    }
}

#[utoipa::path(
    post,
    path = "/api/access/v1/trailbase/continuation",
    operation_id = "complete_trailbase_continuation",
    tag = "access",
    security(("auth_continuation_cookie" = [])),
    request_body = CompleteTrailBaseContinuationRequest,
    responses(
        (status = 204, description = "Opaque Fasti browser session issued"),
        (status = 400, description = "Malformed request body", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Continuation binding or proof is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Continuation request is not authorized", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Sign-in choices changed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body is too large", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Request media type is unsupported", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Choice or revision is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 502, description = "TrailBase session cleanup was not confirmed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored continuation state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "TrailBase identity, trust, or local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "The bounded selection capacity was exceeded", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn complete_trailbase_continuation(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
    payload: Result<Json<CompleteTrailBaseContinuationRequest>, JsonRejection>,
) -> HttpResponse {
    let capability = CapabilityKey::CreateBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let digest = match continuation_binding_digest(&headers, &state.boundary, true, correlation_id)
    {
        Ok(digest) => digest,
        Err(problem) => return continuation_problem(problem),
    };
    let request = payload
        .map_err(|error| json_rejection(capability, correlation_id, error))?
        .0;
    let candidate_revision = Sha256Digest::parse(&request.candidate_revision).map_err(|_| {
        invalid_request(
            capability,
            correlation_id,
            "/candidate_revision",
            "a canonical SHA-256 candidate revision",
        )
    })?;
    let kernel = state.kernel;
    let created = match run_kernel(capability, correlation_id, move || {
        kernel.complete_trailbase_sign_in_continuation(
            CompleteTrailBaseSignInContinuationCommand::new(
                digest,
                request.choice_ordinal,
                candidate_revision,
                correlation_id,
                chrono::Utc::now(),
            ),
        )
    })
    .await
    {
        Ok(created) => created,
        Err(problem) => return continuation_problem(problem),
    };
    let mut response = StatusCode::NO_CONTENT.into_response();
    clear_continuation_cookie(&mut response);
    set_session_cookies(&mut response, &created);
    Ok(no_store(response))
}

#[utoipa::path(
    delete,
    path = "/api/access/v1/trailbase/continuation",
    operation_id = "cancel_trailbase_continuation",
    tag = "access",
    security(("auth_continuation_cookie" = [])),
    responses(
        (status = 204, description = "Sign-in continuation cancelled"),
        (status = 401, description = "Continuation binding or proof is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Continuation request is not authorized", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Continuation input is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Stored continuation state failed integrity checks", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn cancel_trailbase_continuation(
    State(state): State<AccessApiState>,
    headers: HeaderMap,
) -> HttpResponse {
    let capability = CapabilityKey::CreateBrowserSession;
    let correlation_id = RequestCorrelationId::new_v7();
    let digest = match continuation_binding_digest(&headers, &state.boundary, true, correlation_id)
    {
        Ok(digest) => digest,
        Err(problem) => return continuation_problem(problem),
    };
    let kernel = state.kernel;
    if let Err(problem) = run_kernel(capability, correlation_id, move || {
        kernel.cancel_trailbase_sign_in_continuation(CancelTrailBaseSignInContinuationCommand::new(
            digest,
            correlation_id,
            chrono::Utc::now(),
        ))
    })
    .await
    {
        return continuation_problem(problem);
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    clear_continuation_cookie(&mut response);
    Ok(no_store(response))
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
        .route(
            FASTI_ACCESS_CONTINUATION_PATH,
            get(read_trailbase_continuation)
                .post(complete_trailbase_continuation)
                .delete(cancel_trailbase_continuation),
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
    fn operator_callback_parser_requires_the_exact_fixed_url() {
        let code = "aB3".repeat(16);
        let valid = format!("{FASTI_ACCESS_CALLBACK_URL}?code={code}");
        assert_eq!(exact_callback_url_code(&valid), Some(code));
        for invalid in [
            format!(" {valid}"),
            format!("{valid} "),
            format!("{valid}&extra=1"),
            format!("{valid}#fragment"),
            valid.replace("127.0.0.1", "localhost"),
            valid.replace(FASTI_ACCESS_CALLBACK_PATH, "/wrong"),
        ] {
            assert!(exact_callback_url_code(&invalid).is_none());
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

    #[test]
    fn continuation_binding_rejects_ambiguous_or_missing_browser_authority() {
        let boundary = fasti_application::BrowserRequestBoundaryPolicy::try_new(
            crate::FASTI_ACCESS_ORIGIN,
            FASTI_ACCESS_HOST,
        )
        .expect("fixed browser boundary");
        let exact = || {
            let mut headers = HeaderMap::new();
            headers.insert(header::HOST, HeaderValue::from_static(FASTI_ACCESS_HOST));
            headers.insert(
                header::ORIGIN,
                HeaderValue::from_static(crate::FASTI_ACCESS_ORIGIN),
            );
            headers.insert(
                header::COOKIE,
                HeaderValue::from_str(&format!(
                    "{FASTI_ACCESS_CONTINUATION_COOKIE}={}",
                    "a".repeat(64)
                ))
                .expect("cookie header"),
            );
            headers
        };

        let mut exact_read = exact();
        exact_read.remove(header::ORIGIN);
        assert!(continuation_binding_digest(
            &exact_read,
            &boundary,
            false,
            RequestCorrelationId::new_v7(),
        )
        .is_ok());
        assert!(continuation_binding_digest(
            &exact(),
            &boundary,
            true,
            RequestCorrelationId::new_v7(),
        )
        .is_ok());

        let mut cases = Vec::new();
        let mut headers = exact();
        headers.remove(header::HOST);
        cases.push(("missing Host", true, headers, "forbidden"));
        let mut headers = exact();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:8421"));
        cases.push(("wrong Host", true, headers, "forbidden"));
        let mut headers = exact();
        headers.append(header::HOST, HeaderValue::from_static(FASTI_ACCESS_HOST));
        cases.push(("duplicate Host", true, headers, "forbidden"));
        let mut headers = exact();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer forbidden"),
        );
        cases.push(("bearer credential", true, headers, "forbidden"));
        let mut headers = exact();
        headers.remove(header::ORIGIN);
        cases.push(("missing Origin", true, headers, "forbidden"));
        let mut headers = exact();
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://example.test"),
        );
        cases.push(("wrong Origin", true, headers, "forbidden"));
        let mut headers = exact();
        headers.append(
            header::ORIGIN,
            HeaderValue::from_static(crate::FASTI_ACCESS_ORIGIN),
        );
        cases.push(("duplicate Origin", true, headers, "forbidden"));
        let mut headers = exact();
        headers.remove(header::COOKIE);
        cases.push((
            "missing continuation cookie",
            false,
            headers,
            "auth_browser_binding_invalid",
        ));
        let mut headers = exact();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("__Secure-fasti_auth_continuation=not-hex"),
        );
        cases.push((
            "malformed continuation cookie",
            false,
            headers,
            "auth_browser_binding_invalid",
        ));
        let mut headers = exact();
        headers.append(
            header::COOKIE,
            HeaderValue::from_str(&format!(
                "{FASTI_ACCESS_CONTINUATION_COOKIE}={}",
                "b".repeat(64)
            ))
            .expect("duplicate cookie header"),
        );
        cases.push((
            "duplicate continuation cookie",
            false,
            headers,
            "auth_browser_binding_invalid",
        ));

        for (label, require_origin, headers, expected) in cases {
            let error = continuation_binding_digest(
                &headers,
                &boundary,
                require_origin,
                RequestCorrelationId::new_v7(),
            )
            .expect_err(label);
            assert_eq!(error.code(), expected, "{label}");
        }
    }

    #[test]
    fn terminal_continuation_problem_clears_the_path_scoped_cookie() {
        let problem = application_problem(Box::new(FastiProblem::from_code(
            ProblemCode::AuthBrowserBindingInvalid,
            CapabilityKey::CreateBrowserSession,
            RequestCorrelationId::new_v7(),
        )));
        let response = match continuation_problem(problem) {
            Ok(response) => response,
            Err(_) => panic!("terminal continuation problem must be an HTTP response"),
        };
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response
                .headers()
                .get(header::SET_COOKIE)
                .and_then(|value| value.to_str().ok()),
            Some("__Secure-fasti_auth_continuation=; Domain=127.0.0.1; Path=/api/access/v1/trailbase/continuation; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Secure; HttpOnly; SameSite=Strict")
        );
    }

    #[test]
    fn attributable_continuation_problem_keeps_the_cookie_for_dismissal() {
        let problem = application_problem(Box::new(FastiProblem::from_code(
            ProblemCode::IdentityServiceUnavailable,
            CapabilityKey::CreateBrowserSession,
            RequestCorrelationId::new_v7(),
        )));
        let response = match continuation_problem(problem) {
            Err(problem) => problem.into_response(),
            Ok(_) => panic!("attributable evidence must retain continuation authority"),
        };
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }

    #[test]
    fn attributable_callback_rotates_the_same_binding_with_exact_cookie_scope() {
        let binding = fasti_application::SecretMaterial::from_bytes([7; 32]);
        let correlation_id = RequestCorrelationId::new_v7();
        let response = callback_selection_redirect(
            AuthReturnTarget::ApplicationHome,
            &binding,
            chrono::Utc::now() + chrono::Duration::minutes(5),
            correlation_id,
            true,
        );
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some(format!("/?auth=failed&correlation_id={correlation_id}").as_str())
        );
        let cookies: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().expect("cookie header"))
            .collect();
        assert_eq!(cookies.len(), 2);
        assert!(cookies[0].starts_with("__Secure-fasti_auth_binding=;"));
        assert!(cookies[1].starts_with(&format!(
            "{FASTI_ACCESS_CONTINUATION_COOKIE}={};",
            binding.expose_hex()
        )));
        assert!(
            cookies[1].contains("Domain=127.0.0.1; Path=/api/access/v1/trailbase/continuation;")
        );
        let max_age = cookies[1]
            .split("; ")
            .find_map(|part| part.strip_prefix("Max-Age="))
            .and_then(|value| value.parse::<u64>().ok())
            .expect("numeric continuation Max-Age");
        assert!(max_age > 0 && max_age <= 300);
        assert!(cookies[1].contains("Secure; HttpOnly; SameSite=Strict"));
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
