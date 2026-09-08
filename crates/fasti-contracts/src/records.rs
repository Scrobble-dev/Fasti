use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

fn occurred_time_dto(claim: &fasti_domain::ClaimedTime) -> crate::OccurredTimeDto {
    use fasti_domain::{ClaimedPrecision as Precision, ClaimedTrust as Trust};
    crate::OccurredTimeDto {
        original: claim.original().to_owned(),
        precision: match claim.precision() {
            Precision::Date => crate::ClaimedPrecisionDto::Date,
            Precision::Second => crate::ClaimedPrecisionDto::Second,
            Precision::Millisecond => crate::ClaimedPrecisionDto::Millisecond,
            Precision::Microsecond => crate::ClaimedPrecisionDto::Microsecond,
            Precision::Nanosecond => crate::ClaimedPrecisionDto::Nanosecond,
        },
        trust: match claim.trust() {
            Trust::SourceClaim => crate::ClaimedTrustDto::SourceClaim,
            Trust::DeviceObserved => crate::ClaimedTrustDto::DeviceObserved,
            Trust::UserEntered => crate::ClaimedTrustDto::UserEntered,
            Trust::Inferred => crate::ClaimedTrustDto::Inferred,
        },
    }
}

fn resolved_field_dto(field: &fasti_domain::ResolvedField) -> ResolvedFieldDto {
    ResolvedFieldDto {
        tier: serde_json::to_value(field.tier())
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default(),
        value: field.value().map(ToOwned::to_owned),
        source: field.source().map(ToString::to_string),
        is_stale: field.is_stale(),
    }
}

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

impl From<fasti_application::RecordSummary> for RecordSummaryDto {
    fn from(summary: fasti_application::RecordSummary) -> Self {
        Self {
            record_id: summary.record_id().to_string(),
            grain: summary.grain().as_str().to_owned(),
            status: serde_json::to_value(summary.status())
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "active".to_owned()),
            title: resolved_field_dto(summary.title()),
            poster: resolved_field_dto(summary.poster()),
            original_title: Some(resolved_field_dto(summary.original_title())),
            overview: Some(resolved_field_dto(summary.overview())),
            release_year: Some(resolved_field_dto(summary.release_year())),
            identifiers: summary
                .identifiers()
                .iter()
                .map(|identifier| RecordIdentifierDto {
                    namespace: identifier.namespace().to_string(),
                    grain: identifier.grain().as_str().to_owned(),
                    value: identifier.value().to_owned(),
                })
                .collect(),
            latest_activity: summary.latest_activity().map(|activity| RecordActivityDto {
                occurred_at: activity
                    .occurred_at()
                    .map(|value| occurred_time_dto(value.claim())),
                interpretation_state: serde_json::to_value(activity.interpretation_state())
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default(),
            }),
        }
    }
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
