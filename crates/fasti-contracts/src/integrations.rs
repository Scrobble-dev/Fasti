use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

/// Provider-neutral webhook body used by integrations that support a custom
/// JSON template (Tautulli and the Jellyfin Webhook plugin, for example).
///
/// This is transport evidence. Fasti still derives durable identity and the
/// source client from the authenticated credential and provider-specific
/// adapter route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationObservationRequest {
    /// Stable provider event identity. Retries must reuse this value.
    #[schemars(length(min = 1, max = 256))]
    #[schema(min_length = 1, max_length = 256)]
    pub source_event_id: String,
    /// RFC 3339 time at which the adapter observed the event.
    pub observed_at: String,
    /// Optional provider-claimed event time.
    pub occurred_at: Option<String>,
    /// `movie`, `episode`, `track`, or another adapter-supported item kind.
    #[schemars(length(min = 1, max = 32))]
    #[schema(min_length = 1, max_length = 32)]
    pub item_type: String,
    /// Human-readable evidence only. It is never used as an irreversible
    /// identity key.
    #[schemars(length(max = 512))]
    #[schema(max_length = 512)]
    pub title: Option<String>,
    /// Optional series title retained as evidence for episode observations.
    #[schemars(length(max = 512))]
    #[schema(max_length = 512)]
    pub series_title: Option<String>,
    pub season_number: Option<u32>,
    pub episode_number: Option<u32>,
    /// Chronicle ingress currently accepts only complete occurrences.
    pub completed: bool,
    pub position_seconds: Option<u64>,
    pub duration_seconds: Option<u64>,
    /// Provider IDs for the observed item. Keys are lower-case provider names
    /// such as `imdb`, `tmdb`, `tvdb`, `musicbrainz`, or `jellyfin`. Bounded
    /// to 16 here per-field, but `normalize_template_request` additionally
    /// rejects `provider_ids` and `series_provider_ids` combined exceeding
    /// 16 (a bounded-work guard, not expressible as a JSON Schema constraint
    /// across two sibling properties) -- a request with both maps near this
    /// per-field limit can be schema-valid and still receive a runtime 422.
    #[serde(default)]
    #[schemars(length(max = 16), extend("maxProperties" = 16))]
    pub provider_ids: BTreeMap<String, String>,
    /// Provider IDs for the parent series when `item_type` is `episode`. See
    /// `provider_ids`'s doc comment for the combined-with-`provider_ids`
    /// runtime bound this per-field schema limit doesn't capture.
    #[serde(default)]
    #[schemars(length(max = 16), extend("maxProperties" = 16))]
    pub series_provider_ids: BTreeMap<String, String>,
    /// Safe source binding clues. They are recorded as evidence and can also be
    /// checked by a configured adapter before mutation.
    #[schemars(length(max = 128))]
    #[schema(max_length = 128)]
    pub server_id: Option<String>,
    #[schemars(length(max = 128))]
    #[schema(max_length = 128)]
    pub user_id: Option<String>,
    #[schemars(length(max = 128))]
    #[schema(max_length = 128)]
    pub device_id: Option<String>,
}

/// Runtime status exposed to the trusted workbench. It intentionally excludes
/// credentials and raw provider payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationStatusDto {
    pub id: String,
    pub label: String,
    pub state: String,
    pub available: bool,
    pub endpoint_ready: bool,
    pub setup_action: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegrationStatusListResponse {
    #[schemars(length(max = 16))]
    #[schema(max_items = 16)]
    pub integrations: Vec<IntegrationStatusDto>,
}
