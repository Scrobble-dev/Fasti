use super::*;
use crate::records::require_access;
use crate::setup::{complete_setup, test_support::new_kernel, test_support::MemoryStore};
use fasti_application::{
    CredentialReference, CredentialVaultError, CredentialVaultPort, ProviderStatePort,
    StoredCredential,
};
use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

#[derive(Default)]
struct PausedMemoryVault {
    values: Mutex<BTreeMap<String, Vec<u8>>>,
    store_pause: Mutex<Option<WritePause>>,
    revoke_pause: Mutex<Option<WritePause>>,
    write_count: AtomicUsize,
    revoke_count: AtomicUsize,
}

impl CredentialVaultPort for PausedMemoryVault {
    fn source(
        &self,
        reference: &CredentialReference,
    ) -> Result<CredentialVaultSource, CredentialVaultError> {
        Ok(
            if self
                .values
                .lock()
                .expect("memory vault")
                .contains_key(reference.as_str())
            {
                CredentialVaultSource::CredentialStore
            } else {
                CredentialVaultSource::None
            },
        )
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
        self.values
            .lock()
            .expect("memory vault")
            .insert(reference.as_str().to_owned(), secret.expose().to_vec());
        self.write_count.fetch_add(1, Ordering::SeqCst);
        StoredCredential::try_new(reference.clone(), 1).map_err(|_| CredentialVaultError::Rejected)
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
        reference: &CredentialReference,
    ) -> Result<CredentialSecret, CredentialVaultError> {
        let value = self
            .values
            .lock()
            .expect("memory vault")
            .get(reference.as_str())
            .cloned()
            .ok_or(CredentialVaultError::Missing)?;
        CredentialSecret::try_from_bytes(value).map_err(|_| CredentialVaultError::Rejected)
    }

    fn revoke(&self, reference: &CredentialReference) -> Result<(), CredentialVaultError> {
        let pause = self.revoke_pause.lock().expect("revoke pause").take();
        if let Some(pause) = pause {
            pause.wait();
        }
        self.values
            .lock()
            .expect("memory vault")
            .remove(reference.as_str());
        self.revoke_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

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

async fn cancel_after_pause<T: Send + 'static>(
    caller: tokio::task::JoinHandle<T>,
    entered: tokio::sync::oneshot::Receiver<()>,
    finish: std::sync::mpsc::Sender<()>,
    gate: &Arc<tokio::sync::Mutex<()>>,
) {
    let reached_pause = tokio::time::timeout(Duration::from_secs(5), entered).await;
    if !matches!(reached_pause, Ok(Ok(()))) {
        let _ = finish.send(());
        caller.abort();
        let _ = caller.await;
        panic!("provider mutation did not reach the selected vault write");
    }

    caller.abort();
    let cancelled = caller.await;
    let gate_was_held = gate.try_lock().is_err();
    let _ = finish.send(());
    let completed = tokio::time::timeout(Duration::from_secs(5), gate.lock())
        .await
        .expect("blocking provider mutation completes and releases its gate");
    drop(completed);

    match cancelled {
        Err(error) => assert!(
            error.is_cancelled(),
            "caller ended for a reason other than cancellation"
        ),
        Ok(_) => panic!("caller completed instead of being cancelled"),
    }
    assert!(
        gate_was_held,
        "cancellation released the provider gate before vault work completed"
    );
}

fn save_input(credential: &str) -> SaveProviderCredentialInput {
    SaveProviderCredentialInput {
        provider: "tmdb".to_owned(),
        capability_id: READ_CAPABILITY.to_owned(),
        credential: credential.to_owned(),
    }
}

fn delete_input() -> DeleteProviderCredentialInput {
    ProviderCapabilityInput {
        provider: "tmdb".to_owned(),
        capability_id: READ_CAPABILITY.to_owned(),
    }
}

fn assert_tmdb_states(
    kernel: &SqliteKernel,
    workspace_id: WorkspaceId,
    expected_count: usize,
    expected_status: ProviderCapabilityStatus,
    expected_credential_status: ProviderCredentialStatus,
    expected_reference: Option<&str>,
) -> u64 {
    let states = kernel
        .list_provider_capability_states(workspace_id)
        .expect("provider states")
        .into_iter()
        .filter(|state| state.provider_id().as_str() == "tmdb")
        .collect::<Vec<_>>();
    assert_eq!(states.len(), expected_count);

    let version = states
        .first()
        .expect("TMDB capability state")
        .capability_version();
    for state in states {
        assert_eq!(state.capability_status(), expected_status);
        assert_eq!(state.credential_status(), expected_credential_status);
        assert_eq!(
            state
                .credential_reference()
                .map(CredentialReference::as_str),
            expected_reference
        );
        assert_eq!(state.capability_version(), version);
    }
    version
}

#[tokio::test]
async fn cancelled_desktop_credential_waiter_never_starts_after_gate_release() {
    let (_root, kernel) = new_kernel();
    let store = MemoryStore::default();
    complete_setup(&kernel, &store).expect("complete setup");
    let workspace_id = require_access(&kernel, &store)
        .expect("authenticated access")
        .workspace_id();
    let before = kernel
        .list_provider_capability_states(workspace_id)
        .expect("initial provider states");
    let kernel = Arc::new(kernel);
    let vault = Arc::new(PausedMemoryVault::default());
    let runtime = ProviderRuntime::new(vault.clone());
    let gate = Arc::new(tokio::sync::Mutex::new(()));
    let held = gate.lock().await;
    let operation_gate = Arc::clone(&gate);
    let operation_kernel = Arc::clone(&kernel);
    let (started, entered) = tokio::sync::oneshot::channel();
    let caller = tokio::spawn(async move {
        let _ = started.send(());
        crate::run_blocking_provider_operation(&operation_gate, move || {
            save_credential(
                &runtime,
                &operation_kernel,
                workspace_id,
                save_input("cancelled-waiter-fixture"),
            )
        })
        .await
    });
    let reached_gate = tokio::time::timeout(Duration::from_secs(5), entered).await;
    caller.abort();
    let cancelled = caller.await;
    drop(held);
    let released = tokio::time::timeout(Duration::from_secs(5), gate.lock())
        .await
        .expect("cancelled waiter leaves the gate available");
    drop(released);

    assert!(matches!(reached_gate, Ok(Ok(()))));
    assert!(cancelled
        .expect_err("queued caller cancelled")
        .is_cancelled());
    assert!(vault.values.lock().expect("memory vault").is_empty());
    assert_eq!(vault.write_count.load(Ordering::SeqCst), 0);
    assert_eq!(vault.revoke_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        kernel
            .list_provider_capability_states(workspace_id)
            .expect("unchanged provider states"),
        before
    );
}

#[tokio::test]
async fn cancelled_desktop_credential_save_finishes_and_retry_converges() {
    const CREDENTIAL: &str = "cancelled-write-fixture";

    let (_root, kernel) = new_kernel();
    let store = MemoryStore::default();
    complete_setup(&kernel, &store).expect("complete setup");
    let workspace_id = require_access(&kernel, &store)
        .expect("authenticated access")
        .workspace_id();
    let kernel = Arc::new(kernel);
    let vault = Arc::new(PausedMemoryVault::default());
    let runtime = Arc::new(ProviderRuntime::new(vault.clone()));
    let capability_count = runtime
        .descriptor("tmdb")
        .expect("TMDB descriptor")
        .capabilities
        .len();
    let tmdb_reference = runtime
        .credential_reference("tmdb")
        .expect("TMDB credential reference");
    let gate = Arc::new(tokio::sync::Mutex::new(()));
    let (pause, entered, finish) = WritePause::new();
    *vault.store_pause.lock().expect("pause store") = Some(pause);

    let caller = {
        let operation_gate = Arc::clone(&gate);
        let operation_runtime = Arc::clone(&runtime);
        let operation_kernel = Arc::clone(&kernel);
        tokio::spawn(async move {
            crate::run_blocking_provider_operation(&operation_gate, move || {
                save_credential(
                    &operation_runtime,
                    &operation_kernel,
                    workspace_id,
                    save_input(CREDENTIAL),
                )
            })
            .await
        })
    };
    cancel_after_pause(caller, entered, finish, &gate).await;

    {
        let values = vault.values.lock().expect("memory vault");
        assert_eq!(values.len(), 1);
        assert_eq!(
            values.get(tmdb_reference.as_str()).map(Vec::as_slice),
            Some(CREDENTIAL.as_bytes())
        );
    }
    assert_eq!(vault.write_count.load(Ordering::SeqCst), 1);
    let cancelled_version = assert_tmdb_states(
        &kernel,
        workspace_id,
        capability_count,
        ProviderCapabilityStatus::Available,
        ProviderCredentialStatus::StoredUnverified,
        Some(tmdb_reference.as_str()),
    );

    let retry_runtime = Arc::clone(&runtime);
    let retry_kernel = Arc::clone(&kernel);
    crate::run_blocking_provider_operation(&gate, move || {
        save_credential(
            &retry_runtime,
            &retry_kernel,
            workspace_id,
            save_input(CREDENTIAL),
        )
    })
    .await
    .expect("retry save");

    {
        let values = vault.values.lock().expect("memory vault");
        assert_eq!(values.len(), 1);
        assert_eq!(
            values.get(tmdb_reference.as_str()).map(Vec::as_slice),
            Some(CREDENTIAL.as_bytes())
        );
    }
    assert_eq!(vault.write_count.load(Ordering::SeqCst), 2);
    let retry_version = assert_tmdb_states(
        &kernel,
        workspace_id,
        capability_count,
        ProviderCapabilityStatus::Available,
        ProviderCredentialStatus::StoredUnverified,
        Some(tmdb_reference.as_str()),
    );
    assert!(retry_version > cancelled_version);
}

#[tokio::test]
async fn cancelled_desktop_credential_delete_finishes_and_retry_is_idempotent() {
    let (_root, kernel) = new_kernel();
    let store = MemoryStore::default();
    complete_setup(&kernel, &store).expect("complete setup");
    let workspace_id = require_access(&kernel, &store)
        .expect("authenticated access")
        .workspace_id();
    let kernel = Arc::new(kernel);
    let vault = Arc::new(PausedMemoryVault::default());
    let runtime = Arc::new(ProviderRuntime::new(vault.clone()));
    let capability_count = runtime
        .descriptor("tmdb")
        .expect("TMDB descriptor")
        .capabilities
        .len();
    let tmdb_reference = runtime
        .credential_reference("tmdb")
        .expect("TMDB credential reference");
    let gate = Arc::new(tokio::sync::Mutex::new(()));

    let seed_runtime = Arc::clone(&runtime);
    let seed_kernel = Arc::clone(&kernel);
    crate::run_blocking_provider_operation(&gate, move || {
        save_credential(
            &seed_runtime,
            &seed_kernel,
            workspace_id,
            save_input("delete-cancellation-fixture"),
        )
    })
    .await
    .expect("seed provider credential");
    let seeded_version = assert_tmdb_states(
        &kernel,
        workspace_id,
        capability_count,
        ProviderCapabilityStatus::Available,
        ProviderCredentialStatus::StoredUnverified,
        Some(tmdb_reference.as_str()),
    );

    let (pause, entered, finish) = WritePause::new();
    *vault.revoke_pause.lock().expect("pause revoke") = Some(pause);
    let caller = {
        let operation_gate = Arc::clone(&gate);
        let operation_runtime = Arc::clone(&runtime);
        let operation_kernel = Arc::clone(&kernel);
        tokio::spawn(async move {
            crate::run_blocking_provider_operation(&operation_gate, move || {
                delete_credential(
                    &operation_runtime,
                    &operation_kernel,
                    workspace_id,
                    delete_input(),
                )
            })
            .await
        })
    };
    cancel_after_pause(caller, entered, finish, &gate).await;

    assert!(vault.values.lock().expect("memory vault").is_empty());
    assert_eq!(vault.revoke_count.load(Ordering::SeqCst), 1);
    let cancelled_version = assert_tmdb_states(
        &kernel,
        workspace_id,
        capability_count,
        ProviderCapabilityStatus::Unavailable,
        ProviderCredentialStatus::Missing,
        None,
    );
    assert!(cancelled_version > seeded_version);

    let retry_runtime = Arc::clone(&runtime);
    let retry_kernel = Arc::clone(&kernel);
    crate::run_blocking_provider_operation(&gate, move || {
        delete_credential(&retry_runtime, &retry_kernel, workspace_id, delete_input())
    })
    .await
    .expect("retry delete");

    assert!(vault.values.lock().expect("memory vault").is_empty());
    assert_eq!(vault.revoke_count.load(Ordering::SeqCst), 2);
    let retry_version = assert_tmdb_states(
        &kernel,
        workspace_id,
        capability_count,
        ProviderCapabilityStatus::Unavailable,
        ProviderCredentialStatus::Missing,
        None,
    );
    assert!(retry_version > cancelled_version);
}
