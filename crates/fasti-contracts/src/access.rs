//! Public Account and security DTOs.
//!
//! These representations contain only non-secret, currently established
//! Access state. Browser credentials and TrailBase proof material are carried
//! only by cookies or the private backchannel.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartTrailBaseSignInRequest {
    pub remembered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StartTrailBaseSignInResponse {
    #[schemars(length(min = 1, max = 4096))]
    #[schema(min_length = 1, max_length = 4096)]
    pub authorization_url: String,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct CompleteTrailBaseAuthenticationQuery {
    #[schemars(length(equal = 48), regex(pattern = r"^[A-Za-z0-9]{48}$"))]
    #[schema(min_length = 48, max_length = 48)]
    #[param(min_length = 48, max_length = 48, pattern = "^[A-Za-z0-9]{48}$")]
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TrailBaseContinuationChoiceDto {
    /// Zero-based opaque choice submitted with the unchanged candidate revision.
    #[schemars(range(min = 0, max = 63))]
    #[schema(minimum = 0, maximum = 63)]
    pub choice_ordinal: u8,
    /// One-based workspace number for display only.
    #[schemars(range(min = 1, max = 64))]
    #[schema(minimum = 1, maximum = 64)]
    pub workspace_ordinal: u8,
    /// One-based profile number for display only.
    #[schemars(range(min = 1, max = 64))]
    #[schema(minimum = 1, maximum = 64)]
    pub profile_ordinal: u8,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub workspace_created_at: String,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub profile_created_at: String,
    pub membership_state: AccessMembershipLifecycleDto,
    pub role: AccessWorkspaceRoleDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadTrailBaseContinuationResponse {
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub expires_at: String,
    pub remembered: bool,
    /// Revision submitted unchanged with the chosen zero-based choice ordinal.
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"), extend("format" = "sha256"))]
    #[schema(min_length = 71, max_length = 71, pattern = r"^sha256:[0-9a-f]{64}$")]
    pub candidate_revision: String,
    #[schemars(length(min = 1, max = 64))]
    #[schema(min_items = 1, max_items = 64)]
    pub choices: Vec<TrailBaseContinuationChoiceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CompleteTrailBaseContinuationRequest {
    /// Zero-based opaque choice submitted with the unchanged candidate revision.
    #[schemars(range(min = 0, max = 63))]
    #[schema(minimum = 0, maximum = 63)]
    pub choice_ordinal: u8,
    /// Candidate revision returned by the continuation read, echoed unchanged.
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"), extend("format" = "sha256"))]
    #[schema(min_length = 71, max_length = 71, pattern = r"^sha256:[0-9a-f]{64}$")]
    pub candidate_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectBrowserSessionProfileRequest {
    #[schemars(length(equal = 36), regex(pattern = r"^grt_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-profile-grant-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^grt_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub profile_grant_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserSessionDto {
    #[schemars(length(equal = 36), regex(pattern = r"^ses_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-browser-session-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^ses_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub browser_session_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^wsp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-workspace-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^wsp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub workspace_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^grt_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-profile-grant-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^grt_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub selected_profile_grant_id: String,
    pub is_current: bool,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub created_at: String,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub last_seen_at: String,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub idle_expires_at: String,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub absolute_expires_at: String,
    pub rotation_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadBrowserSessionResponse {
    pub session: BrowserSessionDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListBrowserSessionsResponse {
    #[schemars(length(max = 32))]
    #[schema(max_items = 32)]
    pub sessions: Vec<BrowserSessionDto>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RevokeBrowserSessionsResponse {
    pub revoked_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RotateBrowserSessionResponse {
    pub session: BrowserSessionDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectBrowserSessionProfileResponse {
    pub session: BrowserSessionDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessEvidenceStateDto {
    Loading,
    Empty,
    Unavailable,
    NeedsAttention,
    FailedSafely,
    Verified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessSubjectLifecycleDto {
    Active,
    Disabled,
    Deleted,
    RecoveryPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessMembershipLifecycleDto {
    Invited,
    PendingApproval,
    Active,
    Suspended,
    Removed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessWorkspaceRoleDto {
    Member,
    Administrator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrailBaseActivationStateDto {
    Inactive,
    Active,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrailBaseActivationBlockerDto {
    ReleaseMismatch,
    PhysicalRootIdentityMismatch,
    DeclaredRestore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessAuthenticationMethodDto {
    TrailBasePassword,
    TrailBaseSocial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessEvidenceKindDto {
    CurrentSessionIssued,
    FirstAdministratorBootstrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessCeremonyStateDto {
    Pending,
    Claimed,
    SelectionRequired,
    Completed,
    Cancelled,
    Failed,
    CleanupUncertain,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessCeremonyFailureDto {
    VerifierLostOnRestart,
    ExchangeOutcomeUncertain,
    ExchangeFailed,
    StatusRejected,
    LogoutUncertain,
    LocalAuthorizationDenied,
    LocalPersistenceFailed,
    TrustUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccessFirstRunStepKeyDto {
    AccountConfirmed,
    StrongSignIn,
    Recovery,
    DevicesAndClients,
    ExternalIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessSubjectDto {
    #[schemars(length(equal = 36), regex(pattern = r"^sub_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-auth-subject-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^sub_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub auth_subject_id: String,
    pub lifecycle: AccessSubjectLifecycleDto,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub created_at: String,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessMembershipDto {
    #[schemars(length(equal = 36), regex(pattern = r"^mem_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-membership-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^mem_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub membership_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^wsp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-workspace-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^wsp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub workspace_id: String,
    pub lifecycle: AccessMembershipLifecycleDto,
    pub role: AccessWorkspaceRoleDto,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub created_at: String,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessProfileGrantDto {
    #[schemars(length(equal = 36), regex(pattern = r"^grt_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-profile-grant-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^grt_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub profile_grant_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^prf_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-profile-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^prf_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub profile_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^cli_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-client-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^cli_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub owner_client_id: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BrowserSessionPolicyDto {
    pub idle_timeout_seconds: u64,
    pub browser_lifetime_seconds: u64,
    pub remembered_browser_lifetime_seconds: u64,
    pub last_seen_write_interval_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RecentAuthenticationDto {
    pub state: AccessEvidenceStateDto,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessSessionAuthenticationDto {
    pub method: AccessAuthenticationMethodDto,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub verified_at: String,
    pub activation_generation: u64,
    pub recent_authentication: RecentAuthenticationDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TrailBaseActivationDto {
    pub state: TrailBaseActivationStateDto,
    pub blocker: Option<TrailBaseActivationBlockerDto>,
    #[schemars(length(equal = 36), regex(pattern = r"^tbi_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-trailbase-instance-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^tbi_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub trailbase_instance_id: String,
    pub generation: u64,
    pub session_generation_current: bool,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessFirstRunStepDto {
    pub key: AccessFirstRunStepKeyDto,
    pub state: AccessEvidenceStateDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessEvidenceDto {
    pub kind: AccessEvidenceKindDto,
    pub state: AccessEvidenceStateDto,
    #[schemars(length(equal = 35), regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-operation-id"))]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub operation_id: String,
    #[schemars(length(equal = 36), regex(pattern = r"^req_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"), extend("format" = "fasti-request-correlation-id"))]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^req_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub correlation_id: String,
    pub ceremony_state: Option<AccessCeremonyStateDto>,
    pub failure: Option<AccessCeremonyFailureDto>,
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub occurred_at: String,
}

/// One server-derived source for the permanent Account and security surface
/// and the separate resumable first-run journey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AccessProjectionResponse {
    #[schemars(length(min = 20, max = 35), extend("format" = "date-time"))]
    #[schema(min_length = 20, max_length = 35, format = DateTime)]
    pub generated_at: String,
    pub subject: AccessSubjectDto,
    pub membership: AccessMembershipDto,
    pub current_session: BrowserSessionDto,
    #[schemars(length(max = 32))]
    #[schema(max_items = 32)]
    pub sessions: Vec<BrowserSessionDto>,
    pub sessions_truncated: bool,
    #[schemars(length(max = 64))]
    #[schema(max_items = 64)]
    pub profile_grants: Vec<AccessProfileGrantDto>,
    pub profile_grants_truncated: bool,
    pub session_policy: BrowserSessionPolicyDto,
    pub authentication: AccessSessionAuthenticationDto,
    pub trailbase: TrailBaseActivationDto,
    pub first_run_steps: [AccessFirstRunStepDto; 5],
    #[schemars(length(max = 16))]
    #[schema(max_items = 16)]
    pub evidence: Vec<AccessEvidenceDto>,
    pub evidence_truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> BrowserSessionDto {
        BrowserSessionDto {
            browser_session_id: "ses_018f0e0e7f7b70008000000000000000".to_owned(),
            workspace_id: "wsp_018f0e0e7f7b70008000000000000000".to_owned(),
            selected_profile_grant_id: "grt_018f0e0e7f7b70008000000000000000".to_owned(),
            is_current: true,
            created_at: "2026-08-31T12:00:00Z".to_owned(),
            last_seen_at: "2026-08-31T12:01:00Z".to_owned(),
            idle_expires_at: "2026-08-31T12:31:00Z".to_owned(),
            absolute_expires_at: "2026-08-31T20:00:00Z".to_owned(),
            rotation_generation: 1,
        }
    }

    #[test]
    fn access_requests_and_nested_projection_reject_unknown_fields() {
        assert!(serde_json::from_str::<StartTrailBaseSignInRequest>(
            r#"{"remembered":false,"return_url":"https://attacker.example"}"#,
        )
        .is_err());
        assert!(serde_json::from_str::<SelectBrowserSessionProfileRequest>(
            r#"{"profile_grant_id":"grt_018f0e0e7f7b70008000000000000000","csrf":"secret"}"#,
        )
        .is_err());
        assert!(serde_json::from_str::<ReadBrowserSessionResponse>(
            r#"{"session":{"browser_session_id":"ses_018f0e0e7f7b70008000000000000000","workspace_id":"wsp_018f0e0e7f7b70008000000000000000","selected_profile_grant_id":"grt_018f0e0e7f7b70008000000000000000","is_current":true,"created_at":"2026-08-31T12:00:00Z","last_seen_at":"2026-08-31T12:01:00Z","idle_expires_at":"2026-08-31T12:31:00Z","absolute_expires_at":"2026-08-31T20:00:00Z","rotation_generation":1,"credential":"secret"}}"#,
        )
        .is_err());
    }

    #[test]
    fn public_access_state_contains_no_browser_or_vendor_secret_fields() {
        let projection = AccessProjectionResponse {
            generated_at: "2026-08-31T12:01:00Z".to_owned(),
            subject: AccessSubjectDto {
                auth_subject_id: "sub_018f0e0e7f7b70008000000000000000".to_owned(),
                lifecycle: AccessSubjectLifecycleDto::Active,
                created_at: "2026-08-31T12:00:00Z".to_owned(),
                updated_at: "2026-08-31T12:01:00Z".to_owned(),
            },
            membership: AccessMembershipDto {
                membership_id: "mem_018f0e0e7f7b70008000000000000000".to_owned(),
                workspace_id: "wsp_018f0e0e7f7b70008000000000000000".to_owned(),
                lifecycle: AccessMembershipLifecycleDto::Active,
                role: AccessWorkspaceRoleDto::Administrator,
                created_at: "2026-08-31T12:00:00Z".to_owned(),
                updated_at: "2026-08-31T12:01:00Z".to_owned(),
            },
            current_session: session(),
            sessions: vec![session()],
            sessions_truncated: false,
            profile_grants: vec![AccessProfileGrantDto {
                profile_grant_id: "grt_018f0e0e7f7b70008000000000000000".to_owned(),
                profile_id: "prf_018f0e0e7f7b70008000000000000000".to_owned(),
                owner_client_id: "cli_018f0e0e7f7b70008000000000000000".to_owned(),
                selected: true,
            }],
            profile_grants_truncated: false,
            session_policy: BrowserSessionPolicyDto {
                idle_timeout_seconds: 1_800,
                browser_lifetime_seconds: 28_800,
                remembered_browser_lifetime_seconds: 2_592_000,
                last_seen_write_interval_seconds: 60,
            },
            authentication: AccessSessionAuthenticationDto {
                method: AccessAuthenticationMethodDto::TrailBasePassword,
                verified_at: "2026-08-31T12:00:00Z".to_owned(),
                activation_generation: 1,
                recent_authentication: RecentAuthenticationDto {
                    state: AccessEvidenceStateDto::Unavailable,
                    expires_at: None,
                },
            },
            trailbase: TrailBaseActivationDto {
                state: TrailBaseActivationStateDto::Active,
                blocker: None,
                trailbase_instance_id: "tbi_018f0e0e7f7b70008000000000000000".to_owned(),
                generation: 1,
                session_generation_current: true,
                updated_at: "2026-08-31T12:00:00Z".to_owned(),
            },
            first_run_steps: [
                AccessFirstRunStepDto {
                    key: AccessFirstRunStepKeyDto::AccountConfirmed,
                    state: AccessEvidenceStateDto::Verified,
                },
                AccessFirstRunStepDto {
                    key: AccessFirstRunStepKeyDto::StrongSignIn,
                    state: AccessEvidenceStateDto::Unavailable,
                },
                AccessFirstRunStepDto {
                    key: AccessFirstRunStepKeyDto::Recovery,
                    state: AccessEvidenceStateDto::Unavailable,
                },
                AccessFirstRunStepDto {
                    key: AccessFirstRunStepKeyDto::DevicesAndClients,
                    state: AccessEvidenceStateDto::Unavailable,
                },
                AccessFirstRunStepDto {
                    key: AccessFirstRunStepKeyDto::ExternalIdentity,
                    state: AccessEvidenceStateDto::Unavailable,
                },
            ],
            evidence: vec![AccessEvidenceDto {
                kind: AccessEvidenceKindDto::CurrentSessionIssued,
                state: AccessEvidenceStateDto::Verified,
                operation_id: "op_018f0e0e7f7b70008000000000000000".to_owned(),
                correlation_id: "req_018f0e0e7f7b70008000000000000000".to_owned(),
                ceremony_state: Some(AccessCeremonyStateDto::Completed),
                failure: None,
                occurred_at: "2026-08-31T12:01:00Z".to_owned(),
            }],
            evidence_truncated: false,
        };

        let json = serde_json::to_string(&projection).expect("projection is serializable");
        for forbidden in [
            "session_secret",
            "csrf",
            "access_token",
            "refresh_token",
            "code_verifier",
            "credential",
        ] {
            assert!(!json.contains(forbidden), "leaked {forbidden}");
        }
    }
}
