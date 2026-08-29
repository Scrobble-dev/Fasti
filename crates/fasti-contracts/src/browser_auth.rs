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
    #[schemars(range(min = 5, max = 86400))]
    #[schema(minimum = 5, maximum = 86400)]
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
