mod candidate_action_tests {
    include!("search_action_negative_tests.rs");
    include!("search_candidate_policy_tests.rs");
    use super::*;
    use fasti_application::{
        CreateRecordCommand, IdentityPort, ProfileRecordStatePort, ProviderIdentifierActionCommand,
        ProviderIdentifierActionPreparation, ReadSearchCandidateRequest, ScopeKey,
        SearchCandidateActionCommand,
        SearchCandidateActionPreparation, SearchCandidateActionReceipt,
        SearchCandidateEvidenceMode, SearchActionReceiptLimits, SearchRecordAction,
        SearchRecordActionDisposition,
        SetTrackingDispositionCommand,
    };
    use fasti_domain::{
        Grain, OperationId, RecordId, SearchCandidateReceiptId, TrackingDisposition,
    };

    fn fixture() -> (
        TestNode,
        SearchPageRequest,
        SearchCandidateActionCommand,
        u64,
    ) {
        let (node, request) = setup();
        let data = SearchCandidateData {
            original_title: Some("Original title".into()),
            overview: Some("Accepted original overview".into()),
            ..candidate("42").data().clone()
        };
        let saved = commit(&node, &request, &[SearchCandidate::try_new(data).unwrap()]);
        let command = SearchCandidateActionCommand {
            request: details(&request, saved.candidates[0].id()),
            operation_id: OperationId::new_v7(),
            action: SearchRecordAction::Create,
            evidence_mode: SearchCandidateEvidenceMode::Cached,
        };
        (node, request, command, saved.sequence)
    }

    fn act(
        node: &TestNode,
        command: &SearchCandidateActionCommand,
    ) -> ApplicationResult<SearchCandidateActionReceipt> {
        let prepared = node.kernel.prepare_search_candidate_action(command)?;
        node.kernel
            .commit_search_candidate_action(command, &prepared, None)
    }

    fn record(node: &TestNode) -> RecordId {
        node.kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .unwrap()
            .record_id()
    }

    fn rows(node: &TestNode, tables: &[&str]) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        tables
            .iter()
            .map(|table| {
                let mut statement = connection
                    .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
                    .unwrap();
                let columns = statement.column_count();
                statement
                    .query_map([], |row| {
                        (0..columns)
                            .map(|column| row.get::<_, rusqlite::types::Value>(column))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            })
            .collect()
    }

    const MUTATION_TABLES: &[&str] = &[
        "records",
        "namespace_definitions",
        "external_identifiers",
        "metadata_field_claims",
        "metadata_claims",
        "metadata_claim_provenance",
        "local_search_grams",
        "search_action_receipts",
        "workspace_revisions",
    ];

    const UNRELATED_TABLES: &[&str] = &[
        "profile_record_tracking_dispositions",
        "observations",
        "occurrences",
        "interpretations",
        "metadata_profile_field_overrides",
        "metadata_projection_policies",
        "metadata_rating_claims",
        "profile_nuvio_collections",
    ];

    fn remove_scope(node: &TestNode, scope: &str) {
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = ?2",
                params![node.access.grant_id().to_string(), scope],
            )
            .unwrap();
    }

    fn provider_identifier_fixture() -> (
        TestNode,
        ProviderIdentifierActionCommand,
        fasti_domain::ExternalIdentifierClaim,
    ) {
        provider_identifier_fixture_with_limits(SearchActionReceiptLimits::supported_default())
    }

    fn provider_identifier_fixture_with_limits(
        limits: SearchActionReceiptLimits,
    ) -> (
        TestNode,
        ProviderIdentifierActionCommand,
        fasti_domain::ExternalIdentifierClaim,
    ) {
        let node = TestNode::with_search_action_receipt_limits(limits);
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                state_for("metadata.read", 1),
            )
            .unwrap();
        let command = ProviderIdentifierActionCommand {
            correlation_id: RequestCorrelationId::new_v7(),
            access: node.access.into(),
            outbound_policy: OutboundAccessPolicy::default(),
            terms_revision: "tmdb-v1".into(),
            operation_id: OperationId::new_v7(),
            provider: ProviderId::try_new("tmdb").unwrap(),
            provider_record_id: "42".into(),
            grain: Grain::Film,
            action: SearchRecordAction::Create,
        };
        let identifier = fasti_application::provider_identity_mapping_for_grain(
            command.provider.as_str(),
            command.grain,
        )
        .unwrap()
        .identifier(command.provider_record_id.clone())
        .unwrap();
        (node, command, identifier)
    }

    fn act_provider_identifier(
        node: &TestNode,
        command: &ProviderIdentifierActionCommand,
        identifier: &fasti_domain::ExternalIdentifierClaim,
    ) -> ApplicationResult<fasti_application::ProviderIdentifierActionReceipt> {
        let prepared = node.kernel.prepare_provider_identifier_action(command)?;
        node.kernel
            .commit_provider_identifier_action(command, &prepared, identifier)
    }

    #[test]
    fn simultaneous_cached_actions_replay_or_reuse_one_record_without_duplicate_claims() {
        use std::sync::mpsc;
        use std::time::Duration;

        for distinct_operations in [false, true] {
            let (node, _, first, _) = fixture();
            let mut second = first.clone();
            if distinct_operations {
                second.operation_id = OperationId::new_v7();
            }
            let commands = [first, second];
            let preparations = commands.each_ref().map(|command| {
                let prepared = node
                    .kernel
                    .prepare_search_candidate_action(command)
                    .unwrap();
                assert!(matches!(
                    prepared,
                    SearchCandidateActionPreparation::Cached(_)
                ));
                prepared
            });
            let unrelated = rows(&node, UNRELATED_TABLES);
            let (ready_tx, ready_rx) = mpsc::channel();
            let (results_tx, results_rx) = mpsc::channel();
            let mut starts = Vec::new();
            let mut workers = Vec::new();
            // Both callers have observed an uncommitted operation. Keep the
            // real connection unavailable until both workers are ready, then
            // release them together to the existing transaction owner.
            let connection = node.kernel.inner.connection.lock().unwrap();
            for (index, (command, prepared)) in
                commands.iter().cloned().zip(preparations).enumerate()
            {
                let kernel = node.kernel.clone();
                let ready = ready_tx.clone();
                let results = results_tx.clone();
                let (start, started) = mpsc::channel();
                starts.push(start);
                workers.push(std::thread::spawn(move || {
                    ready.send(()).unwrap();
                    started
                        .recv_timeout(Duration::from_secs(10))
                        .expect("bounded action start");
                    let result = kernel.commit_search_candidate_action(&command, &prepared, None);
                    results.send((index, result)).unwrap();
                }));
            }
            drop(ready_tx);
            drop(results_tx);
            for _ in 0..2 {
                ready_rx
                    .recv_timeout(Duration::from_secs(5))
                    .expect("both action workers ready");
            }
            for start in starts {
                start.send(()).unwrap();
            }
            drop(connection);
            let mut results = (0..2)
                .map(|_| {
                    let (index, result) = results_rx
                        .recv_timeout(Duration::from_secs(10))
                        .expect("bounded action completion");
                    (index, result.expect("both real SQLite actions succeed"))
                })
                .collect::<Vec<_>>();
            for worker in workers {
                worker.join().expect("action worker completed");
            }
            results.sort_by_key(|(index, _)| *index);
            let first = &results[0].1;
            let second = &results[1].1;
            assert_eq!(first.record_id, second.record_id);
            if distinct_operations {
                assert_ne!(first.operation_id, second.operation_id);
                assert!(matches!(
                    (first.disposition, second.disposition),
                    (
                        SearchRecordActionDisposition::Created,
                        SearchRecordActionDisposition::Reused
                    ) | (
                        SearchRecordActionDisposition::Reused,
                        SearchRecordActionDisposition::Created
                    )
                ));
            } else {
                assert_eq!(first, second);
                assert_eq!(first.disposition, SearchRecordActionDisposition::Created);
            }
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                for (table, expected) in [
                    ("records", 1_i64),
                    ("external_identifiers", 1),
                    ("metadata_field_claims", 3),
                    ("metadata_claims", 3),
                    ("metadata_claim_provenance", 3),
                    (
                        "search_action_receipts",
                        if distinct_operations { 2 } else { 1 },
                    ),
                ] {
                    assert_eq!(
                        connection
                            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                                .get::<_, i64>(0))
                            .unwrap(),
                        expected,
                        "{table}, distinct_operations={distinct_operations}"
                    );
                }
                for (index, receipt) in &results {
                    assert_eq!(receipt.operation_id, commands[*index].operation_id);
                    let json: String = connection.query_row("SELECT receipt_json FROM search_action_receipts WHERE operation_id = ?1", [receipt.operation_id.to_string()], |row| row.get(0)).unwrap();
                    assert_eq!(
                        crate::search_actions::decode_receipt(
                            &json,
                            RequestCorrelationId::new_v7()
                        )
                        .unwrap(),
                        *receipt
                    );
                }
            }
            let after = rows(&node, MUTATION_TABLES);
            for (index, receipt) in &results {
                assert_eq!(act(&node, &commands[*index]).unwrap(), *receipt);
            }
            assert_eq!(rows(&node, MUTATION_TABLES), after);
            assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
        }
    }

    #[test]
    fn cached_actions_create_reuse_and_attach_without_library_or_profile_mutations() {
        let (node, _, mut command, _) = fixture();
        let existing = record(&node);
        node.kernel
            .set_tracking_disposition(SetTrackingDispositionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                existing,
                Some(TrackingDisposition::OnHold),
            ))
            .unwrap();
        let unrelated = rows(&node, UNRELATED_TABLES);
        let prepared = node
            .kernel
            .prepare_search_candidate_action(&command)
            .unwrap();
        let SearchCandidateActionPreparation::Cached(snapshot) = &prepared else {
            panic!("explicit cached action must prepare original evidence")
        };
        let created = node
            .kernel
            .commit_search_candidate_action(&command, &prepared, None)
            .unwrap();
        assert_eq!(created.disposition, SearchRecordActionDisposition::Created);
        assert_ne!(created.record_id, existing);
        assert_eq!(created.profile_id, node.access.profile_id());
        assert_eq!(created.actor_client_id, node.access.client_id());
        assert_eq!(created.actor_subject_id, None);
        assert_eq!(
            created.search_response_digest,
            *snapshot.receipt.response_digest()
        );
        assert_eq!(created.fetched_at, snapshot.receipt.lifetime().created_at());
        assert_eq!(
            created.expires_at,
            Some(snapshot.receipt.lifetime().fresh_until())
        );
        let claims_before = rows(
            &node,
            &[
                "metadata_claims",
                "metadata_field_claims",
                "metadata_claim_provenance",
            ],
        );
        command.operation_id = OperationId::new_v7();
        let reused = act(&node, &command).unwrap();
        assert_eq!(reused.disposition, SearchRecordActionDisposition::Reused);
        assert_eq!(reused.record_id, created.record_id);
        command.operation_id = OperationId::new_v7();
        command.action = SearchRecordAction::Attach(created.record_id);
        let attached = act(&node, &command).unwrap();
        assert_eq!(
            attached.disposition,
            SearchRecordActionDisposition::AlreadyAttached
        );
        assert_eq!(attached.record_id, created.record_id);
        assert_eq!(
            rows(
                &node,
                &[
                    "metadata_claims",
                    "metadata_field_claims",
                    "metadata_claim_provenance"
                ]
            ),
            claims_before
        );
        assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
    }

    #[test]
    fn cached_attach_uses_explicit_local_record_and_registers_namespace_atomically() {
        let (node, _, mut command, _) = fixture();
        let target = record(&node);
        node.kernel
            .set_tracking_disposition(SetTrackingDispositionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                target,
                Some(TrackingDisposition::Dropped),
            ))
            .unwrap();
        command.action = SearchRecordAction::Attach(target);
        let unrelated = rows(&node, UNRELATED_TABLES);
        let outcome = act(&node, &command).unwrap();
        assert_eq!(outcome.record_id, target);
        assert_eq!(outcome.disposition, SearchRecordActionDisposition::Attached);
        let connection = node.kernel.inner.connection.lock().unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM records", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        let identifier: (String, String) = connection
            .query_row(
                "SELECT namespace, value FROM external_identifiers WHERE record_id = ?1",
                [target.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(identifier, ("tmdb.movie".into(), "42".into()));
        drop(connection);
        assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
    }

    #[test]
    fn completed_action_replays_after_candidate_gc_provider_disable_and_search_scope_removal() {
        let (node, request, command, sequence) = fixture();
        let prepared = node
            .kernel
            .prepare_search_candidate_action(&command)
            .unwrap();
        let original = node
            .kernel
            .commit_search_candidate_action(&command, &prepared, None)
            .unwrap();
        age_page(&node, sequence, 86_401);
        commit(&node, &request, &[candidate("43")]);
        assert_eq!(node.kernel.inner.connection.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM search_candidate_receipts WHERE candidate_receipt_id = ?1",
            [command.request.candidate_receipt_id.to_string()], |row| row.get::<_, i64>(0),
        ).unwrap(), 0);
        let prior = state(1);
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                ProviderCapabilityState::try_new(
                    prior.provider_id().clone(),
                    prior.capability_id().clone(),
                    ProviderCapabilityStatus::Disabled,
                    2,
                    prior.credential_requirement(),
                    prior.credential_reference().cloned(),
                    prior.credential_status(),
                    prior.configuration_digest().clone(),
                    ProviderCheckMetadata::never_run(),
                    ProviderCheckMetadata::never_run(),
                )
                .unwrap(),
            )
            .unwrap();
        remove_scope(&node, "metadata_search");
        let before = rows(&node, MUTATION_TABLES);
        assert_eq!(
            node.kernel
                .prepare_search_candidate_action(&command)
                .unwrap(),
            SearchCandidateActionPreparation::Replay(Box::new(original.clone()))
        );
        assert_eq!(
            node.kernel
                .commit_search_candidate_action(&command, &prepared, None)
                .unwrap(),
            original
        );
        assert_eq!(act(&node, &command).unwrap(), original);
        assert_eq!(rows(&node, MUTATION_TABLES), before);
    }

    #[test]
    fn completed_action_replays_after_scoped_discard_without_changing_durable_history() {
        let (node, request, command, _) = fixture();
        let action_prepared = node
            .kernel
            .prepare_search_candidate_action(&command)
            .unwrap();
        let original = node
            .kernel
            .commit_search_candidate_action(&command, &action_prepared, None)
            .unwrap();
        let page_prepared = node.kernel.prepare_search_page(&request).unwrap();
        let durable = rows(&node, MUTATION_TABLES);
        let unrelated = rows(&node, UNRELATED_TABLES);
        assert_eq!(durable[0].len(), 1, "populated Record");
        assert_eq!(durable[2].len(), 1, "populated identifier");
        assert_eq!(durable[3].len(), 3, "populated field claims");
        assert_eq!(durable[4].len(), 3, "populated metadata claims");
        assert_eq!(durable[5].len(), 3, "populated provenance");
        assert_eq!(durable[7].len(), 1, "populated durable action receipt");
        assert_eq!(
            rows(&node, &["search_pages", "search_candidate_receipts"])
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            [1, 1]
        );

        node.kernel
            .discard_cached_search_page(&request, &page_prepared)
            .unwrap();
        assert!(rows(&node, &["search_pages", "search_candidate_receipts"])
            .iter()
            .all(Vec::is_empty));
        assert_eq!(rows(&node, MUTATION_TABLES), durable);
        assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);

        // Replay needs current IdentityWrite, not permission to reacquire the
        // deleted Search evidence. It returns the exact immutable old outcome.
        remove_scope(&node, "metadata_search");
        assert_eq!(
            node.kernel
                .prepare_search_candidate_action(&command)
                .unwrap(),
            SearchCandidateActionPreparation::Replay(Box::new(original.clone()))
        );
        assert_eq!(
            node.kernel
                .commit_search_candidate_action(&command, &action_prepared, None)
                .unwrap(),
            original
        );
        assert_eq!(act(&node, &command).unwrap(), original);
        assert_eq!(rows(&node, MUTATION_TABLES), durable);
        assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
        assert!(rows(&node, &["search_pages", "search_candidate_receipts"])
            .iter()
            .all(Vec::is_empty));
    }

    #[test]
    fn action_operation_reuse_with_changed_intent_or_profile_conflicts_without_mutation() {
        let (node, _, command, _) = fixture();
        let original = act(&node, &command).unwrap();
        let other = node.add_profile_with_scopes(&[ScopeKey::IdentityWrite]);
        let before = rows(&node, MUTATION_TABLES);
        for mutation in 0..6 {
            let mut changed = command.clone();
            match mutation {
                0 => changed.action = SearchRecordAction::Attach(original.record_id),
                1 => changed.evidence_mode = SearchCandidateEvidenceMode::Refetch,
                2 => changed.request.candidate_receipt_id = SearchCandidateReceiptId::new_v7(),
                3 => changed.request.provider = ProviderId::try_new("google-books").unwrap(),
                4 => changed.request.grain = Grain::Series,
                _ => changed.request.access = other.into(),
            }
            assert_eq!(
                act(&node, &changed).unwrap_err().code(),
                ProblemCode::IdempotencyConflict
            );
        }
        assert_eq!(rows(&node, MUTATION_TABLES), before);
    }

    #[test]
    fn action_commit_and_replay_require_current_identity_write() {
        for completed in [false, true] {
            let (node, _, command, _) = fixture();
            let prepared = node
                .kernel
                .prepare_search_candidate_action(&command)
                .unwrap();
            if completed {
                node.kernel
                    .commit_search_candidate_action(&command, &prepared, None)
                    .unwrap();
            }
            remove_scope(&node, "identity_write");
            let before = rows(&node, MUTATION_TABLES);
            assert_eq!(
                node.kernel
                    .prepare_search_candidate_action(&command)
                    .unwrap_err()
                    .code(),
                ProblemCode::Forbidden
            );
            assert_eq!(
                node.kernel
                    .commit_search_candidate_action(&command, &prepared, None)
                    .unwrap_err()
                    .code(),
                ProblemCode::Forbidden
            );
            assert_eq!(rows(&node, MUTATION_TABLES), before);
        }
    }

    #[test]
    fn new_cached_action_rechecks_search_authority_and_receipt_expiry_at_commit() {
        for expire in [false, true] {
            let (node, _, command, sequence) = fixture();
            let prepared = node
                .kernel
                .prepare_search_candidate_action(&command)
                .unwrap();
            if expire {
                age_page(&node, sequence, 86_401);
            } else {
                remove_scope(&node, "metadata_search");
            }
            let before = rows(&node, MUTATION_TABLES);
            assert!(node
                .kernel
                .prepare_search_candidate_action(&command)
                .is_err());
            assert!(node
                .kernel
                .commit_search_candidate_action(&command, &prepared, None)
                .is_err());
            assert_eq!(rows(&node, MUTATION_TABLES), before);
        }
    }

    #[test]
    fn cached_attach_cannot_redirect_an_identifier_or_invent_a_missing_target() {
        let (node, _, mut command, _) = fixture();
        let original = act(&node, &command).unwrap();
        let other = record(&node);
        let before = rows(&node, MUTATION_TABLES);
        for (target, problem) in [
            (other, ProblemCode::IdentityConflict),
            (RecordId::new_v7(), ProblemCode::RecordNotFound),
        ] {
            command.operation_id = OperationId::new_v7();
            command.action = SearchRecordAction::Attach(target);
            assert_eq!(act(&node, &command).unwrap_err().code(), problem);
        }
        assert_ne!(original.record_id, other);
        assert_eq!(rows(&node, MUTATION_TABLES), before);
    }

    #[test]
    fn cached_action_late_field_or_receipt_failure_rolls_back_all_record_effects() {
        for fail_receipt in [false, true] {
            let (node, _, command, _) = fixture();
            let prepared = node
                .kernel
                .prepare_search_candidate_action(&command)
                .unwrap();
            let before = rows(&node, MUTATION_TABLES);
            let unrelated = rows(&node, UNRELATED_TABLES);
            let sql = if fail_receipt {
                "CREATE TRIGGER reject_search_action_receipt BEFORE INSERT ON search_action_receipts BEGIN SELECT RAISE(ABORT, 'fixture receipt failure'); END;"
            } else {
                "CREATE TRIGGER reject_search_action_field BEFORE INSERT ON metadata_claim_provenance WHEN NEW.field_key = 'core.original_title' AND EXISTS (SELECT 1 FROM metadata_claim_provenance WHERE record_id = NEW.record_id AND field_key = 'core.title') BEGIN SELECT RAISE(ABORT, 'fixture later field failure'); END;"
            };
            node.kernel
                .inner
                .connection
                .lock()
                .unwrap()
                .execute_batch(sql)
                .unwrap();
            assert!(node
                .kernel
                .commit_search_candidate_action(&command, &prepared, None)
                .is_err());
            assert_eq!(rows(&node, MUTATION_TABLES), before);
            assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
        }
    }

    #[test]
    fn provider_identifier_action_saves_once_replays_and_rejects_changed_intent() {
        let (node, command, identifier) = provider_identifier_fixture();
        let prepared = node
            .kernel
            .prepare_provider_identifier_action(&command)
            .unwrap();
        assert!(matches!(
            prepared,
            ProviderIdentifierActionPreparation::Refetch { .. }
        ));
        let saved = node
            .kernel
            .commit_provider_identifier_action(&command, &prepared, &identifier)
            .unwrap();
        assert_eq!(saved.disposition, SearchRecordActionDisposition::Created);
        assert_eq!(
            node.kernel
                .prepare_provider_identifier_action(&command)
                .unwrap(),
            ProviderIdentifierActionPreparation::Replay(Box::new(saved.clone()))
        );
        let before = rows(&node, MUTATION_TABLES);
        assert_eq!(
            act_provider_identifier(&node, &command, &identifier).unwrap(),
            saved
        );
        assert_eq!(rows(&node, MUTATION_TABLES), before);

        let mut changed = command.clone();
        changed.provider_record_id = "43".into();
        let changed_identifier = fasti_application::provider_identity_mapping_for_grain(
            changed.provider.as_str(),
            changed.grain,
        )
        .unwrap()
        .identifier(changed.provider_record_id.clone())
        .unwrap();
        assert_eq!(
            act_provider_identifier(&node, &changed, &changed_identifier)
                .unwrap_err()
                .code(),
            ProblemCode::IdempotencyConflict
        );
        assert_eq!(rows(&node, MUTATION_TABLES), before);
    }

    #[test]
    fn action_receipt_quota_blocks_only_new_operations_and_can_be_raised_locally() {
        let limits = SearchActionReceiptLimits::try_new(1, 1024 * 1024).unwrap();
        let (node, command, identifier) = provider_identifier_fixture_with_limits(limits);
        let saved = act_provider_identifier(&node, &command, &identifier).unwrap();
        let before = rows(&node, MUTATION_TABLES);

        assert_eq!(
            act_provider_identifier(&node, &command, &identifier).unwrap(),
            saved,
            "exact replay remains available at the limit"
        );
        let mut changed_intent = command.clone();
        changed_intent.provider_record_id = "43".into();
        let changed_identifier = fasti_application::provider_identity_mapping_for_grain(
            changed_intent.provider.as_str(),
            changed_intent.grain,
        )
        .unwrap()
        .identifier(changed_intent.provider_record_id.clone())
        .unwrap();
        assert_eq!(
            act_provider_identifier(&node, &changed_intent, &changed_identifier)
                .unwrap_err()
                .code(),
            ProblemCode::IdempotencyConflict,
            "operation conflicts take precedence over capacity"
        );
        changed_intent.operation_id = OperationId::new_v7();
        assert_eq!(
            act_provider_identifier(&node, &changed_intent, &changed_identifier)
                .unwrap_err()
                .code(),
            ProblemCode::CapacityExceeded
        );
        let candidate_action = SearchCandidateActionCommand {
            request: ReadSearchCandidateRequest {
                correlation_id: RequestCorrelationId::new_v7(),
                access: node.access.into(),
                candidate_receipt_id: SearchCandidateReceiptId::new_v7(),
                provider: ProviderId::try_new("tmdb").unwrap(),
                grain: Grain::Film,
                outbound_policy: OutboundAccessPolicy::default(),
                terms_revision: "tmdb-v1".into(),
            },
            operation_id: OperationId::new_v7(),
            action: SearchRecordAction::Create,
            evidence_mode: SearchCandidateEvidenceMode::Cached,
        };
        assert_eq!(
            node.kernel
                .prepare_search_candidate_action(&candidate_action)
                .unwrap_err()
                .code(),
            ProblemCode::CapacityExceeded,
            "candidate and identifier-only actions share one receipt quota"
        );
        assert_eq!(rows(&node, MUTATION_TABLES), before);

        let retained_bytes = u64::try_from(
            node
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT length(CAST(receipt_json AS BLOB)) FROM search_action_receipts",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        )
        .unwrap();

        let (root, access) = node.into_stopped();
        let kernel = SqliteKernel::open_with_search_action_receipt_limits(
            root.path(),
            SearchActionReceiptLimits::try_new(2, retained_bytes).unwrap(),
        )
        .unwrap();
        changed_intent.access = access.into();
        assert_eq!(
            kernel
                .prepare_provider_identifier_action(&changed_intent)
                .unwrap_err()
                .code(),
            ProblemCode::CapacityExceeded,
            "the byte ceiling independently blocks new operations"
        );
        assert_eq!(
            kernel
                .prepare_provider_identifier_action(&command)
                .unwrap(),
            ProviderIdentifierActionPreparation::Replay(Box::new(saved)),
            "exact replay remains available at the byte ceiling"
        );
        drop(kernel);

        let kernel = SqliteKernel::open_with_search_action_receipt_limits(
            root.path(),
            SearchActionReceiptLimits::try_new(2, retained_bytes + 1).unwrap(),
        )
        .unwrap();
        let prepared = kernel
            .prepare_provider_identifier_action(&changed_intent)
            .unwrap();
        assert_eq!(
            kernel
                .commit_provider_identifier_action(
                    &changed_intent,
                    &prepared,
                    &changed_identifier,
                )
                .unwrap_err()
                .code(),
            ProblemCode::CapacityExceeded,
            "the exact receipt size is rechecked after mutation work"
        );
        {
            let connection = kernel.inner.connection.lock().unwrap();
            for table in ["records", "external_identifiers", "search_action_receipts"] {
                assert_eq!(
                    connection
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                            row.get::<_, i64>(0)
                        })
                        .unwrap(),
                    1,
                    "late byte admission must roll back {table}"
                );
            }
        }
        drop(kernel);

        let kernel = SqliteKernel::open_with_search_action_receipt_limits(
            root.path(),
            SearchActionReceiptLimits::try_new(2, retained_bytes + 1024 * 1024).unwrap(),
        )
        .unwrap();
        let prepared = kernel
            .prepare_provider_identifier_action(&changed_intent)
            .unwrap();
        kernel
            .commit_provider_identifier_action(
                &changed_intent,
                &prepared,
                &changed_identifier,
            )
            .unwrap();
        assert_eq!(
            kernel
                .inner
                .connection
                .lock()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM search_action_receipts", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            2
        );
    }

    #[test]
    fn simultaneous_final_slot_allows_exactly_one_new_action() {
        use std::sync::mpsc;
        use std::time::Duration;

        let limits = SearchActionReceiptLimits::try_new(1, 1024 * 1024).unwrap();
        let (node, first, identifier) = provider_identifier_fixture_with_limits(limits);
        let mut second = first.clone();
        second.operation_id = OperationId::new_v7();
        let commands = [first, second];
        let preparations = commands.each_ref().map(|command| {
            node.kernel
                .prepare_provider_identifier_action(command)
                .unwrap()
        });
        let connection = node.kernel.inner.connection.lock().unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (start_tx, start_rx) = mpsc::channel();
        let start_rx = std::sync::Arc::new(std::sync::Mutex::new(start_rx));
        let (result_tx, result_rx) = mpsc::channel();
        let workers = commands
            .into_iter()
            .zip(preparations)
            .map(|(command, prepared)| {
                let kernel = node.kernel.clone();
                let identifier = identifier.clone();
                let ready = ready_tx.clone();
                let start = start_rx.clone();
                let result = result_tx.clone();
                std::thread::spawn(move || {
                    ready.send(()).unwrap();
                    start
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(10))
                        .unwrap();
                    result
                        .send(
                            kernel
                                .commit_provider_identifier_action(
                                    &command,
                                    &prepared,
                                    &identifier,
                                )
                                .map(|_| ())
                                .map_err(|error| error.code()),
                        )
                        .unwrap();
                })
            })
            .collect::<Vec<_>>();
        drop(ready_tx);
        drop(result_tx);
        for _ in 0..2 {
            ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        start_tx.send(()).unwrap();
        start_tx.send(()).unwrap();
        drop(connection);
        let results = (0..2)
            .map(|_| result_rx.recv_timeout(Duration::from_secs(10)).unwrap())
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(ProblemCode::CapacityExceeded)))
                .count(),
            1
        );
        let connection = node.kernel.inner.connection.lock().unwrap();
        for table in ["records", "external_identifiers", "search_action_receipts"] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .unwrap(),
                1,
                "{table}"
            );
        }
    }

    #[test]
    fn provider_identifier_action_retains_only_identity_and_durable_intent() {
        let (node, command, identifier) = provider_identifier_fixture();
        let saved = act_provider_identifier(&node, &command, &identifier).unwrap();
        let connection = node.kernel.inner.connection.lock().unwrap();
        for (table, expected) in [
            ("records", 1_i64),
            ("namespace_definitions", 1),
            ("external_identifiers", 1),
            ("search_action_receipts", 1),
            ("search_pages", 0),
            ("search_candidate_receipts", 0),
            ("metadata_field_claims", 0),
            ("metadata_claims", 0),
            ("metadata_claim_provenance", 0),
            ("metadata_projections", 0),
            ("metadata_attributions", 0),
            ("metadata_cache_entries", 0),
            ("metadata_cache_claims", 0),
            ("local_search_grams", 0),
        ] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .unwrap(),
                expected,
                "unexpected retained rows in {table}"
            );
        }
        let identifier_row: (String, String, String) = connection
            .query_row(
                "SELECT record_id, namespace, value FROM external_identifiers",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            identifier_row,
            (
                saved.record_id.to_string(),
                "tmdb.movie".into(),
                "42".into()
            )
        );
    }

    #[test]
    fn provider_identifier_action_is_atomic_and_rechecks_authority_only() {
        let (node, mut command, identifier) = provider_identifier_fixture();
        let prepared = node
            .kernel
            .prepare_provider_identifier_action(&command)
            .unwrap();
        let base = state_for("metadata.read", 2);
        let observed = ProviderCapabilityState::try_new(
            base.provider_id().clone(),
            base.capability_id().clone(),
            ProviderCapabilityStatus::Degraded,
            base.capability_version(),
            base.credential_requirement(),
            base.credential_reference().cloned(),
            base.credential_status(),
            base.configuration_digest().clone(),
            ProviderCheckMetadata::try_new(
                fasti_application::ProviderCheckStatus::Unavailable,
                Some(now()),
                Some(ProblemCode::ProviderUnavailable),
            )
            .unwrap(),
            base.credential_test().clone(),
        )
        .unwrap();
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), observed)
            .unwrap();
        node.kernel
            .commit_provider_identifier_action(&command, &prepared, &identifier)
            .unwrap();

        command.operation_id = OperationId::new_v7();
        command.provider_record_id = "43".into();
        let identifier = fasti_application::provider_identity_mapping_for_grain(
            command.provider.as_str(),
            command.grain,
        )
        .unwrap()
        .identifier(command.provider_record_id.clone())
        .unwrap();
        let prepared = node
            .kernel
            .prepare_provider_identifier_action(&command)
            .unwrap();
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                ProviderCapabilityState::try_new(
                    ProviderId::try_new("tmdb").unwrap(),
                    ProviderCapabilityId::try_new("metadata.read").unwrap(),
                    ProviderCapabilityStatus::Available,
                    3,
                    CredentialRequirement::BearerToken,
                    Some(CredentialReference::try_new("secret:tmdb-test").unwrap()),
                    ProviderCredentialStatus::StoredUnverified,
                    ConfigurationDigest::parse("b".repeat(64)).unwrap(),
                    ProviderCheckMetadata::never_run(),
                    ProviderCheckMetadata::never_run(),
                )
                .unwrap(),
            )
            .unwrap();
        let before = rows(&node, MUTATION_TABLES);
        assert_eq!(
            node.kernel
                .commit_provider_identifier_action(&command, &prepared, &identifier)
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(rows(&node, MUTATION_TABLES), before);

        command.operation_id = OperationId::new_v7();
        let prepared = node
            .kernel
            .prepare_provider_identifier_action(&command)
            .unwrap();
        let before = rows(&node, MUTATION_TABLES);
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER reject_provider_identifier_receipt BEFORE INSERT ON search_action_receipts BEGIN SELECT RAISE(ABORT, 'fixture receipt failure'); END;",
            )
            .unwrap();
        assert!(node
            .kernel
            .commit_provider_identifier_action(&command, &prepared, &identifier)
            .is_err());
        assert_eq!(rows(&node, MUTATION_TABLES), before);
    }
}
