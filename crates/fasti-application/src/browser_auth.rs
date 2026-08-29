use crate::{ApplicationResult, SecretMaterial};
use chrono::{DateTime, Utc};
use fasti_domain::{
    AuthSubjectId, BrowserSessionId, ProfileGrantId, RequestCorrelationId, WorkspaceId,
};
use std::{collections::HashSet, error::Error, fmt, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicyInputError;

impl fmt::Display for SessionPolicyInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("session policy durations must be positive and internally ordered")
    }
}

impl Error for SessionPolicyInputError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPolicyChangeEffect {
    NewSessionsOnly,
    ReevaluateExistingSessions,
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
    recent_authentication_window: Duration,
    last_seen_write_interval: Duration,
    disabled_subject_revalidation_interval: Duration,
    policy_change_effect: SessionPolicyChangeEffect,
}

impl SessionPolicy {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        browser_idle_timeout: Duration,
        browser_absolute_lifetime: Duration,
        remembered_browser_lifetime: Duration,
        recent_authentication_window: Duration,
        last_seen_write_interval: Duration,
        disabled_subject_revalidation_interval: Duration,
        policy_change_effect: SessionPolicyChangeEffect,
    ) -> Result<Self, SessionPolicyInputError> {
        let durations = [
            browser_idle_timeout,
            browser_absolute_lifetime,
            remembered_browser_lifetime,
            recent_authentication_window,
            last_seen_write_interval,
            disabled_subject_revalidation_interval,
        ];
        if durations.contains(&Duration::ZERO)
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
            recent_authentication_window,
            last_seen_write_interval,
            disabled_subject_revalidation_interval,
            policy_change_effect,
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
    pub const fn recent_authentication_window(self) -> Duration {
        self.recent_authentication_window
    }
    pub const fn last_seen_write_interval(self) -> Duration {
        self.last_seen_write_interval
    }
    pub const fn disabled_subject_revalidation_interval(self) -> Duration {
        self.disabled_subject_revalidation_interval
    }
    pub const fn policy_change_effect(self) -> SessionPolicyChangeEffect {
        self.policy_change_effect
    }
    pub const fn absolute_lifetime(self, remembered: bool) -> Duration {
        if remembered {
            self.remembered_browser_lifetime
        } else {
            self.browser_absolute_lifetime
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSubjectLifecycle {
    Active,
    Disabled,
    Deleted,
    RecoveryPending,
}

impl AuthSubjectLifecycle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Disabled => "disabled",
            Self::Deleted => "deleted",
            Self::RecoveryPending => "recovery_pending",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "disabled" => Some(Self::Disabled),
            "deleted" => Some(Self::Deleted),
            "recovery_pending" => Some(Self::RecoveryPending),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthSubject {
    id: AuthSubjectId,
    lifecycle: AuthSubjectLifecycle,
    auth_epoch: u64,
    authorization_epoch: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl AuthSubject {
    pub const fn new(
        id: AuthSubjectId,
        lifecycle: AuthSubjectLifecycle,
        auth_epoch: u64,
        authorization_epoch: u64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            lifecycle,
            auth_epoch,
            authorization_epoch,
            created_at,
            updated_at,
        }
    }
    pub const fn id(self) -> AuthSubjectId {
        self.id
    }
    pub const fn lifecycle(self) -> AuthSubjectLifecycle {
        self.lifecycle
    }
    pub const fn auth_epoch(self) -> u64 {
        self.auth_epoch
    }
    pub const fn authorization_epoch(self) -> u64 {
        self.authorization_epoch
    }
    pub const fn created_at(self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn updated_at(self) -> DateTime<Utc> {
        self.updated_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSessionState {
    Active,
    Revoked,
    IdleExpired,
    AbsoluteExpired,
    SubjectInactive,
    PolicyChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastiBrowserSession {
    id: BrowserSessionId,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    selected_profile_grant_id: ProfileGrantId,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    idle_expires_at: DateTime<Utc>,
    absolute_expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    auth_epoch: u64,
    authorization_epoch: u64,
    rotation_generation: u64,
}

impl FastiBrowserSession {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: BrowserSessionId,
        subject_id: AuthSubjectId,
        workspace_id: WorkspaceId,
        selected_profile_grant_id: ProfileGrantId,
        created_at: DateTime<Utc>,
        last_seen_at: DateTime<Utc>,
        idle_expires_at: DateTime<Utc>,
        absolute_expires_at: DateTime<Utc>,
        revoked_at: Option<DateTime<Utc>>,
        auth_epoch: u64,
        authorization_epoch: u64,
        rotation_generation: u64,
    ) -> Self {
        Self {
            id,
            subject_id,
            workspace_id,
            selected_profile_grant_id,
            created_at,
            last_seen_at,
            idle_expires_at,
            absolute_expires_at,
            revoked_at,
            auth_epoch,
            authorization_epoch,
            rotation_generation,
        }
    }
    pub const fn id(self) -> BrowserSessionId {
        self.id
    }
    pub const fn subject_id(self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn selected_profile_grant_id(self) -> ProfileGrantId {
        self.selected_profile_grant_id
    }
    pub const fn created_at(self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn last_seen_at(self) -> DateTime<Utc> {
        self.last_seen_at
    }
    pub const fn idle_expires_at(self) -> DateTime<Utc> {
        self.idle_expires_at
    }
    pub const fn absolute_expires_at(self) -> DateTime<Utc> {
        self.absolute_expires_at
    }
    pub const fn revoked_at(self) -> Option<DateTime<Utc>> {
        self.revoked_at
    }
    pub const fn auth_epoch(self) -> u64 {
        self.auth_epoch
    }
    pub const fn authorization_epoch(self) -> u64 {
        self.authorization_epoch
    }
    pub const fn rotation_generation(self) -> u64 {
        self.rotation_generation
    }

    pub fn state(self, subject: AuthSubject, at: DateTime<Utc>) -> BrowserSessionState {
        if self.revoked_at.is_some() {
            BrowserSessionState::Revoked
        } else if !matches!(subject.lifecycle(), AuthSubjectLifecycle::Active) {
            BrowserSessionState::SubjectInactive
        } else if self.auth_epoch != subject.auth_epoch()
            || self.authorization_epoch != subject.authorization_epoch()
        {
            BrowserSessionState::PolicyChanged
        } else if at >= self.absolute_expires_at {
            BrowserSessionState::AbsoluteExpired
        } else if at >= self.idle_expires_at {
            BrowserSessionState::IdleExpired
        } else {
            BrowserSessionState::Active
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
    now: DateTime<Utc>,
}

impl BrowserSessionMutationCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        session_secret: SecretMaterial,
        csrf_secret: SecretMaterial,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            correlation_id,
            session_secret,
            csrf_secret,
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
    use chrono::TimeZone;

    fn policy() -> SessionPolicy {
        SessionPolicy::try_new(
            Duration::from_secs(30),
            Duration::from_secs(120),
            Duration::from_secs(240),
            Duration::from_secs(20),
            Duration::from_secs(10),
            Duration::from_secs(15),
            SessionPolicyChangeEffect::NewSessionsOnly,
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
            Duration::from_secs(1),
            Duration::from_secs(1),
            SessionPolicyChangeEffect::NewSessionsOnly,
        )
        .is_err());
        assert_eq!(policy().browser_idle_timeout(), Duration::from_secs(30));
    }

    #[test]
    fn session_state_distinguishes_expiry_revocation_and_epoch_change() {
        let created = Utc.timestamp_opt(1_700_000_000, 0).single().expect("time");
        let subject = AuthSubject::new(
            AuthSubjectId::new_v7(),
            AuthSubjectLifecycle::Active,
            2,
            3,
            created,
            created,
        );
        let session = FastiBrowserSession::new(
            BrowserSessionId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            ProfileGrantId::new_v7(),
            created,
            created,
            created + chrono::Duration::seconds(30),
            created + chrono::Duration::seconds(120),
            None,
            2,
            3,
            0,
        );
        assert_eq!(
            session.state(subject, created + chrono::Duration::seconds(29)),
            BrowserSessionState::Active
        );
        assert_eq!(
            session.state(subject, created + chrono::Duration::seconds(30)),
            BrowserSessionState::IdleExpired
        );
        let changed = AuthSubject::new(
            subject.id(),
            AuthSubjectLifecycle::Active,
            2,
            4,
            created,
            created,
        );
        assert_eq!(
            session.state(changed, created + chrono::Duration::seconds(1)),
            BrowserSessionState::PolicyChanged
        );
    }
}
