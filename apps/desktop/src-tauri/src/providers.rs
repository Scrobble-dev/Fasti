use crate::setup::DesktopProblem;
use chrono::Utc;
use fasti_application::{
    credential_status_after_failed_check, credential_status_after_successful_check,
    CredentialRequirement, CredentialSecret, CredentialVaultSource, OutboundAccessPolicy,
    ProblemCode, ProviderCapabilityId, ProviderCapabilityState, ProviderCapabilityStatus,
    ProviderCheckKind, ProviderCheckMetadata, ProviderCheckStatus, ProviderCredentialStatus,
    ProviderId, ProviderStatePort, MAX_PROVIDER_CREDENTIAL_BYTES,
};
use fasti_domain::WorkspaceId;
use fasti_provider_runtime::{ProviderRuntime, ProviderRuntimeError, ProviderSpec};
use fasti_store::SqliteKernel;
use serde::{Deserialize, Serialize};

pub(crate) use fasti_provider_runtime::{
    ProviderCandidate, ProviderSearchInput, ProviderSelectionInput,
};

const SEARCH_CAPABILITY: &str = "metadata.search";
const READ_CAPABILITY: &str = "metadata.read";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderCredentialStatusView {
    provider: &'static str,
    capability_id: &'static str,
    label: &'static str,
    purpose: &'static str,
    credential_requirement: CredentialRequirement,
    credential_state: ProviderCredentialStatus,
    state: ProviderCapabilityStatus,
    source: CredentialVaultSourceView,
    writable: bool,
    testable: bool,
    docs_url: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CredentialVaultSourceView {
    None,
    Environment,
    CredentialStore,
    OperatorSecretMount,
}

impl From<CredentialVaultSource> for CredentialVaultSourceView {
    fn from(value: CredentialVaultSource) -> Self {
        match value {
            CredentialVaultSource::None => Self::None,
            CredentialVaultSource::Environment => Self::Environment,
            CredentialVaultSource::CredentialStore => Self::CredentialStore,
            CredentialVaultSource::OperatorSecretMount => Self::OperatorSecretMount,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveProviderCredentialInput {
    provider: String,
    capability_id: String,
    credential: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCapabilityInput {
    provider: String,
    capability_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderInput {
    provider: String,
}

pub(crate) type DeleteProviderCredentialInput = ProviderCapabilityInput;

pub(crate) fn credential_statuses(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let mut rows = Vec::new();
    for spec in runtime.descriptors() {
        for capability in spec.capabilities {
            if !spec.runtime_available {
                rows.push(unavailable_status_view(spec, capability));
                continue;
            }
            let (state, source) = reconcile_state(
                runtime,
                kernel,
                workspace_id,
                spec,
                capability.capability_id,
            )?;
            rows.push(status_view(spec, capability.capability_id, &state, source));
        }
    }
    Ok(rows)
}

fn unavailable_status_view(
    spec: &'static ProviderSpec,
    capability: &'static fasti_provider_runtime::ProviderCapabilitySpec,
) -> ProviderCredentialStatusView {
    ProviderCredentialStatusView {
        provider: spec.provider,
        capability_id: capability.capability_id,
        label: spec.label,
        purpose: capability_purpose(capability.capability_id),
        credential_requirement: capability.credential_requirement,
        credential_state: match capability.credential_requirement {
            CredentialRequirement::None | CredentialRequirement::UserAgentOnly => {
                ProviderCredentialStatus::NotRequired
            }
            CredentialRequirement::OptionalApiKey => ProviderCredentialStatus::Optional,
            _ => ProviderCredentialStatus::Missing,
        },
        state: ProviderCapabilityStatus::Unavailable,
        source: CredentialVaultSourceView::None,
        writable: false,
        testable: false,
        docs_url: spec.docs_url,
    }
}

pub(crate) fn save_credential(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    input: SaveProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let spec = capability_spec(runtime, &input.provider, &input.capability_id)?;
    let reference = runtime
        .credential_reference(spec.provider)
        .map_err(runtime_problem)?;
    let source = runtime
        .credential_source(&reference)
        .map_err(runtime_problem)?;
    if !matches!(
        source,
        CredentialVaultSource::None | CredentialVaultSource::CredentialStore
    ) {
        return Err(DesktopProblem::provider_credential(
            "This provider credential source is read-only on this host.",
        ));
    }
    let credential = input.credential.into_bytes();
    if credential.is_empty()
        || credential.len() > MAX_PROVIDER_CREDENTIAL_BYTES
        || !credential.iter().all(|byte| byte.is_ascii_graphic())
    {
        return Err(DesktopProblem::provider_credential(
            "A provider credential must contain 1 to 4096 visible ASCII characters.",
        ));
    }
    let secret = CredentialSecret::try_from_bytes(credential)
        .map_err(|_| DesktopProblem::provider_credential("The provider credential is invalid."))?;
    mark_provider_credential_pending(runtime, kernel, workspace_id, spec)?;
    let stored = if source == CredentialVaultSource::None {
        runtime
            .store_credential(&reference, secret)
            .map_err(runtime_problem)
    } else {
        runtime
            .replace_credential(&reference, secret)
            .map_err(runtime_problem)
    };
    if let Err(error) = stored {
        reconcile_provider_with_source(runtime, kernel, workspace_id, spec, source, false)?;
        return Err(error);
    }
    reconcile_provider(runtime, kernel, workspace_id, spec.provider, true)?;
    credential_statuses(runtime, kernel, workspace_id)
}

pub(crate) fn delete_credential(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    input: DeleteProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let spec = capability_spec(runtime, &input.provider, &input.capability_id)?;
    let reference = runtime
        .credential_reference(spec.provider)
        .map_err(runtime_problem)?;
    let source = runtime
        .credential_source(&reference)
        .map_err(runtime_problem)?;
    if !matches!(
        source,
        CredentialVaultSource::None | CredentialVaultSource::CredentialStore
    ) {
        return Err(DesktopProblem::provider_credential(
            "This provider credential source is read-only on this host.",
        ));
    }
    mark_provider_credential_pending(runtime, kernel, workspace_id, spec)?;
    if let Err(error) = runtime
        .revoke_credential(&reference)
        .map_err(runtime_problem)
    {
        reconcile_provider_with_source(runtime, kernel, workspace_id, spec, source, false)?;
        return Err(error);
    }
    reconcile_provider(runtime, kernel, workspace_id, spec.provider, false)?;
    credential_statuses(runtime, kernel, workspace_id)
}

fn mark_provider_credential_pending(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    spec: &'static ProviderSpec,
) -> Result<(), DesktopProblem> {
    for capability in spec.capabilities {
        let (current, _) = reconcile_state(
            runtime,
            kernel,
            workspace_id,
            spec,
            capability.capability_id,
        )?;
        let pending = ProviderCapabilityState::try_new(
            current.provider_id().clone(),
            current.capability_id().clone(),
            ProviderCapabilityStatus::Disabled,
            current.capability_version().saturating_add(1),
            current.credential_requirement(),
            current.credential_reference().cloned(),
            current.credential_status(),
            current.configuration_digest().clone(),
            current.health().clone(),
            current.credential_test().clone(),
        )
        .map_err(|_| DesktopProblem::storage("Provider pending state is invalid."))?;
        kernel
            .put_provider_capability_state(workspace_id, pending)
            .map_err(|_| DesktopProblem::storage("Fasti could not save provider pending state."))?;
    }
    Ok(())
}

pub(crate) async fn test_credential(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    input: ProviderCapabilityInput,
    policy: &OutboundAccessPolicy,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let spec = capability_spec(runtime, &input.provider, &input.capability_id)?;
    let (state, _) = reconcile_state(runtime, kernel, workspace_id, spec, &input.capability_id)?;
    let result = runtime
        .check(spec.provider, ProviderCheckKind::Credential, policy, &state)
        .await;
    record_check_result(
        kernel,
        workspace_id,
        &state,
        ProviderCheckKind::Credential,
        &result,
    )?;
    result.map_err(runtime_problem)?;
    credential_statuses(runtime, kernel, workspace_id)
}

pub(crate) async fn health(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    input: ProviderInput,
    policy: &OutboundAccessPolicy,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let spec = runtime
        .descriptor(&input.provider)
        .map_err(runtime_problem)?;
    if !spec.runtime_available {
        return Err(DesktopProblem::provider(
            "This provider is not available in this runtime.",
        ));
    }
    for capability in spec.capabilities {
        let (state, _) = reconcile_state(
            runtime,
            kernel,
            workspace_id,
            spec,
            capability.capability_id,
        )?;
        let result = runtime
            .check(spec.provider, ProviderCheckKind::Health, policy, &state)
            .await;
        record_check_result(
            kernel,
            workspace_id,
            &state,
            ProviderCheckKind::Health,
            &result,
        )?;
        result.map_err(runtime_problem)?;
    }
    credential_statuses(runtime, kernel, workspace_id)
}

pub(crate) async fn search(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    input: ProviderSearchInput,
    policy: &OutboundAccessPolicy,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let spec = capability_spec(runtime, &input.provider, SEARCH_CAPABILITY)?;
    let (state, _) = reconcile_state(runtime, kernel, workspace_id, spec, SEARCH_CAPABILITY)?;
    runtime
        .search(input, policy, &state)
        .await
        .map_err(runtime_problem)
}

pub(crate) async fn fetch_selection(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    input: ProviderSelectionInput,
    policy: &OutboundAccessPolicy,
) -> Result<ProviderCandidate, DesktopProblem> {
    let spec = capability_spec(runtime, &input.provider, READ_CAPABILITY)?;
    let (state, _) = reconcile_state(runtime, kernel, workspace_id, spec, READ_CAPABILITY)?;
    runtime
        .fetch_selection(input, policy, &state)
        .await
        .map_err(runtime_problem)
}

fn reconcile_provider(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    provider: &str,
    credential_present: bool,
) -> Result<(), DesktopProblem> {
    let spec = runtime.descriptor(provider).map_err(runtime_problem)?;
    let reference = runtime
        .credential_reference(provider)
        .map_err(runtime_problem)?;
    let source = if credential_present {
        runtime
            .credential_source(&reference)
            .map_err(runtime_problem)?
    } else {
        CredentialVaultSource::None
    };
    reconcile_provider_with_source(runtime, kernel, workspace_id, spec, source, true)
}

fn reconcile_provider_with_source(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    spec: &'static ProviderSpec,
    source: CredentialVaultSource,
    credential_changed: bool,
) -> Result<(), DesktopProblem> {
    for capability in spec.capabilities {
        reconcile_state_with_source(
            runtime,
            kernel,
            workspace_id,
            spec,
            capability.capability_id,
            source,
            credential_changed,
        )?;
    }
    Ok(())
}

fn reconcile_state(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    spec: &'static ProviderSpec,
    capability_id: &str,
) -> Result<(ProviderCapabilityState, CredentialVaultSource), DesktopProblem> {
    let reference = runtime
        .credential_reference(spec.provider)
        .map_err(runtime_problem)?;
    let source = runtime
        .credential_source(&reference)
        .map_err(runtime_problem)?;
    let state = reconcile_state_with_source(
        runtime,
        kernel,
        workspace_id,
        spec,
        capability_id,
        source,
        false,
    )?;
    Ok((state, source))
}

fn reconcile_state_with_source(
    runtime: &ProviderRuntime,
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    spec: &'static ProviderSpec,
    capability_id: &str,
    source: CredentialVaultSource,
    credential_changed: bool,
) -> Result<ProviderCapabilityState, DesktopProblem> {
    let capability = spec
        .capabilities
        .iter()
        .find(|candidate| candidate.capability_id == capability_id)
        .ok_or_else(|| DesktopProblem::configuration("The provider capability is not declared."))?;
    let provider_id = ProviderId::try_new(spec.provider)
        .map_err(|_| DesktopProblem::configuration("The provider ID is invalid."))?;
    let capability_id = ProviderCapabilityId::try_new(capability_id)
        .map_err(|_| DesktopProblem::configuration("The provider capability ID is invalid."))?;
    let existing = kernel
        .get_provider_capability_state(workspace_id, &provider_id, &capability_id)
        .map_err(|_| DesktopProblem::storage("Fasti could not read provider capability state."))?;
    let digest = runtime
        .configuration_digest(spec.provider, capability.capability_id)
        .map_err(runtime_problem)?;
    let reference = runtime
        .credential_reference(spec.provider)
        .map_err(runtime_problem)?;
    let present = source != CredentialVaultSource::None;
    let unchanged_configuration = !credential_changed
        && existing
            .as_ref()
            .is_some_and(|state| state.configuration_digest() == &digest);
    let credential_status = if present && unchanged_configuration {
        existing
            .as_ref()
            .map(ProviderCapabilityState::credential_status)
            .filter(|status| {
                !matches!(
                    status,
                    ProviderCredentialStatus::Missing
                        | ProviderCredentialStatus::Revoked
                        | ProviderCredentialStatus::Unavailable
                )
            })
            .unwrap_or(ProviderCredentialStatus::StoredUnverified)
    } else if present {
        ProviderCredentialStatus::StoredUnverified
    } else {
        ProviderCredentialStatus::Missing
    };
    let version = existing
        .as_ref()
        .map_or(1, |state| state.capability_version());
    let health = if unchanged_configuration {
        existing
            .as_ref()
            .map(|state| state.health().clone())
            .unwrap_or_else(ProviderCheckMetadata::never_run)
    } else {
        ProviderCheckMetadata::never_run()
    };
    let credential_test = if present && unchanged_configuration {
        existing
            .as_ref()
            .map(|state| state.credential_test().clone())
            .unwrap_or_else(ProviderCheckMetadata::never_run)
    } else {
        ProviderCheckMetadata::never_run()
    };
    let candidate = provider_state(
        provider_id.clone(),
        capability_id.clone(),
        version,
        capability.credential_requirement,
        present.then_some(reference.clone()),
        credential_status,
        digest.clone(),
        health.clone(),
        credential_test.clone(),
    )?;
    if !credential_changed && existing.as_ref() == Some(&candidate) {
        return Ok(candidate);
    }
    let next_version = existing
        .as_ref()
        .map_or(1, |state| state.capability_version().saturating_add(1));
    let next = provider_state(
        provider_id,
        capability_id,
        next_version,
        capability.credential_requirement,
        present.then_some(reference),
        credential_status,
        digest,
        health,
        credential_test,
    )?;
    kernel
        .put_provider_capability_state(workspace_id, next.clone())
        .map_err(|_| DesktopProblem::storage("Fasti could not save provider capability state."))?;
    Ok(next)
}

#[allow(clippy::too_many_arguments)]
fn provider_state(
    provider_id: ProviderId,
    capability_id: ProviderCapabilityId,
    version: u64,
    requirement: CredentialRequirement,
    reference: Option<fasti_application::CredentialReference>,
    credential_status: ProviderCredentialStatus,
    digest: fasti_application::ConfigurationDigest,
    health: ProviderCheckMetadata,
    credential_test: ProviderCheckMetadata,
) -> Result<ProviderCapabilityState, DesktopProblem> {
    ProviderCapabilityState::try_new(
        provider_id,
        capability_id,
        if reference.is_some() {
            ProviderCapabilityStatus::Available
        } else {
            ProviderCapabilityStatus::Unavailable
        },
        version,
        requirement,
        reference,
        credential_status,
        digest,
        health,
        credential_test,
    )
    .map_err(|_| DesktopProblem::storage("Provider capability state is invalid."))
}

fn record_check_result(
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    state: &ProviderCapabilityState,
    kind: ProviderCheckKind,
    result: &Result<(), ProviderRuntimeError>,
) -> Result<(), DesktopProblem> {
    let (check_status, problem_code, capability_status, credential_status) = match result {
        Ok(()) => (
            ProviderCheckStatus::Passed,
            None,
            ProviderCapabilityStatus::Available,
            credential_status_after_successful_check(kind, state.credential_status()),
        ),
        Err(error) => {
            let credential_status =
                credential_status_after_failed_check(kind, state, error.problem_code());
            (
                ProviderCheckStatus::Failed,
                Some(error.problem_code()),
                if credential_status == ProviderCredentialStatus::Missing {
                    ProviderCapabilityStatus::Unavailable
                } else {
                    ProviderCapabilityStatus::Degraded
                },
                credential_status,
            )
        }
    };
    let check = ProviderCheckMetadata::try_new(check_status, Some(Utc::now()), problem_code)
        .map_err(|_| DesktopProblem::storage("Provider check state is invalid."))?;
    let (health, credential_test) = match kind {
        ProviderCheckKind::Health => (check, state.credential_test().clone()),
        ProviderCheckKind::Credential => (state.health().clone(), check),
    };
    let next = ProviderCapabilityState::try_new(
        state.provider_id().clone(),
        state.capability_id().clone(),
        capability_status,
        state.capability_version().saturating_add(1),
        state.credential_requirement(),
        state.credential_reference().cloned(),
        credential_status,
        state.configuration_digest().clone(),
        health,
        credential_test,
    )
    .map_err(|_| DesktopProblem::storage("Provider check state is invalid."))?;
    kernel
        .put_provider_capability_state(workspace_id, next)
        .map_err(|_| DesktopProblem::storage("Fasti could not save provider check state."))?;
    Ok(())
}

fn capability_spec(
    runtime: &ProviderRuntime,
    provider: &str,
    capability_id: &str,
) -> Result<&'static ProviderSpec, DesktopProblem> {
    let spec = runtime.descriptor(provider).map_err(runtime_problem)?;
    if !spec.runtime_available
        || !spec
            .capabilities
            .iter()
            .any(|candidate| candidate.capability_id == capability_id)
    {
        return Err(DesktopProblem::configuration(
            "The provider capability is not available in this runtime.",
        ));
    }
    Ok(spec)
}

fn status_view(
    spec: &'static ProviderSpec,
    capability_id: &'static str,
    state: &ProviderCapabilityState,
    source: CredentialVaultSource,
) -> ProviderCredentialStatusView {
    let capability = spec
        .capabilities
        .iter()
        .find(|candidate| candidate.capability_id == capability_id)
        .expect("registry capability must remain declared");
    ProviderCredentialStatusView {
        provider: spec.provider,
        capability_id,
        label: spec.label,
        purpose: capability_purpose(capability_id),
        credential_requirement: capability.credential_requirement,
        credential_state: state.credential_status(),
        state: state.capability_status(),
        source: source.into(),
        writable: matches!(
            source,
            CredentialVaultSource::None | CredentialVaultSource::CredentialStore
        ) && !matches!(
            capability.credential_requirement,
            CredentialRequirement::None | CredentialRequirement::UserAgentOnly
        ),
        testable: capability.credential_test,
        docs_url: spec.docs_url,
    }
}

fn capability_purpose(capability_id: &str) -> &'static str {
    match capability_id {
        SEARCH_CAPABILITY => "Search provider metadata",
        READ_CAPABILITY => "Read provider metadata",
        _ => "Provider capability",
    }
}

fn runtime_problem(error: ProviderRuntimeError) -> DesktopProblem {
    match error.problem_code() {
        ProblemCode::ProviderCredentialMissing
        | ProblemCode::ProviderCredentialInvalid
        | ProblemCode::ProviderCredentialExpired => {
            DesktopProblem::provider_credential(error.detail())
        }
        ProblemCode::ProviderRouteUnavailable => DesktopProblem::configuration(error.detail()),
        _ => DesktopProblem::provider(error.detail()),
    }
}

impl From<ProviderRuntimeError> for DesktopProblem {
    fn from(error: ProviderRuntimeError) -> Self {
        runtime_problem(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::require_access;
    use crate::setup::{complete_setup, test_support::new_kernel, test_support::MemoryStore};
    use fasti_application::ProviderStatePort;
    use fasti_provider_runtime::{PlatformCredentialVault, PLATFORM_CREDENTIAL_SERVICE};
    use std::sync::Arc;

    #[test]
    fn shared_credential_change_advances_every_capability_generation() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");
        let workspace_id = require_access(&kernel, &store)
            .expect("authenticated access")
            .workspace_id();
        let runtime = ProviderRuntime::new(Arc::new(PlatformCredentialVault::new(
            PLATFORM_CREDENTIAL_SERVICE,
            "metadata-generation-test",
        )));
        let spec = runtime.descriptor("tmdb").expect("TMDB descriptor");

        reconcile_provider_with_source(
            &runtime,
            &kernel,
            workspace_id,
            spec,
            CredentialVaultSource::CredentialStore,
            false,
        )
        .expect("initial reconciliation");
        mark_provider_credential_pending(&runtime, &kernel, workspace_id, spec)
            .expect("pending credential transition");
        let mut pending_version = None;
        for capability in spec.capabilities {
            let state = kernel
                .get_provider_capability_state(
                    workspace_id,
                    &ProviderId::try_new(spec.provider).expect("provider ID"),
                    &ProviderCapabilityId::try_new(capability.capability_id)
                        .expect("capability ID"),
                )
                .expect("read pending state")
                .expect("stored pending state");
            assert_eq!(
                pending_version.get_or_insert(state.capability_version()),
                &state.capability_version()
            );
            assert_eq!(
                state.capability_status(),
                ProviderCapabilityStatus::Disabled
            );
        }
        reconcile_provider_with_source(
            &runtime,
            &kernel,
            workspace_id,
            spec,
            CredentialVaultSource::CredentialStore,
            true,
        )
        .expect("credential rotation reconciliation");

        for capability in spec.capabilities {
            let state = kernel
                .get_provider_capability_state(
                    workspace_id,
                    &ProviderId::try_new(spec.provider).expect("provider ID"),
                    &ProviderCapabilityId::try_new(capability.capability_id)
                        .expect("capability ID"),
                )
                .expect("read state")
                .expect("stored state");
            assert_eq!(
                state.capability_version(),
                pending_version.expect("pending version") + 1
            );
            assert_eq!(
                state.credential_status(),
                ProviderCredentialStatus::StoredUnverified
            );
        }
    }

    #[test]
    fn missing_credentials_keep_valid_references_and_health_does_not_verify_them() {
        for present in [false, true] {
            for kind in [ProviderCheckKind::Health, ProviderCheckKind::Credential] {
                let (_root, kernel) = new_kernel();
                let store = MemoryStore::default();
                complete_setup(&kernel, &store).expect("complete setup");
                let workspace_id = require_access(&kernel, &store)
                    .expect("access")
                    .workspace_id();
                let state = ProviderCapabilityState::try_new(
                    ProviderId::try_new("tmdb").expect("provider"),
                    ProviderCapabilityId::try_new(READ_CAPABILITY).expect("capability"),
                    ProviderCapabilityStatus::Available,
                    1,
                    CredentialRequirement::BearerToken,
                    present.then(|| {
                        fasti_application::CredentialReference::try_new(
                            "secret:providers/tmdb/api-key",
                        )
                        .expect("reference")
                    }),
                    if present {
                        ProviderCredentialStatus::StoredUnverified
                    } else {
                        ProviderCredentialStatus::Missing
                    },
                    fasti_application::ConfigurationDigest::parse("ab".repeat(32)).expect("digest"),
                    ProviderCheckMetadata::never_run(),
                    ProviderCheckMetadata::never_run(),
                )
                .expect("valid initial state");
                kernel
                    .put_provider_capability_state(workspace_id, state.clone())
                    .expect("seed state");
                record_check_result(
                    &kernel,
                    workspace_id,
                    &state,
                    kind,
                    &Err(ProviderRuntimeError::credential_missing(
                        "credential disappeared",
                    )),
                )
                .expect("persist credential failure, not a storage error");
                let updated = kernel
                    .get_provider_capability_state(
                        workspace_id,
                        state.provider_id(),
                        state.capability_id(),
                    )
                    .expect("read state")
                    .expect("persisted check");
                assert_eq!(updated.credential_reference(), state.credential_reference());
                assert_eq!(
                    updated.credential_status(),
                    if present && kind == ProviderCheckKind::Credential {
                        ProviderCredentialStatus::Unavailable
                    } else {
                        state.credential_status()
                    }
                );
                let check = if kind == ProviderCheckKind::Health {
                    updated.health()
                } else {
                    updated.credential_test()
                };
                assert_eq!(
                    check.safe_problem_code(),
                    Some(ProblemCode::ProviderCredentialMissing)
                );
                assert_eq!(check.status(), ProviderCheckStatus::Failed);
            }
        }
    }
}
