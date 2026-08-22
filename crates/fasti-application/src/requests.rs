//! Authenticated semantic commands and queries.
//!
//! Transport DTOs map into these values. Server timestamps, source client
//! identity, commit state, and receipt outcomes are never request fields.

use crate::RequestAccessContext;
use fasti_domain::{
    EvidenceReference, ExternalIdentifierClaim, Grain, ObservedAt, OccurredAt, OperationId,
    ReceiptId, RequestCorrelationId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptObservationCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    operation_id: OperationId,
    occurred_at: Option<OccurredAt>,
    observed_at: ObservedAt,
    prepared_evidence: EvidenceReference,
    identity_clues: Vec<ExternalIdentifierClaim>,
    target_grain: Option<Grain>,
}

impl AcceptObservationCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        operation_id: OperationId,
        occurred_at: Option<OccurredAt>,
        observed_at: ObservedAt,
        prepared_evidence: EvidenceReference,
    ) -> Self {
        Self {
            correlation_id,
            access,
            operation_id,
            occurred_at,
            observed_at,
            prepared_evidence,
            identity_clues: Vec::new(),
            target_grain: None,
        }
    }

    pub fn with_identity_clues(
        mut self,
        identity_clues: Vec<ExternalIdentifierClaim>,
        target_grain: Option<Grain>,
    ) -> Self {
        self.identity_clues = identity_clues;
        self.target_grain = target_grain;
        self
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    pub const fn occurred_at(&self) -> Option<&OccurredAt> {
        self.occurred_at.as_ref()
    }

    pub const fn observed_at(&self) -> &ObservedAt {
        &self.observed_at
    }

    pub const fn prepared_evidence(&self) -> &EvidenceReference {
        &self.prepared_evidence
    }

    pub fn identity_clues(&self) -> &[ExternalIdentifierClaim] {
        &self.identity_clues
    }

    pub const fn target_grain(&self) -> Option<Grain> {
        self.target_grain
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayReceiptQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    receipt_id: ReceiptId,
}

impl ReplayReceiptQuery {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        receipt_id: ReceiptId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            receipt_id,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn receipt_id(&self) -> ReceiptId {
        self.receipt_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RequestAccessContext;
    use fasti_domain::{
        ClaimedTrust, ClientId, CredentialId, EvidenceId, ProfileGrantId, ProfileId, Sha256Digest,
        WorkspaceId,
    };

    fn access() -> RequestAccessContext {
        RequestAccessContext::new(
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            3,
        )
    }

    #[test]
    fn observation_command_derives_source_identity_from_access_context() {
        let access = access();
        let evidence = EvidenceReference::new(
            EvidenceId::new_v7(),
            Sha256Digest::parse(format!("sha256:{}", "ab".repeat(32))).expect("canonical digest"),
            12,
        );
        let command = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            OperationId::new_v7(),
            None,
            ObservedAt::parse("2026-08-21T17:44:15Z", ClaimedTrust::DeviceObserved)
                .expect("valid observed_at"),
            evidence.clone(),
        );

        assert_eq!(command.access().client_id(), access.client_id());
        assert_eq!(command.prepared_evidence(), &evidence);
        assert!(command.identity_clues().is_empty());
    }
}
