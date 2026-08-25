use crate::{InterpretationId, ObservationId, ProfileId, ReviewItemId, WorkspaceId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Open,
    Deferred,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReviewTransitionError {
    #[error("resolved review work cannot be reopened by this operation")]
    AlreadyResolved,
    #[error("review work is not deferred")]
    NotDeferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItem {
    review_item_id: ReviewItemId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    observation_id: ObservationId,
    current_interpretation_id: InterpretationId,
    status: ReviewStatus,
}

impl ReviewItem {
    pub const fn new(
        review_item_id: ReviewItemId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        observation_id: ObservationId,
        current_interpretation_id: InterpretationId,
    ) -> Self {
        Self {
            review_item_id,
            workspace_id,
            profile_id,
            observation_id,
            current_interpretation_id,
            status: ReviewStatus::Open,
        }
    }

    pub const fn review_item_id(&self) -> ReviewItemId {
        self.review_item_id
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

    pub const fn current_interpretation_id(&self) -> InterpretationId {
        self.current_interpretation_id
    }

    pub const fn status(&self) -> ReviewStatus {
        self.status
    }

    pub fn defer(&mut self) -> Result<(), ReviewTransitionError> {
        if self.status == ReviewStatus::Resolved {
            return Err(ReviewTransitionError::AlreadyResolved);
        }
        self.status = ReviewStatus::Deferred;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), ReviewTransitionError> {
        match self.status {
            ReviewStatus::Deferred => {
                self.status = ReviewStatus::Open;
                Ok(())
            }
            ReviewStatus::Resolved => Err(ReviewTransitionError::AlreadyResolved),
            ReviewStatus::Open => Err(ReviewTransitionError::NotDeferred),
        }
    }

    pub fn resolve(
        &mut self,
        replacement_interpretation_id: InterpretationId,
    ) -> Result<(), ReviewTransitionError> {
        if self.status == ReviewStatus::Resolved {
            return Err(ReviewTransitionError::AlreadyResolved);
        }
        self.current_interpretation_id = replacement_interpretation_id;
        self.status = ReviewStatus::Resolved;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review() -> ReviewItem {
        ReviewItem::new(
            ReviewItemId::new_v7(),
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ObservationId::new_v7(),
            InterpretationId::new_v7(),
        )
    }

    #[test]
    fn review_can_be_deferred_resumed_and_resolved() {
        let mut item = review();
        item.defer().expect("defer");
        assert_eq!(item.status(), ReviewStatus::Deferred);
        item.resume().expect("resume");
        assert_eq!(item.status(), ReviewStatus::Open);
        item.resolve(InterpretationId::new_v7()).expect("resolve");
        assert_eq!(item.status(), ReviewStatus::Resolved);
        assert_eq!(item.defer(), Err(ReviewTransitionError::AlreadyResolved));
    }
}
