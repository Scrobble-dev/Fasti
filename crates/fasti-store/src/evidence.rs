use crate::crypto::{encode_hex, sha256_reader};
use crate::kernel::{
    authorize_connection, authorize_transaction, fsync_directory, harden_private_regular_file,
    map_sql, now, prepare_private_directory, problem, reject_unsafe_existing_file, timestamp,
    SqliteKernel, MAX_CONCURRENT_UPLOADS, MAX_EVIDENCE_BYTES, MAX_PREPARED_EVIDENCE_BYTES,
    MAX_TEMP_EVIDENCE_BYTES,
};
use fasti_application::{
    ApplicationResult, CapabilityKey, EvidenceUploadPort, EvidenceUploadRequest,
    EvidenceUploadSession, FastiProblem, ProblemCode,
};
use fasti_domain::{EvidenceId, EvidenceReference, Sha256Digest, WorkspaceId};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

impl EvidenceUploadPort for SqliteKernel {
    fn begin_evidence_upload(
        &self,
        request: EvidenceUploadRequest,
    ) -> ApplicationResult<Box<dyn EvidenceUploadSession>> {
        let correlation_id = request.correlation_id();
        let capability = CapabilityKey::AcceptObservation;
        let limit = request
            .declared_size()
            .unwrap_or(MAX_EVIDENCE_BYTES)
            .min(MAX_EVIDENCE_BYTES);
        if request
            .declared_size()
            .is_some_and(|size| size > MAX_EVIDENCE_BYTES)
        {
            return Err(problem(
                ProblemCode::PayloadTooLarge,
                capability,
                correlation_id,
            ));
        }

        let reserved = request.declared_size().unwrap_or(MAX_EVIDENCE_BYTES);
        {
            // Keep authorization, the durable prepared-byte count, and the
            // in-process reservation under one lock order. A completed upload
            // cannot disappear from the budget before its evidence row exists.
            let connection = self.lock_connection(capability, correlation_id)?;
            authorize_connection(&connection, capability, request.access(), correlation_id)?;
            let prepared_bytes = prepared_evidence_bytes(
                &connection,
                request.access().workspace_id(),
                capability,
                correlation_id,
            )?;
            let mut budget = self.lock_upload_budget(capability, correlation_id)?;
            let next_reserved_bytes =
                budget.reserved_bytes.checked_add(reserved).ok_or_else(|| {
                    Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
                })?;
            let next_prepared_bytes =
                prepared_bytes
                    .checked_add(next_reserved_bytes)
                    .ok_or_else(|| {
                        Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
                    })?;
            if budget.active >= MAX_CONCURRENT_UPLOADS
                || next_reserved_bytes > MAX_TEMP_EVIDENCE_BYTES
                || next_prepared_bytes > MAX_PREPARED_EVIDENCE_BYTES
            {
                return Err(Box::new(FastiProblem::capacity_exceeded(
                    capability,
                    correlation_id,
                )));
            }
            budget.active += 1;
            budget.reserved_bytes = next_reserved_bytes;
        }

        let evidence_id = EvidenceId::new_v7();
        let temp_path = self
            .inner
            .scratch_root
            .join(format!("{}.upload", evidence_id));
        let file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(_) => {
                release_reservation(self, reserved);
                return Err(Box::new(FastiProblem::storage_unavailable(
                    capability,
                    correlation_id,
                )));
            }
        };
        if harden_private_regular_file(&temp_path).is_err() {
            drop(file);
            let _ = fs::remove_file(&temp_path);
            release_reservation(self, reserved);
            return Err(Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            )));
        }

        Ok(Box::new(SqliteEvidenceUpload {
            kernel: self.clone(),
            request,
            evidence_id,
            temp_path,
            file: Some(file),
            hasher: Sha256::new(),
            bytes_written: 0,
            limit,
            reserved,
            completed: false,
            failed: false,
        }))
    }
}

struct SqliteEvidenceUpload {
    kernel: SqliteKernel,
    request: EvidenceUploadRequest,
    evidence_id: EvidenceId,
    temp_path: PathBuf,
    file: Option<File>,
    hasher: Sha256,
    bytes_written: u64,
    limit: u64,
    reserved: u64,
    completed: bool,
    failed: bool,
}

impl EvidenceUploadSession for SqliteEvidenceUpload {
    fn write_chunk(&mut self, bytes: &[u8]) -> ApplicationResult<()> {
        let correlation_id = self.request.correlation_id();
        let capability = CapabilityKey::AcceptObservation;
        if self.failed {
            return Err(problem(
                ProblemCode::IntegrityFailed,
                capability,
                correlation_id,
            ));
        }
        let next = match self.bytes_written.checked_add(bytes.len() as u64) {
            Some(next) => next,
            None => {
                self.failed = true;
                return Err(problem(
                    ProblemCode::PayloadTooLarge,
                    capability,
                    correlation_id,
                ));
            }
        };
        if next > self.limit || next > MAX_EVIDENCE_BYTES {
            self.failed = true;
            return Err(problem(
                ProblemCode::PayloadTooLarge,
                capability,
                correlation_id,
            ));
        }
        let Some(file) = self.file.as_mut() else {
            self.failed = true;
            return Err(problem(
                ProblemCode::IntegrityFailed,
                capability,
                correlation_id,
            ));
        };
        if file.write_all(bytes).is_err() {
            self.failed = true;
            return Err(Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            )));
        }
        self.hasher.update(bytes);
        self.bytes_written = next;
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> ApplicationResult<EvidenceReference> {
        let correlation_id = self.request.correlation_id();
        let capability = CapabilityKey::AcceptObservation;
        if self.failed {
            return Err(problem(
                ProblemCode::IntegrityFailed,
                capability,
                correlation_id,
            ));
        }
        let mut file = self
            .file
            .take()
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        file.flush().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;
        file.sync_all().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;
        drop(file);

        if self
            .request
            .declared_size()
            .is_some_and(|declared| declared != self.bytes_written)
        {
            return Err(problem(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            ));
        }

        let digest_bytes: [u8; 32] = self.hasher.clone().finalize().into();
        let digest_hex = encode_hex(&digest_bytes);
        let digest = Sha256Digest::parse(format!("sha256:{digest_hex}"))
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;

        // Recheck the current credential and grant before durable promotion.
        {
            let connection = self.kernel.lock_connection(capability, correlation_id)?;
            authorize_connection(
                &connection,
                capability,
                self.request.access(),
                correlation_id,
            )?;
        }

        let relative_path = relative_evidence_path(&digest_hex);
        let parent =
            self.kernel
                .inner
                .current_root
                .join(relative_path.parent().ok_or_else(|| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?);
        let parent_created = match fs::symlink_metadata(&parent) {
            Ok(_) => false,
            Err(error) if error.kind() == ErrorKind::NotFound => true,
            Err(_) => {
                return Err(Box::new(FastiProblem::storage_unavailable(
                    capability,
                    correlation_id,
                )))
            }
        };
        prepare_private_directory(&parent).map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;
        if parent_created {
            fsync_directory(&self.kernel.inner.payload_root).map_err(|_| {
                Box::new(FastiProblem::storage_unavailable(
                    capability,
                    correlation_id,
                ))
            })?;
        }
        let destination = self.kernel.inner.current_root.join(&relative_path);
        match fs::hard_link(&self.temp_path, &destination) {
            Ok(()) => {
                if harden_private_regular_file(&destination).is_err() {
                    let _ = fs::remove_file(&destination);
                    return Err(Box::new(FastiProblem::storage_unavailable(
                        capability,
                        correlation_id,
                    )));
                }
                fs::remove_file(&self.temp_path).map_err(|_| {
                    Box::new(FastiProblem::storage_unavailable(
                        capability,
                        correlation_id,
                    ))
                })?;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                reject_unsafe_existing_file(&destination).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?;
                let existing = File::open(&destination).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?;
                let (existing_digest, existing_size) = sha256_reader(existing).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?;
                if existing_digest != digest_bytes || existing_size != self.bytes_written {
                    return Err(Box::new(FastiProblem::integrity_failed(
                        capability,
                        correlation_id,
                    )));
                }
                fs::remove_file(&self.temp_path).map_err(|_| {
                    Box::new(FastiProblem::storage_unavailable(
                        capability,
                        correlation_id,
                    ))
                })?;
            }
            Err(_) => {
                return Err(Box::new(FastiProblem::storage_unavailable(
                    capability,
                    correlation_id,
                )))
            }
        }
        fsync_directory(&parent).map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;

        let relative_path = path_to_storage_value(&relative_path);
        let created_at = timestamp(now());
        let mut connection = self.kernel.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(
            &transaction,
            capability,
            self.request.access(),
            correlation_id,
        )?;
        let existing = map_sql(
            transaction
                .query_row(
                    "SELECT evidence_id, size_bytes FROM evidence WHERE workspace_id = ?1 AND digest = ?2",
                    params![
                        self.request.access().workspace_id().to_string(),
                        digest.to_string()
                    ],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let result = if let Some((existing_id, existing_size)) = existing {
            if u64::try_from(existing_size).ok() != Some(self.bytes_written) {
                return Err(Box::new(FastiProblem::integrity_failed(
                    capability,
                    correlation_id,
                )));
            }
            EvidenceReference::new(
                existing_id.parse::<EvidenceId>().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                digest,
                self.bytes_written,
            )
        } else {
            map_sql(
                transaction.execute(
                    r#"
                    INSERT INTO evidence(
                        evidence_id, workspace_id, digest, size_bytes, relative_path, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                    params![
                        self.evidence_id.to_string(),
                        self.request.access().workspace_id().to_string(),
                        digest.to_string(),
                        i64::try_from(self.bytes_written).unwrap_or(i64::MAX),
                        relative_path,
                        created_at
                    ],
                ),
                capability,
                correlation_id,
            )?;
            EvidenceReference::new(self.evidence_id, digest, self.bytes_written)
        };
        map_sql(transaction.commit(), capability, correlation_id)?;
        drop(connection);

        self.completed = true;
        release_reservation(&self.kernel, self.reserved);
        Ok(result)
    }
}

impl Drop for SqliteEvidenceUpload {
    fn drop(&mut self) {
        if !self.completed {
            self.file.take();
            let _ = fs::remove_file(&self.temp_path);
            release_reservation(&self.kernel, self.reserved);
            self.completed = true;
        }
    }
}

fn prepared_evidence_bytes(
    connection: &Connection,
    workspace_id: WorkspaceId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<u64> {
    let bytes = map_sql(
        connection.query_row(
            r#"
            SELECT COALESCE(SUM(e.size_bytes), 0)
            FROM evidence e
            WHERE e.workspace_id = ?1
              AND NOT EXISTS(
                  SELECT 1 FROM observations o
                  WHERE o.evidence_id = e.evidence_id
              )
            "#,
            [workspace_id.to_string()],
            |row| row.get::<_, i64>(0),
        ),
        capability,
        correlation_id,
    )?;
    u64::try_from(bytes)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

pub(crate) fn canonical_digest_hex(value: &str) -> Option<&str> {
    let value = value.strip_prefix("sha256:")?;
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Some(value)
    } else {
        None
    }
}

pub(crate) fn relative_evidence_path(digest_hex: &str) -> PathBuf {
    PathBuf::from("payloads")
        .join("sha256")
        .join(&digest_hex[..2])
        .join(digest_hex)
}

pub(crate) fn path_to_storage_value(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn release_reservation(kernel: &SqliteKernel, reserved: u64) {
    if let Ok(mut budget) = kernel.inner.upload_budget.lock() {
        budget.active = budget.active.saturating_sub(1);
        budget.reserved_bytes = budget.reserved_bytes.saturating_sub(reserved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{AcceptObservationCommand, ObservationAcceptancePort};
    use fasti_domain::{ClaimedTrust, ObservedAt, OperationId, RequestCorrelationId};

    #[test]
    fn failed_chunk_poisoning_prevents_partial_evidence_commit() {
        let node = TestNode::new();
        let mut upload = node
            .kernel
            .begin_evidence_upload(EvidenceUploadRequest::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Some(1),
            ))
            .expect("begin bounded upload");

        let write_error = upload.write_chunk(b"ab").expect_err("oversized chunk");
        assert_eq!(write_error.code(), ProblemCode::PayloadTooLarge);
        let finish_error = upload.finish().expect_err("failed upload stays failed");
        assert_eq!(finish_error.code(), ProblemCode::IntegrityFailed);
    }

    #[test]
    fn prepared_evidence_capacity_is_bounded_before_temp_file_creation() {
        let node = TestNode::new();
        let evidence_id = EvidenceId::new_v7();
        let digest = format!("sha256:{}", "ab".repeat(32));
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    r#"
                    INSERT INTO evidence(
                        evidence_id, workspace_id, digest, size_bytes, relative_path, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                    params![
                        evidence_id.to_string(),
                        node.access.workspace_id().to_string(),
                        digest,
                        i64::try_from(MAX_PREPARED_EVIDENCE_BYTES).expect("bounded size"),
                        "payloads/sha256/ab/abandoned",
                        timestamp(now())
                    ],
                )
                .expect("insert prepared evidence accounting row");
        }

        let error = match node
            .kernel
            .begin_evidence_upload(EvidenceUploadRequest::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Some(1),
            )) {
            Ok(_) => panic!("prepared evidence quota must reject another upload"),
            Err(error) => error,
        };
        assert_eq!(error.code(), ProblemCode::CapacityExceeded);
        assert!(fs::read_dir(&node.kernel.inner.scratch_root)
            .expect("read scratch directory")
            .next()
            .is_none());
    }

    #[test]
    fn stored_evidence_path_must_match_the_content_digest() {
        let node = TestNode::new();
        let evidence = node.upload(b"governed evidence");
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "UPDATE evidence SET relative_path = '../outside' WHERE evidence_id = ?1",
                    [evidence.evidence_id().to_string()],
                )
                .expect("corrupt stored relative path");
        }
        let command = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            OperationId::new_v7(),
            None,
            ObservedAt::parse("2026-08-23T10:30:00Z", ClaimedTrust::DeviceObserved)
                .expect("observed time"),
            evidence,
        );

        let error = node
            .kernel
            .authorize_and_accept(command)
            .expect_err("untrusted stored path must fail");
        assert_eq!(error.code(), ProblemCode::IntegrityFailed);
    }

    #[cfg(unix)]
    #[test]
    fn upload_scratch_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let node = TestNode::new();
        let upload = node
            .kernel
            .begin_evidence_upload(EvidenceUploadRequest::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Some(1),
            ))
            .expect("begin upload");
        let path = fs::read_dir(&node.kernel.inner.scratch_root)
            .expect("read scratch directory")
            .next()
            .expect("one temp file")
            .expect("temp entry")
            .path();
        let mode = fs::metadata(path)
            .expect("temp file metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        drop(upload);
    }
}
