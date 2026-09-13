mod provider_kitsu_tests {
    use super::*;
    use fasti_application::{
        CredentialReference, CredentialVaultError, CredentialVaultPort, StoredCredential,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct UnusedVault {
        sources: AtomicUsize,
        loads: AtomicUsize,
    }

    impl UnusedVault {
        fn assert_unused(&self) {
            assert_eq!(self.sources.load(Ordering::SeqCst), 0);
            assert_eq!(self.loads.load(Ordering::SeqCst), 0);
        }
    }

    impl CredentialVaultPort for UnusedVault {
        fn source(
            &self,
            _: &CredentialReference,
        ) -> Result<CredentialVaultSource, CredentialVaultError> {
            self.sources.fetch_add(1, Ordering::SeqCst);
            Err(CredentialVaultError::Unavailable)
        }

        fn load(&self, _: &CredentialReference) -> Result<CredentialSecret, CredentialVaultError> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            Err(CredentialVaultError::Unavailable)
        }

        fn store(
            &self,
            _: &CredentialReference,
            _: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            panic!("credential-free reconciliation must not store a secret")
        }

        fn replace(
            &self,
            _: &CredentialReference,
            _: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            panic!("credential-free reconciliation must not replace a secret")
        }

        fn revoke(&self, _: &CredentialReference) -> Result<(), CredentialVaultError> {
            panic!("credential-free reconciliation must not revoke a secret")
        }
    }

    #[test]
    fn kitsu_native_reconciliation_creates_both_public_capabilities_without_vault_access() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");
        let workspace_id = require_access(&kernel, &store)
            .expect("access")
            .workspace_id();
        let vault = Arc::new(UnusedVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let spec = runtime.descriptor("kitsu").expect("Kitsu descriptor");
        assert!(spec.runtime_available);

        for capability_id in [SEARCH_CAPABILITY, READ_CAPABILITY] {
            let provider = ProviderId::try_new("kitsu").unwrap();
            let capability = ProviderCapabilityId::try_new(capability_id).unwrap();
            assert!(kernel
                .get_provider_capability_state(workspace_id, &provider, &capability)
                .unwrap()
                .is_none());
            let (state, source) =
                reconcile_state(&runtime, &kernel, workspace_id, spec, capability_id)
                    .expect("reconcile public capability");
            assert_eq!(source, CredentialVaultSource::None);
            assert_eq!(state.credential_requirement(), CredentialRequirement::None);
            assert_eq!(
                state.credential_status(),
                ProviderCredentialStatus::NotRequired
            );
            assert_eq!(
                state.capability_status(),
                ProviderCapabilityStatus::Available
            );
            assert!(state.credential_reference().is_none());
            assert_eq!(
                kernel
                    .get_provider_capability_state(workspace_id, &provider, &capability)
                    .unwrap(),
                Some(state.clone())
            );

            let view = status_view(spec, capability_id, &state, source);
            assert_eq!(view.provider, "kitsu");
            assert_eq!(view.capability_id, capability_id);
            assert_eq!(view.credential_requirement, CredentialRequirement::None);
            assert_eq!(view.credential_state, ProviderCredentialStatus::NotRequired);
            assert_eq!(view.state, ProviderCapabilityStatus::Available);
            assert_eq!(view.source, CredentialVaultSourceView::None);
            assert!(!view.writable);
            assert!(!view.testable);
            assert!(view.health_checkable);

            let (unchanged, _) =
                reconcile_state(&runtime, &kernel, workspace_id, spec, capability_id)
                    .expect("repeat reconciliation");
            assert_eq!(
                unchanged, state,
                "an inventory read must not advance the generation"
            );
        }
        vault.assert_unused();
    }

    #[test]
    fn kitsu_health_failure_and_recovery_survive_repeated_reconciliation() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");
        let workspace_id = require_access(&kernel, &store)
            .expect("access")
            .workspace_id();
        let vault = Arc::new(UnusedVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let spec = runtime.descriptor("kitsu").expect("Kitsu descriptor");

        for capability_id in [SEARCH_CAPABILITY, READ_CAPABILITY] {
            let (initial, _) =
                reconcile_state(&runtime, &kernel, workspace_id, spec, capability_id)
                    .expect("create public capability");
            assert_eq!(
                initial.capability_status(),
                ProviderCapabilityStatus::Available
            );
            record_check_result(
                &kernel,
                workspace_id,
                &initial,
                ProviderCheckKind::Health,
                &Err(ProviderRuntimeError::network(
                    "fixture upstream unavailable",
                )),
            )
            .expect("persist failed health check");
            let failed = kernel
                .get_provider_capability_state(
                    workspace_id,
                    initial.provider_id(),
                    initial.capability_id(),
                )
                .unwrap()
                .expect("stored failed check");
            assert_eq!(
                failed.capability_status(),
                ProviderCapabilityStatus::Degraded
            );
            assert_eq!(failed.health().status(), ProviderCheckStatus::Failed);
            assert_eq!(
                failed.health().safe_problem_code(),
                Some(ProblemCode::ProviderUnavailable)
            );
            assert_eq!(
                failed.capability_version(),
                initial.capability_version() + 1
            );
            assert_eq!(
                failed.credential_status(),
                ProviderCredentialStatus::NotRequired
            );
            assert!(failed.credential_reference().is_none());
            assert_eq!(failed.credential_test(), initial.credential_test());
            for _ in 0..2 {
                let (reconciled, source) =
                    reconcile_state(&runtime, &kernel, workspace_id, spec, capability_id)
                        .expect("reconcile degraded capability");
                assert_eq!(source, CredentialVaultSource::None);
                assert_eq!(
                    reconciled, failed,
                    "inventory must not erase a failed check or change its version"
                );
                assert_eq!(
                    kernel
                        .get_provider_capability_state(
                            workspace_id,
                            initial.provider_id(),
                            initial.capability_id()
                        )
                        .unwrap(),
                    Some(failed.clone())
                );
            }

            record_check_result(
                &kernel,
                workspace_id,
                &failed,
                ProviderCheckKind::Health,
                &Ok(()),
            )
            .expect("persist successful recovery");
            let recovered = kernel
                .get_provider_capability_state(
                    workspace_id,
                    initial.provider_id(),
                    initial.capability_id(),
                )
                .unwrap()
                .expect("stored recovery");
            assert_eq!(
                recovered.capability_status(),
                ProviderCapabilityStatus::Available
            );
            assert_eq!(recovered.health().status(), ProviderCheckStatus::Passed);
            assert_eq!(recovered.health().safe_problem_code(), None);
            assert_eq!(
                recovered.capability_version(),
                failed.capability_version() + 1
            );
            assert_eq!(
                recovered.credential_status(),
                ProviderCredentialStatus::NotRequired
            );
            assert!(recovered.credential_reference().is_none());
            assert_eq!(recovered.credential_test(), initial.credential_test());
            for _ in 0..2 {
                let (reconciled, source) =
                    reconcile_state(&runtime, &kernel, workspace_id, spec, capability_id)
                        .expect("reconcile recovered capability");
                assert_eq!(source, CredentialVaultSource::None);
                assert_eq!(
                    reconciled, recovered,
                    "inventory must preserve the complete successful check"
                );
                assert_eq!(
                    kernel
                        .get_provider_capability_state(
                            workspace_id,
                            initial.provider_id(),
                            initial.capability_id()
                        )
                        .unwrap(),
                    Some(recovered.clone())
                );
            }
        }
        vault.assert_unused();
    }

    #[test]
    fn kitsu_disabled_capabilities_survive_reconciliation_and_failed_health_checks() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");
        let workspace_id = require_access(&kernel, &store)
            .expect("access")
            .workspace_id();
        let vault = Arc::new(UnusedVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let spec = runtime.descriptor("kitsu").expect("Kitsu descriptor");

        for capability_id in [SEARCH_CAPABILITY, READ_CAPABILITY] {
            let disabled = ProviderCapabilityState::try_new(
                ProviderId::try_new("kitsu").unwrap(),
                ProviderCapabilityId::try_new(capability_id).unwrap(),
                ProviderCapabilityStatus::Disabled,
                7,
                CredentialRequirement::None,
                None,
                ProviderCredentialStatus::NotRequired,
                runtime
                    .configuration_digest("kitsu", capability_id)
                    .unwrap(),
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .unwrap();
            kernel
                .put_provider_capability_state(workspace_id, disabled.clone())
                .expect("seed disabled state");
            let (state, source) =
                reconcile_state(&runtime, &kernel, workspace_id, spec, capability_id)
                    .expect("reconcile disabled capability");
            assert_eq!(state, disabled);
            assert_eq!(source, CredentialVaultSource::None);
            assert_eq!(
                status_view(spec, capability_id, &state, source).state,
                ProviderCapabilityStatus::Disabled
            );

            record_check_result(
                &kernel,
                workspace_id,
                &state,
                ProviderCheckKind::Health,
                &Err(ProviderRuntimeError::network(
                    "fixture upstream unavailable",
                )),
            )
            .expect("persist failed health check");
            let updated = kernel
                .get_provider_capability_state(
                    workspace_id,
                    state.provider_id(),
                    state.capability_id(),
                )
                .unwrap()
                .expect("persisted health result");
            assert_eq!(
                updated.capability_status(),
                ProviderCapabilityStatus::Disabled
            );
            assert_eq!(
                updated.credential_requirement(),
                CredentialRequirement::None
            );
            assert_eq!(
                updated.credential_status(),
                ProviderCredentialStatus::NotRequired
            );
            assert!(updated.credential_reference().is_none());
            assert_eq!(updated.capability_version(), state.capability_version() + 1);
            assert_eq!(updated.health().status(), ProviderCheckStatus::Failed);
            assert_eq!(
                updated.health().safe_problem_code(),
                Some(ProblemCode::ProviderUnavailable)
            );
            assert_eq!(updated.credential_test(), state.credential_test());

            let (after_failure, _) =
                reconcile_state(&runtime, &kernel, workspace_id, spec, capability_id)
                    .expect("reconcile after failed check");
            assert_eq!(
                after_failure, updated,
                "reconciliation must preserve both disablement and failure evidence"
            );
        }
        vault.assert_unused();
    }
}
