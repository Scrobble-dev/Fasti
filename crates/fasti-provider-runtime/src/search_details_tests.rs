mod candidate_details_tests {
    use super::*;
    use std::sync::atomic::AtomicU8;

    const CURRENT: u8 = 0;
    const DENIED: u8 = 1;
    const EXPIRED: u8 = 2;
    const CHANGED_AUTHORITY: u8 = 3;
    const CHANGED_SNAPSHOT: u8 = 4;

    struct DetailsPersistence {
        prepared: PreparedSearchCandidateDetails,
        disposition: AtomicU8,
        calls: Mutex<Vec<&'static str>>,
        pause_prepare: Mutex<
            Option<(
                tokio::sync::oneshot::Sender<()>,
                std::sync::mpsc::Receiver<()>,
            )>,
        >,
    }

    impl DetailsPersistence {
        fn check_request(&self, request: &ReadSearchCandidateRequest) {
            assert_eq!(request.terms_revision, "fasti.public-metadata-cache.v1");
            assert_eq!(
                request.candidate_receipt_id,
                self.prepared.candidate.receipt.id()
            );
            assert_eq!(
                request.provider.as_str(),
                self.prepared.candidate.context.provider()
            );
            assert_eq!(request.grain, Grain::Film);
        }

        fn current(
            &self,
            request: &ReadSearchCandidateRequest,
        ) -> ApplicationResult<Option<PreparedSearchCandidateDetails>> {
            self.check_request(request);
            let mut prepared = self.prepared.clone();
            match self.disposition.load(Ordering::SeqCst) {
                DENIED => {
                    return Err(Box::new(FastiProblem::forbidden(
                        CapabilityKey::SearchMetadata,
                        request.correlation_id,
                    )))
                }
                EXPIRED => return Ok(None),
                CHANGED_AUTHORITY => {
                    prepared.provider_authority_fingerprint = Sha256Digest::from_bytes(&[9; 32]);
                }
                CHANGED_SNAPSHOT => {
                    let receipt = &prepared.candidate.receipt;
                    prepared.candidate.receipt = SearchCandidateReceipt::new(
                        receipt.id(),
                        receipt.partition().clone(),
                        receipt.candidate().clone(),
                        Sha256Digest::from_bytes(&[8; 32]),
                        receipt.lifetime().clone(),
                    );
                }
                CURRENT => {}
                other => panic!("unexpected fixture disposition {other}"),
            }
            Ok(Some(prepared))
        }
    }

    impl SearchPersistencePort for DetailsPersistence {
        fn prepare_search_candidate_action(&self, _: &fasti_application::SearchCandidateActionCommand) -> ApplicationResult<fasti_application::SearchCandidateActionPreparation> {
            panic!("detail reads must not prepare actions")
        }
        fn commit_search_candidate_action(&self, _: &fasti_application::SearchCandidateActionCommand, _: &fasti_application::SearchCandidateActionPreparation, _: Option<&[fasti_application::ProviderMetadataField]>) -> ApplicationResult<fasti_application::SearchCandidateActionReceipt> {
            panic!("detail reads must not commit actions")
        }
        fn search_local_records(
            &self,
            _: &LocalSearchRequest,
        ) -> ApplicationResult<LocalSearchPage> {
            panic!("candidate details must not query local Records")
        }

        fn prepare_search_page(
            &self,
            _: &SearchPageRequest,
        ) -> ApplicationResult<PreparedSearchPage> {
            panic!("candidate details must not prepare a new Search page")
        }

        fn commit_search_page(
            &self,
            _: &SearchPageRequest,
            _: &PreparedSearchPage,
            _: &[SearchCandidate],
            _: &Sha256Digest,
            _: Option<u32>,
        ) -> ApplicationResult<StoredSearchPage> {
            panic!("candidate details must never replace the immutable Search page")
        }

        fn read_cached_search_page(
            &self,
            _: &SearchPageRequest,
            _: bool,
        ) -> ApplicationResult<Option<StoredSearchPage>> {
            panic!("candidate receipt lifetime must not be replaced by page-cache lifetime")
        }

        fn read_search_candidate(
            &self,
            request: &ReadSearchCandidateRequest,
        ) -> ApplicationResult<Option<StoredSearchCandidate>> {
            self.calls.lock().unwrap().push("snapshot");
            self.current(request)
                .map(|prepared| prepared.map(|value| value.candidate))
        }

        fn prepare_search_candidate_details(
            &self,
            request: &ReadSearchCandidateRequest,
        ) -> ApplicationResult<Option<PreparedSearchCandidateDetails>> {
            self.calls.lock().unwrap().push("prepare-details");
            let pause = self.pause_prepare.lock().unwrap().take();
            if let Some((entered, release)) = pause {
                let _ = entered.send(());
                release
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .expect("blocked preparation must be released within the test bound");
            }
            self.current(request)
        }
    }

    fn fixture() -> (
        ProviderSearchService,
        Arc<DetailsPersistence>,
        ReadSearchCandidateRequest,
    ) {
        let (service, _, page_request) = setup(None, None);
        let ApplicationAccessContext::Credential(access) = &page_request.access else {
            panic!("existing fixture uses a credential")
        };
        // Include historical region evidence to prove details never promote it
        // to an unsupported upstream response-region parameter.
        let context = SearchProviderQuery::try_new(
            page_request.query.query().clone(),
            page_request.query.provider().clone(),
            1,
            Some(MetadataLocale::try_new("fr-FR").unwrap()),
            Some(MetadataRegion::try_new("FR").unwrap()),
            vec![Grain::Film],
        )
        .unwrap()
        .receipt_context();
        let partition = SearchReceiptPartition::try_new(
            AuthorizedApplicationAccess::new(
                access.workspace_id(),
                access.profile_id(),
                access.grant_id(),
                AuthorizedActor::Credential {
                    presented_client_id: access.client_id(),
                    credential_id: access.credential_id(),
                },
            ),
            context.digest(),
            Sha256Digest::from_bytes(&[1; 32]),
            Sha256Digest::from_bytes(&[2; 32]),
            "fasti.public-metadata-cache.v1".into(),
        )
        .unwrap();
        let page = page();
        let receipt = SearchCandidateReceipt::new(
            SearchCandidateReceiptId::new_v7(),
            partition,
            candidate().search_evidence().unwrap(),
            page.response_digest,
            page.lifetime,
        );
        let request = ReadSearchCandidateRequest {
            correlation_id: page_request.correlation_id,
            access: page_request.access,
            candidate_receipt_id: receipt.id(),
            provider: page_request.query.provider().clone(),
            grain: Grain::Film,
            outbound_policy: page_request.outbound_policy,
            terms_revision: "caller-selected-old-policy".into(),
        };
        let persistence = Arc::new(DetailsPersistence {
            prepared: PreparedSearchCandidateDetails {
                candidate: StoredSearchCandidate { receipt, context },
                provider_state: ProviderCapabilityState::try_new(
                    ProviderId::try_new("tmdb").unwrap(),
                    ProviderCapabilityId::try_new("metadata.read").unwrap(),
                    ProviderCapabilityStatus::Available,
                    1,
                    CredentialRequirement::BearerToken,
                    Some(CredentialReference::try_new("secret:fixture").unwrap()),
                    ProviderCredentialStatus::StoredUnverified,
                    ConfigurationDigest::parse("ab".repeat(32)).unwrap(),
                    ProviderCheckMetadata::never_run(),
                    ProviderCheckMetadata::never_run(),
                )
                .unwrap(),
                provider_authority_fingerprint: Sha256Digest::from_bytes(&[3; 32]),
            },
            disposition: AtomicU8::new(CURRENT),
            calls: Mutex::new(vec![]),
            pause_prepare: Mutex::new(None),
        });
        (
            ProviderSearchService::new(service.runtime, persistence.clone()),
            persistence,
            request,
        )
    }

    fn candidate() -> ProviderCandidate {
        crate::providers::search_page_fixture().candidates.remove(0)
    }

    #[tokio::test]
    async fn public_offline_details_use_only_authorized_snapshot_and_trusted_policy() {
        for revision in ["", "caller-selected", "tmdb_attribution_required"] {
            let (service, persistence, mut request) = fixture();
            request.terms_revision = revision.into();
            let expected = persistence.prepared.candidate.clone();
            let result = service
                .candidate_details(request, true, lease().await)
                .await
                .unwrap();
            assert_eq!(
                result,
                Some(ProviderCandidateDetailsOutcome::Snapshot(expected))
            );
            assert_eq!(*persistence.calls.lock().unwrap(), ["snapshot"]);
        }
    }

    #[tokio::test]
    async fn details_missing_or_denied_before_fetch_never_call_provider() {
        for offline in [false, true] {
            for disposition in [EXPIRED, DENIED] {
                let (service, persistence, request) = fixture();
                persistence.disposition.store(disposition, Ordering::SeqCst);
                let result = service
                    .candidate_details_with(request, offline, lease().await, |_, _| async {
                        panic!("missing or unauthorized candidate must not fetch")
                    })
                    .await;
                if disposition == EXPIRED {
                    assert_eq!(result.unwrap(), None);
                } else {
                    assert_eq!(result.unwrap_err().code(), ProblemCode::Forbidden);
                }
                assert_eq!(
                    *persistence.calls.lock().unwrap(),
                    [if offline {
                        "snapshot"
                    } else {
                        "prepare-details"
                    }]
                );
            }
        }
    }

    #[tokio::test]
    async fn details_fetch_uses_stored_coordinates_and_preserves_original_snapshot() {
        let (service, persistence, request) = fixture();
        let original = persistence.prepared.candidate.clone();
        let expected_state = persistence.prepared.provider_state.clone();
        let mut fetched = candidate();
        fetched.title = "Fresh French title".into();
        let expected = fetched.clone();
        let result = service
            .candidate_details_with(
                request,
                false,
                lease().await,
                move |selection, state| async move {
                    assert_eq!(
                        selection,
                        ProviderSelectionInput {
                            provider: "tmdb".into(),
                            provider_id: "42".into(),
                            kind: "movie".into(),
                            locale: Some("fr-fr".into()),
                            region: None,
                        }
                    );
                    assert_eq!(state, expected_state);
                    Ok(fetched)
                },
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result,
            ProviderCandidateDetailsOutcome::Refetched {
                snapshot: original.clone(),
                details: Box::new(expected),
                locale: Some(MetadataLocale::try_new("fr-FR").unwrap()),
            }
        );
        assert_eq!(persistence.prepared.candidate, original);
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare-details", "prepare-details"]
        );
    }

    #[tokio::test]
    async fn details_reject_wrong_identity_grain_and_unbounded_fields_after_recheck() {
        let changes: [fn(&mut ProviderCandidate); 9] = [
            |value| value.provider_id = "43".into(),
            |value| value.kind = "show",
            |value| value.provider = "google-books",
            |value| value.title = "x".repeat(513),
            |value| value.original_title = Some("x".repeat(513)),
            |value| value.overview = Some("x".repeat(4097)),
            |value| value.authors = vec!["author".into(); 11],
            |value| value.authors = vec!["x".repeat(129)],
            |value| value.image_url = Some("https://evil.example/poster.jpg".into()),
        ];
        for change in changes {
            let (service, persistence, request) = fixture();
            let mut details = candidate();
            change(&mut details);
            let result =
                service
                    .candidate_details_with(request, false, lease().await, |_, _| async move {
                        Ok(details)
                    })
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(
                result,
                ProviderCandidateDetailsOutcome::Unavailable {
                    snapshot: persistence.prepared.candidate.clone(),
                    problem: ProblemCode::ProviderResponseInvalid,
                }
            );
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare-details", "prepare-details"]
            );
        }
    }

    #[tokio::test]
    async fn provider_details_failures_are_exposed_only_after_authority_recheck() {
        for error in [
            ProviderRuntimeError::network("fixture network failure"),
            ProviderRuntimeError::provider("fixture provider outage"),
            ProviderRuntimeError::rate_limited("fixture rate limit"),
            ProviderRuntimeError::credential("fixture rejected credential"),
            ProviderRuntimeError::response_invalid("fixture malformed response"),
        ] {
            let (service, persistence, request) = fixture();
            let problem = error.problem_code();
            let result =
                service
                    .candidate_details_with(request, false, lease().await, |_, _| async move {
                        Err(error)
                    })
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(
                result,
                ProviderCandidateDetailsOutcome::Unavailable {
                    snapshot: persistence.prepared.candidate.clone(),
                    problem,
                }
            );
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare-details", "prepare-details"]
            );
        }
    }

    #[tokio::test]
    async fn post_fetch_revocation_expiry_and_authority_changes_suppress_success_and_error() {
        for disposition in [DENIED, EXPIRED, CHANGED_AUTHORITY, CHANGED_SNAPSHOT] {
            for success in [false, true] {
                let (service, persistence, request) = fixture();
                let during_fetch = Arc::clone(&persistence);
                let result = service
                    .candidate_details_with(request, false, lease().await, move |_, _| async move {
                        during_fetch
                            .disposition
                            .store(disposition, Ordering::SeqCst);
                        if success {
                            Ok(candidate())
                        } else {
                            Err(ProviderRuntimeError::provider(
                                "must not escape reauthorization",
                            ))
                        }
                    })
                    .await;
                if disposition == EXPIRED {
                    assert_eq!(result.unwrap(), None);
                } else {
                    assert_eq!(result.unwrap_err().code(), ProblemCode::Forbidden);
                }
                assert_eq!(
                    *persistence.calls.lock().unwrap(),
                    ["prepare-details", "prepare-details"]
                );
            }
        }
    }

    #[tokio::test]
    async fn cancelling_details_network_releases_lease_without_post_fetch_work() {
        let (service, persistence, request) = fixture();
        let gate = Arc::new(tokio::sync::Mutex::new(()));
        let lease = ProviderOperationLease::new(Arc::clone(&gate).lock_owned().await);
        let (entered, started) = tokio::sync::oneshot::channel();
        let caller = tokio::spawn(async move {
            service
                .candidate_details_with(request, false, lease, |_, _| async move {
                    let _ = entered.send(());
                    std::future::pending::<Result<ProviderCandidate, ProviderRuntimeError>>().await
                })
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), started)
            .await
            .unwrap()
            .unwrap();
        assert!(gate.try_lock().is_err());
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        let _released = tokio::time::timeout(std::time::Duration::from_secs(5), gate.lock())
            .await
            .unwrap();
        assert_eq!(*persistence.calls.lock().unwrap(), ["prepare-details"]);
    }

    #[tokio::test]
    async fn cancelling_details_post_fetch_preparation_retains_lease_until_completion() {
        for success in [false, true] {
            let (service, persistence, request) = fixture();
            let gate = Arc::new(tokio::sync::Mutex::new(()));
            let lease = ProviderOperationLease::new(Arc::clone(&gate).lock_owned().await);
            let (entered, started) = tokio::sync::oneshot::channel();
            let (release, finish) = std::sync::mpsc::channel();
            let during_fetch = Arc::clone(&persistence);
            let caller = tokio::spawn(async move {
                service
                    .candidate_details_with(request, false, lease, move |_, _| async move {
                        // Arm only after the initial preparation, so cancellation
                        // targets the authorization recheck after completed I/O.
                        *during_fetch.pause_prepare.lock().unwrap() = Some((entered, finish));
                        if success {
                            Ok(candidate())
                        } else {
                            Err(ProviderRuntimeError::provider("fixture outage"))
                        }
                    })
                    .await
            });
            tokio::time::timeout(std::time::Duration::from_secs(5), started)
                .await
                .unwrap()
                .unwrap();
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
            let held = gate.try_lock().is_err();
            let _ = release.send(());
            let _completed = tokio::time::timeout(std::time::Duration::from_secs(5), gate.lock())
                .await
                .unwrap();
            assert!(
                held,
                "cancelled details must retain the running recheck's lease"
            );
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare-details", "prepare-details"]
            );
        }
    }
}
