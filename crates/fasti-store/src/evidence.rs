use crate::crypto::{encode_hex, sha256_reader};
use crate::kernel::{
    authorize_connection, authorize_transaction, fsync_directory, map_sql, now, problem, timestamp,
    SqliteKernel, MAX_CONCURRENT_UPLOADS, MAX_EVIDENCE_BYTES, MAX_TEMP_EVIDENCE_BYTES,
};
use fasti_application::{
    ApplicationResult, CapabilityKey, EvidenceUploadPort, EvidenceUploadRequest,
    EvidenceUploadSession, FastiProblem, ProblemCode,
};
use fasti_domain::{EvidenceId, EvidenceReference, Sha256Digest};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

impl EvidenceUploadPort for SqliteKernel {
    fn begin_evidence_upload(
        &self,
        request: EvidenceUploadRequest,
    ) -> ApplicationResult<Box<dyn EvidenceUploadSession>> {
        let correlation_id = request.correlation_id();
        let capability = CapabilityKey::UploadEvidence;
        let limit = request
            .declared_size()
            .unwrap_or(MAX_EVIDENCE_BYTES)
            .min(MAX_EVIDENCE_BYTES);
        if request.declared_size().is_some_and(|size| size > MAX_EVIDENCE_BYTES) {
            return Err(problem(
                ProblemCode::PayloadTooLarge,
                capability,
                correlation_id,
            ));
        }

        // Authorization is complete before any temp path or file is created.
        {
            let connection = self.lock_connection(capability, correlation_id)?;
            authorize_connection(
                &connection,
                capability,
                request.access(),
                correlation_id,
            )?;
        }

        let reserved = request.declared_size().unwrap_or(MAX_EVIDENCE_BYTES);
        {
            let mut budget = self.lock_upload_budget(capability, correlation_id)?;
            let next_bytes = budget
                .reserved_bytes
                .checked_add(reserved)
                .ok_or_else(|| {
                    Box::new(FastiProblem::capacity_exceeded(
                        capability,
                        correlation_id,
                    ))
                })?;
            if budget.active >= MAX_CONCURRENT_UPLOADS || next_bytes > MAX_TEMP_EVIDENCE_BYTES {
                return Err(Box::new(FastiProblem::capacity_exceeded(
                    capability,
                    correlation_id,
                )));
            }
            budget.active += 1;
            budget.reserved_bytes = next_bytes;
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
}

impl EvidenceUploadSession for SqliteEvidenceUpload {
    fn write_chunk(&mut self, bytes: &[u8]) -> ApplicationResult<()> {
        let correlation_id = self.request.correlation_id();
        let capability = CapabilityKey::UploadEvidence;
        let next = self
            .bytes_written
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| problem(ProblemCode::PayloadTooLarge, capability, correlation_id))?;
        if next > self.limit || next > MAX_EVIDENCE_BYTES {
            return Err(problem(
                ProblemCode::PayloadTooLarge,
                capability,
                correlation_id,
            ));
        }
        self.file
            .as_mut()
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?
            .write_all(bytes)
            .map_err(|_| {
                Box::new(FastiProblem::storage_unavailable(
                    capability,
                    correlation_id,
                ))
            })?;
        self.hasher.update(bytes);
        self.bytes_written = next;
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> ApplicationResult<EvidenceReference> {
        let correlation_id = self.request.correlation_id();
        let capability = CapabilityKey::UploadEvidence;
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
        let digest = Sha256Digest::parse(format!("sha256:{digest_hex}")).map_err(|_| {
            Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            ))
        })?;

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

        let shard = &digest_hex[..2];
        let parent = self.kernel.inner.payload_root.join(shard);
        fs::create_dir_all(&parent).map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;
        let destination = parent.join(&digest_hex);
        match fs::hard_link(&self.temp_path, &destination) {
            Ok(()) => {
                fs::remove_file(&self.temp_path).map_err(|_| {
                    Box::new(FastiProblem::storage_unavailable(
                        capability,
                        correlation_id,
                    ))
                })?;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let existing = File::open(&destination).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(
                        capability,
                        correlation_id,
                    ))
                })?;
                let (existing_digest, existing_size) = sha256_reader(existing).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(
                        capability,
                        correlation_id,
                    ))
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

        let relative_path = destination
            .strip_prefix(&self.kernel.inner.current_root)
            .map_err(|_| {
                Box::new(FastiProblem::integrity_failed(
                    capability,
                    correlation_id,
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
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
                    Box::new(FastiProblem::integrity_failed(
                        capability,
                        correlation_id,
                    ))
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

fn release_reservation(kernel: &SqliteKernel, reserved: u64) {
    if let Ok(mut budget) = kernel.inner.upload_budget.lock() {
        budget.active = budget.active.saturating_sub(1);
        budget.reserved_bytes = budget.reserved_bytes.saturating_sub(reserved);
    }
}
