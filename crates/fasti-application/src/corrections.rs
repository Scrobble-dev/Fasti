//! B3 correction commands, queries, views, and persistence port.
//!
//! Corrections append a new interpretation. They never rewrite the original
//! observation, evidence, or occurrence.

use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{
    CorrectionId, InterpretationId, ObservationId, RecordId, RequestCorrelationId,
};

pub const MAX_CORRECTION_REASON_BYTES: usize = 1024;
pub const MAX_CORRECTION_CHAIN_PAGE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionTarget {
    Unresolved,
    Record(RecordId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendCorrectionCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    observation_id: ObservationId,
    target: CorrectionTarget,
    reason: String,
}

impl AppendCorrectionCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        observation_id: ObservationId,
        target: CorrectionTarget,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            observation_id,
            target,
            reason: reason.into(),
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn observation_id(&self) -> ObservationId {
        self.observation_id
    }

    pub const fn target(&self) -> CorrectionTarget {
        self.target
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectCorrectionChainQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    observation_id: ObservationId,
}

impl InspectCorrectionChainQuery {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        observation_id: ObservationId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            observation_id,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn observation_id(&self) -> ObservationId {
        self.observation_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionEntryView {
    correction_id: CorrectionId,
    prior_interpretation_id: InterpretationId,
    replacement_interpretation_id: InterpretationId,
    record_id: Option<RecordId>,
    reason: String,
}

impl CorrectionEntryView {
    pub fn new(
        correction_id: CorrectionId,
        prior_interpretation_id: InterpretationId,
        replacement_interpretation_id: InterpretationId,
        record_id: Option<RecordId>,
        reason: String,
    ) -> Self {
        Self {
            correction_id,
            prior_interpretation_id,
            replacement_interpretation_id,
            record_id,
            reason,
        }
    }

    pub const fn correction_id(&self) -> CorrectionId {
        self.correction_id
    }

    pub const fn prior_interpretation_id(&self) -> InterpretationId {
        self.prior_interpretation_id
    }

    pub const fn replacement_interpretation_id(&self) -> InterpretationId {
        self.replacement_interpretation_id
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendCorrectionOutcome {
    correction_id: CorrectionId,
    prior_interpretation_id: InterpretationId,
    replacement_interpretation_id: InterpretationId,
    record_id: Option<RecordId>,
}

impl AppendCorrectionOutcome {
    pub const fn new(
        correction_id: CorrectionId,
        prior_interpretation_id: InterpretationId,
        replacement_interpretation_id: InterpretationId,
        record_id: Option<RecordId>,
    ) -> Self {
        Self {
            correction_id,
            prior_interpretation_id,
            replacement_interpretation_id,
            record_id,
        }
    }

    pub const fn correction_id(&self) -> CorrectionId {
        self.correction_id
    }

    pub const fn prior_interpretation_id(&self) -> InterpretationId {
        self.prior_interpretation_id
    }

    pub const fn replacement_interpretation_id(&self) -> InterpretationId {
        self.replacement_interpretation_id
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionChainView {
    observation_id: ObservationId,
    initial_interpretation_id: InterpretationId,
    current_interpretation_id: InterpretationId,
    corrections: Vec<CorrectionEntryView>,
    truncated: bool,
}

impl CorrectionChainView {
    pub fn new(
        observation_id: ObservationId,
        initial_interpretation_id: InterpretationId,
        current_interpretation_id: InterpretationId,
        corrections: Vec<CorrectionEntryView>,
        truncated: bool,
    ) -> Self {
        debug_assert!(corrections.len() <= MAX_CORRECTION_CHAIN_PAGE);
        Self {
            observation_id,
            initial_interpretation_id,
            current_interpretation_id,
            corrections,
            truncated,
        }
    }

    pub const fn observation_id(&self) -> ObservationId {
        self.observation_id
    }

    pub const fn initial_interpretation_id(&self) -> InterpretationId {
        self.initial_interpretation_id
    }

    pub const fn current_interpretation_id(&self) -> InterpretationId {
        self.current_interpretation_id
    }

    pub fn corrections(&self) -> &[CorrectionEntryView] {
        &self.corrections
    }

    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Atomic B3 correction boundary.
///
/// Implementations must authorize inside the transaction, append exactly one
/// interpretation from the current chain leaf, and preserve original evidence
/// and occurrence rows.
pub trait CorrectionPort: Send + Sync {
    fn append_correction(
        &self,
        command: AppendCorrectionCommand,
    ) -> ApplicationResult<AppendCorrectionOutcome>;

    fn inspect_correction_chain(
        &self,
        query: InspectCorrectionChainQuery,
    ) -> ApplicationResult<CorrectionChainView>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId, WorkspaceId};

    fn access() -> RequestAccessContext {
        RequestAccessContext::new(
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        )
    }

    #[test]
    fn correction_command_derives_actor_from_access_context() {
        let access = access();
        let command = AppendCorrectionCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            ObservationId::new_v7(),
            CorrectionTarget::Unresolved,
            "Remove an incorrect interpretation",
        );

        assert_eq!(command.access().client_id(), access.client_id());
        assert_eq!(command.target(), CorrectionTarget::Unresolved);
    }
}
