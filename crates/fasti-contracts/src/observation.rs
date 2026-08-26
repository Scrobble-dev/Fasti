use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservationIdentifierInput {
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

/// Durable provider-neutral ingress envelope.
///
/// `source_event_id` is source-owned and must remain stable when the same
/// delivery is retried. `operation_id` is the Fasti idempotency key. The full
/// request is stored as immutable evidence before observation acceptance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitObservationRequest {
    #[schemars(
        length(equal = 35),
        regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub operation_id: String,
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub source: String,
    #[schemars(length(min = 1, max = 256))]
    #[schema(min_length = 1, max_length = 256)]
    pub source_event_id: String,
    #[schemars(length(min = 20, max = 35))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub observed_at: String,
    #[schemars(length(min = 10, max = 35))]
    #[schema(min_length = 10, max_length = 35)]
    pub occurred_at: Option<String>,
    pub target_grain: Option<String>,
    #[schemars(length(max = 16))]
    #[schema(max_items = 16)]
    pub identifiers: Vec<ObservationIdentifierInput>,
    #[schemars(length(max = 512))]
    #[schema(max_length = 512)]
    pub title: Option<String>,
    #[schemars(range(min = 0.0, max = 100.0))]
    #[schema(minimum = 0, maximum = 100)]
    pub progress_percent: Option<f64>,
    #[schemars(range(min = 0))]
    #[schema(minimum = 0)]
    pub position_seconds: Option<u64>,
    #[schemars(range(min = 1))]
    #[schema(minimum = 1)]
    pub duration_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitObservationResponse {
    pub disposition: String,
    pub receipt_id: String,
    pub operation_id: String,
    pub workspace_id: String,
    pub profile_id: String,
    pub source_client_id: String,
    pub observation_id: String,
    pub occurrence_id: Option<String>,
    pub interpretation_id: Option<String>,
    pub record_id: Option<String>,
    pub review_item_id: Option<String>,
    pub evidence_id: String,
    pub payload_digest: String,
    pub resolution: String,
    pub received_at: String,
    pub committed_at: String,
}
