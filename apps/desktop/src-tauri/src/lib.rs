//! Trusted-host adapters for the Fasti desktop and Android shells.

#![cfg_attr(not(feature = "desktop-runtime"), allow(dead_code))]

mod api_clients;
mod artwork;
mod endpoint;
mod metadata;
mod network_config;
mod nuvio_collections;
mod providers;
mod records;
mod reviews;
mod secure_storage;
mod setup;

#[cfg(feature = "desktop-runtime")]
use axum::response::IntoResponse;
#[cfg(feature = "desktop-runtime")]
use endpoint::{EndpointConnectionInput, EndpointConnectionStatus};
#[cfg(feature = "desktop-runtime")]
use fasti_application::{AccessAdministrationPort, CapabilityKey};
#[cfg(feature = "desktop-runtime")]
use fasti_api::{
    FASTI_ACCESS_BINDING_COOKIE, FASTI_ACCESS_CALLBACK_PATH, FASTI_ACCESS_HOST,
    FASTI_ACCESS_ORIGIN,
};
#[cfg(feature = "desktop-runtime")]
use fasti_domain::{AuthCeremonyPurpose, AuthCeremonySelection, RecordId, RequestCorrelationId};
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
use serde::Serialize;
#[cfg(feature = "desktop-runtime")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "desktop-runtime")]
use tauri::{
    webview::cookie::{time::Duration as CookieDuration, Cookie, SameSite},
    Manager,
};

#[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
struct AccessServer {
    inner: Mutex<AccessServerInner>,
}

#[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
const ACCESS_SERVER_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
struct AccessServerInner {
    task: Option<tauri::async_runtime::JoinHandle<io::Result<()>>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

#[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
impl AccessServer {
    fn is_running(&self) -> bool {
        self.inner
            .lock()
            .is_ok_and(|inner| inner.task.as_ref().is_some_and(|task| !task.inner().is_finished()))
    }

    async fn shutdown(&self) -> io::Result<()> {
        let (shutdown, task) = {
            let mut inner = self
                .inner
                .lock()
                .map_err(|_| io::Error::other("the Access server lock is unavailable"))?;
            (inner.shutdown.take(), inner.task.take())
        };
        if let Some(shutdown) = shutdown {
            let _ = shutdown.send(());
        }
        let Some(task) = task else {
            return Ok(());
        };
        tokio::time::timeout(ACCESS_SERVER_SHUTDOWN_TIMEOUT, task)
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Access server shutdown timed out"))?
            .map_err(io::Error::other)??;
        Ok(())
    }
}

#[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
impl Drop for AccessServer {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(shutdown) = inner.shutdown.take() {
                let _ = shutdown.send(());
            }
        }
    }
}

#[cfg(feature = "desktop-runtime")]
struct DesktopState {
    data_root: PathBuf,
    kernel: Mutex<Option<Arc<SqliteKernel>>>,
    setup_gate: Mutex<()>,
    network: NetworkConfigStore,
    artwork: artwork::ArtworkCache,
    provider_runtime: Mutex<Option<Arc<fasti_provider_runtime::ProviderRuntime>>>,
    // ponytail: one provider gate prevents credential races; split by provider if contention appears.
    provider_operation_gate: tokio::sync::Mutex<()>,
    access_runtime: Option<Arc<fasti_api::DirectLoopbackAccessRuntime>>,
    #[cfg(not(target_os = "android"))]
    access_server: Arc<AccessServer>,
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
fn auth_binding_cookie(value: String, max_age_seconds: i64) -> Cookie<'static> {
    Cookie::build((FASTI_ACCESS_BINDING_COOKIE, value))
        .domain("127.0.0.1")
        .path(FASTI_ACCESS_CALLBACK_PATH)
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(max_age_seconds.clamp(0, 600)))
        .build()
}

#[cfg(feature = "desktop-runtime")]
#[derive(Debug, Serialize)]
struct StartedFirstAdministratorBootstrap {
    authorization_url: String,
    expires_at: String,
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn start_first_administrator_bootstrap(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, DesktopState>,
) -> Result<StartedFirstAdministratorBootstrap, DesktopProblem> {
    #[cfg(target_os = "android")]
    {
        let _ = (window, state);
        return Err(DesktopProblem::access_unavailable(
            "The Android WebView cannot install the required callback-only binding cookie.",
        ));
    }
    #[cfg(not(target_os = "android"))]
    {
        if !state.access_server.is_running() {
            return Err(DesktopProblem::access_unavailable(
                "The fixed local Access listener is not running.",
            ));
        }
        let _guard = state
            .setup_gate
            .lock()
            .map_err(|_| DesktopProblem::storage("The setup lock is unavailable."))?;
        let kernel = state.kernel()?;
        let access = setup::authenticate(
            &kernel,
            &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        )?
        .ok_or_else(DesktopProblem::not_authenticated)?;
        let selection = AuthCeremonySelection::try_new(
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            access.workspace_id(),
            access.grant_id(),
            None,
            None,
        )
        .map_err(|_| {
            DesktopProblem::access_unavailable(
                "The saved local administrator selection is not valid for first-run setup.",
            )
        })?;
        let bootstrap_secret = kernel
            .ensure_bootstrap_secret()
            .map_err(|problem| DesktopProblem::application(&problem))?;
        let runtime = state.access_runtime.as_ref().ok_or_else(|| {
            DesktopProblem::access_unavailable(
                "The pinned TrailBase installation was not active when Fasti started.",
            )
        })?;
        let started = runtime
            .start_first_administrator_bootstrap(selection, bootstrap_secret)
            .map_err(|code| {
                DesktopProblem::application(&fasti_application::FastiProblem::from_code(
                    code,
                    CapabilityKey::AccessIdentityBootstrap,
                    RequestCorrelationId::new_v7(),
                ))
            })?;
        let max_age = (started.expires_at() - chrono::Utc::now()).num_seconds();
        if window
            .set_cookie(auth_binding_cookie(
                started.browser_binding().expose_hex(),
                max_age,
            ))
            .is_err()
        {
            let detail = if runtime
                .cancel_first_administrator_bootstrap(started)
                .is_ok()
            {
                "Fasti could not queue the required callback-only WebView cookie. The unfinished sign-in was cancelled."
            } else {
                "Fasti could not queue the required callback-only WebView cookie or cancel the unfinished sign-in safely."
            };
            return Err(DesktopProblem::access_unavailable(detail));
        }
        Ok(StartedFirstAdministratorBootstrap {
            authorization_url: started.authorization_url().to_owned(),
            expires_at: started.expires_at().to_rfc3339(),
        })
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
async fn run_blocking_provider_operation<T, F>(
    gate: &tokio::sync::Mutex<()>,
    operation: F,
) -> Result<T, DesktopProblem>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, DesktopProblem> + Send + 'static,
{
    let _provider_guard = gate.lock().await;
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| DesktopProblem::storage("The provider operation task did not complete."))?
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn provider_credential_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let runtime = state.provider_runtime(&kernel)?;
    run_blocking_provider_operation(&state.provider_operation_gate, move || {
        let access = records::require_access(
            &kernel,
            &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        )?;
        providers::credential_statuses(&runtime, &kernel, access.workspace_id())
    })
    .await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn save_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: SaveProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let runtime = state.provider_runtime(&kernel)?;
    run_blocking_provider_operation(&state.provider_operation_gate, move || {
        let access = records::require_access(
            &kernel,
            &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        )?;
        providers::save_credential(&runtime, &kernel, access.workspace_id(), input)
    })
    .await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn delete_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: DeleteProviderCredentialInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let kernel = state.kernel()?;
    let runtime = state.provider_runtime(&kernel)?;
    run_blocking_provider_operation(&state.provider_operation_gate, move || {
        let access = records::require_access(
            &kernel,
            &KeyringSetupSecretStore::new(kernel.data_root_identity()),
        )?;
        providers::delete_credential(&runtime, &kernel, access.workspace_id(), input)
    })
    .await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command]
async fn test_provider_credential(
    state: tauri::State<'_, DesktopState>,
    input: ProviderCapabilityInput,
) -> Result<Vec<ProviderCredentialStatusView>, DesktopProblem> {
    let _provider_guard = state.provider_operation_gate.lock().await;
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
    let _provider_guard = state.provider_operation_gate.lock().await;
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
    let _provider_guard = state.provider_operation_gate.lock().await;
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
    let _provider_guard = state.provider_operation_gate.lock().await;
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
    let _provider_guard = state.provider_operation_gate.lock().await;
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
#[tauri::command]
async fn refresh_metadata_claims(
    state: tauri::State<'_, DesktopState>,
    input: fasti_contracts::RefreshMetadataClaimsRequest,
) -> Result<fasti_contracts::RefreshMetadataClaimsResponse, DesktopProblem> {
    let _provider_guard = state.provider_operation_gate.lock().await;
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    let runtime = state.provider_runtime(&kernel)?;
    let policy = state.network.load()?.outbound_policy().clone();
    metadata::refresh(kernel, runtime, policy, access, input).await
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn read_metadata_projection(
    state: tauri::State<'_, DesktopState>,
    input: metadata::ReadMetadataProjectionInput,
) -> Result<fasti_contracts::MetadataProjectionResponse, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    metadata::read(&kernel, access, input)
}

#[cfg(feature = "desktop-runtime")]
#[tauri::command(async)]
fn configure_metadata_projection(
    state: tauri::State<'_, DesktopState>,
    input: fasti_contracts::ConfigureMetadataProjectionRequest,
) -> Result<fasti_contracts::MetadataProjectionConfigurationResponse, DesktopProblem> {
    let kernel = state.kernel()?;
    let access = records::require_access(
        &kernel,
        &KeyringSetupSecretStore::new(kernel.data_root_identity()),
    )?;
    metadata::configure(&kernel, access, input)
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

fn trailbase_root() -> io::Result<Option<PathBuf>> {
    let value = std::env::var_os("FASTI_TRAILBASE_ROOT");
    if value.as_ref().is_some_and(|value| value.is_empty()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FASTI_TRAILBASE_ROOT must name a directory when it is set",
        ));
    }
    Ok(value.map(PathBuf::from))
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

#[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
fn bind_access_listener() -> io::Result<std::net::TcpListener> {
    let listener = std::net::TcpListener::bind(FASTI_ACCESS_HOST).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("Fasti Access requires the fixed 127.0.0.1:8420 listener: {error}"),
        )
    })?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

#[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
fn serves_embedded_asset(method: &axum::http::Method, path: &str) -> bool {
    matches!(method, &axum::http::Method::GET | &axum::http::Method::HEAD)
        && path != "/api"
        && !path.starts_with("/api/")
}

#[cfg(feature = "desktop-runtime")]
#[cfg_attr(target_os = "android", tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(not(target_os = "android"))]
    let access_server_owner = Arc::new(Mutex::new(None::<Arc<AccessServer>>));
    #[cfg(not(target_os = "android"))]
    let setup_access_server_owner = Arc::clone(&access_server_owner);
    let app = tauri::Builder::default()
        .setup(move |app| {
            secure_storage::initialize().map_err(|()| {
                io::Error::other("Fasti could not initialize the platform credential store")
            })?;
            let config_root = app.path().app_config_dir()?;
            let artwork_root = app.path().app_cache_dir()?.join("provider-artwork");
            let data_root = data_root(app)?;
            let kernel = Arc::new(
                SqliteKernel::open(&data_root)
                    .map_err(|_| io::Error::other("Fasti could not open its local data root"))?,
            );
            let artwork = artwork::ArtworkCache::new(artwork_root);
            artwork.prepare().map_err(|_| {
                io::Error::other("Fasti could not prepare its private artwork cache")
            })?;
            app.asset_protocol_scope()
                .allow_directory(artwork.root(), false)?;
            #[cfg(not(target_os = "android"))]
            let (access_runtime, access_server) = {
                let listener = bind_access_listener()?;
                let bound_addr = listener.local_addr()?;
                let trailbase_root = trailbase_root()?;
                let runtime = Arc::new(fasti_api::DirectLoopbackAccessRuntime::new(
                    kernel.clone(),
                    bound_addr,
                    false,
                    &data_root,
                    trailbase_root.as_deref(),
                )?);
                let resolver = Arc::new(app.asset_resolver());
                let router = runtime.router().fallback(
                    move |method: axum::http::Method, uri: axum::http::Uri| {
                        let resolver = resolver.clone();
                        async move {
                            let path = uri.path();
                            if !serves_embedded_asset(&method, path) {
                                return axum::http::StatusCode::NOT_FOUND.into_response();
                            }
                            let Some(asset) = resolver.get(path.to_owned()) else {
                                return axum::http::StatusCode::NOT_FOUND.into_response();
                            };
                            let Ok(content_type) = asset.mime_type().parse::<axum::http::HeaderValue>() else {
                                return axum::http::StatusCode::INTERNAL_SERVER_ERROR
                                    .into_response();
                            };
                            let content_length = asset.bytes().len();
                            let csp = asset.csp_header().map(str::to_owned);
                            let body = if method == axum::http::Method::HEAD {
                                axum::body::Body::empty()
                            } else {
                                axum::body::Body::from(asset.bytes)
                            };
                            let mut response = axum::response::Response::new(body);
                            response.headers_mut().insert(
                                axum::http::header::CONTENT_TYPE,
                                content_type,
                            );
                            response.headers_mut().insert(
                                axum::http::header::CONTENT_LENGTH,
                                axum::http::HeaderValue::try_from(content_length.to_string())
                                    .expect("asset length is valid ASCII"),
                            );
                            if let Some(csp) = csp {
                                let Ok(csp) = csp.parse::<axum::http::HeaderValue>() else {
                                    return axum::http::StatusCode::INTERNAL_SERVER_ERROR
                                        .into_response();
                                };
                                response.headers_mut().insert(
                                    axum::http::header::CONTENT_SECURITY_POLICY,
                                    csp,
                                );
                            }
                            response
                        }
                    },
                );
                let (shutdown, shutdown_signal) = tokio::sync::oneshot::channel();
                let task = tauri::async_runtime::spawn(async move {
                    let listener = tokio::net::TcpListener::from_std(listener)?;
                    axum::serve(listener, router)
                        .with_graceful_shutdown(async move {
                            let _ = shutdown_signal.await;
                        })
                        .await
                        .map_err(io::Error::other)
                });
                let access_server = Arc::new(AccessServer {
                    inner: Mutex::new(AccessServerInner {
                        task: Some(task),
                        shutdown: Some(shutdown),
                    }),
                });
                *setup_access_server_owner
                    .lock()
                    .map_err(|_| io::Error::other("the Access server owner lock is unavailable"))? =
                    Some(Arc::clone(&access_server));
                (
                    Some(runtime),
                    access_server,
                )
            };
            #[cfg(target_os = "android")]
            let access_runtime = None;
            app.manage(DesktopState {
                data_root,
                kernel: Mutex::new(Some(kernel)),
                setup_gate: Mutex::new(()),
                network: NetworkConfigStore::new(&config_root),
                artwork,
                provider_runtime: Mutex::new(None),
                // ponytail: serialize provider vault mutation and metadata claim
                // refresh; use per-provider gates only if measured throughput needs it.
                provider_operation_gate: tokio::sync::Mutex::new(()),
                access_runtime,
                #[cfg(not(target_os = "android"))]
                access_server,
            });
            #[cfg(not(target_os = "android"))]
            app.get_webview_window("main")
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the main WebView is missing"))?
                .navigate(
                    tauri::Url::parse(FASTI_ACCESS_ORIGIN)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?,
                )?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            setup_status,
            complete_setup,
            start_first_administrator_bootstrap,
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
            refresh_metadata_claims,
            read_metadata_projection,
            configure_metadata_projection,
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
        .build(tauri::generate_context!())
        .expect("Fasti desktop shell failed");
    #[cfg(not(target_os = "android"))]
    {
        let exit_code = app.run_return(|_, _| {});
        let access_server = access_server_owner
            .lock()
            .expect("the Access server owner lock is unavailable")
            .take();
        if let Some(access_server) = access_server {
            tauri::async_runtime::block_on(access_server.shutdown())
                .expect("Fasti Access server shutdown failed");
        }
        std::process::exit(exit_code);
    }
    #[cfg(target_os = "android")]
    app.run(|_, _| {});
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "desktop-runtime")]
    use std::sync::atomic::{AtomicBool, Ordering};
    #[cfg(feature = "desktop-runtime")]
    use std::sync::Arc;
    #[cfg(feature = "desktop-runtime")]
    use std::time::Duration;

    #[test]
    fn desktop_requires_an_explicit_non_empty_data_root() {
        assert!(select_data_root(None, None).is_err());
        assert!(select_data_root(Some(OsString::new()), None).is_err());
        assert_eq!(
            select_data_root(Some(OsString::from("/tmp/fasti")), None).expect("data root"),
            PathBuf::from("/tmp/fasti")
        );
    }

    #[cfg(feature = "desktop-runtime")]
    #[test]
    fn first_administrator_binding_cookie_is_narrow_and_bounded() {
        let cookie = auth_binding_cookie("binding".to_owned(), 999);

        assert_eq!(cookie.name(), FASTI_ACCESS_BINDING_COOKIE);
        assert_eq!(cookie.value(), "binding");
        assert_eq!(cookie.domain(), Some("127.0.0.1"));
        assert_eq!(cookie.path(), Some(FASTI_ACCESS_CALLBACK_PATH));
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        assert_eq!(
            cookie.max_age().map(|duration| duration.whole_seconds()),
            Some(600)
        );
    }

    #[cfg(feature = "desktop-runtime")]
    #[test]
    fn first_administrator_command_response_contains_no_secret_or_ceremony_id() {
        let response = StartedFirstAdministratorBootstrap {
            authorization_url: "http://127.0.0.1:4000/_/auth/login".to_owned(),
            expires_at: "2026-08-31T12:00:00Z".to_owned(),
        };
        let value = serde_json::to_value(response).expect("serialize command response");

        assert_eq!(
            value.as_object().expect("response object").keys().collect::<Vec<_>>(),
            ["authorization_url", "expires_at"]
        );
        let encoded = value.to_string();
        assert!(!encoded.contains("binding"));
        assert!(!encoded.contains("secret"));
        assert!(!encoded.contains("ceremony"));
    }

    #[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
    #[test]
    fn packaged_access_refuses_an_occupied_fixed_listener() {
        let occupied = std::net::TcpListener::bind(FASTI_ACCESS_HOST).ok();

        let error = bind_access_listener().expect_err("fallback ports are forbidden");
        assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
        if let Some(occupied) = occupied {
            assert_eq!(
                occupied.local_addr().expect("occupied address"),
                FASTI_ACCESS_HOST.parse().expect("fixed Access address")
            );
        }
    }

    #[cfg(feature = "desktop-runtime")]
    #[test]
    fn packaged_capability_allows_only_the_main_exact_loopback_origin() {
        let configuration: serde_json::Value = serde_json::from_str(include_str!(
            "../tauri.conf.json"
        ))
        .expect("packaged Tauri configuration JSON");
        let desktop: serde_json::Value = serde_json::from_str(include_str!(
            "../capabilities/main-loopback.json"
        ))
        .expect("packaged capability JSON");
        let android: serde_json::Value = serde_json::from_str(include_str!(
            "../capabilities/main-android-local.json"
        ))
        .expect("Android capability JSON");

        assert_eq!(desktop["windows"], serde_json::json!(["main"]));
        assert_eq!(
            desktop["platforms"],
            serde_json::json!(["linux", "macOS", "windows"])
        );
        assert_eq!(desktop["local"], false);
        assert_eq!(
            desktop["remote"]["urls"],
            serde_json::json!(["http://127.0.0.1:8420/*"])
        );
        assert_eq!(desktop["permissions"], serde_json::json!(["main-runtime"]));
        assert_eq!(
            configuration["app"]["security"]["csp"],
            "default-src 'self'; connect-src 'self' ipc: http://ipc.localhost; img-src 'self' data: asset: http://asset.localhost; style-src 'self' 'unsafe-inline'; font-src 'self'"
        );
        assert_eq!(android["windows"], serde_json::json!(["main"]));
        assert_eq!(android["platforms"], serde_json::json!(["android"]));
        assert_eq!(android["local"], true);
        assert!(android.get("remote").is_none());
        assert_eq!(android["permissions"], serde_json::json!(["main-runtime"]));
    }

    #[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
    #[test]
    fn embedded_spa_fallback_serves_navigation_without_masking_api_or_mutations() {
        assert!(serves_embedded_asset(&axum::http::Method::GET, "/first-run"));
        assert!(serves_embedded_asset(&axum::http::Method::HEAD, "/first-run"));
        assert!(!serves_embedded_asset(&axum::http::Method::GET, "/api"));
        assert!(!serves_embedded_asset(
            &axum::http::Method::GET,
            "/api/missing"
        ));
        assert!(!serves_embedded_asset(
            &axum::http::Method::POST,
            "/first-run"
        ));
    }

    #[cfg(all(feature = "desktop-runtime", not(target_os = "android")))]
    #[tokio::test]
    async fn access_server_shutdown_waits_for_in_flight_work() {
        let stopped = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&stopped);
        let (shutdown, shutdown_signal) = tokio::sync::oneshot::channel();
        let task = tauri::async_runtime::spawn(async move {
            let _ = shutdown_signal.await;
            tokio::task::yield_now().await;
            observed.store(true, Ordering::SeqCst);
            Ok(())
        });
        let server = AccessServer {
            inner: Mutex::new(AccessServerInner {
                task: Some(task),
                shutdown: Some(shutdown),
            }),
        };
        assert!(server.is_running());

        server.shutdown().await.expect("graceful shutdown");

        assert!(stopped.load(Ordering::SeqCst));
        assert!(!server.is_running());
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

    #[cfg(feature = "desktop-runtime")]
    #[tokio::test]
    async fn blocking_provider_operations_wait_for_the_shared_gate() {
        let gate = tokio::sync::Mutex::new(());
        let held = gate.lock().await;
        let started = Arc::new(AtomicBool::new(false));
        let operation_started = Arc::clone(&started);
        let operation = run_blocking_provider_operation(&gate, move || {
            operation_started.store(true, Ordering::SeqCst);
            Ok(())
        });
        tokio::pin!(operation);

        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut operation)
                .await
                .is_err()
        );
        assert!(!started.load(Ordering::SeqCst));

        drop(held);
        operation.await.expect("serialized provider operation");
        assert!(started.load(Ordering::SeqCst));
    }
}
