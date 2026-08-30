//! Authenticated M2 metadata refresh, projection, and provenance DTOs.
//!
//! These representations expose immutable claim evidence and non-secret cache
//! partition metadata. Provider response bodies, credentials, and credential
//! values are never part of this contract.

use fasti_application::{
    ConfigureMetadataProjectionOutcome, FieldClaimView, MetadataCacheReadView,
    MetadataProjectionView, MetadataRefreshMode, RatingClaimView, RefreshMetadataClaimsOutcome,
};
use fasti_domain::{
    EnrichmentPolicy, FieldClaimProvenance, FieldClaimStatus, FieldKey, FieldResolutionTier,
    LastKnownGoodPolicy, MetadataAttribution, MetadataCacheInvalidationReason,
    MetadataCachePurpose, MetadataCacheReadState, MetadataDataClassification, MetadataFieldGroup,
    MetadataProjection, RatingClaim, RecordId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataFieldGroupDto {
    Artwork,
    BasicInfo,
    Details,
    ReleaseDates,
    Credits,
    ProductionCompanies,
    Networks,
    Episodes,
    SeasonArtwork,
    Recommendations,
    Collections,
    Trailers,
    WatchProviders,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataRefreshModeDto {
    PreferCache,
    Revalidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RefreshMetadataClaimsRequest {
    #[schemars(
        length(equal = 35),
        regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"),
        extend("format" = "fasti-operation-id")
    )]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-operation-id"
    )]
    pub operation_id: String,
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub record_id: String,
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub provider_id: String,
    #[schemars(length(min = 1, max = 32))]
    #[schema(min_items = 1, max_items = 32)]
    pub field_groups: Vec<MetadataFieldGroupDto>,
    #[schemars(length(min = 2, max = 16))]
    #[schema(min_length = 2, max_length = 16)]
    pub locale: Option<String>,
    #[schemars(length(min = 2, max = 8))]
    #[schema(min_length = 2, max_length = 8)]
    pub region: Option<String>,
    pub mode: MetadataRefreshModeDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataClaimStatusDto {
    Fresh,
    Stale,
    Invalid,
    Revoked,
    Superseded,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataProjectionTierDto {
    UserOverride,
    PreferredProviderClaim,
    FallbackProviderClaim,
    LastKnownGood,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataClaimProvenanceDto {
    pub claim_id: String,
    pub record_id: Option<String>,
    pub field_key: Option<String>,
    pub provider_id: Option<String>,
    pub source_namespace: String,
    pub source_identifier: Option<String>,
    pub locale: Option<String>,
    pub region: Option<String>,
    pub source_version: Option<String>,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    #[schema(pattern = r"^sha256:[0-9a-f]{64}$")]
    pub evidence_digest: Option<String>,
    pub fetched_at: String,
    pub expires_at: Option<String>,
    pub status: MetadataClaimStatusDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataClaimDto {
    pub claim_id: String,
    pub record_id: Option<String>,
    pub field_key: Option<String>,
    pub value: String,
    pub provenance: MetadataClaimProvenanceDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RatingScaleDto {
    pub minimum_millis: u32,
    pub maximum_millis: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RatingClaimDto {
    pub claim_id: String,
    pub record_id: String,
    pub value_millis: u32,
    pub scale: RatingScaleDto,
    pub provenance: MetadataClaimProvenanceDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataProjectedFieldDto {
    pub profile_id: String,
    pub record_id: String,
    pub field_key: String,
    pub tier: MetadataProjectionTierDto,
    pub value: Option<String>,
    pub source_namespace: Option<String>,
    pub is_stale: bool,
    pub provenance: Option<MetadataClaimProvenanceDto>,
    pub projected_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LastKnownGoodPolicyDto {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EnrichmentPolicyDto {
    pub profile_id: String,
    pub preferred_provider_id: Option<String>,
    pub preferred_locale: Option<String>,
    pub original_locale: Option<String>,
    pub allow_english_fallback: bool,
    pub last_known_good: LastKnownGoodPolicyDto,
    pub region: Option<String>,
    #[schemars(length(max = 32))]
    #[schema(max_items = 32)]
    pub enabled_field_groups: Vec<MetadataFieldGroupDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataCachePurposeDto {
    MetadataEnrichment,
    DisplayProjection,
    RatingLookup,
    OfflineRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataDataClassificationDto {
    Public,
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataCacheKeyDto {
    pub provider_id: String,
    pub credential_reference_version: Option<u64>,
    pub record_id: String,
    pub resolved_provider_route: String,
    pub grain: String,
    pub source_namespace: String,
    pub source_identifier: String,
    pub locale: Option<String>,
    pub region: Option<String>,
    pub field_group: MetadataFieldGroupDto,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    #[schema(pattern = r"^sha256:[0-9a-f]{64}$")]
    pub settings_fingerprint: String,
    #[schemars(regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    #[schema(pattern = r"^sha256:[0-9a-f]{64}$")]
    pub configuration_digest: String,
    pub schema_version: u32,
    pub purpose: MetadataCachePurposeDto,
    pub terms_revision: String,
    pub classification: MetadataDataClassificationDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataCacheInvalidationReasonDto {
    ProviderConfigurationChanged,
    CredentialRotated,
    ProjectionPolicyChanged,
    TermsChanged,
    ExplicitRetraction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataCacheInvalidationDto {
    pub reason: MetadataCacheInvalidationReasonDto,
    pub invalidated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetadataCacheReadStateDto {
    Fresh,
    StaleWhileRefreshing,
    StaleOnError,
    Expired,
    Invalidated,
    PartitionDenied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataCacheEntryDto {
    pub key: MetadataCacheKeyDto,
    pub claim_ids: Vec<String>,
    pub created_at: String,
    pub fresh_until: String,
    pub stale_while_refreshing_until: String,
    pub stale_on_error_until: String,
    pub invalidation: Option<MetadataCacheInvalidationDto>,
    pub read_state: MetadataCacheReadStateDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataAttributionDto {
    pub provider_id: String,
    pub text: String,
    pub documentation_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RefreshMetadataClaimsResponse {
    pub record_id: String,
    pub provider_id: String,
    pub claims: Vec<MetadataClaimDto>,
    pub ratings: Vec<RatingClaimDto>,
    pub projections: Vec<MetadataProjectedFieldDto>,
    pub cache_entries: Vec<MetadataCacheEntryDto>,
    pub attributions: Vec<MetadataAttributionDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataProjectionResponse {
    pub profile_id: String,
    pub record_id: String,
    pub policy: EnrichmentPolicyDto,
    pub fields: Vec<MetadataProjectedFieldDto>,
    pub ratings: Vec<RatingClaimDto>,
    pub cache_entries: Vec<MetadataCacheEntryDto>,
    pub attributions: Vec<MetadataAttributionDto>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    JsonSchema,
    ToSchema,
    IntoParams,
)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct MetadataProjectionQueryParameters {
    /// Permit a policy-bounded stale-on-error cache partition when the host is
    /// explicitly operating offline. This never permits an expired,
    /// invalidated, or classification-denied partition.
    #[serde(default)]
    pub offline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum MetadataOverrideMutationDto {
    Set {
        #[schemars(
            length(equal = 36),
            regex(pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
        )]
        #[schema(
            min_length = 36,
            max_length = 36,
            pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
        )]
        record_id: String,
        #[schemars(
            length(min = 1, max = 64),
            regex(pattern = r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$")
        )]
        #[schema(
            min_length = 1,
            max_length = 64,
            pattern = r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$"
        )]
        field_key: String,
        #[schemars(
            length(min = 1, max = 4096),
            regex(pattern = r"^[^\u{0000}-\u{001f}\u{007f}]*$")
        )]
        #[schema(
            min_length = 1,
            max_length = 4096,
            pattern = r"^[^\u0000-\u001f\u007f]*$"
        )]
        value: String,
    },
    Clear {
        #[schemars(
            length(equal = 36),
            regex(pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
        )]
        #[schema(
            min_length = 36,
            max_length = 36,
            pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
        )]
        record_id: String,
        #[schemars(
            length(min = 1, max = 64),
            regex(pattern = r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$")
        )]
        #[schema(
            min_length = 1,
            max_length = 64,
            pattern = r"^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*$"
        )]
        field_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigureMetadataProjectionRequest {
    pub preferred_provider_id: Option<String>,
    pub preferred_locale: Option<String>,
    pub original_locale: Option<String>,
    pub allow_english_fallback: bool,
    pub last_known_good: LastKnownGoodPolicyDto,
    pub region: Option<String>,
    #[schemars(length(max = 32))]
    #[schema(max_items = 32)]
    pub enabled_field_groups: Vec<MetadataFieldGroupDto>,
    #[schemars(length(max = 64))]
    #[schema(max_items = 64)]
    pub overrides: Vec<MetadataOverrideMutationDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MetadataProjectionConfigurationResponse {
    pub policy: EnrichmentPolicyDto,
    pub invalidated_cache_entries: u32,
}

pub const fn metadata_field_group(value: MetadataFieldGroupDto) -> MetadataFieldGroup {
    match value {
        MetadataFieldGroupDto::Artwork => MetadataFieldGroup::Artwork,
        MetadataFieldGroupDto::BasicInfo => MetadataFieldGroup::BasicInfo,
        MetadataFieldGroupDto::Details => MetadataFieldGroup::Details,
        MetadataFieldGroupDto::ReleaseDates => MetadataFieldGroup::ReleaseDates,
        MetadataFieldGroupDto::Credits => MetadataFieldGroup::Credits,
        MetadataFieldGroupDto::ProductionCompanies => MetadataFieldGroup::ProductionCompanies,
        MetadataFieldGroupDto::Networks => MetadataFieldGroup::Networks,
        MetadataFieldGroupDto::Episodes => MetadataFieldGroup::Episodes,
        MetadataFieldGroupDto::SeasonArtwork => MetadataFieldGroup::SeasonArtwork,
        MetadataFieldGroupDto::Recommendations => MetadataFieldGroup::Recommendations,
        MetadataFieldGroupDto::Collections => MetadataFieldGroup::Collections,
        MetadataFieldGroupDto::Trailers => MetadataFieldGroup::Trailers,
        MetadataFieldGroupDto::WatchProviders => MetadataFieldGroup::WatchProviders,
    }
}

pub const fn metadata_refresh_mode(value: MetadataRefreshModeDto) -> MetadataRefreshMode {
    match value {
        MetadataRefreshModeDto::PreferCache => MetadataRefreshMode::PreferCache,
        MetadataRefreshModeDto::Revalidate => MetadataRefreshMode::Revalidate,
    }
}

const fn field_group_dto(value: MetadataFieldGroup) -> MetadataFieldGroupDto {
    match value {
        MetadataFieldGroup::Artwork => MetadataFieldGroupDto::Artwork,
        MetadataFieldGroup::BasicInfo => MetadataFieldGroupDto::BasicInfo,
        MetadataFieldGroup::Details => MetadataFieldGroupDto::Details,
        MetadataFieldGroup::ReleaseDates => MetadataFieldGroupDto::ReleaseDates,
        MetadataFieldGroup::Credits => MetadataFieldGroupDto::Credits,
        MetadataFieldGroup::ProductionCompanies => MetadataFieldGroupDto::ProductionCompanies,
        MetadataFieldGroup::Networks => MetadataFieldGroupDto::Networks,
        MetadataFieldGroup::Episodes => MetadataFieldGroupDto::Episodes,
        MetadataFieldGroup::SeasonArtwork => MetadataFieldGroupDto::SeasonArtwork,
        MetadataFieldGroup::Recommendations => MetadataFieldGroupDto::Recommendations,
        MetadataFieldGroup::Collections => MetadataFieldGroupDto::Collections,
        MetadataFieldGroup::Trailers => MetadataFieldGroupDto::Trailers,
        MetadataFieldGroup::WatchProviders => MetadataFieldGroupDto::WatchProviders,
    }
}

const fn claim_status(value: FieldClaimStatus) -> MetadataClaimStatusDto {
    match value {
        FieldClaimStatus::Fresh => MetadataClaimStatusDto::Fresh,
        FieldClaimStatus::Stale => MetadataClaimStatusDto::Stale,
        FieldClaimStatus::Invalid => MetadataClaimStatusDto::Invalid,
        FieldClaimStatus::Revoked => MetadataClaimStatusDto::Revoked,
        FieldClaimStatus::Superseded => MetadataClaimStatusDto::Superseded,
        FieldClaimStatus::Unavailable => MetadataClaimStatusDto::Unavailable,
    }
}

fn provenance_dto(
    claim_id: fasti_domain::MetadataClaimId,
    record_id: Option<RecordId>,
    field_key: Option<&FieldKey>,
    provenance: &FieldClaimProvenance,
    fetched_at: &impl ToString,
    expires_at: Option<&impl ToString>,
    status: FieldClaimStatus,
) -> MetadataClaimProvenanceDto {
    MetadataClaimProvenanceDto {
        claim_id: claim_id.to_string(),
        record_id: record_id.map(|value| value.to_string()),
        field_key: field_key.map(|value| value.as_str().to_owned()),
        provider_id: provenance
            .provider_id()
            .map(|value| value.as_str().to_owned()),
        source_namespace: provenance.source_namespace().as_str().to_owned(),
        source_identifier: provenance.source_identifier().map(str::to_owned),
        locale: provenance.locale().map(|value| value.as_str().to_owned()),
        region: provenance.region().map(|value| value.as_str().to_owned()),
        source_version: provenance.source_version().map(str::to_owned),
        evidence_digest: provenance
            .evidence_digest()
            .map(|value| value.as_str().to_owned()),
        fetched_at: fetched_at.to_string(),
        expires_at: expires_at.map(ToString::to_string),
        status: claim_status(status),
    }
}

fn field_claim_dto(view: &FieldClaimView) -> MetadataClaimDto {
    let claim = view.claim();
    MetadataClaimDto {
        claim_id: claim.claim_id().to_string(),
        record_id: claim.record_id().map(|value| value.to_string()),
        field_key: claim.field_key().map(|value| value.as_str().to_owned()),
        value: claim.value().to_owned(),
        provenance: provenance_dto(
            claim.claim_id(),
            claim.record_id(),
            claim.field_key(),
            claim.provenance(),
            &claim.fetched_at().to_rfc3339(),
            claim.expires_at().map(|value| value.to_rfc3339()).as_ref(),
            view.status(),
        ),
    }
}

fn rating_claim_dto(view: &RatingClaimView) -> RatingClaimDto {
    let claim: &RatingClaim = view.claim();
    RatingClaimDto {
        claim_id: claim.claim_id().to_string(),
        record_id: claim.record_id().to_string(),
        value_millis: claim.value_millis(),
        scale: RatingScaleDto {
            minimum_millis: claim.scale().minimum_millis(),
            maximum_millis: claim.scale().maximum_millis(),
        },
        provenance: provenance_dto(
            claim.claim_id(),
            Some(claim.record_id()),
            None,
            claim.provenance(),
            &claim.fetched_at().to_rfc3339(),
            claim.expires_at().map(|value| value.to_rfc3339()).as_ref(),
            view.status(),
        ),
    }
}

const fn projection_tier(value: FieldResolutionTier) -> MetadataProjectionTierDto {
    match value {
        FieldResolutionTier::UserOverride => MetadataProjectionTierDto::UserOverride,
        FieldResolutionTier::PreferredProviderClaim => {
            MetadataProjectionTierDto::PreferredProviderClaim
        }
        FieldResolutionTier::FallbackProviderClaim => {
            MetadataProjectionTierDto::FallbackProviderClaim
        }
        FieldResolutionTier::LastKnownGood => MetadataProjectionTierDto::LastKnownGood,
        FieldResolutionTier::Empty => MetadataProjectionTierDto::Empty,
    }
}

fn projection_dto(value: &MetadataProjection) -> MetadataProjectedFieldDto {
    let resolved = value.resolved_field();
    MetadataProjectedFieldDto {
        profile_id: value.profile_id().to_string(),
        record_id: value.record_id().to_string(),
        field_key: value.field_key().as_str().to_owned(),
        tier: projection_tier(resolved.tier()),
        value: resolved.value().map(str::to_owned),
        source_namespace: resolved.source().map(|value| value.as_str().to_owned()),
        is_stale: resolved.is_stale(),
        provenance: resolved.provenance().map(|provenance| {
            provenance_dto(
                provenance.claim_id(),
                provenance.record_id(),
                provenance.field_key(),
                provenance.claim_provenance(),
                &provenance.fetched_at().to_rfc3339(),
                provenance
                    .expires_at()
                    .map(|value| value.to_rfc3339())
                    .as_ref(),
                provenance.status(),
            )
        }),
        projected_at: value.projected_at().to_rfc3339(),
    }
}

fn enrichment_policy_dto(value: &EnrichmentPolicy) -> EnrichmentPolicyDto {
    let policy = value.projection_policy();
    EnrichmentPolicyDto {
        profile_id: value.profile_id().to_string(),
        preferred_provider_id: policy
            .preferred_provider_id()
            .map(|value| value.as_str().to_owned()),
        preferred_locale: policy
            .preferred_locale()
            .map(|value| value.as_str().to_owned()),
        original_locale: policy
            .original_locale()
            .map(|value| value.as_str().to_owned()),
        allow_english_fallback: policy.allow_english_fallback(),
        last_known_good: match policy.last_known_good() {
            LastKnownGoodPolicy::Allow => LastKnownGoodPolicyDto::Allow,
            LastKnownGoodPolicy::Deny => LastKnownGoodPolicyDto::Deny,
        },
        region: value.region().map(|value| value.as_str().to_owned()),
        enabled_field_groups: value
            .enabled_field_groups()
            .iter()
            .copied()
            .map(field_group_dto)
            .collect(),
    }
}

const fn cache_read_state(value: MetadataCacheReadState) -> MetadataCacheReadStateDto {
    match value {
        MetadataCacheReadState::Fresh => MetadataCacheReadStateDto::Fresh,
        MetadataCacheReadState::StaleWhileRefreshing => {
            MetadataCacheReadStateDto::StaleWhileRefreshing
        }
        MetadataCacheReadState::StaleOnError => MetadataCacheReadStateDto::StaleOnError,
        MetadataCacheReadState::Expired => MetadataCacheReadStateDto::Expired,
        MetadataCacheReadState::Invalidated => MetadataCacheReadStateDto::Invalidated,
        MetadataCacheReadState::PartitionDenied => MetadataCacheReadStateDto::PartitionDenied,
    }
}

const fn cache_purpose(value: MetadataCachePurpose) -> MetadataCachePurposeDto {
    match value {
        MetadataCachePurpose::MetadataEnrichment => MetadataCachePurposeDto::MetadataEnrichment,
        MetadataCachePurpose::DisplayProjection => MetadataCachePurposeDto::DisplayProjection,
        MetadataCachePurpose::RatingLookup => MetadataCachePurposeDto::RatingLookup,
        MetadataCachePurpose::OfflineRead => MetadataCachePurposeDto::OfflineRead,
    }
}

const fn classification(value: MetadataDataClassification) -> MetadataDataClassificationDto {
    match value {
        MetadataDataClassification::Public => MetadataDataClassificationDto::Public,
        MetadataDataClassification::Internal => MetadataDataClassificationDto::Internal,
        MetadataDataClassification::Confidential => MetadataDataClassificationDto::Confidential,
        MetadataDataClassification::Restricted => MetadataDataClassificationDto::Restricted,
    }
}

const fn invalidation_reason(
    value: MetadataCacheInvalidationReason,
) -> MetadataCacheInvalidationReasonDto {
    match value {
        MetadataCacheInvalidationReason::ProviderConfigurationChanged => {
            MetadataCacheInvalidationReasonDto::ProviderConfigurationChanged
        }
        MetadataCacheInvalidationReason::CredentialRotated => {
            MetadataCacheInvalidationReasonDto::CredentialRotated
        }
        MetadataCacheInvalidationReason::ProjectionPolicyChanged => {
            MetadataCacheInvalidationReasonDto::ProjectionPolicyChanged
        }
        MetadataCacheInvalidationReason::TermsChanged => {
            MetadataCacheInvalidationReasonDto::TermsChanged
        }
        MetadataCacheInvalidationReason::ExplicitRetraction => {
            MetadataCacheInvalidationReasonDto::ExplicitRetraction
        }
    }
}

fn cache_entry_dto(view: &MetadataCacheReadView) -> MetadataCacheEntryDto {
    let entry = view.entry();
    let key = entry.key();
    MetadataCacheEntryDto {
        key: MetadataCacheKeyDto {
            provider_id: key.provider_id().as_str().to_owned(),
            credential_reference_version: key.credential_reference_version(),
            record_id: key.record_id().to_string(),
            resolved_provider_route: key.resolved_provider_route().to_owned(),
            grain: key.grain().as_str().to_owned(),
            source_namespace: key.source_namespace().as_str().to_owned(),
            source_identifier: key.source_identifier().to_owned(),
            locale: key.locale().map(|value| value.as_str().to_owned()),
            region: key.region().map(|value| value.as_str().to_owned()),
            field_group: field_group_dto(key.field_group()),
            settings_fingerprint: key.settings_fingerprint().as_str().to_owned(),
            configuration_digest: key.configuration_digest().as_str().to_owned(),
            schema_version: key.schema_version(),
            purpose: cache_purpose(key.purpose()),
            terms_revision: key.terms_revision().to_owned(),
            classification: classification(key.classification()),
        },
        claim_ids: entry.claim_ids().iter().map(ToString::to_string).collect(),
        created_at: entry.created_at().to_rfc3339(),
        fresh_until: entry.fresh_until().to_rfc3339(),
        stale_while_refreshing_until: entry.stale_while_refreshing_until().to_rfc3339(),
        stale_on_error_until: entry.stale_on_error_until().to_rfc3339(),
        invalidation: entry
            .invalidation()
            .map(|value| MetadataCacheInvalidationDto {
                reason: invalidation_reason(value.reason()),
                invalidated_at: value.invalidated_at().to_rfc3339(),
            }),
        read_state: cache_read_state(view.state()),
    }
}

fn attribution_dto(value: &MetadataAttribution) -> MetadataAttributionDto {
    MetadataAttributionDto {
        provider_id: value.provider_id().as_str().to_owned(),
        text: value.text().to_owned(),
        documentation_url: value.documentation_url().to_owned(),
    }
}

pub fn metadata_projection_response(view: &MetadataProjectionView) -> MetadataProjectionResponse {
    MetadataProjectionResponse {
        profile_id: view.profile_id().to_string(),
        record_id: view.record_id().to_string(),
        policy: enrichment_policy_dto(view.enrichment_policy()),
        fields: view.fields().iter().map(projection_dto).collect(),
        ratings: view.ratings().iter().map(rating_claim_dto).collect(),
        cache_entries: view.cache_entries().iter().map(cache_entry_dto).collect(),
        attributions: view.attributions().iter().map(attribution_dto).collect(),
    }
}

pub fn refresh_metadata_claims_response(
    record_id: RecordId,
    provider_id: &str,
    outcome: &RefreshMetadataClaimsOutcome,
) -> RefreshMetadataClaimsResponse {
    RefreshMetadataClaimsResponse {
        record_id: record_id.to_string(),
        provider_id: provider_id.to_owned(),
        claims: outcome.field_claims().iter().map(field_claim_dto).collect(),
        ratings: outcome
            .rating_claims()
            .iter()
            .map(rating_claim_dto)
            .collect(),
        projections: outcome.projections().iter().map(projection_dto).collect(),
        cache_entries: outcome
            .cache_entries()
            .iter()
            .map(cache_entry_dto)
            .collect(),
        attributions: outcome.attributions().iter().map(attribution_dto).collect(),
    }
}

pub fn metadata_projection_configuration_response(
    outcome: &ConfigureMetadataProjectionOutcome,
) -> MetadataProjectionConfigurationResponse {
    MetadataProjectionConfigurationResponse {
        policy: enrichment_policy_dto(outcome.enrichment_policy()),
        invalidated_cache_entries: outcome.invalidated_cache_entries(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_request_rejects_unknown_fields() {
        let value = serde_json::json!({
            "record_id": "rec_018f0e0e7f7b70008000000000000000",
            "provider_id": "tmdb",
            "field_groups": ["basic_info"],
            "locale": "en-IE",
            "region": "IE",
            "mode": "revalidate",
            "credential": "must-never-be-accepted"
        });
        assert!(serde_json::from_value::<RefreshMetadataClaimsRequest>(value).is_err());
    }

    #[test]
    fn configuration_is_profile_bound_and_accepts_no_profile_id() {
        let value = serde_json::json!({
            "profile_id": "prf_018f0e0e7f7b70008000000000000000",
            "preferred_provider_id": null,
            "preferred_locale": null,
            "original_locale": null,
            "allow_english_fallback": false,
            "last_known_good": "allow",
            "region": null,
            "enabled_field_groups": [],
            "overrides": []
        });
        assert!(serde_json::from_value::<ConfigureMetadataProjectionRequest>(value).is_err());
    }

    #[test]
    fn override_operations_enforce_their_wire_shape() {
        let record_id = "rec_018f0e0e7f7b70008000000000000000";
        for invalid in [
            serde_json::json!({
                "operation": "set",
                "record_id": record_id,
                "field_key": "core.title"
            }),
            serde_json::json!({
                "operation": "clear",
                "record_id": record_id,
                "field_key": "core.title",
                "value": "must not be accepted"
            }),
        ] {
            assert!(serde_json::from_value::<MetadataOverrideMutationDto>(invalid).is_err());
        }
        assert!(
            serde_json::from_value::<MetadataOverrideMutationDto>(serde_json::json!({
                "operation": "set",
                "record_id": record_id,
                "field_key": "core.title",
                "value": "Replacement"
            }))
            .is_ok()
        );
    }

    #[test]
    fn cache_contract_has_no_secret_or_raw_response_field() {
        let schema = schemars::schema_for!(MetadataCacheEntryDto);
        let text = serde_json::to_string(&schema).expect("serializable cache schema");
        assert!(!text.contains("credential_secret"));
        assert!(!text.contains("raw_response"));
        assert!(!text.contains("access_token"));
    }
}
