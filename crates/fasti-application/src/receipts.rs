//! Capability-bound receipt semantics.
//!
//! A receipt represents a completed durability boundary. B1 exercises this
//! model only through a fixture-only adapter; production persistence arrives
//! in B2 and must not issue one before its commit and flush obligations hold.

use crate::CapabilityKey;
use fasti_domain::{
    ClientId, CommittedAt, EvidenceId, Observation, ObservationId, ObservationResolution,
    OperationId, ProfileId, ReceiptId, ReceivedAt, Sha256Digest, WorkspaceId,
};
use serde::Serialize;
use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AcceptObservationReceipt {
    receipt_id: ReceiptId,
    operation_id: OperationId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    source_client_id: ClientId,
    observation_id: ObservationId,
    evidence_id: EvidenceId,
    payload_digest: Sha256Digest,
    resolution: ObservationResolution,
    received_at: ReceivedAt,
    committed_at: CommittedAt,
}

impl AcceptObservationReceipt {
    pub fn try_from_observation(
        receipt_id: ReceiptId,
        operation_id: OperationId,
        observation: &Observation,
        committed_at: CommittedAt,
    ) -> Result<Self, ReceiptBuildError> {
        let received_at = observation.received_at();
        if committed_at.value() < received_at.value() {
            return Err(ReceiptBuildError::CommitBeforeReceive);
        }
        Ok(Self {
            receipt_id,
            operation_id,
            workspace_id: observation.workspace_id(),
            profile_id: observation.profile_id(),
            source_client_id: observation.source_client_id(),
            observation_id: observation.observation_id(),
            evidence_id: observation.evidence().evidence_id(),
            payload_digest: observation.evidence().digest().clone(),
            resolution: ObservationResolution::Unresolved,
            received_at,
            committed_at,
        })
    }

    pub const fn capability(&self) -> CapabilityKey {
        CapabilityKey::AcceptObservation
    }

    pub const fn receipt_id(&self) -> ReceiptId {
        self.receipt_id
    }

    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn source_client_id(&self) -> ClientId {
        self.source_client_id
    }

    pub const fn observation_id(&self) -> ObservationId {
        self.observation_id
    }

    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    pub fn payload_digest(&self) -> &Sha256Digest {
        &self.payload_digest
    }

    pub const fn resolution(&self) -> ObservationResolution {
        self.resolution
    }

    pub const fn received_at(&self) -> ReceivedAt {
        self.received_at
    }

    pub const fn committed_at(&self) -> CommittedAt {
        self.committed_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "disposition", content = "receipt")]
pub enum AcceptObservationOutcome {
    Committed(AcceptObservationReceipt),
    Replayed(AcceptObservationReceipt),
}

impl AcceptObservationOutcome {
    pub fn receipt(&self) -> &AcceptObservationReceipt {
        match self {
            Self::Committed(receipt) | Self::Replayed(receipt) => receipt,
        }
    }

    pub const fn is_replay(&self) -> bool {
        matches!(self, Self::Replayed(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptBuildError {
    CommitBeforeReceive,
}

impl fmt::Display for ReceiptBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("committed_at cannot precede received_at")
    }
}

impl Error for ReceiptBuildError {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};
    use fasti_domain::{ClaimedTrust, EvidenceReference, ObservedAt, Sha256Digest};

    fn observation(received_at: ReceivedAt) -> Observation {
        let (observation, _) = Observation::new_unresolved(
            ObservationId::new_v7(),
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            EvidenceReference::new(
                EvidenceId::new_v7(),
                Sha256Digest::parse(format!("sha256:{}", "42".repeat(32)))
                    .expect("canonical digest"),
                42,
            ),
            None,
            ObservedAt::parse("2026-08-21T17:44:15Z", ClaimedTrust::DeviceObserved)
                .expect("valid observed_at"),
            received_at,
        );
        observation
    }

    #[test]
    fn receipt_rejects_commit_before_receive() {
        let received = Utc
            .with_ymd_and_hms(2026, 8, 21, 17, 44, 16)
            .single()
            .expect("valid instant");
        let observation = observation(ReceivedAt::from_application_clock(received));
        assert_eq!(
            AcceptObservationReceipt::try_from_observation(
                ReceiptId::new_v7(),
                OperationId::new_v7(),
                &observation,
                CommittedAt::from_durability_boundary(received - Duration::milliseconds(1)),
            ),
            Err(ReceiptBuildError::CommitBeforeReceive)
        );
    }

    #[test]
    fn replay_reuses_the_exact_receipt_without_inventing_identity() {
        let received = Utc
            .with_ymd_and_hms(2026, 8, 21, 17, 44, 16)
            .single()
            .expect("valid instant");
        let observation = observation(ReceivedAt::from_application_clock(received));
        let receipt = AcceptObservationReceipt::try_from_observation(
            ReceiptId::new_v7(),
            OperationId::new_v7(),
            &observation,
            CommittedAt::from_durability_boundary(received + Duration::milliseconds(1)),
        )
        .expect("valid receipt");

        let committed = AcceptObservationOutcome::Committed(receipt.clone());
        let replayed = AcceptObservationOutcome::Replayed(receipt);
        assert_eq!(committed.receipt(), replayed.receipt());
        assert!(!committed.is_replay());
        assert!(replayed.is_replay());
        assert_eq!(
            replayed.receipt().resolution(),
            ObservationResolution::Unresolved
        );

        let value = serde_json::to_value(replayed.receipt()).expect("serialize receipt");
        assert!(value.get("record_id").is_none());
        assert!(value.get("occurrence_id").is_none());
    }
}
