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
    #[error("the authentication ceremony selection binding is invalid")]
    InvalidCeremonySelectionBinding,
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

    const fn can_transition_to(self, next: Self) -> bool {
        !matches!(self, Self::Deleted) && self as u8 != next as u8
    }

    pub const fn audit_event(self) -> AccessAuditEventKind {
        match self {
            Self::Active => AccessAuditEventKind::SubjectReactivated,
            Self::Disabled => AccessAuditEventKind::SubjectDisabled,
            Self::Deleted => AccessAuditEventKind::SubjectDeleted,
            Self::RecoveryPending => AccessAuditEventKind::SubjectRecoveryPending,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessInvalidationEffect {
    SubjectAuthenticationChanged,
    MembershipAuthorizationChanged,
    MembershipAuthorizationChangedAndSessionsRevoked,
}

impl AccessInvalidationEffect {
    pub const fn revoke_browser_sessions(self) -> bool {
        matches!(
            self,
            Self::SubjectAuthenticationChanged
                | Self::MembershipAuthorizationChangedAndSessionsRevoked
        )
    }

    pub const fn invalidate_recent_authentication(self) -> bool {
        matches!(self, Self::SubjectAuthenticationChanged)
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
    ) -> Result<Option<AccessInvalidationEffect>, AccessInvariantError> {
        if at < self.updated_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if lifecycle == self.lifecycle {
            return Ok(None);
        }
        if !self.lifecycle.can_transition_to(lifecycle) {
            return Err(AccessInvariantError::DeletedSubjectIsTerminal);
        }
        if matches!(self.lifecycle, AuthSubjectLifecycle::Active)
            && !matches!(lifecycle, AuthSubjectLifecycle::Active)
        {
            administrator_continuity.ensure()?;
        }
        self.advance_authentication_epoch(at)?;
        self.lifecycle = lifecycle;
        Ok(Some(AccessInvalidationEffect::SubjectAuthenticationChanged))
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

    pub const fn audit_event(self) -> AccessAuditEventKind {
        match self {
            Self::Member => AccessAuditEventKind::MembershipDemoted,
            Self::Administrator => AccessAuditEventKind::MembershipPromoted,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipLifecycleAction {
    AcceptInvitation,
    Approve,
    Suspend,
    Resume,
    Remove,
}

impl MembershipLifecycleAction {
    const fn target(self) -> MembershipLifecycle {
        match self {
            Self::AcceptInvitation | Self::Approve | Self::Resume => MembershipLifecycle::Active,
            Self::Suspend => MembershipLifecycle::Suspended,
            Self::Remove => MembershipLifecycle::Removed,
        }
    }

    const fn applies_to(self, current: MembershipLifecycle) -> bool {
        matches!(
            (self, current),
            (Self::AcceptInvitation, MembershipLifecycle::Invited)
                | (Self::Approve, MembershipLifecycle::PendingApproval)
                | (Self::Suspend, MembershipLifecycle::Active)
                | (Self::Resume, MembershipLifecycle::Suspended)
                | (
                    Self::Remove,
                    MembershipLifecycle::Invited
                        | MembershipLifecycle::PendingApproval
                        | MembershipLifecycle::Active
                        | MembershipLifecycle::Suspended
                )
        )
    }

    pub const fn audit_event(self) -> AccessAuditEventKind {
        match self {
            Self::AcceptInvitation => AccessAuditEventKind::MembershipInvitationAccepted,
            Self::Approve => AccessAuditEventKind::MembershipApproved,
            Self::Suspend => AccessAuditEventKind::MembershipSuspended,
            Self::Resume => AccessAuditEventKind::MembershipResumed,
            Self::Remove => AccessAuditEventKind::MembershipRemoved,
        }
    }
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

    pub fn apply_lifecycle_action(
        &mut self,
        subject: &mut AuthSubject,
        action: MembershipLifecycleAction,
        viable_administrator_count: u64,
        at: DateTime<Utc>,
    ) -> Result<Option<AccessInvalidationEffect>, AccessInvariantError> {
        self.validate_subject_and_time(subject, at)?;
        if matches!(self.lifecycle, MembershipLifecycle::Removed) {
            return Err(AccessInvariantError::RemovedMembershipIsTerminal);
        }
        if !action.applies_to(self.lifecycle) {
            return Err(AccessInvariantError::InvalidMembershipTransition);
        }
        let lifecycle = action.target();
        self.administrator_continuity(subject, lifecycle, self.role, viable_administrator_count)
            .ensure()?;
        subject.advance_authorization_epoch(at)?;
        self.lifecycle = lifecycle;
        self.updated_at = at;
        Ok(Some(
            if matches!(
                lifecycle,
                MembershipLifecycle::Suspended | MembershipLifecycle::Removed
            ) {
                AccessInvalidationEffect::MembershipAuthorizationChangedAndSessionsRevoked
            } else {
                AccessInvalidationEffect::MembershipAuthorizationChanged
            },
        ))
    }

    pub fn change_role(
        &mut self,
        subject: &mut AuthSubject,
        role: WorkspaceRole,
        viable_administrator_count: u64,
        at: DateTime<Utc>,
    ) -> Result<Option<AccessInvalidationEffect>, AccessInvariantError> {
        self.validate_subject_and_time(subject, at)?;
        if role == self.role {
            return Ok(None);
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
        Ok(Some(
            AccessInvalidationEffect::MembershipAuthorizationChanged,
        ))
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
    SelectionRequired,
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
            Self::SelectionRequired => "selection_required",
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
            "selection_required" => Some(Self::SelectionRequired),
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
    LocalPersistenceFailed,
    TrustUnavailable,
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
            Self::LocalPersistenceFailed => "local_persistence_failed",
            Self::TrustUnavailable => "trust_unavailable",
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
            "local_persistence_failed" => Some(Self::LocalPersistenceFailed),
            "trust_unavailable" => Some(Self::TrustUnavailable),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthCeremonySelection {
    workspace_id: WorkspaceId,
    selected_profile_grant_id: ProfileGrantId,
    bound_browser_session_id: Option<BrowserSessionId>,
    invited_membership_id: Option<MembershipId>,
}

impl AuthCeremonySelection {
    pub fn try_new(
        purpose: AuthCeremonyPurpose,
        workspace_id: WorkspaceId,
        selected_profile_grant_id: ProfileGrantId,
        bound_browser_session_id: Option<BrowserSessionId>,
        invited_membership_id: Option<MembershipId>,
    ) -> Result<Self, AccessInvariantError> {
        let selection = Self {
            workspace_id,
            selected_profile_grant_id,
            bound_browser_session_id,
            invited_membership_id,
        };
        if !selection.valid_for(purpose) {
            return Err(AccessInvariantError::InvalidCeremonySelectionBinding);
        }
        Ok(selection)
    }

    const fn valid_for(self, purpose: AuthCeremonyPurpose) -> bool {
        match purpose {
            AuthCeremonyPurpose::SignIn => self.bound_browser_session_id.is_none(),
            AuthCeremonyPurpose::RecentAuthentication => {
                self.bound_browser_session_id.is_some() && self.invited_membership_id.is_none()
            }
            AuthCeremonyPurpose::FirstAdministratorBootstrap => {
                self.bound_browser_session_id.is_none() && self.invited_membership_id.is_none()
            }
        }
    }

    pub const fn workspace_id(self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn selected_profile_grant_id(self) -> ProfileGrantId {
        self.selected_profile_grant_id
    }

    pub const fn bound_browser_session_id(self) -> Option<BrowserSessionId> {
        self.bound_browser_session_id
    }

    pub const fn invited_membership_id(self) -> Option<MembershipId> {
        self.invited_membership_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthCeremonyConfirmation {
    subject_id: AuthSubjectId,
    auth_epoch: u64,
    authorization_epoch: u64,
    provenance: AuthenticationProvenance,
}

impl AuthCeremonyConfirmation {
    pub const fn new(subject: AuthSubject, provenance: AuthenticationProvenance) -> Self {
        Self {
            subject_id: subject.id(),
            auth_epoch: subject.auth_epoch(),
            authorization_epoch: subject.authorization_epoch(),
            provenance,
        }
    }

    pub const fn try_from_persisted(
        subject_id: AuthSubjectId,
        auth_epoch: u64,
        authorization_epoch: u64,
        provenance: AuthenticationProvenance,
    ) -> Self {
        Self {
            subject_id,
            auth_epoch,
            authorization_epoch,
            provenance,
        }
    }

    pub const fn subject_id(self) -> AuthSubjectId {
        self.subject_id
    }

    pub const fn auth_epoch(self) -> u64 {
        self.auth_epoch
    }

    pub const fn authorization_epoch(self) -> u64 {
        self.authorization_epoch
    }

    pub const fn provenance(self) -> AuthenticationProvenance {
        self.provenance
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
    selection: Option<AuthCeremonySelection>,
    remembered: bool,
    confirmation: Option<AuthCeremonyConfirmation>,
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
        selection: Option<AuthCeremonySelection>,
        remembered: bool,
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
        if !Self::initial_binding_is_valid(purpose, selection, remembered) {
            return Err(AccessInvariantError::InvalidCeremonySelectionBinding);
        }
        Ok(Self {
            id,
            purpose,
            protocol,
            trailbase_instance_id,
            activation_generation,
            browser_binding_digest,
            selection,
            remembered,
            confirmation: None,
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
        selection: Option<AuthCeremonySelection>,
        remembered: bool,
        confirmation: Option<AuthCeremonyConfirmation>,
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
            selection,
            remembered,
            confirmation,
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
    pub const fn selection(&self) -> Option<AuthCeremonySelection> {
        self.selection
    }
    pub const fn remembered(&self) -> bool {
        self.remembered
    }
    pub const fn confirmation(&self) -> Option<AuthCeremonyConfirmation> {
        self.confirmation
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
        if self.expire(at)? {
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
        if matches!(self.purpose, AuthCeremonyPurpose::SignIn) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        self.validate_claimed_transition(at)?;
        if at >= self.expires_at {
            return Err(AccessInvariantError::CeremonyExpired);
        }
        self.state = AuthCeremonyState::Completed;
        self.terminal_at = Some(at);
        Ok(())
    }

    pub fn require_selection(
        &mut self,
        confirmation: AuthCeremonyConfirmation,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        self.validate_claimed_transition(at)?;
        if !matches!(self.purpose, AuthCeremonyPurpose::SignIn)
            || self.selection.is_some()
            || confirmation.provenance().activation_generation() != self.activation_generation
            || confirmation.provenance().verified_at() < self.claimed_at.unwrap_or(self.created_at)
            || confirmation.provenance().verified_at() > at
        {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        if at >= self.expires_at {
            return Err(AccessInvariantError::CeremonyExpired);
        }
        self.state = AuthCeremonyState::SelectionRequired;
        self.confirmation = Some(confirmation);
        Ok(())
    }

    pub fn complete_selection(
        &mut self,
        selection: AuthCeremonySelection,
        at: DateTime<Utc>,
    ) -> Result<(), AccessInvariantError> {
        if !matches!(self.state, AuthCeremonyState::SelectionRequired)
            || !selection.valid_for(AuthCeremonyPurpose::SignIn)
            || self.confirmation.is_none()
        {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        self.validate_transition_time(at)?;
        if at >= self.expires_at {
            return Err(AccessInvariantError::CeremonyExpired);
        }
        self.selection = Some(selection);
        self.state = AuthCeremonyState::Completed;
        self.terminal_at = Some(at);
        Ok(())
    }

    pub fn cancel(&mut self, at: DateTime<Utc>) -> Result<(), AccessInvariantError> {
        if !matches!(
            self.state,
            AuthCeremonyState::Pending | AuthCeremonyState::SelectionRequired
        ) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        if self.expire(at)? {
            return Err(AccessInvariantError::CeremonyExpired);
        }
        self.state = AuthCeremonyState::Cancelled;
        self.terminal_at = Some(at);
        Ok(())
    }

    pub fn expire(&mut self, at: DateTime<Utc>) -> Result<bool, AccessInvariantError> {
        if !matches!(
            self.state,
            AuthCeremonyState::Pending | AuthCeremonyState::SelectionRequired
        ) {
            return Ok(false);
        }
        if at < self.created_at {
            return Err(AccessInvariantError::InvalidTimestampOrder);
        }
        if at < self.expires_at {
            return Ok(false);
        }
        self.state = AuthCeremonyState::Expired;
        self.terminal_at = Some(at);
        Ok(true)
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
            AuthCeremonyState::SelectionRequired => Ok(false),
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

        let binding_is_valid = match self.purpose {
            AuthCeremonyPurpose::SignIn => match self.state {
                AuthCeremonyState::Pending | AuthCeremonyState::Claimed => {
                    self.selection.is_none() && self.confirmation.is_none()
                }
                AuthCeremonyState::SelectionRequired => {
                    self.selection.is_none() && self.confirmation.is_some()
                }
                AuthCeremonyState::Completed => {
                    self.selection
                        .is_some_and(|selection| selection.valid_for(self.purpose))
                        && self.confirmation.is_some()
                }
                AuthCeremonyState::Cancelled | AuthCeremonyState::Expired => {
                    self.selection.is_none()
                        && (self.confirmation.is_some() == self.claimed_at.is_some())
                }
                _ => self.selection.is_none() && self.confirmation.is_none(),
            },
            AuthCeremonyPurpose::RecentAuthentication
            | AuthCeremonyPurpose::FirstAdministratorBootstrap => {
                !self.remembered
                    && self.confirmation.is_none()
                    && self
                        .selection
                        .is_some_and(|selection| selection.valid_for(self.purpose))
            }
        };
        if !binding_is_valid {
            return Err(AccessInvariantError::InvalidCeremonySelectionBinding);
        }

        let valid = match self.state {
            AuthCeremonyState::Pending => {
                self.failure.is_none() && self.claimed_at.is_none() && self.terminal_at.is_none()
            }
            AuthCeremonyState::Claimed => {
                self.failure.is_none() && self.claimed_at.is_some() && self.terminal_at.is_none()
            }
            AuthCeremonyState::SelectionRequired => {
                self.failure.is_none() && self.claimed_at.is_some() && self.terminal_at.is_none()
            }
            AuthCeremonyState::Completed => {
                self.failure.is_none()
                    && self.claimed_at.is_some()
                    && self
                        .terminal_at
                        .is_some_and(|terminal_at| terminal_at < self.expires_at)
            }
            AuthCeremonyState::Cancelled => {
                self.failure.is_none()
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
                    | AuthCeremonyFailure::LocalPersistenceFailed
                    | AuthCeremonyFailure::TrustUnavailable
            ),
            _ => false,
        }
    }

    fn initial_binding_is_valid(
        purpose: AuthCeremonyPurpose,
        selection: Option<AuthCeremonySelection>,
        remembered: bool,
    ) -> bool {
        match purpose {
            AuthCeremonyPurpose::SignIn => selection.is_none(),
            AuthCeremonyPurpose::RecentAuthentication
            | AuthCeremonyPurpose::FirstAdministratorBootstrap => {
                !remembered && selection.is_some_and(|selection| selection.valid_for(purpose))
            }
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
    SubjectDisabled,
    SubjectDeleted,
    SubjectRecoveryPending,
    SubjectReactivated,
    MembershipInvited,
    MembershipApprovalRequested,
    MembershipInvitationAccepted,
    MembershipApproved,
    MembershipSuspended,
    MembershipResumed,
    MembershipRemoved,
    MembershipPromoted,
    MembershipDemoted,
    CeremonyClaimed,
    CeremonySelectionRequired,
    CeremonyCompleted,
    CeremonyCancelled,
    CeremonyExpired,
    CeremonyCleanupUncertain,
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
            Self::SubjectDisabled => "subject_disabled",
            Self::SubjectDeleted => "subject_deleted",
            Self::SubjectRecoveryPending => "subject_recovery_pending",
            Self::SubjectReactivated => "subject_reactivated",
            Self::MembershipInvited => "membership_invited",
            Self::MembershipApprovalRequested => "membership_approval_requested",
            Self::MembershipInvitationAccepted => "membership_invitation_accepted",
            Self::MembershipApproved => "membership_approved",
            Self::MembershipSuspended => "membership_suspended",
            Self::MembershipResumed => "membership_resumed",
            Self::MembershipRemoved => "membership_removed",
            Self::MembershipPromoted => "membership_promoted",
            Self::MembershipDemoted => "membership_demoted",
            Self::CeremonyClaimed => "ceremony_claimed",
            Self::CeremonySelectionRequired => "ceremony_selection_required",
            Self::CeremonyCompleted => "ceremony_completed",
            Self::CeremonyCancelled => "ceremony_cancelled",
            Self::CeremonyExpired => "ceremony_expired",
            Self::CeremonyCleanupUncertain => "ceremony_cleanup_uncertain",
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
            "subject_disabled" => Some(Self::SubjectDisabled),
            "subject_deleted" => Some(Self::SubjectDeleted),
            "subject_recovery_pending" => Some(Self::SubjectRecoveryPending),
            "subject_reactivated" => Some(Self::SubjectReactivated),
            "membership_invited" => Some(Self::MembershipInvited),
            "membership_approval_requested" => Some(Self::MembershipApprovalRequested),
            "membership_invitation_accepted" => Some(Self::MembershipInvitationAccepted),
            "membership_approved" => Some(Self::MembershipApproved),
            "membership_suspended" => Some(Self::MembershipSuspended),
            "membership_resumed" => Some(Self::MembershipResumed),
            "membership_removed" => Some(Self::MembershipRemoved),
            "membership_promoted" => Some(Self::MembershipPromoted),
            "membership_demoted" => Some(Self::MembershipDemoted),
            "ceremony_claimed" => Some(Self::CeremonyClaimed),
            "ceremony_selection_required" => Some(Self::CeremonySelectionRequired),
            "ceremony_completed" => Some(Self::CeremonyCompleted),
            "ceremony_cancelled" => Some(Self::CeremonyCancelled),
            "ceremony_expired" => Some(Self::CeremonyExpired),
            "ceremony_cleanup_uncertain" => Some(Self::CeremonyCleanupUncertain),
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

    fn ceremony_selection(purpose: AuthCeremonyPurpose) -> AuthCeremonySelection {
        AuthCeremonySelection::try_new(
            purpose,
            WorkspaceId::new_v7(),
            ProfileGrantId::new_v7(),
            matches!(purpose, AuthCeremonyPurpose::RecentAuthentication)
                .then(BrowserSessionId::new_v7),
            None,
        )
        .expect("ceremony selection")
    }

    fn ceremony_start_selection(purpose: AuthCeremonyPurpose) -> Option<AuthCeremonySelection> {
        (!matches!(purpose, AuthCeremonyPurpose::SignIn)).then(|| ceremony_selection(purpose))
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
        assert_eq!(
            subject
                .transition_lifecycle(
                    AuthSubjectLifecycle::Disabled,
                    AdministratorContinuity::Preserved,
                    at(1),
                )
                .expect("disable"),
            Some(AccessInvalidationEffect::SubjectAuthenticationChanged)
        );
        assert_eq!(subject.auth_epoch(), 3);
        assert_eq!(
            subject
                .transition_lifecycle(
                    AuthSubjectLifecycle::Deleted,
                    AdministratorContinuity::Preserved,
                    at(2),
                )
                .expect("delete"),
            Some(AccessInvalidationEffect::SubjectAuthenticationChanged)
        );
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
    fn subject_lifecycle_table_and_epoch_overflow_are_explicit() {
        let states = [
            AuthSubjectLifecycle::Active,
            AuthSubjectLifecycle::Disabled,
            AuthSubjectLifecycle::RecoveryPending,
            AuthSubjectLifecycle::Deleted,
        ];
        for current in states {
            for next in states {
                let mut subject =
                    AuthSubject::try_new(AuthSubjectId::new_v7(), current, 0, 0, at(0), at(0))
                        .expect("subject");
                let result =
                    subject.transition_lifecycle(next, AdministratorContinuity::Preserved, at(1));
                if current == next {
                    assert_eq!(result, Ok(None));
                } else if matches!(current, AuthSubjectLifecycle::Deleted) {
                    assert_eq!(result, Err(AccessInvariantError::DeletedSubjectIsTerminal));
                } else {
                    assert_eq!(
                        result,
                        Ok(Some(AccessInvalidationEffect::SubjectAuthenticationChanged))
                    );
                    assert_eq!(subject.lifecycle(), next);
                    assert_eq!(subject.auth_epoch(), 1);
                }
            }
        }

        let mut overflow = AuthSubject::try_new(
            AuthSubjectId::new_v7(),
            AuthSubjectLifecycle::Active,
            u64::MAX,
            0,
            at(0),
            at(0),
        )
        .expect("subject");
        assert_eq!(
            overflow.transition_lifecycle(
                AuthSubjectLifecycle::Disabled,
                AdministratorContinuity::Preserved,
                at(1),
            ),
            Err(AccessInvariantError::EpochOverflow)
        );
        assert_eq!(overflow.lifecycle(), AuthSubjectLifecycle::Active);
        assert_eq!(overflow.updated_at(), at(0));
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

        assert_eq!(
            membership
                .change_role(&mut subject, WorkspaceRole::Member, 0, at(0))
                .expect("unchanged role"),
            None
        );
        assert_eq!(subject.authorization_epoch(), 3);

        assert_eq!(
            membership
                .change_role(&mut subject, WorkspaceRole::Administrator, 0, at(1))
                .expect("promote"),
            Some(AccessInvalidationEffect::MembershipAuthorizationChanged)
        );
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

        assert_eq!(
            membership
                .apply_lifecycle_action(&mut subject, MembershipLifecycleAction::Suspend, 2, at(2))
                .expect("suspend"),
            Some(AccessInvalidationEffect::MembershipAuthorizationChangedAndSessionsRevoked)
        );
        assert_eq!(subject.authorization_epoch(), 5);
        assert!(!membership.is_authorization_viable_administrator(&subject));
        assert_eq!(
            session_before_suspension.state(&subject, at(3)),
            BrowserSessionState::PolicyChanged
        );
    }

    #[test]
    fn membership_actions_preserve_meaning_and_invalidation_policy() {
        let cases = [
            (
                MembershipLifecycle::Invited,
                MembershipLifecycleAction::AcceptInvitation,
                MembershipLifecycle::Active,
                AccessAuditEventKind::MembershipInvitationAccepted,
                AccessInvalidationEffect::MembershipAuthorizationChanged,
            ),
            (
                MembershipLifecycle::PendingApproval,
                MembershipLifecycleAction::Approve,
                MembershipLifecycle::Active,
                AccessAuditEventKind::MembershipApproved,
                AccessInvalidationEffect::MembershipAuthorizationChanged,
            ),
            (
                MembershipLifecycle::Active,
                MembershipLifecycleAction::Suspend,
                MembershipLifecycle::Suspended,
                AccessAuditEventKind::MembershipSuspended,
                AccessInvalidationEffect::MembershipAuthorizationChangedAndSessionsRevoked,
            ),
            (
                MembershipLifecycle::Suspended,
                MembershipLifecycleAction::Resume,
                MembershipLifecycle::Active,
                AccessAuditEventKind::MembershipResumed,
                AccessInvalidationEffect::MembershipAuthorizationChanged,
            ),
            (
                MembershipLifecycle::Active,
                MembershipLifecycleAction::Remove,
                MembershipLifecycle::Removed,
                AccessAuditEventKind::MembershipRemoved,
                AccessInvalidationEffect::MembershipAuthorizationChangedAndSessionsRevoked,
            ),
        ];
        for (current, action, expected, audit, effect) in cases {
            let mut subject = active_subject();
            let mut membership = WorkspaceMembership::try_new(
                MembershipId::new_v7(),
                subject.id(),
                WorkspaceId::new_v7(),
                current,
                WorkspaceRole::Member,
                at(0),
                at(0),
            )
            .expect("membership");
            assert_eq!(action.audit_event(), audit);
            assert_eq!(
                membership
                    .apply_lifecycle_action(&mut subject, action, 0, at(1))
                    .expect("action"),
                Some(effect)
            );
            assert_eq!(membership.lifecycle(), expected);
            assert_eq!(subject.authorization_epoch(), 4);
            assert_eq!(
                effect.revoke_browser_sessions(),
                matches!(
                    action,
                    MembershipLifecycleAction::Suspend | MembershipLifecycleAction::Remove
                )
            );
            assert!(!effect.invalidate_recent_authentication());
        }

        let mut subject = active_subject();
        let mut invited = WorkspaceMembership::try_new(
            MembershipId::new_v7(),
            subject.id(),
            WorkspaceId::new_v7(),
            MembershipLifecycle::Invited,
            WorkspaceRole::Member,
            at(0),
            at(0),
        )
        .expect("membership");
        assert_eq!(
            invited.apply_lifecycle_action(
                &mut subject,
                MembershipLifecycleAction::Approve,
                0,
                at(1),
            ),
            Err(AccessInvariantError::InvalidMembershipTransition)
        );
        assert_eq!(invited.lifecycle(), MembershipLifecycle::Invited);
        assert_eq!(subject.authorization_epoch(), 3);

        let mut overflow = AuthSubject::try_new(
            AuthSubjectId::new_v7(),
            AuthSubjectLifecycle::Active,
            0,
            u64::MAX,
            at(0),
            at(0),
        )
        .expect("subject");
        let mut membership = WorkspaceMembership::try_new(
            MembershipId::new_v7(),
            overflow.id(),
            WorkspaceId::new_v7(),
            MembershipLifecycle::Active,
            WorkspaceRole::Member,
            at(0),
            at(0),
        )
        .expect("membership");
        assert_eq!(
            membership.apply_lifecycle_action(
                &mut overflow,
                MembershipLifecycleAction::Suspend,
                0,
                at(1),
            ),
            Err(AccessInvariantError::EpochOverflow)
        );
        assert_eq!(membership.lifecycle(), MembershipLifecycle::Active);
        assert_eq!(membership.updated_at(), at(0));
        assert_eq!(overflow.authorization_epoch(), u64::MAX);
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
            AuthCeremonyState::SelectionRequired,
            AuthCeremonyState::Completed,
            AuthCeremonyState::Cancelled,
            AuthCeremonyState::Failed,
            AuthCeremonyState::CleanupUncertain,
            AuthCeremonyState::Expired,
        ] {
            assert_eq!(AuthCeremonyState::from_storage(value.as_str()), Some(value));
        }
        for value in [
            AuthCeremonyFailure::VerifierLostOnRestart,
            AuthCeremonyFailure::ExchangeOutcomeUncertain,
            AuthCeremonyFailure::ExchangeFailed,
            AuthCeremonyFailure::StatusRejected,
            AuthCeremonyFailure::LogoutUncertain,
            AuthCeremonyFailure::LocalAuthorizationDenied,
            AuthCeremonyFailure::LocalPersistenceFailed,
            AuthCeremonyFailure::TrustUnavailable,
        ] {
            assert_eq!(
                AuthCeremonyFailure::from_storage(value.as_str()),
                Some(value)
            );
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
        assert_eq!(AuthCeremonyFailure::from_storage("database_error"), None);
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
            AccessAuditEventKind::SubjectDisabled,
            AccessAuditEventKind::SubjectDeleted,
            AccessAuditEventKind::SubjectRecoveryPending,
            AccessAuditEventKind::SubjectReactivated,
            AccessAuditEventKind::MembershipInvited,
            AccessAuditEventKind::MembershipApprovalRequested,
            AccessAuditEventKind::MembershipInvitationAccepted,
            AccessAuditEventKind::MembershipApproved,
            AccessAuditEventKind::MembershipSuspended,
            AccessAuditEventKind::MembershipResumed,
            AccessAuditEventKind::MembershipRemoved,
            AccessAuditEventKind::MembershipPromoted,
            AccessAuditEventKind::MembershipDemoted,
            AccessAuditEventKind::CeremonyClaimed,
            AccessAuditEventKind::CeremonyCompleted,
            AccessAuditEventKind::CeremonyCancelled,
            AccessAuditEventKind::CeremonyExpired,
            AccessAuditEventKind::CeremonyCleanupUncertain,
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
            membership.apply_lifecycle_action(
                &mut subject,
                MembershipLifecycleAction::Suspend,
                1,
                at(1),
            ),
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
        assert_eq!(
            zero_count_membership
                .change_role(&mut subject, WorkspaceRole::Member, 0, at(1))
                .expect("zero does not claim a final administrator exists"),
            Some(AccessInvalidationEffect::MembershipAuthorizationChanged)
        );

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
        assert_eq!(
            membership
                .change_role(&mut subject, WorkspaceRole::Member, 0, at(3))
                .expect("demote inactive subject"),
            Some(AccessInvalidationEffect::MembershipAuthorizationChanged)
        );
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
            .apply_lifecycle_action(&mut subject, MembershipLifecycleAction::Remove, 0, at(1))
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
            invited.apply_lifecycle_action(
                &mut subject,
                MembershipLifecycleAction::Suspend,
                0,
                at(1),
            ),
            Err(AccessInvariantError::InvalidMembershipTransition)
        );

        let mut other_subject = active_subject();
        assert_eq!(
            invited.change_role(&mut other_subject, WorkspaceRole::Administrator, 0, at(1),),
            Err(AccessInvariantError::MembershipSubjectMismatch)
        );

        invited
            .apply_lifecycle_action(&mut subject, MembershipLifecycleAction::Remove, 0, at(2))
            .expect("remove");
        assert_eq!(
            invited.change_role(&mut subject, WorkspaceRole::Administrator, 0, at(3)),
            Err(AccessInvariantError::RemovedMembershipIsTerminal)
        );
        assert_eq!(
            invited.apply_lifecycle_action(
                &mut subject,
                MembershipLifecycleAction::AcceptInvitation,
                0,
                at(3),
            ),
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
            ceremony_start_selection(AuthCeremonyPurpose::SignIn),
            false,
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
        let subject = active_subject();
        ceremony
            .require_selection(
                AuthCeremonyConfirmation::new(
                    subject,
                    AuthenticationProvenance::new(
                        AuthenticationMethod::TrailBasePassword,
                        at(1),
                        7,
                    ),
                ),
                at(2),
            )
            .expect("require selection");
        assert!(!ceremony
            .recover_after_restart(at(2))
            .expect("selection survives restart"));
        let confirmation = ceremony.confirmation();
        let mut cancelled_selection = ceremony.clone();
        cancelled_selection
            .cancel(at(3))
            .expect("cancel selected sign-in");
        assert_eq!(cancelled_selection.confirmation(), confirmation);
        let mut expired_selection = ceremony.clone();
        assert!(expired_selection
            .expire(at(30))
            .expect("expire selected sign-in at boundary"));
        assert_eq!(expired_selection.confirmation(), confirmation);
        assert_eq!(
            ceremony
                .clone()
                .complete_selection(ceremony_selection(AuthCeremonyPurpose::SignIn), at(30)),
            Err(AccessInvariantError::CeremonyExpired)
        );
        ceremony
            .complete_selection(ceremony_selection(AuthCeremonyPurpose::SignIn), at(3))
            .expect("complete selected sign-in");
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
            ceremony_start_selection(AuthCeremonyPurpose::RecentAuthentication),
            false,
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
            ceremony_start_selection(AuthCeremonyPurpose::FirstAdministratorBootstrap),
            false,
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
            ceremony_start_selection(AuthCeremonyPurpose::SignIn),
            false,
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
            ceremony_start_selection(AuthCeremonyPurpose::SignIn),
            false,
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
    fn ceremony_expiry_is_explicit_boundary_safe_and_idempotent() {
        let mut ceremony = AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::SignIn,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            TrailBaseInstanceId::new_v7(),
            1,
            Sha256Digest::from_bytes(&[2; 32]),
            ceremony_start_selection(AuthCeremonyPurpose::SignIn),
            false,
            AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback"),
            AuthReturnTarget::ApplicationHome,
            RequestCorrelationId::new_v7(),
            at(1),
            at(2),
        )
        .expect("ceremony");

        assert_eq!(
            ceremony.expire(at(0)),
            Err(AccessInvariantError::InvalidTimestampOrder)
        );
        assert!(!ceremony.expire(at(1)).expect("not yet expired"));
        assert!(ceremony.expire(at(2)).expect("expire at boundary"));
        assert_eq!(ceremony.state(), AuthCeremonyState::Expired);
        assert_eq!(ceremony.terminal_at(), Some(at(2)));
        assert!(!ceremony.expire(at(3)).expect("terminal expiry is stable"));
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
                ceremony_start_selection(AuthCeremonyPurpose::SignIn),
                false,
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
                ceremony_start_selection(purpose),
                false,
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
                            ceremony_start_selection(purpose),
                            false,
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
    fn ceremony_selection_binds_invitations_only_to_sign_in() {
        let workspace_id = WorkspaceId::new_v7();
        let grant_id = ProfileGrantId::new_v7();
        let invitation_id = MembershipId::new_v7();

        assert!(AuthCeremonySelection::try_new(
            AuthCeremonyPurpose::SignIn,
            workspace_id,
            grant_id,
            None,
            Some(invitation_id),
        )
        .is_ok());
        assert_eq!(
            AuthCeremonySelection::try_new(
                AuthCeremonyPurpose::RecentAuthentication,
                workspace_id,
                grant_id,
                Some(BrowserSessionId::new_v7()),
                Some(invitation_id),
            ),
            Err(AccessInvariantError::InvalidCeremonySelectionBinding)
        );
        assert_eq!(
            AuthCeremonySelection::try_new(
                AuthCeremonyPurpose::FirstAdministratorBootstrap,
                workspace_id,
                grant_id,
                None,
                Some(invitation_id),
            ),
            Err(AccessInvariantError::InvalidCeremonySelectionBinding)
        );
    }

    #[test]
    fn ceremony_persistence_rejects_impossible_state_and_failure_combinations() {
        let instance_id = TrailBaseInstanceId::new_v7();
        let binding = Sha256Digest::from_bytes(&[6; 32]);
        let callback = AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback");
        let confirmation = AuthCeremonyConfirmation::new(
            active_subject(),
            AuthenticationProvenance::new(AuthenticationMethod::TrailBasePassword, at(1), 2),
        );
        let restore = |state, failure, claimed_at, terminal_at| {
            let (selection, confirmation) = match state {
                AuthCeremonyState::SelectionRequired => (None, Some(confirmation)),
                AuthCeremonyState::Completed => (
                    Some(ceremony_selection(AuthCeremonyPurpose::SignIn)),
                    Some(confirmation),
                ),
                _ => (None, None),
            };
            AuthCeremony::try_from_persisted(
                OperationId::new_v7(),
                AuthCeremonyPurpose::SignIn,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                instance_id,
                2,
                binding.clone(),
                selection,
                false,
                confirmation,
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
        assert!(restore(
            AuthCeremonyState::SelectionRequired,
            None,
            Some(at(1)),
            None,
        )
        .is_ok());
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
                ceremony_start_selection(AuthCeremonyPurpose::SignIn),
                false,
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
            ceremony_start_selection(AuthCeremonyPurpose::SignIn),
            false,
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
