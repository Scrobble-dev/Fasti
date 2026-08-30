use crate::{
    AnimeGroupingRecordPreview, ApplicationResult, ProviderId, PurposeIdentityRoutePlan,
    RequestAccessContext,
};
use fasti_domain::{
    AnimeGroupingPreference, OperationId, ProfileId, RecordId, RequestCorrelationId,
    ResolutionIntent, Sha256Digest,
};
use std::num::NonZeroU16;

pub const MAX_IDENTITY_IMPACT_PAGE: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityImpactPageLimit(NonZeroU16);

impl IdentityImpactPageLimit {
    pub fn try_new(value: u16) -> Option<Self> {
        NonZeroU16::new(value)
            .filter(|value| value.get() <= MAX_IDENTITY_IMPACT_PAGE)
            .map(Self)
    }

    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveIdentityRouteQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    intent: ResolutionIntent,
    target_provider: ProviderId,
}

impl ResolveIdentityRouteQuery {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        intent: ResolutionIntent,
        target_provider: ProviderId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            intent,
            target_provider,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn intent(&self) -> ResolutionIntent {
        self.intent
    }

    pub const fn target_provider(&self) -> &ProviderId {
        &self.target_provider
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadAnimeGroupingPolicyQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl ReadAnimeGroupingPolicyQuery {
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
pub enum AnimeGroupingPolicyChange {
    Set(AnimeGroupingPreference),
    Rollback { applied_operation_id: OperationId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewAnimeGroupingPolicyChangeQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    change: AnimeGroupingPolicyChange,
    after_record_id: Option<RecordId>,
    limit: IdentityImpactPageLimit,
}

impl PreviewAnimeGroupingPolicyChangeQuery {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        change: AnimeGroupingPolicyChange,
        after_record_id: Option<RecordId>,
        limit: IdentityImpactPageLimit,
    ) -> Self {
        Self {
            correlation_id,
            access,
            change,
            after_record_id,
            limit,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn change(&self) -> AnimeGroupingPolicyChange {
        self.change
    }

    pub const fn after_record_id(&self) -> Option<RecordId> {
        self.after_record_id
    }

    pub const fn limit(&self) -> IdentityImpactPageLimit {
        self.limit
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyAnimeGroupingPolicyChangeCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    operation_id: OperationId,
    semantic_digest: Sha256Digest,
    expected_revision: u64,
    change: AnimeGroupingPolicyChange,
}

impl ApplyAnimeGroupingPolicyChangeCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        operation_id: OperationId,
        semantic_digest: Sha256Digest,
        expected_revision: u64,
        change: AnimeGroupingPolicyChange,
    ) -> Self {
        Self {
            correlation_id,
            access,
            operation_id,
            semantic_digest,
            expected_revision,
            change,
        }
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

    pub const fn semantic_digest(&self) -> &Sha256Digest {
        &self.semantic_digest
    }

    pub const fn expected_revision(&self) -> u64 {
        self.expected_revision
    }

    pub const fn change(&self) -> AnimeGroupingPolicyChange {
        self.change
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimeGroupingPolicyView {
    profile_id: ProfileId,
    preference: AnimeGroupingPreference,
    revision: u64,
}

impl AnimeGroupingPolicyView {
    pub const fn new(
        profile_id: ProfileId,
        preference: AnimeGroupingPreference,
        revision: u64,
    ) -> Self {
        Self {
            profile_id,
            preference,
            revision,
        }
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn preference(&self) -> AnimeGroupingPreference {
        self.preference
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimeGroupingPolicyImpact {
    policy: AnimeGroupingPolicyView,
    proposed_preference: AnimeGroupingPreference,
    total_records: u64,
    affected_records: u64,
    unresolved_routes: u64,
    possible_season_regroupings: u64,
    records: Vec<AnimeGroupingRecordPreview>,
    next_after_record_id: Option<RecordId>,
}

impl AnimeGroupingPolicyImpact {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        policy: AnimeGroupingPolicyView,
        proposed_preference: AnimeGroupingPreference,
        total_records: u64,
        affected_records: u64,
        unresolved_routes: u64,
        possible_season_regroupings: u64,
        records: Vec<AnimeGroupingRecordPreview>,
        next_after_record_id: Option<RecordId>,
    ) -> Self {
        Self {
            policy,
            proposed_preference,
            total_records,
            affected_records,
            unresolved_routes,
            possible_season_regroupings,
            records,
            next_after_record_id,
        }
    }

    pub const fn policy(&self) -> &AnimeGroupingPolicyView {
        &self.policy
    }

    pub const fn proposed_preference(&self) -> AnimeGroupingPreference {
        self.proposed_preference
    }

    pub const fn total_records(&self) -> u64 {
        self.total_records
    }

    pub const fn affected_records(&self) -> u64 {
        self.affected_records
    }

    pub const fn unresolved_routes(&self) -> u64 {
        self.unresolved_routes
    }

    pub const fn possible_season_regroupings(&self) -> u64 {
        self.possible_season_regroupings
    }

    pub fn records(&self) -> &[AnimeGroupingRecordPreview] {
        &self.records
    }

    pub const fn next_after_record_id(&self) -> Option<RecordId> {
        self.next_after_record_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyAnimeGroupingPolicyChangeOutcome {
    operation_id: OperationId,
    previous_preference: AnimeGroupingPreference,
    policy: AnimeGroupingPolicyView,
    affected_records: u64,
    unresolved_routes: u64,
    possible_season_regroupings: u64,
    rolled_back_operation_id: Option<OperationId>,
}

impl ApplyAnimeGroupingPolicyChangeOutcome {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        operation_id: OperationId,
        previous_preference: AnimeGroupingPreference,
        policy: AnimeGroupingPolicyView,
        affected_records: u64,
        unresolved_routes: u64,
        possible_season_regroupings: u64,
        rolled_back_operation_id: Option<OperationId>,
    ) -> Self {
        Self {
            operation_id,
            previous_preference,
            policy,
            affected_records,
            unresolved_routes,
            possible_season_regroupings,
            rolled_back_operation_id,
        }
    }

    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    pub const fn previous_preference(&self) -> AnimeGroupingPreference {
        self.previous_preference
    }

    pub const fn policy(&self) -> &AnimeGroupingPolicyView {
        &self.policy
    }

    pub const fn affected_records(&self) -> u64 {
        self.affected_records
    }

    pub const fn unresolved_routes(&self) -> u64 {
        self.unresolved_routes
    }

    pub const fn possible_season_regroupings(&self) -> u64 {
        self.possible_season_regroupings
    }

    pub const fn rolled_back_operation_id(&self) -> Option<OperationId> {
        self.rolled_back_operation_id
    }
}

/// Authorized identity routing and profile-policy boundary.
///
/// Implementations load identifiers server-side, re-authorize inside the
/// transaction, apply compare-and-set policy revisions, and retain immutable
/// operation receipts. They never move Records or Chronicle history.
pub trait IdentityRoutingPort: Send + Sync {
    fn authorize_and_resolve_identity(
        &self,
        query: ResolveIdentityRouteQuery,
    ) -> ApplicationResult<PurposeIdentityRoutePlan>;

    fn authorize_and_read_anime_grouping_policy(
        &self,
        query: ReadAnimeGroupingPolicyQuery,
    ) -> ApplicationResult<AnimeGroupingPolicyView>;

    fn authorize_and_preview_anime_grouping_policy_change(
        &self,
        query: PreviewAnimeGroupingPolicyChangeQuery,
    ) -> ApplicationResult<AnimeGroupingPolicyImpact>;

    fn authorize_and_apply_anime_grouping_policy_change(
        &self,
        command: ApplyAnimeGroupingPolicyChangeCommand,
    ) -> ApplicationResult<ApplyAnimeGroupingPolicyChangeOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impact_page_limit_is_non_zero_and_bounded() {
        assert!(IdentityImpactPageLimit::try_new(0).is_none());
        assert!(IdentityImpactPageLimit::try_new(MAX_IDENTITY_IMPACT_PAGE + 1).is_none());
        assert_eq!(
            IdentityImpactPageLimit::try_new(MAX_IDENTITY_IMPACT_PAGE)
                .expect("maximum page")
                .get(),
            MAX_IDENTITY_IMPACT_PAGE
        );
    }
}
