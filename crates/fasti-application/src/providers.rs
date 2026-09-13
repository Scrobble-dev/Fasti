//! Provider capability state and application-owned credential vault ports.
//!
//! Provider declarations and runtime adapters use these types without owning
//! credential storage. SQLite stores only the opaque credential reference and
//! safe state. A `CredentialVaultPort` implementation stores secret material.

use crate::{ApplicationAccessContext, ApplicationResult, ProblemCode};
use chrono::{DateTime, Utc};
use fasti_domain::{RequestCorrelationId, WorkspaceId};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::Zeroize;

mod response_cache;
pub use response_cache::{ProviderResponseCachePolicy, ProviderResponseReuse};

pub const MAX_PROVIDER_ID_BYTES: usize = 128;
pub const MAX_PROVIDER_CAPABILITY_ID_BYTES: usize = 128;
pub const MAX_CREDENTIAL_REFERENCE_BYTES: usize = 253;
pub const MAX_CREDENTIAL_SECRET_BYTES: usize = 64 * 1024;
/// Public provider-credential request limit shared by every adapter.
pub const MAX_PROVIDER_CREDENTIAL_BYTES: usize = 4096;

/// Keeps an already-acquired host provider guard alive during blocking work.
/// This has no acquisition or authorization behavior. Hosts must supply their
/// existing provider gate's guard, and workers retain a clone until they finish.
#[derive(Clone)]
pub struct ProviderOperationLease {
    _guard: std::sync::Arc<dyn Send + Sync>,
}

impl ProviderOperationLease {
    pub fn new(guard: impl Send + Sync + 'static) -> Self {
        Self {
            _guard: std::sync::Arc::new(guard),
        }
    }
}
const CONFIGURATION_DIGEST_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderValueError {
    InvalidProviderId,
    InvalidCapabilityId,
    InvalidCredentialReference,
    InvalidConfigurationDigest,
    InvalidCapabilityVersion,
    InvalidCredentialState,
    InvalidCheckMetadata,
    EmptyCredentialSecret,
    CredentialSecretTooLarge,
}

impl fmt::Display for ProviderValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidProviderId => "provider ID is not a bounded canonical identifier",
            Self::InvalidCapabilityId => {
                "provider capability ID is not a bounded canonical identifier"
            }
            Self::InvalidCredentialReference => {
                "credential reference is not a bounded opaque identifier"
            }
            Self::InvalidConfigurationDigest => {
                "configuration digest must contain exactly 64 lowercase hexadecimal characters"
            }
            Self::InvalidCapabilityVersion => "provider capability version must be at least one",
            Self::InvalidCredentialState => {
                "credential requirement, reference, and status are inconsistent"
            }
            Self::InvalidCheckMetadata => "provider check metadata is inconsistent",
            Self::EmptyCredentialSecret => "credential secret must not be empty",
            Self::CredentialSecretTooLarge => "credential secret exceeds its bounded limit",
        })
    }
}

impl std::error::Error for ProviderValueError {}

fn valid_identifier(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.trim() == value
        && value.bytes().all(|byte| {
            !byte.is_ascii_uppercase() && (byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
        })
}

macro_rules! identifier_type {
    ($name:ident, $limit:ident, $error:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(value: impl Into<String>) -> Result<Self, ProviderValueError> {
                let value = value.into();
                if !valid_identifier(&value, $limit) {
                    return Err(ProviderValueError::$error);
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

identifier_type!(ProviderId, MAX_PROVIDER_ID_BYTES, InvalidProviderId);
identifier_type!(
    ProviderCapabilityId,
    MAX_PROVIDER_CAPABILITY_ID_BYTES,
    InvalidCapabilityId
);

/// Opaque locator allocated by a credential vault.
///
/// It is safe to persist, but its shape and backend remain private to the
/// vault. It deliberately has a redacted `Debug` implementation so accidental
/// diagnostics do not disclose backend coordinates.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CredentialReference(String);

impl CredentialReference {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProviderValueError> {
        let value = value.into();
        if !valid_identifier(&value, MAX_CREDENTIAL_REFERENCE_BYTES) {
            return Err(ProviderValueError::InvalidCredentialReference);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CredentialReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialReference(<redacted>)")
    }
}

/// SHA-256 digest of safe, response-relevant provider configuration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigurationDigest(String);

impl ConfigurationDigest {
    pub fn parse(value: impl Into<String>) -> Result<Self, ProviderValueError> {
        let value = value.into();
        if value.len() != CONFIGURATION_DIGEST_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProviderValueError::InvalidConfigurationDigest);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialRequirement {
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

impl CredentialRequirement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::OptionalApiKey => "optional_api_key",
            Self::ApiKey => "api_key",
            Self::BearerToken => "bearer_token",
            Self::BasicAuth => "basic_auth",
            Self::Oauth2 => "oauth2",
            Self::UserAgentOnly => "user_agent_only",
            Self::CustomHeader => "custom_header",
            Self::OperatorSecretMount => "operator_secret_mount",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "optional_api_key" => Some(Self::OptionalApiKey),
            "api_key" => Some(Self::ApiKey),
            "bearer_token" => Some(Self::BearerToken),
            "basic_auth" => Some(Self::BasicAuth),
            "oauth2" => Some(Self::Oauth2),
            "user_agent_only" => Some(Self::UserAgentOnly),
            "custom_header" => Some(Self::CustomHeader),
            "operator_secret_mount" => Some(Self::OperatorSecretMount),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCredentialStatus {
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

impl ProviderCredentialStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Optional => "optional",
            Self::Missing => "missing",
            Self::StoredUnverified => "stored_unverified",
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::Expired => "expired",
            Self::Unavailable => "unavailable",
            Self::Revoked => "revoked",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "not_required" => Some(Self::NotRequired),
            "optional" => Some(Self::Optional),
            "missing" => Some(Self::Missing),
            "stored_unverified" => Some(Self::StoredUnverified),
            "valid" => Some(Self::Valid),
            "invalid" => Some(Self::Invalid),
            "expired" => Some(Self::Expired),
            "unavailable" => Some(Self::Unavailable),
            "revoked" => Some(Self::Revoked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapabilityStatus {
    Available,
    Degraded,
    Unavailable,
    Disabled,
}

impl ProviderCapabilityStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "available" => Some(Self::Available),
            "degraded" => Some(Self::Degraded),
            "unavailable" => Some(Self::Unavailable),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCheckStatus {
    NeverRun,
    Passed,
    Failed,
    Unavailable,
}

impl ProviderCheckStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeverRun => "never_run",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unavailable => "unavailable",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "never_run" => Some(Self::NeverRun),
            "passed" => Some(Self::Passed),
            "failed" => Some(Self::Failed),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCheckKind {
    Health,
    Credential,
}

pub const fn credential_status_after_successful_check(
    kind: ProviderCheckKind,
    current: ProviderCredentialStatus,
) -> ProviderCredentialStatus {
    match kind {
        ProviderCheckKind::Health => current,
        ProviderCheckKind::Credential => ProviderCredentialStatus::Valid,
    }
}

pub fn credential_status_after_failed_check(
    kind: ProviderCheckKind,
    current: &ProviderCapabilityState,
    code: ProblemCode,
) -> ProviderCredentialStatus {
    // Health is not credential verification. An absent reference must keep the
    // requirement's Missing/Optional/NotRequired state, not invent a credential.
    if kind == ProviderCheckKind::Health || current.credential_reference().is_none() {
        return current.credential_status();
    }
    match code {
        ProblemCode::ProviderCredentialMissing => ProviderCredentialStatus::Unavailable,
        ProblemCode::ProviderCredentialInvalid => ProviderCredentialStatus::Invalid,
        ProblemCode::ProviderCredentialExpired => ProviderCredentialStatus::Expired,
        _ => current.credential_status(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCheckMetadata {
    status: ProviderCheckStatus,
    checked_at: Option<DateTime<Utc>>,
    safe_problem_code: Option<ProblemCode>,
}

impl ProviderCheckMetadata {
    pub const fn never_run() -> Self {
        Self {
            status: ProviderCheckStatus::NeverRun,
            checked_at: None,
            safe_problem_code: None,
        }
    }

    pub fn try_new(
        status: ProviderCheckStatus,
        checked_at: Option<DateTime<Utc>>,
        safe_problem_code: Option<ProblemCode>,
    ) -> Result<Self, ProviderValueError> {
        let valid = match status {
            ProviderCheckStatus::NeverRun => checked_at.is_none() && safe_problem_code.is_none(),
            ProviderCheckStatus::Passed => checked_at.is_some() && safe_problem_code.is_none(),
            ProviderCheckStatus::Failed | ProviderCheckStatus::Unavailable => {
                checked_at.is_some() && safe_problem_code.is_some()
            }
        };
        if !valid {
            return Err(ProviderValueError::InvalidCheckMetadata);
        }
        Ok(Self {
            status,
            checked_at,
            safe_problem_code,
        })
    }

    pub const fn status(&self) -> ProviderCheckStatus {
        self.status
    }

    pub const fn checked_at(&self) -> Option<DateTime<Utc>> {
        self.checked_at
    }

    pub const fn safe_problem_code(&self) -> Option<ProblemCode> {
        self.safe_problem_code
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilityState {
    provider_id: ProviderId,
    capability_id: ProviderCapabilityId,
    capability_status: ProviderCapabilityStatus,
    /// Monotonic snapshot version. Increment for every material state change.
    capability_version: u64,
    credential_requirement: CredentialRequirement,
    credential_reference: Option<CredentialReference>,
    credential_status: ProviderCredentialStatus,
    configuration_digest: ConfigurationDigest,
    health: ProviderCheckMetadata,
    credential_test: ProviderCheckMetadata,
}

impl ProviderCapabilityState {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        provider_id: ProviderId,
        capability_id: ProviderCapabilityId,
        capability_status: ProviderCapabilityStatus,
        capability_version: u64,
        credential_requirement: CredentialRequirement,
        credential_reference: Option<CredentialReference>,
        credential_status: ProviderCredentialStatus,
        configuration_digest: ConfigurationDigest,
        health: ProviderCheckMetadata,
        credential_test: ProviderCheckMetadata,
    ) -> Result<Self, ProviderValueError> {
        if capability_version == 0 || capability_version > i64::MAX as u64 {
            return Err(ProviderValueError::InvalidCapabilityVersion);
        }
        if !valid_credential_state(
            credential_requirement,
            credential_reference.as_ref(),
            credential_status,
        ) {
            return Err(ProviderValueError::InvalidCredentialState);
        }
        Ok(Self {
            provider_id,
            capability_id,
            capability_status,
            capability_version,
            credential_requirement,
            credential_reference,
            credential_status,
            configuration_digest,
            health,
            credential_test,
        })
    }

    pub const fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub const fn capability_id(&self) -> &ProviderCapabilityId {
        &self.capability_id
    }

    pub const fn capability_status(&self) -> ProviderCapabilityStatus {
        self.capability_status
    }

    pub const fn capability_version(&self) -> u64 {
        self.capability_version
    }

    pub const fn credential_requirement(&self) -> CredentialRequirement {
        self.credential_requirement
    }

    pub const fn credential_reference(&self) -> Option<&CredentialReference> {
        self.credential_reference.as_ref()
    }

    pub const fn credential_status(&self) -> ProviderCredentialStatus {
        self.credential_status
    }

    pub const fn configuration_digest(&self) -> &ConfigurationDigest {
        &self.configuration_digest
    }

    pub const fn health(&self) -> &ProviderCheckMetadata {
        &self.health
    }

    pub const fn credential_test(&self) -> &ProviderCheckMetadata {
        &self.credential_test
    }
}

fn valid_credential_state(
    requirement: CredentialRequirement,
    reference: Option<&CredentialReference>,
    status: ProviderCredentialStatus,
) -> bool {
    match requirement {
        CredentialRequirement::None | CredentialRequirement::UserAgentOnly => {
            reference.is_none() && status == ProviderCredentialStatus::NotRequired
        }
        CredentialRequirement::OptionalApiKey => match reference {
            None => status == ProviderCredentialStatus::Optional,
            Some(_) => !matches!(
                status,
                ProviderCredentialStatus::NotRequired
                    | ProviderCredentialStatus::Optional
                    | ProviderCredentialStatus::Missing
            ),
        },
        _ => match reference {
            None => status == ProviderCredentialStatus::Missing,
            Some(_) => !matches!(
                status,
                ProviderCredentialStatus::NotRequired
                    | ProviderCredentialStatus::Optional
                    | ProviderCredentialStatus::Missing
            ),
        },
    }
}

/// Secret bytes accepted only at the trusted application boundary.
///
/// The value is bounded, redacted from `Debug`, never serializable, and
/// overwritten before deallocation. Callers must explicitly use `expose()`.
pub struct CredentialSecret(Box<[u8]>);

impl CredentialSecret {
    pub fn try_from_bytes(bytes: impl Into<Vec<u8>>) -> Result<Self, ProviderValueError> {
        let mut bytes = bytes.into();
        if bytes.is_empty() {
            return Err(ProviderValueError::EmptyCredentialSecret);
        }
        if bytes.len() > MAX_CREDENTIAL_SECRET_BYTES {
            bytes.zeroize();
            return Err(ProviderValueError::CredentialSecretTooLarge);
        }
        let mut stored = vec![0; bytes.len()].into_boxed_slice();
        stored.copy_from_slice(&bytes);
        bytes.zeroize();
        Ok(Self(stored))
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for CredentialSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CredentialSecret(<redacted>)")
    }
}

impl Drop for CredentialSecret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCredential {
    reference: CredentialReference,
    version: u64,
}

impl StoredCredential {
    pub fn try_new(
        reference: CredentialReference,
        version: u64,
    ) -> Result<Self, ProviderValueError> {
        if version == 0 {
            return Err(ProviderValueError::InvalidCapabilityVersion);
        }
        Ok(Self { reference, version })
    }

    pub const fn reference(&self) -> &CredentialReference {
        &self.reference
    }

    pub const fn version(&self) -> u64 {
        self.version
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialVaultError {
    Missing,
    Unavailable,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialVaultSource {
    None,
    Environment,
    CredentialStore,
    OperatorSecretMount,
}

impl fmt::Display for CredentialVaultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "credential is not present in the vault",
            Self::Unavailable => "credential vault is unavailable",
            Self::Rejected => "credential vault rejected the operation",
        })
    }
}

impl std::error::Error for CredentialVaultError {}

/// General credential-vault boundary for providers, access, and integrations.
pub trait CredentialVaultPort: Send + Sync {
    fn source(
        &self,
        reference: &CredentialReference,
    ) -> Result<CredentialVaultSource, CredentialVaultError>;

    fn store(
        &self,
        reference: &CredentialReference,
        secret: CredentialSecret,
    ) -> Result<StoredCredential, CredentialVaultError>;

    fn replace(
        &self,
        reference: &CredentialReference,
        secret: CredentialSecret,
    ) -> Result<StoredCredential, CredentialVaultError>;

    fn load(
        &self,
        reference: &CredentialReference,
    ) -> Result<CredentialSecret, CredentialVaultError>;

    fn revoke(&self, reference: &CredentialReference) -> Result<(), CredentialVaultError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStateWriteOutcome {
    Created,
    Replaced,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStatePortError {
    Unavailable,
    Corrupt,
    RevisionConflict,
}

impl fmt::Display for ProviderStatePortError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "provider state storage is unavailable",
            Self::Corrupt => "provider state storage contains invalid data",
            Self::RevisionConflict => "provider capability version conflicts with stored state",
        })
    }
}

impl std::error::Error for ProviderStatePortError {}

/// Provider state boundary. The public inventory read resolves current authority
/// atomically; raw state operations require an already-authorized caller.
/// The store adapter remains transport- and network-free.
pub trait ProviderStatePort: Send + Sync {
    fn authorize_and_list_provider_capability_states(
        &self,
        correlation_id: RequestCorrelationId,
        access: &ApplicationAccessContext,
    ) -> ApplicationResult<Vec<ProviderCapabilityState>>;

    fn get_provider_capability_state(
        &self,
        workspace_id: WorkspaceId,
        provider_id: &ProviderId,
        capability_id: &ProviderCapabilityId,
    ) -> Result<Option<ProviderCapabilityState>, ProviderStatePortError>;

    fn list_provider_capability_states(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<ProviderCapabilityState>, ProviderStatePortError>;

    fn put_provider_capability_state(
        &self,
        workspace_id: WorkspaceId,
        state: ProviderCapabilityState,
    ) -> Result<ProviderStateWriteOutcome, ProviderStatePortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> ConfigurationDigest {
        ConfigurationDigest::parse("ab".repeat(32)).expect("configuration digest")
    }

    #[test]
    fn references_and_secrets_do_not_disclose_debug_values() {
        let reference = CredentialReference::try_new("secret:providers/tmdb/api-key")
            .expect("credential reference");
        let secret = CredentialSecret::try_from_bytes(b"do-not-log".to_vec()).expect("secret");
        assert_eq!(format!("{reference:?}"), "CredentialReference(<redacted>)");
        assert_eq!(format!("{secret:?}"), "CredentialSecret(<redacted>)");
    }

    #[test]
    fn capability_state_enforces_credential_semantics() {
        let provider_id = ProviderId::try_new("tmdb").expect("provider ID");
        let capability_id =
            ProviderCapabilityId::try_new("metadata.search").expect("capability ID");
        let no_secret = ProviderCapabilityState::try_new(
            provider_id.clone(),
            capability_id.clone(),
            ProviderCapabilityStatus::Available,
            1,
            CredentialRequirement::None,
            None,
            ProviderCredentialStatus::NotRequired,
            digest(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        );
        assert!(no_secret.is_ok());

        let missing_required = ProviderCapabilityState::try_new(
            provider_id.clone(),
            capability_id.clone(),
            ProviderCapabilityStatus::Unavailable,
            1,
            CredentialRequirement::ApiKey,
            None,
            ProviderCredentialStatus::Missing,
            digest(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        );
        assert!(missing_required.is_ok());

        let inconsistent = ProviderCapabilityState::try_new(
            provider_id,
            capability_id,
            ProviderCapabilityStatus::Available,
            1,
            CredentialRequirement::ApiKey,
            None,
            ProviderCredentialStatus::Valid,
            digest(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        );
        assert_eq!(
            inconsistent,
            Err(ProviderValueError::InvalidCredentialState)
        );
    }

    #[test]
    fn check_metadata_never_carries_raw_detail() {
        let checked_at = "2026-08-30T12:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("timestamp");
        assert!(ProviderCheckMetadata::try_new(
            ProviderCheckStatus::Failed,
            Some(checked_at),
            Some(ProblemCode::StorageUnavailable),
        )
        .is_ok());
        assert_eq!(
            ProviderCheckMetadata::try_new(ProviderCheckStatus::Failed, Some(checked_at), None),
            Err(ProviderValueError::InvalidCheckMetadata)
        );
        assert_eq!(
            ProviderCheckMetadata::try_new(
                ProviderCheckStatus::Passed,
                Some(checked_at),
                Some(ProblemCode::StorageUnavailable),
            ),
            Err(ProviderValueError::InvalidCheckMetadata)
        );
    }

    #[test]
    fn successful_health_check_does_not_invent_a_credential() {
        assert_eq!(
            credential_status_after_successful_check(
                ProviderCheckKind::Health,
                ProviderCredentialStatus::NotRequired,
            ),
            ProviderCredentialStatus::NotRequired
        );
        assert_eq!(
            credential_status_after_successful_check(
                ProviderCheckKind::Credential,
                ProviderCredentialStatus::StoredUnverified,
            ),
            ProviderCredentialStatus::Valid
        );
    }

    #[test]
    fn failed_checks_preserve_reference_and_health_boundaries() {
        for (requirement, absent_status) in [
            (
                CredentialRequirement::ApiKey,
                ProviderCredentialStatus::Missing,
            ),
            (
                CredentialRequirement::OptionalApiKey,
                ProviderCredentialStatus::Optional,
            ),
            (
                CredentialRequirement::None,
                ProviderCredentialStatus::NotRequired,
            ),
        ] {
            for present in [false, true] {
                if present && requirement == CredentialRequirement::None {
                    continue;
                }
                let state = ProviderCapabilityState::try_new(
                    ProviderId::try_new("tmdb").expect("provider"),
                    ProviderCapabilityId::try_new("metadata.read").expect("capability"),
                    ProviderCapabilityStatus::Available,
                    1,
                    requirement,
                    present.then(|| {
                        CredentialReference::try_new("secret:providers/tmdb/api-key")
                            .expect("reference")
                    }),
                    if present {
                        ProviderCredentialStatus::StoredUnverified
                    } else {
                        absent_status
                    },
                    digest(),
                    ProviderCheckMetadata::never_run(),
                    ProviderCheckMetadata::never_run(),
                )
                .expect("valid state");
                for (code, expected) in [
                    (
                        ProblemCode::ProviderCredentialMissing,
                        ProviderCredentialStatus::Unavailable,
                    ),
                    (
                        ProblemCode::ProviderCredentialInvalid,
                        ProviderCredentialStatus::Invalid,
                    ),
                    (
                        ProblemCode::ProviderCredentialExpired,
                        ProviderCredentialStatus::Expired,
                    ),
                    (ProblemCode::ProviderUnavailable, state.credential_status()),
                ] {
                    assert_eq!(
                        credential_status_after_failed_check(
                            ProviderCheckKind::Health,
                            &state,
                            code
                        ),
                        state.credential_status()
                    );
                    let status = credential_status_after_failed_check(
                        ProviderCheckKind::Credential,
                        &state,
                        code,
                    );
                    assert_eq!(status, if present { expected } else { absent_status });
                    assert!(valid_credential_state(
                        requirement,
                        state.credential_reference(),
                        status
                    ));
                }
            }
        }
    }
}
