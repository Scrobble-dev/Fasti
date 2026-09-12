//! Permanent first-link eligibility; this state grants no linking authority.

use crate::{AuthSubjectId, OperationId, TrailBaseInstanceId};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FirstOidcLinkError {
    #[error("the subject is not the installation's original administrator")]
    SubjectMismatch,
    #[error("the installation's first OIDC link eligibility is already consumed")]
    AlreadyConsumed,
}

/// Installation-owned history, independent of activation generations and links.
///
/// The application must prove the original bootstrap identity and enforce current
/// authorization. Store this record permanently; pruned audits, unlinking and
/// restore cannot create eligibility. Concurrent link transactions must serialize
/// consumption with the unique link and its audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstOidcLinkEligibility {
    trailbase_instance_id: TrailBaseInstanceId,
    original_administrator_subject_id: AuthSubjectId,
    consumed_by: Option<OperationId>,
}

impl FirstOidcLinkEligibility {
    /// Requires unambiguous original bootstrap evidence validated against its
    /// anchor. Current roles, timestamps or ID order are not such evidence.
    /// The caller must insert once per installation, never replace a record.
    pub const fn from_original_bootstrap(
        trailbase_instance_id: TrailBaseInstanceId,
        original_administrator_subject_id: AuthSubjectId,
    ) -> Self {
        Self::from_persisted(
            trailbase_instance_id,
            original_administrator_subject_id,
            None,
        )
    }

    /// Reconstruct an existing authoritative record with its exact consumption.
    /// Missing history is unavailable, not permission to synthesize `None`.
    pub const fn from_persisted(
        trailbase_instance_id: TrailBaseInstanceId,
        original_administrator_subject_id: AuthSubjectId,
        consumed_by: Option<OperationId>,
    ) -> Self {
        Self {
            trailbase_instance_id,
            original_administrator_subject_id,
            consumed_by,
        }
    }

    pub const fn trailbase_instance_id(&self) -> TrailBaseInstanceId {
        self.trailbase_instance_id
    }

    pub const fn original_administrator_subject_id(&self) -> AuthSubjectId {
        self.original_administrator_subject_id
    }

    pub const fn consumed_by(&self) -> Option<OperationId> {
        self.consumed_by
    }

    /// Record the first successful OIDC link, whatever authorized that link.
    /// This transition neither proves authentication nor issues an approval.
    pub fn consume(
        &mut self,
        subject_id: AuthSubjectId,
        operation_id: OperationId,
    ) -> Result<(), FirstOidcLinkError> {
        if subject_id != self.original_administrator_subject_id {
            return Err(FirstOidcLinkError::SubjectMismatch);
        }
        if self.consumed_by.is_some() {
            return Err(FirstOidcLinkError::AlreadyConsumed);
        }
        self.consumed_by = Some(operation_id);
        Ok(())
    }
}
