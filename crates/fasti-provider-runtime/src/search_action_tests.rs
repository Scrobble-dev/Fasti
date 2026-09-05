mod candidate_action_tests {
    use super::*;
    use std::sync::atomic::AtomicU8;

    const CURRENT: u8 = 0;
    const DENIED: u8 = 1;
    const REPLAY: u8 = 2;
    const CHANGED_AUTHORITY: u8 = 3;
    const CHANGED_SNAPSHOT: u8 = 4;

    struct ActionPersistence {
        details: PreparedSearchCandidateDetails,
        receipt: SearchCandidateActionReceipt,
        disposition: AtomicU8,
        calls: Mutex<Vec<&'static str>>,
        fields: Mutex<Option<Vec<ProviderMetadataField>>>,
        pause_commit: Mutex<
            Option<(
                tokio::sync::oneshot::Sender<()>,
                std::sync::mpsc::Receiver<()>,
            )>,
        >,
    }

    impl ActionPersistence {
        fn current(
            &self,
            command: &SearchCandidateActionCommand,
        ) -> ApplicationResult<SearchCandidateActionPreparation> {
            assert_eq!(
                command.request.terms_revision,
                "fasti.public-metadata-cache.v1"
            );
            assert_eq!(command.semantic_digest(), self.receipt.semantic_digest());
            let mut details = self.details.clone();
            match self.disposition.load(Ordering::SeqCst) {
                DENIED => {
                    return Err(Box::new(FastiProblem::forbidden(
                        CapabilityKey::AttachIdentifier,
                        command.request.correlation_id,
                    )))
                }
                REPLAY => {
                    return Ok(SearchCandidateActionPreparation::Replay(Box::new(
                        self.receipt.clone(),
                    )))
                }
                CHANGED_AUTHORITY => {
                    details.provider_authority_fingerprint = Sha256Digest::from_bytes(&[8; 32])
                }
                CHANGED_SNAPSHOT => {
                    let original = &details.candidate.receipt;
                    details.candidate.receipt = SearchCandidateReceipt::new(
                        original.id(),
                        original.partition().clone(),
                        original.candidate().clone(),
                        Sha256Digest::from_bytes(&[9; 32]),
                        original.lifetime().clone(),
                    );
                }
                CURRENT => {}
                other => panic!("unexpected fixture disposition {other}"),
            }
            Ok(match command.evidence_mode {
                SearchCandidateEvidenceMode::Cached => {
                    SearchCandidateActionPreparation::Cached(details.candidate)
                }
                SearchCandidateEvidenceMode::Refetch => {
                    SearchCandidateActionPreparation::Refetch(details)
                }
            })
        }
    }

    impl SearchPersistencePort for ActionPersistence {
        fn authorize_search_page_request(&self, _: RequestCorrelationId, _: &ApplicationAccessContext) -> ApplicationResult<()> {
            unreachable!("action tests do not acquire pages through HTTP")
        }
        fn prepare_search_candidate_action(
            &self,
            command: &SearchCandidateActionCommand,
        ) -> ApplicationResult<SearchCandidateActionPreparation> {
            self.calls.lock().unwrap().push("prepare-action");
            self.current(command)
        }

        fn commit_search_candidate_action(
            &self,
            command: &SearchCandidateActionCommand,
            prepared: &SearchCandidateActionPreparation,
            fields: Option<&[ProviderMetadataField]>,
        ) -> ApplicationResult<SearchCandidateActionReceipt> {
            self.calls.lock().unwrap().push("commit-action");
            let pause = self.pause_commit.lock().unwrap().take();
            if let Some((entered, release)) = pause {
                let _ = entered.send(());
                release
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .expect("bounded commit worker release");
            }
            // The real store owns atomic authorization. This fixture exercises
            // propagation of that boundary's refusal or concurrent replay.
            let current = self.current(command)?;
            if let SearchCandidateActionPreparation::Replay(receipt) = current {
                return Ok(*receipt);
            }
            if &current != prepared {
                return Err(Box::new(FastiProblem::forbidden(
                    CapabilityKey::AttachIdentifier,
                    command.request.correlation_id,
                )));
            }
            match prepared {
                SearchCandidateActionPreparation::Cached(_) => assert!(fields.is_none()),
                SearchCandidateActionPreparation::Refetch(_) => {
                    assert!(fields.is_some_and(|value| !value.is_empty()))
                }
                SearchCandidateActionPreparation::Replay(_) => {
                    panic!("initial replay must not commit")
                }
            }
            *self.fields.lock().unwrap() = fields.map(<[ProviderMetadataField]>::to_vec);
            let mut receipt = self.receipt.clone();
            if let Some(field) = fields.and_then(|value| value.first()) {
                receipt.provenance = field.claim().provenance().clone();
                receipt.fetched_at = field.claim().fetched_at();
                receipt.expires_at = field.claim().expires_at();
                receipt.initial_status = field.claim().initial_status();
            }
            Ok(receipt)
        }

        fn search_local_records(
            &self,
            _: &LocalSearchRequest,
        ) -> ApplicationResult<LocalSearchPage> {
            panic!("action orchestration must not list local Records")
        }
        fn prepare_search_page(
            &self,
            _: &SearchPageRequest,
        ) -> ApplicationResult<PreparedSearchPage> {
            panic!("actions must not create a Search page")
        }
        fn commit_search_page(
            &self,
            _: &SearchPageRequest,
            _: &PreparedSearchPage,
            _: &[SearchCandidate],
            _: &Sha256Digest,
            _: Option<u32>,
        ) -> ApplicationResult<StoredSearchPage> {
            panic!("actions must not overwrite the immutable Search snapshot")
        }
        fn read_cached_search_page(
            &self,
            _: &SearchPageRequest,
            _: bool,
        ) -> ApplicationResult<Option<StoredSearchPage>> {
            panic!("action receipt authority must not use the page-cache lifetime")
        }
        fn read_search_candidate(
            &self,
            _: &ReadSearchCandidateRequest,
        ) -> ApplicationResult<Option<StoredSearchCandidate>> {
            panic!("actions use their mutation-authorized preparation")
        }
        fn prepare_search_candidate_details(
            &self,
            _: &ReadSearchCandidateRequest,
        ) -> ApplicationResult<Option<PreparedSearchCandidateDetails>> {
            panic!("detail-read permission alone must not authorize a save")
        }
    }

    fn candidate() -> ProviderCandidate {
        crate::providers::search_page_fixture().candidates.remove(0)
    }

    fn fixture(
        mode: SearchCandidateEvidenceMode,
    ) -> (
        ProviderSearchService,
        Arc<ActionPersistence>,
        SearchCandidateActionCommand,
    ) {
        let (service, _, request) = setup(None, None);
        let ApplicationAccessContext::Credential(access) = &request.access else {
            panic!("existing credential fixture")
        };
        let context = SearchProviderQuery::try_new(
            request.query.query().clone(),
            request.query.provider().clone(),
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
        let snapshot = StoredSearchCandidate {
            receipt: SearchCandidateReceipt::new(
                SearchCandidateReceiptId::new_v7(),
                partition,
                candidate().search_evidence().unwrap(),
                page.response_digest,
                page.lifetime,
            ),
            context,
        };
        let command = SearchCandidateActionCommand {
            request: ReadSearchCandidateRequest {
                correlation_id: request.correlation_id,
                access: request.access.clone(),
                candidate_receipt_id: snapshot.receipt.id(),
                provider: request.query.provider().clone(),
                grain: Grain::Film,
                outbound_policy: request.outbound_policy,
                terms_revision: "untrusted-caller-revision".into(),
            },
            operation_id: OperationId::new_v7(),
            action: SearchRecordAction::Create,
            evidence_mode: mode,
        };
        let fields = snapshot.metadata_fields().unwrap();
        let claim = fields[0].claim();
        let receipt = SearchCandidateActionReceipt {
            workspace_id: access.workspace_id(),
            profile_id: access.profile_id(),
            actor_client_id: access.client_id(),
            actor_subject_id: None,
            operation_id: command.operation_id,
            candidate_receipt_id: snapshot.receipt.id(),
            provider: "tmdb".into(),
            grain: Grain::Film,
            action: command.action,
            evidence_mode: mode,
            record_id: RecordId::new_v7(),
            disposition: SearchRecordActionDisposition::Created,
            search_context_digest: snapshot.context.digest(),
            search_response_digest: snapshot.receipt.response_digest().clone(),
            provenance: claim.provenance().clone(),
            fetched_at: claim.fetched_at(),
            expires_at: claim.expires_at(),
            initial_status: claim.initial_status(),
            committed_at: chrono::Utc::now(),
        };
        let details = PreparedSearchCandidateDetails {
            candidate: snapshot,
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
        };
        let persistence = Arc::new(ActionPersistence {
            details,
            receipt,
            disposition: AtomicU8::new(CURRENT),
            calls: Mutex::new(vec![]),
            fields: Mutex::new(None),
            pause_commit: Mutex::new(None),
        });
        (
            ProviderSearchService::new(service.runtime, persistence.clone()),
            persistence,
            command,
        )
    }

    #[tokio::test]
    async fn public_cached_save_and_completed_replay_never_fetch_and_use_trusted_policy() {
        for (mode, replay) in [
            (SearchCandidateEvidenceMode::Cached, false),
            (SearchCandidateEvidenceMode::Cached, true),
            (SearchCandidateEvidenceMode::Refetch, true),
        ] {
            for revision in ["", "caller-policy", "tmdb_attribution_required"] {
                let (service, persistence, mut command) = fixture(mode);
                command.request.terms_revision = revision.into();
                if replay {
                    persistence.disposition.store(REPLAY, Ordering::SeqCst);
                }
                let outcome = service
                    .save_candidate(command, lease().await)
                    .await
                    .unwrap();
                assert_eq!(
                    outcome,
                    ProviderSearchActionOutcome::Saved(Box::new(persistence.receipt.clone()))
                );
                assert_eq!(
                    *persistence.calls.lock().unwrap(),
                    if replay {
                        vec!["prepare-action"]
                    } else {
                        vec!["prepare-action", "commit-action"]
                    }
                );
                assert!(persistence.fields.lock().unwrap().is_none());
            }
        }
    }

    #[tokio::test]
    async fn save_refetch_uses_exact_stored_coordinates_and_actual_response_provenance() {
        let (service, persistence, command) = fixture(SearchCandidateEvidenceMode::Refetch);
        let expected_state = persistence.details.provider_state.clone();
        let mut fresh = candidate();
        fresh.title = "Fresh detail title".into();
        fresh.overview = Some("Fresh detail overview".into());
        let expected_digest = crate::providers::search_page_fixture().evidence_digest;
        assert_ne!(expected_digest, persistence.receipt.search_response_digest);
        let before = chrono::Utc::now();
        let outcome = service
            .save_candidate_with(command, lease().await, move |selection, state| async move {
                assert_eq!(
                    selection,
                    ProviderSelectionInput {
                        provider: "tmdb".into(),
                        provider_id: "42".into(),
                        kind: "movie".into(),
                        locale: Some("fr-fr".into()),
                        region: None
                    }
                );
                assert_eq!(state, expected_state);
                Ok(fresh)
            })
            .await
            .unwrap();
        let after = chrono::Utc::now();
        assert!(
            matches!(outcome, ProviderSearchActionOutcome::Saved(receipt) if receipt.record_id == persistence.receipt.record_id && receipt.provenance.evidence_digest() == Some(&expected_digest))
        );
        let fields = persistence.fields.lock().unwrap();
        let fields = fields.as_ref().unwrap();
        assert_eq!(fields[0].claim().value(), "Fresh detail title");
        assert_eq!(fields.len(), 2);
        for field in fields {
            let claim = field.claim();
            assert!((before..=after).contains(&claim.fetched_at()));
            assert_eq!(
                claim.expires_at(),
                Some(claim.fetched_at() + chrono::Duration::seconds(METADATA_FRESH_SECONDS))
            );
            assert_eq!(claim.provenance().locale().unwrap().as_str(), "fr-fr");
            assert_eq!(claim.provenance().region(), None);
            assert_eq!(claim.provenance().source_identifier(), Some("42"));
            assert_eq!(claim.provenance().evidence_digest(), Some(&expected_digest));
        }
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare-action", "commit-action"]
        );
        assert_eq!(
            persistence
                .details
                .candidate
                .receipt
                .candidate()
                .data()
                .title,
            "Fixture film"
        );
    }

    #[tokio::test]
    async fn rejected_save_preparation_never_calls_fetch_or_commit() {
        for mode in [
            SearchCandidateEvidenceMode::Cached,
            SearchCandidateEvidenceMode::Refetch,
        ] {
            let (service, persistence, command) = fixture(mode);
            persistence.disposition.store(DENIED, Ordering::SeqCst);
            let error = service
                .save_candidate_with(command, lease().await, |_, _| async {
                    panic!("unauthorized action must not fetch")
                })
                .await
                .unwrap_err();
            assert_eq!(error.code(), ProblemCode::Forbidden);
            assert_eq!(*persistence.calls.lock().unwrap(), ["prepare-action"]);
        }
    }

    #[tokio::test]
    async fn invalid_identity_fields_and_provider_failure_reauthorize_without_commit() {
        for case in 0..6 {
            let (service, persistence, command) = fixture(SearchCandidateEvidenceMode::Refetch);
            let (response, expected) = match case {
                0 => (
                    Err(ProviderRuntimeError::provider("fixture outage")),
                    ProblemCode::ProviderUnavailable,
                ),
                1 => (
                    Err(ProviderRuntimeError::credential(
                        "fixture credential refusal",
                    )),
                    ProblemCode::ProviderCredentialInvalid,
                ),
                2 => (
                    Err(ProviderRuntimeError::rate_limited("fixture throttling")),
                    ProblemCode::ProviderRateLimited,
                ),
                _ => {
                    let mut value = candidate();
                    match case {
                        3 => value.provider_id = "43".into(),
                        4 => value.kind = "show",
                        _ => value.overview = Some("x".repeat(4097)),
                    }
                    (Ok(value), ProblemCode::ProviderResponseInvalid)
                }
            };
            let outcome = service
                .save_candidate_with(command, lease().await, |_, _| async move { response })
                .await
                .unwrap();
            assert_eq!(
                outcome,
                ProviderSearchActionOutcome::Unavailable { problem: expected }
            );
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare-action", "prepare-action"]
            );
            assert!(persistence.fields.lock().unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn post_io_revocation_or_snapshot_drift_suppresses_success_and_failure() {
        for disposition in [DENIED, CHANGED_AUTHORITY, CHANGED_SNAPSHOT] {
            for success in [false, true] {
                let (service, persistence, command) = fixture(SearchCandidateEvidenceMode::Refetch);
                let during_fetch = Arc::clone(&persistence);
                let result = service
                    .save_candidate_with(command, lease().await, move |_, _| async move {
                        during_fetch
                            .disposition
                            .store(disposition, Ordering::SeqCst);
                        if success {
                            Ok(candidate())
                        } else {
                            Err(ProviderRuntimeError::provider(
                                "must not be exposed after authority changes",
                            ))
                        }
                    })
                    .await;
                assert_eq!(result.unwrap_err().code(), ProblemCode::Forbidden);
                assert_eq!(
                    *persistence.calls.lock().unwrap(),
                    [
                        "prepare-action",
                        if success {
                            "commit-action"
                        } else {
                            "prepare-action"
                        }
                    ]
                );
                assert!(persistence.fields.lock().unwrap().is_none());
            }
        }
    }

    #[tokio::test]
    async fn concurrent_completed_replay_wins_over_fetch_success_or_failure() {
        for success in [false, true] {
            let (service, persistence, command) = fixture(SearchCandidateEvidenceMode::Refetch);
            let during_fetch = Arc::clone(&persistence);
            let outcome = service
                .save_candidate_with(command, lease().await, move |_, _| async move {
                    during_fetch.disposition.store(REPLAY, Ordering::SeqCst);
                    if success {
                        Ok(candidate())
                    } else {
                        Err(ProviderRuntimeError::provider(
                            "concurrent retry already committed",
                        ))
                    }
                })
                .await
                .unwrap();
            assert_eq!(
                outcome,
                ProviderSearchActionOutcome::Saved(Box::new(persistence.receipt.clone()))
            );
            assert!(persistence.fields.lock().unwrap().is_none());
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                [
                    "prepare-action",
                    if success {
                        "commit-action"
                    } else {
                        "prepare-action"
                    }
                ]
            );
        }
    }

    #[tokio::test]
    async fn save_cancellation_releases_network_but_retains_started_commit_lease() {
        for commit in [false, true] {
            let (service, persistence, command) = fixture(SearchCandidateEvidenceMode::Refetch);
            let gate = Arc::new(tokio::sync::Mutex::new(()));
            let lease = ProviderOperationLease::new(Arc::clone(&gate).lock_owned().await);
            let (entered, started) = tokio::sync::oneshot::channel();
            let (release, finish) = std::sync::mpsc::channel();
            let mut network_entered = Some(entered);
            if commit {
                *persistence.pause_commit.lock().unwrap() =
                    Some((network_entered.take().unwrap(), finish));
            }
            let caller = tokio::spawn(async move {
                service
                    .save_candidate_with(command, lease, |_, _| async move {
                        if let Some(entered) = network_entered {
                            let _ = entered.send(());
                            std::future::pending::<()>().await;
                        }
                        Ok(candidate())
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
            let _released = tokio::time::timeout(std::time::Duration::from_secs(5), gate.lock())
                .await
                .unwrap();
            assert_eq!(held, commit);
            assert_eq!(
                persistence.calls.lock().unwrap().contains(&"commit-action"),
                commit
            );
            assert_eq!(persistence.fields.lock().unwrap().is_some(), commit);
        }
    }
}
