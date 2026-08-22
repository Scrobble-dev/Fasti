//! Shared public data-transfer objects.
//!
//! This crate contains representation, not domain policy. It grows only when a
//! body finalizes a public shape. Domain entities and application problems stay
//! inward; adapters map them through these DTOs.

use fasti_application::FastiProblem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

fn explicit_null_openapi() -> utoipa::openapi::schema::Object {
    utoipa::openapi::schema::ObjectBuilder::new()
        .schema_type(utoipa::openapi::schema::Type::Null)
        .build()
}

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
    #[schema(schema_with = explicit_null_openapi)]
    pub actual: (),
}

/// RFC 9457 representation of the one application problem model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    #[schema(minimum = 0, maximum = 65535, format = "uint16")]
    pub status: u16,
    pub detail: String,
    pub code: String,
    pub capability_id: String,
    pub safe_state: String,
    pub retryability: String,
    #[schemars(length(equal = 1))]
    #[schema(min_items = 1, max_items = 1)]
    pub next_actions: Vec<ProblemActionDto>,
    #[schemars(regex(pattern = r"^req_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"))]
    #[schema(pattern = r"^req_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")]
    pub correlation_id: String,
    pub param: Option<String>,
    #[schema(schema_with = explicit_null_openapi)]
    pub actual: (),
    #[schemars(length(max = 32))]
    #[schema(max_items = 32)]
    pub violations: Vec<ViolationDto>,
}

impl ProblemDetails {
    pub fn from_application(
        problem: &FastiProblem,
        capability_id: &str,
        documentation_base: &str,
    ) -> Self {
        Self {
            type_uri: format!(
                "{}/{}",
                documentation_base.trim_end_matches('/'),
                problem.documentation_path()
            ),
            title: problem.title().to_owned(),
            status: problem.status(),
            detail: problem.message().into_owned(),
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
            actual: (),
            violations: problem
                .violations()
                .iter()
                .map(|violation| ViolationDto {
                    code: violation.code().to_owned(),
                    pointer: violation.pointer().to_owned(),
                    reason: violation.reason().to_owned(),
                    expected: violation.expected().to_owned(),
                    actual: (),
                })
                .collect(),
        }
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

    #[test]
    fn problem_schema_freezes_canonical_action_and_violation_bounds() {
        let schema = SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<ProblemDetails>();
        let value = serde_json::to_value(schema).expect("serializable JSON Schema");
        assert_eq!(
            value.pointer("/properties/next_actions/minItems"),
            Some(&serde_json::json!(1))
        );
        assert_eq!(
            value.pointer("/properties/next_actions/maxItems"),
            Some(&serde_json::json!(1))
        );
        assert_eq!(
            value.pointer("/properties/violations/maxItems"),
            Some(&serde_json::json!(32))
        );
        assert_eq!(
            value.pointer("/properties/actual/type"),
            Some(&serde_json::json!("null"))
        );
        assert_eq!(
            value.pointer("/$defs/ViolationDto/properties/actual/type"),
            Some(&serde_json::json!("null"))
        );
        assert_eq!(
            value.pointer("/properties/correlation_id/pattern"),
            Some(&serde_json::json!(
                "^req_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
            ))
        );
    }
}
