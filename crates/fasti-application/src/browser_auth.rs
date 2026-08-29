use crate::{ApplicationResult, SecretMaterial};
use chrono::{DateTime, Utc};
use fasti_domain::{
    AuthSubject, AuthSubjectId, BrowserSessionId, FastiBrowserSession, ProfileGrantId,
    RequestCorrelationId, WorkspaceId,
};
use std::{collections::HashSet, error::Error, fmt, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicyInputError;

impl fmt::Display for SessionPolicyInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "session policy durations must be positive whole seconds and internally ordered",
        )
    }
}

impl Error for SessionPolicyInputError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserRequestBoundaryError {
    InvalidPolicy,
    MissingOrigin,
    MissingHost,
    OriginMismatch,
    HostMismatch,
}

impl fmt::Display for BrowserRequestBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPolicy => "browser request boundary policy is invalid",
            Self::MissingOrigin => "browser mutation is missing Origin",
            Self::MissingHost => "browser mutation is missing Host",
            Self::OriginMismatch => "browser mutation Origin is not allowed",
            Self::HostMismatch => "browser mutation Host is not allowed",
        })
    }
}

impl Error for BrowserRequestBoundaryError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserRequestBoundaryPolicy {
    allowed_origin: String,
    allowed_host: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedBrowserRequestBoundary(());

impl BrowserRequestBoundaryPolicy {
    pub fn try_new(
        allowed_origin: impl Into<String>,
        allowed_host: impl Into<String>,
    ) -> Result<Self, BrowserRequestBoundaryError> {
        let allowed_origin = allowed_origin.into();
        let allowed_host = allowed_host.into();
        let authority = allowed_origin
            .strip_prefix("https://")
            .or_else(|| allowed_origin.strip_prefix("http://"));
        if authority.is_none_or(|value| {
            value.is_empty()
                || value
                    .bytes()
                    .any(|byte| matches!(byte, b'/' | b'?' | b'#' | b',' | b'@'))
        }) || allowed_origin
            .bytes()
            .any(|byte| byte.is_ascii_whitespace())
            || allowed_host.is_empty()
            || allowed_host
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || byte == b',')
            || !authority.is_some_and(|value| value.eq_ignore_ascii_case(&allowed_host))
        {
            return Err(BrowserRequestBoundaryError::InvalidPolicy);
        }
        Ok(Self {
            allowed_origin,
            allowed_host,
        })
    }

    pub fn validate(
        &self,
        origin: Option<&str>,
        host: Option<&str>,
    ) -> Result<ValidatedBrowserRequestBoundary, BrowserRequestBoundaryError> {
        let origin = origin.ok_or(BrowserRequestBoundaryError::MissingOrigin)?;
        let host = host.ok_or(BrowserRequestBoundaryError::MissingHost)?;
        if !origin.eq_ignore_ascii_case(&self.allowed_origin) {
            return Err(BrowserRequestBoundaryError::OriginMismatch);
        }
        if !host.eq_ignore_ascii_case(&self.allowed_host) {
            return Err(BrowserRequestBoundaryError::HostMismatch);
        }
        Ok(ValidatedBrowserRequestBoundary(()))
    }
}

/// Governed browser-session timings.
///
/// PR A deliberately has no `Default`: the approved plan does not provide
/// source-backed values. The configuration owner must supply the approved
/// values before C1 can issue a production session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicy {
    browser_idle_timeout: Duration,
    browser_absolute_lifetime: Duration,
    remembered_browser_lifetime: Duration,
    last_seen_write_interval: Duration,
}

impl SessionPolicy {
    pub fn try_new(
        browser_idle_timeout: Duration,
        browser_absolute_lifetime: Duration,
        remembered_browser_lifetime: Duration,
        last_seen_write_interval: Duration,
    ) -> Result<Self, SessionPolicyInputError> {
        let durations = [
            browser_idle_timeout,
            browser_absolute_lifetime,
            remembered_browser_lifetime,
            last_seen_write_interval,
        ];
        if durations
            .iter()
            .any(|duration| duration.is_zero() || duration.subsec_nanos() != 0)
            || browser_idle_timeout > browser_absolute_lifetime
            || browser_absolute_lifetime > remembered_browser_lifetime
            || last_seen_write_interval > browser_idle_timeout
        {
            return Err(SessionPolicyInputError);
        }
        Ok(Self {
            browser_idle_timeout,
            browser_absolute_lifetime,
            remembered_browser_lifetime,
            last_seen_write_interval,
        })
    }

    pub const fn browser_idle_timeout(self) -> Duration {
        self.browser_idle_timeout
    }
    pub const fn browser_absolute_lifetime(self) -> Duration {
        self.browser_absolute_lifetime
    }
    pub const fn remembered_browser_lifetime(self) -> Duration {
        self.remembered_browser_lifetime
    }
    pub const fn last_seen_write_interval(self) -> Duration {
        self.last_seen_write_interval
    }
    pub const fn absolute_lifetime(self, remembered: bool) -> Duration {
        if remembered {
            self.remembered_browser_lifetime
        } else {
            self.browser_absolute_lifetime
        }
    }
}

pub struct CreateAuthSubjectCommand {
    correlation_id: RequestCorrelationId,
    subject: AuthSubject,
}

impl CreateAuthSubjectCommand {
    pub const fn new(correlation_id: RequestCorrelationId, subject: AuthSubject) -> Self {
        Self {
            correlation_id,
            subject,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn subject(&self) -> AuthSubject {
        self.subject
    }
}

pub struct CreateBrowserSessionCommand {
    correlation_id: RequestCorrelationId,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    authorized_profile_grants: Vec<ProfileGrantId>,
    selected_profile_grant_id: ProfileGrantId,
    policy: SessionPolicy,
    remembered: bool,
    now: DateTime<Utc>,
}

impl CreateBrowserSessionCommand {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        correlation_id: RequestCorrelationId,
        subject_id: AuthSubjectId,
        workspace_id: WorkspaceId,
        authorized_profile_grants: Vec<ProfileGrantId>,
        selected_profile_grant_id: ProfileGrantId,
        policy: SessionPolicy,
        remembered: bool,
        now: DateTime<Utc>,
    ) -> Result<Self, SessionPolicyInputError> {
        let unique: HashSet<_> = authorized_profile_grants.iter().copied().collect();
        if unique.len() != authorized_profile_grants.len()
            || !unique.contains(&selected_profile_grant_id)
        {
            return Err(SessionPolicyInputError);
        }
        Ok(Self {
            correlation_id,
            subject_id,
            workspace_id,
            authorized_profile_grants,
            selected_profile_grant_id,
            policy,
            remembered,
            now,
        })
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn subject_id(&self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub fn authorized_profile_grants(&self) -> &[ProfileGrantId] {
        &self.authorized_profile_grants
    }
    pub const fn selected_profile_grant_id(&self) -> ProfileGrantId {
        self.selected_profile_grant_id
    }
    pub const fn policy(&self) -> SessionPolicy {
        self.policy
    }
    pub const fn remembered(&self) -> bool {
        self.remembered
    }
    pub const fn now(&self) -> DateTime<Utc> {
        self.now
    }
}

pub struct CreatedBrowserSession {
    session: FastiBrowserSession,
    session_secret: SecretMaterial,
    csrf_secret: SecretMaterial,
}

impl CreatedBrowserSession {
    pub const fn new(
        session: FastiBrowserSession,
        session_secret: SecretMaterial,
        csrf_secret: SecretMaterial,
    ) -> Self {
        Self {
            session,
            session_secret,
            csrf_secret,
        }
    }
    pub const fn session(&self) -> FastiBrowserSession {
        self.session
    }
    pub const fn session_secret(&self) -> &SecretMaterial {
        &self.session_secret
    }
    pub const fn csrf_secret(&self) -> &SecretMaterial {
        &self.csrf_secret
    }
}

pub struct BrowserSessionQuery {
    correlation_id: RequestCorrelationId,
    session_secret: SecretMaterial,
    now: DateTime<Utc>,
}

impl BrowserSessionQuery {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session_secret: SecretMaterial,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            correlation_id,
            session_secret,
            now,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session_secret(&self) -> &SecretMaterial {
        &self.session_secret
    }
    pub const fn now(&self) -> DateTime<Utc> {
        self.now
    }
}

pub struct BrowserSessionMutationCommand {
    correlation_id: RequestCorrelationId,
    session_secret: SecretMaterial,
    csrf_secret: SecretMaterial,
    _request_boundary: ValidatedBrowserRequestBoundary,
    now: DateTime<Utc>,
}

impl BrowserSessionMutationCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session_secret: SecretMaterial,
        csrf_secret: SecretMaterial,
        request_boundary: ValidatedBrowserRequestBoundary,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            correlation_id,
            session_secret,
            csrf_secret,
            _request_boundary: request_boundary,
            now,
        }
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn session_secret(&self) -> &SecretMaterial {
        &self.session_secret
    }
    pub const fn csrf_secret(&self) -> &SecretMaterial {
        &self.csrf_secret
    }
    pub const fn now(&self) -> DateTime<Utc> {
        self.now
    }
}

pub struct TargetBrowserSessionCommand {
    proof: BrowserSessionMutationCommand,
    target_session_id: BrowserSessionId,
}

impl TargetBrowserSessionCommand {
    pub const fn new(
        proof: BrowserSessionMutationCommand,
        target_session_id: BrowserSessionId,
    ) -> Self {
        Self {
            proof,
            target_session_id,
        }
    }
    pub const fn proof(&self) -> &BrowserSessionMutationCommand {
        &self.proof
    }
    pub const fn target_session_id(&self) -> BrowserSessionId {
        self.target_session_id
    }
}

pub struct SelectBrowserSessionProfileCommand {
    proof: BrowserSessionMutationCommand,
    target_profile_grant_id: ProfileGrantId,
}

impl SelectBrowserSessionProfileCommand {
    pub const fn new(
        proof: BrowserSessionMutationCommand,
        target_profile_grant_id: ProfileGrantId,
    ) -> Self {
        Self {
            proof,
            target_profile_grant_id,
        }
    }
    pub const fn proof(&self) -> &BrowserSessionMutationCommand {
        &self.proof
    }
    pub const fn target_profile_grant_id(&self) -> ProfileGrantId {
        self.target_profile_grant_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedBrowserSession {
    subject: AuthSubject,
    session: FastiBrowserSession,
}

impl AuthenticatedBrowserSession {
    pub const fn new(subject: AuthSubject, session: FastiBrowserSession) -> Self {
        Self { subject, session }
    }
    pub const fn subject(self) -> AuthSubject {
        self.subject
    }
    pub const fn session(self) -> FastiBrowserSession {
        self.session
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserSessionSummary {
    session: FastiBrowserSession,
    is_current: bool,
}

impl BrowserSessionSummary {
    pub const fn new(session: FastiBrowserSession, is_current: bool) -> Self {
        Self {
            session,
            is_current,
        }
    }
    pub const fn session(self) -> FastiBrowserSession {
        self.session
    }
    pub const fn is_current(self) -> bool {
        self.is_current
    }
}

/// Dormant PR A persistence boundary. Production composition must reject every
/// associated capability until C1 supplies a proven identity exchange.
pub trait BrowserSessionPort: Send + Sync {
    fn create_auth_subject(&self, command: CreateAuthSubjectCommand) -> ApplicationResult<()>;
    fn create_browser_session(
        &self,
        command: CreateBrowserSessionCommand,
    ) -> ApplicationResult<CreatedBrowserSession>;
    fn authenticate_browser_session(
        &self,
        query: BrowserSessionQuery,
    ) -> ApplicationResult<AuthenticatedBrowserSession>;
    fn list_browser_sessions(
        &self,
        query: BrowserSessionQuery,
    ) -> ApplicationResult<Vec<BrowserSessionSummary>>;
    fn revoke_current_browser_session(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<bool>;
    fn revoke_browser_session(
        &self,
        command: TargetBrowserSessionCommand,
    ) -> ApplicationResult<bool>;
    fn revoke_other_browser_sessions(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<u64>;
    fn revoke_all_browser_sessions(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<u64>;
    fn rotate_browser_session(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<CreatedBrowserSession>;
    fn select_browser_session_profile(
        &self,
        command: SelectBrowserSessionProfileCommand,
    ) -> ApplicationResult<CreatedBrowserSession>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> SessionPolicy {
        SessionPolicy::try_new(
            Duration::from_secs(30),
            Duration::from_secs(120),
            Duration::from_secs(240),
            Duration::from_secs(10),
        )
        .expect("valid deterministic policy")
    }

    #[test]
    fn policy_rejects_zero_and_invalid_ordering_without_inventing_defaults() {
        assert!(SessionPolicy::try_new(
            Duration::ZERO,
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(1),
        )
        .is_err());
        assert!(SessionPolicy::try_new(
            Duration::from_millis(1_500),
            Duration::from_secs(2),
            Duration::from_secs(3),
            Duration::from_secs(1),
        )
        .is_err());
        assert_eq!(policy().browser_idle_timeout(), Duration::from_secs(30));
    }

    #[test]
    fn browser_mutation_boundary_rejects_missing_and_mismatched_origin_or_host() {
        let policy =
            BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example")
                .expect("valid boundary policy");
        assert_eq!(
            policy.validate(None, Some("fasti.example")),
            Err(BrowserRequestBoundaryError::MissingOrigin)
        );
        assert_eq!(
            policy.validate(Some("https://fasti.example"), None),
            Err(BrowserRequestBoundaryError::MissingHost)
        );
        assert_eq!(
            policy.validate(Some("https://attacker.example"), Some("fasti.example")),
            Err(BrowserRequestBoundaryError::OriginMismatch)
        );
        assert_eq!(
            policy.validate(Some("https://fasti.example"), Some("attacker.example")),
            Err(BrowserRequestBoundaryError::HostMismatch)
        );
        assert!(policy
            .validate(Some("https://fasti.example"), Some("fasti.example"))
            .is_ok());
    }
}
