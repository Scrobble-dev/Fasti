//! Purpose-specific identity routing and anime projection policy DTOs.

use fasti_application::{
    AcceptedIdentityRouteAssertion, AnimeGroupingPolicyChange, AnimeGroupingPolicyImpact,
    AnimeGroupingPolicyScope, AnimeGroupingPolicySource, AnimeGroupingPolicyView,
    AnimeGroupingRecordPreview, ApplyAnimeGroupingPolicyChangeOutcome, PurposeIdentityRoute,
    PurposeIdentityRoutePlan, PurposeIdentityRouteStatus,
};
use fasti_domain::{
    AnimeGroupingPreference, IdentityAssertionRelation, IdentityRouteKind, ResolutionIntent,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionIntentDto {
    MetadataSearch,
    MetadataLookup,
    MetadataEnrichment,
    RatingLookup,
    CatalogLookup,
    DisplayProjection,
    NuvioExport,
    NuvioImportAttachment,
    TrackerRead,
    TrackerWrite,
    SegmentTranslation,
    DeduplicationReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRouteStatusDto {
    Selected,
    Missing,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRouteKindDto {
    ProviderNative,
    VerifiedAlias,
    AcceptedCrosswalk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentityAssertionRelationDto {
    Exact,
    SubsetOf,
    SupersetOf,
    Overlaps,
    AlternateCutOf,
    Related,
    NotSameAs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityIdentifierDto {
    pub namespace: String,
    pub grain: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptedIdentityRouteAssertionDto {
    pub assertion_id: String,
    pub record_id: String,
    pub source_grain: String,
    pub relation: IdentityAssertionRelationDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IdentityRouteDto {
    pub identifier: IdentityIdentifierDto,
    pub kind: IdentityRouteKindDto,
    pub accepted_assertions: Vec<AcceptedIdentityRouteAssertionDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, IntoParams)]
#[serde(deny_unknown_fields)]
pub struct ResolveIdentityRouteParameters {
    pub intent: ResolutionIntentDto,
    #[schemars(length(min = 2, max = 64))]
    #[param(min_length = 2, max_length = 64)]
    pub target_provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolveIdentityRouteResponse {
    pub record_id: String,
    pub intent: ResolutionIntentDto,
    pub target_provider: String,
    pub status: IdentityRouteStatusDto,
    pub known_identifiers: Vec<IdentityIdentifierDto>,
    pub candidate_routes: Vec<IdentityRouteDto>,
    pub selected_route: Option<IdentityRouteDto>,
    pub nuvio_content_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnimeGroupingPreferenceDto {
    Automatic,
    GroupByTvWork,
    KeepMalReleasesSeparate,
    KeepKitsuReleasesSeparate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnimeGroupingPolicyScopeKindDto {
    Profile,
    Client,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnimeGroupingPolicySourceDto {
    ProfileDefault,
    ClientOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimeGroupingPolicyScopeDto {
    pub kind: AnimeGroupingPolicyScopeKindDto,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, IntoParams)]
#[serde(deny_unknown_fields)]
pub struct ReadAnimeGroupingPolicyParameters {
    pub scope: AnimeGroupingPolicyScopeKindDto,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnimeGroupingPolicyChangeDto {
    Set {
        preference: AnimeGroupingPreferenceDto,
    },
    InheritProfile,
    Rollback {
        applied_operation_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimeGroupingPolicyDto {
    pub profile_id: String,
    pub scope: AnimeGroupingPolicyScopeDto,
    pub source: AnimeGroupingPolicySourceDto,
    pub preference: AnimeGroupingPreferenceDto,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadAnimeGroupingPolicyResponse {
    pub policy: AnimeGroupingPolicyDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewAnimeGroupingPolicyChangeRequest {
    pub scope: AnimeGroupingPolicyScopeDto,
    pub change: AnimeGroupingPolicyChangeDto,
    pub after_record_id: Option<String>,
    #[schemars(range(min = 1, max = 100))]
    #[schema(minimum = 1, maximum = 100)]
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimeGroupingRecordPreviewDto {
    pub record_id: String,
    pub previous_preference: AnimeGroupingPreferenceDto,
    pub proposed_preference: AnimeGroupingPreferenceDto,
    pub previous_status: IdentityRouteStatusDto,
    pub proposed_status: IdentityRouteStatusDto,
    pub previous_route: Option<IdentityRouteDto>,
    pub proposed_route: Option<IdentityRouteDto>,
    pub route_changed: bool,
    pub possible_season_regrouping: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AnimeGroupingPolicyImpactResponse {
    pub policy: AnimeGroupingPolicyDto,
    pub proposed_preference: AnimeGroupingPreferenceDto,
    pub proposed_source: AnimeGroupingPolicySourceDto,
    pub total_records: u64,
    pub affected_records: u64,
    pub unresolved_routes: u64,
    pub possible_season_regroupings: u64,
    pub records: Vec<AnimeGroupingRecordPreviewDto>,
    pub next_after_record_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyAnimeGroupingPolicyChangeRequest {
    pub operation_id: String,
    pub scope: AnimeGroupingPolicyScopeDto,
    pub expected_revision: u64,
    pub change: AnimeGroupingPolicyChangeDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyAnimeGroupingPolicyChangeResponse {
    pub operation_id: String,
    pub change: AnimeGroupingPolicyChangeDto,
    pub previous_preference: AnimeGroupingPreferenceDto,
    pub previous_source: AnimeGroupingPolicySourceDto,
    pub policy: AnimeGroupingPolicyDto,
    pub affected_records: u64,
    pub unresolved_routes: u64,
    pub possible_season_regroupings: u64,
    pub rolled_back_operation_id: Option<String>,
}

pub const fn resolution_intent(value: ResolutionIntentDto) -> ResolutionIntent {
    match value {
        ResolutionIntentDto::MetadataSearch => ResolutionIntent::MetadataSearch,
        ResolutionIntentDto::MetadataLookup => ResolutionIntent::MetadataLookup,
        ResolutionIntentDto::MetadataEnrichment => ResolutionIntent::MetadataEnrichment,
        ResolutionIntentDto::RatingLookup => ResolutionIntent::RatingLookup,
        ResolutionIntentDto::CatalogLookup => ResolutionIntent::CatalogLookup,
        ResolutionIntentDto::DisplayProjection => ResolutionIntent::DisplayProjection,
        ResolutionIntentDto::NuvioExport => ResolutionIntent::NuvioExport,
        ResolutionIntentDto::NuvioImportAttachment => ResolutionIntent::NuvioImportAttachment,
        ResolutionIntentDto::TrackerRead => ResolutionIntent::TrackerRead,
        ResolutionIntentDto::TrackerWrite => ResolutionIntent::TrackerWrite,
        ResolutionIntentDto::SegmentTranslation => ResolutionIntent::SegmentTranslation,
        ResolutionIntentDto::DeduplicationReview => ResolutionIntent::DeduplicationReview,
    }
}

pub const fn anime_grouping_preference(
    value: AnimeGroupingPreferenceDto,
) -> AnimeGroupingPreference {
    match value {
        AnimeGroupingPreferenceDto::Automatic => AnimeGroupingPreference::Automatic,
        AnimeGroupingPreferenceDto::GroupByTvWork => AnimeGroupingPreference::GroupByTvWork,
        AnimeGroupingPreferenceDto::KeepMalReleasesSeparate => {
            AnimeGroupingPreference::KeepMalReleasesSeparate
        }
        AnimeGroupingPreferenceDto::KeepKitsuReleasesSeparate => {
            AnimeGroupingPreference::KeepKitsuReleasesSeparate
        }
    }
}

const fn preference_dto(value: AnimeGroupingPreference) -> AnimeGroupingPreferenceDto {
    match value {
        AnimeGroupingPreference::Automatic => AnimeGroupingPreferenceDto::Automatic,
        AnimeGroupingPreference::GroupByTvWork => AnimeGroupingPreferenceDto::GroupByTvWork,
        AnimeGroupingPreference::KeepMalReleasesSeparate => {
            AnimeGroupingPreferenceDto::KeepMalReleasesSeparate
        }
        AnimeGroupingPreference::KeepKitsuReleasesSeparate => {
            AnimeGroupingPreferenceDto::KeepKitsuReleasesSeparate
        }
    }
}

const fn source_dto(value: AnimeGroupingPolicySource) -> AnimeGroupingPolicySourceDto {
    match value {
        AnimeGroupingPolicySource::ProfileDefault => AnimeGroupingPolicySourceDto::ProfileDefault,
        AnimeGroupingPolicySource::ClientOverride => AnimeGroupingPolicySourceDto::ClientOverride,
    }
}

const fn status_dto(value: PurposeIdentityRouteStatus) -> IdentityRouteStatusDto {
    match value {
        PurposeIdentityRouteStatus::Selected => IdentityRouteStatusDto::Selected,
        PurposeIdentityRouteStatus::Missing => IdentityRouteStatusDto::Missing,
        PurposeIdentityRouteStatus::Ambiguous => IdentityRouteStatusDto::Ambiguous,
    }
}

fn identifier_dto(value: &fasti_domain::ExternalIdentifierClaim) -> IdentityIdentifierDto {
    IdentityIdentifierDto {
        namespace: value.namespace().to_owned(),
        grain: value.grain().as_str().to_owned(),
        value: value.value().to_owned(),
    }
}

fn accepted_assertion_dto(
    value: AcceptedIdentityRouteAssertion,
) -> AcceptedIdentityRouteAssertionDto {
    AcceptedIdentityRouteAssertionDto {
        assertion_id: value.assertion_id().to_string(),
        record_id: value.record_id().to_string(),
        source_grain: value.source_grain().as_str().to_owned(),
        relation: match value.relation() {
            IdentityAssertionRelation::Exact => IdentityAssertionRelationDto::Exact,
            IdentityAssertionRelation::SubsetOf => IdentityAssertionRelationDto::SubsetOf,
            IdentityAssertionRelation::SupersetOf => IdentityAssertionRelationDto::SupersetOf,
            IdentityAssertionRelation::Overlaps => IdentityAssertionRelationDto::Overlaps,
            IdentityAssertionRelation::AlternateCutOf => {
                IdentityAssertionRelationDto::AlternateCutOf
            }
            IdentityAssertionRelation::Related => IdentityAssertionRelationDto::Related,
            IdentityAssertionRelation::NotSameAs => IdentityAssertionRelationDto::NotSameAs,
        },
    }
}

fn route_dto(value: &PurposeIdentityRoute) -> IdentityRouteDto {
    IdentityRouteDto {
        identifier: identifier_dto(value.identifier()),
        kind: match value.kind() {
            IdentityRouteKind::ProviderNative => IdentityRouteKindDto::ProviderNative,
            IdentityRouteKind::VerifiedAlias => IdentityRouteKindDto::VerifiedAlias,
            IdentityRouteKind::AcceptedCrosswalk => IdentityRouteKindDto::AcceptedCrosswalk,
        },
        accepted_assertions: value
            .accepted_assertions()
            .iter()
            .copied()
            .map(accepted_assertion_dto)
            .collect(),
    }
}

fn scope_dto(value: AnimeGroupingPolicyScope) -> AnimeGroupingPolicyScopeDto {
    match value {
        AnimeGroupingPolicyScope::Profile => AnimeGroupingPolicyScopeDto {
            kind: AnimeGroupingPolicyScopeKindDto::Profile,
            client_id: None,
        },
        AnimeGroupingPolicyScope::Client(client_id) => AnimeGroupingPolicyScopeDto {
            kind: AnimeGroupingPolicyScopeKindDto::Client,
            client_id: Some(client_id.to_string()),
        },
    }
}

fn policy_dto(value: &AnimeGroupingPolicyView) -> AnimeGroupingPolicyDto {
    AnimeGroupingPolicyDto {
        profile_id: value.profile_id().to_string(),
        scope: scope_dto(value.scope()),
        source: source_dto(value.source()),
        preference: preference_dto(value.preference()),
        revision: value.revision(),
    }
}

fn change_dto(value: AnimeGroupingPolicyChange) -> AnimeGroupingPolicyChangeDto {
    match value {
        AnimeGroupingPolicyChange::Set(preference) => AnimeGroupingPolicyChangeDto::Set {
            preference: preference_dto(preference),
        },
        AnimeGroupingPolicyChange::InheritProfile => AnimeGroupingPolicyChangeDto::InheritProfile,
        AnimeGroupingPolicyChange::Rollback {
            applied_operation_id,
        } => AnimeGroupingPolicyChangeDto::Rollback {
            applied_operation_id: applied_operation_id.to_string(),
        },
    }
}

fn preview_dto(value: &AnimeGroupingRecordPreview) -> AnimeGroupingRecordPreviewDto {
    AnimeGroupingRecordPreviewDto {
        record_id: value.record_id().to_string(),
        previous_preference: preference_dto(value.previous_preference()),
        proposed_preference: preference_dto(value.proposed_preference()),
        previous_status: status_dto(value.previous_status()),
        proposed_status: status_dto(value.proposed_status()),
        previous_route: value.previous_route().map(route_dto),
        proposed_route: value.proposed_route().map(route_dto),
        route_changed: value.route_changed(),
        possible_season_regrouping: value.possible_season_regrouping(),
    }
}

pub fn resolve_identity_route_response(
    value: &PurposeIdentityRoutePlan,
) -> ResolveIdentityRouteResponse {
    ResolveIdentityRouteResponse {
        record_id: value.record_id().to_string(),
        intent: match value.intent() {
            ResolutionIntent::MetadataSearch => ResolutionIntentDto::MetadataSearch,
            ResolutionIntent::MetadataLookup => ResolutionIntentDto::MetadataLookup,
            ResolutionIntent::MetadataEnrichment => ResolutionIntentDto::MetadataEnrichment,
            ResolutionIntent::RatingLookup => ResolutionIntentDto::RatingLookup,
            ResolutionIntent::CatalogLookup => ResolutionIntentDto::CatalogLookup,
            ResolutionIntent::DisplayProjection => ResolutionIntentDto::DisplayProjection,
            ResolutionIntent::NuvioExport => ResolutionIntentDto::NuvioExport,
            ResolutionIntent::NuvioImportAttachment => ResolutionIntentDto::NuvioImportAttachment,
            ResolutionIntent::TrackerRead => ResolutionIntentDto::TrackerRead,
            ResolutionIntent::TrackerWrite => ResolutionIntentDto::TrackerWrite,
            ResolutionIntent::SegmentTranslation => ResolutionIntentDto::SegmentTranslation,
            ResolutionIntent::DeduplicationReview => ResolutionIntentDto::DeduplicationReview,
        },
        target_provider: value.target_provider().as_str().to_owned(),
        status: status_dto(value.status()),
        known_identifiers: value
            .known_identifiers()
            .iter()
            .map(identifier_dto)
            .collect(),
        candidate_routes: value.candidate_routes().iter().map(route_dto).collect(),
        selected_route: value.selected_route().map(route_dto),
        nuvio_content_id: value.nuvio_content_id(),
    }
}

pub fn read_anime_grouping_policy_response(
    value: &AnimeGroupingPolicyView,
) -> ReadAnimeGroupingPolicyResponse {
    ReadAnimeGroupingPolicyResponse {
        policy: policy_dto(value),
    }
}

pub fn anime_grouping_policy_impact_response(
    value: &AnimeGroupingPolicyImpact,
) -> AnimeGroupingPolicyImpactResponse {
    AnimeGroupingPolicyImpactResponse {
        policy: policy_dto(value.policy()),
        proposed_preference: preference_dto(value.proposed_preference()),
        proposed_source: source_dto(value.proposed_source()),
        total_records: value.total_records(),
        affected_records: value.affected_records(),
        unresolved_routes: value.unresolved_routes(),
        possible_season_regroupings: value.possible_season_regroupings(),
        records: value.records().iter().map(preview_dto).collect(),
        next_after_record_id: value.next_after_record_id().map(|id| id.to_string()),
    }
}

pub fn apply_anime_grouping_policy_change_response(
    value: &ApplyAnimeGroupingPolicyChangeOutcome,
) -> ApplyAnimeGroupingPolicyChangeResponse {
    ApplyAnimeGroupingPolicyChangeResponse {
        operation_id: value.operation_id().to_string(),
        change: change_dto(value.change()),
        previous_preference: preference_dto(value.previous_preference()),
        previous_source: source_dto(value.previous_source()),
        policy: policy_dto(value.policy()),
        affected_records: value.affected_records(),
        unresolved_routes: value.unresolved_routes(),
        possible_season_regroupings: value.possible_season_regroupings(),
        rolled_back_operation_id: value.rolled_back_operation_id().map(|id| id.to_string()),
    }
}
