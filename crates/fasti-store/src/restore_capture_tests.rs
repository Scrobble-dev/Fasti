mod restore_capture_tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    struct ObservedSource {
        cursor: Cursor<Vec<u8>>,
        replacement: Option<Vec<u8>>,
        seeks: usize,
        requests: Vec<usize>,
        returned: usize,
        short_read: usize,
        fail_read_after: Option<usize>,
        fail_seek: Option<usize>,
        cancel_after_read: Option<CancellationSignal>,
        cancel_on_initial_seek: Option<CancellationSignal>,
        lie_about_read_count: bool,
    }

    impl ObservedSource {
        fn new(bytes: Vec<u8>) -> Self {
            Self {
                cursor: Cursor::new(bytes),
                replacement: None,
                seeks: 0,
                requests: Vec::new(),
                returned: 0,
                short_read: 127,
                fail_read_after: None,
                fail_seek: None,
                cancel_after_read: None,
                cancel_on_initial_seek: None,
                lie_about_read_count: false,
            }
        }
    }

    impl Read for ObservedSource {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.requests.push(bytes.len());
            if self.lie_about_read_count {
                return Ok(bytes.len() + 1);
            }
            if self
                .fail_read_after
                .is_some_and(|limit| self.returned >= limit)
            {
                return Err(io::Error::other("injected capture read failure"));
            }
            let requested = bytes.len().min(self.short_read);
            let read = self.cursor.read(&mut bytes[..requested])?;
            self.returned += read;
            if let Some(signal) = self.cancel_after_read.take() {
                signal.cancel();
            }
            Ok(read)
        }
    }

    impl Seek for ObservedSource {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.seeks += 1;
            if self.seeks == 1 {
                if let Some(signal) = self.cancel_on_initial_seek.take() {
                    signal.cancel();
                }
            }
            if self.fail_seek == Some(self.seeks) {
                return Err(io::Error::other("injected capture rewind failure"));
            }
            if self.seeks == 2 {
                if let Some(bytes) = self.replacement.take() {
                    self.cursor = Cursor::new(bytes);
                }
            }
            self.cursor.seek(position)
        }
    }

    fn names(root: &Path) -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        names
    }

    fn assert_staged_original(
        lock: &LockedDataRoot,
        captured: CapturedRestoreArchive,
        fixture: &FullFixture,
    ) {
        let staged = stage_preflighted_workspace_archive_pass_two(
            lock,
            captured,
            RestoreAttemptId::new_v7(),
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage immutable original capture");
        assert_eq!(staged.workspace_revision(), fixture.archive_revision);
        let path = descriptor_child_path(
            &staged.attempt,
            &path_to_storage_value(&relative_evidence_path(
                canonical_digest_hex(fixture.evidence_digest.as_str()).unwrap(),
            )),
        );
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .unwrap();
        assert!(
            database
                .query_row("SELECT COUNT(*) FROM records", [], |row| row
                    .get::<_, i64>(0))
                .unwrap()
                > 0
        );
        drop(database);
        assert_eq!(std::fs::read(path).unwrap(), fixture.evidence_bytes);
        staged.cleanup().unwrap();
    }

    #[test]
    fn capture_second_rewind_mutation_cannot_replace_verified_import_bytes() {
        let fixture = full_fixture();
        let hostile = rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::Workspaces,
            |bytes| {
                let mut row: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                row["workspace_id"] = WorkspaceId::new_v7().to_string().into();
                *bytes = serde_json::to_vec(&row).unwrap();
                bytes.push(b'\n');
            },
        );
        let root = tempfile::tempdir().unwrap();
        let lock = LockedDataRoot::acquire(root.path()).unwrap();
        let baseline = names(root.path());
        let mut source = ObservedSource::new(fixture.archive.clone());
        source.replacement = Some(hostile.clone());
        let captured = capture_restore_source(
            lock.anchored_directory().unwrap(),
            &mut source,
            limits(),
            &CancellationSignal::new(),
        )
        .unwrap();
        assert_eq!(source.seeks, 2);
        assert_eq!(source.cursor.get_ref(), &hostile);
        assert_eq!(source.cursor.position(), 0);
        assert_eq!(
            captured.preflight.archive_digest(),
            &digest(&fixture.archive)
        );
        assert_eq!(captured.file.metadata().unwrap().nlink(), 0);
        assert_eq!(captured.file.metadata().unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            names(root.path()),
            baseline,
            "capture has no named scratch artifact"
        );
        let reads = source.requests.len();
        assert_staged_original(&lock, captured, &fixture);
        assert_eq!(
            source.requests.len(),
            reads,
            "pass two never touches caller source"
        );
    }

    #[test]
    fn capture_isolated_from_external_original_file_overwrite_and_truncation() {
        let fixture = full_fixture();
        let root = tempfile::tempdir().unwrap();
        let lock = LockedDataRoot::acquire(root.path()).unwrap();
        let mut original = tempfile::tempfile().unwrap();
        original.write_all(&fixture.archive).unwrap();
        let mut other_handle = original.try_clone().unwrap();
        let captured = capture_restore_source(
            lock.anchored_directory().unwrap(),
            &mut original,
            limits(),
            &CancellationSignal::new(),
        )
        .unwrap();
        other_handle.seek(SeekFrom::Start(0)).unwrap();
        other_handle.write_all(b"changed source bytes").unwrap();
        other_handle.set_len(20).unwrap();
        assert_eq!(
            captured.preflight.archive_digest(),
            &digest(&fixture.archive)
        );
        assert_eq!(
            captured.file.metadata().unwrap().len(),
            fixture.archive.len() as u64
        );
        assert_staged_original(&lock, captured, &fixture);
    }

    #[test]
    fn capture_short_reads_accept_exact_limit_only_after_bounded_eof_probe() {
        let fixture = full_fixture();
        let root = tempfile::tempdir().unwrap();
        let lock = LockedDataRoot::acquire(root.path()).unwrap();
        let baseline = names(root.path());
        let mut configured = limits();
        configured.max_archive_bytes = nonzero(fixture.archive.len() as u64);
        let mut source = ObservedSource::new(fixture.archive.clone());
        let captured = capture_restore_source(
            lock.anchored_directory().unwrap(),
            &mut source,
            configured,
            &CancellationSignal::new(),
        )
        .unwrap();
        assert_eq!(source.returned, fixture.archive.len());
        assert_eq!(
            source.requests.last(),
            Some(&1),
            "exact cap requires EOF probe"
        );
        assert!(source
            .requests
            .iter()
            .all(|size| *size > 0 && *size <= MAX_IO_CHUNK_BYTES));
        assert_eq!(source.cursor.position(), 0);
        assert_eq!(
            captured.preflight.archive_bytes(),
            fixture.archive.len() as u64
        );
        assert_eq!(captured.file.metadata().unwrap().nlink(), 0);
        drop(captured);
        assert_eq!(names(root.path()), baseline);

        let mut oversized = fixture.archive;
        oversized.push(1);
        let mut source = ObservedSource::new(oversized);
        let error = capture_restore_source(
            lock.anchored_directory().unwrap(),
            &mut source,
            configured,
            &CancellationSignal::new(),
        )
        .err()
        .expect("limit plus one rejected");
        assert!(matches!(error, RestoreImportError::CapacityExceeded));
        assert_eq!(
            source.returned as u64,
            configured.max_archive_bytes.get() + 1
        );
        assert_eq!(source.requests.last(), Some(&1));
        assert_eq!(source.cursor.position(), 0);
        assert_eq!(names(root.path()), baseline);
    }

    #[test]
    fn capture_read_failure_cancellation_and_rewind_failure_leave_no_named_artifacts() {
        let fixture = full_fixture();
        for case in [
            "read",
            "cancel-before",
            "cancel-during",
            "cancel-and-rewind-failure",
            "cancel-and-initial-seek-failure",
            "invalid-read-count",
            "initial-seek",
            "final-rewind",
        ] {
            let root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(root.path()).unwrap();
            let baseline = names(root.path());
            let cancel = CancellationSignal::new();
            let mut source = ObservedSource::new(fixture.archive.clone());
            match case {
                "read" => source.fail_read_after = Some(127),
                "cancel-before" => cancel.cancel(),
                "cancel-during" => source.cancel_after_read = Some(cancel.clone()),
                "cancel-and-rewind-failure" => {
                    source.cancel_after_read = Some(cancel.clone());
                    source.fail_seek = Some(2);
                }
                "cancel-and-initial-seek-failure" => {
                    source.cancel_on_initial_seek = Some(cancel.clone());
                    source.fail_seek = Some(1);
                }
                "invalid-read-count" => source.lie_about_read_count = true,
                "initial-seek" => source.fail_seek = Some(1),
                "final-rewind" => source.fail_seek = Some(2),
                _ => unreachable!(),
            }
            let error = capture_restore_source(
                lock.anchored_directory().unwrap(),
                &mut source,
                limits(),
                &cancel,
            )
            .err()
            .unwrap_or_else(|| panic!("{case} accepted"));
            match case {
                "read" => assert!(matches!(
                    error,
                    RestoreImportError::Archive(ArchiveError::Io(_))
                )),
                "cancel-before"
                | "cancel-during"
                | "cancel-and-rewind-failure"
                | "cancel-and-initial-seek-failure" => {
                    assert!(matches!(error, RestoreImportError::Canceled))
                }
                "invalid-read-count" => assert!(matches!(
                    error,
                    RestoreImportError::Archive(ArchiveError::Io(ref error))
                        if error.kind() == io::ErrorKind::InvalidData
                )),
                "initial-seek" => assert!(matches!(
                    error,
                    RestoreImportError::Preflight(RestorePreflightError::InitialSeek(_))
                )),
                "final-rewind" => assert!(matches!(error, RestoreImportError::Rewind(_))),
                _ => unreachable!(),
            }
            if matches!(
                case,
                "cancel-before" | "initial-seek" | "cancel-and-initial-seek-failure"
            ) {
                assert!(source.requests.is_empty());
            }
            if matches!(
                case,
                "cancel-during" | "cancel-and-rewind-failure" | "invalid-read-count"
            ) {
                assert_eq!(source.requests.len(), 1);
            }
            if matches!(case, "read" | "cancel-during" | "invalid-read-count") {
                assert_eq!(source.cursor.position(), 0);
            }
            if case == "cancel-and-rewind-failure" {
                assert_eq!(source.seeks, 2, "rewind attempted despite cancellation");
            }
            assert_eq!(
                names(root.path()),
                baseline,
                "{case}: capture scratch must be unnamed and staging absent"
            );
        }
    }

    #[test]
    fn capture_truncated_archive_rejects_after_copy_without_staging() {
        let fixture = full_fixture();
        for length in [0, fixture.archive.len() / 2, fixture.archive.len() - 1] {
            let root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(root.path()).unwrap();
            let baseline = names(root.path());
            let mut source = ObservedSource::new(fixture.archive[..length].to_vec());
            let error = capture_restore_source(
                lock.anchored_directory().unwrap(),
                &mut source,
                limits(),
                &CancellationSignal::new(),
            )
            .err()
            .expect("truncated archive must not become an importable capture");
            assert!(
                matches!(error, RestoreImportError::Preflight(_)),
                "{length}: {error:?}"
            );
            assert_eq!(source.cursor.position(), 0);
            assert_eq!(source.returned, length);
            assert_eq!(names(root.path()), baseline);
        }
    }

    const WRITE_FAILURE_ROOT_ENV: &str = "FASTI_CAPTURE_TEST_WRITE_FAILURE_ROOT";
    const WRITE_FAILURE_ARCHIVE_ENV: &str = "FASTI_CAPTURE_TEST_WRITE_FAILURE_ARCHIVE";

    #[test]
    #[ignore = "isolated file-size-limit worker invoked by capture_anonymous_write_failure"]
    fn capture_write_failure_worker() {
        let (Ok(root), Ok(archive)) = (
            std::env::var(WRITE_FAILURE_ROOT_ENV),
            std::env::var(WRITE_FAILURE_ARCHIVE_ENV),
        ) else {
            return;
        };
        // The parent creates the real database/archive before imposing RLIMIT_FSIZE.
        let root = Path::new(&root);
        let lock = LockedDataRoot::acquire(root).unwrap();
        let baseline = names(root);
        let mut source = File::open(archive).unwrap();
        assert!(source.metadata().unwrap().len() > 1024);
        let error = capture_restore_source(
            lock.anchored_directory().unwrap(),
            &mut source,
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .expect("anonymous capture write must hit the kernel file-size limit");
        assert!(
            matches!(error, RestoreImportError::Archive(ArchiveError::Io(ref error))
            if error.kind() == io::ErrorKind::FileTooLarge),
            "expected actual EFBIG write error, got {error:?}"
        );
        assert_eq!(source.stream_position().unwrap(), 0);
        assert_eq!(names(root), baseline);
    }

    #[test]
    fn capture_anonymous_write_failure_is_reported_and_cleans_up_in_isolated_process() {
        let fixture = full_fixture();
        assert!(
            fixture.archive.len() > 1024,
            "fixture must exceed child's 1 KiB file limit"
        );
        let inputs = tempfile::tempdir().unwrap();
        let archive = inputs.path().join("source.fasti");
        std::fs::write(&archive, &fixture.archive).unwrap();
        let root = tempfile::tempdir().unwrap();
        // nosemgrep: rust.lang.security.current-exe.current-exe -- test-only re-exec worker, never compiled into a release binary
        let executable = std::env::current_exe().unwrap();
        let output = Command::new("/bin/bash")
            .args([
                "-c",
                "trap '' XFSZ; ulimit -f 1 || exit 125; exec \"$@\"",
                "capture-write-limit",
            ])
            .arg(executable)
            .args([
                "--exact",
                "restore_import::tests::restore_capture_tests::capture_write_failure_worker",
                "--ignored",
                "--nocapture",
            ])
            // The deliberate 1 KiB file limit also truncates LLVM's regular-file profile.
            .env("LLVM_PROFILE_FILE", "/dev/null")
            .env(WRITE_FAILURE_ROOT_ENV, root.path())
            .env(WRITE_FAILURE_ARCHIVE_ENV, &archive)
            .output()
            .expect("run isolated capture write-limit worker");
        assert!(
            output.status.success(),
            "worker {:?}: stdout={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "exact worker must run"
        );
        assert!(!root.path().join(RESTORE_STAGING_DIRECTORY).exists());
        assert!(!root.path().join("current").exists());
    }
}
