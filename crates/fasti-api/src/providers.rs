use crate::local::{
    application_request_authentication, authenticate_application_request, authenticate_request,
    request_authentication,
};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use crate::ProviderOperationLocks;
use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use fasti_application::{
    credential_status_after_failed_check, credential_status_after_successful_check,
    ApplicationAccessContext, BrowserRequestBoundaryPolicy, CapabilityKey, ConfigurationDigest,
    CredentialReference, CredentialRequirement, CredentialSecret, CredentialVaultSource,
    FastiProblem, OutboundAccessPolicy, ProblemCode, ProviderCapabilityId, ProviderCapabilityState,
    ProviderCapabilityStatus, ProviderCheckKind, ProviderCheckMetadata, ProviderCheckStatus,
    ProviderCredentialStatus, ProviderId, ProviderStatePort, ProviderStatePortError,
    RequestAccessContext, Violation, MAX_PROVIDER_CREDENTIAL_BYTES,
};
use fasti_contracts::{
    ConfigureProviderCredentialRequest, CredentialRequirementDto, ListProvidersResponse,
    ProblemDetails, ProviderCapabilityDto, ProviderCapabilityResponse, ProviderCapabilityStateDto,
    ProviderCheckDto, ProviderCheckStateDto, ProviderCredentialSourceDto,
    ProviderCredentialStateDto, ProviderDescriptorDto, ProviderHealthResponse, ProviderKindDto,
};
use fasti_domain::{RequestCorrelationId, WorkspaceId};
use fasti_provider_runtime::{
    ProviderCapabilitySpec, ProviderKind, ProviderRuntime, ProviderRuntimeError, ProviderSpec,
};
use std::{collections::BTreeMap, sync::Arc};

const MAX_PROVIDER_JSON_BODY_BYTES: usize = 8 * 1024;
type HttpResult<T> = Result<Json<T>, HttpProblem>;

#[derive(Clone)]
pub(crate) struct ProviderApiState {
    pub(crate) kernel: Arc<dyn fasti_application::LocalKernel>,
    pub(crate) provider_state: Arc<dyn ProviderStatePort>,
    pub(crate) runtime: Arc<ProviderRuntime>,
    pub(crate) provider_operation_locks: ProviderOperationLocks,
    pub(crate) browser_boundary: Option<BrowserRequestBoundaryPolicy>,
}

impl ProviderApiState {
    fn operation_lock(&self, provider: &ProviderSpec) -> Arc<tokio::sync::Mutex<()>> {
        self.provider_operation_locks
            .get(provider.provider)
            .expect("resolved providers have operation locks")
    }
}

async fn run_provider_state_operation<T: Send + 'static>(
    gate: Arc<tokio::sync::Mutex<()>>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    operation: impl std::future::Future<Output = Result<T, HttpProblem>> + Send + 'static,
) -> Result<T, HttpProblem> {
    // Cancelled waiters do nothing. Once admitted, finish vault and state reconciliation
    // under the same gate even if the request disappears; blocking writes cannot abort.
    let guard = gate.lock_owned().await;
    tokio::spawn(async move {
        let _guard = guard;
        operation.await
    })
    .await
    .map_err(|_| storage_problem(capability, correlation_id))?
}

async fn authorize(
    state: &ProviderApiState,
    headers: &HeaderMap,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<RequestAccessContext, HttpProblem> {
    let authentication = request_authentication(headers, capability, correlation_id)?;
    let kernel = Arc::clone(&state.kernel);
    tokio::task::spawn_blocking(move || {
        authenticate_request(kernel.as_ref(), authentication, capability, correlation_id)
    })
    .await
    .map_err(|_| storage_problem(capability, correlation_id))?
    .map_err(application_problem)
}

fn storage_problem(capability: CapabilityKey, correlation_id: RequestCorrelationId) -> HttpProblem {
    application_problem(Box::new(FastiProblem::storage_unavailable(
        capability,
        correlation_id,
    )))
}

fn state_problem(
    error: ProviderStatePortError,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    application_problem(Box::new(match error {
        ProviderStatePortError::Unavailable => {
            FastiProblem::storage_unavailable(capability, correlation_id)
        }
        ProviderStatePortError::Corrupt | ProviderStatePortError::RevisionConflict => {
            FastiProblem::integrity_failed(capability, correlation_id)
        }
    }))
}

async fn list_states(
    state: &ProviderApiState,
    workspace_id: WorkspaceId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<Vec<ProviderCapabilityState>, HttpProblem> {
    let port = Arc::clone(&state.provider_state);
    tokio::task::spawn_blocking(move || port.list_provider_capability_states(workspace_id))
        .await
        .map_err(|_| storage_problem(capability, correlation_id))?
        .map_err(|error| state_problem(error, capability, correlation_id))
}

async fn get_state(
    state: &ProviderApiState,
    workspace_id: WorkspaceId,
    provider_id: ProviderId,
    capability_id: ProviderCapabilityId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<Option<ProviderCapabilityState>, HttpProblem> {
    let port = Arc::clone(&state.provider_state);
    tokio::task::spawn_blocking(move || {
        port.get_provider_capability_state(workspace_id, &provider_id, &capability_id)
    })
    .await
    .map_err(|_| storage_problem(capability, correlation_id))?
    .map_err(|error| state_problem(error, capability, correlation_id))
}

async fn put_state(
    state: &ProviderApiState,
    workspace_id: WorkspaceId,
    value: ProviderCapabilityState,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<(), HttpProblem> {
    let port = Arc::clone(&state.provider_state);
    tokio::task::spawn_blocking(move || port.put_provider_capability_state(workspace_id, value))
        .await
        .map_err(|_| storage_problem(capability, correlation_id))?
        .map(|_| ())
        .map_err(|error| state_problem(error, capability, correlation_id))
}

async fn provider_states(
    state: &ProviderApiState,
    workspace_id: WorkspaceId,
    provider: &'static ProviderSpec,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<Vec<(&'static ProviderCapabilitySpec, ProviderCapabilityState)>, HttpProblem> {
    let persisted = list_states(state, workspace_id, capability, correlation_id).await?;
    provider
        .capabilities
        .iter()
        .map(|spec| {
            let current = persisted
                .iter()
                .find(|item| {
                    item.provider_id().as_str() == provider.provider
                        && item.capability_id().as_str() == spec.capability_id
                })
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| initial_state(&state.runtime, provider, spec))
                .map_err(|error| runtime_problem(&error, capability, correlation_id))?;
            Ok((spec, current))
        })
        .collect()
}

fn runtime_problem(
    error: &ProviderRuntimeError,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    let mut code = error.problem_code();
    if capability == CapabilityKey::ReadProviderHealth
        && matches!(
            code,
            ProblemCode::ProviderCredentialMissing
                | ProblemCode::ProviderCredentialInvalid
                | ProblemCode::ProviderCredentialExpired
        )
    {
        code = ProblemCode::ProviderUnavailable;
    }
    if capability == CapabilityKey::ConfigureProviderCredential
        && !matches!(
            code,
            ProblemCode::ProviderCredentialInvalid | ProblemCode::ProviderUnavailable
        )
    {
        code = ProblemCode::ProviderUnavailable;
    }
    application_problem(Box::new(FastiProblem::from_code(
        code,
        capability,
        correlation_id,
    )))
}

fn invalid_path(capability: CapabilityKey, correlation_id: RequestCorrelationId) -> HttpProblem {
    if capability != CapabilityKey::ConfigureProviderCredential {
        return application_problem(Box::new(FastiProblem::from_code(
            ProblemCode::ProviderRouteUnavailable,
            capability,
            correlation_id,
        )));
    }
    let violation = Violation::try_new(
        "invalid_provider_capability",
        "/",
        "provider ID or capability ID is not declared by the runtime",
        "a provider and capability returned by GET /api/v1/providers",
    )
    .expect("adapter-owned provider violation is valid");
    application_problem(Box::new(
        FastiProblem::validation_failed(capability, correlation_id, vec![violation])
            .expect("one provider violation is within bounds"),
    ))
}

fn invalid_credential(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    let violation = Violation::try_new(
        "invalid_provider_credential",
        "/secret",
        "provider credential must contain visible ASCII characters",
        "1 to 4096 visible ASCII characters",
    )
    .expect("adapter-owned credential violation is valid");
    application_problem(Box::new(
        FastiProblem::validation_failed(capability, correlation_id, vec![violation])
            .expect("one credential violation is within bounds"),
    ))
}

fn resolve(
    runtime: &ProviderRuntime,
    provider_id: &str,
    capability_id: &str,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<(&'static ProviderSpec, &'static ProviderCapabilitySpec), HttpProblem> {
    let provider = runtime
        .descriptor(provider_id)
        .map_err(|_| invalid_path(capability, correlation_id))?;
    if !provider.runtime_available {
        return Err(application_problem(Box::new(FastiProblem::from_code(
            ProblemCode::ProviderUnavailable,
            capability,
            correlation_id,
        ))));
    }
    let provider_capability = provider
        .capabilities
        .iter()
        .find(|item| item.capability_id == capability_id)
        .ok_or_else(|| invalid_path(capability, correlation_id))?;
    Ok((provider, provider_capability))
}

fn initial_state(
    runtime: &ProviderRuntime,
    provider: &ProviderSpec,
    capability: &ProviderCapabilitySpec,
) -> Result<ProviderCapabilityState, ProviderRuntimeError> {
    let reference = runtime.credential_reference(provider.provider)?;
    let source = runtime.credential_source(&reference)?;
    let present = source != CredentialVaultSource::None;
    ProviderCapabilityState::try_new(
        ProviderId::try_new(provider.provider)
            .map_err(|_| ProviderRuntimeError::configuration("Invalid provider ID."))?,
        ProviderCapabilityId::try_new(capability.capability_id)
            .map_err(|_| ProviderRuntimeError::configuration("Invalid capability ID."))?,
        if present {
            ProviderCapabilityStatus::Available
        } else {
            ProviderCapabilityStatus::Unavailable
        },
        1,
        capability.credential_requirement,
        present.then_some(reference),
        if present {
            ProviderCredentialStatus::StoredUnverified
        } else {
            match capability.credential_requirement {
                CredentialRequirement::None | CredentialRequirement::UserAgentOnly => {
                    ProviderCredentialStatus::NotRequired
                }
                CredentialRequirement::OptionalApiKey => ProviderCredentialStatus::Optional,
                _ => ProviderCredentialStatus::Missing,
            }
        },
        runtime.configuration_digest(provider.provider, capability.capability_id)?,
        ProviderCheckMetadata::never_run(),
        ProviderCheckMetadata::never_run(),
    )
    .map_err(|_| ProviderRuntimeError::configuration("Invalid provider state."))
}

#[allow(clippy::too_many_arguments)]
fn next_state(
    current: &ProviderCapabilityState,
    capability_status: ProviderCapabilityStatus,
    credential_reference: Option<CredentialReference>,
    credential_status: ProviderCredentialStatus,
    configuration_digest: ConfigurationDigest,
    health: ProviderCheckMetadata,
    credential_test: ProviderCheckMetadata,
) -> Result<ProviderCapabilityState, ProviderStatePortError> {
    ProviderCapabilityState::try_new(
        current.provider_id().clone(),
        current.capability_id().clone(),
        capability_status,
        current
            .capability_version()
            .checked_add(1)
            .ok_or(ProviderStatePortError::Corrupt)?,
        current.credential_requirement(),
        credential_reference,
        credential_status,
        configuration_digest,
        health,
        credential_test,
    )
    .map_err(|_| ProviderStatePortError::Corrupt)
}

fn check_dto(value: &ProviderCheckMetadata) -> ProviderCheckDto {
    ProviderCheckDto {
        state: match value.status() {
            ProviderCheckStatus::NeverRun => ProviderCheckStateDto::NeverRun,
            ProviderCheckStatus::Passed => ProviderCheckStateDto::Passed,
            ProviderCheckStatus::Failed => ProviderCheckStateDto::Failed,
            ProviderCheckStatus::Unavailable => ProviderCheckStateDto::Unavailable,
        },
        checked_at: value.checked_at().map(|time| time.to_rfc3339()),
        safe_problem_code: value
            .safe_problem_code()
            .map(|code| code.as_str().to_owned()),
    }
}

fn credential_source(
    runtime: &ProviderRuntime,
    state: Option<&ProviderCapabilityState>,
) -> (ProviderCredentialSourceDto, bool, bool) {
    let Some(reference) = state.and_then(ProviderCapabilityState::credential_reference) else {
        return (ProviderCredentialSourceDto::None, true, false);
    };
    match runtime.credential_source(reference) {
        Ok(CredentialVaultSource::Environment) => {
            (ProviderCredentialSourceDto::Environment, true, true)
        }
        Ok(CredentialVaultSource::CredentialStore) => {
            (ProviderCredentialSourceDto::CredentialStore, true, true)
        }
        Ok(CredentialVaultSource::OperatorSecretMount) => {
            (ProviderCredentialSourceDto::OperatorSecretMount, true, true)
        }
        Ok(CredentialVaultSource::None) => (ProviderCredentialSourceDto::None, true, false),
        Err(_) => (ProviderCredentialSourceDto::None, false, false),
    }
}

fn capability_dto(
    runtime: &ProviderRuntime,
    runtime_available: bool,
    spec: &ProviderCapabilitySpec,
    state: Option<&ProviderCapabilityState>,
) -> ProviderCapabilityDto {
    let (credential_source, vault_available, credential_present) =
        credential_source(runtime, state);
    ProviderCapabilityDto {
        capability_id: spec.capability_id.to_owned(),
        purpose: match spec.capability_id {
            "metadata.search" => "Search provider metadata",
            "metadata.read" => "Read provider metadata",
            _ => "Use this provider capability",
        }
        .to_owned(),
        credential_requirement: match spec.credential_requirement {
            CredentialRequirement::None => CredentialRequirementDto::None,
            CredentialRequirement::OptionalApiKey => CredentialRequirementDto::OptionalApiKey,
            CredentialRequirement::ApiKey => CredentialRequirementDto::ApiKey,
            CredentialRequirement::BearerToken => CredentialRequirementDto::BearerToken,
            CredentialRequirement::BasicAuth => CredentialRequirementDto::BasicAuth,
            CredentialRequirement::Oauth2 => CredentialRequirementDto::Oauth2,
            CredentialRequirement::UserAgentOnly => CredentialRequirementDto::UserAgentOnly,
            CredentialRequirement::CustomHeader => CredentialRequirementDto::CustomHeader,
            CredentialRequirement::OperatorSecretMount => {
                CredentialRequirementDto::OperatorSecretMount
            }
        },
        credential_state: match (
            vault_available,
            credential_present,
            state.map(ProviderCapabilityState::credential_status),
        ) {
            (false, _, _) => ProviderCredentialStateDto::Unavailable,
            (true, false, Some(status))
                if !matches!(
                    status,
                    ProviderCredentialStatus::NotRequired | ProviderCredentialStatus::Optional
                ) =>
            {
                ProviderCredentialStateDto::Missing
            }
            (_, _, None) => match spec.credential_requirement {
                CredentialRequirement::None | CredentialRequirement::UserAgentOnly => {
                    ProviderCredentialStateDto::NotRequired
                }
                CredentialRequirement::OptionalApiKey => ProviderCredentialStateDto::Optional,
                _ => ProviderCredentialStateDto::Missing,
            },
            (_, _, Some(ProviderCredentialStatus::Missing)) => ProviderCredentialStateDto::Missing,
            (_, _, Some(ProviderCredentialStatus::NotRequired)) => {
                ProviderCredentialStateDto::NotRequired
            }
            (_, _, Some(ProviderCredentialStatus::Optional)) => {
                ProviderCredentialStateDto::Optional
            }
            (_, _, Some(ProviderCredentialStatus::StoredUnverified)) => {
                ProviderCredentialStateDto::StoredUnverified
            }
            (_, _, Some(ProviderCredentialStatus::Valid)) => ProviderCredentialStateDto::Valid,
            (_, _, Some(ProviderCredentialStatus::Invalid)) => ProviderCredentialStateDto::Invalid,
            (_, _, Some(ProviderCredentialStatus::Expired)) => ProviderCredentialStateDto::Expired,
            (_, _, Some(ProviderCredentialStatus::Unavailable)) => {
                ProviderCredentialStateDto::Unavailable
            }
            (_, _, Some(ProviderCredentialStatus::Revoked)) => ProviderCredentialStateDto::Revoked,
        },
        credential_source,
        state: match (
            vault_available,
            credential_present,
            state.map(ProviderCapabilityState::capability_status),
        ) {
            (false, _, _) => ProviderCapabilityStateDto::Degraded,
            (true, false, Some(ProviderCapabilityStatus::Available)) => {
                ProviderCapabilityStateDto::Unavailable
            }
            (_, _, Some(ProviderCapabilityStatus::Available)) => {
                ProviderCapabilityStateDto::Available
            }
            (_, _, Some(ProviderCapabilityStatus::Degraded)) => {
                ProviderCapabilityStateDto::Degraded
            }
            (_, _, Some(ProviderCapabilityStatus::Disabled)) => {
                ProviderCapabilityStateDto::Disabled
            }
            (_, _, Some(ProviderCapabilityStatus::Unavailable) | None) => {
                ProviderCapabilityStateDto::Unavailable
            }
        },
        version: state
            .map(ProviderCapabilityState::capability_version)
            .unwrap_or(0),
        writable: runtime_available
            && vault_available
            && !matches!(
                credential_source,
                ProviderCredentialSourceDto::Environment
                    | ProviderCredentialSourceDto::OperatorSecretMount
            )
            && !matches!(
                spec.credential_requirement,
                CredentialRequirement::None | CredentialRequirement::UserAgentOnly
            ),
        testable: spec.credential_test,
        health: state
            .map(ProviderCapabilityState::health)
            .map(check_dto)
            .unwrap_or_else(|| check_dto(&ProviderCheckMetadata::never_run())),
        credential_test: state
            .map(ProviderCapabilityState::credential_test)
            .map(check_dto)
            .unwrap_or_else(|| check_dto(&ProviderCheckMetadata::never_run())),
    }
}

fn descriptor_dto(
    runtime: &ProviderRuntime,
    provider: &ProviderSpec,
    states: &BTreeMap<(&str, &str), &ProviderCapabilityState>,
) -> ProviderDescriptorDto {
    ProviderDescriptorDto {
        provider_id: provider.provider.to_owned(),
        display_name: provider.label.to_owned(),
        provider_kind: match provider.kind {
            ProviderKind::Metadata => ProviderKindDto::Metadata,
        },
        documentation_url: provider.docs_url.to_owned(),
        attribution: provider.attribution.to_owned(),
        supported_media_grains: provider
            .media_grains
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        capabilities: provider
            .capabilities
            .iter()
            .map(|capability| {
                capability_dto(
                    runtime,
                    provider.runtime_available,
                    capability,
                    states
                        .get(&(provider.provider, capability.capability_id))
                        .copied(),
                )
            })
            .collect(),
        network_hosts: provider
            .network_hosts
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
        locale_support: provider.locale_support != "unavailable",
        region_support: !matches!(provider.region_support, "unavailable" | "not_supported"),
        identity_namespaces: provider
            .identity_namespaces
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/providers",
    operation_id = "list_providers",
    tag = "providers",
    security(("credential_bearer" = []), ("browser_session_cookie" = [])),
    responses(
        (status = 200, description = "Provider inventory scoped to the authenticated workspace", body = ListProvidersResponse),
        (status = 401, description = "Credential or browser session is missing, inactive, or outside its listener boundary", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks provider-read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Provider state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Provider state storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn list_providers(
    State(state): State<ProviderApiState>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ListProviders;
    let authentication = application_request_authentication(
        &headers,
        state.browser_boundary.as_ref(),
        false,
        capability,
        correlation_id,
    )?;
    let kernel = Arc::clone(&state.kernel);
    let port = Arc::clone(&state.provider_state);
    let (mut persisted, browser) = tokio::task::spawn_blocking(move || {
        let access = authenticate_application_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
        )?;
        let states = port.authorize_and_list_provider_capability_states(correlation_id, &access)?;
        Ok::<_, Box<FastiProblem>>((
            states,
            matches!(access, ApplicationAccessContext::BrowserSession(_)),
        ))
    })
    .await
    .map_err(|_| storage_problem(capability, correlation_id))?
    .map_err(application_problem)?;
    for provider in state
        .runtime
        .descriptors()
        .iter()
        .filter(|provider| provider.runtime_available)
    {
        for provider_capability in provider.capabilities {
            if !persisted.iter().any(|item| {
                item.provider_id().as_str() == provider.provider
                    && item.capability_id().as_str() == provider_capability.capability_id
            }) {
                if let Ok(virtual_state) =
                    initial_state(&state.runtime, provider, provider_capability)
                {
                    persisted.push(virtual_state);
                }
            }
        }
    }
    let indexed = persisted
        .iter()
        .map(|item| {
            (
                (item.provider_id().as_str(), item.capability_id().as_str()),
                item,
            )
        })
        .collect();
    let mut response = ListProvidersResponse {
        providers: state
            .runtime
            .descriptors()
            .iter()
            .map(|provider| descriptor_dto(&state.runtime, provider, &indexed))
            .collect(),
    };
    if browser {
        // Inventory access does not activate bearer-only credential or health operations.
        for capability in response
            .providers
            .iter_mut()
            .flat_map(|provider| &mut provider.capabilities)
        {
            capability.writable = false;
            capability.testable = false;
        }
    }
    Ok((
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(response),
    )
        .into_response())
}

#[utoipa::path(
    put,
    path = "/api/v1/providers/{provider_id}/credentials/{capability_id}",
    operation_id = "configure_provider_credential",
    tag = "providers",
    security(("credential_bearer" = [])),
    params(
        ("provider_id" = String, Path, description = "Canonical provider ID"),
        ("capability_id" = String, Path, description = "Canonical provider capability ID")
    ),
    request_body = ConfigureProviderCredentialRequest,
    responses(
        (status = 200, description = "Write-only provider credential was stored and every affected provider capability state is returned", body = ProviderCapabilityResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential is missing, inactive, or rejected by the provider vault", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks provider-credential-management scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Provider path or body is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Provider state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Provider runtime or storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn configure_provider_credential(
    State(state): State<ProviderApiState>,
    Path((provider_id, capability_id)): Path<(String, String)>,
    headers: HeaderMap,
    request: Result<Json<ConfigureProviderCredentialRequest>, JsonRejection>,
) -> HttpResult<ProviderCapabilityResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ConfigureProviderCredential;
    authorize(&state, &headers, capability, correlation_id).await?;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let secret_bytes = request.secret.into_bytes();
    if secret_bytes.is_empty()
        || secret_bytes.len() > MAX_PROVIDER_CREDENTIAL_BYTES
        || !secret_bytes.iter().all(|byte| byte.is_ascii_graphic())
    {
        return Err(invalid_credential(capability, correlation_id));
    }
    let (provider, _) = resolve(
        &state.runtime,
        &provider_id,
        &capability_id,
        capability,
        correlation_id,
    )?;
    let operation_lock = state.operation_lock(provider);
    run_provider_state_operation(operation_lock, capability, correlation_id, async move {
        let access = authorize(&state, &headers, capability, correlation_id).await?;
        let reference = state
            .runtime
            .credential_reference(&provider_id)
            .map_err(|error| runtime_problem(&error, capability, correlation_id))?;
        let source = state
            .runtime
            .credential_source(&reference)
            .map_err(|error| runtime_problem(&error, capability, correlation_id))?;
        if matches!(
            source,
            CredentialVaultSource::Environment | CredentialVaultSource::OperatorSecretMount
        ) {
            return Err(application_problem(Box::new(FastiProblem::from_code(
                ProblemCode::ProviderCredentialInvalid,
                capability,
                correlation_id,
            ))));
        }
        let currents = provider_states(
            &state,
            access.workspace_id(),
            provider,
            capability,
            correlation_id,
        )
        .await?;
        let mut pending = Vec::with_capacity(currents.len());
        for (spec, current) in &currents {
            let value = next_state(
                current,
                ProviderCapabilityStatus::Disabled,
                Some(reference.clone()),
                ProviderCredentialStatus::StoredUnverified,
                state
                    .runtime
                    .configuration_digest(&provider_id, spec.capability_id)
                    .map_err(|error| runtime_problem(&error, capability, correlation_id))?,
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .map_err(|error| state_problem(error, capability, correlation_id))?;
            put_state(
                &state,
                access.workspace_id(),
                value.clone(),
                capability,
                correlation_id,
            )
            .await?;
            pending.push((*spec, value));
        }
        let secret = CredentialSecret::try_from_bytes(secret_bytes).map_err(|_| {
            application_problem(Box::new(FastiProblem::from_code(
                ProblemCode::ProviderCredentialInvalid,
                capability,
                correlation_id,
            )))
        })?;
        let runtime = Arc::clone(&state.runtime);
        let replacing = source == CredentialVaultSource::CredentialStore;
        let vault_reference = reference.clone();
        let stored = tokio::task::spawn_blocking(move || {
            if replacing {
                runtime.replace_credential(&vault_reference, secret)
            } else {
                runtime.store_credential(&vault_reference, secret)
            }
        })
        .await
        .map_err(|_| storage_problem(capability, correlation_id))?;
        if let Err(error) = stored {
            for ((_, current), (_, pending)) in currents.iter().zip(&pending) {
                let restored = next_state(
                    pending,
                    current.capability_status(),
                    current.credential_reference().cloned(),
                    current.credential_status(),
                    current.configuration_digest().clone(),
                    current.health().clone(),
                    current.credential_test().clone(),
                )
                .map_err(|state_error| state_problem(state_error, capability, correlation_id))?;
                put_state(
                    &state,
                    access.workspace_id(),
                    restored,
                    capability,
                    correlation_id,
                )
                .await?;
            }
            return Err(runtime_problem(&error, capability, correlation_id));
        }

        let mut final_states = Vec::with_capacity(pending.len());
        for (spec, pending) in pending {
            let available = next_state(
                &pending,
                ProviderCapabilityStatus::Available,
                Some(reference.clone()),
                ProviderCredentialStatus::StoredUnverified,
                pending.configuration_digest().clone(),
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .map_err(|error| state_problem(error, capability, correlation_id))?;
            put_state(
                &state,
                access.workspace_id(),
                available.clone(),
                capability,
                correlation_id,
            )
            .await?;
            final_states.push((spec, available));
        }
        Ok(Json(ProviderCapabilityResponse {
            provider_id,
            capabilities: final_states
                .iter()
                .map(|(spec, value)| {
                    capability_dto(
                        &state.runtime,
                        provider.runtime_available,
                        spec,
                        Some(value),
                    )
                })
                .collect(),
        }))
    })
    .await
}

#[utoipa::path(
    delete,
    path = "/api/v1/providers/{provider_id}/credentials/{capability_id}",
    operation_id = "remove_provider_credential",
    tag = "providers",
    security(("credential_bearer" = [])),
    params(
        ("provider_id" = String, Path, description = "Canonical provider ID"),
        ("capability_id" = String, Path, description = "Canonical provider capability ID")
    ),
    responses(
        (status = 200, description = "Provider credential grant was removed and every affected provider capability state is returned", body = ProviderCapabilityResponse),
        (status = 401, description = "Credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks provider-credential-management scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Provider path is invalid", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Provider state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Provider runtime or storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn remove_provider_credential(
    State(state): State<ProviderApiState>,
    Path((provider_id, capability_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> HttpResult<ProviderCapabilityResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ConfigureProviderCredential;
    authorize(&state, &headers, capability, correlation_id).await?;
    let (provider, _) = resolve(
        &state.runtime,
        &provider_id,
        &capability_id,
        capability,
        correlation_id,
    )?;
    let operation_lock = state.operation_lock(provider);
    run_provider_state_operation(operation_lock, capability, correlation_id, async move {
        let access = authorize(&state, &headers, capability, correlation_id).await?;
        let reference = state
            .runtime
            .credential_reference(&provider_id)
            .map_err(|error| runtime_problem(&error, capability, correlation_id))?;
        let source = state
            .runtime
            .credential_source(&reference)
            .map_err(|error| runtime_problem(&error, capability, correlation_id))?;
        if matches!(
            source,
            CredentialVaultSource::Environment | CredentialVaultSource::OperatorSecretMount
        ) {
            return Err(application_problem(Box::new(FastiProblem::from_code(
                ProblemCode::ProviderCredentialInvalid,
                capability,
                correlation_id,
            ))));
        }
        let currents = provider_states(
            &state,
            access.workspace_id(),
            provider,
            capability,
            correlation_id,
        )
        .await?;
        let mut pending = Vec::with_capacity(currents.len());
        for (spec, current) in &currents {
            let value = next_state(
                current,
                ProviderCapabilityStatus::Disabled,
                Some(reference.clone()),
                current.credential_status(),
                current.configuration_digest().clone(),
                current.health().clone(),
                current.credential_test().clone(),
            )
            .map_err(|error| state_problem(error, capability, correlation_id))?;
            put_state(
                &state,
                access.workspace_id(),
                value.clone(),
                capability,
                correlation_id,
            )
            .await?;
            pending.push((*spec, value));
        }
        let revoked = if source == CredentialVaultSource::None {
            Ok(())
        } else {
            let runtime = Arc::clone(&state.runtime);
            let vault_reference = reference.clone();
            tokio::task::spawn_blocking(move || runtime.revoke_credential(&vault_reference))
                .await
                .map_err(|_| storage_problem(capability, correlation_id))?
        };
        if let Err(error) = revoked {
            for ((_, current), (_, pending)) in currents.iter().zip(&pending) {
                let restored = next_state(
                    pending,
                    current.capability_status(),
                    current.credential_reference().cloned(),
                    current.credential_status(),
                    current.configuration_digest().clone(),
                    current.health().clone(),
                    current.credential_test().clone(),
                )
                .map_err(|state_error| state_problem(state_error, capability, correlation_id))?;
                put_state(
                    &state,
                    access.workspace_id(),
                    restored,
                    capability,
                    correlation_id,
                )
                .await?;
            }
            return Err(runtime_problem(&error, capability, correlation_id));
        }

        let mut removed = Vec::with_capacity(pending.len());
        for (spec, pending) in pending {
            let value = next_state(
                &pending,
                ProviderCapabilityStatus::Unavailable,
                None,
                if pending.credential_requirement() == CredentialRequirement::OptionalApiKey {
                    ProviderCredentialStatus::Optional
                } else {
                    ProviderCredentialStatus::Missing
                },
                pending.configuration_digest().clone(),
                pending.health().clone(),
                ProviderCheckMetadata::never_run(),
            )
            .map_err(|error| state_problem(error, capability, correlation_id))?;
            put_state(
                &state,
                access.workspace_id(),
                value.clone(),
                capability,
                correlation_id,
            )
            .await?;
            removed.push((spec, value));
        }
        Ok(Json(ProviderCapabilityResponse {
            provider_id,
            capabilities: removed
                .iter()
                .map(|(spec, value)| {
                    capability_dto(
                        &state.runtime,
                        provider.runtime_available,
                        spec,
                        Some(value),
                    )
                })
                .collect(),
        }))
    })
    .await
}

async fn execute_check(
    state: &ProviderApiState,
    workspace_id: WorkspaceId,
    provider_id: &str,
    spec: &ProviderCapabilitySpec,
    kind: ProviderCheckKind,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<ProviderCapabilityState, HttpProblem> {
    let provider = state
        .runtime
        .descriptor(provider_id)
        .map_err(|_| invalid_path(capability, correlation_id))?;
    let current = get_state(
        state,
        workspace_id,
        ProviderId::try_new(provider_id).map_err(|_| invalid_path(capability, correlation_id))?,
        ProviderCapabilityId::try_new(spec.capability_id)
            .map_err(|_| invalid_path(capability, correlation_id))?,
        capability,
        correlation_id,
    )
    .await?
    .map(Ok)
    .unwrap_or_else(|| initial_state(&state.runtime, provider, spec))
    .map_err(|error| runtime_problem(&error, capability, correlation_id))?;
    let outcome = state
        .runtime
        .check(
            provider_id,
            kind,
            &OutboundAccessPolicy::default(),
            &current,
        )
        .await;
    let (code, check_status, credential_status) = match &outcome {
        Ok(()) => (
            None,
            ProviderCheckStatus::Passed,
            credential_status_after_successful_check(kind, current.credential_status()),
        ),
        Err(error) => {
            let code = error.problem_code();
            let check_status = if code == ProblemCode::ProviderUnavailable {
                ProviderCheckStatus::Unavailable
            } else {
                ProviderCheckStatus::Failed
            };
            let credential_status = credential_status_after_failed_check(kind, &current, code);
            (Some(code), check_status, credential_status)
        }
    };
    let check = ProviderCheckMetadata::try_new(check_status, Some(Utc::now()), code)
        .map_err(|_| state_problem(ProviderStatePortError::Corrupt, capability, correlation_id))?;
    let updated = next_state(
        &current,
        if outcome.is_ok() {
            ProviderCapabilityStatus::Available
        } else {
            ProviderCapabilityStatus::Degraded
        },
        current.credential_reference().cloned(),
        credential_status,
        current.configuration_digest().clone(),
        if kind == ProviderCheckKind::Health {
            check.clone()
        } else {
            current.health().clone()
        },
        if kind == ProviderCheckKind::Credential {
            check
        } else {
            current.credential_test().clone()
        },
    )
    .map_err(|error| state_problem(error, capability, correlation_id))?;
    put_state(
        state,
        workspace_id,
        updated.clone(),
        capability,
        correlation_id,
    )
    .await?;
    if let Err(error) = outcome {
        return Err(runtime_problem(&error, capability, correlation_id));
    }
    Ok(updated)
}

#[utoipa::path(
    post,
    path = "/api/v1/providers/{provider_id}/credentials/{capability_id}/tests",
    operation_id = "test_provider_credential",
    tag = "providers",
    security(("credential_bearer" = [])),
    params(
        ("provider_id" = String, Path, description = "Canonical provider ID"),
        ("capability_id" = String, Path, description = "Canonical provider capability ID")
    ),
    responses(
        (status = 200, description = "Provider accepted the stored credential and every affected provider capability state is returned", body = ProviderCapabilityResponse),
        (status = 401, description = "Credential is missing, inactive, expired, or rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks provider-credential-management scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Provider credential is not stored", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Provider route is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 429, description = "Provider rate limit was reached", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Provider state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 502, description = "Provider returned an invalid response", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Provider runtime or storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn test_provider_credential(
    State(state): State<ProviderApiState>,
    Path((provider_id, capability_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> HttpResult<ProviderCapabilityResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::TestProviderCredential;
    authorize(&state, &headers, capability, correlation_id).await?;
    let (provider, spec) = resolve(
        &state.runtime,
        &provider_id,
        &capability_id,
        capability,
        correlation_id,
    )?;
    let operation_lock = state.operation_lock(provider);
    run_provider_state_operation(operation_lock, capability, correlation_id, async move {
        let access = authorize(&state, &headers, capability, correlation_id).await?;
        execute_check(
            &state,
            access.workspace_id(),
            &provider_id,
            spec,
            ProviderCheckKind::Credential,
            capability,
            correlation_id,
        )
        .await?;
        let states = provider_states(
            &state,
            access.workspace_id(),
            provider,
            capability,
            correlation_id,
        )
        .await?;
        Ok(Json(ProviderCapabilityResponse {
            provider_id,
            capabilities: states
                .iter()
                .map(|(item, value)| {
                    capability_dto(
                        &state.runtime,
                        provider.runtime_available,
                        item,
                        Some(value),
                    )
                })
                .collect(),
        }))
    })
    .await
}

#[utoipa::path(
    get,
    path = "/api/v1/providers/{provider_id}/health",
    operation_id = "read_provider_health",
    tag = "providers",
    security(("credential_bearer" = [])),
    params(("provider_id" = String, Path, description = "Canonical provider ID")),
    responses(
        (status = 200, description = "Live provider health scoped to configured capabilities", body = ProviderHealthResponse),
        (status = 401, description = "Credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks provider-read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Provider route is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 429, description = "Provider rate limit was reached", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Provider state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 502, description = "Provider returned an invalid response", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Provider runtime or storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn read_provider_health(
    State(state): State<ProviderApiState>,
    Path(provider_id): Path<String>,
    headers: HeaderMap,
) -> HttpResult<ProviderHealthResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ReadProviderHealth;
    authorize(&state, &headers, capability, correlation_id).await?;
    let provider = state
        .runtime
        .descriptor(&provider_id)
        .map_err(|_| invalid_path(capability, correlation_id))?;
    if !provider.runtime_available {
        return Err(application_problem(Box::new(FastiProblem::from_code(
            ProblemCode::ProviderUnavailable,
            capability,
            correlation_id,
        ))));
    }
    let operation_lock = state.operation_lock(provider);
    run_provider_state_operation(operation_lock, capability, correlation_id, async move {
        let access = authorize(&state, &headers, capability, correlation_id).await?;
        let mut capabilities = Vec::with_capacity(provider.capabilities.len());
        for spec in provider.capabilities {
            let updated = execute_check(
                &state,
                access.workspace_id(),
                &provider_id,
                spec,
                ProviderCheckKind::Health,
                capability,
                correlation_id,
            )
            .await?;
            capabilities.push(capability_dto(
                &state.runtime,
                provider.runtime_available,
                spec,
                Some(&updated),
            ));
        }
        Ok(Json(ProviderHealthResponse {
            provider_id,
            capabilities,
        }))
    })
    .await
}

pub(crate) fn router() -> Router<ProviderApiState> {
    Router::new()
        .route("/api/v1/providers", get(list_providers))
        .route(
            "/api/v1/providers/{provider_id}/credentials/{capability_id}",
            axum::routing::put(configure_provider_credential).delete(remove_provider_credential),
        )
        .route(
            "/api/v1/providers/{provider_id}/credentials/{capability_id}/tests",
            axum::routing::post(test_provider_credential),
        )
        .route(
            "/api/v1/providers/{provider_id}/health",
            get(read_provider_health),
        )
        .layer(DefaultBodyLimit::max(MAX_PROVIDER_JSON_BODY_BYTES))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
    };
    use fasti_application::ApplicationResult;
    use fasti_application::{
        AccessAdministrationPort, CredentialVaultError, CredentialVaultPort,
        EnrollFirstClientCommand, InitializeNodeCommand, ProviderStateWriteOutcome, SecretMaterial,
        StoredCredential,
    };
    use fasti_store::SqliteKernel;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    use std::time::Duration;
    use tower::ServiceExt;

    struct MemoryVault {
        source: CredentialVaultSource,
        value: Mutex<Option<Vec<u8>>>,
        store_pause: Mutex<Option<WritePause>>,
        reject_store: std::sync::atomic::AtomicBool,
    }

    impl MemoryVault {
        fn new(source: CredentialVaultSource) -> Self {
            Self {
                source,
                value: Mutex::new(None),
                store_pause: Mutex::new(None),
                reject_store: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    impl CredentialVaultPort for MemoryVault {
        fn source(
            &self,
            _reference: &CredentialReference,
        ) -> Result<CredentialVaultSource, CredentialVaultError> {
            if self.source != CredentialVaultSource::None {
                return Ok(self.source);
            }
            Ok(if self.value.lock().expect("memory vault").is_some() {
                CredentialVaultSource::CredentialStore
            } else {
                CredentialVaultSource::None
            })
        }

        fn store(
            &self,
            reference: &CredentialReference,
            secret: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            let pause = self.store_pause.lock().expect("store pause").take();
            if let Some(pause) = pause {
                pause.wait();
            }
            if self.reject_store.load(Ordering::SeqCst) {
                return Err(CredentialVaultError::Rejected);
            }
            if matches!(
                self.source,
                CredentialVaultSource::Environment | CredentialVaultSource::OperatorSecretMount
            ) {
                return Err(CredentialVaultError::Rejected);
            }
            *self.value.lock().expect("memory vault") = Some(secret.expose().to_vec());
            StoredCredential::try_new(reference.clone(), 1)
                .map_err(|_| CredentialVaultError::Rejected)
        }

        fn replace(
            &self,
            reference: &CredentialReference,
            secret: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            self.store(reference, secret)
        }

        fn load(
            &self,
            _reference: &CredentialReference,
        ) -> Result<CredentialSecret, CredentialVaultError> {
            let value = self
                .value
                .lock()
                .expect("memory vault")
                .clone()
                .unwrap_or_else(|| b"environment-secret".to_vec());
            CredentialSecret::try_from_bytes(value).map_err(|_| CredentialVaultError::Rejected)
        }

        fn revoke(&self, _reference: &CredentialReference) -> Result<(), CredentialVaultError> {
            *self.value.lock().expect("memory vault") = None;
            Ok(())
        }
    }

    struct CountingState(AtomicUsize);

    struct WritePause {
        entered: tokio::sync::oneshot::Sender<()>,
        release: std::sync::mpsc::Receiver<()>,
    }

    impl WritePause {
        fn new() -> (
            Self,
            tokio::sync::oneshot::Receiver<()>,
            std::sync::mpsc::Sender<()>,
        ) {
            let (entered, started) = tokio::sync::oneshot::channel();
            let (finish, release) = std::sync::mpsc::channel();
            (Self { entered, release }, started, finish)
        }

        fn wait(self) {
            let _ = self.entered.send(());
            // Dropping the test's sender also releases this worker after an assertion failure.
            let _ = self.release.recv();
        }
    }

    struct PausedState {
        kernel: Arc<SqliteKernel>,
        pause: Mutex<Option<(ProviderCapabilityStatus, WritePause)>>,
    }

    impl ProviderStatePort for PausedState {
        fn authorize_and_list_provider_capability_states(
            &self,
            correlation_id: RequestCorrelationId,
            access: &ApplicationAccessContext,
        ) -> ApplicationResult<Vec<ProviderCapabilityState>> {
            self.kernel
                .authorize_and_list_provider_capability_states(correlation_id, access)
        }

        fn get_provider_capability_state(
            &self,
            workspace_id: WorkspaceId,
            provider_id: &ProviderId,
            capability_id: &ProviderCapabilityId,
        ) -> Result<Option<ProviderCapabilityState>, ProviderStatePortError> {
            self.kernel
                .get_provider_capability_state(workspace_id, provider_id, capability_id)
        }

        fn list_provider_capability_states(
            &self,
            workspace_id: WorkspaceId,
        ) -> Result<Vec<ProviderCapabilityState>, ProviderStatePortError> {
            self.kernel.list_provider_capability_states(workspace_id)
        }

        fn put_provider_capability_state(
            &self,
            workspace_id: WorkspaceId,
            state: ProviderCapabilityState,
        ) -> Result<ProviderStateWriteOutcome, ProviderStatePortError> {
            let pause = {
                let mut selected = self.pause.lock().expect("state pause");
                if selected
                    .as_ref()
                    .is_some_and(|(status, _)| *status == state.capability_status())
                {
                    selected.take().map(|(_, pause)| pause)
                } else {
                    None
                }
            };
            if let Some(pause) = pause {
                pause.wait();
            }
            self.kernel
                .put_provider_capability_state(workspace_id, state)
        }
    }

    async fn cancel_paused_request(
        app: Router,
        request: Request<Body>,
        entered: tokio::sync::oneshot::Receiver<()>,
        finish: std::sync::mpsc::Sender<()>,
        gate: &Arc<tokio::sync::Mutex<()>>,
    ) {
        let caller = tokio::spawn(app.oneshot(request));
        let entered = tokio::time::timeout(Duration::from_secs(5), entered)
            .await
            .expect("request reaches selected write");
        if entered.is_err() {
            let response = caller
                .await
                .expect("request task")
                .expect("request response");
            let status = response.status();
            let body = to_bytes(response.into_body(), 8192)
                .await
                .expect("problem body");
            panic!(
                "request ended before selected write: {status} {}",
                String::from_utf8_lossy(&body)
            );
        }
        caller.abort();
        assert!(caller.await.expect_err("cancel request").is_cancelled());
        let held = gate.try_lock().is_err();
        finish.send(()).expect("release selected write");
        let _completed = tokio::time::timeout(Duration::from_secs(5), gate.lock())
            .await
            .expect("entire state reconciliation completes");
        assert!(
            held,
            "cancelled request released its gate before the write completed"
        );
    }

    impl ProviderStatePort for CountingState {
        fn authorize_and_list_provider_capability_states(
            &self,
            correlation_id: RequestCorrelationId,
            _access: &ApplicationAccessContext,
        ) -> ApplicationResult<Vec<ProviderCapabilityState>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(Box::new(FastiProblem::forbidden(
                CapabilityKey::ListProviders,
                correlation_id,
            )))
        }

        fn get_provider_capability_state(
            &self,
            _workspace_id: WorkspaceId,
            _provider_id: &ProviderId,
            _capability_id: &ProviderCapabilityId,
        ) -> Result<Option<ProviderCapabilityState>, ProviderStatePortError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }

        fn list_provider_capability_states(
            &self,
            _workspace_id: WorkspaceId,
        ) -> Result<Vec<ProviderCapabilityState>, ProviderStatePortError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }

        fn put_provider_capability_state(
            &self,
            _workspace_id: WorkspaceId,
            _state: ProviderCapabilityState,
        ) -> Result<ProviderStateWriteOutcome, ProviderStatePortError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(ProviderStateWriteOutcome::Created)
        }
    }

    fn enrolled_kernel() -> (tempfile::TempDir, Arc<SqliteKernel>, WorkspaceId, String) {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = Arc::new(SqliteKernel::open(root.path()).expect("SQLite kernel"));
        let initialized = kernel
            .initialize_node(InitializeNodeCommand::new(RequestCorrelationId::new_v7()))
            .expect("initialize node");
        let workspace_id = initialized.workspace_id();
        let proof = SecretMaterial::try_from_hex(&initialized.initialization_proof().expose_hex())
            .expect("copy one-time proof for enrollment");
        let enrolled = kernel
            .enroll_first_client(EnrollFirstClientCommand::new(
                RequestCorrelationId::new_v7(),
                proof,
            ))
            .expect("enroll client");
        (
            root,
            kernel,
            workspace_id,
            enrolled.credential().expose_hex(),
        )
    }

    #[tokio::test]
    async fn authentication_precedes_provider_enumeration() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = Arc::new(SqliteKernel::open(root.path()).expect("SQLite kernel"));
        let state = Arc::new(CountingState(AtomicUsize::new(0)));
        let runtime = Arc::new(ProviderRuntime::new(Arc::new(MemoryVault::new(
            CredentialVaultSource::None,
        ))));
        let app = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel,
            provider_state: state.clone(),
            provider_operation_locks: ProviderOperationLocks::new(&runtime),
            runtime,
        });
        let response = app
            .oneshot(
                Request::get("/api/v1/providers/unknown/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(state.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn provider_operations_are_serialized_per_provider() {
        let (_root, kernel, _workspace_id, credential) = enrolled_kernel();
        let runtime = Arc::new(ProviderRuntime::new(Arc::new(MemoryVault::new(
            CredentialVaultSource::None,
        ))));
        let operation_locks = ProviderOperationLocks::new(&runtime);
        let tmdb_lock = operation_locks.get("tmdb").expect("TMDB operation lock");
        let google_books_lock = operation_locks
            .get("google-books")
            .expect("Google Books operation lock");
        assert!(!Arc::ptr_eq(&tmdb_lock, &google_books_lock));
        let app = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel: kernel.clone(),
            provider_state: kernel,
            runtime,
            provider_operation_locks: operation_locks,
        });

        for request in [
            Request::post("/api/v1/providers/tmdb/credentials/metadata.read/tests")
                .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                .body(Body::empty())
                .expect("credential-test request"),
            Request::get("/api/v1/providers/tmdb/health")
                .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                .body(Body::empty())
                .expect("provider-health request"),
        ] {
            let guard = tmdb_lock.lock().await;
            let result =
                tokio::time::timeout(Duration::from_millis(200), app.clone().oneshot(request))
                    .await;
            assert!(result.is_err(), "TMDB operation bypassed its provider lock");
            drop(guard);
        }

        let guard = tmdb_lock.lock().await;
        let response = tokio::time::timeout(
            Duration::from_secs(1),
            app.oneshot(
                Request::put("/api/v1/providers/google-books/credentials/metadata.read")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"secret":"google-books-secret"}"#))
                    .expect("Google Books credential request"),
            ),
        )
        .await
        .expect("unrelated provider was delayed by the TMDB lock")
        .expect("Google Books credential response");
        assert_eq!(response.status(), StatusCode::OK);
        drop(guard);
    }

    #[test]
    fn environment_credentials_are_visible_and_read_only() {
        let runtime = ProviderRuntime::new(Arc::new(MemoryVault::new(
            CredentialVaultSource::Environment,
        )));
        let provider = runtime.descriptor("tmdb").expect("TMDB descriptor");
        let spec = &provider.capabilities[0];
        let state = initial_state(&runtime, provider, spec).expect("virtual environment state");
        let dto = capability_dto(&runtime, true, spec, Some(&state));
        assert_eq!(
            dto.credential_source,
            ProviderCredentialSourceDto::Environment
        );
        assert_eq!(
            dto.credential_state,
            ProviderCredentialStateDto::StoredUnverified
        );
        assert!(!dto.writable);
    }

    #[tokio::test]
    async fn cancelled_configure_finishes_vault_and_every_capability_state() {
        let (_root, kernel, workspace_id, credential) = enrolled_kernel();
        let vault = Arc::new(MemoryVault::new(CredentialVaultSource::None));
        let (pause, entered, finish) = WritePause::new();
        *vault.store_pause.lock().expect("pause vault") = Some(pause);
        let runtime = Arc::new(ProviderRuntime::new(vault.clone()));
        let locks = ProviderOperationLocks::new(&runtime);
        let gate = locks.get("tmdb").expect("TMDB gate");
        let app = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel: kernel.clone(),
            provider_state: kernel.clone(),
            runtime,
            provider_operation_locks: locks,
        });
        cancel_paused_request(
            app,
            Request::put("/api/v1/providers/tmdb/credentials/metadata.read")
                .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"secret":"cancelled-write-fixture"}"#))
                .expect("request"),
            entered,
            finish,
            &gate,
        )
        .await;
        assert_eq!(
            vault.value.lock().expect("vault").as_deref(),
            Some(b"cancelled-write-fixture".as_slice())
        );
        let states = kernel
            .list_provider_capability_states(workspace_id)
            .expect("states");
        assert!(states.len() > 1);
        assert!(states
            .iter()
            .all(|state| state.capability_status() == ProviderCapabilityStatus::Available));
    }

    #[tokio::test]
    async fn cancelled_remove_or_rollback_finishes_all_state_reconciliation() {
        for rollback in [false, true] {
            let (_root, kernel, workspace_id, credential) = enrolled_kernel();
            let vault = Arc::new(MemoryVault::new(CredentialVaultSource::None));
            let runtime = Arc::new(ProviderRuntime::new(vault.clone()));
            let locks = ProviderOperationLocks::new(&runtime);
            let gate = locks.get("tmdb").expect("TMDB gate");
            let state = Arc::new(PausedState {
                kernel: kernel.clone(),
                pause: Mutex::new(None),
            });
            let app = router().with_state(ProviderApiState {
                browser_boundary: None,
                kernel: kernel.clone(),
                provider_state: state.clone(),
                runtime,
                provider_operation_locks: locks,
            });
            let response = app
                .clone()
                .oneshot(
                    Request::put("/api/v1/providers/tmdb/credentials/metadata.read")
                        .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"secret":"original-fixture"}"#))
                        .expect("seed request"),
                )
                .await
                .expect("seed response");
            assert_eq!(response.status(), StatusCode::OK);
            let original = kernel
                .list_provider_capability_states(workspace_id)
                .expect("original states");
            let (pause, entered, finish) = WritePause::new();
            *state.pause.lock().expect("pause reconciliation") = Some((
                if rollback {
                    ProviderCapabilityStatus::Available
                } else {
                    ProviderCapabilityStatus::Unavailable
                },
                pause,
            ));
            vault.reject_store.store(rollback, Ordering::SeqCst);
            let request = if rollback {
                Request::put("/api/v1/providers/tmdb/credentials/metadata.read")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::from(r#"{"secret":"rejected-fixture"}"#))
            } else {
                Request::delete("/api/v1/providers/tmdb/credentials/metadata.read")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
            }
            .expect("mutation request");
            cancel_paused_request(app, request, entered, finish, &gate).await;
            let current = kernel
                .list_provider_capability_states(workspace_id)
                .expect("final states");
            assert_eq!(current.len(), original.len());
            for (current, original) in current.iter().zip(&original) {
                assert_eq!(
                    current.capability_status(),
                    if rollback {
                        original.capability_status()
                    } else {
                        ProviderCapabilityStatus::Unavailable
                    }
                );
                assert_eq!(
                    current.credential_reference(),
                    if rollback {
                        original.credential_reference()
                    } else {
                        None
                    }
                );
                if rollback {
                    assert_eq!(
                        current.configuration_digest(),
                        original.configuration_digest()
                    );
                    assert_eq!(current.credential_status(), original.credential_status());
                }
            }
            assert_eq!(
                vault.value.lock().expect("vault").as_deref(),
                if rollback {
                    Some(b"original-fixture".as_slice())
                } else {
                    None
                }
            );
        }
    }

    #[tokio::test]
    async fn cancelled_provider_checks_finish_their_persisted_result() {
        for path in [
            "/api/v1/providers/tmdb/health",
            "/api/v1/providers/tmdb/credentials/metadata.read/tests",
        ] {
            let (_root, kernel, workspace_id, credential) = enrolled_kernel();
            let runtime = Arc::new(ProviderRuntime::new(Arc::new(MemoryVault::new(
                CredentialVaultSource::None,
            ))));
            let locks = ProviderOperationLocks::new(&runtime);
            let gate = locks.get("tmdb").expect("TMDB gate");
            let (pause, entered, finish) = WritePause::new();
            let state = Arc::new(PausedState {
                kernel: kernel.clone(),
                pause: Mutex::new(Some((ProviderCapabilityStatus::Degraded, pause))),
            });
            let app = router().with_state(ProviderApiState {
                browser_boundary: None,
                kernel: kernel.clone(),
                provider_state: state,
                runtime,
                provider_operation_locks: locks,
            });
            let request = if path.ends_with("/health") {
                Request::get(path)
            } else {
                Request::post(path)
            }
            .header(header::AUTHORIZATION, format!("Bearer {credential}"))
            .body(Body::empty())
            .expect("check request");
            // Missing configured credentials fails before egress; persist that real outcome.
            cancel_paused_request(app, request, entered, finish, &gate).await;
            let states = kernel
                .list_provider_capability_states(workspace_id)
                .expect("states");
            assert_eq!(states.len(), 1);
            assert_eq!(
                states[0].capability_status(),
                ProviderCapabilityStatus::Degraded
            );
        }
    }

    #[tokio::test]
    async fn cancelled_provider_waiter_never_starts_its_operation() {
        use std::{future::Future, task::Poll};
        let gate = Arc::new(tokio::sync::Mutex::new(()));
        let held = gate.lock().await;
        let calls = Arc::new(AtomicUsize::new(0));
        {
            let started = Arc::clone(&calls);
            let operation = run_provider_state_operation(
                Arc::clone(&gate),
                CapabilityKey::ConfigureProviderCredential,
                RequestCorrelationId::new_v7(),
                async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            );
            tokio::pin!(operation);
            assert!(
                std::future::poll_fn(|cx| Poll::Ready(operation.as_mut().poll(cx)))
                    .await
                    .is_pending()
            );
        }
        drop(held);
        let _available = gate.lock().await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unavailable_provider_capabilities_keep_credential_requirements_truthful() {
        let runtime = ProviderRuntime::new(Arc::new(MemoryVault::new(CredentialVaultSource::None)));
        let provider = runtime
            .descriptor("open-library")
            .expect("Open Library descriptor");
        let dto = capability_dto(&runtime, false, &provider.capabilities[0], None);
        assert_eq!(
            dto.credential_state,
            ProviderCredentialStateDto::NotRequired
        );
        assert_eq!(dto.state, ProviderCapabilityStateDto::Unavailable);
        assert!(!dto.writable);
        assert!(!dto.testable);
    }

    #[tokio::test]
    async fn configure_is_write_only_and_rejects_read_only_sources_without_state_mutation() {
        let (_root, kernel, workspace_id, credential) = enrolled_kernel();
        let writable_vault = Arc::new(MemoryVault::new(CredentialVaultSource::None));
        let writable_runtime = Arc::new(ProviderRuntime::new(writable_vault.clone()));
        let writable = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel: kernel.clone(),
            provider_state: kernel.clone(),
            provider_operation_locks: ProviderOperationLocks::new(&writable_runtime),
            runtime: writable_runtime,
        });
        let response = writable
            .clone()
            .oneshot(
                Request::put("/api/v1/providers/tmdb/credentials/metadata.search")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"secret":"do-not-return"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded response");
        assert!(!body
            .windows(b"do-not-return".len())
            .any(|value| value == b"do-not-return"));
        let configured: serde_json::Value =
            serde_json::from_slice(&body).expect("provider response JSON");
        let configured_capabilities = configured["capabilities"]
            .as_array()
            .expect("provider capabilities");
        assert!(!configured_capabilities.is_empty());
        assert!(configured_capabilities.iter().all(|capability| {
            capability["credential_source"] == serde_json::json!("credential_store")
        }));
        let states = kernel
            .list_provider_capability_states(workspace_id)
            .expect("configured provider states");
        let tmdb = ProviderRuntime::new(writable_vault.clone())
            .descriptor("tmdb")
            .expect("TMDB provider");
        assert_eq!(states.len(), tmdb.capabilities.len());
        assert!(states.iter().all(|state| {
            state.capability_status() == ProviderCapabilityStatus::Available
                && state.capability_version() == 3
        }));

        let response = writable
            .oneshot(
                Request::delete("/api/v1/providers/tmdb/credentials/metadata.read")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let states = kernel
            .list_provider_capability_states(workspace_id)
            .expect("removed provider states");
        assert_eq!(states.len(), tmdb.capabilities.len());
        assert!(states.iter().all(|state| {
            state.capability_status() == ProviderCapabilityStatus::Unavailable
                && state.capability_version() == 5
                && state.credential_reference().is_none()
        }));
        assert!(writable_vault.value.lock().expect("memory vault").is_none());

        let (_root, kernel, workspace_id, credential) = enrolled_kernel();
        let readonly_runtime = Arc::new(ProviderRuntime::new(Arc::new(MemoryVault::new(
            CredentialVaultSource::Environment,
        ))));
        let readonly = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel: kernel.clone(),
            provider_state: kernel.clone(),
            provider_operation_locks: ProviderOperationLocks::new(&readonly_runtime),
            runtime: readonly_runtime,
        });
        let response = readonly
            .clone()
            .oneshot(
                Request::put("/api/v1/providers/tmdb/credentials/metadata.search")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"secret":"replacement"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(kernel
            .list_provider_capability_states(workspace_id)
            .expect("list provider state")
            .is_empty());
        let response = readonly
            .oneshot(
                Request::delete("/api/v1/providers/tmdb/credentials/metadata.search")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(kernel
            .list_provider_capability_states(workspace_id)
            .expect("list provider state")
            .is_empty());

        let (_root, kernel, workspace_id, credential) = enrolled_kernel();
        let invalid_runtime = Arc::new(ProviderRuntime::new(Arc::new(MemoryVault::new(
            CredentialVaultSource::None,
        ))));
        let invalid = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel: kernel.clone(),
            provider_state: kernel.clone(),
            provider_operation_locks: ProviderOperationLocks::new(&invalid_runtime),
            runtime: invalid_runtime,
        });
        let response = invalid
            .clone()
            .oneshot(
                Request::put("/api/v1/providers/tmdb/credentials/metadata.search")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"secret":"not visible ASCII"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(kernel
            .list_provider_capability_states(workspace_id)
            .expect("list provider state")
            .is_empty());
        let response = invalid
            .clone()
            .oneshot(
                Request::put("/api/v1/providers/tmdb/credentials/metadata.search")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        r#"{{"secret":"{}"}}"#,
                        "x".repeat(MAX_PROVIDER_CREDENTIAL_BYTES + 1)
                    )))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(kernel
            .list_provider_capability_states(workspace_id)
            .expect("list provider state")
            .is_empty());
        let response = invalid
            .oneshot(
                Request::put("/api/v1/providers/tmdb/credentials/metadata.search")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(format!(
                        r#"{{"secret":"{}"}}"#,
                        "x".repeat(MAX_PROVIDER_CREDENTIAL_BYTES)
                    )))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }
}
