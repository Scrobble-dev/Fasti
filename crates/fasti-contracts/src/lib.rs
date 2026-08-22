//! Shared public data-transfer objects.
//!
//! This crate contains representation, not domain policy. It grows only when a
//! body finalizes a public shape. Domain entities and application problems stay
//! inward; adapters map them through these DTOs.

use fasti_application::{FastiProblem, ProblemCode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

mod conformance;
mod generated_capability_ids;

pub use conformance::*;
pub use generated_capability_ids::public_capability_id;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProblemActionDto {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ViolationDto {
    pub code: String,
    pub pointer: String,
    pub reason: String,
    pub expected: String,
    pub actual: Option<String>,
}

/// RFC 9457 representation of the one application problem model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    pub code: String,
    pub capability_id: String,
    pub safe_state: String,
    pub retryability: String,
    pub next_actions: Vec<ProblemActionDto>,
    pub correlation_id: String,
    pub param: Option<String>,
    pub actual: Option<String>,
    pub violations: Vec<ViolationDto>,
}

impl ProblemDetails {
    pub fn from_application(
        problem: &FastiProblem,
        capability_id: &str,
        documentation_base: &str,
    ) -> Self {
        let documentation_path = problem
            .documentation_path()
            .unwrap_or("v1/problems/unknown");
        Self {
            type_uri: format!(
                "{}/{}",
                documentation_base.trim_end_matches('/'),
                documentation_path
            ),
            title: title_for(problem.code()).to_owned(),
            status: status_for(problem.code()),
            detail: problem.message().to_owned(),
            code: problem.code().as_str().to_owned(),
            capability_id: capability_id.to_owned(),
            safe_state: problem.safe_state().as_str().to_owned(),
            retryability: problem.retryability().as_str().to_owned(),
            next_actions: problem
                .next_actions()
                .iter()
                .map(|action| ProblemActionDto {
                    id: action.id().to_owned(),
                    label: action.label().to_owned(),
                })
                .collect(),
            correlation_id: problem.correlation_id().to_string(),
            param: problem.param().map(str::to_owned),
            actual: problem.actual().map(str::to_owned),
            violations: problem
                .violations()
                .iter()
                .map(|violation| ViolationDto {
                    code: violation.code().to_owned(),
                    pointer: violation.pointer().to_owned(),
                    reason: violation.reason().to_owned(),
                    expected: violation.expected().to_owned(),
                    actual: violation.actual().map(str::to_owned),
                })
                .collect(),
        }
    }
}

const fn status_for(code: ProblemCode) -> u16 {
    match code {
        ProblemCode::CapacityExceeded => 507,
        ProblemCode::Forbidden => 403,
        ProblemCode::ReceiptNotFound => 404,
        ProblemCode::IdempotencyConflict => 409,
        ProblemCode::InvalidIdentifier
        | ProblemCode::InvalidObservation
        | ProblemCode::InvalidTime
        | ProblemCode::ValidationFailed => 422,
        ProblemCode::CapabilityUnavailable => 501,
        ProblemCode::ContractDrift => 500,
    }
}

const fn title_for(code: ProblemCode) -> &'static str {
    match code {
        ProblemCode::CapacityExceeded => "Capacity exceeded",
        ProblemCode::CapabilityUnavailable => "Capability unavailable",
        ProblemCode::ContractDrift => "Contract drift",
        ProblemCode::Forbidden => "Forbidden",
        ProblemCode::IdempotencyConflict => "Idempotency conflict",
        ProblemCode::InvalidIdentifier => "Invalid identifier",
        ProblemCode::InvalidObservation => "Invalid observation",
        ProblemCode::InvalidTime => "Invalid time",
        ProblemCode::ReceiptNotFound => "Receipt not found",
        ProblemCode::ValidationFailed => "Validation failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_application::{CapabilityKey, FastiProblem};
    use fasti_domain::RequestCorrelationId;
    use schemars::generate::SchemaSettings;

    #[test]
    fn problem_mapping_uses_registry_owned_capability_id() {
        let problem = FastiProblem::capability_unavailable(
            CapabilityKey::RestoreWorkspace,
            RequestCorrelationId::new_v7(),
        );
        let dto = ProblemDetails::from_application(
            &problem,
            "portability.workspace.restore",
            "https://fasti.scrobble.dev",
        );
        assert_eq!(dto.status, 501);
        assert_eq!(dto.capability_id, "portability.workspace.restore");
        assert_eq!(dto.safe_state, "no_mutation");
        assert_eq!(dto.next_actions.len(), 1);
    }

    #[test]
    fn unknown_problem_fields_are_rejected() {
        let value = r#"{
          "type":"https://fasti.scrobble.dev/problems/x",
          "title":"x","status":400,"detail":"x","code":"x",
          "capability_id":"system.health","safe_state":"no_mutation",
          "retryability":"not_retryable","next_actions":[],
          "correlation_id":"req_018f0e0e7f7b70008000000000000000",
          "param":null,"actual":null,"violations":[],"extra":true
        }"#;
        assert!(serde_json::from_str::<ProblemDetails>(value).is_err());
    }

    #[test]
    fn health_json_schema_explicitly_uses_draft_2020_12() {
        let schema = SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<HealthResponse>();
        let value = serde_json::to_value(schema).expect("serializable JSON Schema");

        assert_eq!(
            value.get("$schema").and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema")
        );
    }
}
