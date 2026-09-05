//! Durable local bootstrap DTOs.

use crate::CredentialSchemeDto;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// One-time proof returned by durable node initialization.
///
/// Deliberately omits `Debug` and `Clone` so diagnostics cannot print the
/// secret accidentally.
#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NodeInitializationResponse {
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

/// One-time credential returned by durable first-client enrollment.
///
/// Deliberately omits `Debug` and `Clone` so diagnostics cannot print the
/// secret accidentally.
#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientEnrollmentResponse {
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

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_not_impl_any;

    assert_not_impl_any!(NodeInitializationResponse: std::fmt::Debug, Clone);
    assert_not_impl_any!(ClientEnrollmentResponse: std::fmt::Debug, Clone);

    #[test]
    fn secret_responses_reject_unknown_fields() {
        assert!(serde_json::from_str::<NodeInitializationResponse>(
            r#"{"initialization_proof":"0000000000000000000000000000000000000000000000000000000000000000","extra":true}"#
        )
        .is_err());
        assert!(serde_json::from_str::<ClientEnrollmentResponse>(
            r#"{"credential_scheme":"Bearer","credential":"0000000000000000000000000000000000000000000000000000000000000000","extra":true}"#
        )
        .is_err());
    }
}
