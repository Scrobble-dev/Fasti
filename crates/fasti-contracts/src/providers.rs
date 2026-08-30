use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKindDto {
    Metadata,
    Ratings,
    Catalog,
    Tracking,
    Identity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRequirementDto {
    None,
    OptionalApiKey,
    ApiKey,
    BearerToken,
    BasicAuth,
    Oauth2,
    UserAgentOnly,
    CustomHeader,
    OperatorSecretMount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentialStateDto {
    NotRequired,
    Optional,
    Missing,
    StoredUnverified,
    Valid,
    Invalid,
    Expired,
    Unavailable,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentialSourceDto {
    None,
    Environment,
    CredentialStore,
    OperatorSecretMount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapabilityStateDto {
    Available,
    Degraded,
    Unavailable,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCheckStateDto {
    NeverRun,
    Passed,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderCheckDto {
    pub state: ProviderCheckStateDto,
    #[schema(format = DateTime)]
    pub checked_at: Option<String>,
    pub safe_problem_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilityDto {
    pub capability_id: String,
    pub purpose: String,
    pub credential_requirement: CredentialRequirementDto,
    pub credential_state: ProviderCredentialStateDto,
    pub credential_source: ProviderCredentialSourceDto,
    pub state: ProviderCapabilityStateDto,
    #[schema(minimum = 0)]
    pub version: u64,
    pub writable: bool,
    pub testable: bool,
    pub health: ProviderCheckDto,
    pub credential_test: ProviderCheckDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderDescriptorDto {
    pub provider_id: String,
    pub display_name: String,
    pub provider_kind: ProviderKindDto,
    pub documentation_url: String,
    pub attribution: String,
    pub supported_media_grains: Vec<String>,
    pub capabilities: Vec<ProviderCapabilityDto>,
    pub network_hosts: Vec<String>,
    pub locale_support: bool,
    pub region_support: bool,
    pub identity_namespaces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListProvidersResponse {
    pub providers: Vec<ProviderDescriptorDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigureProviderCredentialRequest {
    #[schemars(length(min = 1, max = 4096))]
    #[schema(min_length = 1, max_length = 4096, write_only)]
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilityResponse {
    pub provider_id: String,
    pub capability: ProviderCapabilityDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderHealthResponse {
    pub provider_id: String,
    pub capabilities: Vec<ProviderCapabilityDto>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::generate::SchemaSettings;

    #[test]
    fn credential_request_is_strict_and_bounded() {
        assert!(serde_json::from_str::<ConfigureProviderCredentialRequest>(
            r#"{"secret":"token","extra":true}"#
        )
        .is_err());

        let schema = SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<ConfigureProviderCredentialRequest>();
        let value = serde_json::to_value(schema).expect("serializable schema");
        assert_eq!(
            value.pointer("/properties/secret/minLength"),
            Some(&serde_json::json!(1))
        );
        assert_eq!(
            value.pointer("/properties/secret/maxLength"),
            Some(&serde_json::json!(4096))
        );
    }

    #[test]
    fn provider_responses_have_no_secret_or_reference_field() {
        fn has_key(value: &serde_json::Value, forbidden: &str) -> bool {
            match value {
                serde_json::Value::Object(object) => {
                    object.contains_key(forbidden)
                        || object.values().any(|value| has_key(value, forbidden))
                }
                serde_json::Value::Array(values) => {
                    values.iter().any(|value| has_key(value, forbidden))
                }
                _ => false,
            }
        }

        let schema = SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<ListProvidersResponse>();
        let value = serde_json::to_value(schema).expect("serializable schema");
        for forbidden in ["secret", "credential_reference", "configuration_digest"] {
            assert!(!has_key(&value, forbidden), "response exposes {forbidden}");
        }
    }
}
