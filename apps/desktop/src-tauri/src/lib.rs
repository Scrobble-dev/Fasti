//! Trusted-host adapters for the Fasti desktop and Android shells.

#![cfg_attr(not(feature = "desktop-runtime"), allow(dead_code))]

mod api_clients;
mod artwork;
mod endpoint;
mod network_config;
mod nuvio_collections;
mod providers;
mod records;
mod reviews;
mod secure_storage;
mod setup;

#[cfg(feature = "desktop-runtime")]
use endpoint::{EndpointConnectionInput, EndpointConnectionStatus};
#[cfg(feature = "desktop-runtime")]
use fasti_domain::RecordId;
#[cfg(feature = "desktop-runtime")]
use fasti_store::SqliteKernel;
#[cfg(feature = "desktop-runtime")]
use network_config::{NetworkConfigStore, NetworkConfiguration, SaveNetworkConfigurationInput};
#[cfg(feature = "desktop-runtime")]
use providers::{
    DeleteProviderCredentialInput, ProviderCandidate, ProviderCapabilityInput,
    ProviderCredentialStatusView, ProviderInput, ProviderSearchInput, ProviderSelectionInput,
    SaveProviderCredentialInput,
};
#[cfg(feature = "desktop-runtime")]
use setup::{DesktopProblem, KeyringSetupSecretStore, SetupStatus};
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
#[cfg(feature = "desktop-runtime")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "desktop-runtime")]
use tauri::Manager;

#[cfg(feature = "desktop-runtime")]
struct DesktopState {
    data_root: PathBuf,
    kernel: Mutex<Option<Arc<SqliteKernel>>>,
    setup_gate: Mutex<()>,
    network: NetworkConfigStore,
    artwork: artwork::ArtworkCache,
    provider_runtime: Mutex<Option<Arc<fasti_provider_runtime::ProviderRuntime>>>,
}

#[cfg(feature = "desktop-runtime")]
impl DesktopState {
    fn kernel(&self) -> Result<Arc<SqliteKernel>, DesktopProblem> {
        let mut current = self
            .kernel
            .lock()
            .map_err(|_| DesktopProblem::storage("The local kernel lock is unavailable."))?;
        if let Some(kernel) = current.as_ref() {
            return Ok(Arc::clone(kernel));
        }
        let kernel =
            Arc::new(SqliteKernel::open(&self.data_root).map_err(|_| {
                DesktopProblem::storage("Fasti could not open its local data root.")
            })?);
        *current = Some(Arc::clone(&kernel));
        Ok(kernel)
    }

    fn provider_runtime(
        &self,
        kernel: &SqliteKernel,
    ) -> Result<Arc<fasti_provider_runtime::ProviderRuntime>, DesktopProblem> {
        let mut current = self.provider_runtime.lock().map_err(|_| {
            DesktopProblem::provider("The shared provider runtime lock is unavailable.")
        })?;
        if let Some(runtime) = current.as_ref() {
            return Ok(Arc::clone(runtime));
        }
        let vault = Arc::new(fasti_provider_runtime::PlatformCredentialVault::new(
            fasti_provider_runtime::PLATFORM_CREDENTIAL_SERVICE,
            secure_storage::account_scope(kernel.data_root_identity()),
        ));
        let runtime = Arc::new(fasti_provider_runtime::ProviderRuntime::new(vault));
        *current = Some(Arc::clone(&runtime));
        Ok(runtime)
    }
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn setup_status(state: tauri::State<'_, DesktopState>) -> Result<SetupStatus, DesktopProblem> {
    let kernel = state.kernel()?;
    setup::inspect_setup(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn complete_setup(state: tauri::State<'_, DesktopState>) -> Result<SetupStatus, DesktopProblem> {
    let _guard = state
        .setup_gate
        .lock()
        .map_err(|_| DesktopProblem::storage("The setup lock is unavailable."))?;
    let kernel = state.kernel()?;
    setup::complete_setup(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn list_api_clients(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<api_clients::ApiClientSummary>, DesktopProblem> {
    let kernel = state.kernel()?;
    api_clients::list(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn create_api_client(
    state: tauri::State<'_, DesktopState>,
    input: api_clients::CreateApiClientInput,
) -> Result<api_clients::CreatedApiClient, DesktopProblem> {
    let kernel = state.kernel()?;
    api_clients::create(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        input,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn revoke_api_client(
    state: tauri::State<'_, DesktopState>,
    input: api_clients::RevokeApiClientInput,
) -> Result<Vec<api_clients::ApiClientSummary>, DesktopProblem> {
    let kernel = state.kernel()?;
    api_clients::revoke(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        input,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn load_network_configuration(
    state: tauri::State<'_, DesktopState>,
) -> Result<NetworkConfiguration, DesktopProblem> {
    state.network.load()
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn save_network_configuration(
    state: tauri::State<'_, DesktopState>,
    input: SaveNetworkConfigurationInput,
) -> Result<NetworkConfiguration, DesktopProblem> {
    state.network.save(input)
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn test_endpoint_connection(
    input: EndpointConnectionInput,
) -> Result<EndpointConnectionStatus, DesktopProblem> {
    endpoint::test_connection(input).await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn provider_credential_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    let runtime = state.provider_runtime(&kernel)?;
    providers::credential_statuses(&runtime, &kernel, access.workspace_id())
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn save_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: SaveProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    let runtime = state.provider_runtime(&kernel)?;
    providers::save_credential(&runtime, &kernel, access.workspace_id(), input)
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn delete_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: DeleteProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    let runtime = state.provider_runtime(&kernel)?;
    providers::delete_credential(&runtime, &kernel, access.workspace_id(), input)
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn test_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: ProviderCapabilityInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    let runtime = state.provider_runtime(&kernel)?;
    let configuration = state.network.load()?;
    providers::test_credential(
        &runtime,
        &kernel,
        access.workspace_id(),
        input,
        configuration.outbound_policy(),
    )
    .await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn read_provider_health(
    state: tauri::State<'_, DesktopState>,
    input: ProviderInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    let runtime = state.provider_runtime(&kernel)?;
    let configuration = state.network.load()?;
    providers::health(
        &runtime,
        &kernel,
        access.workspace_id(),
        input,
        configuration.outbound_policy(),
    )
    .await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn search_provider(
    state: tauri::State<'_, DesktopState>,
    input: ProviderSearchInput,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    let runtime = state.provider_runtime(&kernel)?;
    let configuration = state.network.load()?;
    providers::search(
        &runtime,
        &kernel,
        access.workspace_id(),
        input,
        configuration.outbound_policy(),
    )
    .await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn track_provider_candidate(
    state: tauri::State<'_, DesktopState>,
    input: ProviderSelectionInput,
) -> Result<records::CreateRecordView, DesktopProblem> {
    let kernel = state.kernel()?;
    let store = KeyringSetupSecretStore::new(kernel.data_root_identity());
    let access = records::require_access(&kernel, &store)?;
    let runtime = state.provider_runtime(&kernel)?;
    let configuration = state.network.load()?;
    let candidate = providers::fetch_selection(
        &runtime,
        &kernel,
        access.workspace_id(),
        input,
        configuration.outbound_policy(),
    )
    .await?;
    state
        .artwork
        .cache_candidate(
            &candidate,
            configuration.outbound_policy(),
            runtime.transport(),
        )
        .await?;
    records::create_provider_record(&kernel, access, candidate)
}

#[cfg(feature = "desktop-runtime")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyProviderMetadataInput {
    record_id: String,
    selection: ProviderSelectionInput,
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn apply_provider_metadata(
    state: tauri::State<'_, DesktopState>,
    input: ApplyProviderMetadataInput,
) -> Result<(), DesktopProblem> {
    let kernel = state.kernel()?;
    let store = KeyringSetupSecretStore::new(kernel.data_root_identity());
    let access = records::require_access(&kernel, &store)?;
    let runtime = state.provider_runtime(&kernel)?;
    let record_id = input
        .record_id
        .parse::<RecordId>()
        .map_err(|_| DesktopProblem::invalid_input("record_id is not a valid record identifier"))?;
    let configuration = state.network.load()?;
    let candidate = providers::fetch_selection(
        &runtime,
        &kernel,
        access.workspace_id(),
        input.selection,
        configuration.outbound_policy(),
    )
    .await?;
    state
        .artwork
        .cache_candidate(
            &candidate,
            configuration.outbound_policy(),
            runtime.transport(),
        )
        .await?;
    records::apply_provider_metadata(&kernel, access, record_id, candidate)
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn list_records(
    state: tauri::State<'_, DesktopState>,
) -> Result<records::RecordPage, DesktopProblem> {
    let kernel = state.kernel()?;
    records::list_records(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        &state.artwork,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn create_record(
    state: tauri::State<'_, DesktopState>,
    grain: fasti_domain::Grain,
) -> Result<records::CreateRecordView, DesktopProblem> {
    let kernel = state.kernel()?;
    records::create_record(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        grain,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn attach_identifier(
    state: tauri::State<'_, DesktopState>,
    input: records::AttachIdentifierInput,
) -> Result<records::AttachIdentifierView, DesktopProblem> {
    let kernel = state.kernel()?;
    records::attach_identifier(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        input,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn register_namespace(
    state: tauri::State<'_, DesktopState>,
    input: records::RegisterNamespaceInput,
) -> Result<records::RegisterNamespaceView, DesktopProblem> {
    let kernel = state.kernel()?;
    records::register_namespace(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        input,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn list_tracking_dispositions(
    state: tauri::State<'_, DesktopState>,
) -> Result<fasti_contracts::ListTrackingDispositionsResponse, DesktopProblem> {
    let kernel = state.kernel()?;
    records::list_tracking_dispositions(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn set_tracking_disposition(
    state: tauri::State<'_, DesktopState>,
    input: records::SetTrackingDispositionInput,
) -> Result<fasti_contracts::TrackingDispositionStateDto, DesktopProblem> {
    let kernel = state.kernel()?;
    records::set_tracking_disposition(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        input,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn get_nuvio_collections(
    state: tauri::State<'_, DesktopState>,
) -> Result<fasti_contracts::NuvioCollectionsStateDto, DesktopProblem> {
    let kernel = state.kernel()?;
    nuvio_collections::get(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn replace_nuvio_collections(
    state: tauri::State<'_, DesktopState>,
    document: fasti_contracts::NuvioCollectionsDocumentDto,
) -> Result<fasti_contracts::NuvioCollectionsStateDto, DesktopProblem> {
    let kernel = state.kernel()?;
    nuvio_collections::replace(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        document,
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn clear_nuvio_collections(
    state: tauri::State<'_, DesktopState>,
) -> Result<fasti_contracts::NuvioCollectionsStateDto, DesktopProblem> {
    let kernel = state.kernel()?;
    nuvio_collections::clear(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn list_reviews(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<reviews::ReviewItem>, DesktopProblem> {
    let kernel = state.kernel()?;
    reviews::list_reviews(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn resolve_review(
    state: tauri::State<'_, DesktopState>,
    input: reviews::ResolveReviewInput,
) -> Result<reviews::ResolveReviewOutcome, DesktopProblem> {
    let kernel = state.kernel()?;
    reviews::resolve_review(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        input,
    )
}

fn explicit_data_root(value: Option<OsString>) -> io::Result<PathBuf> {
    value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "FASTI_DATA_ROOT must name an explicit, non-empty directory",
            )
        })
}

fn select_data_root(
    explicit: Option<OsString>,
    platform_sandbox: Option<PathBuf>,
) -> io::Result<PathBuf> {
    match explicit {
        Some(value) => explicit_data_root(Some(value)),
        None => platform_sandbox.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "FASTI_DATA_ROOT must name an explicit, non-empty directory",
            )
        }),
    }
}

#[cfg(feature = "desktop-runtime")]
fn data_root(app: &tauri::App) -> io::Result<PathBuf> {
    let explicit = std::env::var_os("FASTI_DATA_ROOT");
    if explicit.is_some() {
        return select_data_root(explicit, None);
    }
    #[cfg(target_os = "android")]
    {
        let platform_sandbox = app.path().app_data_dir().map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Fasti could not resolve the Android app data directory",
            )
        })?;
        select_data_root(None, Some(platform_sandbox))
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        select_data_root(None, None)
    }
}

#[cfg(feature = "desktop-runtime")]
#[cfg_attr(target_os = "android", tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            secure_storage::initialize().map_err(|()| {
                io::Error::other("Fasti could not initialize the platform credential store")
            })?;
            let config_root = app.path().app_config_dir()?;
            let artwork_root = app.path().app_cache_dir()?.join("provider-artwork");
            let data_root = data_root(app)?;
            let artwork = artwork::ArtworkCache::new(artwork_root);
            artwork.prepare().map_err(|_| {
                io::Error::other("Fasti could not prepare its private artwork cache")
            })?;
            app.asset_protocol_scope()
                .allow_directory(artwork.root(), false)?;
            app.manage(DesktopState {
                data_root,
                kernel: Mutex::new(None),
                setup_gate: Mutex::new(()),
                network: NetworkConfigStore::new(&config_root),
                artwork,
                provider_runtime: Mutex::new(None),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            setup_status,
            complete_setup,
            list_api_clients,
            create_api_client,
            revoke_api_client,
            load_network_configuration,
            save_network_configuration,
            test_endpoint_connection,
            provider_credential_status,
            save_provider_credential,
            delete_provider_credential,
            test_provider_credential,
            read_provider_health,
            search_provider,
            track_provider_candidate,
            apply_provider_metadata,
            list_records,
            create_record,
            attach_identifier,
            register_namespace,
            list_tracking_dispositions,
            set_tracking_disposition,
            get_nuvio_collections,
            replace_nuvio_collections,
            clear_nuvio_collections,
            list_reviews,
            resolve_review
        ])
        .run(tauri::generate_context!())
        .expect("Fasti desktop shell failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_requires_an_explicit_non_empty_data_root() {
        assert!(select_data_root(None, None).is_err());
        assert!(select_data_root(Some(OsString::new()), None).is_err());
        assert_eq!(
            select_data_root(Some(OsString::from("/tmp/fasti")), None).expect("data root"),
            PathBuf::from("/tmp/fasti")
        );
    }

    #[test]
    fn explicit_data_root_precedes_a_platform_sandbox() {
        assert_eq!(
            select_data_root(
                Some(OsString::from("/explicit/fasti")),
                Some(PathBuf::from("/sandbox/fasti")),
            )
            .expect("explicit data root"),
            PathBuf::from("/explicit/fasti")
        );
    }

    #[test]
    fn platform_sandbox_is_used_when_an_explicit_root_is_absent() {
        assert_eq!(
            select_data_root(None, Some(PathBuf::from("/sandbox/fasti")))
                .expect("platform sandbox"),
            PathBuf::from("/sandbox/fasti")
        );
    }
}
