use crate::{ApplicationResult, BrowserSessionQuery, SessionPolicy};
use chrono::{DateTime, Utc};
use fasti_domain::{
    AuthCeremonyFailure, AuthCeremonyState, AuthSubjectId, AuthSubjectLifecycle,
    AuthenticationMethod, ClientId, FastiBrowserSession, MembershipId, MembershipLifecycle,
    OperationId, ProfileGrantId, ProfileId, RequestCorrelationId, TrailBaseActivationState,
    TrailBaseInstanceId, WorkspaceId, WorkspaceRole,
};

pub const ACCESS_SESSION_INVENTORY_LIMIT: usize = 32;
pub const ACCESS_PROFILE_GRANT_LIMIT: usize = 64;
pub const ACCESS_EVIDENCE_LIMIT: usize = 16;

/// Shared Gate 10 evidence language.
///
/// `Loading` is a client-only transient state. A completed store projection
/// never emits it, but keeping it in the shared vocabulary avoids a parallel
/// UI status model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessEvidenceState {
    Loading,
    Empty,
    Unavailable,
    NeedsAttention,
    FailedSafely,
    Verified,
}

impl AccessEvidenceState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Unavailable => "unavailable",
            Self::NeedsAttention => "needs_attention",
            Self::FailedSafely => "failed_safely",
            Self::Verified => "verified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessSubjectSummary {
    id: AuthSubjectId,
    lifecycle: AuthSubjectLifecycle,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl AccessSubjectSummary {
    pub const fn new(
        id: AuthSubjectId,
        lifecycle: AuthSubjectLifecycle,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            lifecycle,
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

    pub const fn created_at(self) -> DateTime<Utc> {
        self.created_at
    }

    pub const fn updated_at(self) -> DateTime<Utc> {
        self.updated_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessMembershipSummary {
    id: MembershipId,
    workspace_id: WorkspaceId,
    lifecycle: MembershipLifecycle,
    role: WorkspaceRole,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl AccessMembershipSummary {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        id: MembershipId,
        workspace_id: WorkspaceId,
        lifecycle: MembershipLifecycle,
        role: WorkspaceRole,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            workspace_id,
            lifecycle,
            role,
            created_at,
            updated_at,
        }
    }

    pub const fn id(self) -> MembershipId {
        self.id
    }

    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn lifecycle(self) -> MembershipLifecycle {
        self.lifecycle
    }

    pub const fn role(self) -> WorkspaceRole {
        self.role
    }

    pub const fn created_at(self) -> DateTime<Utc> {
        self.created_at
    }

    pub const fn updated_at(self) -> DateTime<Utc> {
        self.updated_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessBrowserSessionSummary {
    session: FastiBrowserSession,
    current: bool,
    idle_timeout_seconds: u64,
    last_seen_write_interval_seconds: u64,
}

impl AccessBrowserSessionSummary {
    pub const fn new(
        session: FastiBrowserSession,
        current: bool,
        idle_timeout_seconds: u64,
        last_seen_write_interval_seconds: u64,
    ) -> Self {
        Self {
            session,
            current,
            idle_timeout_seconds,
            last_seen_write_interval_seconds,
        }
    }

    pub const fn session(self) -> FastiBrowserSession {
        self.session
    }

    pub const fn is_current(self) -> bool {
        self.current
    }

    pub const fn idle_timeout_seconds(self) -> u64 {
        self.idle_timeout_seconds
    }

    pub const fn last_seen_write_interval_seconds(self) -> u64 {
        self.last_seen_write_interval_seconds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessProfileGrantSummary {
    grant_id: ProfileGrantId,
    profile_id: ProfileId,
    owner_client_id: ClientId,
    selected: bool,
}

impl AccessProfileGrantSummary {
    pub const fn new(
        grant_id: ProfileGrantId,
        profile_id: ProfileId,
        owner_client_id: ClientId,
        selected: bool,
    ) -> Self {
        Self {
            grant_id,
            profile_id,
            owner_client_id,
            selected,
        }
    }

    pub const fn grant_id(self) -> ProfileGrantId {
        self.grant_id
    }

    pub const fn profile_id(self) -> ProfileId {
        self.profile_id
    }

    pub const fn owner_client_id(self) -> ClientId {
        self.owner_client_id
    }

    pub const fn is_selected(self) -> bool {
        self.selected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessRecentAuthenticationSummary {
    state: AccessEvidenceState,
    expires_at: Option<DateTime<Utc>>,
}

impl AccessRecentAuthenticationSummary {
    pub const fn new(state: AccessEvidenceState, expires_at: Option<DateTime<Utc>>) -> Self {
        Self { state, expires_at }
    }

    pub const fn state(self) -> AccessEvidenceState {
        self.state
    }

    pub const fn expires_at(self) -> Option<DateTime<Utc>> {
        self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessSessionAuthenticationSummary {
    method: AuthenticationMethod,
    verified_at: DateTime<Utc>,
    activation_generation: u64,
    recent_authentication: AccessRecentAuthenticationSummary,
}

impl AccessSessionAuthenticationSummary {
    pub const fn new(
        method: AuthenticationMethod,
        verified_at: DateTime<Utc>,
        activation_generation: u64,
        recent_authentication: AccessRecentAuthenticationSummary,
    ) -> Self {
        Self {
            method,
            verified_at,
            activation_generation,
            recent_authentication,
        }
    }

    pub const fn method(self) -> AuthenticationMethod {
        self.method
    }

    pub const fn verified_at(self) -> DateTime<Utc> {
        self.verified_at
    }

    pub const fn activation_generation(self) -> u64 {
        self.activation_generation
    }

    pub const fn recent_authentication(self) -> AccessRecentAuthenticationSummary {
        self.recent_authentication
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessTrailBaseActivationSummary {
    instance_id: TrailBaseInstanceId,
    state: TrailBaseActivationState,
    generation: u64,
    session_generation_current: bool,
    updated_at: DateTime<Utc>,
}

impl AccessTrailBaseActivationSummary {
    pub const fn new(
        instance_id: TrailBaseInstanceId,
        state: TrailBaseActivationState,
        generation: u64,
        session_generation_current: bool,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            instance_id,
            state,
            generation,
            session_generation_current,
            updated_at,
        }
    }

    pub const fn instance_id(self) -> TrailBaseInstanceId {
        self.instance_id
    }

    pub const fn state(self) -> TrailBaseActivationState {
        self.state
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn session_generation_is_current(self) -> bool {
        self.session_generation_current
    }

    pub const fn updated_at(self) -> DateTime<Utc> {
        self.updated_at
    }

    pub const fn evidence_state(self) -> AccessEvidenceState {
        match (self.state, self.session_generation_current) {
            (TrailBaseActivationState::Active, true) => AccessEvidenceState::Verified,
            (TrailBaseActivationState::Inactive, _) => AccessEvidenceState::Unavailable,
            (TrailBaseActivationState::Active | TrailBaseActivationState::Blocked(_), _) => {
                AccessEvidenceState::NeedsAttention
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessEvidenceKind {
    CurrentSessionIssued,
    FirstAdministratorBootstrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessCeremonyEvidence {
    kind: AccessEvidenceKind,
    state: AccessEvidenceState,
    operation_id: OperationId,
    correlation_id: RequestCorrelationId,
    ceremony_state: Option<AuthCeremonyState>,
    failure: Option<AuthCeremonyFailure>,
    occurred_at: DateTime<Utc>,
}

impl AccessCeremonyEvidence {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        kind: AccessEvidenceKind,
        state: AccessEvidenceState,
        operation_id: OperationId,
        correlation_id: RequestCorrelationId,
        ceremony_state: Option<AuthCeremonyState>,
        failure: Option<AuthCeremonyFailure>,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        Self {
            kind,
            state,
            operation_id,
            correlation_id,
            ceremony_state,
            failure,
            occurred_at,
        }
    }

    pub const fn kind(self) -> AccessEvidenceKind {
        self.kind
    }

    pub const fn state(self) -> AccessEvidenceState {
        self.state
    }

    pub const fn operation_id(self) -> OperationId {
        self.operation_id
    }

    pub const fn correlation_id(self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn ceremony_state(self) -> Option<AuthCeremonyState> {
        self.ceremony_state
    }

    pub const fn failure(self) -> Option<AuthCeremonyFailure> {
        self.failure
    }

    pub const fn occurred_at(self) -> DateTime<Utc> {
        self.occurred_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessFirstRunStepKey {
    AccountConfirmed,
    StrongSignIn,
    Recovery,
    DevicesAndClients,
    ExternalIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessFirstRunStep {
    key: AccessFirstRunStepKey,
    state: AccessEvidenceState,
}

impl AccessFirstRunStep {
    pub const fn new(key: AccessFirstRunStepKey, state: AccessEvidenceState) -> Self {
        Self { key, state }
    }

    pub const fn key(self) -> AccessFirstRunStepKey {
        self.key
    }

    pub const fn state(self) -> AccessEvidenceState {
        self.state
    }
}

/// C1 confirms only the account/session step. Later Access packages replace
/// the explicit unavailable states with their own proven evidence.
pub const fn c1_first_run_steps() -> [AccessFirstRunStep; 5] {
    [
        AccessFirstRunStep::new(
            AccessFirstRunStepKey::AccountConfirmed,
            AccessEvidenceState::Verified,
        ),
        AccessFirstRunStep::new(
            AccessFirstRunStepKey::StrongSignIn,
            AccessEvidenceState::Unavailable,
        ),
        AccessFirstRunStep::new(
            AccessFirstRunStepKey::Recovery,
            AccessEvidenceState::Unavailable,
        ),
        AccessFirstRunStep::new(
            AccessFirstRunStepKey::DevicesAndClients,
            AccessEvidenceState::Unavailable,
        ),
        AccessFirstRunStep::new(
            AccessFirstRunStepKey::ExternalIdentity,
            AccessEvidenceState::Unavailable,
        ),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessProjection {
    generated_at: DateTime<Utc>,
    subject: AccessSubjectSummary,
    membership: AccessMembershipSummary,
    current_session: AccessBrowserSessionSummary,
    sessions: Vec<AccessBrowserSessionSummary>,
    sessions_truncated: bool,
    profile_grants: Vec<AccessProfileGrantSummary>,
    profile_grants_truncated: bool,
    session_policy: SessionPolicy,
    authentication: AccessSessionAuthenticationSummary,
    trailbase: AccessTrailBaseActivationSummary,
    first_run_steps: [AccessFirstRunStep; 5],
    evidence: Vec<AccessCeremonyEvidence>,
    evidence_truncated: bool,
}

impl AccessProjection {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        generated_at: DateTime<Utc>,
        subject: AccessSubjectSummary,
        membership: AccessMembershipSummary,
        current_session: AccessBrowserSessionSummary,
        sessions: Vec<AccessBrowserSessionSummary>,
        sessions_truncated: bool,
        profile_grants: Vec<AccessProfileGrantSummary>,
        profile_grants_truncated: bool,
        session_policy: SessionPolicy,
        authentication: AccessSessionAuthenticationSummary,
        trailbase: AccessTrailBaseActivationSummary,
        first_run_steps: [AccessFirstRunStep; 5],
        evidence: Vec<AccessCeremonyEvidence>,
        evidence_truncated: bool,
    ) -> Self {
        assert!(sessions.len() <= ACCESS_SESSION_INVENTORY_LIMIT);
        assert!(profile_grants.len() <= ACCESS_PROFILE_GRANT_LIMIT);
        assert!(evidence.len() <= ACCESS_EVIDENCE_LIMIT);
        Self {
            generated_at,
            subject,
            membership,
            current_session,
            sessions,
            sessions_truncated,
            profile_grants,
            profile_grants_truncated,
            session_policy,
            authentication,
            trailbase,
            first_run_steps,
            evidence,
            evidence_truncated,
        }
    }

    pub const fn generated_at(&self) -> DateTime<Utc> {
        self.generated_at
    }

    pub const fn subject(&self) -> AccessSubjectSummary {
        self.subject
    }

    pub const fn membership(&self) -> AccessMembershipSummary {
        self.membership
    }

    pub const fn current_session(&self) -> AccessBrowserSessionSummary {
        self.current_session
    }

    pub fn sessions(&self) -> &[AccessBrowserSessionSummary] {
        &self.sessions
    }

    pub const fn sessions_truncated(&self) -> bool {
        self.sessions_truncated
    }

    pub fn profile_grants(&self) -> &[AccessProfileGrantSummary] {
        &self.profile_grants
    }

    pub const fn profile_grants_truncated(&self) -> bool {
        self.profile_grants_truncated
    }

    pub const fn session_policy(&self) -> SessionPolicy {
        self.session_policy
    }

    pub const fn authentication(&self) -> AccessSessionAuthenticationSummary {
        self.authentication
    }

    pub const fn trailbase(&self) -> AccessTrailBaseActivationSummary {
        self.trailbase
    }

    pub fn first_run_steps(&self) -> &[AccessFirstRunStep] {
        &self.first_run_steps
    }

    pub fn evidence(&self) -> &[AccessCeremonyEvidence] {
        &self.evidence
    }

    pub const fn evidence_truncated(&self) -> bool {
        self.evidence_truncated
    }

    pub fn evidence_state(&self) -> AccessEvidenceState {
        self.evidence
            .first()
            .map_or(AccessEvidenceState::Empty, |evidence| evidence.state())
    }
}

/// One authenticated, bounded read of the complete Gate 10 A+C state.
pub trait AccessProjectionPort: Send + Sync {
    fn read_access_projection(
        &self,
        query: BrowserSessionQuery,
    ) -> ApplicationResult<AccessProjection>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c1_does_not_claim_later_access_packages_are_verified() {
        let steps = c1_first_run_steps();
        assert_eq!(steps[0].state(), AccessEvidenceState::Verified);
        assert!(steps[1..]
            .iter()
            .all(|step| step.state() == AccessEvidenceState::Unavailable));
    }
}
