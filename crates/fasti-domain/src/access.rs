use crate::{AuthSubjectId, BrowserSessionId, ProfileGrantId, WorkspaceId};
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AccessInvariantError {
    #[error("Access timestamps are not monotonic")]
    InvalidTimestampOrder,
    #[error("a deleted authentication subject is terminal")]
    DeletedSubjectIsTerminal,
    #[error("the Access epoch cannot advance")]
    EpochOverflow,
    #[error("the membership transition is not allowed")]
    InvalidMembershipTransition,
    #[error("a removed membership is terminal")]
    RemovedMembershipIsTerminal,
    #[error("the final viable administrator cannot be removed")]
    FinalAdministratorRequired,
    #[error("the membership does not belong to this authentication subject")]
    MembershipSubjectMismatch,
    #[error("the authentication ceremony transition is not allowed")]
    InvalidCeremonyTransition,
    #[error("the authentication proof expiry must follow verification")]
    InvalidAuthenticationProofExpiry,
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
        self.auth_epoch = self
            .auth_epoch
            .checked_add(1)
            .ok_or(AccessInvariantError::EpochOverflow)?;
        self.lifecycle = lifecycle;
        self.updated_at = at;
        Ok(true)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipLifecycle {
    Invited,
    PendingApproval,
    Active,
    Suspended,
    Removed,
}

impl MembershipLifecycle {
    pub const fn grants_access(self) -> bool {
        matches!(self, Self::Active)
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
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    lifecycle: MembershipLifecycle,
    role: WorkspaceRole,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl WorkspaceMembership {
    pub fn try_new(
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
        Ok(Self {
            subject_id,
            workspace_id,
            lifecycle,
            role,
            created_at,
            updated_at,
        })
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
    pub const fn is_authorization_viable_administrator(&self) -> bool {
        self.lifecycle.grants_access() && matches!(self.role, WorkspaceRole::Administrator)
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
        self.ensure_administrator_continuity(lifecycle, self.role, viable_administrator_count)?;
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
        self.ensure_administrator_continuity(self.lifecycle, role, viable_administrator_count)?;
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

    fn ensure_administrator_continuity(
        &self,
        lifecycle: MembershipLifecycle,
        role: WorkspaceRole,
        viable_administrator_count: u64,
    ) -> Result<(), AccessInvariantError> {
        let remains_viable =
            lifecycle.grants_access() && matches!(role, WorkspaceRole::Administrator);
        if self.is_authorization_viable_administrator()
            && !remains_viable
            && viable_administrator_count == 1
        {
            return Err(AccessInvariantError::FinalAdministratorRequired);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthCeremonyState {
    Pending,
    Claimed,
    Completed,
    Failed,
    CleanupUncertain,
    Expired,
}

impl AuthCeremonyState {
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::CleanupUncertain | Self::Expired
        )
    }

    pub fn claim(&mut self) -> Result<(), AccessInvariantError> {
        if !matches!(self, Self::Pending) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        *self = Self::Claimed;
        Ok(())
    }

    pub fn complete(&mut self) -> Result<(), AccessInvariantError> {
        if !matches!(self, Self::Claimed) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        *self = Self::Completed;
        Ok(())
    }

    pub fn fail(&mut self) -> Result<(), AccessInvariantError> {
        if self.is_terminal() {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        *self = Self::Failed;
        Ok(())
    }

    pub fn mark_cleanup_uncertain(&mut self) -> Result<(), AccessInvariantError> {
        if !matches!(self, Self::Claimed) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        *self = Self::CleanupUncertain;
        Ok(())
    }

    pub fn expire(&mut self) -> Result<(), AccessInvariantError> {
        if !matches!(self, Self::Pending) {
            return Err(AccessInvariantError::InvalidCeremonyTransition);
        }
        *self = Self::Expired;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationMethod {
    TrailBasePassword,
    TrailBasePasswordTotp,
    TrailBaseSocial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthenticationAssurance {
    SingleFactor,
    MultiFactor,
}

impl AuthenticationMethod {
    pub const fn assurance(self) -> AuthenticationAssurance {
        match self {
            Self::TrailBasePassword | Self::TrailBaseSocial => {
                AuthenticationAssurance::SingleFactor
            }
            Self::TrailBasePasswordTotp => AuthenticationAssurance::MultiFactor,
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
    ) -> Result<Self, AccessInvariantError> {
        if expires_at <= provenance.verified_at() {
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

    fn subject() -> AuthSubject {
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
        let mut subject = subject();
        assert!(subject
            .transition_lifecycle(AuthSubjectLifecycle::Disabled, at(1))
            .expect("disable"));
        assert_eq!(subject.auth_epoch(), 3);
        assert!(subject
            .transition_lifecycle(AuthSubjectLifecycle::Deleted, at(2))
            .expect("delete"));
        assert_eq!(
            subject.transition_lifecycle(AuthSubjectLifecycle::Active, at(3)),
            Err(AccessInvariantError::DeletedSubjectIsTerminal)
        );
    }

    #[test]
    fn session_rejects_invalid_time_order_and_distinguishes_terminal_states() {
        let subject = subject();
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
    }

    #[test]
    fn membership_changes_advance_authorization_epoch() {
        let mut subject = subject();
        let mut membership = membership(&subject, WorkspaceRole::Member);

        assert!(membership
            .change_role(&mut subject, WorkspaceRole::Administrator, 0, at(1))
            .expect("promote"));
        assert_eq!(subject.authorization_epoch(), 4);
        assert!(membership.is_authorization_viable_administrator());

        assert!(membership
            .transition_lifecycle(&mut subject, MembershipLifecycle::Suspended, 2, at(2))
            .expect("suspend"));
        assert_eq!(subject.authorization_epoch(), 5);
        assert!(!membership.is_authorization_viable_administrator());
    }

    #[test]
    fn final_viable_administrator_is_preserved_without_partial_mutation() {
        let mut subject = subject();
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
    }

    #[test]
    fn ceremony_can_be_claimed_and_completed_only_once() {
        let mut ceremony = AuthCeremonyState::Pending;
        ceremony.claim().expect("claim");
        assert_eq!(
            ceremony.claim(),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
        ceremony.complete().expect("complete");
        assert!(ceremony.is_terminal());
        assert_eq!(
            ceremony.fail(),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );

        let mut cleanup_failure = AuthCeremonyState::Claimed;
        cleanup_failure
            .mark_cleanup_uncertain()
            .expect("terminal failure");
        assert_eq!(cleanup_failure, AuthCeremonyState::CleanupUncertain);
        assert!(cleanup_failure.is_terminal());

        let mut never_exchanged = AuthCeremonyState::Pending;
        assert_eq!(
            never_exchanged.mark_cleanup_uncertain(),
            Err(AccessInvariantError::InvalidCeremonyTransition)
        );
    }

    #[test]
    fn recent_auth_requires_current_subject_generation_epoch_and_assurance() {
        let mut subject = subject();
        let social = AuthenticationProvenance::new(AuthenticationMethod::TrailBaseSocial, at(1), 7);
        let recent =
            RecentAuthentication::try_new(subject.id(), social, subject.auth_epoch(), at(11))
                .expect("recent authentication");

        assert!(recent.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(2),));
        assert!(!recent.satisfies(&subject, 7, AuthenticationAssurance::MultiFactor, at(2),));
        assert!(!recent.satisfies(&subject, 8, AuthenticationAssurance::SingleFactor, at(2),));
        assert!(!recent.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(11),));

        subject
            .transition_lifecycle(AuthSubjectLifecycle::Disabled, at(3))
            .expect("disable");
        assert!(!recent.satisfies(&subject, 7, AuthenticationAssurance::SingleFactor, at(4),));

        let password_totp =
            AuthenticationProvenance::new(AuthenticationMethod::TrailBasePasswordTotp, at(5), 7);
        assert_eq!(
            password_totp.assurance(),
            AuthenticationAssurance::MultiFactor
        );
    }
}
