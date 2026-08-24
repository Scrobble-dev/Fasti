//! B3 workspace integrity verification capability and adapter port.
//!
//! Verification reports bounded summary counts. It does not expose SQLite,
//! filesystem, transport, provider, or UI details to callers.

use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{RequestCorrelationId, WorkspaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyWorkspaceQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl VerifyWorkspaceQuery {
    pub const fn new(correlation_id: RequestCorrelationId, access: RequestAccessContext) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceVerificationOutcome {
    workspace_id: WorkspaceId,
    observations_verified: u64,
    evidence_verified: u64,
    corrections_verified: u64,
}

impl WorkspaceVerificationOutcome {
    pub const fn new(
        workspace_id: WorkspaceId,
        observations_verified: u64,
        evidence_verified: u64,
        corrections_verified: u64,
    ) -> Self {
        Self {
            workspace_id,
            observations_verified,
            evidence_verified,
            corrections_verified,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn observations_verified(&self) -> u64 {
        self.observations_verified
    }

    pub const fn evidence_verified(&self) -> u64 {
        self.evidence_verified
    }

    pub const fn corrections_verified(&self) -> u64 {
        self.corrections_verified
    }
}

/// Read-only B3 integrity-verification boundary.
///
/// Implementations must re-authorize against current durable state and verify
/// persisted Chronicle relations and evidence bytes before returning success.
pub trait WorkspaceVerificationPort: Send + Sync {
    fn verify_workspace(
        &self,
        query: VerifyWorkspaceQuery,
    ) -> ApplicationResult<WorkspaceVerificationOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId};

    #[test]
    fn verification_query_derives_workspace_from_access_context() {
        let workspace_id = WorkspaceId::new_v7();
        let access = RequestAccessContext::new(
            workspace_id,
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let query = VerifyWorkspaceQuery::new(RequestCorrelationId::new_v7(), access);

        assert_eq!(query.access().workspace_id(), workspace_id);
    }
}
