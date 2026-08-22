//! Capability-bound receipt semantics.
//!
//! A receipt represents a completed durability boundary. Production
//! persistence must not issue one before its database and evidence obligations
//! hold.

use crate::CapabilityKey;
use fasti_domain::{
    ClientId, CommittedAt, EvidenceId, InterpretationId, Observation, ObservationId,
    ObservationResolution, OccurrenceId, OperationId, ProfileId, ReceiptId, ReceivedAt, RecordId,
    ReviewItemId, Sha256Digest, WorkspaceId,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    occurrence_id: Option<OccurrenceId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interpretation_id: Option<InterpretationId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_id: Option<RecordId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_item_id: Option<ReviewItemId>,
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
        Self::from_committed(
            receipt_id,
            operation_id,
            observation,
            None,
            None,
            None,
            None,
            ObservationResolution::Unresolved,
            committed_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_committed(
        receipt_id: ReceiptId,
        operation_id: OperationId,
        observation: &Observation,
        occurrence_id: Option<OccurrenceId>,
        interpretation_id: Option<InterpretationId>,
        record_id: Option<RecordId>,
        review_item_id: Option<ReviewItemId>,
        resolution: ObservationResolution,
        committed_at: CommittedAt,
    ) -> Result<Self, ReceiptBuildError> {
        let received_at = observation.received_at();
        if committed_at.value() < received_at.value() {
            return Err(ReceiptBuildError::CommitBeforeReceive);
        }
        match resolution {
            ObservationResolution::Resolved if record_id.is_none() => {
                return Err(ReceiptBuildError::ResolvedWithoutRecord)
            }
            ObservationResolution::Conflicted if review_item_id.is_none() => {
                return Err(ReceiptBuildError::ConflictWithoutReview)
            }
            _ => {}
        }
        Ok(Self {
            receipt_id,
            operation_id,
            workspace_id: observation.workspace_id(),
            profile_id: observation.profile_id(),
            source_client_id: observation.source_client_id(),
            observation_id: observation.observation_id(),
            occurrence_id,
            interpretation_id,
            record_id,
            review_item_id,
            evidence_id: observation.evidence().evidence_id(),
            payload_digest: observation.evidence().digest().clone(),
            resolution,
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

    pub const fn occurrence_id(&self) -> Option<OccurrenceId> {
        self.occurrence_id
    }

    pub const fn interpretation_id(&self) -> Option<InterpretationId> {
        self.interpretation_id
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }

    pub const fn review_item_id(&self) -> Option<ReviewItemId> {
        self.review_item_id
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
    ResolvedWithoutRecord,
    ConflictWithoutReview,
}

impl fmt::Display for ReceiptBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CommitBeforeReceive => "committed_at cannot precede received_at",
            Self::ResolvedWithoutRecord => "resolved receipt requires a record",
            Self::ConflictWithoutReview => "conflicted receipt requires review work",
        })
    }
}

impl Error for ReceiptBuildError {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};
    use fasti_domain::{ClaimedTrust, EvidenceReference, ObservedAt};

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
    fn resolved_and_conflicted_receipts_require_their_governed_targets() {
        let received = ReceivedAt::from_application_clock(Utc::now());
        let observation = observation(received);
        let committed_at = CommittedAt::from_durability_boundary(Utc::now());
        assert_eq!(
            AcceptObservationReceipt::from_committed(
                ReceiptId::new_v7(),
                OperationId::new_v7(),
                &observation,
                Some(OccurrenceId::new_v7()),
                Some(InterpretationId::new_v7()),
                None,
                None,
                ObservationResolution::Resolved,
                committed_at,
            ),
            Err(ReceiptBuildError::ResolvedWithoutRecord)
        );
        assert_eq!(
            AcceptObservationReceipt::from_committed(
                ReceiptId::new_v7(),
                OperationId::new_v7(),
                &observation,
                Some(OccurrenceId::new_v7()),
                Some(InterpretationId::new_v7()),
                None,
                None,
                ObservationResolution::Conflicted,
                committed_at,
            ),
            Err(ReceiptBuildError::ConflictWithoutReview)
        );
    }
}
