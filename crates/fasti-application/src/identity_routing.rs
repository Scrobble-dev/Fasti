use crate::{
    AnimeGroupingRecordPreview, ApplicationResult, ProviderId, PurposeIdentityRoutePlan,
    RequestAccessContext,
};
use fasti_domain::{
    AnimeGroupingPreference, ClientId, OperationId, ProfileId, RecordId, RequestCorrelationId,
    ResolutionIntent, Sha256Digest,
};
use std::{error::Error, fmt, num::NonZeroU16};

pub const MAX_IDENTITY_IMPACT_PAGE: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimeGroupingPolicyScope {
    Profile,
    Client(ClientId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimeGroupingPolicySource {
    ProfileDefault,
    ClientOverride,
}

impl AnimeGroupingPolicySource {
    const fn is_valid_for(self, scope: AnimeGroupingPolicyScope) -> bool {
        !matches!(
            (scope, self),
            (AnimeGroupingPolicyScope::Profile, Self::ClientOverride)
        )
    }
}

impl AnimeGroupingPolicyScope {
    pub const fn client_id(self) -> Option<ClientId> {
        match self {
            Self::Profile => None,
            Self::Client(client_id) => Some(client_id),
        }
    }
}

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
    scope: AnimeGroupingPolicyScope,
}

impl ReadAnimeGroupingPolicyQuery {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        scope: AnimeGroupingPolicyScope,
    ) -> Self {
        Self {
            correlation_id,
            access,
            scope,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn scope(&self) -> AnimeGroupingPolicyScope {
        self.scope
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimeGroupingPolicyChange {
    Set(AnimeGroupingPreference),
    InheritProfile,
    Rollback { applied_operation_id: OperationId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimeGroupingPolicyChangeError;

impl fmt::Display for AnimeGroupingPolicyChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("only a client policy can inherit the profile default")
    }
}

impl Error for AnimeGroupingPolicyChangeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimeGroupingPolicyResultError;

impl fmt::Display for AnimeGroupingPolicyResultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("anime grouping policy result is internally inconsistent")
    }
}

impl Error for AnimeGroupingPolicyResultError {}

impl AnimeGroupingPolicyChange {
    const fn is_valid_for(self, scope: AnimeGroupingPolicyScope) -> bool {
        !matches!(
            (scope, self),
            (AnimeGroupingPolicyScope::Profile, Self::InheritProfile)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewAnimeGroupingPolicyChangeQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    scope: AnimeGroupingPolicyScope,
    change: AnimeGroupingPolicyChange,
    after_record_id: Option<RecordId>,
    limit: IdentityImpactPageLimit,
}

impl PreviewAnimeGroupingPolicyChangeQuery {
    pub fn try_new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        scope: AnimeGroupingPolicyScope,
        change: AnimeGroupingPolicyChange,
        after_record_id: Option<RecordId>,
        limit: IdentityImpactPageLimit,
    ) -> Result<Self, AnimeGroupingPolicyChangeError> {
        if !change.is_valid_for(scope) {
            return Err(AnimeGroupingPolicyChangeError);
        }
        Ok(Self {
            correlation_id,
            access,
            scope,
            change,
            after_record_id,
            limit,
        })
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn scope(&self) -> AnimeGroupingPolicyScope {
        self.scope
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
    scope: AnimeGroupingPolicyScope,
    operation_id: OperationId,
    semantic_digest: Sha256Digest,
    expected_revision: u64,
    change: AnimeGroupingPolicyChange,
}

impl ApplyAnimeGroupingPolicyChangeCommand {
    pub fn try_new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        scope: AnimeGroupingPolicyScope,
        operation_id: OperationId,
        semantic_digest: Sha256Digest,
        expected_revision: u64,
        change: AnimeGroupingPolicyChange,
    ) -> Result<Self, AnimeGroupingPolicyChangeError> {
        if !change.is_valid_for(scope) {
            return Err(AnimeGroupingPolicyChangeError);
        }
        Ok(Self {
            correlation_id,
            access,
            scope,
            operation_id,
            semantic_digest,
            expected_revision,
            change,
        })
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn scope(&self) -> AnimeGroupingPolicyScope {
        self.scope
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
    scope: AnimeGroupingPolicyScope,
    source: AnimeGroupingPolicySource,
    preference: AnimeGroupingPreference,
    revision: u64,
}

impl AnimeGroupingPolicyView {
    pub const fn try_new(
        profile_id: ProfileId,
        scope: AnimeGroupingPolicyScope,
        source: AnimeGroupingPolicySource,
        preference: AnimeGroupingPreference,
        revision: u64,
    ) -> Result<Self, AnimeGroupingPolicyResultError> {
        if !source.is_valid_for(scope) {
            return Err(AnimeGroupingPolicyResultError);
        }
        Ok(Self {
            profile_id,
            scope,
            source,
            preference,
            revision,
        })
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn scope(&self) -> AnimeGroupingPolicyScope {
        self.scope
    }

    pub const fn source(&self) -> AnimeGroupingPolicySource {
        self.source
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
    proposed_source: AnimeGroupingPolicySource,
    total_records: u64,
    affected_records: u64,
    unresolved_routes: u64,
    possible_season_regroupings: u64,
    records: Vec<AnimeGroupingRecordPreview>,
    next_after_record_id: Option<RecordId>,
}

impl AnimeGroupingPolicyImpact {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        policy: AnimeGroupingPolicyView,
        proposed_preference: AnimeGroupingPreference,
        proposed_source: AnimeGroupingPolicySource,
        total_records: u64,
        affected_records: u64,
        unresolved_routes: u64,
        possible_season_regroupings: u64,
        records: Vec<AnimeGroupingRecordPreview>,
        next_after_record_id: Option<RecordId>,
    ) -> Result<Self, AnimeGroupingPolicyResultError> {
        let records_are_strictly_ordered = records
            .windows(2)
            .all(|pair| pair[0].record_id().uuid() < pair[1].record_id().uuid());
        let cursor_is_valid = next_after_record_id.is_none_or(|cursor| {
            records
                .last()
                .is_some_and(|record| record.record_id() == cursor)
        });
        if !proposed_source.is_valid_for(policy.scope())
            || records.len() > usize::from(MAX_IDENTITY_IMPACT_PAGE)
            || records.len() as u64 > total_records
            || affected_records > total_records
            || unresolved_routes > total_records
            || possible_season_regroupings > affected_records
            || !records_are_strictly_ordered
            || !cursor_is_valid
        {
            return Err(AnimeGroupingPolicyResultError);
        }
        Ok(Self {
            policy,
            proposed_preference,
            proposed_source,
            total_records,
            affected_records,
            unresolved_routes,
            possible_season_regroupings,
            records,
            next_after_record_id,
        })
    }

    pub const fn policy(&self) -> &AnimeGroupingPolicyView {
        &self.policy
    }

    pub const fn proposed_preference(&self) -> AnimeGroupingPreference {
        self.proposed_preference
    }

    pub const fn proposed_source(&self) -> AnimeGroupingPolicySource {
        self.proposed_source
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
    previous_source: AnimeGroupingPolicySource,
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
        previous_source: AnimeGroupingPolicySource,
        policy: AnimeGroupingPolicyView,
        affected_records: u64,
        unresolved_routes: u64,
        possible_season_regroupings: u64,
        rolled_back_operation_id: Option<OperationId>,
    ) -> Self {
        Self {
            operation_id,
            previous_preference,
            previous_source,
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

    pub const fn previous_source(&self) -> AnimeGroupingPolicySource {
        self.previous_source
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

    #[test]
    fn impact_result_rejects_oversized_pages_and_impossible_counts() {
        let policy = AnimeGroupingPolicyView::try_new(
            ProfileId::new_v7(),
            AnimeGroupingPolicyScope::Profile,
            AnimeGroupingPolicySource::ProfileDefault,
            AnimeGroupingPreference::Automatic,
            0,
        )
        .expect("valid profile policy");
        let mut records = (0..=MAX_IDENTITY_IMPACT_PAGE)
            .map(|_| {
                crate::preview_anime_grouping_change_for_record(
                    RecordId::new_v7(),
                    AnimeGroupingPreference::Automatic,
                    AnimeGroupingPreference::Automatic,
                    &[],
                )
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.record_id().uuid());

        assert!(AnimeGroupingPolicyImpact::try_new(
            policy,
            AnimeGroupingPreference::Automatic,
            AnimeGroupingPolicySource::ProfileDefault,
            u64::from(MAX_IDENTITY_IMPACT_PAGE) + 1,
            0,
            u64::from(MAX_IDENTITY_IMPACT_PAGE) + 1,
            0,
            records,
            None,
        )
        .is_err());
        assert!(AnimeGroupingPolicyImpact::try_new(
            policy,
            AnimeGroupingPreference::Automatic,
            AnimeGroupingPolicySource::ProfileDefault,
            1,
            0,
            0,
            1,
            Vec::new(),
            None,
        )
        .is_err());
    }

    #[test]
    fn policy_scope_distinguishes_profile_default_from_client_override() {
        let client_id = ClientId::new_v7();
        assert_eq!(AnimeGroupingPolicyScope::Profile.client_id(), None);
        assert_eq!(
            AnimeGroupingPolicyScope::Client(client_id).client_id(),
            Some(client_id)
        );

        let inherited = AnimeGroupingPolicyView::try_new(
            ProfileId::new_v7(),
            AnimeGroupingPolicyScope::Client(client_id),
            AnimeGroupingPolicySource::ProfileDefault,
            AnimeGroupingPreference::Automatic,
            4,
        )
        .expect("valid inherited client policy");
        assert_eq!(
            inherited.source(),
            AnimeGroupingPolicySource::ProfileDefault
        );
        assert_eq!(inherited.revision(), 4);
        assert!(AnimeGroupingPolicyView::try_new(
            ProfileId::new_v7(),
            AnimeGroupingPolicyScope::Profile,
            AnimeGroupingPolicySource::ClientOverride,
            AnimeGroupingPreference::Automatic,
            0,
        )
        .is_err());
    }

    #[test]
    fn profile_policy_cannot_inherit_itself() {
        let access = RequestAccessContext::new(
            fasti_domain::WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            fasti_domain::CredentialId::new_v7(),
            fasti_domain::ProfileGrantId::new_v7(),
            1,
        );
        let limit = IdentityImpactPageLimit::try_new(1).expect("one-record preview");

        assert!(PreviewAnimeGroupingPolicyChangeQuery::try_new(
            RequestCorrelationId::new_v7(),
            access,
            AnimeGroupingPolicyScope::Profile,
            AnimeGroupingPolicyChange::InheritProfile,
            None,
            limit,
        )
        .is_err());
        assert!(PreviewAnimeGroupingPolicyChangeQuery::try_new(
            RequestCorrelationId::new_v7(),
            access,
            AnimeGroupingPolicyScope::Client(ClientId::new_v7()),
            AnimeGroupingPolicyChange::InheritProfile,
            None,
            limit,
        )
        .is_ok());
        assert!(ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            access,
            AnimeGroupingPolicyScope::Profile,
            OperationId::new_v7(),
            Sha256Digest::from_bytes(&[0; 32]),
            0,
            AnimeGroupingPolicyChange::InheritProfile,
        )
        .is_err());
    }
}
