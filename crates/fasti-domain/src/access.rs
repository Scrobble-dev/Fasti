use crate::{
    AuthSubjectId, BrowserSessionId, MembershipId, OperationId, ProfileGrantId,
    RequestCorrelationId, Sha256Digest, TrailBaseInstanceId, WorkspaceId,
};
use chrono::{DateTime, TimeDelta, Utc};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AccessInvariantError {
    #[error("Access timestamps are not monotonic")]
    InvalidTimestampOrder,
    #[error("a deleted authentication subject is terminal")]
    DeletedSubjectIsTerminal,
    #[error("the Access epoch cannot advance")]
    EpochOverflow,
    #[error("the TrailBase activation generation cannot advance")]
    ActivationGenerationOverflow,
    #[error("the TrailBase activation state is invalid")]
    InvalidActivationState,
    #[error("the TrailBase activation generation is invalid")]
    InvalidActivationGeneration,
    #[error("the TrailBase installation is blocked")]
    TrailBaseInstallationBlocked,
    #[error("the membership transition is not allowed")]
    InvalidMembershipTransition,
    #[error("the membership role is not allowed for its lifecycle")]
    InvalidMembershipRole,
    #[error("a removed membership is terminal")]
    RemovedMembershipIsTerminal,
    #[error("the final viable administrator cannot be removed")]
    FinalAdministratorRequired,
    #[error("the membership does not belong to this authentication subject")]
    MembershipSubjectMismatch,
    #[error("the authentication ceremony transition is not allowed")]
    InvalidCeremonyTransition,
    #[error("the authentication ceremony purpose and return target do not match")]
    InvalidCeremonyPurposeTarget,
    #[error("the authentication callback path is invalid")]
    InvalidCallbackPath,
    #[error("the authentication ceremony browser binding does not match")]
    CeremonyBindingMismatch,
    #[error("the authentication ceremony TrailBase installation does not match")]
    CeremonyInstallationMismatch,
    #[error("the authentication ceremony activation generation does not match")]
    CeremonyGenerationMismatch,
    #[error("the authentication ceremony callback path does not match")]
    CeremonyCallbackMismatch,
    #[error("the authentication ceremony has expired")]
    CeremonyExpired,
    #[error("the authentication proof expiry must follow verification")]
    InvalidAuthenticationProofExpiry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrailBaseSubject([u8; 16]);

impl TrailBaseSubject {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailBaseActivationBlocker {
    ReleaseMismatch,
    PhysicalRootIdentityMismatch,
    DeclaredRestore,
}

impl TrailBaseActivationBlocker {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReleaseMismatch => "release_mismatch",
            Self::PhysicalRootIdentityMismatch => "physical_root_identity_mismatch",
            Self::DeclaredRestore => "declared_restore",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "release_mismatch" => Some(Self::ReleaseMismatch),
            "physical_root_identity_mismatch" => Some(Self::PhysicalRootIdentityMismatch),
            "declared_restore" => Some(Self::DeclaredRestore),
            _ => None,
        }
    }

    const fn is_recoverable_in_c1(self) -> bool {
        matches!(self, Self::ReleaseMismatch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailBaseActivationState {
    Inactive,
    Active,
    Blocked(TrailBaseActivationBlocker),
}

impl TrailBaseActivationState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inactive => "inactive",
            Self::Active => "active",
            Self::Blocked(_) => "blocked",
        }
    }

    pub fn from_storage(state: &str, blocker: Option<&str>) -> Option<Self> {
        match (state, blocker) {
            ("inactive", None) => Some(Self::Inactive),
            ("active", None) => Some(Self::Active),
            ("blocked", Some(value)) => {
                TrailBaseActivationBlocker::from_storage(value).map(Self::Blocked)
            }
            _ => None,
        }
    }

    pub const fn blocker(self) -> Option<TrailBaseActivationBlocker> {
        match self {
            Self::Blocked(blocker) => Some(blocker),
            Self::Inactive | Self::Active => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrailBaseInstallation {
    id: TrailBaseInstanceId,
    physical_root_identity: Sha256Digest,
    activation_state: TrailBaseActivationState,
    activation_generation: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TrailBaseInstallation {
    pub fn new(
        id: TrailBaseInstanceId,
        physical_root_identity: Sha256Digest,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            physical_root_identity,
            activation_state: TrailBaseActivationState::Inactive,
            activation_generation: 0,
            created_at,
            updated_at: created_at,
        }
    }

    pub fn try_from_persisted(
        id: TrailBaseInstanceId,
        physical_root_identity: Sha256Digest,
        activation_state: TrailBaseActivationState,
        activation_generation: u64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, AccessInvariantError> {
        if updated_at < created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if matches!(activation_state, TrailBaseActivationState::Inactive)
            && activation_generation != 0
            || matches!(activation_state, TrailBaseActivationState::Active)
                && activation_generation == 0
        {
            return Err(AccessInvariantError::InvalidActivationState);
        }
        Ok(Self {
            id,
            physical_root_identity,
            activation_state,
            activation_generation,
            created_at,
            updated_at,
        })
    }

    pub const fn id(&self) -> TrailBaseInstanceId {
        self.id
    }
    pub const fn physical_root_identity(&self) -> &Sha256Digest {
        &self.physical_root_identity
    }
    pub const fn activation_state(&self) -> TrailBaseActivationState {
        self.activation_state
    }
    pub const fn activation_generation(&self) -> u64 {
        self.activation_generation
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn verify(
        &mut self,
        observed_root_identity: &Sha256Digest,
        release_matches: bool,
        at: DateTime<Utc>,
    ) -> Result<bool, AccessInvariantError> {
        self.validate_time(at)?;
        if matches!(
            self.activation_state,
            TrailBaseActivationState::Blocked(blocker) if !blocker.is_recoverable_in_c1()
        ) {
            return Err(AccessInvariantError::TrailBaseInstallationBlocked);
        }
        if observed_root_identity != &self.physical_root_identity {
            return self.block(TrailBaseActivationBlocker::PhysicalRootIdentityMismatch, at);
        }
        if !release_matches {
            return self.block(TrailBaseActivationBlocker::ReleaseMismatch, at);
        }

        let next_generation = match self.activation_state {
            TrailBaseActivationState::Inactive => self
                .activation_generation
                .checked_add(1)
                .ok_or(AccessInvariantError::ActivationGenerationOverflow)?,
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::ReleaseMismatch)
                if self.activation_generation == 0 =>
            {
                self.activation_generation
                    .checked_add(1)
                    .ok_or(AccessInvariantError::ActivationGenerationOverflow)?
            }
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::ReleaseMismatch) => {
                self.activation_generation
            }
            TrailBaseActivationState::Active => return Ok(false),
            TrailBaseActivationState::Blocked(_) => {
                return Err(AccessInvariantError::TrailBaseInstallationBlocked);
            }
        };
        self.activation_state = TrailBaseActivationState::Active;
        self.activation_generation = next_generation;
        self.updated_at = at;
        Ok(true)
    }

    pub fn declare_restore(&mut self, at: DateTime<Utc>) -> Result<bool, AccessInvariantError> {
        self.block(TrailBaseActivationBlocker::DeclaredRestore, at)
    }

    fn block(
        &mut self,
        blocker: TrailBaseActivationBlocker,
        at: DateTime<Utc>,
    ) -> Result<bool, AccessInvariantError> {
        self.validate_time(at)?;
        match self.activation_state {
            TrailBaseActivationState::Blocked(current) if current == blocker => return Ok(false),
            TrailBaseActivationState::Blocked(current) if !current.is_recoverable_in_c1() => {
                return Ok(false);
            }
            TrailBaseActivationState::Active => {
                self.activation_generation = self
                    .activation_generation
                    .checked_add(1)
                    .ok_or(AccessInvariantError::ActivationGenerationOverflow)?;
            }
            TrailBaseActivationState::Inactive | TrailBaseActivationState::Blocked(_) => {}
        }
        self.activation_state = TrailBaseActivationState::Blocked(blocker);
        self.updated_at = at;
        Ok(true)
    }

    fn validate_time(&self, at: DateTime<Utc>) -> Result<(), AccessInvariantError> {
        if at < self.updated_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailBaseExternalAnchor {
    trailbase_instance_id: TrailBaseInstanceId,
    trailbase_subject: TrailBaseSubject,
    auth_subject_id: AuthSubjectId,
    linked_at: DateTime<Utc>,
}

impl TrailBaseExternalAnchor {
    pub const fn new(
        trailbase_instance_id: TrailBaseInstanceId,
        trailbase_subject: TrailBaseSubject,
        auth_subject_id: AuthSubjectId,
        linked_at: DateTime<Utc>,
    ) -> Self {
        Self {
            trailbase_instance_id,
            trailbase_subject,
            auth_subject_id,
            linked_at,
        }
    }

    pub const fn trailbase_instance_id(&self) -> TrailBaseInstanceId {
        self.trailbase_instance_id
    }
    pub const fn trailbase_subject(&self) -> TrailBaseSubject {
        self.trailbase_subject
    }
    pub const fn auth_subject_id(&self) -> AuthSubjectId {
        self.auth_subject_id
    }
    pub const fn linked_at(&self) -> DateTime<Utc> {
        self.linked_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdministratorContinuity {
    Preserved,
    FinalAdministratorWouldBeRemoved,
}

impl AdministratorContinuity {
    pub const fn for_membership_change(
        currently_viable: bool,
        remains_viable: bool,
        viable_administrator_count: u64,
    ) -> Self {
        if currently_viable && !remains_viable && viable_administrator_count == 1 {
            Self::FinalAdministratorWouldBeRemoved
        } else {
            Self::Preserved
        }
    }

    pub const fn for_subject_deactivation(sole_administrator_workspace_count: u64) -> Self {
        if sole_administrator_workspace_count > 0 {
            Self::FinalAdministratorWouldBeRemoved
        } else {
            Self::Preserved
        }
    }

    const fn ensure(self) -> Result<(), AccessInvariantError> {
        match self {
            Self::Preserved => Ok(()),
            Self::FinalAdministratorWouldBeRemoved => {
                Err(AccessInvariantError::FinalAdministratorRequired)
            }
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
    pub fn try_new(
        id: AuthSubjectId,
        lifecycle: AuthSubjectLifecycle,
        auth_epoch: u64,
        authorization_epoch: u64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, AccessInvariantError> {
        if updated_at < created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        Ok(Self {
            id,
            lifecycle,
            auth_epoch,
            authorization_epoch,
            created_at,
            updated_at,
        })
    }

    pub const fn id(&self) -> AuthSubjectId {
        self.id
    }
    pub const fn lifecycle(&self) -> AuthSubjectLifecycle {
        self.lifecycle
    }
    pub const fn auth_epoch(&self) -> u64 {
        self.auth_epoch
    }
    pub const fn authorization_epoch(&self) -> u64 {
        self.authorization_epoch
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn transition_lifecycle(
        &mut self,
        lifecycle: AuthSubjectLifecycle,
        administrator_continuity: AdministratorContinuity,
        at: DateTime<Utc>,
    ) -> Result<bool, AccessInvariantError> {
        if at < self.updated_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if lifecycle == self.lifecycle {
            return Ok(false);
        }
        if matches!(self.lifecycle, AuthSubjectLifecycle::Deleted) {
            return Err(AccessInvariantError::DeletedSubjectIsTerminal);
        }
        if matches!(self.lifecycle, AuthSubjectLifecycle::Active)
            && !matches!(lifecycle, AuthSubjectLifecycle::Active)
        {
            administrator_continuity.ensure()?;
        }
        self.advance_authentication_epoch(at)?;
        self.lifecycle = lifecycle;
        Ok(true)
    }

    pub fn advance_authentication_epoch(
        &mut self,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        if at < self.updated_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        self.auth_epoch = self
            .auth_epoch
            .checked_add(1)
            .ok_or(AccessInvariantError::EpochOverflow)?;
        self.updated_at = at;
        Ok(())
    }

    pub fn advance_authorization_epoch(
        &mut self,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        if at < self.updated_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        self.authorization_epoch = self
            .authorization_epoch
            .checked_add(1)
            .ok_or(AccessInvariantError::EpochOverflow)?;
        self.updated_at = at;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRole {
    Member,
    Administrator,
}

impl WorkspaceRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Member => "member",
            Self::Administrator => "administrator",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "member" => Some(Self::Member),
            "administrator" => Some(Self::Administrator),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipLifecycle {
    Invited,
    PendingApproval,
    Active,
    Suspended,
    Removed,
}

impl MembershipLifecycle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invited => "invited",
            Self::PendingApproval => "pending_approval",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Removed => "removed",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "invited" => Some(Self::Invited),
            "pending_approval" => Some(Self::PendingApproval),
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            "removed" => Some(Self::Removed),
            _ => None,
        }
    }

    pub const fn grants_access(self) -> bool {
        matches!(self, Self::Active)
    }

    const fn allows_role(self, role: WorkspaceRole) -> bool {
        !matches!(self, Self::Invited | Self::PendingApproval)
            || matches!(role, WorkspaceRole::Member)
    }

    const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Invited, Self::Active | Self::Removed)
                | (Self::PendingApproval, Self::Active | Self::Removed)
                | (Self::Active, Self::Suspended | Self::Removed)
                | (Self::Suspended, Self::Active | Self::Removed)
        )
    }
}

/// Workspace authorization is independent from human identity and profile
/// grants. Persistence must save this value and the subject epoch in one
/// transaction after a successful transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceMembership {
    id: MembershipId,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    lifecycle: MembershipLifecycle,
    role: WorkspaceRole,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl WorkspaceMembership {
    pub fn try_new(
        id: MembershipId,
        subject_id: AuthSubjectId,
        workspace_id: WorkspaceId,
        lifecycle: MembershipLifecycle,
        role: WorkspaceRole,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, AccessInvariantError> {
        if updated_at < created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if !lifecycle.allows_role(role) {
            return Err(AccessInvariantError::InvalidMembershipRole);
        }
        Ok(Self {
            id,
            subject_id,
            workspace_id,
            lifecycle,
            role,
            created_at,
            updated_at,
        })
    }

    pub const fn id(&self) -> MembershipId {
        self.id
    }
    pub const fn subject_id(&self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn lifecycle(&self) -> MembershipLifecycle {
        self.lifecycle
    }
    pub const fn role(&self) -> WorkspaceRole {
        self.role
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
    pub fn is_authorization_viable_administrator(&self, subject: &AuthSubject) -> bool {
        subject.id() == self.subject_id
            && matches!(subject.lifecycle(), AuthSubjectLifecycle::Active)
            && self.lifecycle.grants_access()
            && matches!(self.role, WorkspaceRole::Administrator)
    }

    pub fn transition_lifecycle(
        &mut self,
        subject: &mut AuthSubject,
        lifecycle: MembershipLifecycle,
        viable_administrator_count: u64,
        at: DateTime<Utc>,
    ) -> Result<bool, AccessInvariantError> {
        self.validate_subject_and_time(subject, at)?;
        if lifecycle == self.lifecycle {
            return Ok(false);
        }
        if matches!(self.lifecycle, MembershipLifecycle::Removed) {
            return Err(AccessInvariantError::RemovedMembershipIsTerminal);
        }
        if !self.lifecycle.can_transition_to(lifecycle) {
            return Err(AccessInvariantError::InvalidMembershipTransition);
        }
        self.administrator_continuity(subject, lifecycle, self.role, viable_administrator_count)
            .ensure()?;
        subject.advance_authorization_epoch(at)?;
        self.lifecycle = lifecycle;
        self.updated_at = at;
        Ok(true)
    }

    pub fn change_role(
        &mut self,
        subject: &mut AuthSubject,
        role: WorkspaceRole,
        viable_administrator_count: u64,
        at: DateTime<Utc>,
    ) -> Result<bool, AccessInvariantError> {
        self.validate_subject_and_time(subject, at)?;
        if role == self.role {
            return Ok(false);
        }
        if matches!(self.lifecycle, MembershipLifecycle::Removed) {
            return Err(AccessInvariantError::RemovedMembershipIsTerminal);
        }
        if !self.lifecycle.allows_role(role) {
            return Err(AccessInvariantError::InvalidMembershipRole);
        }
        self.administrator_continuity(subject, self.lifecycle, role, viable_administrator_count)
            .ensure()?;
        subject.advance_authorization_epoch(at)?;
        self.role = role;
        self.updated_at = at;
        Ok(true)
    }

    fn validate_subject_and_time(
        &self,
        subject: &AuthSubject,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        if subject.id() != self.subject_id {
            return Err(AccessInvariantError::MembershipSubjectMismatch);
        }
        if at < self.updated_at || at < subject.updated_at() {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        Ok(())
    }

    fn administrator_continuity(
        &self,
        subject: &AuthSubject,
        lifecycle: MembershipLifecycle,
        role: WorkspaceRole,
        viable_administrator_count: u64,
    ) -> AdministratorContinuity {
        let remains_viable = matches!(subject.lifecycle(), AuthSubjectLifecycle::Active)
            && lifecycle.grants_access()
            && matches!(role, WorkspaceRole::Administrator);
        AdministratorContinuity::for_membership_change(
            self.is_authorization_viable_administrator(subject),
            remains_viable,
            viable_administrator_count,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthCeremonyState {
    Pending,
    Claimed,
    Completed,
    Cancelled,
    Failed,
    CleanupUncertain,
    Expired,
}

impl AuthCeremonyState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Claimed => "claimed",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::CleanupUncertain => "cleanup_uncertain",
            Self::Expired => "expired",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "claimed" => Some(Self::Claimed),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            "cleanup_uncertain" => Some(Self::CleanupUncertain),
            "expired" => Some(Self::Expired),
            _ => None,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::Cancelled
                | Self::Failed
                | Self::CleanupUncertain
                | Self::Expired
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthCeremonyPurpose {
    SignIn,
    RecentAuthentication,
    FirstAdministratorBootstrap,
}

impl AuthCeremonyPurpose {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SignIn => "sign_in",
            Self::RecentAuthentication => "recent_authentication",
            Self::FirstAdministratorBootstrap => "first_administrator_bootstrap",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "sign_in" => Some(Self::SignIn),
            "recent_authentication" => Some(Self::RecentAuthentication),
            "first_administrator_bootstrap" => Some(Self::FirstAdministratorBootstrap),
            _ => None,
        }
    }

    pub const fn return_target(self) -> AuthReturnTarget {
        match self {
            Self::SignIn => AuthReturnTarget::ApplicationHome,
            Self::RecentAuthentication => AuthReturnTarget::AccountSecurity,
            Self::FirstAdministratorBootstrap => AuthReturnTarget::FirstRun,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthReturnTarget {
    ApplicationHome,
    AccountSecurity,
    FirstRun,
}

impl AuthReturnTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApplicationHome => "application_home",
            Self::AccountSecurity => "account_security",
            Self::FirstRun => "first_run",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "application_home" => Some(Self::ApplicationHome),
            "account_security" => Some(Self::AccountSecurity),
            "first_run" => Some(Self::FirstRun),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthCeremonyProtocol {
    TrailBaseAuthorizationCodePkce,
}

impl AuthCeremonyProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TrailBaseAuthorizationCodePkce => "trailbase_authorization_code_pkce",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "trailbase_authorization_code_pkce" => Some(Self::TrailBaseAuthorizationCodePkce),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthCeremonyFailure {
    VerifierLostOnRestart,
    ExchangeOutcomeUncertain,
    ExchangeFailed,
    StatusRejected,
    LogoutUncertain,
    LocalAuthorizationDenied,
}

impl AuthCeremonyFailure {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VerifierLostOnRestart => "verifier_lost_on_restart",
            Self::ExchangeOutcomeUncertain => "exchange_outcome_uncertain",
            Self::ExchangeFailed => "exchange_failed",
            Self::StatusRejected => "status_rejected",
            Self::LogoutUncertain => "logout_uncertain",
            Self::LocalAuthorizationDenied => "local_authorization_denied",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "verifier_lost_on_restart" => Some(Self::VerifierLostOnRestart),
            "exchange_outcome_uncertain" => Some(Self::ExchangeOutcomeUncertain),
            "exchange_failed" => Some(Self::ExchangeFailed),
            "status_rejected" => Some(Self::StatusRejected),
            "logout_uncertain" => Some(Self::LogoutUncertain),
            "local_authorization_denied" => Some(Self::LocalAuthorizationDenied),
            _ => None,
        }
    }
}

const MAX_AUTH_CALLBACK_PATH_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuthCallbackPath(String);

impl AuthCallbackPath {
    pub fn parse(value: impl Into<String>) -> Result<Self, AccessInvariantError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_AUTH_CALLBACK_PATH_BYTES
            || !value.starts_with('/')
            || value.starts_with("//")
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.')
            })
        {
            return Err(AccessInvariantError::InvalidCallbackPath);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCeremony {
    id: OperationId,
    purpose: AuthCeremonyPurpose,
    protocol: AuthCeremonyProtocol,
    trailbase_instance_id: TrailBaseInstanceId,
    activation_generation: u64,
    browser_binding_digest: Sha256Digest,
    callback_path: AuthCallbackPath,
    return_target: AuthReturnTarget,
    correlation_id: RequestCorrelationId,
    state: AuthCeremonyState,
    failure: Option<AuthCeremonyFailure>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    claimed_at: Option<DateTime<Utc>>,
    terminal_at: Option<DateTime<Utc>>,
}

impl AuthCeremony {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        id: OperationId,
        purpose: AuthCeremonyPurpose,
        protocol: AuthCeremonyProtocol,
        trailbase_instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        browser_binding_digest: Sha256Digest,
        callback_path: AuthCallbackPath,
        return_target: AuthReturnTarget,
        correlation_id: RequestCorrelationId,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, AccessInvariantError> {
        if expires_at <= created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if activation_generation == 0 {
            return Err(AccessInvariantError::InvalidActivationGeneration);
        }
        if purpose.return_target() != return_target {
            return Err(AccessInvariantError::InvalidCeremonyPurposeTarget);
        }
        Ok(Self {
            id,
            purpose,
            protocol,
            trailbase_instance_id,
            activation_generation,
            browser_binding_digest,
            callback_path,
            return_target,
            correlation_id,
            state: AuthCeremonyState::Pending,
            failure: None,
            created_at,
            expires_at,
            claimed_at: None,
            terminal_at: None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_from_persisted(
        id: OperationId,
        purpose: AuthCeremonyPurpose,
        protocol: AuthCeremonyProtocol,
        trailbase_instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        browser_binding_digest: Sha256Digest,
        callback_path: AuthCallbackPath,
        return_target: AuthReturnTarget,
        correlation_id: RequestCorrelationId,
        state: AuthCeremonyState,
        failure: Option<AuthCeremonyFailure>,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        claimed_at: Option<DateTime<Utc>>,
        terminal_at: Option<DateTime<Utc>>,
    ) -> Result<Self, AccessInvariantError> {
        let ceremony = Self {
            id,
            purpose,
            protocol,
            trailbase_instance_id,
            activation_generation,
            browser_binding_digest,
            callback_path,
            return_target,
            correlation_id,
            state,
            failure,
            created_at,
            expires_at,
            claimed_at,
            terminal_at,
        };
        ceremony.validate_persisted_state()?;
        Ok(ceremony)
    }

    pub const fn id(&self) -> OperationId {
        self.id
    }
    pub const fn purpose(&self) -> AuthCeremonyPurpose {
        self.purpose
    }
    pub const fn protocol(&self) -> AuthCeremonyProtocol {
        self.protocol
    }
    pub const fn trailbase_instance_id(&self) -> TrailBaseInstanceId {
        self.trailbase_instance_id
    }
    pub const fn activation_generation(&self) -> u64 {
        self.activation_generation
    }
    pub const fn browser_binding_digest(&self) -> &Sha256Digest {
        &self.browser_binding_digest
    }
    pub const fn callback_path(&self) -> &AuthCallbackPath {
        &self.callback_path
    }
    pub const fn return_target(&self) -> AuthReturnTarget {
        self.return_target
    }
    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub const fn state(&self) -> AuthCeremonyState {
        self.state
    }
    pub const fn failure(&self) -> Option<AuthCeremonyFailure> {
        self.failure
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
    pub const fn claimed_at(&self) -> Option<DateTime<Utc>> {
        self.claimed_at
    }
    pub const fn terminal_at(&self) -> Option<DateTime<Utc>> {
        self.terminal_at
    }

    pub fn claim(
        &mut self,
        browser_binding_digest: &Sha256Digest,
        trailbase_instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        callback_path: &AuthCallbackPath,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        if !matches!(self.state, AuthCeremonyState::Pending) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        if at < self.created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if at >= self.expires_at {
            self.state = AuthCeremonyState::Expired;
            self.terminal_at = Some(at);
            return Err(AccessInvariantError::CeremonyExpired);
        }
        if browser_binding_digest != &self.browser_binding_digest {
            return Err(AccessInvariantError::CeremonyBindingMismatch);
        }
        if trailbase_instance_id != self.trailbase_instance_id {
            return Err(AccessInvariantError::CeremonyInstallationMismatch);
        }
        if activation_generation != self.activation_generation {
            return Err(AccessInvariantError::CeremonyGenerationMismatch);
        }
        if callback_path != &self.callback_path {
            return Err(AccessInvariantError::CeremonyCallbackMismatch);
        }
        self.state = AuthCeremonyState::Claimed;
        self.claimed_at = Some(at);
        Ok(())
    }

    pub fn complete(&mut self, at: DateTime<Utc>) -> Result<(), AccessInvariantError> {
        self.validate_claimed_transition(at)?;
        self.state = AuthCeremonyState::Completed;
        self.terminal_at = Some(at);
        Ok(())
    }

    pub fn cancel(&mut self, at: DateTime<Utc>) -> Result<(), AccessInvariantError> {
        if !matches!(self.state, AuthCeremonyState::Pending) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        if at < self.created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if at >= self.expires_at {
            self.state = AuthCeremonyState::Expired;
            self.terminal_at = Some(at);
            return Err(AccessInvariantError::CeremonyExpired);
        }
        self.state = AuthCeremonyState::Cancelled;
        self.terminal_at = Some(at);
        Ok(())
    }

    pub fn fail(
        &mut self,
        failure: AuthCeremonyFailure,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        if self.state.is_terminal() || !Self::failure_matches_state(self.state, failure) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        self.validate_transition_time(at)?;
        self.state = AuthCeremonyState::Failed;
        self.failure = Some(failure);
        self.terminal_at = Some(at);
        Ok(())
    }

    pub fn mark_cleanup_uncertain(
        &mut self,
        failure: AuthCeremonyFailure,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        self.validate_claimed_transition(at)?;
        if !matches!(
            failure,
            AuthCeremonyFailure::ExchangeOutcomeUncertain | AuthCeremonyFailure::LogoutUncertain
        ) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        self.state = AuthCeremonyState::CleanupUncertain;
        self.failure = Some(failure);
        self.terminal_at = Some(at);
        Ok(())
    }

    pub fn recover_after_restart(
        &mut self,
        at: DateTime<Utc>,
    ) -> Result<bool, AccessInvariantError> {
        match self.state {
            AuthCeremonyState::Pending => {
                self.fail(AuthCeremonyFailure::VerifierLostOnRestart, at)?;
                Ok(true)
            }
            AuthCeremonyState::Claimed => {
                self.mark_cleanup_uncertain(AuthCeremonyFailure::ExchangeOutcomeUncertain, at)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn validate_claimed_transition(&self, at: DateTime<Utc>) -> Result<(), AccessInvariantError> {
        if !matches!(self.state, AuthCeremonyState::Claimed) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        self.validate_transition_time(at)
    }

    fn validate_transition_time(&self, at: DateTime<Utc>) -> Result<(), AccessInvariantError> {
        if at < self.claimed_at.unwrap_or(self.created_at) {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        Ok(())
    }

    fn validate_persisted_state(&self) -> Result<(), AccessInvariantError> {
        if self.expires_at <= self.created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if self.activation_generation == 0 {
            return Err(AccessInvariantError::InvalidActivationGeneration);
        }
        if self.purpose.return_target() != self.return_target {
            return Err(AccessInvariantError::InvalidCeremonyPurposeTarget);
        }
        if self
            .claimed_at
            .is_some_and(|claimed_at| claimed_at < self.created_at || claimed_at >= self.expires_at)
            || self
                .terminal_at
                .is_some_and(|terminal_at| terminal_at < self.claimed_at.unwrap_or(self.created_at))
        {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }

        let valid = match self.state {
            AuthCeremonyState::Pending => {
                self.failure.is_none() && self.claimed_at.is_none() && self.terminal_at.is_none()
            }
            AuthCeremonyState::Claimed => {
                self.failure.is_none() && self.claimed_at.is_some() && self.terminal_at.is_none()
            }
            AuthCeremonyState::Completed => {
                self.failure.is_none() && self.claimed_at.is_some() && self.terminal_at.is_some()
            }
            AuthCeremonyState::Cancelled => {
                self.failure.is_none()
                    && self.claimed_at.is_none()
                    && self
                        .terminal_at
                        .is_some_and(|terminal_at| terminal_at < self.expires_at)
            }
            AuthCeremonyState::Failed => {
                self.failure.is_some_and(|failure| {
                    Self::failure_matches_state(
                        if self.claimed_at.is_some() {
                            AuthCeremonyState::Claimed
                        } else {
                            AuthCeremonyState::Pending
                        },
                        failure,
                    )
                }) && self.terminal_at.is_some()
            }
            AuthCeremonyState::CleanupUncertain => {
                self.claimed_at.is_some()
                    && self.terminal_at.is_some()
                    && matches!(
                        self.failure,
                        Some(
                            AuthCeremonyFailure::ExchangeOutcomeUncertain
                                | AuthCeremonyFailure::LogoutUncertain
                        )
                    )
            }
            AuthCeremonyState::Expired => {
                self.failure.is_none()
                    && self.claimed_at.is_none()
                    && self
                        .terminal_at
                        .is_some_and(|terminal_at| terminal_at >= self.expires_at)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(AccessInvariantError::InvalidCeremonyTransition)
        }
    }

    const fn failure_matches_state(state: AuthCeremonyState, failure: AuthCeremonyFailure) -> bool {
        match state {
            AuthCeremonyState::Pending => {
                matches!(failure, AuthCeremonyFailure::VerifierLostOnRestart)
            }
            AuthCeremonyState::Claimed => matches!(
                failure,
                AuthCeremonyFailure::ExchangeFailed
                    | AuthCeremonyFailure::StatusRejected
                    | AuthCeremonyFailure::LocalAuthorizationDenied
            ),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationMethod {
    TrailBasePassword,
    TrailBaseSocial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthenticationAssurance {
    SingleFactor,
    MultiFactor,
}

impl AuthenticationMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TrailBasePassword => "trailbase_password",
            Self::TrailBaseSocial => "trailbase_social",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "trailbase_password" => Some(Self::TrailBasePassword),
            "trailbase_social" => Some(Self::TrailBaseSocial),
            _ => None,
        }
    }

    pub const fn assurance(self) -> AuthenticationAssurance {
        AuthenticationAssurance::SingleFactor
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationUnavailableReason {
    TrailBasePasswordTotpContinuityUnavailable,
}

impl AuthenticationUnavailableReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TrailBasePasswordTotpContinuityUnavailable => {
                "trailbase_password_totp_continuity_unavailable"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessAuditEventKind {
    TrailBaseActivated,
    TrailBaseBlocked,
    AnchorLinked,
    FirstAdministratorBootstrapped,
    SubjectLifecycleChanged,
    MembershipInvited,
    MembershipLifecycleChanged,
    MembershipRoleChanged,
    CeremonyClaimed,
    CeremonyCompleted,
    CeremonyCancelled,
    CeremonyFailed,
    BrowserSessionIssued,
    BrowserSessionRevoked,
}

impl AccessAuditEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TrailBaseActivated => "trailbase_activated",
            Self::TrailBaseBlocked => "trailbase_blocked",
            Self::AnchorLinked => "anchor_linked",
            Self::FirstAdministratorBootstrapped => "first_administrator_bootstrapped",
            Self::SubjectLifecycleChanged => "subject_lifecycle_changed",
            Self::MembershipInvited => "membership_invited",
            Self::MembershipLifecycleChanged => "membership_lifecycle_changed",
            Self::MembershipRoleChanged => "membership_role_changed",
            Self::CeremonyClaimed => "ceremony_claimed",
            Self::CeremonyCompleted => "ceremony_completed",
            Self::CeremonyCancelled => "ceremony_cancelled",
            Self::CeremonyFailed => "ceremony_failed",
            Self::BrowserSessionIssued => "browser_session_issued",
            Self::BrowserSessionRevoked => "browser_session_revoked",
        }
    }

    pub fn from_storage(value: &str) -> Option<Self> {
        match value {
            "trailbase_activated" => Some(Self::TrailBaseActivated),
            "trailbase_blocked" => Some(Self::TrailBaseBlocked),
            "anchor_linked" => Some(Self::AnchorLinked),
            "first_administrator_bootstrapped" => Some(Self::FirstAdministratorBootstrapped),
            "subject_lifecycle_changed" => Some(Self::SubjectLifecycleChanged),
            "membership_invited" => Some(Self::MembershipInvited),
            "membership_lifecycle_changed" => Some(Self::MembershipLifecycleChanged),
            "membership_role_changed" => Some(Self::MembershipRoleChanged),
            "ceremony_claimed" => Some(Self::CeremonyClaimed),
            "ceremony_completed" => Some(Self::CeremonyCompleted),
            "ceremony_cancelled" => Some(Self::CeremonyCancelled),
            "ceremony_failed" => Some(Self::CeremonyFailed),
            "browser_session_issued" => Some(Self::BrowserSessionIssued),
            "browser_session_revoked" => Some(Self::BrowserSessionRevoked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticationProvenance {
    method: AuthenticationMethod,
    verified_at: DateTime<Utc>,
    activation_generation: u64,
}

impl AuthenticationProvenance {
    pub const fn new(
        method: AuthenticationMethod,
        verified_at: DateTime<Utc>,
        activation_generation: u64,
    ) -> Self {
        Self {
            method,
            verified_at,
            activation_generation,
        }
    }

    pub const fn method(&self) -> AuthenticationMethod {
        self.method
    }
    pub const fn verified_at(&self) -> DateTime<Utc> {
        self.verified_at
    }
    pub const fn activation_generation(&self) -> u64 {
        self.activation_generation
    }
    pub const fn assurance(&self) -> AuthenticationAssurance {
        self.method.assurance()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentAuthentication {
    subject_id: AuthSubjectId,
    provenance: AuthenticationProvenance,
    auth_epoch: u64,
    expires_at: DateTime<Utc>,
}

impl RecentAuthentication {
    pub fn try_new(
        subject_id: AuthSubjectId,
        provenance: AuthenticationProvenance,
        auth_epoch: u64,
        expires_at: DateTime<Utc>,
        maximum_window: TimeDelta,
    ) -> Result<Self, AccessInvariantError> {
        let latest_expiry = provenance
            .verified_at()
            .checked_add_signed(maximum_window)
            .ok_or(AccessInvariantError::InvalidAuthenticationProofExpiry)?;
        if maximum_window <= TimeDelta::zero()
            || expires_at <= provenance.verified_at()
            || expires_at > latest_expiry
        {
            return Err(AccessInvariantError::InvalidAuthenticationProofExpiry);
        }
        Ok(Self {
            subject_id,
            provenance,
            auth_epoch,
            expires_at,
        })
    }

    pub const fn subject_id(&self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn provenance(&self) -> AuthenticationProvenance {
        self.provenance
    }
    pub const fn auth_epoch(&self) -> u64 {
        self.auth_epoch
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn satisfies(
        &self,
        subject: &AuthSubject,
        activation_generation: u64,
        minimum_assurance: AuthenticationAssurance,
        at: DateTime<Utc>,
    ) -> bool {
        self.subject_id == subject.id()
            && matches!(subject.lifecycle(), AuthSubjectLifecycle::Active)
            && self.auth_epoch == subject.auth_epoch()
            && self.provenance.activation_generation() == activation_generation
            && self.provenance.assurance() >= minimum_assurance
            && at >= self.provenance.verified_at()
            && at < self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSessionState {
    Active,
    Revoked,
    IdleExpired,
    AbsoluteExpired,
    SubjectInactive,
    SubjectMismatch,
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
    pub fn try_new(
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
    ) -> Result<Self, AccessInvariantError> {
        if last_seen_at < created_at
            || idle_expires_at <= last_seen_at
            || absolute_expires_at < idle_expires_at
            || revoked_at.is_some_and(|at| at < created_at)
        {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        Ok(Self {
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
        })
    }

    pub const fn id(&self) -> BrowserSessionId {
        self.id
    }
    pub const fn subject_id(&self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn selected_profile_grant_id(&self) -> ProfileGrantId {
        self.selected_profile_grant_id
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn last_seen_at(&self) -> DateTime<Utc> {
        self.last_seen_at
    }
    pub const fn idle_expires_at(&self) -> DateTime<Utc> {
        self.idle_expires_at
    }
    pub const fn absolute_expires_at(&self) -> DateTime<Utc> {
        self.absolute_expires_at
    }
    pub const fn revoked_at(&self) -> Option<DateTime<Utc>> {
        self.revoked_at
    }
    pub const fn auth_epoch(&self) -> u64 {
        self.auth_epoch
    }
    pub const fn authorization_epoch(&self) -> u64 {
        self.authorization_epoch
    }
    pub const fn rotation_generation(&self) -> u64 {
        self.rotation_generation
    }

    pub fn state(&self, subject: &AuthSubject, at: DateTime<Utc>) -> BrowserSessionState {
        if self.revoked_at.is_some() {
            BrowserSessionState::Revoked
        } else if self.subject_id != subject.id() {
            BrowserSessionState::SubjectMismatch
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_700_000_000 + seconds, 0)
            .single()
            .expect("time")
    }

    fn active_subject() -> AuthSubject {
        AuthSubject::try_new(
            AuthSubjectId::new_v7(),
            AuthSubjectLifecycle::Active,
            2,
            3,
            at(0),
            at(0),
        )
        .expect("subject")
    }

    fn session(subject: &AuthSubject) -> FastiBrowserSession {
        FastiBrowserSession::try_new(
            BrowserSessionId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            ProfileGrantId::new_v7(),
            at(0),
            at(0),
            at(30),
            at(120),
            None,
            subject.auth_epoch(),
            subject.authorization_epoch(),
            0,
        )
        .expect("session")
    }

    fn membership(subject: &AuthSubject, role: WorkspaceRole) -> WorkspaceMembership {
        WorkspaceMembership::try_new(
            MembershipId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            MembershipLifecycle::Active,
            role,
            at(0),
            at(0),
        )
        .expect("membership")
    }

    #[test]
    fn lifecycle_changes_advance_auth_epoch_and_deleted_is_terminal() {
        let mut subject = active_subject();
        assert!(subject
            .transition_lifecycle(
                AuthSubjectLifecycle::Disabled,
                AdministratorContinuity::Preserved,
                at(1),
            )
            .expect("disable"));
        assert_eq!(subject.auth_epoch(), 3);
        assert!(subject
            .transition_lifecycle(
                AuthSubjectLifecycle::Deleted,
                AdministratorContinuity::Preserved,
                at(2),
            )
            .expect("delete"));
        assert_eq!(
            subject.transition_lifecycle(
                AuthSubjectLifecycle::Active,
                AdministratorContinuity::Preserved,
                at(3),
            ),
            Err(AccessInvariantError::DeletedSubjectIsTerminal)
        );
    }

    #[test]
    fn session_rejects_invalid_time_order_and_distinguishes_terminal_states() {
        let subject = active_subject();
        assert!(FastiBrowserSession::try_new(
            BrowserSessionId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            ProfileGrantId::new_v7(),
            at(1),
            at(0),
            at(30),
            at(120),
            None,
            2,
            3,
            0,
        )
        .is_err());
        let session = session(&subject);
        assert_eq!(session.state(&subject, at(29)), BrowserSessionState::Active);
        assert_eq!(
            session.state(&subject, at(30)),
            BrowserSessionState::IdleExpired
        );
        let other = AuthSubject::try_new(
            AuthSubjectId::new_v7(),
            AuthSubjectLifecycle::Active,
            2,
            3,
            at(0),
            at(0),
        )
        .expect("other subject");
        assert_eq!(
            session.state(&other, at(1)),
            BrowserSessionState::SubjectMismatch
        );
        assert_eq!(
            session.state(&subject, at(120)),
            BrowserSessionState::AbsoluteExpired
        );

        let revoked = FastiBrowserSession::try_new(
            BrowserSessionId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            ProfileGrantId::new_v7(),
            at(0),
            at(0),
            at(30),
            at(120),
            Some(at(1)),
            subject.auth_epoch(),
            subject.authorization_epoch(),
            0,
        )
        .expect("revoked session");
        assert_eq!(revoked.state(&subject, at(2)), BrowserSessionState::Revoked);

        let mut inactive = subject;
        inactive
            .transition_lifecycle(
                AuthSubjectLifecycle::Disabled,
                AdministratorContinuity::Preserved,
                at(1),
            )
            .expect("disable");
        assert_eq!(
            session.state(&inactive, at(2)),
            BrowserSessionState::SubjectInactive
        );
    }

    #[test]
    fn membership_changes_advance_authorization_epoch() {
        let mut subject = active_subject();
        let mut membership = membership(&subject, WorkspaceRole::Member);
        let session_before_promotion = session(&subject);

        assert!(!membership
            .change_role(&mut subject, WorkspaceRole::Member, 0, at(0))
            .expect("unchanged role"));
        assert!(!membership
            .transition_lifecycle(&mut subject, MembershipLifecycle::Active, 0, at(0))
            .expect("unchanged lifecycle"));
        assert_eq!(subject.authorization_epoch(), 3);

        assert!(membership
            .change_role(&mut subject, WorkspaceRole::Administrator, 0, at(1))
            .expect("promote"));
        assert_eq!(subject.authorization_epoch(), 4);
        assert!(membership.is_authorization_viable_administrator(&subject));
        assert_eq!(
            session_before_promotion.state(&subject, at(2)),
            BrowserSessionState::PolicyChanged
        );

        let session_before_suspension = FastiBrowserSession::try_new(
            BrowserSessionId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            ProfileGrantId::new_v7(),
            at(1),
            at(1),
            at(30),
            at(120),
            None,
            subject.auth_epoch(),
            subject.authorization_epoch(),
            0,
        )
        .expect("session before suspension");

        assert!(membership
            .transition_lifecycle(&mut subject, MembershipLifecycle::Suspended, 2, at(2))
            .expect("suspend"));
        assert_eq!(subject.authorization_epoch(), 5);
        assert!(!membership.is_authorization_viable_administrator(&subject));
        assert_eq!(
            session_before_suspension.state(&subject, at(3)),
            BrowserSessionState::PolicyChanged
        );
    }

    #[test]
    fn access_storage_vocabularies_round_trip_and_reject_unknown_values() {
        for value in [WorkspaceRole::Member, WorkspaceRole::Administrator] {
            assert_eq!(WorkspaceRole::from_storage(value.as_str()), Some(value));
        }
        for value in [
            MembershipLifecycle::Invited,
            MembershipLifecycle::PendingApproval,
            MembershipLifecycle::Active,
            MembershipLifecycle::Suspended,
            MembershipLifecycle::Removed,
        ] {
            assert_eq!(
                MembershipLifecycle::from_storage(value.as_str()),
                Some(value)
            );
        }
        for value in [
            AuthCeremonyState::Pending,
            AuthCeremonyState::Claimed,
            AuthCeremonyState::Completed,
            AuthCeremonyState::Cancelled,
            AuthCeremonyState::Failed,
            AuthCeremonyState::CleanupUncertain,
            AuthCeremonyState::Expired,
        ] {
            assert_eq!(AuthCeremonyState::from_storage(value.as_str()), Some(value));
        }
        for value in [
            AuthenticationMethod::TrailBasePassword,
            AuthenticationMethod::TrailBaseSocial,
        ] {
            assert_eq!(
                AuthenticationMethod::from_storage(value.as_str()),
                Some(value)
            );
        }

        assert_eq!(WorkspaceRole::from_storage("owner"), None);
        assert_eq!(MembershipLifecycle::from_storage("unknown"), None);
        assert_eq!(AuthCeremonyState::from_storage("retryable"), None);
        assert_eq!(AuthenticationMethod::from_storage("totp_enrolled"), None);
        assert_eq!(
            AuthenticationMethod::from_storage("trailbase_password_totp"),
            None
        );
        assert_eq!(
            AuthenticationUnavailableReason::TrailBasePasswordTotpContinuityUnavailable.as_str(),
            "trailbase_password_totp_continuity_unavailable"
        );
        for value in [
            AccessAuditEventKind::TrailBaseActivated,
            AccessAuditEventKind::TrailBaseBlocked,
            AccessAuditEventKind::AnchorLinked,
            AccessAuditEventKind::FirstAdministratorBootstrapped,
            AccessAuditEventKind::SubjectLifecycleChanged,
            AccessAuditEventKind::MembershipInvited,
            AccessAuditEventKind::MembershipLifecycleChanged,
            AccessAuditEventKind::MembershipRoleChanged,
            AccessAuditEventKind::CeremonyClaimed,
            AccessAuditEventKind::CeremonyCompleted,
            AccessAuditEventKind::CeremonyCancelled,
            AccessAuditEventKind::CeremonyFailed,
            AccessAuditEventKind::BrowserSessionIssued,
            AccessAuditEventKind::BrowserSessionRevoked,
        ] {
            assert_eq!(
                AccessAuditEventKind::from_storage(value.as_str()),
                Some(value)
            );
        }
        assert_eq!(
            AccessAuditEventKind::from_storage("vendor_token_saved"),
            None
        );
    }

    #[test]
    fn final_viable_administrator_is_preserved_without_partial_mutation() {
        let mut subject = active_subject();
        let mut membership = membership(&subject, WorkspaceRole::Administrator);

        assert_eq!(
            membership
                .transition_lifecycle(&mut subject, MembershipLifecycle::Suspended, 1, at(1),),
            Err(AccessInvariantError::FinalAdministratorRequired)
        );
        assert_eq!(membership.lifecycle(), MembershipLifecycle::Active);
        assert_eq!(subject.authorization_epoch(), 3);

        assert_eq!(
            membership.change_role(&mut subject, WorkspaceRole::Member, 1, at(1)),
            Err(AccessInvariantError::FinalAdministratorRequired)
        );
        assert_eq!(membership.role(), WorkspaceRole::Administrator);
        assert_eq!(subject.authorization_epoch(), 3);

        let mut zero_count_membership = WorkspaceMembership::try_new(
            MembershipId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            MembershipLifecycle::Active,
            WorkspaceRole::Administrator,
            at(0),
            at(0),
        )
        .expect("zero-count membership");
        assert!(zero_count_membership
            .change_role(&mut subject, WorkspaceRole::Member, 0, at(1))
            .expect("zero does not claim a final administrator exists"));

        assert_eq!(
            subject.transition_lifecycle(
                AuthSubjectLifecycle::Disabled,
                AdministratorContinuity::for_subject_deactivation(1),
                at(2),
            ),
            Err(AccessInvariantError::FinalAdministratorRequired)
        );
        subject
            .transition_lifecycle(
                AuthSubjectLifecycle::Disabled,
                AdministratorContinuity::for_subject_deactivation(0),
                at(2),
            )
            .expect("disable subject with continuity");
        assert!(!membership.is_authorization_viable_administrator(&subject));
        assert!(membership
            .change_role(&mut subject, WorkspaceRole::Member, 0, at(3))
            .expect("demote inactive subject"));
    }

    #[test]
    fn trailbase_activation_is_generation_bound_and_nonrecoverable_blocks_are_terminal_in_c1() {
        let root = Sha256Digest::from_bytes(&[1; 32]);
        let mut installation =
            TrailBaseInstallation::new(TrailBaseInstanceId::new_v7(), root.clone(), at(0));

        assert_eq!(
            installation.activation_state(),
            TrailBaseActivationState::Inactive
        );
        assert!(installation.verify(&root, true, at(1)).expect("activate"));
        assert_eq!(installation.activation_generation(), 1);
        assert!(!installation
            .verify(&root, true, at(1))
            .expect("repeat verification"));

        assert!(installation
            .verify(&root, false, at(2))
            .expect("release mismatch blocks"));
        assert_eq!(installation.activation_generation(), 2);
        assert_eq!(
            installation.activation_state(),
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::ReleaseMismatch)
        );
        assert!(!installation
            .verify(&root, false, at(2))
            .expect("repeat blocker is idempotent"));
        assert!(installation
            .verify(&root, true, at(3))
            .expect("exact release repair"));
        assert_eq!(installation.activation_generation(), 2);

        assert!(installation
            .declare_restore(at(4))
            .expect("declared restore"));
        assert_eq!(installation.activation_generation(), 3);
        assert_eq!(
            installation.verify(&root, false, at(5)),
            Err(AccessInvariantError::TrailBaseInstallationBlocked)
        );
        assert_eq!(
            installation.activation_state(),
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::DeclaredRestore)
        );

        let mut root_mismatch =
            TrailBaseInstallation::new(TrailBaseInstanceId::new_v7(), root.clone(), at(0));
        root_mismatch.verify(&root, true, at(1)).expect("activate");
        root_mismatch
            .verify(&root, false, at(2))
            .expect("release mismatch");
        let generation_after_release_block = root_mismatch.activation_generation();
        assert!(root_mismatch
            .verify(&Sha256Digest::from_bytes(&[9; 32]), true, at(3))
            .expect("non-recoverable blocker replaces release mismatch"));
        assert_eq!(
            root_mismatch.activation_state(),
            TrailBaseActivationState::Blocked(
                TrailBaseActivationBlocker::PhysicalRootIdentityMismatch
            )
        );
        assert_eq!(
            root_mismatch.activation_generation(),
            generation_after_release_block
        );
        assert_eq!(
            root_mismatch.verify(&root, false, at(4)),
            Err(AccessInvariantError::TrailBaseInstallationBlocked)
        );
        assert_eq!(
            root_mismatch.activation_state(),
            TrailBaseActivationState::Blocked(
                TrailBaseActivationBlocker::PhysicalRootIdentityMismatch
            )
        );
    }

    #[test]
    fn trailbase_activation_overflow_fails_without_partial_mutation() {
        let root = Sha256Digest::from_bytes(&[2; 32]);
        let mut installation = TrailBaseInstallation::try_from_persisted(
            TrailBaseInstanceId::new_v7(),
            root.clone(),
            TrailBaseActivationState::Active,
            u64::MAX,
            at(0),
            at(0),
        )
        .expect("persisted installation");

        assert_eq!(
            installation.verify(&root, false, at(1)),
            Err(AccessInvariantError::ActivationGenerationOverflow)
        );
        assert_eq!(
            installation.activation_state(),
            TrailBaseActivationState::Active
        );
        assert_eq!(installation.activation_generation(), u64::MAX);
        assert_eq!(installation.updated_at(), at(0));
    }

    #[test]
    fn trailbase_anchor_keeps_exact_instance_subject_and_fasti_subject() {
        let instance_id = TrailBaseInstanceId::new_v7();
        let trailbase_subject = TrailBaseSubject::from_bytes([7; 16]);
        let auth_subject_id = AuthSubjectId::new_v7();
        let anchor =
            TrailBaseExternalAnchor::new(instance_id, trailbase_subject, auth_subject_id, at(1));

        assert_eq!(anchor.trailbase_instance_id(), instance_id);
        assert_eq!(anchor.trailbase_subject(), trailbase_subject);
        assert_eq!(anchor.trailbase_subject().as_bytes(), &[7; 16]);
        assert_eq!(anchor.auth_subject_id(), auth_subject_id);
        assert_eq!(anchor.linked_at(), at(1));
    }

    #[test]
    fn removed_membership_stays_terminal_and_reinvite_has_fresh_identity_and_no_access() {
        let mut subject = active_subject();
        let workspace_id = WorkspaceId::new_v7();
        let first_id = MembershipId::new_v7();
        let mut removed = WorkspaceMembership::try_new(
            first_id,
            subject.id(),
            workspace_id,
            MembershipLifecycle::Invited,
            WorkspaceRole::Member,
            at(0),
            at(0),
        )
        .expect("membership");
        removed
            .transition_lifecycle(&mut subject, MembershipLifecycle::Removed, 0, at(1))
            .expect("remove");

        let reinvited = WorkspaceMembership::try_new(
            MembershipId::new_v7(),
            subject.id(),
            workspace_id,
            MembershipLifecycle::Invited,
            WorkspaceRole::Member,
            at(2),
            at(2),
        )
        .expect("reinvite");
        assert_ne!(reinvited.id(), first_id);
        assert_eq!(removed.lifecycle(), MembershipLifecycle::Removed);
        assert_eq!(reinvited.lifecycle(), MembershipLifecycle::Invited);
        assert_eq!(reinvited.role(), WorkspaceRole::Member);
        assert!(!reinvited.lifecycle().grants_access());
    }

    #[test]
    fn membership_rejects_invalid_terminal_subject_and_time_transitions() {
        let mut subject = active_subject();
        assert_eq!(
            WorkspaceMembership::try_new(
                MembershipId::new_v7(),
                subject.id(),
                WorkspaceId::new_v7(),
                MembershipLifecycle::Invited,
                WorkspaceRole::Administrator,
                at(0),
                at(0),
            ),
            Err(AccessInvariantError::InvalidMembershipRole)
        );
        let mut invited = WorkspaceMembership::try_new(
            MembershipId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            MembershipLifecycle::Invited,
            WorkspaceRole::Member,
            at(0),
            at(0),
        )
        .expect("invited membership");

        let authorization_epoch = subject.authorization_epoch();
        assert_eq!(
            invited.change_role(&mut subject, WorkspaceRole::Administrator, 0, at(1)),
            Err(AccessInvariantError::InvalidMembershipRole)
        );
        assert_eq!(invited.role(), WorkspaceRole::Member);
        assert_eq!(subject.authorization_epoch(), authorization_epoch);

        assert_eq!(
            invited.transition_lifecycle(&mut subject, MembershipLifecycle::Suspended, 0, at(1),),
            Err(AccessInvariantError::InvalidMembershipTransition)
        );

        let mut other_subject = active_subject();
        assert_eq!(
            invited.change_role(&mut other_subject, WorkspaceRole::Administrator, 0, at(1),),
            Err(AccessInvariantError::MembershipSubjectMismatch)
        );

        invited
            .transition_lifecycle(&mut subject, MembershipLifecycle::Removed, 0, at(2))
            .expect("remove");
        assert_eq!(
            invited.change_role(&mut subject, WorkspaceRole::Administrator, 0, at(3)),
            Err(AccessInvariantError::RemovedMembershipIsTerminal)
        );
        assert_eq!(
            invited.transition_lifecycle(&mut subject, MembershipLifecycle::Active, 0, at(3),),
            Err(AccessInvariantError::RemovedMembershipIsTerminal)
        );

        let mut active = membership(&subject, WorkspaceRole::Member);
        assert_eq!(
            active.change_role(&mut subject, WorkspaceRole::Administrator, 0, at(1)),
            Err(AccessInvariantError::InvalidTimestampOrder)
        );
    }

    #[test]
    fn ceremony_can_be_claimed_and_completed_only_once() {
        let instance_id = TrailBaseInstanceId::new_v7();
        let binding = Sha256Digest::from_bytes(&[1; 32]);
        let callback = AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback");
        let mut ceremony = AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::SignIn,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            instance_id,
            7,
            binding.clone(),
            callback.clone(),
            AuthReturnTarget::ApplicationHome,
            RequestCorrelationId::new_v7(),
            at(0),
            at(30),
        )
        .expect("ceremony");
        ceremony
            .claim(&binding, instance_id, 7, &callback, at(1))
            .expect("claim");
        assert_eq!(
            ceremony.claim(&binding, instance_id, 7, &callback, at(2)),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        assert_eq!(
            ceremony.cancel(at(2)),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        ceremony.complete(at(2)).expect("complete");
        assert!(ceremony.state().is_terminal());
        assert_eq!(
            ceremony.fail(AuthCeremonyFailure::ExchangeFailed, at(3)),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );

        let mut cleanup_failure = AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::RecentAuthentication,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            instance_id,
            7,
            binding.clone(),
            callback.clone(),
            AuthReturnTarget::AccountSecurity,
            RequestCorrelationId::new_v7(),
            at(0),
            at(30),
        )
        .expect("cleanup ceremony");
        cleanup_failure
            .claim(&binding, instance_id, 7, &callback, at(1))
            .expect("claim cleanup ceremony");
        cleanup_failure
            .mark_cleanup_uncertain(AuthCeremonyFailure::LogoutUncertain, at(2))
            .expect("terminal failure");
        assert_eq!(cleanup_failure.state(), AuthCeremonyState::CleanupUncertain);
        assert!(cleanup_failure.state().is_terminal());

        let mut never_exchanged = AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            instance_id,
            7,
            binding.clone(),
            callback.clone(),
            AuthReturnTarget::FirstRun,
            RequestCorrelationId::new_v7(),
            at(0),
            at(30),
        )
        .expect("bootstrap ceremony");
        assert_eq!(
            never_exchanged
                .mark_cleanup_uncertain(AuthCeremonyFailure::ExchangeOutcomeUncertain, at(1),),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );

        never_exchanged
            .fail(AuthCeremonyFailure::VerifierLostOnRestart, at(1))
            .expect("fail before exchange");
        assert_eq!(never_exchanged.state(), AuthCeremonyState::Failed);

        let mut expired = AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::SignIn,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            instance_id,
            7,
            binding.clone(),
            callback.clone(),
            AuthReturnTarget::ApplicationHome,
            RequestCorrelationId::new_v7(),
            at(0),
            at(2),
        )
        .expect("expiring ceremony");
        assert_eq!(
            expired.claim(&binding, instance_id, 7, &callback, at(2)),
            Err(AccessInvariantError::CeremonyExpired)
        );
        assert_eq!(expired.state(), AuthCeremonyState::Expired);

        let mut cancelled = AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::SignIn,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            instance_id,
            7,
            binding,
            callback,
            AuthReturnTarget::ApplicationHome,
            RequestCorrelationId::new_v7(),
            at(0),
            at(30),
        )
        .expect("cancellable ceremony");
        cancelled.cancel(at(1)).expect("cancel");
        assert_eq!(cancelled.state(), AuthCeremonyState::Cancelled);
        assert_eq!(cancelled.terminal_at(), Some(at(1)));
        assert_eq!(
            cancelled.cancel(at(2)),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
    }

    #[test]
    fn ceremony_rejects_wrong_proof_without_consumption_and_restart_is_terminal() {
        let instance_id = TrailBaseInstanceId::new_v7();
        let binding = Sha256Digest::from_bytes(&[3; 32]);
        let wrong_binding = Sha256Digest::from_bytes(&[4; 32]);
        let callback = AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback");
        let wrong_callback = AuthCallbackPath::parse("/auth/other/callback").expect("callback");
        let make = || {
            AuthCeremony::try_new(
                OperationId::new_v7(),
                AuthCeremonyPurpose::SignIn,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                instance_id,
                7,
                binding.clone(),
                callback.clone(),
                AuthReturnTarget::ApplicationHome,
                RequestCorrelationId::new_v7(),
                at(0),
                at(30),
            )
            .expect("ceremony")
        };
        let mut ceremony = make();

        assert_eq!(
            ceremony.claim(&wrong_binding, instance_id, 7, &callback, at(1)),
            Err(AccessInvariantError::CeremonyBindingMismatch)
        );
        assert_eq!(
            ceremony.claim(&binding, TrailBaseInstanceId::new_v7(), 7, &callback, at(1),),
            Err(AccessInvariantError::CeremonyInstallationMismatch)
        );
        assert_eq!(
            ceremony.claim(&binding, instance_id, 8, &callback, at(1)),
            Err(AccessInvariantError::CeremonyGenerationMismatch)
        );
        assert_eq!(
            ceremony.claim(&binding, instance_id, 7, &wrong_callback, at(1)),
            Err(AccessInvariantError::CeremonyCallbackMismatch)
        );
        assert_eq!(ceremony.state(), AuthCeremonyState::Pending);

        assert!(ceremony.recover_after_restart(at(2)).expect("restart"));
        assert_eq!(ceremony.state(), AuthCeremonyState::Failed);
        assert_eq!(
            ceremony.failure(),
            Some(AuthCeremonyFailure::VerifierLostOnRestart)
        );
        assert!(!ceremony
            .recover_after_restart(at(3))
            .expect("terminal restart is unchanged"));

        let mut claimed = make();
        claimed
            .claim(&binding, instance_id, 7, &callback, at(1))
            .expect("claim");
        assert!(claimed.recover_after_restart(at(2)).expect("restart"));
        assert_eq!(claimed.state(), AuthCeremonyState::CleanupUncertain);
        assert_eq!(
            claimed.failure(),
            Some(AuthCeremonyFailure::ExchangeOutcomeUncertain)
        );
    }

    #[test]
    fn ceremony_uses_only_fixed_purpose_target_pairs_and_local_callback_paths() {
        let instance_id = TrailBaseInstanceId::new_v7();
        let binding = Sha256Digest::from_bytes(&[5; 32]);
        let callback = AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback");
        let pairs = [
            (
                AuthCeremonyPurpose::SignIn,
                AuthReturnTarget::ApplicationHome,
            ),
            (
                AuthCeremonyPurpose::RecentAuthentication,
                AuthReturnTarget::AccountSecurity,
            ),
            (
                AuthCeremonyPurpose::FirstAdministratorBootstrap,
                AuthReturnTarget::FirstRun,
            ),
        ];
        for (purpose, target) in pairs {
            assert!(AuthCeremony::try_new(
                OperationId::new_v7(),
                purpose,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                instance_id,
                1,
                binding.clone(),
                callback.clone(),
                target,
                RequestCorrelationId::new_v7(),
                at(0),
                at(30),
            )
            .is_ok());
            for mismatched in [
                AuthReturnTarget::ApplicationHome,
                AuthReturnTarget::AccountSecurity,
                AuthReturnTarget::FirstRun,
            ] {
                if mismatched != target {
                    assert_eq!(
                        AuthCeremony::try_new(
                            OperationId::new_v7(),
                            purpose,
                            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                            instance_id,
                            1,
                            binding.clone(),
                            callback.clone(),
                            mismatched,
                            RequestCorrelationId::new_v7(),
                            at(0),
                            at(30),
                        ),
                        Err(AccessInvariantError::InvalidCeremonyPurposeTarget)
                    );
                }
            }
        }

        for invalid in [
            "https://fasti.invalid/callback",
            "//fasti.invalid/callback",
            "/callback?next=/settings",
            "/callback#fragment",
            "/callback%2fother",
            "/callback\\other",
            "/callback other",
            "",
        ] {
            assert_eq!(
                AuthCallbackPath::parse(invalid),
                Err(AccessInvariantError::InvalidCallbackPath)
            );
        }
    }

    #[test]
    fn ceremony_persistence_rejects_impossible_state_and_failure_combinations() {
        let instance_id = TrailBaseInstanceId::new_v7();
        let binding = Sha256Digest::from_bytes(&[6; 32]);
        let callback = AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback");
        let restore = |state, failure, claimed_at, terminal_at| {
            AuthCeremony::try_from_persisted(
                OperationId::new_v7(),
                AuthCeremonyPurpose::SignIn,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                instance_id,
                2,
                binding.clone(),
                callback.clone(),
                AuthReturnTarget::ApplicationHome,
                RequestCorrelationId::new_v7(),
                state,
                failure,
                at(0),
                at(30),
                claimed_at,
                terminal_at,
            )
        };

        assert!(restore(AuthCeremonyState::Pending, None, None, None).is_ok());
        assert!(restore(AuthCeremonyState::Claimed, None, Some(at(1)), None).is_ok());
        assert!(restore(AuthCeremonyState::Completed, None, Some(at(1)), Some(at(2)),).is_ok());
        assert!(restore(
            AuthCeremonyState::Failed,
            Some(AuthCeremonyFailure::VerifierLostOnRestart),
            None,
            Some(at(2)),
        )
        .is_ok());
        assert!(restore(
            AuthCeremonyState::Failed,
            Some(AuthCeremonyFailure::LocalAuthorizationDenied),
            Some(at(1)),
            Some(at(2)),
        )
        .is_ok());
        assert!(restore(
            AuthCeremonyState::CleanupUncertain,
            Some(AuthCeremonyFailure::ExchangeOutcomeUncertain),
            Some(at(1)),
            Some(at(2)),
        )
        .is_ok());
        assert!(restore(AuthCeremonyState::Expired, None, None, Some(at(30))).is_ok());
        assert!(restore(AuthCeremonyState::Cancelled, None, None, Some(at(2))).is_ok());

        assert_eq!(
            restore(
                AuthCeremonyState::Pending,
                Some(AuthCeremonyFailure::ExchangeFailed),
                None,
                None,
            ),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        assert_eq!(
            restore(
                AuthCeremonyState::Failed,
                Some(AuthCeremonyFailure::LocalAuthorizationDenied),
                None,
                Some(at(2)),
            ),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        assert_eq!(
            restore(
                AuthCeremonyState::CleanupUncertain,
                Some(AuthCeremonyFailure::ExchangeFailed),
                Some(at(1)),
                Some(at(2)),
            ),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        assert_eq!(
            restore(AuthCeremonyState::Claimed, None, Some(at(30)), None),
            Err(AccessInvariantError::InvalidTimestampOrder)
        );
        assert_eq!(
            restore(AuthCeremonyState::Cancelled, None, None, Some(at(30))),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        assert_eq!(
            AuthCeremony::try_new(
                OperationId::new_v7(),
                AuthCeremonyPurpose::SignIn,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                instance_id,
                0,
                binding,
                callback,
                AuthReturnTarget::ApplicationHome,
                RequestCorrelationId::new_v7(),
                at(0),
                at(30),
            ),
            Err(AccessInvariantError::InvalidActivationGeneration)
        );
    }

    #[test]
    fn ceremony_failure_reasons_follow_the_exchange_state() {
        let instance_id = TrailBaseInstanceId::new_v7();
        let binding = Sha256Digest::from_bytes(&[8; 32]);
        let callback = AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback");
        let mut ceremony = AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::SignIn,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            instance_id,
            1,
            binding.clone(),
            callback.clone(),
            AuthReturnTarget::ApplicationHome,
            RequestCorrelationId::new_v7(),
            at(0),
            at(30),
        )
        .expect("ceremony");

        assert_eq!(
            ceremony.fail(AuthCeremonyFailure::LocalAuthorizationDenied, at(1)),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        ceremony
            .claim(&binding, instance_id, 1, &callback, at(1))
            .expect("claim");
        assert_eq!(
            ceremony.fail(AuthCeremonyFailure::VerifierLostOnRestart, at(2)),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        assert_eq!(
            ceremony.mark_cleanup_uncertain(AuthCeremonyFailure::ExchangeFailed, at(2)),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        ceremony
            .fail(AuthCeremonyFailure::LocalAuthorizationDenied, at(2))
            .expect("post-exchange authorization denial");
    }

    #[test]
    fn recent_auth_requires_current_subject_generation_epoch_and_assurance() {
        let mut subject = active_subject();
        let social = AuthenticationProvenance::new(AuthenticationMethod::TrailBaseSocial, at(1), 7);
        let recent = RecentAuthentication::try_new(
            subject.id(),
            social,
            subject.auth_epoch(),
            at(11),
            TimeDelta::seconds(10),
        )
        .expect("recent authentication");

        assert!(RecentAuthentication::try_new(
            subject.id(),
            social,
            subject.auth_epoch(),
            at(1),
            TimeDelta::seconds(10),
        )
        .is_err());
        assert!(RecentAuthentication::try_new(
            subject.id(),
            social,
            subject.auth_epoch(),
            at(12),
            TimeDelta::seconds(10),
        )
        .is_err());
        assert!(recent.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(2),));
        assert!(!recent.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(0),));
        assert!(!recent.satisfies(&subject, 7, AuthenticationAssurance::MultiFactor, at(2),));
        assert!(!recent.satisfies(&subject, 8, AuthenticationAssurance::SingleFactor, at(2),));
        assert!(!recent.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(11),));

        let wrong_subject = active_subject();
        assert!(!recent.satisfies(
            &wrong_subject,
            7,
            AuthenticationAssurance::SingleFactor,
            at(2),
        ));

        let stale_epoch = RecentAuthentication::try_new(
            subject.id(),
            social,
            subject.auth_epoch() - 1,
            at(11),
            TimeDelta::seconds(10),
        )
        .expect("stale proof");
        assert!(!stale_epoch.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(2),));

        subject
            .transition_lifecycle(
                AuthSubjectLifecycle::Disabled,
                AdministratorContinuity::Preserved,
                at(3),
            )
            .expect("disable");
        assert!(!recent.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(4),));

        let password =
            AuthenticationProvenance::new(AuthenticationMethod::TrailBasePassword, at(5), 7);
        assert_eq!(password.assurance(), AuthenticationAssurance::SingleFactor);
        assert!(!RecentAuthentication::try_new(
            subject.id(),
            password,
            subject.auth_epoch(),
            at(10),
            TimeDelta::seconds(10),
        )
        .expect("single-factor proof")
        .satisfies(&subject, 7, AuthenticationAssurance::MultiFactor, at(6),));
    }
}
