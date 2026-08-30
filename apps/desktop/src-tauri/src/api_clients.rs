use crate::setup::{self, DesktopProblem, SetupSecretStore};
use fasti_application::{
    ClientCredentialAdministrationPort, ClientCredentialSummary,
    CreateScopedClientCredentialCommand, ListClientCredentialsQuery, RevokeClientCredentialCommand,
    ScopeKey,
};
use fasti_domain::{CredentialId, RequestCorrelationId};
use fasti_store::SqliteKernel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateApiClientInput {
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RevokeApiClientInput {
    pub credential_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApiClientSummary {
    pub client_id: String,
    pub credential_id: String,
    pub profile_id: String,
    pub scopes: Vec<String>,
    pub active: bool,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct CreatedApiClient {
    #[serde(flatten)]
    pub summary: ApiClientSummary,
    pub credential: String,
}

fn parse_scope(value: &str) -> Option<ScopeKey> {
    ScopeKey::ALL
        .iter()
        .copied()
        .find(|scope| scope.as_str() == value)
}

fn parse_scopes(values: Vec<String>) -> Result<Vec<ScopeKey>, DesktopProblem> {
    if values.is_empty() {
        return Err(DesktopProblem::invalid_input(
            "Select at least one client scope.",
        ));
    }
    let mut scopes = Vec::with_capacity(values.len());
    for value in values {
        let scope = parse_scope(&value).ok_or_else(|| {
            DesktopProblem::invalid_input(format!("Unknown client scope: {value}"))
        })?;
        if scope == ScopeKey::ClientEnroll || scopes.contains(&scope) {
            return Err(DesktopProblem::invalid_input(
                "Client scopes must be unique and cannot include bootstrap enrollment.",
            ));
        }
        scopes.push(scope);
    }
    Ok(scopes)
}

fn summary(value: ClientCredentialSummary) -> ApiClientSummary {
    ApiClientSummary {
        client_id: value.client_id().to_string(),
        credential_id: value.credential_id().to_string(),
        profile_id: value.profile_id().to_string(),
        scopes: value
            .scopes()
            .iter()
            .map(|scope| scope.as_str().to_owned())
            .collect(),
        active: value.active(),
        created_at: value.created_at().to_rfc3339(),
        revoked_at: value.revoked_at().map(|value| value.to_rfc3339()),
    }
}

pub(crate) fn list(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<Vec<ApiClientSummary>, DesktopProblem> {
    let access =
        setup::authenticate(kernel, store)?.ok_or_else(DesktopProblem::not_authenticated)?;
    kernel
        .list_client_credentials(ListClientCredentialsQuery::new(
            RequestCorrelationId::new_v7(),
            access,
        ))
        .map(|values| values.into_iter().map(summary).collect())
        .map_err(|problem| DesktopProblem::application(&problem))
}

pub(crate) fn create(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    input: CreateApiClientInput,
) -> Result<CreatedApiClient, DesktopProblem> {
    let access =
        setup::authenticate(kernel, store)?.ok_or_else(DesktopProblem::not_authenticated)?;
    let scopes = parse_scopes(input.scopes)?;
    let outcome = kernel
        .create_scoped_client_credential(CreateScopedClientCredentialCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            scopes,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(CreatedApiClient {
        summary: ApiClientSummary {
            client_id: outcome.client_id().to_string(),
            credential_id: outcome.credential_id().to_string(),
            profile_id: outcome.profile_id().to_string(),
            scopes: outcome
                .scopes()
                .iter()
                .map(|scope| scope.as_str().to_owned())
                .collect(),
            active: true,
            created_at: outcome.created_at().to_rfc3339(),
            revoked_at: None,
        },
        credential: outcome.credential().expose_hex(),
    })
}

pub(crate) fn revoke(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    input: RevokeApiClientInput,
) -> Result<Vec<ApiClientSummary>, DesktopProblem> {
    let access =
        setup::authenticate(kernel, store)?.ok_or_else(DesktopProblem::not_authenticated)?;
    let credential_id = input
        .credential_id
        .parse::<CredentialId>()
        .map_err(|_| DesktopProblem::invalid_input("Credential ID is invalid."))?;
    kernel
        .revoke_client_credential(RevokeClientCredentialCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            credential_id,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    list(kernel, store)
}
