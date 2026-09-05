use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRecordRequest {
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub grain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateRecordResponse {
    pub record_id: String,
    pub grain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachIdentifierRequest {
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub record_id: String,
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub namespace: String,
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub grain: String,
    #[schemars(length(min = 1, max = 512))]
    #[schema(min_length = 1, max_length = 512)]
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachIdentifierResponse {
    pub external_identifier_id: String,
    pub record_id: String,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterNamespaceRequest {
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub namespace: String,
    #[schemars(length(min = 1, max = 128))]
    #[schema(min_length = 1, max_length = 128)]
    pub label: String,
    #[schemars(length(min = 1, max = 16))]
    #[schema(min_items = 1, max_items = 16)]
    pub grains: Vec<String>,
    #[schemars(length(min = 1, max = 256))]
    #[schema(min_length = 1, max_length = 256)]
    pub id_pattern: String,
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub normalization: String,
    pub licence_posture: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterNamespaceResponse {
    pub namespace: String,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFieldDto {
    pub tier: String,
    pub value: Option<String>,
    pub source: Option<String>,
    pub is_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordActivityDto {
    /// The full claimed-time structure (original text, precision, trust), not
    /// a collapsed string -- matches the desktop host's `RecordActivityView`
    /// so both surfaces expose the same field shape for the same data.
    pub occurred_at: Option<crate::OccurredTimeDto>,
    pub interpretation_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordIdentifierDto {
    pub namespace: String,
    pub grain: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordSummaryDto {
    pub record_id: String,
    pub grain: String,
    pub status: String,
    pub title: ResolvedFieldDto,
    pub poster: ResolvedFieldDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_title: Option<ResolvedFieldDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<ResolvedFieldDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_year: Option<ResolvedFieldDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub identifiers: Vec<RecordIdentifierDto>,
    pub latest_activity: Option<RecordActivityDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListRecordsResponse {
    pub records: Vec<RecordSummaryDto>,
    /// `true` when more active Records exist in this workspace beyond the
    /// bounded page returned here (see `ListTrackingDispositionsResponse`
    /// for the same pattern on a sibling listing capability).
    pub truncated: bool,
}

#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams,
)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct ListRecordsQueryParameters {
    /// Select one active Record; missing or inaccessible Records return an empty page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        length(equal = 36),
        regex(pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    #[param(
        min_length = 36,
        max_length = 36,
        pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub record_id: Option<String>,
}
