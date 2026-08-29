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
}
