mod provider_kitsu_tests {
    use super::*;

    #[derive(Default)]
    struct RejectingVault(AtomicUsize);

    impl CredentialVaultPort for RejectingVault {
        fn source(
            &self,
            _reference: &CredentialReference,
        ) -> Result<CredentialVaultSource, CredentialVaultError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(CredentialVaultError::Rejected)
        }

        fn store(
            &self,
            _reference: &CredentialReference,
            _secret: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(CredentialVaultError::Rejected)
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
            _reference: &CredentialReference,
        ) -> Result<CredentialSecret, CredentialVaultError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(CredentialVaultError::Rejected)
        }

        fn revoke(&self, _reference: &CredentialReference) -> Result<(), CredentialVaultError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(CredentialVaultError::Rejected)
        }
    }

    #[test]
    fn kitsu_initial_and_retrieved_state_need_no_vault_or_credential_test() {
        let (_root, kernel, workspace_id, _credential) = enrolled_kernel();
        let vault = Arc::new(RejectingVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let provider = runtime.descriptor("kitsu").expect("Kitsu descriptor");
        assert_eq!(provider.capabilities.len(), 2);
        for capability_id in ["metadata.search", "metadata.read"] {
            let spec = provider
                .capabilities
                .iter()
                .find(|spec| spec.capability_id == capability_id)
                .expect("Kitsu capability");
            let initial = initial_state(&runtime, provider, spec).expect("credential-free state");
            assert_eq!(
                initial.credential_requirement(),
                CredentialRequirement::None
            );
            assert_eq!(
                initial.credential_status(),
                ProviderCredentialStatus::NotRequired
            );
            assert_eq!(
                initial.capability_status(),
                ProviderCapabilityStatus::Available
            );
            assert!(initial.credential_reference().is_none());
            assert_eq!(
                initial.configuration_digest(),
                &runtime
                    .configuration_digest("kitsu", capability_id)
                    .expect("digest")
            );
            // Building an inventory view must not activate persistent Search authority.
            assert!(kernel
                .get_provider_capability_state(
                    workspace_id,
                    initial.provider_id(),
                    initial.capability_id()
                )
                .expect("uninitialized state")
                .is_none());
            kernel
                .put_provider_capability_state(workspace_id, initial.clone())
                .expect("persist through existing state owner");
            let retrieved = kernel
                .get_provider_capability_state(
                    workspace_id,
                    initial.provider_id(),
                    initial.capability_id(),
                )
                .expect("retrieve state")
                .expect("persisted state");
            assert_eq!(retrieved, initial);
            for state in [&initial, &retrieved] {
                let dto = capability_dto(&runtime, true, spec, Some(state));
                assert_eq!(dto.credential_requirement, CredentialRequirementDto::None);
                assert_eq!(
                    dto.credential_state,
                    ProviderCredentialStateDto::NotRequired
                );
                assert_eq!(dto.credential_source, ProviderCredentialSourceDto::None);
                assert_eq!(dto.state, ProviderCapabilityStateDto::Available);
                assert!(!dto.writable);
                assert!(!dto.testable);
                assert!(dto.health_checkable);
                assert_eq!(dto.health.state, ProviderCheckStateDto::NeverRun);
            }
        }
        assert_eq!(
            vault.0.load(Ordering::SeqCst),
            0,
            "no vault method is needed"
        );
    }

    #[tokio::test]
    #[ignore = "explicit public Kitsu health requests; real governed transport, no provider credentials"]
    async fn kitsu_live_health_route_initializes_both_capabilities_and_survives_reopen() {
        let (root, kernel, workspace_id, credential) = enrolled_kernel();
        let vault = Arc::new(RejectingVault::default());
        let runtime = Arc::new(ProviderRuntime::new(vault.clone()));
        assert!(kernel
            .list_provider_capability_states(workspace_id)
            .expect("initial provider states")
            .is_empty());
        let app = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel: kernel.clone(),
            provider_state: kernel.clone(),
            provider_operation_locks: ProviderOperationLocks::new(&runtime),
            runtime: runtime.clone(),
        });
        let response = app
            .oneshot(
                Request::get("/api/v1/providers/kitsu/health")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("health request"),
            )
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 16 * 1024)
            .await
            .expect("bounded health response");
        let response: ProviderHealthResponse =
            serde_json::from_slice(&body).expect("health response JSON");
        assert_eq!(response.provider_id, "kitsu");
        assert_eq!(response.capabilities.len(), 2);
        let expected = kernel
            .list_provider_capability_states(workspace_id)
            .expect("initialized states");
        assert_eq!(expected.len(), 2);
        for capability_id in ["metadata.search", "metadata.read"] {
            let state = expected
                .iter()
                .find(|state| state.capability_id().as_str() == capability_id)
                .expect("initialized capability");
            assert_eq!(state.provider_id().as_str(), "kitsu");
            assert_eq!(
                state.capability_status(),
                ProviderCapabilityStatus::Available
            );
            assert_eq!(state.credential_requirement(), CredentialRequirement::None);
            assert_eq!(
                state.credential_status(),
                ProviderCredentialStatus::NotRequired
            );
            assert!(state.credential_reference().is_none());
            assert_eq!(state.health().status(), ProviderCheckStatus::Passed);
            assert!(state.health().checked_at().is_some());
            assert_eq!(state.health().safe_problem_code(), None);
            assert_eq!(state.credential_test(), &ProviderCheckMetadata::never_run());
            assert_eq!(
                state.configuration_digest(),
                &runtime
                    .configuration_digest("kitsu", capability_id)
                    .expect("digest")
            );
            let dto = response
                .capabilities
                .iter()
                .find(|dto| dto.capability_id == capability_id)
                .expect("returned capability");
            assert_eq!(dto.state, ProviderCapabilityStateDto::Available);
            assert_eq!(
                dto.credential_state,
                ProviderCredentialStateDto::NotRequired
            );
            assert_eq!(dto.health.state, ProviderCheckStateDto::Passed);
            assert_eq!(dto.version, state.capability_version());
            assert!(!dto.testable);
            assert!(dto.health_checkable);
        }
        // Close the last handle before reopening, rather than inspecting a shared connection.
        drop(Arc::try_unwrap(kernel).unwrap_or_else(|_| panic!("health released kernel handles")));
        let reopened = SqliteKernel::open(root.path()).expect("reopen SQLite kernel");
        for state in expected {
            assert_eq!(
                reopened
                    .get_provider_capability_state(
                        workspace_id,
                        state.provider_id(),
                        state.capability_id()
                    )
                    .expect("read reopened state"),
                Some(state)
            );
        }
        assert_eq!(
            vault.0.load(Ordering::SeqCst),
            0,
            "public health never accesses the vault"
        );
    }

    #[tokio::test]
    async fn kitsu_disabled_health_route_preserves_operator_disable_without_vault_access() {
        let (_root, kernel, workspace_id, credential) = enrolled_kernel();
        let vault = Arc::new(RejectingVault::default());
        let runtime = Arc::new(ProviderRuntime::new(vault.clone()));
        let provider = runtime.descriptor("kitsu").expect("Kitsu descriptor");
        let mut before = Vec::new();
        for spec in provider.capabilities {
            let initial = initial_state(&runtime, provider, spec).expect("initial state");
            let disabled = next_state(
                &initial,
                ProviderCapabilityStatus::Disabled,
                None,
                ProviderCredentialStatus::NotRequired,
                initial.configuration_digest().clone(),
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .expect("disabled state");
            kernel
                .put_provider_capability_state(workspace_id, disabled.clone())
                .expect("persist operator disable");
            before.push(disabled);
        }
        let app = router().with_state(ProviderApiState {
            browser_boundary: None,
            kernel: kernel.clone(),
            provider_state: kernel.clone(),
            provider_operation_locks: ProviderOperationLocks::new(&runtime),
            runtime,
        });
        let response = app
            .oneshot(
                Request::get("/api/v1/providers/kitsu/health")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("health request"),
            )
            .await
            .expect("health response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let states = kernel
            .list_provider_capability_states(workspace_id)
            .expect("states");
        assert_eq!(states.len(), 2);
        for (index, previous) in before.iter().enumerate() {
            let current = states
                .iter()
                .find(|state| state.capability_id() == previous.capability_id())
                .expect("retained capability");
            assert_eq!(
                current.capability_status(),
                ProviderCapabilityStatus::Disabled
            );
            assert_eq!(
                current.credential_status(),
                ProviderCredentialStatus::NotRequired
            );
            assert!(current.credential_reference().is_none());
            assert_eq!(
                current.configuration_digest(),
                previous.configuration_digest()
            );
            assert_eq!(current.credential_test(), previous.credential_test());
            if index == 0 {
                // The first disabled capability rejects before egress; the route stops there.
                assert_eq!(
                    current.capability_version(),
                    previous.capability_version() + 1
                );
                assert_eq!(current.health().status(), ProviderCheckStatus::Unavailable);
                assert_eq!(
                    current.health().safe_problem_code(),
                    Some(ProblemCode::ProviderUnavailable)
                );
                assert!(current.health().checked_at().is_some());
            } else {
                assert_eq!(current, previous);
            }
        }
        assert_eq!(
            vault.0.load(Ordering::SeqCst),
            0,
            "disabled health never accesses the vault"
        );
    }
}
