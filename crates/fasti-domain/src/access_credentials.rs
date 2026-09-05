//! Access credential labels and application-client classifications.
//!
//! These values describe Fasti authority, not human passwords or provider secrets.

use crate::{
    AccessConsentRevisionId, AuthSubject, AuthSubjectId, AuthSubjectLifecycle, ClientId,
    CredentialId, PersonalAccessTokenId, ProfileGrantId, ProfileId, Sha256Digest,
    TrailBaseActivationState, TrailBaseInstallation, TrailBaseInstanceId, WorkspaceId,
};
use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AccessCredentialInvariantError {
    #[error("credential names must contain 1 to 128 UTF-8 bytes and no control or bidirectional formatting characters")]
    InvalidName,
    #[error("the client authentication type and purpose do not match")]
    InvalidClientClassification,
    #[error("only confidential CLI and integration clients can be registered here")]
    ClientRegistrationUnavailable,
    #[error("a revoked client cannot issue credentials")]
    ClientRevoked,
    #[error("only person-owned confidential clients can issue credentials")]
    ClientRotationUnavailable,
    #[error("the client credential epoch is outside the supported range")]
    InvalidCredentialEpoch,
    #[error("the client credential epoch cannot advance")]
    CredentialEpochOverflow,
    #[error("credential timestamps are not monotonic")]
    InvalidCredentialTimestampOrder,
    #[error("personal access token authority is not active")]
    PersonalAccessTokenUnavailable,
    #[error("personal access token replacement is inconsistent")]
    InvalidPersonalAccessTokenReplacement,
    #[error("consent revision identity or sequence is inconsistent")]
    InvalidConsentRevision,
    #[error("consent authority does not match the active client owner")]
    ConsentAuthorityMismatch,
    #[error("a withdrawn consent chain cannot grant authority again")]
    ConsentRevoked,
}

/// A display label shared by application clients and personal access tokens.
///
/// Labels are trimmed but neither normalized nor unique. Persistence must pass
/// through this constructor too; deserialization must not bypass validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessCredentialName(String);

impl AccessCredentialName {
    pub fn try_new(value: &str) -> Result<Self, AccessCredentialInvariantError> {
        let trimmed = value.trim();
        if trimmed.is_empty()
            || trimmed.len() > 128
            || value.chars().any(|character| {
                character.is_control()
                    || matches!(character,
                        '\u{061c}' | '\u{200e}' | '\u{200f}'
                        | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            })
        {
            return Err(AccessCredentialInvariantError::InvalidName);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAuthenticationType {
    FirstParty,
    Confidential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationClientPurpose {
    Node,
    Cli,
    Device,
    Integration,
}

/// One valid classification of the existing Fasti application-client owner.
/// The node classification is immutable; this type exposes no conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApplicationClientClassification {
    authentication_type: ClientAuthenticationType,
    purpose: ApplicationClientPurpose,
}

impl ApplicationClientClassification {
    pub fn try_from_persisted(
        authentication_type: ClientAuthenticationType,
        purpose: ApplicationClientPurpose,
    ) -> Result<Self, AccessCredentialInvariantError> {
        match (authentication_type, purpose) {
            (ClientAuthenticationType::FirstParty, ApplicationClientPurpose::Node)
            | (
                ClientAuthenticationType::Confidential,
                ApplicationClientPurpose::Cli
                | ApplicationClientPurpose::Device
                | ApplicationClientPurpose::Integration,
            ) => Ok(Self {
                authentication_type,
                purpose,
            }),
            _ => Err(AccessCredentialInvariantError::InvalidClientClassification),
        }
    }

    /// C2 registration excludes bootstrap and the later device-pairing flow.
    pub fn for_registration(
        purpose: ApplicationClientPurpose,
    ) -> Result<Self, AccessCredentialInvariantError> {
        match purpose {
            ApplicationClientPurpose::Cli | ApplicationClientPurpose::Integration => Ok(Self {
                authentication_type: ClientAuthenticationType::Confidential,
                purpose,
            }),
            ApplicationClientPurpose::Node | ApplicationClientPurpose::Device => {
                Err(AccessCredentialInvariantError::ClientRegistrationUnavailable)
            }
        }
    }

    pub const fn authentication_type(self) -> ClientAuthenticationType {
        self.authentication_type
    }

    pub const fn purpose(self) -> ApplicationClientPurpose {
        self.purpose
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationClientLifecycle {
    Active,
    Revoked,
}

/// The existing `clients` aggregate, extended with explicit ownership.
///
/// Classification and ownership cannot be reassigned through this model.
/// Lifecycle and epoch changes must commit with credential/grant changes and
/// audit in one authorized application transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationClient {
    id: ClientId,
    workspace_id: WorkspaceId,
    owner_subject_id: Option<AuthSubjectId>,
    name: Option<AccessCredentialName>,
    classification: ApplicationClientClassification,
    lifecycle: ApplicationClientLifecycle,
    current_credential_epoch: u64,
    created_at: DateTime<Utc>,
}

impl ApplicationClient {
    pub fn register(
        id: ClientId,
        workspace_id: WorkspaceId,
        owner_subject_id: AuthSubjectId,
        name: AccessCredentialName,
        purpose: ApplicationClientPurpose,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        Ok(Self {
            id,
            workspace_id,
            owner_subject_id: Some(owner_subject_id),
            name: Some(name),
            classification: ApplicationClientClassification::for_registration(purpose)?,
            lifecycle: ApplicationClientLifecycle::Active,
            current_credential_epoch: 1,
            created_at,
        })
    }

    /// Historical and archive-restored clients may lack human ownership and a
    /// label. Epoch zero is a valid shell; it does not imply usable credentials.
    #[allow(clippy::too_many_arguments)]
    pub fn try_from_persisted(
        id: ClientId,
        workspace_id: WorkspaceId,
        owner_subject_id: Option<AuthSubjectId>,
        name: Option<AccessCredentialName>,
        classification: ApplicationClientClassification,
        lifecycle: ApplicationClientLifecycle,
        current_credential_epoch: u64,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        if current_credential_epoch > i64::MAX as u64 {
            return Err(AccessCredentialInvariantError::InvalidCredentialEpoch);
        }
        Ok(Self {
            id,
            workspace_id,
            owner_subject_id,
            name,
            classification,
            lifecycle,
            current_credential_epoch,
            created_at,
        })
    }

    pub const fn id(&self) -> ClientId {
        self.id
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn owner_subject_id(&self) -> Option<AuthSubjectId> {
        self.owner_subject_id
    }
    pub const fn name(&self) -> Option<&AccessCredentialName> {
        self.name.as_ref()
    }
    pub const fn classification(&self) -> ApplicationClientClassification {
        self.classification
    }
    pub const fn lifecycle(&self) -> ApplicationClientLifecycle {
        self.lifecycle
    }
    pub const fn current_credential_epoch(&self) -> u64 {
        self.current_credential_epoch
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn advance_credential_epoch(&mut self) -> Result<u64, AccessCredentialInvariantError> {
        self.require_human_credential_management()?;
        let next_epoch = self
            .current_credential_epoch
            .checked_add(1)
            .filter(|epoch| *epoch <= i64::MAX as u64)
            .ok_or(AccessCredentialInvariantError::CredentialEpochOverflow)?;
        self.current_credential_epoch = next_epoch;
        Ok(next_epoch)
    }

    fn require_human_credential_management(&self) -> Result<(), AccessCredentialInvariantError> {
        if self.lifecycle == ApplicationClientLifecycle::Revoked {
            return Err(AccessCredentialInvariantError::ClientRevoked);
        }
        if self.owner_subject_id.is_none()
            || self.classification.authentication_type() != ClientAuthenticationType::Confidential
        {
            return Err(AccessCredentialInvariantError::ClientRotationUnavailable);
        }
        Ok(())
    }

    /// Terminal transition. Returns whether the row changed; repeated
    /// revocation preserves its final epoch and cannot restore authority.
    pub fn revoke(&mut self) -> bool {
        if self.lifecycle == ApplicationClientLifecycle::Revoked {
            return false;
        }
        self.lifecycle = ApplicationClientLifecycle::Revoked;
        true
    }
}

/// Digest-only state of one registered-client credential. Rotation creates a
/// new row; identity, digest, epoch and expiry are immutable on this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredClientCredential {
    id: CredentialId,
    workspace_id: WorkspaceId,
    client_id: ClientId,
    digest: Sha256Digest,
    epoch: u64,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

impl RegisteredClientCredential {
    /// The application must validate the explicit expiry with `TokenPolicy`
    /// and authorize issuance in the same transaction as consent and audit.
    pub fn issue(
        id: CredentialId,
        client: &ApplicationClient,
        digest: Sha256Digest,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        client.require_human_credential_management()?;
        if created_at < client.created_at() {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        Self::try_from_persisted(
            id,
            client.workspace_id(),
            client.id(),
            digest,
            client.current_credential_epoch(),
            created_at,
            Some(expires_at),
            None,
        )
    }

    /// Missing expiry preserves existing node and legacy credentials. The store
    /// must also reject a persisted status that contradicts `revoked_at`.
    #[allow(clippy::too_many_arguments)]
    pub fn try_from_persisted(
        id: CredentialId,
        workspace_id: WorkspaceId,
        client_id: ClientId,
        digest: Sha256Digest,
        epoch: u64,
        created_at: DateTime<Utc>,
        expires_at: Option<DateTime<Utc>>,
        revoked_at: Option<DateTime<Utc>>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        if epoch == 0 || epoch > i64::MAX as u64 {
            return Err(AccessCredentialInvariantError::InvalidCredentialEpoch);
        }
        if expires_at.is_some_and(|at| at <= created_at)
            || revoked_at.is_some_and(|at| at < created_at)
        {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        Ok(Self {
            id,
            workspace_id,
            client_id,
            digest,
            epoch,
            created_at,
            expires_at,
            revoked_at,
        })
    }

    pub const fn id(&self) -> CredentialId {
        self.id
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }
    pub const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }
    pub const fn revoked_at(&self) -> Option<DateTime<Utc>> {
        self.revoked_at
    }

    /// Necessary credential state, not authorization. Current grants, scopes,
    /// actor policy and human authority still require transaction-level checks.
    pub fn is_current_for(&self, client: &ApplicationClient, at: DateTime<Utc>) -> bool {
        self.workspace_id == client.workspace_id()
            && self.client_id == client.id()
            && self.epoch == client.current_credential_epoch()
            && client.lifecycle() == ApplicationClientLifecycle::Active
            && self.revoked_at.is_none()
            && at >= self.created_at
            && at >= client.created_at()
            && self.expires_at.is_none_or(|expiry| at < expiry)
    }

    pub fn revoke(&mut self, at: DateTime<Utc>) -> Result<bool, AccessCredentialInvariantError> {
        if at < self.revoked_at.unwrap_or(self.created_at) {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        if self.revoked_at.is_some() {
            return Ok(false);
        }
        self.revoked_at = Some(at);
        Ok(true)
    }
}

/// Subject-owned token state. ScopeKey sets remain application-owned and are
/// intersected with current grants inside each protected operation transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonalAccessToken {
    id: PersonalAccessTokenId,
    workspace_id: WorkspaceId,
    subject_id: AuthSubjectId,
    profile_grant_id: ProfileGrantId,
    name: AccessCredentialName,
    digest: Sha256Digest,
    auth_epoch: u64,
    authorization_epoch: u64,
    trailbase_instance_id: TrailBaseInstanceId,
    activation_generation: u64,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
    replaced_by: Option<PersonalAccessTokenId>,
}

impl PersonalAccessToken {
    /// Application TokenPolicy, browser authorization, and grant checks must
    /// pass before issuance. Rotation calls this with current authority models,
    /// never with the predecessor's captured epochs or installation generation.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        id: PersonalAccessTokenId,
        workspace_id: WorkspaceId,
        profile_grant_id: ProfileGrantId,
        name: AccessCredentialName,
        digest: Sha256Digest,
        subject: &AuthSubject,
        installation: &TrailBaseInstallation,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        if subject.lifecycle() != AuthSubjectLifecycle::Active
            || installation.activation_state() != TrailBaseActivationState::Active
        {
            return Err(AccessCredentialInvariantError::PersonalAccessTokenUnavailable);
        }
        if created_at < subject.updated_at() || created_at < installation.updated_at() {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        Self::try_from_persisted(
            id,
            workspace_id,
            subject.id(),
            profile_grant_id,
            name,
            digest,
            subject.auth_epoch(),
            subject.authorization_epoch(),
            installation.id(),
            installation.activation_generation(),
            created_at,
            expires_at,
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_from_persisted(
        id: PersonalAccessTokenId,
        workspace_id: WorkspaceId,
        subject_id: AuthSubjectId,
        profile_grant_id: ProfileGrantId,
        name: AccessCredentialName,
        digest: Sha256Digest,
        auth_epoch: u64,
        authorization_epoch: u64,
        trailbase_instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        last_used_at: Option<DateTime<Utc>>,
        revoked_at: Option<DateTime<Utc>>,
        replaced_by: Option<PersonalAccessTokenId>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        if [auth_epoch, authorization_epoch, activation_generation]
            .into_iter()
            .any(|epoch| epoch > i64::MAX as u64)
            || activation_generation == 0
        {
            return Err(AccessCredentialInvariantError::InvalidCredentialEpoch);
        }
        if expires_at <= created_at
            || last_used_at.is_some_and(|at| at < created_at || at >= expires_at)
            || revoked_at.is_some_and(|at| at < last_used_at.unwrap_or(created_at))
        {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        if replaced_by.is_some_and(|replacement| replacement == id || revoked_at.is_none()) {
            return Err(AccessCredentialInvariantError::InvalidPersonalAccessTokenReplacement);
        }
        Ok(Self {
            id,
            workspace_id,
            subject_id,
            profile_grant_id,
            name,
            digest,
            auth_epoch,
            authorization_epoch,
            trailbase_instance_id,
            activation_generation,
            created_at,
            expires_at,
            last_used_at,
            revoked_at,
            replaced_by,
        })
    }

    pub const fn id(&self) -> PersonalAccessTokenId {
        self.id
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn subject_id(&self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn profile_grant_id(&self) -> ProfileGrantId {
        self.profile_grant_id
    }
    pub const fn name(&self) -> &AccessCredentialName {
        &self.name
    }
    pub const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }
    pub const fn auth_epoch(&self) -> u64 {
        self.auth_epoch
    }
    pub const fn authorization_epoch(&self) -> u64 {
        self.authorization_epoch
    }
    pub const fn trailbase_instance_id(&self) -> TrailBaseInstanceId {
        self.trailbase_instance_id
    }
    pub const fn activation_generation(&self) -> u64 {
        self.activation_generation
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
    pub const fn last_used_at(&self) -> Option<DateTime<Utc>> {
        self.last_used_at
    }
    pub const fn revoked_at(&self) -> Option<DateTime<Utc>> {
        self.revoked_at
    }
    pub const fn replaced_by(&self) -> Option<PersonalAccessTokenId> {
        self.replaced_by
    }

    /// Necessary token state only. Membership, grant ownership, effective scopes,
    /// accepted actor and physical-root evidence remain transaction-level checks.
    pub fn is_current_for(
        &self,
        subject: &AuthSubject,
        installation: &TrailBaseInstallation,
        at: DateTime<Utc>,
    ) -> bool {
        self.usable_at(at)
            && subject.id() == self.subject_id
            && subject.lifecycle() == AuthSubjectLifecycle::Active
            && subject.auth_epoch() == self.auth_epoch
            && subject.authorization_epoch() == self.authorization_epoch
            && at >= subject.updated_at()
            && installation.id() == self.trailbase_instance_id
            && installation.activation_state() == TrailBaseActivationState::Active
            && installation.activation_generation() == self.activation_generation
            && at >= installation.updated_at()
    }

    fn usable_at(&self, at: DateTime<Utc>) -> bool {
        self.revoked_at.is_none()
            && at >= self.last_used_at.unwrap_or(self.created_at)
            && at < self.expires_at
    }

    /// Called only after successful current-state authorization. The store owns
    /// the conditional 60-second write throttle and concurrent-loser recheck.
    pub fn record_use(
        &mut self,
        at: DateTime<Utc>,
    ) -> Result<bool, AccessCredentialInvariantError> {
        if !self.usable_at(at) {
            return Err(AccessCredentialInvariantError::PersonalAccessTokenUnavailable);
        }
        if self.last_used_at == Some(at) {
            return Ok(false);
        }
        self.last_used_at = Some(at);
        Ok(true)
    }

    pub fn revoke(&mut self, at: DateTime<Utc>) -> Result<bool, AccessCredentialInvariantError> {
        if at
            < self
                .revoked_at
                .or(self.last_used_at)
                .unwrap_or(self.created_at)
        {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        if self.revoked_at.is_some() {
            return Ok(false);
        }
        self.revoked_at = Some(at);
        Ok(true)
    }

    /// Both rows and audit must commit together. The successor is independently
    /// issued against current authority; old-token bearer validity is irrelevant
    /// to the owner's fresh browser authorization to replace an expired token.
    pub fn replace_with(
        &mut self,
        replacement: &Self,
        at: DateTime<Utc>,
    ) -> Result<(), AccessCredentialInvariantError> {
        if self.revoked_at.is_some()
            || replacement.id == self.id
            || replacement.digest == self.digest
            || replacement.workspace_id != self.workspace_id
            || replacement.subject_id != self.subject_id
            || replacement.profile_grant_id != self.profile_grant_id
            || replacement.created_at != at
            || replacement.last_used_at.is_some()
            || !replacement.usable_at(at)
        {
            return Err(AccessCredentialInvariantError::InvalidPersonalAccessTokenReplacement);
        }
        self.revoke(at)?;
        self.replaced_by = Some(replacement.id);
        Ok(())
    }
}

/// Granted evidence contains the application-computed canonical scope digest.
/// Withdrawn consent has no scopes; it cannot carry a residual grant digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessConsentDecision {
    Granted(Sha256Digest),
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessConsentRevision {
    id: AccessConsentRevisionId,
    workspace_id: WorkspaceId,
    client_id: ClientId,
    subject_id: AuthSubjectId,
    profile_id: ProfileId,
    profile_grant_id: ProfileGrantId,
    revision: u64,
    previous_revision_id: Option<AccessConsentRevisionId>,
    decision: AccessConsentDecision,
    created_at: DateTime<Utc>,
}

impl AccessConsentRevision {
    /// Application authorization must establish that the selected canonical
    /// grant and profile belong to this workspace/client and approving subject.
    #[allow(clippy::too_many_arguments)]
    pub fn grant(
        id: AccessConsentRevisionId,
        client: &ApplicationClient,
        subject: &AuthSubject,
        profile_id: ProfileId,
        profile_grant_id: ProfileGrantId,
        scope_digest: Sha256Digest,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        Self::validate_approver(client, subject, created_at)?;
        Self::try_from_persisted(
            id,
            client.workspace_id(),
            client.id(),
            subject.id(),
            profile_id,
            profile_grant_id,
            1,
            None,
            AccessConsentDecision::Granted(scope_digest),
            created_at,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_from_persisted(
        id: AccessConsentRevisionId,
        workspace_id: WorkspaceId,
        client_id: ClientId,
        subject_id: AuthSubjectId,
        profile_id: ProfileId,
        profile_grant_id: ProfileGrantId,
        revision: u64,
        previous_revision_id: Option<AccessConsentRevisionId>,
        decision: AccessConsentDecision,
        created_at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        if revision == 0
            || revision > i64::MAX as u64
            || (revision == 1) != previous_revision_id.is_none()
            || previous_revision_id == Some(id)
            || (revision == 1 && decision == AccessConsentDecision::Revoked)
        {
            return Err(AccessCredentialInvariantError::InvalidConsentRevision);
        }
        Ok(Self {
            id,
            workspace_id,
            client_id,
            subject_id,
            profile_id,
            profile_grant_id,
            revision,
            previous_revision_id,
            decision,
            created_at,
        })
    }

    pub const fn id(&self) -> AccessConsentRevisionId {
        self.id
    }
    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }
    pub const fn subject_id(&self) -> AuthSubjectId {
        self.subject_id
    }
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }
    pub const fn profile_grant_id(&self) -> ProfileGrantId {
        self.profile_grant_id
    }
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    pub const fn previous_revision_id(&self) -> Option<AccessConsentRevisionId> {
        self.previous_revision_id
    }
    pub const fn decision(&self) -> &AccessConsentDecision {
        &self.decision
    }
    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// The operation transaction must prove this is still the current revision
    /// before appending the successor and changing the canonical grant/scopes.
    pub fn grant_successor(
        &self,
        id: AccessConsentRevisionId,
        client: &ApplicationClient,
        subject: &AuthSubject,
        scope_digest: Sha256Digest,
        at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        Self::validate_approver(client, subject, at)?;
        if client.id() != self.client_id
            || client.workspace_id() != self.workspace_id
            || subject.id() != self.subject_id
        {
            return Err(AccessCredentialInvariantError::ConsentAuthorityMismatch);
        }
        self.successor(id, AccessConsentDecision::Granted(scope_digest), at)
    }

    /// An authorized administrator can withdraw another person's consent.
    /// Authorization is enforced by the transaction, not by this value method.
    pub fn revoke_successor(
        &self,
        id: AccessConsentRevisionId,
        at: DateTime<Utc>,
    ) -> Result<Option<Self>, AccessCredentialInvariantError> {
        if at < self.created_at {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        if self.decision == AccessConsentDecision::Revoked {
            return Ok(None);
        }
        self.successor(id, AccessConsentDecision::Revoked, at)
            .map(Some)
    }

    fn successor(
        &self,
        id: AccessConsentRevisionId,
        decision: AccessConsentDecision,
        at: DateTime<Utc>,
    ) -> Result<Self, AccessCredentialInvariantError> {
        if self.decision == AccessConsentDecision::Revoked {
            return Err(AccessCredentialInvariantError::ConsentRevoked);
        }
        if at < self.created_at {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        let revision = self
            .revision
            .checked_add(1)
            .filter(|next| *next <= i64::MAX as u64)
            .ok_or(AccessCredentialInvariantError::InvalidConsentRevision)?;
        Self::try_from_persisted(
            id,
            self.workspace_id,
            self.client_id,
            self.subject_id,
            self.profile_id,
            self.profile_grant_id,
            revision,
            Some(self.id),
            decision,
            at,
        )
    }

    fn validate_approver(
        client: &ApplicationClient,
        subject: &AuthSubject,
        at: DateTime<Utc>,
    ) -> Result<(), AccessCredentialInvariantError> {
        client.require_human_credential_management()?;
        if client.owner_subject_id() != Some(subject.id())
            || subject.lifecycle() != AuthSubjectLifecycle::Active
        {
            return Err(AccessCredentialInvariantError::ConsentAuthorityMismatch);
        }
        if at < client.created_at() || at < subject.updated_at() {
            return Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registered_client() -> ApplicationClient {
        ApplicationClient::register(
            ClientId::new_v7(),
            WorkspaceId::new_v7(),
            AuthSubjectId::new_v7(),
            AccessCredentialName::try_new("CLI").unwrap(),
            ApplicationClientPurpose::Cli,
            DateTime::<Utc>::UNIX_EPOCH,
        )
        .unwrap()
    }

    fn issued_credential(client: &ApplicationClient) -> RegisteredClientCredential {
        RegisteredClientCredential::issue(
            CredentialId::new_v7(),
            client,
            Sha256Digest::from_bytes(&[0; 32]),
            client.created_at(),
            client.created_at() + chrono::TimeDelta::days(1),
        )
        .unwrap()
    }

    #[test]
    fn credential_validity_requires_exact_client_epoch_and_time_boundaries() {
        let client = registered_client();
        let credential = issued_credential(&client);
        let expiry = credential.expires_at().unwrap();
        let tick = chrono::TimeDelta::nanoseconds(1);
        assert!(!credential.is_current_for(&client, credential.created_at() - tick));
        assert!(credential.is_current_for(&client, credential.created_at()));
        assert!(credential.is_current_for(&client, expiry - tick));
        assert!(!credential.is_current_for(&client, expiry));
        assert!(!credential.is_current_for(&client, expiry + tick));

        for mismatch in 0..5 {
            let mut changed = client.clone();
            match mismatch {
                0 => changed.id = ClientId::new_v7(),
                1 => changed.workspace_id = WorkspaceId::new_v7(),
                2 => {
                    changed.advance_credential_epoch().unwrap();
                }
                3 => {
                    changed.revoke();
                }
                4 => changed.created_at += tick,
                _ => unreachable!(),
            }
            assert!(!credential.is_current_for(&changed, credential.created_at()));
        }
        let mut revoked = credential.clone();
        assert_eq!(revoked.revoke(credential.created_at()), Ok(true));
        assert!(!revoked.is_current_for(&client, credential.created_at()));
    }

    #[test]
    fn credential_revocation_retains_first_time_and_immutable_fields() {
        let client = registered_client();
        let mut credential = issued_credential(&client);
        let original = credential.clone();
        let tick = chrono::TimeDelta::nanoseconds(1);
        assert_eq!(
            credential.revoke(credential.created_at() - tick),
            Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
        );
        assert_eq!(credential, original);
        // Expired credentials remain revocable; this does not revive them.
        let at = credential.expires_at().unwrap() + tick;
        assert_eq!(credential.revoke(at), Ok(true));
        let terminal = credential.clone();
        assert_eq!(credential.revoke(at), Ok(false));
        assert_eq!(credential.revoke(at + tick), Ok(false));
        assert_eq!(
            credential.revoke(at - tick),
            Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
        );
        assert_eq!(credential, terminal);
        assert_eq!(credential.id(), original.id());
        assert_eq!(credential.client_id(), client.id());
        assert_eq!(credential.workspace_id(), client.workspace_id());
        assert_eq!(credential.epoch(), original.epoch());
        assert_eq!(credential.digest(), original.digest());
        assert_eq!(credential.expires_at(), original.expires_at());
        assert_eq!(credential.revoked_at(), Some(at));
    }

    #[test]
    fn credential_construction_preserves_legacy_expiry_but_rejects_invalid_state() {
        let client = registered_client();
        let at = client.created_at();
        let day = chrono::TimeDelta::days(1);
        let persisted = |epoch, expiry, revoked| {
            RegisteredClientCredential::try_from_persisted(
                CredentialId::new_v7(),
                client.workspace_id(),
                client.id(),
                Sha256Digest::from_bytes(&[0; 32]),
                epoch,
                at,
                expiry,
                revoked,
            )
        };
        for epoch in [0, i64::MAX as u64 + 1, u64::MAX] {
            assert_eq!(
                persisted(epoch, None, None),
                Err(AccessCredentialInvariantError::InvalidCredentialEpoch)
            );
        }
        assert!(persisted(i64::MAX as u64, None, None).is_ok());
        for (expiry, revoked) in [
            (Some(at), None),
            (Some(at - day), None),
            (None, Some(at - day)),
        ] {
            assert_eq!(
                persisted(1, expiry, revoked),
                Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
            );
        }
        let legacy = persisted(1, None, None).unwrap();
        assert!(legacy.expires_at().is_none());
        let mut ownerless = client.clone();
        ownerless.owner_subject_id = None;
        assert!(legacy.is_current_for(&ownerless, at + day * 1000));
        let mut node = ownerless.clone();
        node.classification = ApplicationClientClassification::try_from_persisted(
            ClientAuthenticationType::FirstParty,
            ApplicationClientPurpose::Node,
        )
        .unwrap();
        assert!(legacy.is_current_for(&node, at + day * 1000));
        let mut revoked_client = client.clone();
        revoked_client.revoke();
        let mut zero_epoch = client.clone();
        zero_epoch.current_credential_epoch = 0;
        for disallowed in [&ownerless, &node, &revoked_client, &zero_epoch] {
            assert!(RegisteredClientCredential::issue(
                CredentialId::new_v7(),
                disallowed,
                Sha256Digest::from_bytes(&[0; 32]),
                at,
                at + day,
            )
            .is_err());
        }
        for (created, expires) in [(at - day, at), (at, at), (at, at - day)] {
            assert_eq!(
                RegisteredClientCredential::issue(
                    CredentialId::new_v7(),
                    &client,
                    Sha256Digest::from_bytes(&[0; 32]),
                    created,
                    expires,
                ),
                Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
            );
        }
    }

    #[test]
    fn client_revocation_is_terminal_and_retains_identity_and_epoch() {
        let mut client = registered_client();
        let id = client.id();
        let owner = client.owner_subject_id();
        let workspace = client.workspace_id();
        assert_eq!(client.name().unwrap().as_str(), "CLI");
        assert_eq!(client.created_at(), DateTime::<Utc>::UNIX_EPOCH);
        assert_eq!(client.current_credential_epoch(), 1);
        assert_eq!(client.advance_credential_epoch(), Ok(2));
        assert!(client.revoke());
        let terminal = client.clone();
        assert!(!client.revoke());
        assert_eq!(
            client.advance_credential_epoch(),
            Err(AccessCredentialInvariantError::ClientRevoked)
        );
        assert_eq!(client, terminal);
        assert_eq!(client.id(), id);
        assert_eq!(client.workspace_id(), workspace);
        assert_eq!(client.owner_subject_id(), owner);
        assert_eq!(client.current_credential_epoch(), 2);
        assert_eq!(client.lifecycle(), ApplicationClientLifecycle::Revoked);
    }

    #[test]
    fn historical_clients_remain_representable_without_rotation_authority() {
        for (authentication, purpose) in [
            (
                ClientAuthenticationType::FirstParty,
                ApplicationClientPurpose::Node,
            ),
            (
                ClientAuthenticationType::Confidential,
                ApplicationClientPurpose::Integration,
            ),
            (
                ClientAuthenticationType::Confidential,
                ApplicationClientPurpose::Device,
            ),
        ] {
            let classification =
                ApplicationClientClassification::try_from_persisted(authentication, purpose)
                    .unwrap();
            let mut client = ApplicationClient::try_from_persisted(
                ClientId::new_v7(),
                WorkspaceId::new_v7(),
                None,
                None,
                classification,
                ApplicationClientLifecycle::Active,
                0,
                DateTime::<Utc>::UNIX_EPOCH,
            )
            .unwrap();
            let before = client.clone();
            assert_eq!(
                client.advance_credential_epoch(),
                Err(AccessCredentialInvariantError::ClientRotationUnavailable)
            );
            assert_eq!(client, before);
            assert_eq!(client.classification(), classification);
            assert!(client.name().is_none());
            assert!(client.owner_subject_id().is_none());
            assert!(client.revoke());
        }
        let mut node = registered_client();
        node.classification = ApplicationClientClassification::try_from_persisted(
            ClientAuthenticationType::FirstParty,
            ApplicationClientPurpose::Node,
        )
        .unwrap();
        let before = node.clone();
        assert_eq!(
            node.advance_credential_epoch(),
            Err(AccessCredentialInvariantError::ClientRotationUnavailable)
        );
        assert_eq!(node, before);
    }

    #[test]
    fn client_epoch_overflow_does_not_partially_mutate() {
        let client = registered_client();
        let restored = |epoch| {
            ApplicationClient::try_from_persisted(
                client.id(),
                client.workspace_id(),
                client.owner_subject_id(),
                client.name().cloned(),
                client.classification(),
                client.lifecycle(),
                epoch,
                client.created_at(),
            )
        };
        assert_eq!(
            restored(i64::MAX as u64 + 1),
            Err(AccessCredentialInvariantError::InvalidCredentialEpoch)
        );
        let mut at_limit = restored(i64::MAX as u64).unwrap();
        let before = at_limit.clone();
        assert_eq!(
            at_limit.advance_credential_epoch(),
            Err(AccessCredentialInvariantError::CredentialEpochOverflow)
        );
        assert_eq!(at_limit, before);
        let mut below_limit = restored(i64::MAX as u64 - 1).unwrap();
        assert_eq!(below_limit.advance_credential_epoch(), Ok(i64::MAX as u64));
    }

    #[test]
    fn registration_cannot_create_node_or_device_clients() {
        for purpose in [
            ApplicationClientPurpose::Node,
            ApplicationClientPurpose::Device,
        ] {
            assert_eq!(
                ApplicationClient::register(
                    ClientId::new_v7(),
                    WorkspaceId::new_v7(),
                    AuthSubjectId::new_v7(),
                    AccessCredentialName::try_new("client").unwrap(),
                    purpose,
                    DateTime::<Utc>::UNIX_EPOCH
                ),
                Err(AccessCredentialInvariantError::ClientRegistrationUnavailable)
            );
        }
    }

    #[test]
    fn names_enforce_utf8_byte_boundaries_and_preserve_display_text() {
        for value in ["", "   ", &"a".repeat(129), &"é".repeat(65)] {
            assert_eq!(
                AccessCredentialName::try_new(value),
                Err(AccessCredentialInvariantError::InvalidName)
            );
        }
        for value in ["a".repeat(127), "a".repeat(128), "é".repeat(64)] {
            let name = AccessCredentialName::try_new(&value).expect("bounded label");
            assert_eq!(name.as_str(), value);
        }
        let name = AccessCredentialName::try_new("  Ryan’s CLI  ").expect("label");
        assert_eq!(name.as_str(), "Ryan’s CLI");
        assert_eq!(AccessCredentialName::try_new(name.as_str()).unwrap(), name);
        let decomposed = "e\u{0301}";
        assert_eq!(
            AccessCredentialName::try_new(decomposed).unwrap().as_str(),
            decomposed
        );
    }

    #[test]
    fn names_reject_controls_even_at_trimmed_edges() {
        for character in [
            '\0', '\r', '\n', '\t', '\u{7f}', '\u{85}', '\u{061c}', '\u{200e}', '\u{200f}',
            '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}',
            '\u{2068}', '\u{2069}',
        ] {
            for value in [
                format!("{character}CLI"),
                format!("C{character}LI"),
                format!("CLI{character}"),
            ] {
                assert_eq!(
                    AccessCredentialName::try_new(&value),
                    Err(AccessCredentialInvariantError::InvalidName)
                );
            }
        }
        assert!(AccessCredentialName::try_new("日本語 العربية").is_ok());
    }

    #[test]
    fn persisted_classifications_and_registration_have_distinct_boundaries() {
        use ApplicationClientPurpose::{Cli, Device, Integration, Node};
        use ClientAuthenticationType::{Confidential, FirstParty};
        for authentication in [FirstParty, Confidential] {
            for purpose in [Node, Cli, Device, Integration] {
                let classification =
                    ApplicationClientClassification::try_from_persisted(authentication, purpose);
                let valid = matches!(
                    (authentication, purpose),
                    (FirstParty, Node) | (Confidential, Cli | Device | Integration)
                );
                assert_eq!(classification.is_ok(), valid);
                if let Ok(classification) = classification {
                    assert_eq!(classification.authentication_type(), authentication);
                    assert_eq!(classification.purpose(), purpose);
                }
            }
        }
        for purpose in [Node, Cli, Device, Integration] {
            let registered = ApplicationClientClassification::for_registration(purpose);
            assert_eq!(registered.is_ok(), matches!(purpose, Cli | Integration));
            if let Ok(registered) = registered {
                assert_eq!(registered.authentication_type(), Confidential);
                assert_eq!(registered.purpose(), purpose);
            }
        }
    }
}
