//! Public B1 conformance transport shapes.
//!
//! These DTOs describe the executable, in-memory conformance surface only.
//! They do not imply that the B2 durable runtime exists. Profile selection,
//! credential administration, and listener configuration are intentionally
//! absent: their registry bindings remain incomplete until their application
//! semantics are implemented rather than guessed at this boundary.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAvailabilityDto {
    FixtureOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DurabilityDto {
    None,
}

/// Required on every successful conformance response so fixture state cannot
/// be mistaken for durable production state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConformanceMarkerDto {
    pub availability: RuntimeAvailabilityDto,
    pub durability: DurabilityDto,
}

impl ConformanceMarkerDto {
    pub const FIXTURE_ONLY: Self = Self {
        availability: RuntimeAvailabilityDto::FixtureOnly,
        durability: DurabilityDto::None,
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityLifecycleDto {
    pub introduced_in: String,
    pub contract_state: String,
    pub runtime_availability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityUatDto {
    pub id: String,
    pub relationship: String,
    pub owner_body: String,
    pub reason: String,
}

/// Registry-owned public capability descriptor. Internal application keys are
/// intentionally not exposed at the transport boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescriptorDto {
    pub id: String,
    pub bounded_context: String,
    pub contract_body: String,
    pub runtime_body: String,
    pub authorization: String,
    pub lifecycle: CapabilityLifecycleDto,
    pub surface_profile: String,
    pub scopes: Vec<String>,
    pub problems: Vec<String>,
    pub examples: Vec<String>,
    pub uat: Vec<CapabilityUatDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilitySurfaceDispositionDto {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding_visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GeneratedCapabilitiesDto {
    pub capabilities: Vec<CapabilityDescriptorDto>,
    pub capability_base_uri: String,
    pub contract_version: String,
    pub surface_profiles: BTreeMap<String, BTreeMap<String, CapabilitySurfaceDispositionDto>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDiscoveryResponse {
    pub conformance: ConformanceMarkerDto,
    pub contract_version: String,
    pub capability_base_uri: String,
    pub surface_profiles: BTreeMap<String, BTreeMap<String, CapabilitySurfaceDispositionDto>>,
    pub capabilities: Vec<CapabilityDescriptorDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializeNodeRequest {}

/// Opaque bootstrap proof. It is delivered in a JSON body exactly once and is
/// intentionally not `Debug` so diagnostics cannot print it accidentally.
#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializeNodeResponse {
    pub conformance: ConformanceMarkerDto,
    #[schemars(
        length(equal = 64),
        regex(pattern = r"^[0-9a-f]{64}$"),
        extend("format" = "opaque-secret")
    )]
    #[schema(
        min_length = 64,
        max_length = 64,
        pattern = r"^[0-9a-f]{64}$",
        format = "opaque-secret"
    )]
    pub initialization_proof: String,
}

/// Bootstrap proof is body-only. It must never be placed in a URL or log.
#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EnrollFirstClientRequest {
    #[schemars(
        length(equal = 64),
        regex(pattern = r"^[0-9a-f]{64}$"),
        extend("format" = "opaque-secret")
    )]
    #[schema(
        min_length = 64,
        max_length = 64,
        pattern = r"^[0-9a-f]{64}$",
        format = "opaque-secret"
    )]
    pub initialization_proof: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub enum CredentialSchemeDto {
    Bearer,
}

/// Credential plaintext is returned only by the first successful enrollment.
/// Deliberately omits `Debug` and `Clone`.
#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EnrollFirstClientResponse {
    pub conformance: ConformanceMarkerDto,
    pub credential_scheme: CredentialSchemeDto,
    #[schemars(
        length(equal = 64),
        regex(pattern = r"^[0-9a-f]{64}$"),
        extend("format" = "opaque-secret")
    )]
    #[schema(
        min_length = 64,
        max_length = 64,
        pattern = r"^[0-9a-f]{64}$",
        format = "opaque-secret"
    )]
    pub credential: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimedPrecisionDto {
    Date,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClaimedTrustDto {
    SourceClaim,
    DeviceObserved,
    UserEntered,
    Inferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OccurredTimeDto {
    #[schemars(
        length(min = 10, max = 35),
        regex(pattern = r"^(?:[0-9]{4}-[0-9]{2}-[0-9]{2}|[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2}))$"),
        extend("format" = "iso-date-or-rfc3339")
    )]
    #[schema(
        min_length = 10,
        max_length = 35,
        pattern = r"^(?:[0-9]{4}-[0-9]{2}-[0-9]{2}|[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2}))$",
        format = "iso-date-or-rfc3339"
    )]
    pub original: String,
    pub precision: ClaimedPrecisionDto,
    pub trust: ClaimedTrustDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservedTimeDto {
    #[schemars(
        length(min = 20, max = 35),
        regex(pattern = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2})$"),
        extend("format" = "date-time")
    )]
    #[schema(
        min_length = 20,
        max_length = 35,
        pattern = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2})$",
        format = DateTime
    )]
    pub original: String,
    pub precision: ClaimedPrecisionDto,
    pub trust: ClaimedTrustDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReferenceDto {
    #[schemars(
        length(equal = 36),
        regex(pattern = r"^evd_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"),
        extend("format" = "fasti-evidence-id")
    )]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^evd_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-evidence-id"
    )]
    pub evidence_id: String,
    #[schemars(
        length(equal = 71),
        regex(pattern = r"^sha256:[0-9a-f]{64}$"),
        extend("format" = "sha256")
    )]
    #[schema(
        min_length = 71,
        max_length = 71,
        pattern = r"^sha256:[0-9a-f]{64}$",
        format = "sha256"
    )]
    pub digest: String,
    #[schemars(range(min = 0))]
    #[schema(minimum = 0)]
    pub byte_length: u64,
}

/// Source client identity and server-owned timestamps are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptObservationRequest {
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
    pub occurred_at: Option<OccurredTimeDto>,
    pub observed_at: ObservedTimeDto,
    pub evidence: EvidenceReferenceDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptDispositionDto {
    Committed,
    Replayed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ObservationResolutionDto {
    Unresolved,
}

/// A capability-bound receipt. Record and occurrence IDs are impossible in
/// this shape because identity resolution does not occur during acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservationReceiptDto {
    #[schemars(length(equal = 36), regex(pattern = r"^rcp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-receipt-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^rcp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-receipt-id"
    )]
    pub receipt_id: String,
    #[schemars(length(equal = 35), regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-operation-id"))]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-operation-id"
    )]
    pub operation_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^wsp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-workspace-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^wsp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-workspace-id"
    )]
    pub workspace_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^prf_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-profile-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^prf_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-profile-id"
    )]
    pub profile_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^cli_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-client-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^cli_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-client-id"
    )]
    pub source_client_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^obs_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-observation-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^obs_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-observation-id"
    )]
    pub observation_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^evd_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-evidence-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^evd_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
        format = "fasti-evidence-id"
    )]
    pub evidence_id: String,
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"), extend("format" = "sha256"))]
    #[schema(
        min_length = 71,
        max_length = 71,
        pattern = r"^sha256:[0-9a-f]{64}$",
        format = "sha256"
    )]
    pub payload_digest: String,
    pub resolution: ObservationResolutionDto,
    #[schemars(length(min = 20, max = 35), regex(pattern = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2})$"), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, pattern = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2})$", format = DateTime)]
    pub received_at: String,
    #[schemars(length(min = 20, max = 35), regex(pattern = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2})$"), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, pattern = r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,9})?(?:Z|[+-][0-9]{2}:[0-9]{2})$", format = DateTime)]
    pub committed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptObservationResponse {
    pub conformance: ConformanceMarkerDto,
    pub disposition: ReceiptDispositionDto,
    pub receipt: ObservationReceiptDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplayReceiptResponse {
    pub conformance: ConformanceMarkerDto,
    pub receipt: ObservationReceiptDto,
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::generate::SchemaSettings;
    use static_assertions::assert_not_impl_any;

    assert_not_impl_any!(InitializeNodeResponse: std::fmt::Debug);
    assert_not_impl_any!(EnrollFirstClientRequest: std::fmt::Debug);
    assert_not_impl_any!(EnrollFirstClientResponse: std::fmt::Debug);

    #[test]
    fn every_request_rejects_unknown_fields() {
        assert!(serde_json::from_str::<InitializeNodeRequest>(r#"{"extra":true}"#).is_err());
        assert!(serde_json::from_str::<EnrollFirstClientRequest>(
            r#"{"initialization_proof":"opaque","extra":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<AcceptObservationRequest>(
            r#"{
              "operation_id":"op_value",
              "occurred_at":null,
              "observed_at":{"original":"2026-08-22T00:00:00Z","precision":"second","trust":"device_observed"},
              "evidence":{"evidence_id":"evd_value","digest":"sha256:value","byte_length":1},
              "extra":true
            }"#
        )
        .is_err());
    }

    #[test]
    fn conformance_schema_is_explicitly_draft_2020_12() {
        let schema = SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<AcceptObservationRequest>();
        let value = serde_json::to_value(schema).expect("serializable JSON Schema");
        assert_eq!(
            value.get("$schema").and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema")
        );

        assert_eq!(
            value.pointer("/properties/operation_id/minLength"),
            Some(&serde_json::json!(35))
        );
        assert_eq!(
            value.pointer("/properties/operation_id/maxLength"),
            Some(&serde_json::json!(35))
        );
        assert_eq!(
            value
                .pointer("/properties/operation_id/pattern")
                .and_then(serde_json::Value::as_str),
            Some(r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
        );
        assert_eq!(
            value
                .pointer("/$defs/EvidenceReferenceDto/properties/digest/pattern")
                .and_then(serde_json::Value::as_str),
            Some(r"^sha256:[0-9a-f]{64}$")
        );
        assert_eq!(
            value
                .pointer("/$defs/ObservedTimeDto/properties/original/format")
                .and_then(serde_json::Value::as_str),
            Some("date-time")
        );
        assert_eq!(
            value.pointer("/$defs/ObservedTimeDto/properties/original/minLength"),
            Some(&serde_json::json!(20))
        );
    }

    #[test]
    fn bootstrap_and_credential_schemas_are_fixed_lowercase_secret_values() {
        for value in [
            SchemaSettings::draft2020_12()
                .into_generator()
                .into_root_schema_for::<InitializeNodeResponse>(),
            SchemaSettings::draft2020_12()
                .into_generator()
                .into_root_schema_for::<EnrollFirstClientRequest>(),
            SchemaSettings::draft2020_12()
                .into_generator()
                .into_root_schema_for::<EnrollFirstClientResponse>(),
        ] {
            let value = serde_json::to_value(value).expect("serializable secret schema");
            let properties = value["properties"].as_object().expect("object properties");
            let secret = properties
                .get("initialization_proof")
                .or_else(|| properties.get("credential"))
                .expect("secret property");
            assert_eq!(secret["minLength"], 64);
            assert_eq!(secret["maxLength"], 64);
            assert_eq!(secret["pattern"], r"^[0-9a-f]{64}$");
            assert_eq!(secret["format"], "opaque-secret");
        }
    }
}
