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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_root = explicit_data_root(std::env::var_os("FASTI_DATA_ROOT"))?;
            app.manage(DesktopState {
                data_root,
                kernel: Mutex::new(None),
                setup_gate: Mutex::new(()),
                secrets: KeyringSetupSecretStore,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![setup_status, complete_setup])
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
