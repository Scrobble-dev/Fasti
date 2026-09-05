//! Governed application policy for Access credentials.

use crate::{
    BrowserSessionMutationCommand, BrowserSessionQuery, ScopeKey, SecretMaterial, SecretParseError,
    ValidatedBrowserReadBoundary,
};
use chrono::{DateTime, SubsecRound, TimeDelta, Utc};
use fasti_domain::{
    AccessConsentDecision, AccessConsentRevision, AccessConsentRevisionId, AccessCredentialName,
    ApplicationClient, ApplicationClientClassification, ApplicationClientPurpose,
    ClientAuthenticationType, ClientId, PersonalAccessToken, PersonalAccessTokenId,
    RegisteredClientCredential, Sha256Digest,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, num::NonZeroU16, time::Duration};
use zeroize::Zeroizing;

const DAY_SECONDS: u64 = 24 * 60 * 60;
const MAX_LIFETIME: Duration = Duration::from_secs(365 * DAY_SECONDS);
const PAT_PREFIX: &str = "fasti_pat_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessInventoryInputError {
    InvalidLimit,
    IncompleteCursor,
}

impl fmt::Display for AccessInventoryInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "inventory page size must be from 1 through 100",
            Self::IncompleteCursor => "inventory cursors require both creation time and typed ID",
        })
    }
}

impl Error for AccessInventoryInputError {}

/// Shared keyset input for client, PAT, consent and device inventories. The
/// endpoint selects its concrete ID type; a cursor never proves ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessInventoryPage<Id> {
    limit: NonZeroU16,
    after: Option<(DateTime<Utc>, Id)>,
}

impl<Id> AccessInventoryPage<Id> {
    pub fn try_new(
        limit: Option<u16>,
        after_created_at: Option<DateTime<Utc>>,
        after_id: Option<Id>,
    ) -> Result<Self, AccessInventoryInputError> {
        let limit = NonZeroU16::new(limit.unwrap_or(32))
            .filter(|value| value.get() <= 100)
            .ok_or(AccessInventoryInputError::InvalidLimit)?;
        let after = match (after_created_at, after_id) {
            (None, None) => None,
            (Some(created_at), Some(id)) => Some((created_at.trunc_subsecs(6), id)),
            _ => return Err(AccessInventoryInputError::IncompleteCursor),
        };
        Ok(Self { limit, after })
    }

    pub const fn limit(&self) -> u16 {
        self.limit.get()
    }

    pub const fn after(&self) -> Option<&(DateTime<Utc>, Id)> {
        self.after.as_ref()
    }
}

/// Read-only browser evidence, not authenticated authority. The transaction
/// reloads current session, membership and resource ownership before pagination.
/// Inventory inspection does not require the mutation/recent-auth envelope.
pub struct AccessInventoryQuery<Id> {
    request: BrowserSessionQuery,
    _request_boundary: ValidatedBrowserReadBoundary,
    page: AccessInventoryPage<Id>,
}

impl<Id> AccessInventoryQuery<Id> {
    pub const fn new(
        request: BrowserSessionQuery,
        request_boundary: ValidatedBrowserReadBoundary,
        page: AccessInventoryPage<Id>,
    ) -> Self {
        Self {
            request,
            _request_boundary: request_boundary,
            page,
        }
    }

    pub const fn browser_request(&self) -> &BrowserSessionQuery {
        &self.request
    }

    pub const fn page(&self) -> &AccessInventoryPage<Id> {
        &self.page
    }
}

/// Browser request evidence for human-only Access administration, not verified
/// authority. The operation transaction must authenticate the retained secrets
/// and reload current membership, ownership and recent authentication using its
/// trusted execution time, not the request's earlier timestamp.
pub struct AccessAdministrationRequest(BrowserSessionMutationCommand);

impl AccessAdministrationRequest {
    pub const fn new(request: BrowserSessionMutationCommand) -> Self {
        Self(request)
    }

    pub const fn browser_request(&self) -> &BrowserSessionMutationCommand {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessScopeSetInputError;

impl fmt::Display for AccessScopeSetInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("scope sets must contain only distinct registered scopes")
    }
}

impl Error for AccessScopeSetInputError {}

/// A canonical structural set, not a grant or delegability decision. Empty sets
/// represent withdrawal evidence; issuance commands must reject them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessScopeSet(Vec<ScopeKey>);

impl AccessScopeSet {
    pub fn try_new(scopes: &[ScopeKey]) -> Result<Self, AccessScopeSetInputError> {
        if scopes.len() > ScopeKey::ALL.len() {
            return Err(AccessScopeSetInputError);
        }
        // ponytail: bounded vocabulary scan; use indexed membership if the
        // registered scope vocabulary becomes large enough to warrant it.
        let canonical: Vec<_> = ScopeKey::ALL
            .iter()
            .copied()
            .filter(|scope| scopes.contains(scope))
            .collect();
        if canonical.len() != scopes.len() {
            return Err(AccessScopeSetInputError);
        }
        Ok(Self(canonical))
    }

    pub fn scopes(&self) -> &[ScopeKey] {
        &self.0
    }

    pub fn digest(&self) -> Sha256Digest {
        let mut hasher = Sha256::new();
        for (index, scope) in self.0.iter().enumerate() {
            if index != 0 {
                hasher.update(b"\n");
            }
            hasher.update(scope.as_str().as_bytes());
        }
        Sha256Digest::from_bytes(&hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAccessClientInputError {
    InvalidPurpose,
    EmptyScopes,
    InvalidExpiry,
}

impl fmt::Display for RegisterAccessClientInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPurpose => "only CLI and integration clients can be registered",
            Self::EmptyScopes => "client registration requires at least one requested scope",
            Self::InvalidExpiry => "client expiry must be within the configured lifetime bounds",
        })
    }
}

impl Error for RegisterAccessClientInputError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantAccessConsentInputError;

impl fmt::Display for GrantAccessConsentInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("consent requires at least one scope; revoke consent to withdraw it")
    }
}

impl Error for GrantAccessConsentInputError {}

/// Change an existing consent revision. Initial consent belongs to registration
/// or legacy rotation; the transaction derives all binding from this revision,
/// compares it to current state and authorizes the exact requested scopes.
pub struct GrantAccessConsentCommand {
    request: AccessAdministrationRequest,
    expected_current_revision: AccessConsentRevisionId,
    scopes: AccessScopeSet,
}

impl GrantAccessConsentCommand {
    pub fn try_new(
        request: AccessAdministrationRequest,
        expected_current_revision: AccessConsentRevisionId,
        scopes: AccessScopeSet,
    ) -> Result<Self, GrantAccessConsentInputError> {
        if scopes.scopes().is_empty() {
            return Err(GrantAccessConsentInputError);
        }
        Ok(Self {
            request,
            expected_current_revision,
            scopes,
        })
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn expected_current_revision(&self) -> AccessConsentRevisionId {
        self.expected_current_revision
    }

    pub const fn scopes(&self) -> &AccessScopeSet {
        &self.scopes
    }
}

/// Withdraw exactly the expected current revision, never a newer successor.
/// The transaction owns stale-revision rejection and current-revoked no-ops.
pub struct RevokeAccessConsentCommand {
    request: AccessAdministrationRequest,
    expected_current_revision: AccessConsentRevisionId,
}

impl RevokeAccessConsentCommand {
    pub const fn new(
        request: AccessAdministrationRequest,
        expected_current_revision: AccessConsentRevisionId,
    ) -> Self {
        Self {
            request,
            expected_current_revision,
        }
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn expected_current_revision(&self) -> AccessConsentRevisionId {
        self.expected_current_revision
    }
}

/// A bounded registration request, not permission to issue credentials.
/// Derive the owner, workspace and profile from current browser authority in
/// the operation transaction; recheck expiry and exact scope authorization.
pub struct RegisterAccessClientCommand {
    request: AccessAdministrationRequest,
    name: AccessCredentialName,
    classification: ApplicationClientClassification,
    scopes: AccessScopeSet,
    expires_at: DateTime<Utc>,
}

impl RegisterAccessClientCommand {
    pub fn try_new(
        request: AccessAdministrationRequest,
        name: AccessCredentialName,
        purpose: ApplicationClientPurpose,
        scopes: AccessScopeSet,
        expires_at: DateTime<Utc>,
        policy: TokenPolicy,
    ) -> Result<Self, RegisterAccessClientInputError> {
        let classification = ApplicationClientClassification::for_registration(purpose)
            .map_err(|_| RegisterAccessClientInputError::InvalidPurpose)?;
        if scopes.scopes().is_empty() {
            return Err(RegisterAccessClientInputError::EmptyScopes);
        }
        policy
            .client_secret_expiry(request.browser_request().now(), expires_at)
            .map_err(|_| RegisterAccessClientInputError::InvalidExpiry)?;
        Ok(Self {
            request,
            name,
            classification,
            scopes,
            expires_at,
        })
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn name(&self) -> &AccessCredentialName {
        &self.name
    }

    pub const fn classification(&self) -> ApplicationClientClassification {
        self.classification
    }

    pub const fn scopes(&self) -> &AccessScopeSet {
        &self.scopes
    }

    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotateAccessClientSecretInputError {
    InvalidEpoch,
    EmptyScopes,
    InvalidExpiry,
}

impl fmt::Display for RotateAccessClientSecretInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEpoch => "the expected credential epoch must permit a next stored epoch",
            Self::EmptyScopes => "client rotation requires the exact nonempty current scope set",
            Self::InvalidExpiry => "client expiry must be within the configured lifetime bounds",
        })
    }
}

impl Error for RotateAccessClientSecretInputError {}

/// The transaction compares both expected values to current state and requires
/// the submitted scopes to equal current scopes before issuing a new secret.
/// A missing consent revision means expect none, never skip the comparison.
pub struct RotateAccessClientSecretCommand {
    request: AccessAdministrationRequest,
    client_id: ClientId,
    expected_credential_epoch: u64,
    expected_consent_revision: Option<AccessConsentRevisionId>,
    scopes: AccessScopeSet,
    expires_at: DateTime<Utc>,
}

impl RotateAccessClientSecretCommand {
    pub fn try_new(
        request: AccessAdministrationRequest,
        client_id: ClientId,
        expected_credential_epoch: u64,
        expected_consent_revision: Option<AccessConsentRevisionId>,
        scopes: AccessScopeSet,
        expires_at: DateTime<Utc>,
        policy: TokenPolicy,
    ) -> Result<Self, RotateAccessClientSecretInputError> {
        if expected_credential_epoch >= i64::MAX as u64 {
            return Err(RotateAccessClientSecretInputError::InvalidEpoch);
        }
        if scopes.scopes().is_empty() {
            return Err(RotateAccessClientSecretInputError::EmptyScopes);
        }
        policy
            .client_secret_expiry(request.browser_request().now(), expires_at)
            .map_err(|_| RotateAccessClientSecretInputError::InvalidExpiry)?;
        Ok(Self {
            request,
            client_id,
            expected_credential_epoch,
            expected_consent_revision,
            scopes,
            expires_at,
        })
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub const fn expected_credential_epoch(&self) -> u64 {
        self.expected_credential_epoch
    }

    pub const fn expected_consent_revision(&self) -> Option<AccessConsentRevisionId> {
        self.expected_consent_revision
    }

    pub const fn scopes(&self) -> &AccessScopeSet {
        &self.scopes
    }

    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

/// Revoke the client's current authority, not merely one credential epoch.
/// Current ownership, administrator continuity and idempotence stay in storage.
pub struct RevokeAccessClientCommand {
    request: AccessAdministrationRequest,
    client_id: ClientId,
}

impl RevokeAccessClientCommand {
    pub const fn new(request: AccessAdministrationRequest, client_id: ClientId) -> Self {
        Self { request, client_id }
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersonalAccessTokenInputError {
    EmptyScopes,
    InvalidExpiry,
}

impl fmt::Display for PersonalAccessTokenInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyScopes => "personal token creation requires at least one requested scope",
            Self::InvalidExpiry => {
                "personal token expiry must be within the configured lifetime bounds"
            }
        })
    }
}

impl Error for PersonalAccessTokenInputError {}

/// The transaction derives current owner/workspace/grant authority and resolves
/// omitted expiry from issuance time. Construction grants no permission.
pub struct CreatePersonalAccessTokenCommand {
    request: AccessAdministrationRequest,
    name: AccessCredentialName,
    scopes: AccessScopeSet,
    requested_expires_at: Option<DateTime<Utc>>,
}

impl CreatePersonalAccessTokenCommand {
    pub fn try_new(
        request: AccessAdministrationRequest,
        name: AccessCredentialName,
        scopes: AccessScopeSet,
        requested_expires_at: Option<DateTime<Utc>>,
        policy: TokenPolicy,
    ) -> Result<Self, PersonalAccessTokenInputError> {
        if scopes.scopes().is_empty() {
            return Err(PersonalAccessTokenInputError::EmptyScopes);
        }
        policy
            .pat_expiry(request.browser_request().now(), requested_expires_at)
            .map_err(|_| PersonalAccessTokenInputError::InvalidExpiry)?;
        Ok(Self {
            request,
            name,
            scopes,
            requested_expires_at,
        })
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn name(&self) -> &AccessCredentialName {
        &self.name
    }

    pub const fn scopes(&self) -> &AccessScopeSet {
        &self.scopes
    }

    pub const fn requested_expires_at(&self) -> Option<DateTime<Utc>> {
        self.requested_expires_at
    }
}

/// Replace exactly this predecessor. The transaction preserves its name,
/// binding and issued scopes, rejecting scopes no longer permitted by current
/// authority. The target ID is neither an actor nor proof of ownership.
pub struct RotatePersonalAccessTokenCommand {
    request: AccessAdministrationRequest,
    token_id: PersonalAccessTokenId,
    requested_expires_at: Option<DateTime<Utc>>,
}

impl RotatePersonalAccessTokenCommand {
    pub fn try_new(
        request: AccessAdministrationRequest,
        token_id: PersonalAccessTokenId,
        requested_expires_at: Option<DateTime<Utc>>,
        policy: TokenPolicy,
    ) -> Result<Self, PersonalAccessTokenInputError> {
        policy
            .pat_expiry(request.browser_request().now(), requested_expires_at)
            .map_err(|_| PersonalAccessTokenInputError::InvalidExpiry)?;
        Ok(Self {
            request,
            token_id,
            requested_expires_at,
        })
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn token_id(&self) -> PersonalAccessTokenId {
        self.token_id
    }

    pub const fn requested_expires_at(&self) -> Option<DateTime<Utc>> {
        self.requested_expires_at
    }
}

/// An immutable target for an authorized owner/administrator withdrawal.
/// Current ownership and terminal/idempotent state are transaction checks.
pub struct RevokePersonalAccessTokenCommand {
    request: AccessAdministrationRequest,
    token_id: PersonalAccessTokenId,
}

impl RevokePersonalAccessTokenCommand {
    pub const fn new(
        request: AccessAdministrationRequest,
        token_id: PersonalAccessTokenId,
    ) -> Self {
        Self { request, token_id }
    }

    pub const fn request(&self) -> &AccessAdministrationRequest {
        &self.request
    }

    pub const fn token_id(&self) -> PersonalAccessTokenId {
        self.token_id
    }
}

/// PAT secret material has a distinct transport and digest domain from client
/// credentials. Parsing proves syntax only; it never establishes authority.
/// Plaintext has no Debug, Clone, or serialization implementation.
pub struct PersonalAccessTokenSecret(SecretMaterial);

impl PersonalAccessTokenSecret {
    pub fn from_secret(material: SecretMaterial) -> Self {
        Self(material)
    }

    pub fn try_from_bearer(value: &str) -> Result<Self, SecretParseError> {
        let hex = value.strip_prefix(PAT_PREFIX).ok_or(SecretParseError)?;
        SecretMaterial::try_from_hex(hex).map(Self)
    }

    /// Only a successful creation/rotation response may expose this value.
    pub fn expose_bearer(&self) -> Zeroizing<String> {
        let hex = Zeroizing::new(self.0.expose_hex());
        let mut bearer = Zeroizing::new(String::with_capacity(PAT_PREFIX.len() + hex.len()));
        bearer.push_str(PAT_PREFIX);
        bearer.push_str(&hex);
        bearer
    }

    pub fn digest(&self) -> Sha256Digest {
        let mut hasher = Sha256::new();
        hasher.update(b"fasti-pat-v1:");
        hasher.update(self.0.expose_bytes());
        Sha256Digest::from_bytes(&hasher.finalize().into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessIssuanceResultError {
    EmptyScopes,
    InvalidClientCredential,
    InvalidConsent,
    NonfreshPersonalToken,
    SecretMismatch,
}

impl fmt::Display for AccessIssuanceResultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("credential issuance result contains inconsistent evidence")
    }
}

impl Error for AccessIssuanceResultError {}

/// Structurally consistent output, not a commit receipt. The transaction builds
/// this before commit and releases it only after mutation and audit both commit.
pub struct IssuedAccessClientCredential {
    client: ApplicationClient,
    credential: RegisteredClientCredential,
    consent: AccessConsentRevision,
    scopes: AccessScopeSet,
    secret: SecretMaterial,
}

impl IssuedAccessClientCredential {
    pub fn try_new(
        client: ApplicationClient,
        credential: RegisteredClientCredential,
        consent: AccessConsentRevision,
        scopes: AccessScopeSet,
        secret: SecretMaterial,
    ) -> Result<Self, AccessIssuanceResultError> {
        if scopes.scopes().is_empty() {
            return Err(AccessIssuanceResultError::EmptyScopes);
        }
        if client.owner_subject_id().is_none()
            || client.classification().authentication_type()
                != ClientAuthenticationType::Confidential
            || !matches!(
                client.classification().purpose(),
                ApplicationClientPurpose::Cli | ApplicationClientPurpose::Integration
            )
            || credential.expires_at().is_none()
            || !credential.is_current_for(&client, credential.created_at())
        {
            return Err(AccessIssuanceResultError::InvalidClientCredential);
        }
        if consent.client_id() != client.id()
            || consent.workspace_id() != client.workspace_id()
            || Some(consent.subject_id()) != client.owner_subject_id()
            || consent.created_at() < client.created_at()
            || consent.created_at() > credential.created_at()
            || consent.decision() != &AccessConsentDecision::Granted(scopes.digest())
        {
            return Err(AccessIssuanceResultError::InvalidConsent);
        }
        if credential.digest()
            != &Sha256Digest::from_bytes(&Sha256::digest(secret.expose_bytes()).into())
        {
            return Err(AccessIssuanceResultError::SecretMismatch);
        }
        Ok(Self {
            client,
            credential,
            consent,
            scopes,
            secret,
        })
    }

    pub const fn client(&self) -> &ApplicationClient {
        &self.client
    }
    pub const fn credential(&self) -> &RegisteredClientCredential {
        &self.credential
    }
    pub const fn consent(&self) -> &AccessConsentRevision {
        &self.consent
    }
    pub const fn scopes(&self) -> &AccessScopeSet {
        &self.scopes
    }

    pub fn into_secret(self) -> SecretMaterial {
        self.secret
    }
}

/// One new PAT and its secret. Scope persistence and current authority remain
/// transaction checks; this result cannot prove them from the token alone.
pub struct IssuedPersonalAccessToken {
    token: PersonalAccessToken,
    scopes: AccessScopeSet,
    secret: PersonalAccessTokenSecret,
}

impl IssuedPersonalAccessToken {
    pub fn try_new(
        token: PersonalAccessToken,
        scopes: AccessScopeSet,
        secret: PersonalAccessTokenSecret,
    ) -> Result<Self, AccessIssuanceResultError> {
        if scopes.scopes().is_empty() {
            return Err(AccessIssuanceResultError::EmptyScopes);
        }
        if token.last_used_at().is_some()
            || token.revoked_at().is_some()
            || token.replaced_by().is_some()
        {
            return Err(AccessIssuanceResultError::NonfreshPersonalToken);
        }
        if token.digest() != &secret.digest() {
            return Err(AccessIssuanceResultError::SecretMismatch);
        }
        Ok(Self {
            token,
            scopes,
            secret,
        })
    }

    pub const fn token(&self) -> &PersonalAccessToken {
        &self.token
    }
    pub const fn scopes(&self) -> &AccessScopeSet {
        &self.scopes
    }

    pub fn into_secret(self) -> PersonalAccessTokenSecret {
        self.secret
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPolicyInputError;

impl fmt::Display for TokenPolicyInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("credential lifetimes must be ordered whole days from 1 through 365")
    }
}

impl Error for TokenPolicyInputError {}

/// Explicit C2 product policy. The trusted host selects `TokenPolicy::C2`.
/// This policy neither extends existing credentials nor grants authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPolicy {
    pat_minimum: Duration,
    pat_default: Duration,
    pat_maximum: Duration,
    client_secret_minimum: Duration,
    client_secret_maximum: Duration,
}

impl TokenPolicy {
    pub const C2: Self = Self {
        pat_minimum: Duration::from_secs(DAY_SECONDS),
        pat_default: Duration::from_secs(30 * DAY_SECONDS),
        pat_maximum: MAX_LIFETIME,
        client_secret_minimum: Duration::from_secs(DAY_SECONDS),
        client_secret_maximum: MAX_LIFETIME,
    };

    pub fn try_new(
        pat_minimum: Duration,
        pat_default: Duration,
        pat_maximum: Duration,
        client_secret_minimum: Duration,
        client_secret_maximum: Duration,
    ) -> Result<Self, TokenPolicyInputError> {
        if [
            pat_minimum,
            pat_default,
            pat_maximum,
            client_secret_minimum,
            client_secret_maximum,
        ]
        .into_iter()
        .any(|duration| !whole_days(duration) || duration > MAX_LIFETIME)
            || pat_minimum > pat_default
            || pat_default > pat_maximum
            || client_secret_minimum > client_secret_maximum
        {
            return Err(TokenPolicyInputError);
        }
        Ok(Self {
            pat_minimum,
            pat_default,
            pat_maximum,
            client_secret_minimum,
            client_secret_maximum,
        })
    }

    /// An omitted PAT expiry selects the explicitly configured PAT default.
    pub fn pat_expiry(
        self,
        created_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<DateTime<Utc>, TokenPolicyInputError> {
        let expires_at = match expires_at {
            Some(value) => value,
            None => created_at
                .checked_add_signed(
                    TimeDelta::from_std(self.pat_default).map_err(|_| TokenPolicyInputError)?,
                )
                .ok_or(TokenPolicyInputError)?,
        };
        validate_expiry(created_at, expires_at, self.pat_minimum, self.pat_maximum)
    }

    /// Human-created client credentials require an explicit expiry.
    /// Existing bootstrap credentials are outside this policy.
    pub fn client_secret_expiry(
        self,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<DateTime<Utc>, TokenPolicyInputError> {
        validate_expiry(
            created_at,
            expires_at,
            self.client_secret_minimum,
            self.client_secret_maximum,
        )
    }
}

fn whole_days(duration: Duration) -> bool {
    !duration.is_zero()
        && duration.subsec_nanos() == 0
        && duration.as_secs().is_multiple_of(DAY_SECONDS)
}

fn validate_expiry(
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    minimum: Duration,
    maximum: Duration,
) -> Result<DateTime<Utc>, TokenPolicyInputError> {
    let lifetime = (expires_at - created_at)
        .to_std()
        .map_err(|_| TokenPolicyInputError)?;
    if lifetime < minimum || lifetime > maximum {
        return Err(TokenPolicyInputError);
    }
    Ok(expires_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::assert_not_impl_any;

    assert_not_impl_any!(TokenPolicy: Default);
    assert_not_impl_any!(PersonalAccessTokenSecret: fmt::Debug, Clone, serde::Serialize);
    assert_not_impl_any!(AccessAdministrationRequest:
        fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
        From<crate::RequestAccessContext>, From<crate::ApplicationAccessContext>,
        From<SecretMaterial>, From<PersonalAccessTokenSecret>,
        From<crate::AuthenticatedBrowserSession>);

    #[test]
    fn administration_request_retains_browser_evidence_without_claiming_authority() {
        let boundary =
            crate::BrowserRequestBoundaryPolicy::try_new("http://127.0.0.1:8420", "127.0.0.1:8420")
                .unwrap()
                .validate(Some("http://127.0.0.1:8420"), Some("127.0.0.1:8420"))
                .unwrap();
        let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
        let received_at = DateTime::<Utc>::UNIX_EPOCH;
        let request = AccessAdministrationRequest::new(BrowserSessionMutationCommand::new(
            correlation_id,
            SecretMaterial::from_bytes([1; 32]),
            SecretMaterial::from_bytes([2; 32]),
            boundary,
            received_at,
        ));
        let browser = request.browser_request();
        assert_eq!(browser.correlation_id(), correlation_id);
        assert_eq!(browser.now(), received_at);
        assert!(browser
            .session_secret()
            .constant_time_eq(&SecretMaterial::from_bytes([1; 32])));
        assert!(browser
            .csrf_secret()
            .constant_time_eq(&SecretMaterial::from_bytes([2; 32])));
        assert!(!browser
            .session_secret()
            .constant_time_eq(browser.csrf_secret()));
        assert!(std::ptr::eq(browser, request.browser_request()));
    }

    #[test]
    fn pat_encoding_round_trips_and_uses_the_exact_digest_domain() {
        let secret = PersonalAccessTokenSecret::from_secret(SecretMaterial::from_bytes([0; 32]));
        // Independently computed using Node's crypto SHA-256 over the tag and
        // 32 zero bytes. These synthetic bytes are not a persisted credential.
        assert_eq!(
            secret.digest().as_str(),
            "sha256:427b1e41d7ef2b4517ebea06534e2b78087b4c8770bd98e850b0a3c3831ba714"
        );
        let bearer = secret.expose_bearer();
        assert_eq!(bearer.len(), PAT_PREFIX.len() + 64);
        assert_eq!(
            PersonalAccessTokenSecret::try_from_bearer(&bearer)
                .unwrap()
                .digest(),
            secret.digest()
        );
        let client_digest = Sha256Digest::from_bytes(&Sha256::digest([0; 32]).into());
        assert_ne!(client_digest, secret.digest());
        assert!(SecretMaterial::try_from_hex(&bearer).is_err());
    }

    #[test]
    fn pat_parser_rejects_wrong_prefix_lengths_and_noncanonical_hex() {
        for value in [
            String::new(),
            "0".repeat(64),
            format!("FASTI_PAT_{}", "0".repeat(64)),
            format!("fasti_client_{}", "0".repeat(64)),
            format!("fasti_pat_{}", "0".repeat(63)),
            format!("fasti_pat_{}", "0".repeat(65)),
            format!("fasti_pat_{}", "A".repeat(64)),
            format!("fasti_pat_{}", "é".repeat(32)),
            format!(" fasti_pat_{}", "0".repeat(64)),
        ] {
            assert!(PersonalAccessTokenSecret::try_from_bearer(&value).is_err());
        }
        for byte in 0_u8..=127 {
            let mut suffix = "0".repeat(63);
            suffix.push(char::from(byte));
            let parsed = PersonalAccessTokenSecret::try_from_bearer(&format!("fasti_pat_{suffix}"));
            assert_eq!(
                parsed.is_ok(),
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            );
        }
    }

    #[test]
    fn policy_rejects_invalid_durations_in_every_field() {
        let day = Duration::from_secs(DAY_SECONDS);
        for invalid in [
            Duration::ZERO,
            day - Duration::from_secs(1),
            day + Duration::from_nanos(1),
            day + Duration::from_secs(1),
            MAX_LIFETIME + day,
            Duration::MAX,
        ] {
            for index in 0..5 {
                let mut values = [day; 5];
                values[index] = invalid;
                assert!(TokenPolicy::try_new(
                    values[0], values[1], values[2], values[3], values[4]
                )
                .is_err());
            }
        }
        assert!(TokenPolicy::try_new(day, day, day, day, day).is_ok());
        assert!(TokenPolicy::try_new(day, day * 30, MAX_LIFETIME, day, MAX_LIFETIME).is_ok());
        for values in [
            [day * 2, day, day * 3, day, day],
            [day, day * 3, day * 2, day, day],
            [day, day, day, day * 2, day],
        ] {
            assert!(
                TokenPolicy::try_new(values[0], values[1], values[2], values[3], values[4])
                    .is_err()
            );
        }
    }

    #[test]
    fn expiry_preserves_explicit_time_and_exact_duration_bounds() {
        let created = DateTime::parse_from_rfc3339("2026-09-04T09:15:00.123456Z")
            .unwrap()
            .to_utc();
        let policy = TokenPolicy::C2;
        assert_eq!(
            policy.pat_expiry(created, None).unwrap(),
            created + TimeDelta::days(30)
        );
        for days in [1, 30, 365] {
            let expiry = created + TimeDelta::days(days);
            assert_eq!(policy.pat_expiry(created, Some(expiry)).unwrap(), expiry);
            assert_eq!(
                policy.client_secret_expiry(created, expiry).unwrap(),
                expiry
            );
        }
        for delta in [
            TimeDelta::days(-1),
            TimeDelta::zero(),
            TimeDelta::hours(23),
            TimeDelta::days(1) - TimeDelta::nanoseconds(1),
            TimeDelta::days(365) + TimeDelta::nanoseconds(1),
            TimeDelta::days(366),
        ] {
            let expiry = created + delta;
            assert!(policy.pat_expiry(created, Some(expiry)).is_err());
            assert!(policy.client_secret_expiry(created, expiry).is_err());
        }
        assert!(policy.pat_expiry(DateTime::<Utc>::MAX_UTC, None).is_err());
        // User-selected absolute timestamps need not align to server time.
        // Rechecking after a queue delay must not introduce day rounding.
        let expiry = created + TimeDelta::days(30);
        for checked_at in [
            created + TimeDelta::nanoseconds(1),
            created + TimeDelta::hours(1),
        ] {
            assert_eq!(policy.pat_expiry(checked_at, Some(expiry)).unwrap(), expiry);
            assert_eq!(
                policy.client_secret_expiry(checked_at, expiry).unwrap(),
                expiry
            );
        }
        let minimum_expiry = created + TimeDelta::days(1);
        assert!(policy
            .client_secret_expiry(created + TimeDelta::nanoseconds(1), minimum_expiry)
            .is_err());
    }

    #[test]
    fn narrower_policy_does_not_fall_back_to_product_bounds() {
        let day = Duration::from_secs(DAY_SECONDS);
        let policy = TokenPolicy::try_new(day * 2, day * 3, day * 4, day * 5, day * 6).unwrap();
        let created = DateTime::<Utc>::UNIX_EPOCH;
        assert_eq!(
            policy.pat_expiry(created, None).unwrap(),
            created + TimeDelta::days(3)
        );
        assert!(policy
            .pat_expiry(created, Some(created + TimeDelta::days(1)))
            .is_err());
        assert!(policy
            .pat_expiry(created, Some(created + TimeDelta::days(5)))
            .is_err());
        assert!(policy
            .client_secret_expiry(created, created + TimeDelta::days(4))
            .is_err());
        assert!(policy
            .client_secret_expiry(created, created + TimeDelta::days(7))
            .is_err());
    }
}
