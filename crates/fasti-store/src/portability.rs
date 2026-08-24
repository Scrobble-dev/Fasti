use crate::crypto::{encode_hex, sha256_reader};
use crate::evidence::{canonical_digest_hex, path_to_storage_value, relative_evidence_path};
use crate::kernel::{authorize_transaction, map_sql, reject_unsafe_existing_file, SqliteKernel};
use fasti_application::{
    ApplicationResult, CapabilityKey, FastiProblem, ProblemCode, RequestAccessContext,
    VerifyWorkspaceQuery, WorkspaceVerificationOutcome, WorkspaceVerificationPort,
};
use rusqlite::{params, Transaction, TransactionBehavior};
use std::fs::File;
use std::io::Read;

const EVIDENCE_VERIFY_PAGE: i64 = 128;
type EvidenceRow = (i64, String, i64, String);
type Snapshot = (u64, u64, i64);

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

fn verify_sqlite_integrity(
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

fn verify_domain_relations(
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
    let expected_size = u64::try_from(size_bytes)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let path = kernel.inner.current_root.join(&expected_relative);
    reject_unsafe_existing_file(&path)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let file = File::open(&path)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let bounded_reader = file.take(expected_size.saturating_add(1));
    let (actual_digest, actual_size) = sha256_reader(bounded_reader)
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
    use fasti_application::{EvidenceUploadPort, EvidenceUploadRequest, ScopeKey};
    use fasti_domain::RequestCorrelationId;
    use std::fs;

    fn grant_verify_scope(node: &TestNode) {
        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        connection
            .execute(
                "INSERT OR IGNORE INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                params![
                    node.access.grant_id().to_string(),
                    ScopeKey::WorkspaceVerify.as_str()
                ],
            )
            .expect("grant staged verification scope");
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
}
