use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateBrowserSessionRequest {
    #[schemars(length(min = 3, max = 64))]
    #[schema(min_length = 3, max_length = 64)]
    pub username: String,
    #[schemars(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128, format = "password")]
    pub password: String,
    /// 5-1440 (24h) covers every normal deployment. The published maximum
    /// here is wider than that -- 52,560,000 (100 years), matching apps/
    /// fastid/src/main.rs's DEVELOPMENT_UNBOUNDED_SESSION_MINUTES -- because
    /// the generated SDK's client re-validates outgoing requests against
    /// this exact published schema (packages/sdk/src/generated.ts's
    /// validateOpenApiValue): a narrower published maximum would make the
    /// SDK itself reject the value before a legitimate loopback dev-auto-
    /// login request ever reached the daemon. The runtime bound enforced by
    /// CreateBrowserSessionCommand::try_new is still the actual source of
    /// truth for what a given instance accepts -- only a daemon with the
    /// loopback-gated FASTI_DEVELOPMENT_AUTO_LOGIN convenience active
    /// accepts anything above 1440, and rejects it (422) otherwise.
    #[schemars(range(min = 5, max = 52_560_000))]
    #[schema(minimum = 5, maximum = 52_560_000)]
    pub session_timeout_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserUserDto {
    pub user_id: String,
    pub username: String,
    pub is_admin: bool,
    pub is_test_account: bool,
    pub active: bool,
    #[schema(format = "date-time")]
    pub created_at: String,
    #[schema(format = "date-time")]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserSessionResponse {
    pub user: BrowserUserDto,
    #[schema(format = "date-time")]
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserSessionItemDto {
    pub session_id: String,
    #[schema(format = "date-time")]
    pub created_at: String,
    #[schema(format = "date-time")]
    pub expires_at: String,
    #[schema(format = "date-time")]
    pub last_seen_at: String,
    pub location: String,
    pub device_type: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListBrowserSessionsResponse {
    pub sessions: Vec<BrowserSessionItemDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListBrowserUsersResponse {
    pub users: Vec<BrowserUserDto>,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateBrowserUserRequest {
    #[schemars(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128, format = "password")]
    pub current_password: String,
    #[schemars(length(min = 3, max = 64))]
    #[schema(min_length = 3, max_length = 64)]
    pub username: Option<String>,
    #[schemars(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128, format = "password")]
    pub password: Option<String>,
    pub active: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteBrowserUserRequest {
    #[schemars(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128, format = "password")]
    pub current_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SwitchProfileRequest {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PasskeyDto {
    pub passkey_id: String,
    pub name: String,
    #[schema(format = "date-time")]
    pub created_at: String,
    #[schema(format = "date-time")]
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListPasskeysResponse {
    pub passkeys: Vec<PasskeyDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BeginPasskeyRegistrationResponse {
    pub challenge: String,
    pub rp_name: String,
    pub rp_id: String,
    pub user_id: String,
    pub user_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletePasskeyRegistrationRequest {
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_length = 1, max_length = 64)]
    pub name: String,
    pub credential_id: String,
    pub client_data_json: String,
    pub attestation_object: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BeginPasskeyAuthenticationResponse {
    pub challenge: String,
    pub rp_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompletePasskeyAuthenticationRequest {
    pub credential_id: String,
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TotpStatusDto {
    pub enabled: bool,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EnrollTotpResponse {
    pub secret: String,
    pub otpauth_uri: String,
    pub backup_codes: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfirmTotpRequest {
    #[schemars(length(min = 6, max = 6))]
    #[schema(min_length = 6, max_length = 6)]
    pub code: String,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyTotpRequest {
    #[schemars(length(min = 6, max = 16))]
    #[schema(min_length = 6, max_length = 16)]
    pub code: String,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DisableTotpRequest {
    #[schemars(length(min = 8, max = 128))]
    #[schema(min_length = 8, max_length = 128, format = "password")]
    pub current_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OidcConfigDto {
    pub issuer_url: String,
    pub client_id: String,
    pub pkce_enabled: bool,
    pub scopes: Vec<String>,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SaveOidcConfigRequest {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub pkce_enabled: bool,
    pub scopes: Vec<String>,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OidcDiscoveryRequest {
    pub issuer_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OidcDiscoveryResponse {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_not_impl_any;

    assert_not_impl_any!(CreateBrowserSessionRequest: std::fmt::Debug, Clone);
    assert_not_impl_any!(UpdateBrowserUserRequest: std::fmt::Debug, Clone);
    assert_not_impl_any!(DeleteBrowserUserRequest: std::fmt::Debug, Clone);

    #[test]
    fn password_requests_reject_unknown_fields() {
        assert!(serde_json::from_str::<CreateBrowserSessionRequest>(
            r#"{"username":"testadmin","password":"testadmin","session_timeout_minutes":60,"extra":true}"#,
        )
        .is_err());
    }
}
