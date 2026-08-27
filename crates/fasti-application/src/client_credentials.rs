use crate::{ApplicationResult, RequestAccessContext, ScopeKey, SecretMaterial};
use chrono::{DateTime, Utc};
use fasti_domain::{ClientId, CredentialId, ProfileId, RequestCorrelationId};

/// Create one independently revocable client credential for the caller's profile.
///
/// Plaintext credential material is returned only from the successful outcome.
/// Persistence stores only its digest.
pub struct CreateScopedClientCredentialCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    scopes: Vec<ScopeKey>,
}

impl CreateScopedClientCredentialCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        scopes: Vec<ScopeKey>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            scopes,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub fn scopes(&self) -> &[ScopeKey] {
        &self.scopes
    }
}

pub struct CreateScopedClientCredentialOutcome {
    client_id: ClientId,
    credential_id: CredentialId,
    profile_id: ProfileId,
    scopes: Vec<ScopeKey>,
    credential: SecretMaterial,
    created_at: DateTime<Utc>,
}

impl CreateScopedClientCredentialOutcome {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client_id: ClientId,
        credential_id: CredentialId,
        profile_id: ProfileId,
        scopes: Vec<ScopeKey>,
        credential: SecretMaterial,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            client_id,
            credential_id,
            profile_id,
            scopes,
            credential,
            created_at,
        }
    }

    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn scopes(&self) -> &[ScopeKey] {
        &self.scopes
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }

    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCredentialSummary {
    client_id: ClientId,
    credential_id: CredentialId,
    profile_id: ProfileId,
    scopes: Vec<ScopeKey>,
    active: bool,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

impl ClientCredentialSummary {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        client_id: ClientId,
        credential_id: CredentialId,
        profile_id: ProfileId,
        scopes: Vec<ScopeKey>,
        active: bool,
        created_at: DateTime<Utc>,
        revoked_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            client_id,
            credential_id,
            profile_id,
            scopes,
            active,
            created_at,
            revoked_at,
        }
    }

    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn scopes(&self) -> &[ScopeKey] {
        &self.scopes
    }

    pub const fn active(&self) -> bool {
        self.active
    }

    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub const fn revoked_at(&self) -> Option<DateTime<Utc>> {
        self.revoked_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListClientCredentialsQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl ListClientCredentialsQuery {
    pub const fn new(correlation_id: RequestCorrelationId, access: RequestAccessContext) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevokeClientCredentialCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    credential_id: CredentialId,
}

impl RevokeClientCredentialCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        credential_id: CredentialId,
    ) -> Self {
        Self {
            correlation_id,
            access,
            credential_id,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }
}

pub trait ClientCredentialAdministrationPort: Send + Sync {
    fn create_scoped_client_credential(
        &self,
        command: CreateScopedClientCredentialCommand,
    ) -> ApplicationResult<CreateScopedClientCredentialOutcome>;

    fn list_client_credentials(
        &self,
        query: ListClientCredentialsQuery,
    ) -> ApplicationResult<Vec<ClientCredentialSummary>>;

    fn revoke_client_credential(
        &self,
        command: RevokeClientCredentialCommand,
    ) -> ApplicationResult<()>;
}
