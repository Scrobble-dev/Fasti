#[cfg(target_os = "linux")]
mod restore_capture_capacity_tests {
    use super::*;

    fn root_entries(root: &Path) -> BTreeSet<std::ffi::OsString> {
        std::fs::read_dir(root)
            .expect("read restore root")
            .map(|entry| entry.expect("root entry").file_name())
            .collect()
    }

    #[test]
    fn captured_archive_database_blobs_and_cleanup_share_exact_scratch_boundary() {
        let fixture = full_fixture();
        let cancellation = CancellationSignal::new();
        let preflight =
            preflight_restore_source(&mut Cursor::new(&fixture.archive), limits(), &cancellation)
                .expect("genuine archive preflight");
        let compressed = u64::try_from(fixture.archive.len()).unwrap();
        assert_eq!(preflight.archive_bytes(), compressed);
        let blobs = preflight
            .manifest()
            .manifest()
            .blobs()
            .iter()
            .map(|blob| blob.byte_length())
            .try_fold(0_u64, u64::checked_add)
            .unwrap();
        assert!(blobs > 0, "the real fixture must exercise blob accounting");
        assert_eq!(blobs, u64::try_from(fixture.evidence_bytes.len()).unwrap());
        let configured = limits();
        let exact = compressed
            .checked_add(configured.max_snapshot_bytes.get())
            .and_then(|bytes| bytes.checked_add(blobs))
            .and_then(|bytes| bytes.checked_add(configured.cleanup_reserve_bytes.get()))
            .unwrap();

        for deficit in [0_u64, 1] {
            let root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(root.path()).unwrap();
            let anchored = lock.anchored_directory().unwrap();
            let before = root_entries(root.path());
            let mut configured = configured;
            configured.scratch_ceiling_bytes = nonzero(exact - deficit);
            let mut source = Cursor::new(&fixture.archive);
            let captured = capture_restore_source(anchored, &mut source, configured, &cancellation)
                .expect("capture fits before database and blob admission");
            assert_eq!(source.position(), 0, "caller source is rewound");
            assert_eq!(captured.file.metadata().unwrap().nlink(), 0);
            assert_eq!(captured.file.metadata().unwrap().len(), compressed);
            assert_eq!(root_entries(root.path()), before, "capture is unnamed");
            if deficit == 0 {
                let remaining = remaining_restore_bytes(&captured.preflight, configured)
                    .expect("exact total scratch boundary leaves a valid remaining reserve");
                assert_eq!(
                    remaining,
                    configured.max_snapshot_bytes.get()
                        + blobs
                        + configured.cleanup_reserve_bytes.get(),
                    "free-space admission reserves only database, blobs and cleanup"
                );
                assert_eq!(remaining.checked_add(compressed), Some(exact));
                assert!(
                    remaining < exact,
                    "already captured bytes are not charged twice"
                );
            }

            let attempt = RestoreAttemptId::new_v7();
            let result = stage_preflighted_workspace_archive_pass_two(
                &lock,
                captured,
                attempt,
                RequestCorrelationId::new_v7(),
                configured,
                &cancellation,
            );
            if deficit == 0 {
                let staged = result.expect("exact total scratch boundary is admitted");
                assert_eq!(staged.workspace_id(), fixture.node.access.workspace_id());
                assert_eq!(staged.workspace_revision(), fixture.archive_revision);
                assert!(
                    staged.database_path().metadata().unwrap().len()
                        <= configured.max_snapshot_bytes.get()
                );
                staged.cleanup().expect("remove successful private staging");
                assert_attempt_removed(root.path(), attempt);
            } else {
                assert!(matches!(result, Err(RestoreImportError::CapacityExceeded)));
                assert_eq!(root_entries(root.path()), before);
                assert!(!root.path().join(RESTORE_STAGING_DIRECTORY).exists());
            }
            assert!(!root.path().join("current").exists());
        }
    }

    #[test]
    fn captured_restore_capacity_checked_additions_reject_before_staging() {
        let fixture = full_fixture();
        for overflow_in_total in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(root.path()).unwrap();
            let anchored = lock.anchored_directory().unwrap();
            let before = root_entries(root.path());
            let cancellation = CancellationSignal::new();
            let captured = capture_restore_source(
                anchored,
                &mut Cursor::new(&fixture.archive),
                limits(),
                &cancellation,
            )
            .expect("capture genuine archive before overflow admission");
            let blobs = captured
                .preflight
                .manifest()
                .manifest()
                .blobs()
                .iter()
                .map(|blob| blob.byte_length())
                .try_fold(0_u64, u64::checked_add)
                .unwrap();
            assert!(blobs > 0);
            let mut configured = limits();
            configured.scratch_ceiling_bytes = nonzero(u64::MAX);
            configured.max_snapshot_bytes = nonzero(if overflow_in_total {
                // D+B+R is representable, but adding the already captured C is not.
                u64::MAX - blobs - configured.cleanup_reserve_bytes.get()
            } else {
                // D+B itself overflows, before a filesystem capacity request.
                u64::MAX
            });
            let result = stage_preflighted_workspace_archive_pass_two(
                &lock,
                captured,
                RestoreAttemptId::new_v7(),
                RequestCorrelationId::new_v7(),
                configured,
                &cancellation,
            );
            assert!(matches!(result, Err(RestoreImportError::CapacityExceeded)));
            assert_eq!(root_entries(root.path()), before);
            assert!(!root.path().join(RESTORE_STAGING_DIRECTORY).exists());
            assert!(!root.path().join("current").exists());
        }
    }

    #[test]
    fn compressed_capture_budget_failures_leave_no_named_payload_or_staging() {
        let fixture = full_fixture();
        let compressed = u64::try_from(fixture.archive.len()).unwrap();
        assert!(compressed > 1);
        for failure in [
            "archive_limit",
            "scratch_limit",
            "reserve_underflow",
            "zero_budget",
        ] {
            let root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(root.path()).unwrap();
            let before = root_entries(root.path());
            let mut configured = limits();
            match failure {
                "archive_limit" => configured.max_archive_bytes = nonzero(compressed - 1),
                "scratch_limit" => {
                    configured.scratch_ceiling_bytes =
                        nonzero(compressed + configured.cleanup_reserve_bytes.get() - 1);
                }
                "reserve_underflow" => {
                    configured.scratch_ceiling_bytes =
                        nonzero(configured.cleanup_reserve_bytes.get() - 1);
                }
                "zero_budget" => {
                    configured.scratch_ceiling_bytes = configured.cleanup_reserve_bytes;
                }
                _ => unreachable!(),
            }
            let mut source = Cursor::new(&fixture.archive);
            let result = capture_restore_source(
                lock.anchored_directory().unwrap(),
                &mut source,
                configured,
                &CancellationSignal::new(),
            );
            assert!(
                matches!(result, Err(RestoreImportError::CapacityExceeded)),
                "{failure} must reject capture before staging"
            );
            assert_eq!(source.position(), 0);
            assert_eq!(root_entries(root.path()), before);
            assert!(!root.path().join(RESTORE_STAGING_DIRECTORY).exists());
            assert!(!root.path().join("current").exists());
        }
    }

    #[test]
    fn subpage_database_limit_rejects_without_allocating_schema() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("empty.sqlite3");
        let connection = Connection::open(&path).unwrap();
        let page_size: u64 = connection
            .query_row("PRAGMA main.page_size", [], |row| {
                row.get::<_, u32>(0).map(u64::from)
            })
            .unwrap();
        assert!(page_size > 1);
        assert!(matches!(
            enforce_restore_database_limit(&connection, page_size - 1),
            Err(RestoreImportError::CapacityExceeded)
        ));
        let tables: i64 = connection
            .query_row("SELECT COUNT(*) FROM sqlite_schema", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tables, 0);
        assert_eq!(path.metadata().unwrap().len(), 0);
    }

    #[test]
    fn nonaligned_database_limit_floors_pages_and_sqlite_enforces_allocation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("bounded.sqlite3");
        let connection = Connection::open(&path).unwrap();
        let page_size: u64 = connection
            .query_row("PRAGMA main.page_size", [], |row| {
                row.get::<_, u32>(0).map(u64::from)
            })
            .unwrap();
        let budget = 8 * page_size + page_size - 1;
        enforce_restore_database_limit(&connection, budget).unwrap();
        let maximum: u64 = connection
            .query_row("PRAGMA main.max_page_count", [], |row| {
                row.get::<_, u32>(0).map(u64::from)
            })
            .unwrap();
        assert_eq!(maximum, 8);
        connection
            .execute_batch(
                "CREATE TABLE bounded_payload(id INTEGER PRIMARY KEY, payload BLOB NOT NULL);",
            )
            .unwrap();
        connection
            .execute("INSERT INTO bounded_payload VALUES (1, X'01')", [])
            .unwrap();
        let constraint = connection
            .execute("INSERT INTO bounded_payload VALUES (1, X'02')", [])
            .unwrap_err();
        assert_eq!(
            constraint.sqlite_error_code(),
            Some(rusqlite::ErrorCode::ConstraintViolation)
        );
        let full = connection
            .execute(
                "INSERT INTO bounded_payload VALUES (2, zeroblob(?1))",
                [i64::try_from(16 * page_size).unwrap()],
            )
            .unwrap_err();
        assert_eq!(
            full.sqlite_error_code(),
            Some(rusqlite::ErrorCode::DiskFull)
        );
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM bounded_payload", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "rejected allocation must not leave a partial row");
        let value: Vec<u8> = connection
            .query_row(
                "SELECT payload FROM bounded_payload WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, [1]);
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        assert!(path.metadata().unwrap().len() <= 8 * page_size);
        assert!(path.metadata().unwrap().len() <= budget);
        assert!(path.metadata().unwrap().len() > page_size);
        assert!(
            matches!(
                enforce_restore_database_limit(&connection, page_size),
                Err(RestoreImportError::CapacityExceeded)
            ),
            "SQLite cannot lower the limit below its existing allocation"
        );
    }

    #[test]
    fn genuine_restore_database_limit_failures_cleanup_and_retry() {
        let fixture = full_fixture();
        // Use the real Record owner to ensure populated table/index pages exceed
        // the empty schema, without fabricating archive rows or SQLite errors.
        for _ in 0..256 {
            fixture
                .node
                .kernel
                .create_record(CreateRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    fixture.node.access,
                    Grain::Release,
                ))
                .unwrap();
        }
        let destination = Arc::new(Mutex::new(DestinationState::default()));
        export_online_workspace_archive(
            &fixture.node.kernel,
            ExportWorkspaceRequest::new(
                ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), fixture.node.access),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(MemoryDestination(Arc::clone(&destination))),
        )
        .unwrap();
        let archive = destination.lock().unwrap().bytes.clone();
        let schema_root = tempfile::tempdir().unwrap();
        let schema_path = schema_root.path().join("schema.sqlite3");
        let schema = Connection::open(&schema_path).unwrap();
        schema.pragma_update(None, "foreign_keys", "ON").unwrap();
        let page_size: u64 = schema
            .query_row("PRAGMA main.page_size", [], |row| {
                row.get::<_, u32>(0).map(u64::from)
            })
            .unwrap();
        migrate(&schema).unwrap();
        let schema_pages: u64 = schema
            .query_row("PRAGMA main.page_count", [], |row| {
                row.get::<_, u32>(0).map(u64::from)
            })
            .unwrap();
        let schema_bytes = schema_pages.checked_mul(page_size).unwrap();
        assert!(schema_pages > 1);
        schema.close().unwrap();

        for budget in [page_size - 1, page_size, schema_bytes] {
            let root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(root.path()).unwrap();
            let cancellation = CancellationSignal::new();
            let mut configured = limits();
            configured.max_snapshot_bytes = nonzero(budget);
            let captured = capture_restore_source(
                lock.anchored_directory().unwrap(),
                &mut Cursor::new(&archive),
                configured,
                &cancellation,
            )
            .unwrap();
            let attempt = RestoreAttemptId::new_v7();
            let result = stage_preflighted_workspace_archive_pass_two(
                &lock,
                captured,
                attempt,
                RequestCorrelationId::new_v7(),
                configured,
                &cancellation,
            );
            assert!(
                matches!(result, Err(RestoreImportError::CapacityExceeded)),
                "budget {budget} must report capacity, not storage or row corruption"
            );
            assert_attempt_removed(root.path(), attempt);
            assert!(!root.path().join("current").exists());
            assert!(
                std::fs::read_dir(root.path().join(RESTORE_STAGING_DIRECTORY))
                    .unwrap()
                    .next()
                    .is_none()
            );

            let retry = RestoreAttemptId::new_v7();
            let captured = capture_restore_source(
                lock.anchored_directory().unwrap(),
                &mut Cursor::new(&archive),
                limits(),
                &cancellation,
            )
            .unwrap();
            let staged = stage_preflighted_workspace_archive_pass_two(
                &lock,
                captured,
                retry,
                RequestCorrelationId::new_v7(),
                limits(),
                &cancellation,
            )
            .expect("normal budget restores the same populated archive after rejection");
            assert_eq!(staged.workspace_id(), fixture.node.access.workspace_id());
            assert!(staged.database_path().metadata().unwrap().len() > schema_bytes);
            assert!(
                staged.database_path().metadata().unwrap().len()
                    <= limits().max_snapshot_bytes.get()
            );
            let database = Connection::open_with_flags(
                staged.database_path(),
                OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            let records: i64 = database
                .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
                .unwrap();
            assert_eq!(records, 258);
            database.close().unwrap();
            staged.cleanup().unwrap();
            assert_attempt_removed(root.path(), retry);
            assert!(!root.path().join("current").exists());
        }
    }
}
