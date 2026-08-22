use crate::{
    InterpretationId, ObservationId, OccurredAt, OccurrenceId, ProfileId, RecordId, WorkspaceId,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationState {
    Unresolved,
    Resolved,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Occurrence {
    occurrence_id: OccurrenceId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    observation_id: ObservationId,
    record_id: Option<RecordId>,
    occurred_at: Option<OccurredAt>,
}

impl Occurrence {
    pub const fn new(
        occurrence_id: OccurrenceId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        observation_id: ObservationId,
        record_id: Option<RecordId>,
        occurred_at: Option<OccurredAt>,
    ) -> Self {
        Self {
            occurrence_id,
            workspace_id,
            profile_id,
            observation_id,
            record_id,
            occurred_at,
        }
    }

    pub const fn occurrence_id(&self) -> OccurrenceId {
        self.occurrence_id
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn observation_id(&self) -> ObservationId {
        self.observation_id
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }

    pub const fn occurred_at(&self) -> Option<&OccurredAt> {
        self.occurred_at.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interpretation {
    interpretation_id: InterpretationId,
    observation_id: ObservationId,
    occurrence_id: OccurrenceId,
    prior_interpretation_id: Option<InterpretationId>,
    record_id: Option<RecordId>,
    state: InterpretationState,
}

impl Interpretation {
    pub const fn new(
        interpretation_id: InterpretationId,
        observation_id: ObservationId,
        occurrence_id: OccurrenceId,
        prior_interpretation_id: Option<InterpretationId>,
        record_id: Option<RecordId>,
        state: InterpretationState,
    ) -> Self {
        Self {
            interpretation_id,
            observation_id,
            occurrence_id,
            prior_interpretation_id,
            record_id,
            state,
        }
    }

    pub const fn interpretation_id(&self) -> InterpretationId {
        self.interpretation_id
    }

    pub const fn observation_id(&self) -> ObservationId {
        self.observation_id
    }

    pub const fn occurrence_id(&self) -> OccurrenceId {
        self.occurrence_id
    }

    pub const fn prior_interpretation_id(&self) -> Option<InterpretationId> {
        self.prior_interpretation_id
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }

    pub const fn state(&self) -> InterpretationState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_occurrence_does_not_invent_a_record() {
        let occurrence = Occurrence::new(
            OccurrenceId::new_v7(),
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ObservationId::new_v7(),
            None,
            None,
        );
        let interpretation = Interpretation::new(
            InterpretationId::new_v7(),
            occurrence.observation_id(),
            occurrence.occurrence_id(),
            None,
            None,
            InterpretationState::Unresolved,
        );
        assert_eq!(occurrence.record_id(), None);
        assert_eq!(interpretation.record_id(), None);
        assert_eq!(interpretation.state(), InterpretationState::Unresolved);
    }
}
