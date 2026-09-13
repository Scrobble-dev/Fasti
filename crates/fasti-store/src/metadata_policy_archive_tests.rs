mod metadata_policy_archive_tests {
    use super::*;
    use fasti_application::{ProviderResponseCachePolicy, ProviderResponseReuse};
    use std::time::Duration;

    fn policy(reuse: ProviderResponseReuse) -> String {
        // Both real claims in metadata_v3_fixture use this observation time.
        ProviderResponseCachePolicy::new(
            reuse,
            DateTime::parse_from_rfc3339("2026-08-30T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            Duration::ZERO,
            Some(Duration::from_secs(120)),
            Some(Duration::from_secs(600)),
        )
        .to_canonical_json()
    }

    fn change_claim(
        fixture: &MetadataV3Fixture,
        claim_id: MetadataClaimId,
        mutate: impl FnOnce(&mut serde_json::Value),
    ) -> Vec<u8> {
        rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::MetadataClaims,
            |bytes| {
                let mut rows: Vec<serde_json::Value> = bytes
                    .split_inclusive(|byte| *byte == b'\n')
                    .map(|line| serde_json::from_slice(line).unwrap())
                    .collect();
                let row = rows
                    .iter_mut()
                    .find(|row| row["claim_id"] == claim_id.to_string())
                    .expect("real field or rating claim registry row");
                mutate(row);
                *bytes = rows
                    .into_iter()
                    .flat_map(|row| {
                        let mut line = serde_json::to_vec(&row).unwrap();
                        line.push(b'\n');
                        line
                    })
                    .collect();
            },
        )
    }

    fn reject_before_staging(archive: Vec<u8>, case: &str) {
        let root = tempfile::tempdir().unwrap();
        let lock = LockedDataRoot::acquire(root.path()).unwrap();
        let attempt_id = RestoreAttemptId::new_v7();
        let mut source = Cursor::new(archive);
        source.set_position(7);
        let error = stage_workspace_archive_pass_two(
            &lock,
            &mut source,
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .unwrap_or_else(|| panic!("{case}: hostile archive was accepted"));
        assert!(
            matches!(
                error,
                RestoreImportError::Preflight(RestorePreflightError::MetadataPolicy { .. })
            ),
            "{case}: expected policy preflight rejection, got {error:?}"
        );
        assert_eq!(
            source.position(),
            0,
            "{case}: source must rewind on rejection"
        );
        assert!(
            !root.path().join(RESTORE_STAGING_DIRECTORY).exists(),
            "{case}: not even the staging directory may be created"
        );
        assert!(!root.path().join("current").exists(), "{case}");
        assert_attempt_removed(root.path(), attempt_id);
    }

    fn with_v6_manifest(archive: &[u8]) -> Vec<u8> {
        let mut entries = archive_entries(archive);
        let manifest_bytes = entries.pop().unwrap().1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&manifest_bytes, limits())
                .unwrap();
        let manifest = verified.manifest();
        let rebuilt = WorkspaceManifest::try_new_for_format(
            WORKSPACE_ARCHIVE_V6_FORMAT_VERSION,
            manifest.workspace_id(),
            manifest.workspace_revision(),
            manifest.contract_version().to_owned(),
            16,
            Sha256Digest::parse(
                "sha256:d7ae3b1ab15c0223245d1a9008833049e58e9ec882a6e1ba70a2a080fa3fd7a6",
            )
            .unwrap(),
            manifest.streams().to_vec(),
            manifest.blobs().to_vec(),
        )
        .unwrap();
        let projection =
            CanonicalWorkspaceManifestProjection::try_from_application(rebuilt).unwrap();
        entries.push((
            "manifest.json".to_owned(),
            projection.canonical_json_bytes().to_vec(),
        ));
        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024).unwrap();
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).unwrap();
        for (path, bytes) in entries {
            writer
                .append(&path, bytes.len() as u64, Cursor::new(bytes))
                .unwrap();
        }
        writer.finish().unwrap()
    }

    #[test]
    fn archive_v7_rehashed_hostile_nested_policy_rejects_before_any_staging() {
        let fixture = metadata_v3_fixture();
        let entries = archive_entries(&fixture.archive);
        let field_index = entries
            .iter()
            .position(|(path, _)| path == "metadata_field_claims.ndjson")
            .unwrap();
        let registry_index = entries
            .iter()
            .position(|(path, _)| path == "metadata_claims.ndjson")
            .unwrap();
        assert!(
            field_index < registry_index,
            "real field payload precedes policy registry"
        );
        assert!(String::from_utf8_lossy(&entries[field_index].1).contains("Archive title"));
        let canonical = policy(ProviderResponseReuse::Reusable);
        let cases = [
            ("no-store", policy(ProviderResponseReuse::NoStore)),
            ("leading whitespace", format!(" {canonical}")),
            (
                "duplicate reuse",
                canonical.replace("\"reuse\":", "\"reuse\":\"no_store\",\"reuse\":"),
            ),
            (
                "unknown field",
                canonical.replace("\"reuse\":", "\"unknown\":true,\"reuse\":"),
            ),
            (
                "duplicate duration",
                canonical.replace("\"secs\":0", "\"secs\":0,\"secs\":1"),
            ),
            (
                "unknown duration field",
                canonical.replace("\"secs\":0", "\"unknown\":0,\"secs\":0"),
            ),
            (
                "normalized duration",
                canonical.replacen("\"nanos\":0", "\"nanos\":1000000000", 1),
            ),
            (
                "duration overflow",
                canonical.replace("\"secs\":0", "\"secs\":18446744073709551616"),
            ),
            (
                "noncanonical timestamp",
                canonical.replace("12:00:00Z", "12:00:00+00:00"),
            ),
            (
                "oversized policy",
                format!(
                    "{canonical}{}",
                    " ".repeat(ProviderResponseCachePolicy::MAX_JSON_BYTES + 1 - canonical.len())
                ),
            ),
            ("oversized row", format!("{canonical}{}", " ".repeat(5000))),
        ];
        for claim_id in [fixture.field_claim_id, fixture.rating_claim_id] {
            for (case, json) in &cases {
                assert_ne!(json, &canonical, "{case}: mutation must change evidence");
                let hostile = change_claim(&fixture, claim_id, |row| {
                    row["response_policy_json"] = json.clone().into();
                });
                reject_before_staging(hostile, &format!("{claim_id}: {case}"));
            }
        }
    }

    #[test]
    fn archive_v7_policy_round_trips_canonically_without_dropping_earlier_field_payload() {
        let fixture = metadata_v3_fixture();
        for (claim_id, other_claim_id) in [
            (fixture.field_claim_id, fixture.rating_claim_id),
            (fixture.rating_claim_id, fixture.field_claim_id),
        ] {
            for reuse in [
                ProviderResponseReuse::Reusable,
                ProviderResponseReuse::ValidateEveryReuse,
                ProviderResponseReuse::ValidateWhenStale,
            ] {
                let json = policy(reuse);
                let archive = change_claim(&fixture, claim_id, |row| {
                    row["response_policy_json"] = json.clone().into()
                });
                let root = tempfile::tempdir().unwrap();
                let lock = LockedDataRoot::acquire(root.path()).unwrap();
                let mut source = Cursor::new(archive);
                preflight_restore_source(&mut source, limits(), &CancellationSignal::new())
                    .expect("canonical policy passes before staging");
                assert_eq!(source.position(), 0);
                assert!(!root.path().join(RESTORE_STAGING_DIRECTORY).exists());
                let staged = stage_workspace_archive_pass_two(
                    &lock,
                    &mut source,
                    RestoreAttemptId::new_v7(),
                    RequestCorrelationId::new_v7(),
                    limits(),
                    &CancellationSignal::new(),
                )
                .expect("stage canonical policy-bearing v7 archive");
                let database = Connection::open_with_flags(
                    staged.database_path(),
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                )
                .unwrap();
                let restored: String = database
                    .query_row(
                        "SELECT response_policy_json FROM metadata_claims WHERE claim_id = ?1",
                        [claim_id.to_string()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(restored, json);
                let untouched: Option<String> = database
                    .query_row(
                        "SELECT response_policy_json FROM metadata_claims WHERE claim_id = ?1",
                        [other_claim_id.to_string()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(untouched, None, "the other claim must remain unchanged");
                let count: i64 = database
                .query_row(
                    "SELECT COUNT(*) FROM metadata_field_claims WHERE record_id = ?1 AND field_key = 'core.title' AND value = 'Archive title'",
                    [fixture.record_id.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
                assert_eq!(count, 1);
                drop(database);
                staged.cleanup().unwrap();
            }
        }
    }

    #[test]
    fn archive_v7_rejects_legacy_missing_policy_member_and_non_string_policy() {
        let fixture = metadata_v3_fixture();
        for claim_id in [fixture.field_claim_id, fixture.rating_claim_id] {
            let missing = change_claim(&fixture, claim_id, |row| {
                assert!(row
                    .as_object_mut()
                    .unwrap()
                    .remove("response_policy_json")
                    .unwrap()
                    .is_null());
            });
            reject_before_staging(missing, "v6 row shape in v7 archive");
            let object = change_claim(&fixture, claim_id, |row| {
                row["response_policy_json"] =
                    serde_json::from_str(&policy(ProviderResponseReuse::Reusable)).unwrap();
            });
            reject_before_staging(object, "nested policy must remain canonical JSON string");
        }
    }

    #[test]
    fn archive_v6_rejects_v7_policy_members_before_staging_but_accepts_frozen_legacy_rows() {
        let fixture = metadata_v3_fixture();
        let legacy = with_v6_manifest(&legacy_metadata_claims_fixture(&fixture.archive));
        let mut source = Cursor::new(legacy);
        source.set_position(11);
        preflight_restore_source(&mut source, limits(), &CancellationSignal::new())
            .expect("frozen v6 rows remain valid; v7 validation must not apply");
        assert_eq!(source.position(), 0);

        reject_before_staging(with_v6_manifest(&fixture.archive), "v7 null member in v6");
        for claim_id in [fixture.field_claim_id, fixture.rating_claim_id] {
            let no_store = change_claim(&fixture, claim_id, |row| {
                row["response_policy_json"] = policy(ProviderResponseReuse::NoStore).into();
            });
            reject_before_staging(with_v6_manifest(&no_store), "v7 no-store policy in v6");
        }
    }
}
