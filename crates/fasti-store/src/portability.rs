use crate::crypto::{encode_hex, sha256_reader};
use crate::evidence::{canonical_digest_hex, path_to_storage_value, relative_evidence_path};
use crate::kernel::{authorize_transaction, map_sql, SqliteKernel, StoreOpenError};
use crate::schema::workspace_revision;
use fasti_application::{
    ApplicationResult, CapabilityKey, ExportWorkspaceQuery, FastiProblem, PortabilityLimits,
    ProblemCode, RequestAccessContext, VerifyWorkspaceQuery, WorkspaceBlobDescriptor,
    WorkspaceExportEntity, WorkspaceExportOutcome, WorkspaceExportPort, WorkspaceStreamDescriptor,
    WorkspaceVerificationOutcome, WorkspaceVerificationPort, MAX_PORTABLE_JSON_INTEGER,
    WORKSPACE_EXPORT_FORMAT_VERSION,
};
use fasti_domain::{
    EvidenceId, RequestCorrelationId, Sha256Digest, WorkspaceId, ARCHIVE_MAX_IO_CHUNK_BYTES,
};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{params, Connection, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

const EVIDENCE_VERIFY_PAGE: i64 = 128;
type EvidenceRow = (i64, String, i64, String);
type Snapshot = (u64, u64, i64);

/// Map a stopped-node verify open failure without exposing adapter detail.
///
/// The shared lock has a specific recovery action. Only explicitly transient
/// operating-system or SQLite failures are retry-safe. Path, configuration,
/// schema, and database-format failures are integrity failures so offline
/// verification fails closed.
pub fn map_offline_verify_open_error(
    error: StoreOpenError,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    map_offline_open_error(error, CapabilityKey::VerifyWorkspace, correlation_id)
}

pub(crate) fn map_offline_open_error(
    error: StoreOpenError,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    match error {
        StoreOpenError::DataRootLocked => {
            Box::new(FastiProblem::data_root_locked(capability, correlation_id))
        }
        StoreOpenError::Io(error) if transient_io_error(error.kind()) => Box::new(
            FastiProblem::storage_unavailable(capability, correlation_id),
        ),
        StoreOpenError::Sqlite(error) if transient_sqlite_error(&error) => Box::new(
            FastiProblem::storage_unavailable(capability, correlation_id),
        ),
        StoreOpenError::UnsupportedPlatform
            if capability
                .allowed_problem_codes()
                .contains(&ProblemCode::UnsupportedPlatform) =>
        {
            Box::new(FastiProblem::unsupported_platform(
                capability,
                correlation_id,
            ))
        }
        StoreOpenError::UnsupportedPlatform
        | StoreOpenError::Io(_)
        | StoreOpenError::UnsafePath { .. }
        | StoreOpenError::Sqlite(_)
        | StoreOpenError::JournalMode(_)
        | StoreOpenError::SynchronousLevel(_)
        | StoreOpenError::SchemaVersion { .. }
        | StoreOpenError::RestoreActivation => {
            Box::new(FastiProblem::integrity_failed(capability, correlation_id))
        }
    }
}

fn transient_io_error(kind: ErrorKind) -> bool {
    matches!(
        kind,
        ErrorKind::Interrupted | ErrorKind::WouldBlock | ErrorKind::TimedOut
    )
}

fn transient_sqlite_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(sqlite, _)
            if matches!(
                sqlite.code,
                rusqlite::ffi::ErrorCode::DatabaseBusy
                    | rusqlite::ffi::ErrorCode::DatabaseLocked
                    | rusqlite::ffi::ErrorCode::OperationInterrupted
            )
    )
}

impl WorkspaceVerificationPort for SqliteKernel {
    fn verify_workspace(
        &self,
        query: VerifyWorkspaceQuery,
    ) -> ApplicationResult<WorkspaceVerificationOutcome> {
        let capability = CapabilityKey::VerifyWorkspace;
        let correlation_id = query.correlation_id();
        let workspace_id = query.access().workspace_id();
        let snapshot = verify_database_snapshot(self, query.access(), capability, correlation_id)?;

        let evidence_verified =
            verify_evidence_pages(self, query.access(), snapshot.2, capability, correlation_id)?;

        // Evidence hashing deliberately releases the connection lock between
        // bounded pages. Re-check the durable snapshot before reporting
        // success so a concurrent Chronicle mutation cannot be silently
        // omitted from a successful verification receipt.
        let final_snapshot =
            verify_database_snapshot(self, query.access(), capability, correlation_id)?;
        if final_snapshot != snapshot {
            return Err(Box::new(FastiProblem::from_code(
                ProblemCode::StorageUnavailable,
                capability,
                correlation_id,
            )));
        }

        Ok(WorkspaceVerificationOutcome::new(
            workspace_id,
            snapshot.0,
            evidence_verified,
            snapshot.1,
        ))
    }
}

fn verify_database_snapshot(
    kernel: &SqliteKernel,
    access: &RequestAccessContext,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Snapshot> {
    let mut connection = kernel.lock_connection(capability, correlation_id)?;
    let transaction = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Deferred),
        capability,
        correlation_id,
    )?;
    authorize_transaction(&transaction, capability, access, correlation_id)?;
    verify_sqlite_integrity(&transaction, capability, correlation_id)?;
    verify_domain_relations(
        &transaction,
        access.workspace_id(),
        capability,
        correlation_id,
    )?;
    let snapshot = map_sql(
        transaction.query_row(
            r#"
            SELECT
                (SELECT COUNT(*) FROM observations WHERE workspace_id = ?1),
                (SELECT COUNT(*) FROM corrections WHERE workspace_id = ?1),
                (SELECT COALESCE(MAX(rowid), 0) FROM evidence WHERE workspace_id = ?1)
            "#,
            [access.workspace_id().to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    map_sql(transaction.commit(), capability, correlation_id)?;
    Ok((
        u64::try_from(snapshot.0)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        u64::try_from(snapshot.1)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        snapshot.2,
    ))
}

pub(crate) fn verify_sqlite_integrity(
    transaction: &Transaction<'_>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    let quick_check = map_sql(
        transaction.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0)),
        capability,
        correlation_id,
    )?;
    if quick_check != "ok" {
        return integrity_failure(capability, correlation_id);
    }

    let has_foreign_key_failure = {
        let mut statement = map_sql(
            transaction.prepare("PRAGMA foreign_key_check"),
            capability,
            correlation_id,
        )?;
        let mut rows = map_sql(statement.query([]), capability, correlation_id)?;
        map_sql(rows.next(), capability, correlation_id)?.is_some()
    };
    if has_foreign_key_failure {
        return integrity_failure(capability, correlation_id);
    }
    Ok(())
}

pub(crate) fn verify_domain_relations(
    transaction: &Transaction<'_>,
    workspace_id: fasti_domain::WorkspaceId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    // Foreign keys prove existence. This query proves the cross-workspace,
    // cross-profile, interpretation-chain, and receipt relationships that
    // cannot be expressed by the current single-column foreign keys alone.
    let invalid = map_sql(
        transaction.query_row(
            r#"
            SELECT COUNT(*) FROM (
                SELECT i.interpretation_id AS invalid_id
                FROM interpretations i
                JOIN observations o ON o.observation_id = i.observation_id
                LEFT JOIN occurrences oc ON oc.occurrence_id = i.occurrence_id
                LEFT JOIN interpretations prior ON prior.interpretation_id = i.prior_interpretation_id
                LEFT JOIN records record ON record.record_id = i.record_id
                WHERE o.workspace_id = ?1 AND (
                    oc.occurrence_id IS NULL
                    OR oc.observation_id <> i.observation_id
                    OR oc.workspace_id <> o.workspace_id
                    OR oc.profile_id <> o.profile_id
                    OR (prior.interpretation_id IS NOT NULL AND prior.observation_id <> i.observation_id)
                    OR (prior.interpretation_id IS NOT NULL AND prior.occurrence_id <> i.occurrence_id)
                    OR (record.record_id IS NOT NULL AND record.workspace_id <> o.workspace_id)
                )

                UNION ALL

                SELECT o.observation_id
                FROM observations o
                LEFT JOIN interpretations i ON i.observation_id = o.observation_id
                WHERE o.workspace_id = ?1
                GROUP BY o.observation_id
                HAVING
                    SUM(CASE WHEN i.interpretation_id IS NOT NULL AND i.prior_interpretation_id IS NULL THEN 1 ELSE 0 END) <> 1
                    OR SUM(CASE WHEN i.interpretation_id IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM interpretations child
                        WHERE child.prior_interpretation_id = i.interpretation_id
                    ) THEN 1 ELSE 0 END) <> 1

                UNION ALL

                SELECT c.correction_id
                FROM corrections c
                LEFT JOIN observations o ON o.observation_id = c.observation_id
                LEFT JOIN interpretations prior ON prior.interpretation_id = c.prior_interpretation_id
                LEFT JOIN interpretations replacement ON replacement.interpretation_id = c.replacement_interpretation_id
                LEFT JOIN occurrences oc ON oc.occurrence_id = replacement.occurrence_id
                LEFT JOIN clients actor ON actor.client_id = c.actor_client_id
                LEFT JOIN records record ON record.record_id = c.record_id
                WHERE c.workspace_id = ?1 AND (
                    o.observation_id IS NULL
                    OR o.workspace_id <> c.workspace_id
                    OR o.profile_id <> c.profile_id
                    OR prior.observation_id <> c.observation_id
                    OR replacement.observation_id <> c.observation_id
                    OR prior.occurrence_id <> replacement.occurrence_id
                    OR replacement.prior_interpretation_id <> c.prior_interpretation_id
                    OR oc.observation_id <> c.observation_id
                    OR oc.workspace_id <> c.workspace_id
                    OR oc.profile_id <> c.profile_id
                    OR actor.workspace_id <> c.workspace_id
                    OR (record.record_id IS NOT NULL AND record.workspace_id <> c.workspace_id)
                    OR COALESCE(replacement.record_id, '') <> COALESCE(c.record_id, '')
                )

                UNION ALL

                SELECT review.review_item_id
                FROM review_items review
                LEFT JOIN observations o ON o.observation_id = review.observation_id
                LEFT JOIN interpretations i ON i.interpretation_id = review.current_interpretation_id
                WHERE review.workspace_id = ?1 AND (
                    o.workspace_id <> review.workspace_id
                    OR o.profile_id <> review.profile_id
                    OR i.observation_id <> review.observation_id
                )

                UNION ALL

                SELECT receipt.receipt_id
                FROM receipts receipt
                LEFT JOIN observations o ON o.observation_id = receipt.observation_id
                LEFT JOIN evidence e ON e.evidence_id = receipt.evidence_id
                LEFT JOIN occurrences oc ON oc.occurrence_id = receipt.occurrence_id
                LEFT JOIN interpretations i ON i.interpretation_id = receipt.interpretation_id
                LEFT JOIN review_items review ON review.review_item_id = receipt.review_item_id
                LEFT JOIN records record ON record.record_id = receipt.record_id
                WHERE receipt.workspace_id = ?1 AND (
                    o.workspace_id <> receipt.workspace_id
                    OR o.profile_id <> receipt.profile_id
                    OR e.workspace_id <> receipt.workspace_id
                    OR e.evidence_id <> o.evidence_id
                    OR (oc.occurrence_id IS NOT NULL AND oc.observation_id <> receipt.observation_id)
                    OR (i.interpretation_id IS NOT NULL AND i.observation_id <> receipt.observation_id)
                    OR (review.review_item_id IS NOT NULL AND review.observation_id <> receipt.observation_id)
                    OR (record.record_id IS NOT NULL AND record.workspace_id <> receipt.workspace_id)
                )

                UNION ALL

                SELECT operation.operation_id
                FROM operations operation
                LEFT JOIN receipts receipt ON receipt.receipt_id = operation.receipt_id
                WHERE operation.workspace_id = ?1 AND (
                    receipt.workspace_id <> operation.workspace_id
                    OR receipt.client_id <> operation.client_id
                    OR receipt.operation_id <> operation.operation_id
                    OR receipt.capability_key <> operation.capability_key
                )

            ) invalid
            "#,
            [workspace_id.to_string()],
            |row| row.get::<_, i64>(0),
        ),
        capability,
        correlation_id,
    )?;
    if invalid != 0 {
        return integrity_failure(capability, correlation_id);
    }
    verify_namespace_bindings(transaction, workspace_id, capability, correlation_id)?;
    Ok(())
}

fn verify_namespace_bindings(
    transaction: &Transaction<'_>,
    workspace_id: fasti_domain::WorkspaceId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    let invalid = map_sql(
        transaction.query_row(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM external_identifiers external
                LEFT JOIN namespace_definitions namespace
                  ON namespace.workspace_id = external.workspace_id
                 AND namespace.namespace = external.namespace
                 AND instr(',' || namespace.supported_grains || ',', ',' || external.grain || ',') > 0
                WHERE external.workspace_id = ?1 AND namespace.namespace IS NULL
            )
            "#,
            [workspace_id.to_string()],
            |row| row.get::<_, bool>(0),
        ),
        capability,
        correlation_id,
    )?;
    if invalid {
        return integrity_failure(capability, correlation_id);
    }
    Ok(())
}

fn verify_evidence_pages(
    kernel: &SqliteKernel,
    access: &RequestAccessContext,
    upper_rowid: i64,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<u64> {
    let mut cursor = 0_i64;
    let mut verified = 0_u64;
    loop {
        let page = read_evidence_page(
            kernel,
            access,
            cursor,
            upper_rowid,
            capability,
            correlation_id,
        )?;
        if page.is_empty() {
            return Ok(verified);
        }
        for (rowid, digest, size_bytes, stored_path) in page {
            verify_evidence_file(
                kernel,
                &digest,
                size_bytes,
                &stored_path,
                capability,
                correlation_id,
            )?;
            cursor = rowid;
            verified = verified.checked_add(1).ok_or_else(|| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?;
        }
    }
}

fn read_evidence_page(
    kernel: &SqliteKernel,
    access: &RequestAccessContext,
    cursor: i64,
    upper_rowid: i64,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Vec<EvidenceRow>> {
    let mut connection = kernel.lock_connection(capability, correlation_id)?;
    let transaction = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Deferred),
        capability,
        correlation_id,
    )?;
    authorize_transaction(&transaction, capability, access, correlation_id)?;
    let page = {
        let mut statement = map_sql(
            transaction.prepare(
                r#"
                SELECT rowid, digest, size_bytes, relative_path
                FROM evidence
                WHERE workspace_id = ?1 AND rowid > ?2 AND rowid <= ?3
                ORDER BY rowid LIMIT ?4
                "#,
            ),
            capability,
            correlation_id,
        )?;
        let rows = map_sql(
            statement.query_map(
                params![
                    access.workspace_id().to_string(),
                    cursor,
                    upper_rowid,
                    EVIDENCE_VERIFY_PAGE
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            ),
            capability,
            correlation_id,
        )?;
        let mut page = Vec::with_capacity(EVIDENCE_VERIFY_PAGE as usize);
        for row in rows {
            page.push(map_sql(row, capability, correlation_id)?);
        }
        page
    };
    map_sql(transaction.commit(), capability, correlation_id)?;
    Ok(page)
}

fn verify_evidence_file(
    kernel: &SqliteKernel,
    digest: &str,
    size_bytes: i64,
    stored_path: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    let digest_hex = canonical_digest_hex(digest)
        .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let expected_relative = relative_evidence_path(digest_hex);
    if path_to_storage_value(&expected_relative) != stored_path {
        return integrity_failure(capability, correlation_id);
    }
    let file = kernel
        .open_evidence_file(digest_hex)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let (actual_digest, actual_size) = sha256_reader(file)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let expected_size = u64::try_from(size_bytes)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    if encode_hex(&actual_digest) != digest_hex || actual_size != expected_size {
        return integrity_failure(capability, correlation_id);
    }
    Ok(())
}

fn integrity_failure<T>(
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<T> {
    Err(Box::new(FastiProblem::integrity_failed(
        capability,
        correlation_id,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{
        EvidenceUploadPort, EvidenceUploadRequest, IdentityPort,
        RegisterNamespaceDefinitionCommand, ScopeKey,
    };
    use fasti_domain::{
        Grain, InterpretationId, NamespaceDefinition, NamespaceLicencePosture, ObservationId,
        OccurrenceId, OperationId, ReceiptId, RecordId, RequestCorrelationId,
    };
    use std::fs;
    use std::io;
    use std::num::NonZeroU64;

    struct MutatingSink {
        bytes: Vec<u8>,
        kernel: SqliteKernel,
        workspace_id: String,
        mutated: bool,
    }

    impl Write for MutatingSink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            if !self.mutated {
                let connection = self
                    .kernel
                    .inner
                    .connection
                    .lock()
                    .expect("SQLite connection");
                connection
                    .execute(
                        "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES ('rec_concurrent_export', ?1, 'movie', 'active', '2026-08-24T00:00:00.000000Z')",
                        [&self.workspace_id],
                    )
                    .expect("concurrent record insert");
                self.mutated = true;
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn grant_scope(node: &TestNode, scope: ScopeKey) {
        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        connection
            .execute(
                "INSERT OR IGNORE INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                params![node.access.grant_id().to_string(), scope.as_str()],
            )
            .expect("grant staged scope");
    }

    fn grant_verify_scope(node: &TestNode) {
        grant_scope(node, ScopeKey::WorkspaceVerify);
    }

    fn portability_limits(max_rows: u64, max_entry_bytes: u64) -> PortabilityLimits {
        let general = NonZeroU64::new(16 * 1024 * 1024).expect("non-zero general limit");
        let entries = NonZeroU64::new(1_024).expect("non-zero entry count");
        let one = NonZeroU64::new(1).expect("non-zero unit limit");
        PortabilityLimits {
            max_snapshot_bytes: general,
            max_wal_growth_bytes: general,
            max_archive_bytes: general,
            max_uncompressed_bytes: general,
            max_entry_bytes: NonZeroU64::new(max_entry_bytes).expect("non-zero entry bytes"),
            max_entries: entries,
            max_rows_per_stream: NonZeroU64::new(max_rows).expect("non-zero row limit"),
            max_path_bytes: general,
            max_path_depth: entries,
            max_decompression_ratio: entries,
            scratch_ceiling_bytes: general,
            cleanup_reserve_bytes: general,
            backup_step_pages: one,
            backup_step_millis: one,
        }
    }

    fn insert_records(node: &TestNode, count: usize) {
        let mut connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let transaction = connection.transaction().expect("record transaction");
        for _ in 0..count {
            transaction
                .execute(
                    "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, 'film', 'active', '2026-08-24T00:00:00.000000Z')",
                    params![RecordId::new_v7().to_string(), node.access.workspace_id().to_string()],
                )
                .expect("insert record");
        }
        transaction.commit().expect("commit records");
    }

    fn open_read_only_database(node: &TestNode) -> Connection {
        let connection = Connection::open_with_flags(
            node.kernel.database_path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open read-only snapshot connection");
        connection
            .pragma_update(None, "query_only", "ON")
            .expect("set query-only mode");
        connection
    }

    fn encode_single_value(value_sql: &str, max_bytes: u64) -> ApplicationResult<Vec<u8>> {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        let mut statement = connection.prepare(value_sql).expect("value query");
        let column_names: Vec<String> = statement
            .column_names()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        let mut output_columns: Vec<usize> = (0..column_names.len()).collect();
        output_columns
            .sort_unstable_by(|left, right| column_names[*left].cmp(&column_names[*right]));
        let mut rows = statement.query([]).expect("query value");
        let row = rows.next().expect("read value row").expect("one value row");
        let encoded = encode_row(
            row,
            &column_names,
            &output_columns,
            &[],
            max_bytes,
            CapabilityKey::ExportWorkspace,
            RequestCorrelationId::new_v7(),
        )?;
        Ok(encoded.0)
    }

    #[test]
    fn offline_verify_maps_shared_lock_contention_without_panicking() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let _held = crate::LockedDataRoot::acquire(&root).expect("held data-root lock");
        let open_error = SqliteKernel::open(&root).expect_err("second lock must fail");
        let correlation_id = RequestCorrelationId::new_v7();

        let problem = map_offline_verify_open_error(open_error, correlation_id);
        assert_eq!(problem.code(), ProblemCode::DataRootLocked);
        assert_eq!(problem.capability(), CapabilityKey::VerifyWorkspace);
        assert_eq!(problem.correlation_id(), correlation_id);
        assert!(CapabilityKey::VerifyWorkspace
            .allowed_problem_codes()
            .contains(&ProblemCode::DataRootLocked));
    }

    #[test]
    fn offline_verify_fails_closed_when_platform_locking_is_unsupported() {
        let correlation_id = RequestCorrelationId::new_v7();
        let problem =
            map_offline_verify_open_error(StoreOpenError::UnsupportedPlatform, correlation_id);

        assert_eq!(problem.code(), ProblemCode::IntegrityFailed);
        assert_eq!(problem.capability(), CapabilityKey::VerifyWorkspace);
        assert_eq!(problem.correlation_id(), correlation_id);
    }

    #[test]
    fn offline_restore_reports_unsupported_platform_without_mutation() {
        let correlation_id = RequestCorrelationId::new_v7();
        let problem = map_offline_open_error(
            StoreOpenError::UnsupportedPlatform,
            CapabilityKey::RestoreWorkspace,
            correlation_id,
        );

        assert_eq!(problem.code(), ProblemCode::UnsupportedPlatform);
        assert_eq!(problem.capability(), CapabilityKey::RestoreWorkspace);
        assert_eq!(problem.correlation_id(), correlation_id);
        assert_eq!(
            problem.safe_state(),
            fasti_application::SafeState::NoMutation
        );
    }

    fn sqlite_open_error(result_code: i32) -> StoreOpenError {
        StoreOpenError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(result_code),
            None,
        ))
    }

    #[test]
    fn offline_verify_maps_invariant_and_corrupt_open_failures_to_integrity() {
        let failures = [
            (
                "unsafe path",
                StoreOpenError::UnsafePath {
                    path: PathBuf::from("unsafe-link"),
                    reason: "symbolic link",
                },
            ),
            (
                "journal mode",
                StoreOpenError::JournalMode("delete".to_owned()),
            ),
            ("synchronous level", StoreOpenError::SynchronousLevel(1)),
            (
                "schema version",
                StoreOpenError::SchemaVersion {
                    expected: 9,
                    actual: 8,
                },
            ),
            ("restore activation", StoreOpenError::RestoreActivation),
            (
                "SQLite corrupt",
                sqlite_open_error(rusqlite::ffi::SQLITE_CORRUPT),
            ),
            (
                "SQLite not a database",
                sqlite_open_error(rusqlite::ffi::SQLITE_NOTADB),
            ),
            (
                "non-transient I/O",
                StoreOpenError::Io(io::Error::from(ErrorKind::PermissionDenied)),
            ),
            (
                "non-transient SQLite",
                sqlite_open_error(rusqlite::ffi::SQLITE_CANTOPEN),
            ),
        ];

        for (label, failure) in failures {
            let problem = map_offline_verify_open_error(failure, RequestCorrelationId::new_v7());
            assert_eq!(problem.code(), ProblemCode::IntegrityFailed, "{label}");
        }
    }

    #[test]
    fn offline_verify_maps_only_explicitly_transient_open_failures_to_storage() {
        let failures = [
            (
                "interrupted I/O",
                StoreOpenError::Io(io::Error::from(ErrorKind::Interrupted)),
            ),
            (
                "would-block I/O",
                StoreOpenError::Io(io::Error::from(ErrorKind::WouldBlock)),
            ),
            (
                "timed-out I/O",
                StoreOpenError::Io(io::Error::from(ErrorKind::TimedOut)),
            ),
            ("SQLite busy", sqlite_open_error(rusqlite::ffi::SQLITE_BUSY)),
            (
                "SQLite locked",
                sqlite_open_error(rusqlite::ffi::SQLITE_LOCKED),
            ),
            (
                "SQLite interrupted",
                sqlite_open_error(rusqlite::ffi::SQLITE_INTERRUPT),
            ),
        ];

        for (label, failure) in failures {
            let problem = map_offline_verify_open_error(failure, RequestCorrelationId::new_v7());
            assert_eq!(problem.code(), ProblemCode::StorageUnavailable, "{label}");
        }
    }

    #[test]
    fn verify_workspace_accepts_a_clean_workspace() {
        let node = TestNode::new();
        grant_verify_scope(&node);
        let result = node
            .kernel
            .verify_workspace(VerifyWorkspaceQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("verify clean workspace");

        assert_eq!(result.workspace_id(), node.access.workspace_id());
        assert_eq!(result.observations_verified(), 0);
        assert_eq!(result.evidence_verified(), 0);
        assert_eq!(result.corrections_verified(), 0);
    }

    #[test]
    fn verify_workspace_rejects_tampered_evidence_bytes() {
        let node = TestNode::new();
        grant_verify_scope(&node);
        let bytes = b"immutable verification evidence";
        let mut upload = node
            .kernel
            .begin_evidence_upload(EvidenceUploadRequest::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Some(bytes.len() as u64),
            ))
            .expect("begin evidence upload");
        upload.write_chunk(bytes).expect("write evidence");
        let evidence = upload.finish().expect("finish evidence");
        let digest_hex =
            canonical_digest_hex(evidence.digest().as_str()).expect("canonical digest");
        let path = node
            .kernel
            .inner
            .current_root
            .join(relative_evidence_path(digest_hex));
        fs::write(path, b"tampered verification evidence").expect("tamper evidence");

        let error = node
            .kernel
            .verify_workspace(VerifyWorkspaceQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect_err("tampered evidence must fail verification");
        assert_eq!(error.code(), ProblemCode::IntegrityFailed);
    }

    #[test]
    fn export_includes_registered_namespace_definitions() {
        let node = TestNode::new();
        grant_scope(&node, ScopeKey::WorkspaceExport);
        node.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                NamespaceDefinition::try_new(
                    "imdb_title",
                    "IMDb title",
                    [Grain::Film, Grain::Series],
                    "^tt[0-9]+$",
                    "trim",
                    NamespaceLicencePosture::IdentifiersOnly,
                )
                .expect("valid namespace"),
            ))
            .expect("register namespace");

        let mut bytes = Vec::new();
        let outcome = node
            .kernel
            .export_workspace(
                ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), node.access),
                &mut bytes,
            )
            .expect("export namespace");
        assert_eq!(
            outcome.count(WorkspaceExportEntity::NamespaceDefinitions),
            1
        );
        let namespace = std::str::from_utf8(&bytes)
            .expect("UTF-8 export")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON line"))
            .find(|line| line.get("namespace") == Some(&serde_json::json!("imdb_title")))
            .expect("namespace row");
        assert_eq!(
            namespace.get("supported_grains"),
            Some(&serde_json::json!("film,series"))
        );
    }

    #[test]
    fn export_writes_every_operation_across_keyset_pages() {
        let node = TestNode::new();
        grant_scope(&node, ScopeKey::WorkspaceExport);
        let evidence = node.upload(b"operation export evidence");
        let observation_id = ObservationId::new_v7();
        let occurrence_id = OccurrenceId::new_v7();
        let interpretation_id = InterpretationId::new_v7();
        let created_at = "2026-08-24T00:00:00.000000Z";
        let operation_count = usize::try_from(EXPORT_PAGE).expect("positive page size") + 1;

        {
            let mut connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            let transaction = connection
                .transaction()
                .expect("operation fixture transaction");
            transaction
                .execute(
                    r#"
                    INSERT INTO observations(
                        observation_id, workspace_id, profile_id, source_client_id,
                        evidence_id, observed_at_json, received_at, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, '{}', ?6, ?6)
                    "#,
                    params![
                        observation_id.to_string(),
                        node.access.workspace_id().to_string(),
                        node.access.profile_id().to_string(),
                        node.access.client_id().to_string(),
                        evidence.evidence_id().to_string(),
                        created_at
                    ],
                )
                .expect("observation");
            transaction
                .execute(
                    "INSERT INTO occurrences(occurrence_id, workspace_id, profile_id, observation_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        occurrence_id.to_string(),
                        node.access.workspace_id().to_string(),
                        node.access.profile_id().to_string(),
                        observation_id.to_string(),
                        created_at
                    ],
                )
                .expect("occurrence");
            transaction
                .execute(
                    "INSERT INTO interpretations(interpretation_id, observation_id, occurrence_id, state, created_at) VALUES (?1, ?2, ?3, 'unresolved', ?4)",
                    params![
                        interpretation_id.to_string(),
                        observation_id.to_string(),
                        occurrence_id.to_string(),
                        created_at
                    ],
                )
                .expect("interpretation");

            for _ in 0..operation_count {
                let operation_id = OperationId::new_v7();
                let receipt_id = ReceiptId::new_v7();
                transaction
                    .execute(
                        r#"
                        INSERT INTO receipts(
                            receipt_id, operation_id, workspace_id, profile_id, client_id,
                            capability_key, observation_id, occurrence_id, interpretation_id,
                            evidence_id, payload_digest, resolution, received_at, committed_at,
                            created_at
                        ) VALUES (
                            ?1, ?2, ?3, ?4, ?5, 'observation.accept', ?6, ?7, ?8,
                            ?9, ?10, 'unresolved', ?11, ?11, ?11
                        )
                        "#,
                        params![
                            receipt_id.to_string(),
                            operation_id.to_string(),
                            node.access.workspace_id().to_string(),
                            node.access.profile_id().to_string(),
                            node.access.client_id().to_string(),
                            observation_id.to_string(),
                            occurrence_id.to_string(),
                            interpretation_id.to_string(),
                            evidence.evidence_id().to_string(),
                            evidence.digest().to_string(),
                            created_at
                        ],
                    )
                    .expect("receipt");
                transaction
                    .execute(
                        r#"
                        INSERT INTO operations(
                            workspace_id, client_id, operation_id, capability_key,
                            semantic_digest, receipt_id, created_at
                        ) VALUES (?1, ?2, ?3, 'observation.accept', ?4, ?5, ?6)
                        "#,
                        params![
                            node.access.workspace_id().to_string(),
                            node.access.client_id().to_string(),
                            operation_id.to_string(),
                            evidence.digest().to_string(),
                            receipt_id.to_string(),
                            created_at
                        ],
                    )
                    .expect("operation");
            }
            transaction.commit().expect("commit operation fixtures");
        }

        let expected: u64 = {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .query_row(
                    "SELECT COUNT(*) FROM operations WHERE workspace_id = ?1",
                    [node.access.workspace_id().to_string()],
                    |row| row.get(0),
                )
                .expect("expected operation count")
        };
        let mut bytes = Vec::new();
        let outcome = node
            .kernel
            .export_workspace(
                ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), node.access),
                &mut bytes,
            )
            .expect("export workspace");

        assert_eq!(expected, operation_count as u64);
        assert_eq!(outcome.count(WorkspaceExportEntity::Operations), expected);
        let lines: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
            .expect("UTF-8 export")
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON export line"))
            .collect();
        let operations = lines
            .iter()
            .position(|line| line.get("section") == Some(&serde_json::json!("operations")))
            .expect("operations marker");
        let trailer = lines
            .iter()
            .position(|line| line.get("section") == Some(&serde_json::json!("trailer")))
            .expect("trailer marker");
        assert_eq!(trailer - operations - 1, operation_count);
    }

    #[test]
    fn export_reports_concurrent_count_change_as_storage_unavailable() {
        let node = TestNode::new();
        grant_scope(&node, ScopeKey::WorkspaceExport);
        let mut sink = MutatingSink {
            bytes: Vec::new(),
            kernel: node.kernel.clone(),
            workspace_id: node.access.workspace_id().to_string(),
            mutated: false,
        };

        let error = node
            .kernel
            .export_workspace(
                ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), node.access),
                &mut sink,
            )
            .expect_err("concurrent mutation must fail export");

        assert!(sink.mutated);
        assert_eq!(error.code(), ProblemCode::StorageUnavailable);
    }

    #[test]
    fn archive_entity_descriptor_is_deterministic_across_read_only_reopen() {
        let node = TestNode::new();
        insert_records(&node, 3);
        let correlation_id = RequestCorrelationId::new_v7();
        let limits = portability_limits(16, 64 * 1024);

        let (first_descriptor, first_bytes) = {
            let connection = open_read_only_database(&node);
            let mut bytes = Vec::new();
            let descriptor = stream_archive_entity(
                &connection,
                node.access.workspace_id(),
                WorkspaceExportEntity::Records,
                limits,
                &mut bytes,
                &mut || Ok(()),
                correlation_id,
            )
            .expect("first entity stream");
            (descriptor, bytes)
        };
        let (second_descriptor, second_bytes) = {
            let connection = open_read_only_database(&node);
            let mut bytes = Vec::new();
            let descriptor = stream_archive_entity(
                &connection,
                node.access.workspace_id(),
                WorkspaceExportEntity::Records,
                limits,
                &mut bytes,
                &mut || Ok(()),
                correlation_id,
            )
            .expect("reopened entity stream");
            (descriptor, bytes)
        };

        assert_eq!(first_descriptor, second_descriptor);
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(first_descriptor.row_count(), 3);
        assert_eq!(first_descriptor.byte_length(), first_bytes.len() as u64);
    }

    #[test]
    fn archive_entity_freezes_exact_ndjson_key_order_and_bytes() {
        let node = TestNode::new();
        let connection = open_read_only_database(&node);
        let created_at: String = connection
            .query_row(
                "SELECT created_at FROM workspaces WHERE workspace_id = ?1",
                [node.access.workspace_id().to_string()],
                |row| row.get(0),
            )
            .expect("workspace creation time");
        let mut bytes = Vec::new();
        stream_archive_entity(
            &connection,
            node.access.workspace_id(),
            WorkspaceExportEntity::Workspaces,
            portability_limits(1, 4 * 1024),
            &mut bytes,
            &mut || Ok(()),
            RequestCorrelationId::new_v7(),
        )
        .expect("workspace stream");
        let expected = format!(
            "{{\"created_at\":{},\"workspace_id\":{}}}\n",
            serde_json::to_string(&created_at).expect("creation JSON"),
            serde_json::to_string(&node.access.workspace_id().to_string()).expect("workspace JSON")
        );

        assert_eq!(bytes, expected.as_bytes());
    }

    #[test]
    fn archive_row_rejects_nonportable_integers() {
        for value in ["-1", "9007199254740992"] {
            let error = encode_single_value(&format!("SELECT {value} AS ordinal"), 4 * 1024)
                .expect_err("nonportable integer must fail closed");
            assert_eq!(error.code(), ProblemCode::IntegrityFailed);
        }
    }

    #[test]
    fn archive_row_rejects_escape_expansion_before_page_materialization() {
        let error = encode_single_value(
            "SELECT printf('%.*c', 1000000, char(1)) AS hostile_text",
            4 * 1024,
        )
        .expect_err("escape-expanded row must remain bounded");

        assert_eq!(error.code(), ProblemCode::CapacityExceeded);
    }

    #[test]
    fn archive_namespace_entity_uses_frozen_key_order_without_section_markers() {
        let node = TestNode::new();
        assert!(
            EXPORT_SECTIONS
                .iter()
                .map(|section| section.entity)
                .eq(WorkspaceExportEntity::ALL),
            "store section order must match frozen archive-v1 order"
        );
        for (key, label) in [("zeta", "Zeta"), ("alpha", "Alpha")] {
            node.kernel
                .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    NamespaceDefinition::try_new(
                        key,
                        label,
                        [Grain::Film],
                        "^[a-z]+$",
                        "identity",
                        NamespaceLicencePosture::Unknown,
                    )
                    .expect("valid namespace"),
                ))
                .expect("register namespace");
        }
        let connection = open_read_only_database(&node);
        let mut bytes = Vec::new();
        let descriptor = stream_archive_entity(
            &connection,
            node.access.workspace_id(),
            WorkspaceExportEntity::NamespaceDefinitions,
            portability_limits(16, 64 * 1024),
            &mut bytes,
            &mut || Ok(()),
            RequestCorrelationId::new_v7(),
        )
        .expect("namespace entity stream");
        let rows: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
            .expect("UTF-8 NDJSON")
            .lines()
            .map(|line| serde_json::from_str(line).expect("namespace JSON row"))
            .collect();

        assert_eq!(descriptor.row_count(), 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["namespace"], "alpha");
        assert_eq!(rows[1]["namespace"], "zeta");
        assert!(rows.iter().all(|row| row.get("section").is_none()));
    }

    #[test]
    fn archive_entity_rejects_row_and_byte_ceiling_breaches() {
        let node = TestNode::new();
        insert_records(&node, 2);
        let connection = open_read_only_database(&node);
        let correlation_id = RequestCorrelationId::new_v7();

        let mut row_limited = Vec::new();
        let row_error = stream_archive_entity(
            &connection,
            node.access.workspace_id(),
            WorkspaceExportEntity::Records,
            portability_limits(1, 64 * 1024),
            &mut row_limited,
            &mut || Ok(()),
            correlation_id,
        )
        .expect_err("row ceiling must reject the entity");
        assert_eq!(row_error.code(), ProblemCode::CapacityExceeded);
        assert!(row_limited.is_empty());

        let mut byte_limited = Vec::new();
        let byte_error = stream_archive_entity(
            &connection,
            node.access.workspace_id(),
            WorkspaceExportEntity::Records,
            portability_limits(16, 1),
            &mut byte_limited,
            &mut || Ok(()),
            correlation_id,
        )
        .expect_err("byte ceiling must reject the entity");
        assert_eq!(byte_error.code(), ProblemCode::CapacityExceeded);
        assert!(byte_limited.is_empty());
    }

    #[test]
    fn archive_entity_monitor_can_cancel_before_each_page() {
        let node = TestNode::new();
        insert_records(&node, EXPORT_PAGE as usize + 1);
        let connection = open_read_only_database(&node);
        let correlation_id = RequestCorrelationId::new_v7();
        let mut calls = 0_u64;
        let mut bytes = Vec::new();

        let error = stream_archive_entity(
            &connection,
            node.access.workspace_id(),
            WorkspaceExportEntity::Records,
            portability_limits(1_024, 1024 * 1024),
            &mut bytes,
            &mut || {
                calls += 1;
                if calls == 2 {
                    Err(Box::new(FastiProblem::export_canceled(correlation_id)))
                } else {
                    Ok(())
                }
            },
            correlation_id,
        )
        .expect_err("second-page monitor cancellation must stop streaming");

        assert_eq!(error.code(), ProblemCode::ExportCanceled);
        assert_eq!(calls, 2);
        assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 256);
    }

    #[test]
    fn schema_fingerprint_changes_with_actual_schema_mutation() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TABLE records(record_id TEXT PRIMARY KEY) STRICT; PRAGMA user_version = 4;",
            )
            .expect("initial schema");
        let correlation_id = RequestCorrelationId::new_v7();
        let before = schema_fingerprint(&connection, correlation_id).expect("schema fingerprint");

        connection
            .execute_batch("CREATE INDEX records_order_idx ON records(record_id);")
            .expect("schema mutation");
        let after = schema_fingerprint(&connection, correlation_id).expect("mutated fingerprint");

        assert_eq!(before.migration_version(), 4);
        assert_eq!(after.migration_version(), 4);
        assert_ne!(before.digest(), after.digest());
    }

    #[test]
    fn snapshot_blob_inventory_is_sorted_unique_and_path_checked() {
        let node = TestNode::new();
        let first = node.upload(b"first inventory blob");
        let second = node.upload(b"second inventory blob");
        let correlation_id = RequestCorrelationId::new_v7();
        let connection = open_read_only_database(&node);
        let inventory = snapshot_evidence_blobs(
            &connection,
            node.access.workspace_id(),
            portability_limits(16, 64 * 1024),
            correlation_id,
        )
        .expect("snapshot blob inventory");

        assert_eq!(inventory.len(), 2);
        assert!(inventory.windows(2).all(|pair| {
            pair[0].descriptor().evidence_id().uuid() < pair[1].descriptor().evidence_id().uuid()
        }));
        let ids: HashSet<_> = inventory
            .iter()
            .map(|blob| blob.descriptor().evidence_id())
            .collect();
        let digests: HashSet<_> = inventory
            .iter()
            .map(|blob| blob.descriptor().digest().clone())
            .collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(digests.len(), 2);
        assert!(ids.contains(&first.evidence_id()));
        assert!(ids.contains(&second.evidence_id()));
        for blob in &inventory {
            let digest_hex = canonical_digest_hex(blob.descriptor().digest().as_str())
                .expect("canonical descriptor digest");
            assert_eq!(blob.relative_path(), relative_evidence_path(digest_hex));
        }
    }

    #[test]
    fn snapshot_blob_inventory_rejects_noncanonical_stored_metadata() {
        let node = TestNode::new();
        let evidence = node.upload(b"path validation blob");
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "UPDATE evidence SET relative_path = '../payload' WHERE evidence_id = ?1",
                    [evidence.evidence_id().to_string()],
                )
                .expect("corrupt stored path");
        }
        let connection = open_read_only_database(&node);
        let error = snapshot_evidence_blobs(
            &connection,
            node.access.workspace_id(),
            portability_limits(16, 64 * 1024),
            RequestCorrelationId::new_v7(),
        )
        .expect_err("noncanonical evidence path must fail inventory");
        assert_eq!(error.code(), ProblemCode::IntegrityFailed);
    }
}

// ---------------------------------------------------------------------------
// B3 workspace export
// ---------------------------------------------------------------------------

/// Rows read per bounded export page.
const EXPORT_PAGE: i64 = 256;

/// Start sentinel for a keyset cursor column.
///
/// Every exported key column is TEXT or INTEGER; the schema declares no REAL
/// or BLOB column, and `write_row` rejects those types if one ever appears.
#[derive(Debug, Clone, Copy)]
enum CursorColumn {
    /// Sorts before any non-empty TEXT key. Every identifier column is a
    /// non-empty ULID-style string.
    Text(usize),
    /// Sorts before any non-negative INTEGER key.
    NonNegativeInteger(usize),
}

impl CursorColumn {
    const fn index(self) -> usize {
        match self {
            Self::Text(index) | Self::NonNegativeInteger(index) => index,
        }
    }

    fn value(self) -> rusqlite::types::Value {
        match self {
            Self::Text(_) => rusqlite::types::Value::Text(String::new()),
            Self::NonNegativeInteger(_) => rusqlite::types::Value::Integer(-1),
        }
    }
}

/// One durable entity stream, and how to page it deterministically.
///
/// `sql` binds `?1` = workspace id, then one parameter per cursor column, then
/// the page limit last. Every statement orders by its full primary key, so the
/// last row of a page is a total-order cursor for the next one. Ordering by a
/// non-unique column would let fragmented pages reorder rows between exports.
struct ExportSection {
    entity: WorkspaceExportEntity,
    sql: &'static str,
    /// Row count for the change fence. Binds `?1` = workspace id. It must
    /// count exactly the rows `sql` pages, including the same joins.
    count_sql: &'static str,
    /// Columns, in SQL keyset order, that form the next-page cursor.
    cursor_columns: &'static [CursorColumn],
}

const EXPORT_SECTIONS: &[ExportSection] = &[
    ExportSection {
        entity: WorkspaceExportEntity::Workspaces,
        sql: "SELECT workspace_id, created_at FROM workspaces \
              WHERE workspace_id = ?1 AND workspace_id > ?2 \
              ORDER BY workspace_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM workspaces WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::Profiles,
        sql: "SELECT profile_id, workspace_id, created_at FROM profiles \
              WHERE workspace_id = ?1 AND profile_id > ?2 \
              ORDER BY profile_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM profiles WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    // `current_credential_epoch` is deliberately absent: it is the live epoch
    // fence, and exporting it would let a stale credential re-validate after a
    // restore. Only the non-secret client shell is exported, because
    // observations and receipts carry client foreign keys.
    ExportSection {
        entity: WorkspaceExportEntity::Clients,
        sql: "SELECT client_id, workspace_id, status, created_at FROM clients \
              WHERE workspace_id = ?1 AND client_id > ?2 \
              ORDER BY client_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM clients WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::Records,
        sql: "SELECT record_id, workspace_id, grain, status, created_at FROM records \
              WHERE workspace_id = ?1 AND record_id > ?2 \
              ORDER BY record_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM records WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::NamespaceDefinitions,
        sql: "SELECT workspace_id, namespace, label, supported_grains, id_pattern, \
                     normalization, licence_posture, created_at \
              FROM namespace_definitions \
              WHERE workspace_id = ?1 AND namespace > ?2 \
              ORDER BY namespace LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM namespace_definitions WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(1)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::ExternalIdentifiers,
        sql: "SELECT external_identifier_id, workspace_id, record_id, namespace, grain, value, \
                     created_at \
              FROM external_identifiers \
              WHERE workspace_id = ?1 AND external_identifier_id > ?2 \
              ORDER BY external_identifier_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM external_identifiers WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    // Evidence is exported as a manifest of digests and sizes. Blob content is
    // not embedded in format version 1; see `evidence_content` in the header.
    // `relative_path` is deliberately absent because it is a redundant column
    // that is re-derived and verified from the digest.
    ExportSection {
        entity: WorkspaceExportEntity::Evidence,
        sql: "SELECT evidence_id, workspace_id, digest, size_bytes, created_at FROM evidence \
              WHERE workspace_id = ?1 AND evidence_id > ?2 \
              ORDER BY evidence_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM evidence WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::Observations,
        sql: "SELECT observation_id, workspace_id, profile_id, source_client_id, evidence_id, \
                     occurred_at_json, observed_at_json, received_at, created_at \
              FROM observations \
              WHERE workspace_id = ?1 AND observation_id > ?2 \
              ORDER BY observation_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM observations WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    // Clues carry no workspace column; scope is reached through observations.
    ExportSection {
        entity: WorkspaceExportEntity::ObservationClues,
        sql: "SELECT c.observation_id AS observation_id, c.ordinal AS ordinal, \
                     c.namespace AS namespace, c.grain AS grain, c.value AS value \
              FROM observation_clues c \
              JOIN observations o ON o.observation_id = c.observation_id \
              WHERE o.workspace_id = ?1 AND (c.observation_id, c.ordinal) > (?2, ?3) \
              ORDER BY c.observation_id, c.ordinal LIMIT ?4",
        count_sql: "SELECT COUNT(*) FROM observation_clues c JOIN observations o ON o.observation_id = c.observation_id WHERE o.workspace_id = ?1",
        cursor_columns: &[
            CursorColumn::Text(0),
            CursorColumn::NonNegativeInteger(1),
        ],
    },
    ExportSection {
        entity: WorkspaceExportEntity::Occurrences,
        sql: "SELECT occurrence_id, workspace_id, profile_id, observation_id, record_id, \
                     occurred_at_json, created_at \
              FROM occurrences \
              WHERE workspace_id = ?1 AND occurrence_id > ?2 \
              ORDER BY occurrence_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM occurrences WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    // Interpretations carry no workspace column either.
    ExportSection {
        entity: WorkspaceExportEntity::Interpretations,
        sql: "SELECT i.interpretation_id AS interpretation_id, \
                     i.observation_id AS observation_id, i.occurrence_id AS occurrence_id, \
                     i.prior_interpretation_id AS prior_interpretation_id, \
                     i.record_id AS record_id, i.state AS state, i.created_at AS created_at \
              FROM interpretations i \
              JOIN observations o ON o.observation_id = i.observation_id \
              WHERE o.workspace_id = ?1 AND i.interpretation_id > ?2 \
              ORDER BY i.interpretation_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM interpretations i JOIN observations o ON o.observation_id = i.observation_id WHERE o.workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::ReviewItems,
        sql: "SELECT review_item_id, workspace_id, profile_id, observation_id, \
                     current_interpretation_id, status, created_at, updated_at \
              FROM review_items \
              WHERE workspace_id = ?1 AND review_item_id > ?2 \
              ORDER BY review_item_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM review_items WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::ReviewCandidates,
        sql: "SELECT c.review_item_id AS review_item_id, c.record_id AS record_id \
              FROM review_candidates c \
              JOIN review_items r ON r.review_item_id = c.review_item_id \
              WHERE r.workspace_id = ?1 AND (c.review_item_id, c.record_id) > (?2, ?3) \
              ORDER BY c.review_item_id, c.record_id LIMIT ?4",
        count_sql: "SELECT COUNT(*) FROM review_candidates c JOIN review_items r ON r.review_item_id = c.review_item_id WHERE r.workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0), CursorColumn::Text(1)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::Corrections,
        sql: "SELECT correction_id, workspace_id, profile_id, observation_id, \
                     prior_interpretation_id, replacement_interpretation_id, actor_client_id, \
                     record_id, reason, created_at \
              FROM corrections \
              WHERE workspace_id = ?1 AND correction_id > ?2 \
              ORDER BY correction_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM corrections WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    // Ordered by receipt_id, not the AUTOINCREMENT sequence: `sequence` is
    // node-local and would not survive a restore into a clean database.
    ExportSection {
        entity: WorkspaceExportEntity::Receipts,
        sql: "SELECT receipt_id, operation_id, workspace_id, profile_id, client_id, \
                     capability_key, observation_id, occurrence_id, interpretation_id, \
                     record_id, review_item_id, evidence_id, payload_digest, resolution, \
                     received_at, committed_at, created_at \
              FROM receipts \
              WHERE workspace_id = ?1 AND receipt_id > ?2 \
              ORDER BY receipt_id LIMIT ?3",
        count_sql: "SELECT COUNT(*) FROM receipts WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(0)],
    },
    ExportSection {
        entity: WorkspaceExportEntity::Operations,
        sql: "SELECT workspace_id, client_id, operation_id, capability_key, semantic_digest, \
                     receipt_id, created_at \
              FROM operations \
              WHERE workspace_id = ?1 AND (client_id, operation_id) > (?2, ?3) \
              ORDER BY client_id, operation_id LIMIT ?4",
        count_sql: "SELECT COUNT(*) FROM operations WHERE workspace_id = ?1",
        cursor_columns: &[CursorColumn::Text(1), CursorColumn::Text(2)],
    },
];

/// The migration version and a digest of the actual SQLite schema in a frozen
/// snapshot. Physical root-page allocation is excluded because it is not
/// schema meaning and can change after a byte-equivalent restore or VACUUM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SchemaFingerprint {
    migration_version: u32,
    digest: Sha256Digest,
}

impl SchemaFingerprint {
    pub(crate) const fn migration_version(&self) -> u32 {
        self.migration_version
    }

    pub(crate) const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }
}

/// One validated evidence entry discovered in the frozen snapshot database.
///
/// The relative path is retained for the later archive copier. This helper
/// does not open or copy the immutable blob bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnapshotEvidenceBlob {
    descriptor: WorkspaceBlobDescriptor,
    relative_path: PathBuf,
}

impl SnapshotEvidenceBlob {
    pub(crate) const fn descriptor(&self) -> &WorkspaceBlobDescriptor {
        &self.descriptor
    }

    pub(crate) fn relative_path(&self) -> &Path {
        &self.relative_path
    }
}

/// Streams one frozen archive-v1 entity as plain NDJSON.
///
/// `connection` must be the caller-owned, read-only connection to the frozen
/// snapshot database. The monitor runs before every bounded page query. It is
/// responsible for cancellation and any live authorization/resource fence the
/// caller must retain while disclosing snapshot bytes.
pub(crate) fn stream_archive_entity(
    connection: &Connection,
    workspace_id: WorkspaceId,
    entity: WorkspaceExportEntity,
    limits: PortabilityLimits,
    sink: &mut dyn Write,
    monitor: &mut dyn FnMut() -> ApplicationResult<()>,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<WorkspaceStreamDescriptor> {
    let section = section_for_entity(entity, correlation_id)?;
    let workspace = workspace_id.to_string();
    stream_entity_pages(
        section,
        limits
            .max_rows_per_stream
            .get()
            .min(MAX_PORTABLE_JSON_INTEGER),
        limits.max_entry_bytes.get().min(MAX_PORTABLE_JSON_INTEGER),
        sink,
        correlation_id,
        |cursor, max_page_bytes| {
            monitor()?;
            read_section_page(
                connection,
                section,
                &workspace,
                cursor,
                max_page_bytes,
                correlation_id,
            )
        },
    )
}

/// Fingerprints the migration version plus every semantically relevant,
/// non-SQLite-owned row in `sqlite_schema`, in a fixed binary collation order.
pub(crate) fn schema_fingerprint(
    connection: &Connection,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<SchemaFingerprint> {
    let capability = CapabilityKey::ExportWorkspace;
    let version = map_sql(
        connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0)),
        capability,
        correlation_id,
    )?;
    let migration_version = u32::try_from(version)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;

    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT type, name, tbl_name, sql
            FROM sqlite_schema
            WHERE name NOT LIKE 'sqlite_%'
            ORDER BY
                type COLLATE BINARY,
                name COLLATE BINARY,
                tbl_name COLLATE BINARY,
                sql COLLATE BINARY
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        }),
        capability,
        correlation_id,
    )?;

    let mut hasher = Sha256::new();
    hasher.update(b"fasti.sqlite-schema.v1\0");
    hasher.update(migration_version.to_be_bytes());
    for row in rows {
        let (kind, name, table, sql) = map_sql(row, capability, correlation_id)?;
        hash_schema_field(&mut hasher, Some(&kind), capability, correlation_id)?;
        hash_schema_field(&mut hasher, Some(&name), capability, correlation_id)?;
        hash_schema_field(&mut hasher, Some(&table), capability, correlation_id)?;
        hash_schema_field(&mut hasher, sql.as_deref(), capability, correlation_id)?;
    }
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(SchemaFingerprint {
        migration_version,
        digest: Sha256Digest::parse(format!("sha256:{}", encode_hex(&digest)))
            .expect("SHA-256 output is canonical lowercase hexadecimal"),
    })
}

fn hash_schema_field(
    hasher: &mut Sha256,
    value: Option<&str>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    match value {
        None => hasher.update([0]),
        Some(value) => {
            hasher.update([1]);
            let length = u64::try_from(value.len()).map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?;
            hasher.update(length.to_be_bytes());
            hasher.update(value.as_bytes());
        }
    }
    Ok(())
}

/// Inventories snapshot evidence without opening or copying blob files.
///
/// Every descriptor is strictly ordered by parsed `EvidenceId`; both IDs and
/// digests are unique. Stored digest, size, and path columns must be canonical
/// before an archive copier is allowed to consume the inventory.
pub(crate) fn snapshot_evidence_blobs(
    connection: &Connection,
    workspace_id: WorkspaceId,
    limits: PortabilityLimits,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<SnapshotEvidenceBlob>> {
    let capability = CapabilityKey::ExportWorkspace;
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT evidence_id, digest, size_bytes, relative_path
            FROM evidence
            WHERE workspace_id = ?1
            ORDER BY evidence_id COLLATE BINARY
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([workspace_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        }),
        capability,
        correlation_id,
    )?;

    let mut inventory = Vec::new();
    let mut previous_id = None;
    let mut digests = HashSet::new();
    let mut blob_bytes = 0_u64;
    for row in rows {
        let (evidence_id, digest, size_bytes, stored_path) =
            map_sql(row, capability, correlation_id)?;
        let evidence_id = evidence_id
            .parse::<EvidenceId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        if previous_id.is_some_and(|previous: EvidenceId| previous.uuid() >= evidence_id.uuid()) {
            return integrity_failure(capability, correlation_id);
        }
        previous_id = Some(evidence_id);

        let digest = Sha256Digest::parse(&digest)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        if !digests.insert(digest.clone()) {
            return integrity_failure(capability, correlation_id);
        }
        let byte_length = u64::try_from(size_bytes)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        if byte_length > limits.max_entry_bytes.get() || byte_length > MAX_PORTABLE_JSON_INTEGER {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
        blob_bytes = blob_bytes
            .checked_add(byte_length)
            .filter(|bytes| *bytes <= limits.max_uncompressed_bytes.get())
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        let digest_hex = canonical_digest_hex(digest.as_str())
            .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let relative_path = relative_evidence_path(digest_hex);
        let path_depth = u64::try_from(relative_path.components().count())
            .map_err(|_| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        let path_bytes = u64::try_from(stored_path.len())
            .map_err(|_| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        if path_bytes > limits.max_path_bytes.get() || path_depth > limits.max_path_depth.get() {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
        if path_to_storage_value(&relative_path) != stored_path {
            return integrity_failure(capability, correlation_id);
        }

        inventory
            .len()
            .checked_add(1)
            .and_then(|count| u64::try_from(count).ok())
            .filter(|count| *count <= limits.max_entries.get())
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;

        inventory.push(SnapshotEvidenceBlob {
            descriptor: WorkspaceBlobDescriptor::new(evidence_id, byte_length, digest),
            relative_path,
        });
    }
    Ok(inventory)
}

fn section_for_entity(
    entity: WorkspaceExportEntity,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<&'static ExportSection> {
    EXPORT_SECTIONS
        .get(entity.index())
        .filter(|section| section.entity == entity)
        .ok_or_else(|| {
            Box::new(FastiProblem::integrity_failed(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            ))
        })
}

fn stream_entity_pages<F>(
    section: &ExportSection,
    max_rows: u64,
    max_bytes: u64,
    sink: &mut dyn Write,
    correlation_id: RequestCorrelationId,
    mut read_page: F,
) -> ApplicationResult<WorkspaceStreamDescriptor>
where
    F: FnMut(&[Value], u64) -> ApplicationResult<SectionPage>,
{
    let capability = CapabilityKey::ExportWorkspace;
    let mut cursor: Vec<Value> = section
        .cursor_columns
        .iter()
        .map(|column| column.value())
        .collect();
    let mut written = 0_u64;
    let mut sink = EntityDigestSink::new(sink, max_bytes);

    loop {
        let page = read_page(&cursor, sink.remaining_bytes())?;
        if page.rows.is_empty() {
            break;
        }
        let page_len = page.rows.len();
        let next_written = written
            .checked_add(u64::try_from(page_len).map_err(|_| {
                Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
            })?)
            .filter(|count| *count <= max_rows)
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;

        for (line, next_cursor) in page.rows {
            sink.write_line(&line, capability, correlation_id)?;
            cursor = next_cursor;
        }
        written = next_written;
        if page.exhausted {
            break;
        }
    }

    Ok(sink.finish(section.entity, written))
}

struct EntityDigestSink<'a> {
    inner: &'a mut dyn Write,
    hasher: Sha256,
    bytes: u64,
    max_bytes: u64,
}

impl<'a> EntityDigestSink<'a> {
    fn new(inner: &'a mut dyn Write, max_bytes: u64) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
            max_bytes,
        }
    }

    fn remaining_bytes(&self) -> u64 {
        self.max_bytes - self.bytes
    }

    fn write_line(
        &mut self,
        line: &[u8],
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<()> {
        let next_bytes = self
            .bytes
            .checked_add(u64::try_from(line.len()).map_err(|_| {
                Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
            })?)
            .filter(|bytes| *bytes <= self.max_bytes)
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        self.inner.write_all(line).map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;
        self.hasher.update(line);
        self.bytes = next_bytes;
        Ok(())
    }

    fn finish(self, entity: WorkspaceExportEntity, row_count: u64) -> WorkspaceStreamDescriptor {
        let digest: [u8; 32] = self.hasher.finalize().into();
        let digest = Sha256Digest::parse(format!("sha256:{}", encode_hex(&digest)))
            .expect("SHA-256 output is canonical lowercase hexadecimal");
        WorkspaceStreamDescriptor::new(entity, row_count, self.bytes, digest)
    }
}

/// Hashes and counts every byte handed to the caller's sink.
///
/// The archive digest covers the whole stream, so it cannot be embedded in the
/// archive; it is returned in the outcome instead.
struct DigestSink<'a> {
    inner: &'a mut dyn Write,
    hasher: Sha256,
    bytes: u64,
}

impl<'a> DigestSink<'a> {
    fn new(inner: &'a mut dyn Write) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
        }
    }

    fn finish(
        self,
        capability: CapabilityKey,
        correlation_id: fasti_domain::RequestCorrelationId,
    ) -> ApplicationResult<(String, u64)> {
        let inner = self.inner;
        inner.flush().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;
        let digest: [u8; 32] = self.hasher.finalize().into();
        Ok((format!("sha256:{}", encode_hex(&digest)), self.bytes))
    }
}

impl Write for DigestSink<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.hasher.update(&buf[..written]);
        self.bytes = self
            .bytes
            .checked_add(written as u64)
            .ok_or_else(|| std::io::Error::other("export byte count overflow"))?;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl WorkspaceExportPort for SqliteKernel {
    fn export_workspace(
        &self,
        query: ExportWorkspaceQuery,
        sink: &mut dyn Write,
    ) -> ApplicationResult<WorkspaceExportOutcome> {
        let capability = CapabilityKey::ExportWorkspace;
        let correlation_id = query.correlation_id();
        let access = query.access();
        let workspace_id = access.workspace_id();

        // Fence the export with a durable revision and row-count snapshot. Pages release
        // the connection lock so acceptance is not blocked for the whole
        // export; the closing comparison turns any concurrent mutation into a
        // failure instead of an archive with dangling references.
        //
        // ponytail: revision fence, not the accepted online-backup snapshot. A single
        // long-lived read transaction would be a true snapshot but holds the
        // one global connection mutex for the entire export. Revisit if a
        // reader pool or bounded online backup exists.
        let opening = export_snapshot(self, access, capability, correlation_id)?;

        let mut sink = DigestSink::new(sink);
        write_header(&mut sink, workspace_id, capability, correlation_id)?;

        let mut counts = [0_u64; WorkspaceExportEntity::ALL.len()];
        for (section_index, section) in EXPORT_SECTIONS.iter().enumerate() {
            let written =
                write_section(self, access, section, &mut sink, capability, correlation_id)?;
            let expected = u64::try_from(opening.counts[section_index]).map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?;
            if written != expected {
                let current = export_snapshot(self, access, capability, correlation_id)?;
                if current.revision != opening.revision {
                    return Err(Box::new(FastiProblem::from_code(
                        ProblemCode::StorageUnavailable,
                        capability,
                        correlation_id,
                    )));
                }
                return integrity_failure(capability, correlation_id);
            }
            counts[section.entity.index()] = written;
        }

        let closing = export_snapshot(self, access, capability, correlation_id)?;
        if closing != opening {
            return Err(Box::new(FastiProblem::from_code(
                ProblemCode::StorageUnavailable,
                capability,
                correlation_id,
            )));
        }

        write_trailer(&mut sink, &counts, capability, correlation_id)?;
        let (archive_digest, bytes_written) = sink.finish(capability, correlation_id)?;

        Ok(WorkspaceExportOutcome::new(
            workspace_id,
            WORKSPACE_EXPORT_FORMAT_VERSION,
            counts,
            bytes_written,
            archive_digest,
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ExportFence {
    revision: i64,
    counts: Vec<i64>,
}

/// Durable revision and row counts for every exported section.
fn export_snapshot(
    kernel: &SqliteKernel,
    access: &RequestAccessContext,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<ExportFence> {
    let mut connection = kernel.lock_connection(capability, correlation_id)?;
    let transaction = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Deferred),
        capability,
        correlation_id,
    )?;
    authorize_transaction(&transaction, capability, access, correlation_id)?;
    verify_namespace_bindings(
        &transaction,
        access.workspace_id(),
        capability,
        correlation_id,
    )?;

    let workspace = access.workspace_id().to_string();
    let revision = map_sql(
        workspace_revision(&transaction, &workspace),
        capability,
        correlation_id,
    )?;
    let mut counts = Vec::with_capacity(EXPORT_SECTIONS.len());
    for section in EXPORT_SECTIONS {
        let count = map_sql(
            transaction.query_row(section.count_sql, [workspace.as_str()], |row| {
                row.get::<_, i64>(0)
            }),
            capability,
            correlation_id,
        )?;
        counts.push(count);
    }
    map_sql(transaction.commit(), capability, correlation_id)?;
    Ok(ExportFence { revision, counts })
}

fn write_header(
    sink: &mut DigestSink<'_>,
    workspace_id: fasti_domain::WorkspaceId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    // No wall-clock time and no host identity: both would break byte equality
    // between two exports of the same durable state.
    let header = serde_json::json!({
        "section": "header",
        "format": "fasti.workspace.export",
        "format_version": WORKSPACE_EXPORT_FORMAT_VERSION,
        "workspace_id": workspace_id.to_string(),
        "evidence_content": "excluded_v1",
        "sections": WorkspaceExportEntity::ALL
            .into_iter()
            .map(WorkspaceExportEntity::as_str)
            .collect::<Vec<_>>(),
    });
    write_line(sink, &header, capability, correlation_id)
}

fn write_trailer(
    sink: &mut DigestSink<'_>,
    counts: &[u64; WorkspaceExportEntity::ALL.len()],
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    let mut entries = serde_json::Map::new();
    for entity in WorkspaceExportEntity::ALL {
        entries.insert(
            entity.as_str().to_owned(),
            serde_json::Value::from(counts[entity.index()]),
        );
    }
    let trailer = serde_json::json!({
        "section": "trailer",
        "counts": serde_json::Value::Object(entries),
    });
    write_line(sink, &trailer, capability, correlation_id)
}

fn write_line(
    sink: &mut dyn Write,
    value: &serde_json::Value,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let line = serialized_line(value, capability, correlation_id)?;
    sink.write_all(&line).map_err(|_| {
        Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        ))
    })
}

fn serialized_line(
    value: &serde_json::Value,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<u8>> {
    let mut line = serde_json::to_vec(value)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    line.push(b'\n');
    Ok(line)
}

/// Streams one section in bounded pages, re-authorizing on every page.
fn write_section(
    kernel: &SqliteKernel,
    access: &RequestAccessContext,
    section: &ExportSection,
    sink: &mut DigestSink<'_>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<u64> {
    let marker = serde_json::json!({
        "section": section.entity.as_str(),
    });
    write_line(sink, &marker, capability, correlation_id)?;

    let workspace = access.workspace_id().to_string();
    let descriptor = stream_entity_pages(
        section,
        u64::MAX,
        u64::MAX,
        sink,
        correlation_id,
        |cursor, max_page_bytes| {
            read_authorized_section_page(
                kernel,
                access,
                section,
                &workspace,
                cursor,
                max_page_bytes,
                correlation_id,
            )
        },
    )?;
    Ok(descriptor.row_count())
}

type SectionRow = (Vec<u8>, Vec<Value>);

struct SectionPage {
    rows: Vec<SectionRow>,
    exhausted: bool,
}

fn read_authorized_section_page(
    kernel: &SqliteKernel,
    access: &RequestAccessContext,
    section: &ExportSection,
    workspace: &str,
    cursor: &[Value],
    max_page_bytes: u64,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<SectionPage> {
    let capability = CapabilityKey::ExportWorkspace;
    let mut connection = kernel.lock_connection(capability, correlation_id)?;
    let transaction = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Deferred),
        capability,
        correlation_id,
    )?;
    // Revocation part-way through a long export must stop further disclosure,
    // so every page re-authorizes against current durable state.
    authorize_transaction(&transaction, capability, access, correlation_id)?;

    let page = read_section_page(
        &transaction,
        section,
        workspace,
        cursor,
        max_page_bytes,
        correlation_id,
    )?;
    map_sql(transaction.commit(), capability, correlation_id)?;
    Ok(page)
}

fn read_section_page(
    connection: &Connection,
    section: &ExportSection,
    workspace: &str,
    cursor: &[Value],
    max_page_bytes: u64,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<SectionPage> {
    let capability = CapabilityKey::ExportWorkspace;
    let mut statement = map_sql(connection.prepare(section.sql), capability, correlation_id)?;

    let column_names: Vec<String> = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let mut output_columns: Vec<usize> = (0..column_names.len()).collect();
    output_columns.sort_unstable_by(|left, right| column_names[*left].cmp(&column_names[*right]));

    let mut bindings: Vec<Value> = Vec::with_capacity(cursor.len() + 2);
    bindings.push(Value::Text(workspace.to_owned()));
    bindings.extend(cursor.iter().cloned());
    bindings.push(Value::Integer(EXPORT_PAGE));

    let mut rows = map_sql(
        statement.query(rusqlite::params_from_iter(bindings.iter())),
        capability,
        correlation_id,
    )?;

    let page_byte_limit = max_page_bytes.min(ARCHIVE_MAX_IO_CHUNK_BYTES as u64);
    let mut page = Vec::new();
    let mut page_bytes = 0_u64;
    let mut query_rows = 0_usize;
    let mut stopped_at_page_limit = false;
    while let Some(row) = map_sql(rows.next(), capability, correlation_id)? {
        query_rows = query_rows
            .checked_add(1)
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        let encoded = encode_row(
            row,
            &column_names,
            &output_columns,
            section.cursor_columns,
            page_byte_limit,
            capability,
            correlation_id,
        )?;
        let encoded_bytes = u64::try_from(encoded.0.len())
            .map_err(|_| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        let Some(next_page_bytes) = page_bytes.checked_add(encoded_bytes) else {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        };
        if next_page_bytes > page_byte_limit {
            if page.is_empty() {
                return Err(Box::new(FastiProblem::capacity_exceeded(
                    capability,
                    correlation_id,
                )));
            }
            stopped_at_page_limit = true;
            break;
        }
        page_bytes = next_page_bytes;
        page.push(encoded);
    }
    drop(rows);
    drop(statement);
    Ok(SectionPage {
        exhausted: !stopped_at_page_limit && query_rows < EXPORT_PAGE as usize,
        rows: page,
    })
}

struct BoundedRowEncoder {
    bytes: Vec<u8>,
    max_bytes: usize,
    limit_hit: bool,
}

impl BoundedRowEncoder {
    fn new(max_bytes: u64) -> Self {
        let max_bytes = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        Self {
            bytes: Vec::with_capacity(max_bytes.min(4 * 1024)),
            max_bytes,
            limit_hit: false,
        }
    }

    fn write_bytes(
        &mut self,
        bytes: &[u8],
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<()> {
        let result = self.write_all(bytes);
        self.map_result(result, capability, correlation_id)
    }

    fn write_json_string(
        &mut self,
        value: &str,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<()> {
        let result = serde_json::to_writer(&mut *self, value);
        self.map_result(result, capability, correlation_id)
    }

    fn write_integer(
        &mut self,
        value: u64,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<()> {
        let result = write!(self, "{value}");
        self.map_result(result, capability, correlation_id)
    }

    fn map_result<T, E>(
        &self,
        result: Result<T, E>,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<T> {
        result.map_err(|_| {
            if self.limit_hit {
                Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
            } else {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            }
        })
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for BoundedRowEncoder {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let admitted = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_some_and(|length| length <= self.max_bytes);
        if !admitted {
            self.limit_hit = true;
            return Err(std::io::Error::other("export row byte limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Encodes one row as frozen NDJSON bytes plus the cursor for the next page.
///
/// Archive-v1 keeps the existing lexicographic key order explicitly instead
/// of inheriting `serde_json::Map` feature selection. The bounded writer
/// rejects an oversized or escape-expanded row before it is materialized.
fn encode_row(
    row: &rusqlite::Row<'_>,
    column_names: &[String],
    output_columns: &[usize],
    cursor_columns: &[CursorColumn],
    max_bytes: u64,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<SectionRow> {
    let mut encoded = BoundedRowEncoder::new(max_bytes);
    encoded.write_bytes(b"{", capability, correlation_id)?;
    for (position, index) in output_columns.iter().copied().enumerate() {
        if position != 0 {
            encoded.write_bytes(b",", capability, correlation_id)?;
        }
        encoded.write_json_string(&column_names[index], capability, correlation_id)?;
        encoded.write_bytes(b":", capability, correlation_id)?;
        match map_sql(row.get_ref(index), capability, correlation_id)? {
            ValueRef::Null => encoded.write_bytes(b"null", capability, correlation_id)?,
            ValueRef::Integer(value) => encoded.write_integer(
                portable_integer(value, capability, correlation_id)?,
                capability,
                correlation_id,
            )?,
            ValueRef::Text(bytes) => {
                let text = std::str::from_utf8(bytes).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?;
                encoded.write_json_string(text, capability, correlation_id)?;
            }
            // The schema declares no REAL or BLOB column. Fail closed rather
            // than emit a value whose text form is not stable across hosts.
            ValueRef::Real(_) | ValueRef::Blob(_) => {
                return Err(Box::new(FastiProblem::integrity_failed(
                    capability,
                    correlation_id,
                )))
            }
        }
    }
    encoded.write_bytes(b"}\n", capability, correlation_id)?;

    let mut cursor = Vec::with_capacity(cursor_columns.len());
    for column in cursor_columns {
        cursor.push(
            match map_sql(row.get_ref(column.index()), capability, correlation_id)? {
                ValueRef::Text(bytes) => Value::Text(
                    std::str::from_utf8(bytes)
                        .map_err(|_| {
                            Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                        })?
                        .to_owned(),
                ),
                ValueRef::Integer(value) => {
                    portable_integer(value, capability, correlation_id)?;
                    Value::Integer(value)
                }
                // A NULL key column would make the keyset cursor unable to
                // advance, silently truncating the section.
                ValueRef::Null | ValueRef::Real(_) | ValueRef::Blob(_) => {
                    return integrity_failure(capability, correlation_id);
                }
            },
        );
    }

    Ok((encoded.finish(), cursor))
}

fn portable_integer(
    value: i64,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<u64> {
    u64::try_from(value)
        .ok()
        .filter(|value| *value <= MAX_PORTABLE_JSON_INTEGER)
        .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}
