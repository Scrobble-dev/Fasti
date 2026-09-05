mod search_action_archive_tests {
    use super::*;
    use fasti_application::{
        OutboundAccessPolicy, ProviderId, ProviderStatePort, ReadSearchCandidateRequest,
        SearchCandidateActionCommand, SearchCandidateActionReceipt, SearchCandidateEvidenceMode,
        SearchPageRequest, SearchPersistencePort, SearchProviderQuery, SearchRecordAction,
    };
    use fasti_domain::SearchQuery;

    struct Fixture {
        archive: Vec<u8>,
        credential: SearchCandidateActionReceipt,
        historical_browser: SearchCandidateActionReceipt,
    }

    fn fixture() -> Fixture {
        let node = TestNode::new();
        grant_export(&node);
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, 'metadata_search')",
                [node.access.grant_id().to_string()],
            )
            .unwrap();
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                crate::search::tests::state(1),
            )
            .unwrap();
        let request = SearchPageRequest {
            correlation_id: RequestCorrelationId::new_v7(),
            access: node.access.into(),
            query: SearchProviderQuery::try_new(
                SearchQuery::try_new("Archive candidate").unwrap(),
                ProviderId::try_new("tmdb").unwrap(),
                1,
                None,
                None,
                vec![Grain::Film],
            )
            .unwrap(),
            outbound_policy: OutboundAccessPolicy::default(),
            terms_revision: "tmdb-v1".into(),
        };
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        let page = node
            .kernel
            .commit_search_page(
                &request,
                &prepared,
                &[crate::search::tests::candidate("42")],
                &Sha256Digest::from_bytes(&[7; 32]),
                None,
            )
            .unwrap();
        let command = SearchCandidateActionCommand {
            request: ReadSearchCandidateRequest {
                correlation_id: request.correlation_id,
                access: request.access,
                candidate_receipt_id: page.candidates[0].id(),
                provider: request.query.provider().clone(),
                grain: Grain::Film,
                outbound_policy: request.outbound_policy,
                terms_revision: request.terms_revision,
            },
            operation_id: OperationId::new_v7(),
            action: SearchRecordAction::Create,
            evidence_mode: SearchCandidateEvidenceMode::Cached,
        };
        let prepared = node
            .kernel
            .prepare_search_candidate_action(&command)
            .unwrap();
        let credential = node
            .kernel
            .commit_search_candidate_action(&command, &prepared, None)
            .unwrap();
        assert_eq!(credential.actor_subject_id, None);

        // This is an explicit historical portable audit row, not a sign-in
        // fixture. The old human subject no longer exists on this node.
        let mut historical_browser = credential.clone();
        historical_browser.operation_id = OperationId::new_v7();
        historical_browser.actor_subject_id = Some(AuthSubjectId::new_v7());
        historical_browser.disposition = fasti_application::SearchRecordActionDisposition::Reused;
        let json = serde_json::to_string(&historical_browser).unwrap();
        crate::search_actions::decode_receipt(&json, RequestCorrelationId::new_v7()).unwrap();
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM auth_subjects", [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            connection.execute(
                "INSERT INTO search_action_receipts(workspace_id, operation_id, profile_id, actor_client_id, actor_subject_id, record_id, semantic_digest, receipt_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![historical_browser.workspace_id.to_string(), historical_browser.operation_id.to_string(), historical_browser.profile_id.to_string(), historical_browser.actor_client_id.to_string(), historical_browser.actor_subject_id.unwrap().to_string(), historical_browser.record_id.to_string(), historical_browser.semantic_digest().to_string(), json],
            ).unwrap();
            for table in [
                "credentials",
                "profile_grants",
                "grant_scopes",
                "provider_capability_states",
                "search_pages",
                "search_candidate_receipts",
            ] {
                assert!(
                    connection
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                            .get::<_, i64>(0))
                        .unwrap()
                        > 0,
                    "source {table} must be populated to prove exclusion"
                );
            }
        }
        let destination = Arc::new(Mutex::new(DestinationState::default()));
        export_online_workspace_archive(
            &node.kernel,
            ExportWorkspaceRequest::new(
                ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), node.access),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(MemoryDestination(Arc::clone(&destination))),
        )
        .unwrap();
        let state = destination.lock().unwrap();
        assert!(state.completed && !state.aborted);
        Fixture {
            archive: state.bytes.clone(),
            credential,
            historical_browser,
        }
    }

    fn assert_receipts(database: &Connection, fixture: &Fixture) {
        assert_eq!(
            database
                .query_row("SELECT COUNT(*) FROM search_action_receipts", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
        for expected in [&fixture.credential, &fixture.historical_browser] {
            let (subject, json): (Option<String>, String) = database.query_row(
                "SELECT actor_subject_id, receipt_json FROM search_action_receipts WHERE operation_id = ?1",
                [expected.operation_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?)),
            ).unwrap();
            assert_eq!(subject, expected.actor_subject_id.map(|id| id.to_string()));
            assert_eq!(
                crate::search_actions::decode_receipt(&json, RequestCorrelationId::new_v7())
                    .unwrap(),
                *expected
            );
        }
        assert_eq!(
            database
                .query_row(NODE_LOCAL_STATE_COUNT_SQL, [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        for table in [
            "search_pages",
            "search_candidate_receipts",
            "provider_capability_states",
        ] {
            assert_eq!(
                database
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0,
                "node-local {table}"
            );
        }
    }

    #[test]
    fn archive_v6_search_actions_roundtrip_and_reexport_without_local_authority_or_candidates() {
        let fixture = fixture();
        let entries = archive_entries(&fixture.archive);
        let manifest_bytes = &entries.last().unwrap().1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(manifest_bytes, limits())
                .unwrap();
        assert_eq!(verified.manifest().format_version(), 6);
        assert_eq!(verified.manifest().streams().len(), 35);
        assert_eq!(
            verified.manifest().streams().last().unwrap().entity(),
            WorkspaceExportEntity::SearchActionReceipts
        );
        assert_eq!(verified.manifest().streams().last().unwrap().row_count(), 2);
        assert!(verified.manifest().blobs().is_empty());
        let restore_root = tempfile::tempdir().unwrap();
        let lock = LockedDataRoot::acquire(restore_root.path()).unwrap();
        let attempt = RestoreAttemptId::new_v7();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(&fixture.archive),
            attempt,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .unwrap();
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        assert_receipts(&database, &fixture);
        for statement in [
            "DELETE FROM search_action_receipts",
            "UPDATE search_action_receipts SET actor_subject_id = NULL",
        ] {
            assert!(
                database.execute(statement, []).is_err(),
                "receipt immutability survived restore"
            );
        }

        // Re-export every stream through the production portable stream owner.
        // The canonical original manifest is reusable only if all regenerated
        // descriptors and bytes are identical, which is asserted here.
        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024).unwrap();
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).unwrap();
        for (descriptor, (path, original)) in verified.manifest().streams().iter().zip(&entries) {
            let mut bytes = Vec::new();
            let actual = stream_archive_entity(
                &database,
                fixture.credential.workspace_id,
                descriptor.entity(),
                limits(),
                &mut bytes,
                &mut || Ok(()),
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
            assert_eq!(actual, *descriptor);
            assert_eq!(&bytes, original, "re-export {path}");
            writer
                .append(path, bytes.len() as u64, Cursor::new(bytes))
                .unwrap();
        }
        writer
            .append(
                "manifest.json",
                manifest_bytes.len() as u64,
                Cursor::new(manifest_bytes),
            )
            .unwrap();
        let reexport = writer.finish().unwrap();
        drop(database);
        staged.cleanup().unwrap();
        assert_attempt_removed(restore_root.path(), attempt);
        let second = RestoreAttemptId::new_v7();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(reexport),
            second,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .unwrap();
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        assert_receipts(&database, &fixture);
        drop(database);
        staged.cleanup().unwrap();
        assert_attempt_removed(restore_root.path(), second);
        assert!(!restore_root.path().join("current").exists());
    }

    #[test]
    fn archive_v6_search_actions_do_not_rebind_old_actor_receipts_to_recovery_identity() {
        let fixture = fixture();
        let root = tempfile::tempdir().unwrap();
        let adapter = crate::StoppedNodePortabilityAdapter::new(root.path());
        let attempt = RestoreAttemptId::new_v7();
        let workspace = fixture.credential.workspace_id;
        let profile = fixture.credential.profile_id;
        WorkspaceRestorePort::restore_workspace(
            &adapter,
            RestoreWorkspaceRequest::new(
                attempt,
                RequestCorrelationId::new_v7(),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(Cursor::new(fixture.archive.clone())),
        )
        .unwrap();
        let prepared = RecoveryBootstrapPort::prepare_recovery_bootstrap(
            &adapter,
            PrepareRecoveryBootstrapRequest::new(
                attempt,
                RequestCorrelationId::new_v7(),
                workspace,
                profile,
                false,
            ),
        )
        .unwrap();
        let completed = RecoveryBootstrapPort::complete_recovery_bootstrap(
            &adapter,
            CompleteRecoveryBootstrapRequest::new(
                attempt,
                RequestCorrelationId::new_v7(),
                workspace,
                profile,
                prepared.client_id(),
                SecretMaterial::from_bytes(*prepared.initialization_proof().expose_bytes()),
                SecretMaterial::from_bytes([31; 32]),
            ),
        )
        .unwrap();
        let kernel = crate::SqliteKernel::open(root.path()).unwrap();
        let access = kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                RequestCorrelationId::new_v7(),
                CapabilityKey::AttachIdentifier,
                SecretMaterial::from_bytes([31; 32]),
            ))
            .unwrap();
        assert_eq!(&access, completed.access());
        assert_ne!(access.client_id(), fixture.credential.actor_client_id);
        let before = {
            let connection = kernel.inner.connection.lock().unwrap();
            workspace_revision(&connection, &workspace.to_string()).unwrap()
        };
        for receipt in [&fixture.credential, &fixture.historical_browser] {
            let command = SearchCandidateActionCommand {
                request: ReadSearchCandidateRequest {
                    correlation_id: RequestCorrelationId::new_v7(),
                    access: access.into(),
                    candidate_receipt_id: receipt.candidate_receipt_id,
                    provider: ProviderId::try_new(&receipt.provider).unwrap(),
                    grain: receipt.grain,
                    outbound_policy: OutboundAccessPolicy::default(),
                    terms_revision: "tmdb-v1".into(),
                },
                operation_id: receipt.operation_id,
                action: receipt.action,
                evidence_mode: receipt.evidence_mode,
            };
            assert_eq!(
                kernel
                    .prepare_search_candidate_action(&command)
                    .unwrap_err()
                    .code(),
                fasti_application::ProblemCode::IdempotencyConflict
            );
            assert_eq!(
                kernel
                    .commit_search_candidate_action(
                        &command,
                        &fasti_application::SearchCandidateActionPreparation::Replay(Box::new(
                            receipt.clone()
                        )),
                        None,
                    )
                    .unwrap_err()
                    .code(),
                fasti_application::ProblemCode::IdempotencyConflict
            );
            let connection = kernel.inner.connection.lock().unwrap();
            let json: String = connection
                .query_row(
                    "SELECT receipt_json FROM search_action_receipts WHERE operation_id = ?1",
                    [receipt.operation_id.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                crate::search_actions::decode_receipt(&json, RequestCorrelationId::new_v7())
                    .unwrap(),
                *receipt
            );
        }
        let connection = kernel.inner.connection.lock().unwrap();
        assert_eq!(
            workspace_revision(&connection, &workspace.to_string()).unwrap(),
            before
        );
        for table in [
            "auth_subjects",
            "search_pages",
            "search_candidate_receipts",
            "provider_capability_states",
        ] {
            assert_eq!(
                connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0,
                "recovery must not recreate {table}"
            );
        }
    }

    fn reject(archive: Vec<u8>, case: &str) {
        let root = tempfile::tempdir().unwrap();
        let lock = LockedDataRoot::acquire(root.path()).unwrap();
        let attempt = RestoreAttemptId::new_v7();
        let error = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(archive),
            attempt,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .unwrap_or_else(|| panic!("accepted hostile {case}"));
        assert!(
            matches!(
                error,
                RestoreImportError::DomainInvariant
                    | RestoreImportError::AggregateInvariant
                    | RestoreImportError::InvalidRow { .. }
                    | RestoreImportError::NonCanonicalRow { .. }
                    | RestoreImportError::RowOrder { .. }
                    | RestoreImportError::RowInvariant { .. }
            ),
            "unexpected {case}: {error:?}"
        );
        assert_attempt_removed(root.path(), attempt);
        assert!(!root.path().join("current").exists());
    }

    #[test]
    fn archive_v6_search_actions_reject_typed_canonical_column_and_duplicate_corruption() {
        let fixture = fixture();
        for case in [
            "subject_type",
            "unknown_column",
            "workspace_column",
            "profile_column",
            "client_column",
            "subject_column",
            "operation_column",
            "record_column",
            "digest_column",
            "receipt_oversize",
            "receipt_noncanonical",
            "receipt_unknown",
            "receipt_provider",
            "receipt_status",
            "receipt_target",
            "duplicate",
        ] {
            let hostile = rewrite_stream(
                &fixture.archive,
                WorkspaceExportEntity::SearchActionReceipts,
                |bytes| {
                    let mut rows: Vec<serde_json::Value> = bytes
                        .split(|byte| *byte == b'\n')
                        .filter(|line| !line.is_empty())
                        .map(|line| serde_json::from_slice(line).unwrap())
                        .collect();
                    let row = &mut rows[0];
                    match case {
                        "subject_type" => {
                            row["actor_subject_id"] = serde_json::json!("invalid-subject")
                        }
                        "unknown_column" => row["unexpected"] = serde_json::json!(true),
                        "workspace_column" => {
                            row["workspace_id"] =
                                serde_json::json!(WorkspaceId::new_v7().to_string())
                        }
                        "profile_column" => {
                            row["profile_id"] = serde_json::json!(ProfileId::new_v7().to_string())
                        }
                        "client_column" => {
                            row["actor_client_id"] =
                                serde_json::json!(ClientId::new_v7().to_string())
                        }
                        "subject_column" => {
                            row["actor_subject_id"] =
                                serde_json::json!(AuthSubjectId::new_v7().to_string())
                        }
                        "operation_column" => {
                            row["operation_id"] =
                                serde_json::json!(OperationId::new_v7().to_string())
                        }
                        "record_column" => {
                            row["record_id"] = serde_json::json!(RecordId::new_v7().to_string())
                        }
                        "digest_column" => {
                            row["semantic_digest"] =
                                serde_json::json!(Sha256Digest::from_bytes(&[0; 32]).to_string())
                        }
                        "duplicate" => {}
                        _ => {
                            let original = row["receipt_json"].as_str().unwrap();
                            let mut receipt: SearchCandidateActionReceipt =
                                serde_json::from_str(original).unwrap();
                            let changed = match case {
                                "receipt_oversize" => {
                                    format!("{original}{}", " ".repeat(16 * 1024))
                                }
                                "receipt_noncanonical" => {
                                    serde_json::to_string_pretty(&receipt).unwrap()
                                }
                                "receipt_unknown" => {
                                    format!("{{\"unknown\":true,{}", &original[1..])
                                }
                                "receipt_provider" => {
                                    receipt.provider = "google_books".into();
                                    serde_json::to_string(&receipt).unwrap()
                                }
                                "receipt_status" => {
                                    receipt.initial_status = FieldClaimStatus::Invalid;
                                    serde_json::to_string(&receipt).unwrap()
                                }
                                "receipt_target" => {
                                    receipt.action = SearchRecordAction::Attach(RecordId::new_v7());
                                    serde_json::to_string(&receipt).unwrap()
                                }
                                _ => unreachable!(),
                            };
                            row["receipt_json"] = serde_json::json!(changed);
                        }
                    }
                    if case == "duplicate" {
                        rows.insert(0, rows[0].clone());
                    }
                    bytes.clear();
                    for row in rows {
                        bytes.extend(serde_json::to_vec(&row).unwrap());
                        bytes.push(b'\n');
                    }
                },
            );
            reject(hostile, case);
        }
    }

    #[test]
    fn archive_v6_search_actions_require_portable_profile_client_and_record_relations() {
        let fixture = fixture();
        for case in ["profile", "client", "record", "record_grain"] {
            let hostile = rewrite_stream(
                &fixture.archive,
                WorkspaceExportEntity::SearchActionReceipts,
                |bytes| {
                    let mut rewritten = Vec::new();
                    for (index, line) in bytes
                        .split(|byte| *byte == b'\n')
                        .filter(|line| !line.is_empty())
                        .enumerate()
                    {
                        let mut row: serde_json::Value = serde_json::from_slice(line).unwrap();
                        if index == 0 {
                            let mut receipt: SearchCandidateActionReceipt =
                                serde_json::from_str(row["receipt_json"].as_str().unwrap())
                                    .unwrap();
                            match case {
                                "profile" => {
                                    receipt.profile_id = ProfileId::new_v7();
                                    row["profile_id"] =
                                        serde_json::json!(receipt.profile_id.to_string());
                                }
                                "client" => {
                                    receipt.actor_client_id = ClientId::new_v7();
                                    row["actor_client_id"] =
                                        serde_json::json!(receipt.actor_client_id.to_string());
                                }
                                "record" => {
                                    receipt.record_id = RecordId::new_v7();
                                    row["record_id"] =
                                        serde_json::json!(receipt.record_id.to_string());
                                }
                                "record_grain" => {
                                    receipt.grain = Grain::Series;
                                    receipt.provenance = FieldClaimProvenance::try_new(
                                        MetadataProviderId::try_new("tmdb").unwrap(),
                                        NamespaceKey::try_new("tmdb.tv").unwrap(),
                                        "42",
                                        receipt.provenance.locale().cloned(),
                                        None,
                                        None,
                                        receipt.search_response_digest.clone(),
                                    )
                                    .unwrap();
                                }
                                _ => unreachable!(),
                            }
                            row["receipt_json"] =
                                serde_json::json!(serde_json::to_string(&receipt).unwrap());
                            row["semantic_digest"] =
                                serde_json::json!(receipt.semantic_digest().to_string());
                        }
                        rewritten.extend(serde_json::to_vec(&row).unwrap());
                        rewritten.push(b'\n');
                    }
                    *bytes = rewritten;
                },
            );
            reject(hostile, case);
        }
    }
}
