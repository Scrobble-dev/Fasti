use fasti_application::{
    AccessAdministrationPort, AuthenticateCredentialQuery, CapabilityKey, EnrollFirstClientCommand,
    FastiProblem, InitializeNodeCommand, ProblemCode, RequestAccessContext, SecretMaterial,
};
use fasti_domain::RequestCorrelationId;
use fasti_store::{DataRootIdentity, SqliteKernel};
use serde::Serialize;

pub(crate) const KEYRING_SERVICE: &str = "dev.scrobble.fasti.desktop";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SetupSecret {
    Proof,
    Credential,
}

impl SetupSecret {
    const fn label(self) -> &'static str {
        match self {
            Self::Proof => "local-bootstrap-proof-v1",
            Self::Credential => "local-admin-credential-v1",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DesktopProblem {
    code: &'static str,
    title: &'static str,
    detail: String,
    next_action: &'static str,
}

impl DesktopProblem {
    #[cfg(test)]
    pub(crate) fn code(&self) -> &'static str {
        self.code
    }

    pub(crate) fn secure_storage(detail: impl Into<String>) -> Self {
        Self {
            code: "secure_storage_unavailable",
            title: "Secure storage is unavailable",
            detail: detail.into(),
            next_action: "Unlock the system credential store, then retry setup.",
        }
    }

    fn recovery_required() -> Self {
        Self {
            code: "setup_recovery_required",
            title: "Setup needs recovery",
            detail: "The local node is initialized, but its saved administrator credential cannot resume setup.".to_owned(),
            next_action: "Keep the data root unchanged and use the recovery flow.",
        }
    }

    pub(crate) fn storage(detail: impl Into<String>) -> Self {
        Self {
            code: "storage_unavailable",
            title: "Local storage is unavailable",
            detail: detail.into(),
            next_action: "Check the Fasti data directory, then retry.",
        }
    }

    pub(crate) fn configuration(detail: impl Into<String>) -> Self {
        Self {
            code: "configuration_invalid",
            title: "Network configuration is invalid",
            detail: detail.into(),
            next_action: "Check the network settings, then retry.",
        }
    }

    pub(crate) fn invalid_input(detail: impl Into<String>) -> Self {
        Self {
            code: "invalid_input",
            title: "Invalid input",
            detail: detail.into(),
            next_action: "Correct the input and retry.",
        }
    }

    pub(crate) fn connection(detail: impl Into<String>) -> Self {
        Self {
            code: "connection_failed",
            title: "Connection failed",
            detail: detail.into(),
            next_action: "Check the address, network policy, and certificate trust, then retry.",
        }
    }

    pub(crate) fn provider(detail: impl Into<String>) -> Self {
        Self {
            code: "provider_unavailable",
            title: "Provider is unavailable",
            detail: detail.into(),
            next_action: "Check the provider settings and outbound policy, then retry.",
        }
    }

    pub(crate) fn provider_credential(detail: impl Into<String>) -> Self {
        Self {
            code: "provider.credential_invalid",
            title: "Provider credential is invalid",
            detail: detail.into(),
            next_action:
                "Replace or remove the provider credential. Fasti local data is unchanged.",
        }
    }

    pub(crate) fn not_authenticated() -> Self {
        Self {
            code: "not_authenticated",
            title: "Setup is not complete",
            detail: "This capability needs a saved administrator credential.".to_owned(),
            next_action: "Complete setup, then retry.",
        }
    }

    pub(crate) fn application(problem: &FastiProblem) -> Self {
        let contract = problem.code().contract();
        Self {
            code: problem.code().as_str(),
            title: contract.title(),
            detail: contract.detail(problem.capability()).into_owned(),
            next_action: contract.default_next_action().label(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SetupPhase {
    NeedsSetup,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct SetupStatus {
    phase: SetupPhase,
    proof_cleanup_pending: bool,
}

impl SetupStatus {
    const fn needs_setup() -> Self {
        Self {
            phase: SetupPhase::NeedsSetup,
            proof_cleanup_pending: false,
        }
    }

    const fn ready(proof_cleanup_pending: bool) -> Self {
        Self {
            phase: SetupPhase::Ready,
            proof_cleanup_pending,
        }
    }
}

pub(crate) trait SetupSecretStore: Send + Sync {
    fn load(&self, secret: SetupSecret) -> Result<Option<SecretMaterial>, DesktopProblem>;
    fn store(&self, secret: SetupSecret, value: &SecretMaterial) -> Result<(), DesktopProblem>;
    fn delete(&self, secret: SetupSecret) -> Result<(), DesktopProblem>;
}

pub(crate) struct KeyringSetupSecretStore {
    account_scope: String,
}

impl KeyringSetupSecretStore {
    pub(crate) fn new(identity: DataRootIdentity) -> Self {
        Self {
            account_scope: crate::secure_storage::account_scope(identity),
        }
    }

    fn account(&self, secret: SetupSecret) -> String {
        format!("{}-{}", secret.label(), self.account_scope)
    }

    fn entry(&self, secret: SetupSecret) -> Result<crate::secure_storage::Entry, DesktopProblem> {
        crate::secure_storage::Entry::new(KEYRING_SERVICE, &self.account(secret)).map_err(|_| {
            DesktopProblem::secure_storage("Fasti could not open the system credential store.")
        })
    }
}

impl SetupSecretStore for KeyringSetupSecretStore {
    fn load(&self, secret: SetupSecret) -> Result<Option<SecretMaterial>, DesktopProblem> {
        let entry = self.entry(secret)?;
        match entry.get_secret() {
            Ok(value) => {
                let bytes: [u8; 32] = value.try_into().map_err(|_| {
                    DesktopProblem::secure_storage(
                        "A saved Fasti setup secret has an invalid length and was not overwritten.",
                    )
                })?;
                Ok(Some(SecretMaterial::from_bytes(bytes)))
            }
            Err(crate::secure_storage::Error::NoEntry) => Ok(None),
            Err(_) => Err(DesktopProblem::secure_storage(
                "Fasti could not read the system credential store.",
            )),
        }
    }

    fn store(&self, secret: SetupSecret, value: &SecretMaterial) -> Result<(), DesktopProblem> {
        let entry = self.entry(secret)?;
        entry.set_secret(value.expose_bytes()).map_err(|_| {
            DesktopProblem::secure_storage("Fasti could not save setup secrets securely.")
        })?;
        let stored = self.load(secret)?.ok_or_else(|| {
            DesktopProblem::secure_storage("The system credential store did not retain a secret.")
        })?;
        if stored.expose_bytes() != value.expose_bytes() {
            return Err(DesktopProblem::secure_storage(
                "The system credential store did not return the saved secret.",
            ));
        }
        Ok(())
    }

    fn delete(&self, secret: SetupSecret) -> Result<(), DesktopProblem> {
        let entry = self.entry(secret)?;
        match entry.delete_credential() {
            Ok(()) | Err(crate::secure_storage::Error::NoEntry) => Ok(()),
            Err(_) => Err(DesktopProblem::secure_storage(
                "Fasti could not remove the consumed setup proof.",
            )),
        }
    }
}

pub(crate) fn authenticate(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<Option<RequestAccessContext>, DesktopProblem> {
    let Some(credential) = store.load(SetupSecret::Credential)? else {
        return Ok(None);
    };
    match kernel.authenticate_credential(AuthenticateCredentialQuery::new(
        RequestCorrelationId::new_v7(),
        CapabilityKey::InspectReview,
        credential,
    )) {
        Ok(access) => Ok(Some(access)),
        Err(problem) if problem.code() == ProblemCode::AuthenticationFailed => Ok(None),
        Err(problem) => Err(DesktopProblem::application(&problem)),
    }
}

fn ready_status(store: &impl SetupSecretStore) -> SetupStatus {
    SetupStatus::ready(store.delete(SetupSecret::Proof).is_err())
}

pub(crate) fn inspect_setup(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<SetupStatus, DesktopProblem> {
    Ok(match authenticate(kernel, store)? {
        Some(_) => ready_status(store),
        None => SetupStatus::needs_setup(),
    })
}

pub(crate) fn complete_setup(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<SetupStatus, DesktopProblem> {
    if authenticate(kernel, store)?.is_some() {
        return Ok(ready_status(store));
    }

    let proof = match store.load(SetupSecret::Proof)? {
        Some(proof) => proof,
        None => {
            match kernel.initialize_node(InitializeNodeCommand::new(RequestCorrelationId::new_v7()))
            {
                Ok(outcome) => {
                    let proof = outcome.initialization_proof();
                    store.store(SetupSecret::Proof, proof)?;
                    SecretMaterial::from_bytes(*proof.expose_bytes())
                }
                Err(problem) if problem.code() == ProblemCode::AlreadyInitialized => {
                    return Err(DesktopProblem::recovery_required());
                }
                Err(problem) => return Err(DesktopProblem::application(&problem)),
            }
        }
    };

    match kernel.enroll_first_client(EnrollFirstClientCommand::new(
        RequestCorrelationId::new_v7(),
        proof,
    )) {
        Ok(outcome) => {
            store.store(SetupSecret::Credential, outcome.credential())?;
            let _ = store.delete(SetupSecret::Proof);
        }
        Err(problem) if problem.code() == ProblemCode::BootstrapClosed => {}
        Err(problem) => return Err(DesktopProblem::application(&problem)),
    }

    if authenticate(kernel, store)?.is_none() {
        return Err(DesktopProblem::recovery_required());
    }
    Ok(ready_status(store))
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::{DesktopProblem, SecretMaterial, SetupSecret, SetupSecretStore};
    use fasti_store::SqliteKernel;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub(crate) struct MemoryStore(Mutex<BTreeMap<SetupSecret, [u8; 32]>>);

    impl SetupSecretStore for MemoryStore {
        fn load(&self, secret: SetupSecret) -> Result<Option<SecretMaterial>, DesktopProblem> {
            Ok(self
                .0
                .lock()
                .expect("memory store")
                .get(&secret)
                .copied()
                .map(SecretMaterial::from_bytes))
        }

        fn store(&self, secret: SetupSecret, value: &SecretMaterial) -> Result<(), DesktopProblem> {
            self.0
                .lock()
                .expect("memory store")
                .insert(secret, *value.expose_bytes());
            Ok(())
        }

        fn delete(&self, secret: SetupSecret) -> Result<(), DesktopProblem> {
            self.0.lock().expect("memory store").remove(&secret);
            Ok(())
        }
    }

    pub(crate) fn new_kernel() -> (tempfile::TempDir, SqliteKernel) {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
        (root, kernel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::{new_kernel, MemoryStore};

    #[test]
    fn setup_initializes_and_enrolls_cleanly() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();

        assert_eq!(
            complete_setup(&kernel, &store).expect("complete setup"),
            SetupStatus::ready(false)
        );
        assert!(store
            .load(SetupSecret::Credential)
            .expect("credential lookup")
            .is_some());
        assert_eq!(
            inspect_setup(&kernel, &store).expect("ready status"),
            SetupStatus::ready(false)
        );
    }

    #[test]
    fn setup_resumes_from_saved_proof_after_initialization() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();

        let outcome = kernel
            .initialize_node(InitializeNodeCommand::new(RequestCorrelationId::new_v7()))
            .expect("initialize node");
        store
            .store(SetupSecret::Proof, outcome.initialization_proof())
            .expect("persist proof before enrollment");

        assert_eq!(
            complete_setup(&kernel, &store).expect("resume enrollment from proof"),
            SetupStatus::ready(false)
        );
        assert_eq!(
            inspect_setup(&kernel, &store).expect("ready status"),
            SetupStatus::ready(false)
        );
    }

    #[test]
    fn keyring_accounts_are_scoped_to_the_data_root() {
        let (_root_a, kernel_a) = new_kernel();
        let (_root_b, kernel_b) = new_kernel();
        let store_a = KeyringSetupSecretStore::new(kernel_a.data_root_identity());
        let store_b = KeyringSetupSecretStore::new(kernel_b.data_root_identity());

        assert_eq!(
            store_a.account(SetupSecret::Credential),
            format!(
                "local-admin-credential-v1-{}",
                crate::secure_storage::account_scope(kernel_a.data_root_identity())
            )
        );

        assert_ne!(
            store_a.account(SetupSecret::Credential),
            store_b.account(SetupSecret::Credential)
        );
        assert_ne!(
            store_a.account(SetupSecret::Proof),
            store_a.account(SetupSecret::Credential)
        );
    }
}
