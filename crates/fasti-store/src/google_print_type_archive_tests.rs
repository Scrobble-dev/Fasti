mod google_print_type_archive_tests {
    use super::*;
    use crate::metadata::load_field_claims;
    use fasti_application::{
        GoogleBooksPrintType, SearchPersistencePort, GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
    };
    use fasti_domain::{FieldClaimStatus, FieldKey};

    #[test]
    fn archive_v7_google_print_type_roundtrips_and_reexports_without_search_cache() {
        for value in [
            GoogleBooksPrintType::Book,
            GoogleBooksPrintType::Magazine,
            GoogleBooksPrintType::Unknown,
        ] {
            let (node, command) =
                crate::search::tests::google_print_type_tests::google_fixture(value);
            let snapshot = node
                .kernel
                .read_search_candidate(&command.request)
                .unwrap()
                .unwrap();
            let prepared = node
                .kernel
                .prepare_search_candidate_action(&command)
                .unwrap();
            let receipt = node
                .kernel
                .commit_search_candidate_action(&command, &prepared, None)
                .unwrap();
            let key = FieldKey::try_new(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY).unwrap();
            let original = {
                let connection = node.kernel.inner.connection.lock().unwrap();
                for table in ["search_pages", "search_candidate_receipts"] {
                    assert!(
                        connection
                            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                                row.get::<_, i64>(0)
                            })
                            .unwrap()
                            > 0,
                        "source {table} must exist to prove exclusion"
                    );
                }
                let claims = load_field_claims(
                    &connection,
                    node.access.workspace_id(),
                    receipt.record_id,
                    &key,
                    CapabilityKey::SearchMetadata,
                    RequestCorrelationId::new_v7(),
                    crate::kernel::now(),
                )
                .unwrap();
                assert_eq!(claims.len(), 1);
                let claim = &claims[0];
                assert_eq!(claim.value(), value.as_str());
                assert_eq!(claim.record_id(), Some(receipt.record_id));
                assert_eq!(claim.field_key(), Some(&key));
                assert_eq!(claim.fetched_at(), snapshot.receipt.lifetime().created_at());
                assert_eq!(
                    claim.expires_at(),
                    Some(snapshot.receipt.lifetime().fresh_until())
                );
                assert_eq!(claim.initial_status(), FieldClaimStatus::Fresh);
                assert!(!claim.is_fresh(crate::kernel::now()));
                assert!(claim.provenance().is_complete());
                assert_eq!(
                    claim.provenance().provider_id().unwrap().as_str(),
                    "google-books"
                );
                assert_eq!(claim.source().as_str(), "googlebooks.volume");
                assert_eq!(claim.provenance().source_identifier(), Some("42"));
                assert_eq!(
                    claim.provenance().evidence_digest(),
                    Some(snapshot.receipt.response_digest())
                );
                claims
            };
            grant_export(&node);
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
            let archive = {
                let state = destination.lock().unwrap();
                assert!(state.completed && !state.aborted);
                state.bytes.clone()
            };
            let entries = archive_entries(&archive);
            assert!(entries.iter().all(|(path, _)| {
                path != "search_pages.ndjson" && path != "search_candidate_receipts.ndjson"
            }));
            let manifest_bytes = &entries.last().unwrap().1;
            let verified =
                VerifiedInboundWorkspaceManifest::try_from_canonical_json(manifest_bytes, limits())
                    .unwrap();
            assert_eq!(verified.manifest().format_version(), 7);
            assert_eq!(verified.manifest().streams().len(), 35);
            assert!(verified.manifest().blobs().is_empty());

            let restore_root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(restore_root.path()).unwrap();
            let attempt = RestoreAttemptId::new_v7();
            let staged = stage_workspace_archive_pass_two(
                &lock,
                &mut Cursor::new(&archive),
                attempt,
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
            let restored = load_field_claims(
                &database,
                receipt.workspace_id,
                receipt.record_id,
                &key,
                CapabilityKey::SearchMetadata,
                RequestCorrelationId::new_v7(),
                crate::kernel::now(),
            )
            .unwrap();
            // Claim IDs, bound identity, full provenance, lifecycle and original
            // observation lifetime survive; this is not a fresh observation.
            assert_eq!(restored, original);
            for table in ["search_pages", "search_candidate_receipts"] {
                assert_eq!(
                    database
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                            row.get::<_, i64>(0)
                        })
                        .unwrap(),
                    0,
                    "restored {table} remains node-local"
                );
            }
            assert_eq!(
                database
                    .query_row(NODE_LOCAL_STATE_COUNT_SQL, [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                0
            );

            // Reuse the portable stream owner for canonical re-export, including
            // the metadata registry's existing response-policy representation.
            let archive_limits =
                ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                    .unwrap();
            let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).unwrap();
            for (descriptor, (path, original_bytes)) in
                verified.manifest().streams().iter().zip(&entries)
            {
                let mut bytes = Vec::new();
                let actual = stream_archive_entity(
                    &database,
                    receipt.workspace_id,
                    descriptor.entity(),
                    verified.manifest().format_version(),
                    limits(),
                    &mut bytes,
                    &mut || Ok(()),
                    RequestCorrelationId::new_v7(),
                )
                .unwrap();
                assert_eq!(actual, *descriptor);
                assert_eq!(&bytes, original_bytes, "re-export {path}");
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
            assert_eq!(archive_entries(&writer.finish().unwrap()), entries);
            drop(database);
            staged.cleanup().unwrap();
            assert_attempt_removed(restore_root.path(), attempt);
            assert!(!restore_root.path().join("current").exists());
        }
    }
}
