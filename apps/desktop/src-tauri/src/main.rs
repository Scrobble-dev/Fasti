mod setup;

use fasti_store::SqliteKernel;
use setup::{DesktopProblem, KeyringSetupSecretStore, SetupStatus};
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

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let data_root = app.path().app_data_dir()?.join("data");
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
