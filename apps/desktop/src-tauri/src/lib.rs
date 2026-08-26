//! Trusted-host adapters for the Fasti desktop and Android shells.

#![cfg_attr(not(feature = "desktop-runtime"), allow(dead_code))]

mod api_clients;
mod endpoint;
mod network_config;
mod outbound_http;
mod providers;
mod records;
mod reviews;
mod secure_storage;
mod setup;

#[cfg(feature = "desktop-runtime")]
use endpoint::{EndpointConnectionInput, EndpointConnectionStatus};
#[cfg(feature = "desktop-runtime")]
use fasti_store::SqliteKernel;
#[cfg(feature = "desktop-runtime")]
use network_config::{NetworkConfigStore, NetworkConfiguration, SaveNetworkConfigurationInput};
#[cfg(feature = "desktop-runtime")]
use providers::{
    DeleteProviderCredentialInput, ProviderCandidate, ProviderCredentialStatus,
    ProviderSearchInput, SaveProviderCredentialInput,
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
        let kernel = Arc::new(
            SqliteKernel::open(&self.data_root)
                .map_err(|_| DesktopProblem::storage("Fasti could not open its local data root."))?,
        );
        *current = Some(Arc::clone(&kernel));
        Ok(kernel)
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
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    let kernel = state.kernel()?;
    providers::credential_statuses(kernel.data_root_identity())
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn save_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: SaveProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    let kernel = state.kernel()?;
    providers::save_credential(input, kernel.data_root_identity())
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn delete_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: DeleteProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    let kernel = state.kernel()?;
    providers::delete_credential(input, kernel.data_root_identity())
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn search_provider(
    state: tauri::State<'_, DesktopState>,
    input: ProviderSearchInput,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let kernel = state.kernel()?;
    let configuration = state.network.load()?;
    providers::search(
        input,
        configuration.outbound_policy(),
        kernel.data_root_identity(),
    )
    .await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn list_records(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<records::RecordSummary>, DesktopProblem> {
    let kernel = state.kernel()?;
    records::list_records(
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
            let data_root = data_root(app)?;
            app.manage(DesktopState {
                data_root,
                kernel: Mutex::new(None),
                setup_gate: Mutex::new(()),
                network: NetworkConfigStore::new(&config_root),
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
            search_provider,
            list_records,
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
