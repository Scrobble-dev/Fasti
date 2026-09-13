mod kitsu_archive_tests {
    use super::*;
    use crate::metadata::load_field_claims;
    use fasti_application::{
        KitsuMangaSubtype, SearchPersistencePort, KITSU_MANGA_SUBTYPE_FIELD_KEY,
    };
    use fasti_domain::{FieldClaimStatus, FieldKey};

    #[test]
    fn archive_v7_kitsu_positive_negative_and_unknown_subtypes_keep_native_selection() {
        for subtype in [
            KitsuMangaSubtype::Manga,
            KitsuMangaSubtype::Novel,
            KitsuMangaSubtype::Unknown,
        ] {
            let (node, command) = crate::search::tests::kitsu_tests::fixture(subtype);
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
            let key = FieldKey::try_new(KITSU_MANGA_SUBTYPE_FIELD_KEY).unwrap();
            assert!(fasti_application::is_native_publication_field(key.as_str()));
            let original = {
                let connection = node.kernel.inner.connection.lock().unwrap();
                for table in ["search_pages", "search_candidate_receipts"] {
                    assert!(
                        connection
                            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                                .get::<_, i64>(0))
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
                assert_eq!(claim.value(), subtype.as_str());
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
                assert_eq!(claim.provenance().provider_id().unwrap().as_str(), "kitsu");
                assert_eq!(claim.source().as_str(), "kitsu.manga");
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
            assert!(entries.iter().all(|(path, _)| path != "search_pages.ndjson"
                && path != "search_candidate_receipts.ndjson"));
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
            // The production native loader must select the same historical
            // observation, including explicit novel/unknown evidence.
            assert_eq!(restored, original);
            assert_eq!(
                KitsuMangaSubtype::parse_claim(restored[0].value()),
                Some(subtype)
            );
            let stored_policy: String = database
                .query_row(
                    "SELECT response_policy_json FROM metadata_claims WHERE claim_id = ?1",
                    [restored[0].claim_id().to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(stored_policy, snapshot.response_policy.to_canonical_json());
            let identity: (String, String, String) = database.query_row(
                "SELECT r.grain, e.namespace, e.value FROM records r JOIN external_identifiers e ON e.record_id = r.record_id WHERE r.record_id = ?1",
                [receipt.record_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
            assert_eq!(identity, ("work".into(), "kitsu.manga".into(), "42".into()));
            for table in ["search_pages", "search_candidate_receipts"] {
                assert_eq!(
                    database
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                            .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
            }
            assert_eq!(
                database
                    .query_row(NODE_LOCAL_STATE_COUNT_SQL, [], |row| row.get::<_, i64>(0))
                    .unwrap(),
                0
            );

            // Canonical re-export covers every portable stream, including claim
            // lifecycle, response policy, identity and completed action receipt.
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
            }
            drop(database);
            staged.cleanup().unwrap();
            assert_attempt_removed(restore_root.path(), attempt);
            assert!(!restore_root.path().join("current").exists());
        }
    }
}
