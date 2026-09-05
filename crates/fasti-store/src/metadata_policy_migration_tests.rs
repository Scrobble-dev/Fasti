mod metadata_policy_migration_tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use fasti_application::{
        CapabilityKey, ProviderResponseCachePolicy, ProviderResponseReuse,
        RefreshMetadataClaimsOutcome, SearchCandidateActionReceipt, SearchCandidateEvidenceMode,
        SearchRecordAction, SearchRecordActionDisposition,
    };
    use fasti_domain::{
        ClientId, FieldClaimProvenance, FieldClaimStatus, MetadataProviderId, NamespaceKey,
        OperationId, ProfileId, RecordId, RequestCorrelationId, SearchCandidateReceiptId,
        Sha256Digest, WorkspaceId,
    };
    use rusqlite::types::Value;
    use std::time::Duration as StdDuration;

    struct Fixture {
        connection: Connection,
        workspace: WorkspaceId,
        record: RecordId,
        claim: MetadataClaimId,
    }

    fn populated_v16() -> Fixture {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        migrate_to_version_fourteen(&connection);
        migrate_v15(&connection).unwrap();
        migrate_v16(&connection).unwrap();
        let workspace = WorkspaceId::new_v7();
        let profile = ProfileId::new_v7();
        let client = ClientId::new_v7();
        let record = RecordId::new_v7();
        let claim = MetadataClaimId::new_v7();
        let fetched = DateTime::parse_from_rfc3339(CREATED_AT)
            .unwrap()
            .with_timezone(&Utc);
        let expires = crate::kernel::timestamp(fetched + Duration::seconds(120));
        let digest = Sha256Digest::from_bytes(&[7; 32]);
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id,created_at) VALUES (?1,?2)",
                params![workspace.to_string(), CREATED_AT],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO profiles(profile_id,workspace_id,created_at) VALUES (?1,?2,?3)",
                params![profile.to_string(), workspace.to_string(), CREATED_AT],
            )
            .unwrap();
        connection.execute("INSERT INTO clients(client_id,workspace_id,status,current_credential_epoch,created_at) VALUES (?1,?2,'active',1,?3)", params![client.to_string(), workspace.to_string(), CREATED_AT]).unwrap();
        connection.execute("INSERT INTO records(record_id,workspace_id,grain,status,created_at) VALUES (?1,?2,'film','active',?3)", params![record.to_string(), workspace.to_string(), CREATED_AT]).unwrap();
        connection.execute("INSERT INTO namespace_definitions(workspace_id,namespace,label,supported_grains,id_pattern,normalization,licence_posture,created_at) VALUES (?1,'tmdb.movie','TMDB movies','film','[0-9]+','identity','identifiers_only',?2)", params![workspace.to_string(), CREATED_AT]).unwrap();
        connection.execute("INSERT INTO external_identifiers(external_identifier_id,workspace_id,record_id,namespace,grain,value,created_at) VALUES (?1,?2,?3,'tmdb.movie','film','42',?4)", params![fasti_domain::ExternalIdentifierId::new_v7().to_string(), workspace.to_string(), record.to_string(), CREATED_AT]).unwrap();
        // Exact old-column SQL is intentional: this is a populated v16
        // database, not a current TestNode with its user_version relabeled.
        connection.execute("INSERT INTO metadata_field_claims(workspace_id,record_id,field_key,source,value,fetched_at,expires_at,created_at) VALUES (?1,?2,'core.title','tmdb.movie','Preserved historical title',?3,?4,?3)", params![workspace.to_string(), record.to_string(), CREATED_AT, expires]).unwrap();
        connection.execute("INSERT INTO metadata_claims(claim_id,workspace_id,record_id,claim_kind,created_at) VALUES (?1,?2,?3,'field',?4)", params![claim.to_string(), workspace.to_string(), record.to_string(), CREATED_AT]).unwrap();
        connection.execute("INSERT INTO metadata_claim_provenance(claim_id,workspace_id,record_id,field_key,source,fetched_at,provider_id,source_record_id,evidence_digest,classification,provenance_state,initial_status,created_at) VALUES (?1,?2,?3,'core.title','tmdb.movie',?4,'tmdb','42',?5,'internal','complete','fresh',?4)", params![claim.to_string(), workspace.to_string(), record.to_string(), CREATED_AT, digest.as_str()]).unwrap();
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").unwrap(),
            NamespaceKey::try_new("tmdb.movie").unwrap(),
            "42",
            None,
            None,
            None,
            digest.clone(),
        )
        .unwrap();
        let receipt = SearchCandidateActionReceipt {
            workspace_id: workspace,
            profile_id: profile,
            actor_client_id: client,
            actor_subject_id: None,
            operation_id: OperationId::new_v7(),
            candidate_receipt_id: SearchCandidateReceiptId::new_v7(),
            provider: "tmdb".into(),
            grain: Grain::Film,
            action: SearchRecordAction::Create,
            evidence_mode: SearchCandidateEvidenceMode::Cached,
            record_id: record,
            disposition: SearchRecordActionDisposition::Created,
            search_context_digest: Sha256Digest::from_bytes(&[8; 32]),
            search_response_digest: digest.clone(),
            provenance,
            fetched_at: fetched,
            expires_at: Some(fetched + Duration::seconds(120)),
            initial_status: FieldClaimStatus::Fresh,
            committed_at: fetched + Duration::seconds(1),
        };
        let receipt_json = serde_json::to_string(&receipt).unwrap();
        crate::search_actions::decode_receipt(&receipt_json, RequestCorrelationId::new_v7())
            .unwrap();
        connection.execute("INSERT INTO search_action_receipts(workspace_id,operation_id,profile_id,actor_client_id,actor_subject_id,record_id,semantic_digest,receipt_json) VALUES (?1,?2,?3,?4,NULL,?5,?6,?7)", params![workspace.to_string(), receipt.operation_id.to_string(), profile.to_string(), client.to_string(), record.to_string(), receipt.semantic_digest().as_str(), receipt_json]).unwrap();
        let refresh = crate::metadata::encode_refresh_receipt_outcome(
            record,
            &MetadataProviderId::try_new("tmdb").unwrap(),
            &RefreshMetadataClaimsOutcome::new(vec![], vec![], vec![], vec![], vec![]),
            CapabilityKey::ExportWorkspace,
            RequestCorrelationId::new_v7(),
        )
        .unwrap();
        connection.execute("INSERT INTO metadata_refresh_receipts(workspace_id,profile_id,client_id,operation_id,semantic_digest,record_id,provider_id,response_json,created_at) VALUES (?1,?2,?3,?4,?5,?6,'tmdb',?7,?8)", params![workspace.to_string(), profile.to_string(), client.to_string(), OperationId::new_v7().to_string(), digest.as_str(), record.to_string(), refresh, CREATED_AT]).unwrap();
        connection.execute("INSERT INTO metadata_projection_policies(workspace_id,profile_id,preferred_provider_id,preferred_locale,enabled_field_groups,allow_english_fallback,last_known_good_policy,updated_at) VALUES (?1,?2,'tmdb','fr-fr','[]',1,'allow',?3)", params![workspace.to_string(), profile.to_string(), CREATED_AT]).unwrap();
        connection.execute("INSERT INTO metadata_profile_field_overrides(workspace_id,profile_id,record_id,field_key,value,created_at,updated_at,origin) VALUES (?1,?2,?3,'core.title','Private preserved title',?4,?4,'user')", params![workspace.to_string(), profile.to_string(), record.to_string(), CREATED_AT]).unwrap();
        Fixture {
            connection,
            workspace,
            record,
            claim,
        }
    }

    fn rows(connection: &Connection, sql: &str) -> Vec<Vec<Value>> {
        let mut statement = connection.prepare(sql).unwrap();
        let columns = statement.column_count();
        statement
            .query_map([], |row| {
                (0..columns).map(|column| row.get(column)).collect()
            })
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap()
    }

    fn preserved_rows(connection: &Connection) -> Vec<Vec<Vec<Value>>> {
        [
            "SELECT * FROM records ORDER BY record_id",
            "SELECT * FROM external_identifiers ORDER BY external_identifier_id",
            "SELECT * FROM metadata_field_claims ORDER BY record_id,field_key,source,fetched_at",
            "SELECT claim_id,workspace_id,record_id,claim_kind,created_at FROM metadata_claims ORDER BY claim_id",
            "SELECT * FROM metadata_claim_provenance ORDER BY claim_id",
            "SELECT * FROM metadata_refresh_receipts ORDER BY operation_id",
            "SELECT * FROM search_action_receipts ORDER BY operation_id",
            "SELECT * FROM workspace_revisions ORDER BY workspace_id",
            "SELECT * FROM metadata_projection_policies ORDER BY profile_id",
            "SELECT * FROM metadata_profile_field_overrides ORDER BY profile_id,record_id,field_key",
        ].map(|sql| rows(connection, sql)).into()
    }

    #[cfg(target_os = "linux")]
    fn assert_archive_roundtrip(fixture: &Fixture, format: u32) {
        use crate::archive::{ArchiveLimits, ArchiveWriter};
        use crate::kernel::LockedDataRoot;
        use crate::portability::{schema_fingerprint, stream_archive_entity};
        use crate::restore_import::stage_workspace_archive_pass_two;
        use fasti_application::{
            CancellationSignal, PortabilityLimits, WorkspaceExportEntity, WorkspaceManifest,
            WORKSPACE_ARCHIVE_CONTRACT_VERSION,
        };
        use fasti_contracts::CanonicalWorkspaceManifestProjection;
        use fasti_domain::RestoreAttemptId;
        use std::{io::Cursor, num::NonZeroU64};

        let nonzero = |value| NonZeroU64::new(value).unwrap();
        let limits = PortabilityLimits {
            max_snapshot_bytes: nonzero(32 * 1024 * 1024),
            max_wal_growth_bytes: nonzero(8 * 1024 * 1024),
            max_archive_bytes: nonzero(64 * 1024 * 1024),
            max_uncompressed_bytes: nonzero(32 * 1024 * 1024),
            max_entry_bytes: nonzero(8 * 1024 * 1024),
            max_entries: nonzero(64),
            max_rows_per_stream: nonzero(1024),
            max_path_bytes: nonzero(100),
            max_path_depth: nonzero(8),
            max_decompression_ratio: nonzero(1024),
            scratch_ceiling_bytes: nonzero(64 * 1024 * 1024),
            cleanup_reserve_bytes: nonzero(1024 * 1024),
            backup_step_pages: nonzero(64),
            backup_step_millis: nonzero(1000),
        };
        let id = RequestCorrelationId::new_v7();
        let fingerprint = schema_fingerprint(&fixture.connection, id).unwrap();
        assert_eq!(
            fingerprint.migration_version(),
            if format == 6 { 16 } else { 17 }
        );
        let revision = u64::try_from(
            workspace_revision(&fixture.connection, &fixture.workspace.to_string()).unwrap(),
        )
        .unwrap();
        let mut writer = ArchiveWriter::new(
            Vec::new(),
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024).unwrap(),
        )
        .unwrap();
        let mut streams = Vec::new();
        for &entity in WorkspaceExportEntity::for_format(format).unwrap() {
            let mut bytes = Vec::new();
            let descriptor = stream_archive_entity(
                &fixture.connection,
                fixture.workspace,
                entity,
                format,
                limits,
                &mut bytes,
                &mut || Ok(()),
                id,
            )
            .unwrap();
            if entity == WorkspaceExportEntity::MetadataClaims {
                for line in bytes
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                {
                    let value: serde_json::Value = serde_json::from_slice(line).unwrap();
                    assert_eq!(value.get("response_policy_json").is_some(), format == 7);
                }
            }
            writer
                .append(
                    &format!("{}.ndjson", entity.as_str()),
                    bytes.len() as u64,
                    Cursor::new(&bytes),
                )
                .unwrap();
            streams.push((entity, bytes, descriptor));
        }
        assert_eq!(streams.len(), 35);
        let manifest = WorkspaceManifest::try_new_for_format(
            format,
            fixture.workspace,
            revision,
            WORKSPACE_ARCHIVE_CONTRACT_VERSION.to_owned(),
            fingerprint.migration_version(),
            fingerprint.digest().clone(),
            streams
                .iter()
                .map(|(_, _, descriptor)| descriptor.clone())
                .collect(),
            Vec::new(),
        )
        .unwrap();
        let projection =
            CanonicalWorkspaceManifestProjection::try_from_application(manifest).unwrap();
        let manifest = projection.canonical_json_bytes();
        writer
            .append(
                "manifest.json",
                manifest.len() as u64,
                Cursor::new(manifest),
            )
            .unwrap();
        let archive = writer.finish().unwrap();
        let root = tempfile::tempdir().unwrap();
        let lock = LockedDataRoot::acquire(root.path()).unwrap();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(archive),
            RestoreAttemptId::new_v7(),
            id,
            limits,
            &CancellationSignal::new(),
        )
        .unwrap();
        let restored = Connection::open_with_flags(
            staged.database_path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        assert_eq!(
            restored
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            17
        );
        assert_eq!(
            preserved_rows(&restored),
            preserved_rows(&fixture.connection)
        );
        if format == 6 {
            assert_eq!(
                rows(
                    &restored,
                    "SELECT response_policy_json FROM metadata_claims"
                ),
                vec![vec![Value::Null]]
            );
        } else {
            assert_eq!(
                rows(
                    &restored,
                    "SELECT claim_id,response_policy_json FROM metadata_claims ORDER BY claim_id"
                ),
                rows(
                    &fixture.connection,
                    "SELECT claim_id,response_policy_json FROM metadata_claims ORDER BY claim_id"
                )
            );
        }
        // Re-emit the original version, not merely the latest format. This
        // catches accidental insertion of a new null key into frozen v6 bytes.
        for (entity, expected_bytes, expected_descriptor) in streams {
            let mut actual = Vec::new();
            let descriptor = stream_archive_entity(
                &restored,
                fixture.workspace,
                entity,
                format,
                limits,
                &mut actual,
                &mut || Ok(()),
                id,
            )
            .unwrap();
            assert_eq!(
                descriptor,
                expected_descriptor,
                "{} descriptor",
                entity.as_str()
            );
            assert_eq!(actual, expected_bytes, "{} bytes", entity.as_str());
        }
        assert!(rows(&restored, "PRAGMA foreign_key_check").is_empty());
        for table in [
            "search_pages",
            "search_candidate_receipts",
            "profile_grants",
            "credentials",
            "fasti_browser_sessions",
        ] {
            assert_eq!(
                restored
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                        .get::<_, i64>(0))
                    .unwrap(),
                0,
                "node-local {table}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn metadata_policy_migration_genuine_v16_archive_v6_restores_original_bytes_into_v17() {
        assert_archive_roundtrip(&populated_v16(), 6);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn metadata_policy_migration_v7_roundtrip_preserves_null_and_observation_bound_policy() {
        let fixture = populated_v16();
        migrate_v17(&fixture.connection).unwrap();
        // Preserve the old NULL row and insert a distinct, complete claim with
        // original observation time. Do not rewrite immutable historical claims.
        let claim = MetadataClaimId::new_v7();
        let observed = DateTime::parse_from_rfc3339(CREATED_AT)
            .unwrap()
            .with_timezone(&Utc);
        let expires = crate::kernel::timestamp(observed + Duration::seconds(120));
        let policy = policy(ProviderResponseReuse::Reusable);
        fixture.connection.execute("INSERT INTO metadata_field_claims(workspace_id,record_id,field_key,source,value,fetched_at,expires_at,created_at) VALUES (?1,?2,'core.original_title','tmdb.movie','Original preserved title',?3,?4,?3)", params![fixture.workspace.to_string(), fixture.record.to_string(), CREATED_AT, expires]).unwrap();
        fixture.connection.execute("INSERT INTO metadata_claims(claim_id,workspace_id,record_id,claim_kind,created_at,response_policy_json) VALUES (?1,?2,?3,'field',?4,?5)", params![claim.to_string(), fixture.workspace.to_string(), fixture.record.to_string(), CREATED_AT, policy]).unwrap();
        fixture.connection.execute("INSERT INTO metadata_claim_provenance(claim_id,workspace_id,record_id,field_key,source,fetched_at,provider_id,source_record_id,evidence_digest,classification,provenance_state,initial_status,created_at) VALUES (?1,?2,?3,'core.original_title','tmdb.movie',?4,'tmdb','42',?5,'internal','complete','fresh',?4)", params![claim.to_string(), fixture.workspace.to_string(), fixture.record.to_string(), CREATED_AT, Sha256Digest::from_bytes(&[7; 32]).as_str()]).unwrap();
        assert_eq!(
            fixture
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM metadata_claims WHERE response_policy_json IS NULL",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            fixture
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM metadata_claims WHERE response_policy_json IS NOT NULL",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            1
        );
        assert_archive_roundtrip(&fixture, 7);
    }

    fn claim_columns(connection: &Connection) -> Vec<Vec<Value>> {
        rows(connection, "PRAGMA table_info(metadata_claims)")
    }

    fn policy(reuse: ProviderResponseReuse) -> String {
        serde_json::to_string(&ProviderResponseCachePolicy::new(
            reuse,
            DateTime::parse_from_rfc3339(CREATED_AT)
                .unwrap()
                .with_timezone(&Utc),
            StdDuration::ZERO,
            None,
            None,
        ))
        .unwrap()
    }

    fn insert_policy(fixture: &Fixture, value: Option<&str>) -> Result<usize> {
        fixture.connection.execute(
            "INSERT INTO metadata_claims(claim_id,workspace_id,record_id,claim_kind,created_at,response_policy_json) VALUES (?1,?2,?3,'field',?4,?5)",
            params![MetadataClaimId::new_v7().to_string(), fixture.workspace.to_string(), fixture.record.to_string(), CREATED_AT, value],
        )
    }

    #[test]
    fn metadata_policy_migration_populated_v16_preserves_claims_receipts_and_reopen_idempotence() {
        let fixture = populated_v16();
        let connection = &fixture.connection;
        let fingerprint =
            crate::portability::schema_fingerprint(connection, RequestCorrelationId::new_v7())
                .unwrap();
        assert_eq!(fingerprint.migration_version(), 16);
        assert_eq!(
            fingerprint.digest().as_str(),
            "sha256:d7ae3b1ab15c0223245d1a9008833049e58e9ec882a6e1ba70a2a080fa3fd7a6"
        );
        let before = preserved_rows(connection);
        assert!(before.iter().all(|table| !table.is_empty()));
        let columns = claim_columns(connection);
        assert_eq!(columns.len(), 5);
        let stable_schema = rows(connection, "SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' AND name != 'metadata_claims' ORDER BY type,name");
        migrate(connection).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            17
        );
        assert_eq!(preserved_rows(connection), before);
        let upgraded_columns = claim_columns(connection);
        assert_eq!(&upgraded_columns[..5], columns.as_slice());
        assert_eq!(upgraded_columns.len(), 6);
        assert_eq!(
            upgraded_columns[5][1],
            Value::Text("response_policy_json".into())
        );
        assert_eq!(upgraded_columns[5][3], Value::Integer(0));
        assert_eq!(
            connection
                .query_row(
                    "SELECT response_policy_json FROM metadata_claims WHERE claim_id = ?1",
                    [fixture.claim.to_string()],
                    |row| row.get::<_, Option<String>>(0)
                )
                .unwrap(),
            None
        );
        assert_eq!(rows(connection, "SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%' AND name != 'metadata_claims' ORDER BY type,name"), stable_schema);
        assert!(rows(connection, "PRAGMA foreign_key_check").is_empty());
        assert_eq!(
            rows(connection, "PRAGMA integrity_check"),
            vec![vec![Value::Text("ok".into())]]
        );
        let upgraded_fingerprint =
            crate::portability::schema_fingerprint(connection, RequestCorrelationId::new_v7())
                .unwrap();
        migrate(connection).unwrap();
        assert_eq!(preserved_rows(connection), before);
        assert_eq!(claim_columns(connection), upgraded_columns);
        assert_eq!(
            crate::portability::schema_fingerprint(connection, RequestCorrelationId::new_v7())
                .unwrap(),
            upgraded_fingerprint
        );
    }

    #[test]
    fn metadata_policy_migration_sql_accepts_null_and_all_three_durable_modes() {
        let fixture = populated_v16();
        migrate_v17(&fixture.connection).unwrap();
        assert_eq!(insert_policy(&fixture, None).unwrap(), 1);
        for reuse in [
            ProviderResponseReuse::Reusable,
            ProviderResponseReuse::ValidateWhenStale,
            ProviderResponseReuse::ValidateEveryReuse,
        ] {
            assert_eq!(insert_policy(&fixture, Some(&policy(reuse))).unwrap(), 1);
        }
        assert_eq!(
            rows(&fixture.connection, "SELECT * FROM metadata_claims").len(),
            5
        );
        assert!(rows(&fixture.connection, "PRAGMA foreign_key_check").is_empty());
    }

    #[test]
    fn metadata_policy_migration_sql_rejects_no_store_malformed_shapes_and_oversized_bytes() {
        let fixture = populated_v16();
        migrate_v17(&fixture.connection).unwrap();
        let valid = policy(ProviderResponseReuse::Reusable);
        let mut invalid = vec![
            policy(ProviderResponseReuse::NoStore),
            "{".into(),
            "null".into(),
            "[]".into(),
            "true".into(),
            "{}".into(),
        ];
        for (key, replacement) in [
            ("reuse", serde_json::json!("invented")),
            ("reuse", serde_json::json!(null)),
            ("reuse", serde_json::json!(1)),
            ("received_at", serde_json::json!(null)),
            ("received_at", serde_json::json!({})),
            ("corrected_initial_age", serde_json::json!(null)),
            ("corrected_initial_age", serde_json::json!(1)),
            ("source_freshness", serde_json::json!(false)),
            ("source_stale_if_error", serde_json::json!("120")),
        ] {
            let mut value: serde_json::Value = serde_json::from_str(&valid).unwrap();
            value[key] = replacement;
            invalid.push(serde_json::to_string(&value).unwrap());
        }
        for key in [
            "reuse",
            "received_at",
            "corrected_initial_age",
            "source_freshness",
            "source_stale_if_error",
        ] {
            let mut value: serde_json::Value = serde_json::from_str(&valid).unwrap();
            value.as_object_mut().unwrap().remove(key);
            invalid.push(serde_json::to_string(&value).unwrap());
        }
        let oversized = format!("{valid}{}", " ".repeat(1025 - valid.len()));
        assert_eq!(oversized.len(), 1025);
        invalid.push(oversized);
        let before = preserved_rows(&fixture.connection);
        for value in invalid {
            let error = insert_policy(&fixture, Some(&value)).unwrap_err();
            assert!(
                matches!(error, rusqlite::Error::SqliteFailure(ref inner, _) if inner.code == rusqlite::ErrorCode::ConstraintViolation)
            );
            assert_eq!(preserved_rows(&fixture.connection), before);
        }
    }

    #[test]
    fn metadata_policy_migration_preserves_immutable_carrier_and_failed_forward_transaction() {
        let fixture = populated_v16();
        // An interrupted/conflicting schema must not leave a transaction open
        // or advance user_version. Drop the fixture column, then retry normally.
        fixture
            .connection
            .execute(
                "ALTER TABLE metadata_claims ADD COLUMN response_policy_json TEXT",
                [],
            )
            .unwrap();
        let before = preserved_rows(&fixture.connection);
        assert!(migrate_v17(&fixture.connection).is_err());
        assert!(fixture.connection.is_autocommit());
        assert_eq!(
            fixture
                .connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            16
        );
        assert_eq!(preserved_rows(&fixture.connection), before);
        fixture
            .connection
            .execute(
                "ALTER TABLE metadata_claims DROP COLUMN response_policy_json",
                [],
            )
            .unwrap();
        migrate_v17(&fixture.connection).unwrap();
        assert_eq!(preserved_rows(&fixture.connection), before);
        assert!(fixture
            .connection
            .execute(
                "UPDATE metadata_claims SET response_policy_json = ?1 WHERE claim_id = ?2",
                params![
                    policy(ProviderResponseReuse::Reusable),
                    fixture.claim.to_string()
                ]
            )
            .is_err());
        assert!(fixture
            .connection
            .execute(
                "DELETE FROM metadata_claims WHERE claim_id = ?1",
                [fixture.claim.to_string()]
            )
            .is_err());
        assert_eq!(preserved_rows(&fixture.connection), before);
        assert!(rows(&fixture.connection, "PRAGMA foreign_key_check").is_empty());
    }
}
