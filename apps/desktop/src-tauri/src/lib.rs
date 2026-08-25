mod connection;
mod outbound_http;
mod provider;
mod secure_storage;
mod setup;

use fasti_store::SqliteKernel;
use setup::{DesktopProblem, KeyringSetupSecretStore, SetupStatus};
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;

struct DesktopState {
    data_root: PathBuf,
    kernel: Mutex<Option<Arc<SqliteKernel>>>,
    setup_gate: Mutex<()>,
    secrets: KeyringSetupSecretStore,
}

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
}

#[tauri::command(async)]
fn setup_status(state: tauri::State<'_, DesktopState>) -> Result<SetupStatus, DesktopProblem> {
    let kernel = state.kernel()?;
    setup::inspect_setup(&kernel, &state.secrets)
}

#[tauri::command(async)]
fn complete_setup(state: tauri::State<'_, DesktopState>) -> Result<SetupStatus, DesktopProblem> {
    let _guard = state
        .setup_gate
        .lock()
        .map_err(|_| DesktopProblem::storage("The setup lock is unavailable."))?;
    let kernel = state.kernel()?;
    setup::complete_setup(&kernel, &state.secrets)
}

#[tauri::command(async)]
async fn test_endpoint_connection(
    endpoint: String,
) -> Result<connection::ConnectionStatus, DesktopProblem> {
    connection::test(endpoint).await
}

#[tauri::command(async)]
fn provider_credential_status() -> Result<Vec<provider::ProviderCredentialStatus>, DesktopProblem> {
    provider::credential_status()
}

#[tauri::command(async)]
fn save_provider_key(
    provider: String,
    key: Option<String>,
) -> Result<Vec<provider::ProviderCredentialStatus>, DesktopProblem> {
    provider::save_key(&provider, key)?;
    provider::credential_status()
}

#[tauri::command(async)]
async fn search_provider(
    provider: String,
    query: String,
    policy: fasti_application::OutboundAccessPolicy,
) -> Result<Vec<fasti_application::ProviderCandidate>, DesktopProblem> {
    provider::search(provider, query, policy).await
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

fn data_root(_app: &tauri::App) -> io::Result<PathBuf> {
    if let Some(value) = std::env::var_os("FASTI_DATA_ROOT") {
        return explicit_data_root(Some(value));
    }
    #[cfg(mobile)]
    return _app.path().app_data_dir().map_err(io::Error::other);
    #[cfg(not(mobile))]
    explicit_data_root(None)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            secure_storage::initialize()
                .map_err(|_| io::Error::other("Fasti secure storage is unavailable."))?;
            app.manage(DesktopState {
                data_root: data_root(app)?,
                kernel: Mutex::new(None),
                setup_gate: Mutex::new(()),
                secrets: KeyringSetupSecretStore,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            setup_status,
            complete_setup,
            test_endpoint_connection,
            provider_credential_status,
            save_provider_key,
            search_provider
        ])
        .run(tauri::generate_context!())
        .expect("Fasti desktop shell failed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_requires_an_explicit_non_empty_data_root() {
        assert!(explicit_data_root(None).is_err());
        assert!(explicit_data_root(Some(OsString::new())).is_err());
        assert_eq!(
            explicit_data_root(Some(OsString::from("/tmp/fasti"))).expect("data root"),
            PathBuf::from("/tmp/fasti")
        );
    }
}
