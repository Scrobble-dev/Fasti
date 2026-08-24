//! Private, non-activating pass-two import for verified B3 archives.
//!
//! Pass one and pass two consume the same already-open `Read + Seek` source.
//! This module creates only a fresh descriptor-relative staging attempt. It
//! does not write an activation marker, rename `current`, dispatch recovery
//! bootstrap, or implement an application port.

use crate::archive::{
    create_staging_attempt, open_new_file_beneath, open_or_create_private_directory,
    open_private_directory, sync_open_handle, visit_archive_entries, ArchiveEntryReader,
    ArchiveError, ArchiveLimits,
};
use crate::evidence::{canonical_digest_hex, path_to_storage_value, relative_evidence_path};
use crate::kernel::{timestamp, LockedDataRoot};
use crate::portability::{
    schema_fingerprint, stream_archive_entity, verify_domain_relations, verify_sqlite_integrity,
};
use crate::restore::{
    preflight_workspace_archive, read_manifest, DigestingReader, RestorePreflightError,
    VerifiedArchivePreflight,
};
use crate::restore_activation::{
    activate_verified_restore, recover_current_activation, verify_complete_restore,
    write_restore_phase, RestoreActivationError, RestoreActivationMarker,
    RESTORE_STAGING_DIRECTORY, RESTORE_STATE_FILES,
};
use crate::schema::{migrate, workspace_revision, SCHEMA_VERSION};
use chrono::{DateTime, Utc};
use fasti_application::{
    CancellationSignal, CapabilityKey, FastiProblem, PortabilityLimits, ReadSeek,
    WorkspaceExportEntity, MAX_CORRECTION_REASON_BYTES,
};
use fasti_contracts::VerifiedInboundWorkspaceManifest;
use fasti_domain::{
    ClientId, CorrectionId, EvidenceId, ExternalIdentifierClaim, ExternalIdentifierId, Grain,
    InterpretationId, InterpretationState, NamespaceDefinition, NamespaceLicencePosture,
    ObservationId, ObservedAt, OccurredAt, OccurrenceId, OperationId, ProfileId, ReceiptId,
    RecordId, RecordStatus, RequestCorrelationId, RestoreAttemptId, RestoreStatus, ReviewItemId,
    ReviewStatus, Sha256Digest, WorkspaceId,
};
use rusqlite::{params, Connection, OpenFlags, Transaction, TransactionBehavior};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

const DATABASE_NAME: &str = "fasti.sqlite3";

struct CancellableSource<'a> {
    source: &'a mut dyn ReadSeek,
    cancellation: &'a CancellationSignal,
}

impl Read for CancellableSource<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.cancellation.is_cancelled() {
            return Err(io::Error::from(io::ErrorKind::Interrupted));
        }
        self.source.read(bytes)
    }
}

impl Seek for CancellableSource<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.source.seek(position)
    }
}

fn check_cancellation(cancellation: &CancellationSignal) -> Result<(), RestoreImportError> {
    if cancellation.is_cancelled() {
        Err(RestoreImportError::Canceled)
    } else {
        Ok(())
    }
}

fn admit_restore_capacity(
    root: &File,
    preflight: &VerifiedArchivePreflight,
    limits: PortabilityLimits,
) -> Result<(), RestoreImportError> {
    let blob_bytes = preflight
        .manifest()
        .manifest()
        .blobs()
        .iter()
        .try_fold(0_u64, |total, blob| total.checked_add(blob.byte_length()))
        .ok_or(RestoreImportError::CapacityExceeded)?;
    let required = limits
        .max_snapshot_bytes
        .get()
        .checked_add(blob_bytes)
        .and_then(|bytes| bytes.checked_add(limits.cleanup_reserve_bytes.get()))
        .filter(|bytes| *bytes <= limits.scratch_ceiling_bytes.get())
        .ok_or(RestoreImportError::CapacityExceeded)?;
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, required);
        return Err(RestoreImportError::UnsupportedPlatform);
    }
    #[cfg(target_os = "linux")]
    {
        let stats = rustix::fs::fstatvfs(root)
            .map_err(|error| RestoreImportError::Archive(ArchiveError::Io(error.into())))?;
        let available = stats
            .f_bavail
            .checked_mul(stats.f_frsize)
            .ok_or(RestoreImportError::CapacityExceeded)?;
        if available < required {
            return Err(RestoreImportError::CapacityExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(crate) enum RestoreImportError {
    #[error(transparent)]
    Preflight(#[from] RestorePreflightError),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("pass-two archive source could not be rewound")]
    Rewind(#[source] io::Error),
    #[error("staged SQLite could not be opened or migrated")]
    Sqlite(#[source] rusqlite::Error),
    #[error("pass-two archive differs from the verified pass-one bytes")]
    ArchiveChanged,
    #[error("pass-two manifest differs from the verified pass-one manifest")]
    ManifestChanged,
    #[error("pass-two entry {actual} is out of order; expected {expected}")]
    EntryOrder { expected: String, actual: String },
    #[error("pass-two stream {path} has a row larger than {limit} bytes")]
    RowTooLarge { path: String, limit: u64 },
    #[error("pass-two stream {path} contains an invalid typed row")]
    InvalidRow { path: String },
    #[error("pass-two stream {path} contains a non-canonical row")]
    NonCanonicalRow { path: String },
    #[error("pass-two stream {path} is not strictly ordered")]
    RowOrder { path: String },
    #[error("pass-two stream descriptor differs for {path}")]
    StreamDescriptor { path: String },
    #[error("pass-two blob descriptor differs for {path}")]
    BlobDescriptor { path: String },
    #[error("staged row violates a SQLite or domain invariant in {entity}")]
    RowInvariant {
        entity: &'static str,
        #[source]
        source: rusqlite::Error,
    },
    #[error("staged rows violate workspace, profile, namespace, reference, or domain invariants")]
    DomainInvariant,
    #[error("staged SQLite schema does not match the verified manifest")]
    SchemaMismatch,
    #[error("staged workspace revision does not match the verified manifest")]
    RevisionMismatch,
    #[error("staged row counts do not match the verified manifest")]
    CountMismatch,
    #[error("staged evidence rows do not match the verified blob inventory")]
    EvidenceMismatch,
    #[error("staged restore unexpectedly contains node-local authorization state")]
    NodeLocalStatePresent,
    #[error("pass-two staging is unavailable on this platform")]
    UnsupportedPlatform,
    #[error("restore was canceled")]
    Canceled,
    #[error("restore staging capacity is insufficient")]
    CapacityExceeded,
    #[error(transparent)]
    Activation(#[from] RestoreActivationError),
    #[error("staged files could not be synchronized")]
    Sync(#[source] ArchiveError),
    #[error("restore failed and its staging attempt could not be removed")]
    Cleanup {
        failure: Box<RestoreImportError>,
        cleanup: Box<RestoreImportError>,
    },
}

/// An imported but deliberately unactivated workspace.
///
/// The open handles keep the staging and attempt directories anchored. A
/// future activation slice must consume this value while it still holds those
/// handles. Dropping it removes the private attempt on a best-effort basis.
#[allow(dead_code)] // inspection accessors remain private until store-adapter activation
pub(crate) struct StagedWorkspaceImport {
    staging: File,
    attempt: File,
    attempt_name: String,
    blob_digests: Vec<String>,
    blob_prefixes: BTreeSet<String>,
    workspace_id: WorkspaceId,
    workspace_revision: u64,
    marker: RestoreActivationMarker,
    cleaned: bool,
}

#[allow(dead_code)] // inspection accessors remain private until store-adapter activation
impl StagedWorkspaceImport {
    pub(crate) const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub(crate) const fn workspace_revision(&self) -> u64 {
        self.workspace_revision
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn database_path(&self) -> PathBuf {
        descriptor_child_path(&self.attempt, DATABASE_NAME)
    }

    pub(crate) const fn marker(&self) -> &RestoreActivationMarker {
        &self.marker
    }

    pub(crate) fn activate(
        mut self,
        data_root: &File,
    ) -> Result<RestoreActivationMarker, RestoreImportError> {
        let marker = self.marker.clone();
        match activate_verified_restore(
            data_root,
            &self.staging,
            &self.attempt,
            &self.attempt_name,
            &marker,
        ) {
            Ok(()) => self.cleaned = true,
            Err(activation) => match open_private_directory(&self.staging, &self.attempt_name) {
                Ok(_) => {
                    let failure = RestoreImportError::Activation(activation);
                    return match self.cleanup_internal() {
                        Ok(()) => Err(failure),
                        Err(cleanup) => Err(RestoreImportError::Cleanup {
                            failure: Box::new(failure),
                            cleanup: Box::new(cleanup),
                        }),
                    };
                }
                Err(ArchiveError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {
                    self.cleaned = true;
                    recover_current_activation(data_root)?;
                }
                Err(error) => {
                    self.cleaned = true;
                    return Err(RestoreImportError::Archive(error));
                }
            },
        }
        let verified = verify_complete_restore(
            data_root,
            marker.restore_attempt_id(),
            marker.workspace_id(),
        )?;
        if verified != marker {
            return Err(RestoreActivationError::MarkerMismatch.into());
        }
        Ok(marker)
    }

    pub(crate) fn cleanup(mut self) -> Result<(), RestoreImportError> {
        self.cleanup_internal()
    }

    fn cleanup_internal(&mut self) -> Result<(), RestoreImportError> {
        if self.cleaned {
            return Ok(());
        }
        cleanup_attempt(
            &self.staging,
            &self.attempt,
            &self.attempt_name,
            &self.blob_digests,
            &self.blob_prefixes,
        )?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for StagedWorkspaceImport {
    fn drop(&mut self) {
        let _ = self.cleanup_internal();
    }
}

/// Verify pass one, rewind the same source, and import pass two into staging.
///
/// This is deliberately private and has no `RestoreWorkspacePort`
/// implementation. The returned attempt contains no COMPLETE marker and is
/// never renamed into `current` by this slice.
pub(crate) fn stage_workspace_archive_pass_two(
    data_root: &LockedDataRoot,
    source: &mut dyn ReadSeek,
    restore_attempt_id: RestoreAttemptId,
    correlation_id: RequestCorrelationId,
    limits: PortabilityLimits,
    cancellation: &CancellationSignal,
) -> Result<StagedWorkspaceImport, RestoreImportError> {
    check_cancellation(cancellation)?;
    let mut guarded_source = CancellableSource {
        source,
        cancellation,
    };
    let preflight = match preflight_workspace_archive(&mut guarded_source, limits) {
        Ok(preflight) => preflight,
        Err(_) if cancellation.is_cancelled() => return Err(RestoreImportError::Canceled),
        Err(error) => return Err(error.into()),
    };
    check_cancellation(cancellation)?;
    let root = data_root
        .anchored_directory()
        .ok_or(RestoreImportError::UnsupportedPlatform)?;
    admit_restore_capacity(root, &preflight, limits)?;
    let attempt_name = restore_attempt_id.to_string();
    let (staging, attempt) =
        create_staging_attempt(root, RESTORE_STAGING_DIRECTORY, &attempt_name)?;
    let blob_digests = preflight
        .manifest()
        .manifest()
        .blobs()
        .iter()
        .map(|blob| {
            canonical_digest_hex(blob.digest().as_str())
                .expect("verified manifest digest is canonical")
                .to_owned()
        })
        .collect();
    let blob_prefixes = preflight
        .manifest()
        .manifest()
        .blobs()
        .iter()
        .map(|blob| {
            canonical_digest_hex(blob.digest().as_str())
                .expect("verified manifest digest is canonical")[..2]
                .to_owned()
        })
        .collect();
    let mut staged = StagedWorkspaceImport {
        staging,
        attempt,
        attempt_name,
        blob_digests,
        blob_prefixes,
        workspace_id: preflight.manifest().manifest().workspace_id(),
        workspace_revision: preflight.manifest().manifest().workspace_revision(),
        marker: RestoreActivationMarker::from_preflight(restore_attempt_id, &preflight),
        cleaned: false,
    };

    let imported = (|| {
        write_restore_phase(&staged.attempt, RestoreStatus::Received)?;
        write_restore_phase(&staged.attempt, RestoreStatus::Staging)?;
        check_cancellation(cancellation)?;
        import_verified_pass_two(
            &mut guarded_source,
            &staged,
            &preflight,
            correlation_id,
            limits,
            cancellation,
        )?;
        check_cancellation(cancellation)?;
        write_restore_phase(&staged.attempt, RestoreStatus::Verified)?;
        Ok(())
    })();
    let imported = if cancellation.is_cancelled() {
        Err(RestoreImportError::Canceled)
    } else {
        imported
    };
    let source = guarded_source.source;
    let rewind = source
        .seek(SeekFrom::Start(0))
        .map(|_| ())
        .map_err(RestoreImportError::Rewind);
    let result = imported.and(rewind);
    match result {
        Ok(()) => Ok(staged),
        Err(failure) => match staged.cleanup_internal() {
            Ok(()) => Err(failure),
            Err(cleanup) => Err(RestoreImportError::Cleanup {
                failure: Box::new(failure),
                cleanup: Box::new(cleanup),
            }),
        },
    }
}

fn import_verified_pass_two(
    source: &mut dyn ReadSeek,
    staged: &StagedWorkspaceImport,
    preflight: &VerifiedArchivePreflight,
    correlation_id: RequestCorrelationId,
    limits: PortabilityLimits,
    cancellation: &CancellationSignal,
) -> Result<(), RestoreImportError> {
    let payloads = open_or_create_private_directory(&staged.attempt, "payloads")?;
    let sha256 = open_or_create_private_directory(&payloads, "sha256")?;
    let database_file = open_new_file_beneath(&staged.attempt, Path::new(DATABASE_NAME))?;
    let database_path = descriptor_child_path(&staged.attempt, DATABASE_NAME);
    let mut connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(RestoreImportError::Sqlite)?;
    verify_database_identity(&staged.attempt, &database_file)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(RestoreImportError::Sqlite)?;
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .map_err(RestoreImportError::Sqlite)?;
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(RestoreImportError::Sqlite)?;
    migrate(&connection).map_err(RestoreImportError::Sqlite)?;
    verify_schema(&connection, preflight.manifest(), correlation_id)?;

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(RestoreImportError::Sqlite)?;
    transaction
        .pragma_update(None, "defer_foreign_keys", "ON")
        .map_err(RestoreImportError::Sqlite)?;
    let mut imported_counts = [0_u64; WorkspaceExportEntity::ALL.len()];
    let pass_two = visit_import_entries(
        source,
        &transaction,
        &sha256,
        preflight,
        &mut imported_counts,
        limits,
        cancellation,
    );
    pass_two?;

    verify_imported_database(
        &transaction,
        preflight.manifest(),
        correlation_id,
        &imported_counts,
        limits,
        cancellation,
    )?;
    transaction.commit().map_err(RestoreImportError::Sqlite)?;
    connection
        .close()
        .map_err(|(_, error)| RestoreImportError::Sqlite(error))?;

    if database_file
        .metadata()
        .map_err(|source| RestoreImportError::Archive(ArchiveError::Io(source)))?
        .len()
        > limits.max_snapshot_bytes.get()
    {
        return Err(RestoreImportError::CapacityExceeded);
    }

    sync_open_handle(&database_file).map_err(RestoreImportError::Sync)?;
    sync_open_handle(&sha256).map_err(RestoreImportError::Sync)?;
    sync_open_handle(&payloads).map_err(RestoreImportError::Sync)?;
    sync_open_handle(&staged.attempt).map_err(RestoreImportError::Sync)?;
    sync_open_handle(&staged.staging).map_err(RestoreImportError::Sync)?;
    Ok(())
}

fn visit_import_entries(
    source: &mut dyn ReadSeek,
    transaction: &Transaction<'_>,
    sha256_root: &File,
    preflight: &VerifiedArchivePreflight,
    imported_counts: &mut [u64; WorkspaceExportEntity::ALL.len()],
    limits: PortabilityLimits,
    cancellation: &CancellationSignal,
) -> Result<(), RestoreImportError> {
    let expanded_ceiling = limits
        .archive_expanded_ceiling()
        .ok_or(RestoreImportError::DomainInvariant)?;
    let archive_limits = ArchiveLimits::new(
        limits.max_archive_bytes.get(),
        limits.max_entries.get(),
        limits.max_entry_bytes.get(),
        expanded_ceiling,
    )?;
    let manifest = preflight.manifest().manifest();
    let mut digesting = DigestingReader::new(&mut *source);
    let mut stream_index = 0_usize;
    let mut blob_index = 0_usize;
    let mut manifest_seen = false;
    let summary = visit_archive_entries(&mut digesting, archive_limits, |path, size, reader| {
        check_cancellation(cancellation)?;
        if stream_index < WorkspaceExportEntity::ALL.len() {
            let entity = WorkspaceExportEntity::ALL[stream_index];
            let expected_path = format!("{}.ndjson", entity.as_str());
            if path != expected_path {
                return Err(RestoreImportError::EntryOrder {
                    expected: expected_path,
                    actual: path.to_owned(),
                });
            }
            let count = import_stream(
                transaction,
                entity,
                size,
                reader,
                manifest.workspace_id(),
                &manifest.streams()[stream_index],
                limits.max_entry_bytes.get(),
                cancellation,
            )?;
            imported_counts[stream_index] = count;
            stream_index += 1;
            return Ok(());
        }

        if path == "manifest.json" {
            let bytes = read_manifest(path, size, reader)?;
            let pass_two =
                VerifiedInboundWorkspaceManifest::try_from_canonical_json(&bytes, limits)
                    .map_err(RestorePreflightError::Manifest)?;
            if &pass_two != preflight.manifest() {
                return Err(RestoreImportError::ManifestChanged);
            }
            manifest_seen = true;
            return Ok(());
        }

        let expected =
            manifest
                .blobs()
                .get(blob_index)
                .ok_or_else(|| RestoreImportError::EntryOrder {
                    expected: "manifest.json".to_owned(),
                    actual: path.to_owned(),
                })?;
        copy_blob(sha256_root, path, size, reader, expected, cancellation)?;
        blob_index += 1;
        Ok(())
    })?;

    if stream_index != WorkspaceExportEntity::ALL.len()
        || blob_index != manifest.blobs().len()
        || !manifest_seen
    {
        return Err(RestoreImportError::ArchiveChanged);
    }
    if digesting.bytes_read() != preflight.archive_bytes()
        || digesting.digest() != *preflight.archive_digest()
        || summary != preflight.archive_summary()
    {
        return Err(RestoreImportError::ArchiveChanged);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn import_stream(
    transaction: &Transaction<'_>,
    entity: WorkspaceExportEntity,
    declared_size: u64,
    reader: &mut ArchiveEntryReader<'_>,
    workspace_id: WorkspaceId,
    expected: &fasti_application::WorkspaceStreamDescriptor,
    max_row_bytes: u64,
    cancellation: &CancellationSignal,
) -> Result<u64, RestoreImportError> {
    let path = format!("{}.ndjson", entity.as_str());
    let mut reader = BufReader::new(DigestingReader::new(reader));
    let mut line = Vec::new();
    line.try_reserve_exact(usize::try_from(declared_size.min(4096)).map_err(|_| {
        RestoreImportError::RowTooLarge {
            path: path.clone(),
            limit: max_row_bytes,
        }
    })?)
    .map_err(|_| RestoreImportError::RowTooLarge {
        path: path.clone(),
        limit: max_row_bytes,
    })?;
    let mut row_count = 0_u64;
    let mut prior_key = None;
    loop {
        check_cancellation(cancellation)?;
        let read = read_bounded_line(&mut reader, &mut line, max_row_bytes, &path)?;
        if read == 0 {
            break;
        }
        if line.last() != Some(&b'\n') {
            return Err(RestoreImportError::InvalidRow { path: path.clone() });
        }
        let key = import_row(transaction, entity, &line, workspace_id, &path)?;
        if prior_key.as_ref().is_some_and(|prior| prior >= &key) {
            return Err(RestoreImportError::RowOrder { path: path.clone() });
        }
        prior_key = Some(key);
        row_count = row_count
            .checked_add(1)
            .ok_or(RestoreImportError::StreamDescriptor { path: path.clone() })?;
    }
    let observed = reader.into_inner();
    if observed.bytes_read() != declared_size
        || observed.bytes_read() != expected.byte_length()
        || row_count != expected.row_count()
        || observed.digest() != *expected.digest()
        || expected.entity() != entity
    {
        return Err(RestoreImportError::StreamDescriptor { path });
    }
    Ok(row_count)
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    line: &mut Vec<u8>,
    limit: u64,
    path: &str,
) -> Result<usize, RestoreImportError> {
    line.clear();
    loop {
        let available = reader
            .fill_buf()
            .map_err(|_| RestoreImportError::InvalidRow {
                path: path.to_owned(),
            })?;
        if available.is_empty() {
            return Ok(line.len());
        }
        let consumed = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |position| position + 1);
        let next = line
            .len()
            .checked_add(consumed)
            .and_then(|length| u64::try_from(length).ok())
            .filter(|length| *length <= limit)
            .ok_or_else(|| RestoreImportError::RowTooLarge {
                path: path.to_owned(),
                limit,
            })?;
        let additional = usize::try_from(next)
            .ok()
            .and_then(|next| next.checked_sub(line.len()))
            .ok_or_else(|| RestoreImportError::RowTooLarge {
                path: path.to_owned(),
                limit,
            })?;
        line.try_reserve(additional)
            .map_err(|_| RestoreImportError::RowTooLarge {
                path: path.to_owned(),
                limit,
            })?;
        line.extend_from_slice(&available[..consumed]);
        let complete = available.get(consumed.wrapping_sub(1)) == Some(&b'\n');
        reader.consume(consumed);
        if complete {
            return Ok(line.len());
        }
    }
}

fn copy_blob(
    sha256_root: &File,
    path: &str,
    declared_size: u64,
    reader: &mut ArchiveEntryReader<'_>,
    expected: &fasti_application::WorkspaceBlobDescriptor,
    cancellation: &CancellationSignal,
) -> Result<(), RestoreImportError> {
    let digest_hex = canonical_digest_hex(expected.digest().as_str())
        .expect("verified manifest digest is canonical");
    let relative = relative_evidence_path(digest_hex);
    let expected_path = path_to_storage_value(&relative);
    if path != expected_path || declared_size != expected.byte_length() {
        return Err(RestoreImportError::BlobDescriptor {
            path: expected_path,
        });
    }
    let prefix = open_or_create_private_directory(sha256_root, &digest_hex[..2])?;
    let mut destination = open_new_file_beneath(&prefix, Path::new(digest_hex))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; crate::archive::MAX_IO_CHUNK_BYTES];
    loop {
        check_cancellation(cancellation)?;
        let read = reader
            .read(&mut buffer)
            .map_err(|_| RestoreImportError::BlobDescriptor {
                path: expected_path.clone(),
            })?;
        if read == 0 {
            break;
        }
        destination
            .write_all(&buffer[..read])
            .map_err(|source| RestoreImportError::Archive(ArchiveError::Io(source)))?;
        hasher.update(&buffer[..read]);
        bytes =
            bytes
                .checked_add(read as u64)
                .ok_or_else(|| RestoreImportError::BlobDescriptor {
                    path: expected_path.clone(),
                })?;
    }
    let digest = Sha256Digest::parse(format!("sha256:{:x}", hasher.finalize()))
        .expect("SHA-256 output is canonical");
    if bytes != declared_size || digest != *expected.digest() {
        return Err(RestoreImportError::BlobDescriptor {
            path: expected_path,
        });
    }
    sync_open_handle(&destination).map_err(RestoreImportError::Sync)?;
    sync_open_handle(&prefix).map_err(RestoreImportError::Sync)?;
    Ok(())
}

fn verify_schema(
    connection: &Connection,
    verified: &VerifiedInboundWorkspaceManifest,
    correlation_id: RequestCorrelationId,
) -> Result<(), RestoreImportError> {
    let manifest = verified.manifest();
    if manifest.migration_version() != u32::try_from(SCHEMA_VERSION).unwrap_or(u32::MAX) {
        return Err(RestoreImportError::SchemaMismatch);
    }
    let fingerprint = schema_fingerprint(connection, correlation_id)
        .map_err(|_| RestoreImportError::SchemaMismatch)?;
    if fingerprint.migration_version() != manifest.migration_version()
        || fingerprint.digest() != manifest.migration_digest()
    {
        return Err(RestoreImportError::SchemaMismatch);
    }
    Ok(())
}

fn verify_imported_database(
    transaction: &Transaction<'_>,
    verified: &VerifiedInboundWorkspaceManifest,
    correlation_id: RequestCorrelationId,
    imported_counts: &[u64; WorkspaceExportEntity::ALL.len()],
    limits: PortabilityLimits,
    cancellation: &CancellationSignal,
) -> Result<(), RestoreImportError> {
    check_cancellation(cancellation)?;
    let manifest = verified.manifest();
    for (index, descriptor) in manifest.streams().iter().enumerate() {
        if imported_counts[index] != descriptor.row_count() {
            return Err(RestoreImportError::CountMismatch);
        }
    }
    verify_sql_counts(transaction, imported_counts)?;
    verify_evidence_inventory(transaction, verified)?;
    verify_node_local_state_absent(transaction)?;
    verify_import_domain_invariants(transaction, manifest.workspace_id())?;
    verify_sqlite_integrity(transaction, CapabilityKey::RestoreWorkspace, correlation_id)
        .map_err(|_| RestoreImportError::DomainInvariant)?;
    verify_domain_relations(
        transaction,
        manifest.workspace_id(),
        CapabilityKey::RestoreWorkspace,
        correlation_id,
    )
    .map_err(|_| RestoreImportError::DomainInvariant)?;

    for expected in manifest.streams() {
        let mut sink = io::sink();
        let actual = stream_archive_entity(
            transaction,
            manifest.workspace_id(),
            expected.entity(),
            limits,
            &mut sink,
            &mut || {
                if cancellation.is_cancelled() {
                    Err(Box::new(FastiProblem::restore_canceled(correlation_id)))
                } else {
                    Ok(())
                }
            },
            correlation_id,
        )
        .map_err(|_| {
            if cancellation.is_cancelled() {
                RestoreImportError::Canceled
            } else {
                RestoreImportError::DomainInvariant
            }
        })?;
        if actual != *expected {
            return Err(RestoreImportError::StreamDescriptor {
                path: format!("{}.ndjson", expected.entity().as_str()),
            });
        }
    }

    let revision = i64::try_from(manifest.workspace_revision())
        .map_err(|_| RestoreImportError::RevisionMismatch)?;
    if transaction
        .execute(
            "UPDATE workspace_revisions SET revision = ?1 WHERE workspace_id = ?2",
            params![revision, manifest.workspace_id().to_string()],
        )
        .map_err(RestoreImportError::Sqlite)?
        != 1
    {
        return Err(RestoreImportError::RevisionMismatch);
    }
    let restored = workspace_revision(transaction, &manifest.workspace_id().to_string())
        .map_err(RestoreImportError::Sqlite)?;
    if restored != revision {
        return Err(RestoreImportError::RevisionMismatch);
    }
    verify_schema(transaction, verified, correlation_id)?;
    Ok(())
}

fn verify_sql_counts(
    transaction: &Transaction<'_>,
    expected: &[u64; WorkspaceExportEntity::ALL.len()],
) -> Result<(), RestoreImportError> {
    const TABLES: [&str; WorkspaceExportEntity::ALL.len()] = [
        "workspaces",
        "profiles",
        "clients",
        "records",
        "namespace_definitions",
        "external_identifiers",
        "evidence",
        "observations",
        "observation_clues",
        "occurrences",
        "interpretations",
        "review_items",
        "review_candidates",
        "corrections",
        "receipts",
        "operations",
    ];
    for (index, table) in TABLES.iter().enumerate() {
        let count: i64 = transaction
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .map_err(RestoreImportError::Sqlite)?;
        if u64::try_from(count).ok() != Some(expected[index]) {
            return Err(RestoreImportError::CountMismatch);
        }
    }
    Ok(())
}

fn verify_evidence_inventory(
    transaction: &Transaction<'_>,
    verified: &VerifiedInboundWorkspaceManifest,
) -> Result<(), RestoreImportError> {
    let count: i64 = transaction
        .query_row("SELECT COUNT(*) FROM evidence", [], |row| row.get(0))
        .map_err(RestoreImportError::Sqlite)?;
    if usize::try_from(count).ok() != Some(verified.manifest().blobs().len()) {
        return Err(RestoreImportError::EvidenceMismatch);
    }
    for blob in verified.manifest().blobs() {
        let digest_hex = canonical_digest_hex(blob.digest().as_str())
            .expect("verified manifest digest is canonical");
        let expected_path = path_to_storage_value(&relative_evidence_path(digest_hex));
        let matched: bool = transaction
            .query_row(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM evidence
                    WHERE evidence_id = ?1 AND workspace_id = ?2
                      AND digest = ?3 AND size_bytes = ?4 AND relative_path = ?5
                )
                "#,
                params![
                    blob.evidence_id().to_string(),
                    verified.manifest().workspace_id().to_string(),
                    blob.digest().to_string(),
                    i64::try_from(blob.byte_length())
                        .map_err(|_| RestoreImportError::EvidenceMismatch)?,
                    expected_path,
                ],
                |row| row.get(0),
            )
            .map_err(RestoreImportError::Sqlite)?;
        if !matched {
            return Err(RestoreImportError::EvidenceMismatch);
        }
    }
    Ok(())
}

fn verify_node_local_state_absent(transaction: &Transaction<'_>) -> Result<(), RestoreImportError> {
    let count: i64 = transaction
        .query_row(
            r#"
            SELECT (SELECT COUNT(*) FROM node_state)
                 + (SELECT COUNT(*) FROM credentials)
                 + (SELECT COUNT(*) FROM profile_grants)
                 + (SELECT COUNT(*) FROM grant_scopes)
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(RestoreImportError::Sqlite)?;
    if count != 0 {
        return Err(RestoreImportError::NodeLocalStatePresent);
    }
    Ok(())
}

fn verify_import_domain_invariants(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
) -> Result<(), RestoreImportError> {
    let invalid: i64 = transaction
        .query_row(
            r#"
            SELECT COUNT(*) FROM (
                SELECT external.external_identifier_id AS invalid_id
                FROM external_identifiers external
                JOIN records record ON record.record_id = external.record_id
                WHERE external.workspace_id = ?1 AND (
                    record.workspace_id <> external.workspace_id
                    OR record.grain <> external.grain
                )

                UNION ALL

                SELECT interpretation.interpretation_id
                FROM interpretations interpretation
                JOIN observations observation
                  ON observation.observation_id = interpretation.observation_id
                WHERE observation.workspace_id = ?1 AND (
                    (interpretation.state = 'resolved' AND interpretation.record_id IS NULL)
                    OR (interpretation.state <> 'resolved' AND interpretation.record_id IS NOT NULL)
                )

                UNION ALL

                SELECT receipt.receipt_id
                FROM receipts receipt
                JOIN observations receipt_observation
                  ON receipt_observation.observation_id = receipt.observation_id
                JOIN evidence receipt_evidence
                  ON receipt_evidence.evidence_id = receipt.evidence_id
                LEFT JOIN interpretations receipt_interpretation
                  ON receipt_interpretation.interpretation_id = receipt.interpretation_id
                WHERE receipt.workspace_id = ?1 AND (
                    receipt.committed_at < receipt.received_at
                    OR receipt_observation.source_client_id <> receipt.client_id
                    OR receipt.payload_digest <> receipt_evidence.digest
                    OR (receipt.resolution = 'resolved' AND receipt.record_id IS NULL)
                    OR (receipt.resolution = 'conflicted' AND receipt.review_item_id IS NULL)
                    OR (receipt.resolution <> 'conflicted' AND receipt.review_item_id IS NOT NULL)
                    OR (
                        receipt.interpretation_id IS NULL
                        AND (
                            receipt.occurrence_id IS NOT NULL
                            OR receipt.record_id IS NOT NULL
                            OR receipt.review_item_id IS NOT NULL
                        )
                    )
                    OR (
                        receipt.interpretation_id IS NOT NULL
                        AND (
                            receipt.occurrence_id IS NULL
                            OR receipt_interpretation.occurrence_id <> receipt.occurrence_id
                            OR receipt_interpretation.state <> receipt.resolution
                            OR COALESCE(receipt_interpretation.record_id, '')
                               <> COALESCE(receipt.record_id, '')
                        )
                    )
                )

                UNION ALL

                SELECT review.review_item_id
                FROM review_items review
                WHERE review.workspace_id = ?1 AND EXISTS (
                    SELECT 1
                    FROM interpretations child
                    WHERE child.prior_interpretation_id = review.current_interpretation_id
                )

                UNION ALL

                SELECT correction.correction_id
                FROM corrections correction
                WHERE correction.workspace_id = ?1
                  AND correction.prior_interpretation_id = correction.replacement_interpretation_id

                UNION ALL

                SELECT review.review_item_id
                FROM review_items review
                LEFT JOIN review_candidates candidate
                  ON candidate.review_item_id = review.review_item_id
                LEFT JOIN records candidate_record
                  ON candidate_record.record_id = candidate.record_id
                WHERE review.workspace_id = ?1
                GROUP BY review.review_item_id
                HAVING COUNT(candidate.record_id) < 2
                    OR COUNT(DISTINCT candidate_record.grain) <> 1
            ) invalid
            "#,
            [workspace_id.to_string()],
            |row| row.get(0),
        )
        .map_err(RestoreImportError::Sqlite)?;
    if invalid != 0 {
        return Err(RestoreImportError::DomainInvariant);
    }

    let chain_count: (i64, i64) = transaction
        .query_row(
            r#"
            WITH RECURSIVE reachable(interpretation_id) AS (
                SELECT interpretation_id
                FROM interpretations
                WHERE prior_interpretation_id IS NULL
                UNION
                SELECT child.interpretation_id
                FROM interpretations child
                JOIN reachable parent
                  ON child.prior_interpretation_id = parent.interpretation_id
            )
            SELECT
                (SELECT COUNT(*) FROM interpretations),
                (SELECT COUNT(*) FROM reachable)
            "#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(RestoreImportError::Sqlite)?;
    if chain_count.0 != chain_count.1 {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RowKey {
    One(String),
    Two(String, String),
    TextInteger(String, u64),
}

fn import_row(
    transaction: &Transaction<'_>,
    entity: WorkspaceExportEntity,
    line: &[u8],
    workspace_id: WorkspaceId,
    path: &str,
) -> Result<RowKey, RestoreImportError> {
    match entity {
        WorkspaceExportEntity::Workspaces => {
            let row: WorkspaceRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![row.workspace_id.to_string(), row.created_at],
                ),
            )?;
            Ok(RowKey::One(row.workspace_id.to_string()))
        }
        WorkspaceExportEntity::Profiles => {
            let row: ProfileRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                    params![
                        row.profile_id.to_string(),
                        row.workspace_id.to_string(),
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.profile_id.to_string()))
        }
        WorkspaceExportEntity::Clients => {
            let row: ClientRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO clients(
                        client_id, workspace_id, status, current_credential_epoch, created_at
                    ) VALUES (?1, ?2, ?3, 0, ?4)
                    "#,
                    params![
                        row.client_id.to_string(),
                        row.workspace_id.to_string(),
                        row.status.as_str(),
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.client_id.to_string()))
        }
        WorkspaceExportEntity::Records => {
            let row: RecordRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, ?3, 'active', ?4)",
                    params![
                        row.record_id.to_string(),
                        row.workspace_id.to_string(),
                        row.grain.as_str(),
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.record_id.to_string()))
        }
        WorkspaceExportEntity::NamespaceDefinitions => {
            let row: NamespaceRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            validate_namespace(&row)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO namespace_definitions(
                        workspace_id, namespace, label, supported_grains,
                        id_pattern, normalization, licence_posture, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#,
                    params![
                        row.workspace_id.to_string(),
                        row.namespace,
                        row.label,
                        row.supported_grains,
                        row.id_pattern,
                        row.normalization,
                        row.licence_posture.as_str(),
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.namespace))
        }
        WorkspaceExportEntity::ExternalIdentifiers => {
            let row: ExternalIdentifierRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            validate_claim(&row.namespace, row.grain, &row.value)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO external_identifiers(
                        external_identifier_id, workspace_id, record_id,
                        namespace, grain, value, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    params![
                        row.external_identifier_id.to_string(),
                        row.workspace_id.to_string(),
                        row.record_id.to_string(),
                        row.namespace,
                        row.grain.as_str(),
                        row.value,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.external_identifier_id.to_string()))
        }
        WorkspaceExportEntity::Evidence => {
            let row: EvidenceRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            let digest_hex = canonical_digest_hex(row.digest.as_str())
                .ok_or(RestoreImportError::DomainInvariant)?;
            let relative_path = path_to_storage_value(&relative_evidence_path(digest_hex));
            let size =
                i64::try_from(row.size_bytes).map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO evidence(
                        evidence_id, workspace_id, digest, size_bytes, relative_path, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    "#,
                    params![
                        row.evidence_id.to_string(),
                        row.workspace_id.to_string(),
                        row.digest.to_string(),
                        size,
                        relative_path,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.evidence_id.to_string()))
        }
        WorkspaceExportEntity::Observations => {
            let row: ObservationRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            validate_timestamp(&row.received_at)?;
            validate_claimed_json::<ObservedAt>(&row.observed_at_json)?;
            if let Some(value) = row.occurred_at_json.as_deref() {
                validate_claimed_json::<OccurredAt>(value)?;
            }
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO observations(
                        observation_id, workspace_id, profile_id, source_client_id,
                        evidence_id, occurred_at_json, observed_at_json, received_at, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    "#,
                    params![
                        row.observation_id.to_string(),
                        row.workspace_id.to_string(),
                        row.profile_id.to_string(),
                        row.source_client_id.to_string(),
                        row.evidence_id.to_string(),
                        row.occurred_at_json,
                        row.observed_at_json,
                        row.received_at,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.observation_id.to_string()))
        }
        WorkspaceExportEntity::ObservationClues => {
            let row: ObservationClueRow = decode_row(line, path)?;
            validate_claim(&row.namespace, row.grain, &row.value)?;
            let ordinal =
                i64::try_from(row.ordinal).map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO observation_clues(observation_id, ordinal, namespace, grain, value) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        row.observation_id.to_string(),
                        ordinal,
                        row.namespace,
                        row.grain.as_str(),
                        row.value,
                    ],
                ),
            )?;
            Ok(RowKey::TextInteger(
                row.observation_id.to_string(),
                row.ordinal,
            ))
        }
        WorkspaceExportEntity::Occurrences => {
            let row: OccurrenceRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            if let Some(value) = row.occurred_at_json.as_deref() {
                validate_claimed_json::<OccurredAt>(value)?;
            }
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO occurrences(
                        occurrence_id, workspace_id, profile_id, observation_id,
                        record_id, occurred_at_json, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    params![
                        row.occurrence_id.to_string(),
                        row.workspace_id.to_string(),
                        row.profile_id.to_string(),
                        row.observation_id.to_string(),
                        row.record_id.map(|value| value.to_string()),
                        row.occurred_at_json,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.occurrence_id.to_string()))
        }
        WorkspaceExportEntity::Interpretations => {
            let row: InterpretationRow = decode_row(line, path)?;
            validate_timestamp(&row.created_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO interpretations(
                        interpretation_id, observation_id, occurrence_id,
                        prior_interpretation_id, record_id, state, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    params![
                        row.interpretation_id.to_string(),
                        row.observation_id.to_string(),
                        row.occurrence_id.to_string(),
                        row.prior_interpretation_id.map(|value| value.to_string()),
                        row.record_id.map(|value| value.to_string()),
                        interpretation_state(row.state),
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.interpretation_id.to_string()))
        }
        WorkspaceExportEntity::ReviewItems => {
            let row: ReviewItemRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            validate_timestamp(&row.updated_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO review_items(
                        review_item_id, workspace_id, profile_id, observation_id,
                        current_interpretation_id, status, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#,
                    params![
                        row.review_item_id.to_string(),
                        row.workspace_id.to_string(),
                        row.profile_id.to_string(),
                        row.observation_id.to_string(),
                        row.current_interpretation_id.to_string(),
                        review_status(row.status),
                        row.created_at,
                        row.updated_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.review_item_id.to_string()))
        }
        WorkspaceExportEntity::ReviewCandidates => {
            let row: ReviewCandidateRow = decode_row(line, path)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO review_candidates(review_item_id, record_id) VALUES (?1, ?2)",
                    params![row.review_item_id.to_string(), row.record_id.to_string()],
                ),
            )?;
            Ok(RowKey::Two(
                row.review_item_id.to_string(),
                row.record_id.to_string(),
            ))
        }
        WorkspaceExportEntity::Corrections => {
            let row: CorrectionRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            if row.reason.trim().is_empty()
                || row.reason.len() > MAX_CORRECTION_REASON_BYTES
                || row.reason.contains('\0')
            {
                return Err(RestoreImportError::DomainInvariant);
            }
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO corrections(
                        correction_id, workspace_id, profile_id, observation_id,
                        prior_interpretation_id, replacement_interpretation_id,
                        actor_client_id, record_id, reason, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                    "#,
                    params![
                        row.correction_id.to_string(),
                        row.workspace_id.to_string(),
                        row.profile_id.to_string(),
                        row.observation_id.to_string(),
                        row.prior_interpretation_id.to_string(),
                        row.replacement_interpretation_id.to_string(),
                        row.actor_client_id.to_string(),
                        row.record_id.map(|value| value.to_string()),
                        row.reason,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.correction_id.to_string()))
        }
        WorkspaceExportEntity::Receipts => {
            let row: ReceiptRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            if row.capability_key != CapabilityKey::AcceptObservation {
                return Err(RestoreImportError::DomainInvariant);
            }
            validate_timestamp(&row.received_at)?;
            validate_timestamp(&row.committed_at)?;
            validate_timestamp(&row.created_at)?;
            let received = parse_timestamp(&row.received_at)?;
            let committed = parse_timestamp(&row.committed_at)?;
            if committed < received
                || matches!(row.resolution, StoredResolution::Resolved) && row.record_id.is_none()
                || matches!(row.resolution, StoredResolution::Conflicted)
                    && row.review_item_id.is_none()
            {
                return Err(RestoreImportError::DomainInvariant);
            }
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO receipts(
                        receipt_id, operation_id, workspace_id, profile_id, client_id,
                        capability_key, observation_id, occurrence_id, interpretation_id,
                        record_id, review_item_id, evidence_id, payload_digest, resolution,
                        received_at, committed_at, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                    "#,
                    params![
                        row.receipt_id.to_string(),
                        row.operation_id.to_string(),
                        row.workspace_id.to_string(),
                        row.profile_id.to_string(),
                        row.client_id.to_string(),
                        capability_storage(row.capability_key),
                        row.observation_id.to_string(),
                        row.occurrence_id.map(|value| value.to_string()),
                        row.interpretation_id.map(|value| value.to_string()),
                        row.record_id.map(|value| value.to_string()),
                        row.review_item_id.map(|value| value.to_string()),
                        row.evidence_id.to_string(),
                        row.payload_digest.to_string(),
                        row.resolution.as_str(),
                        row.received_at,
                        row.committed_at,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.receipt_id.to_string()))
        }
        WorkspaceExportEntity::Operations => {
            let row: OperationRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            if row.capability_key != CapabilityKey::AcceptObservation {
                return Err(RestoreImportError::DomainInvariant);
            }
            validate_timestamp(&row.created_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO operations(
                        workspace_id, client_id, operation_id, capability_key,
                        semantic_digest, receipt_id, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    params![
                        row.workspace_id.to_string(),
                        row.client_id.to_string(),
                        row.operation_id.to_string(),
                        capability_storage(row.capability_key),
                        row.semantic_digest.to_string(),
                        row.receipt_id.to_string(),
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::Two(
                row.client_id.to_string(),
                row.operation_id.to_string(),
            ))
        }
    }
}

fn decode_row<T>(line: &[u8], path: &str) -> Result<T, RestoreImportError>
where
    T: DeserializeOwned + Serialize,
{
    let value: T = serde_json::from_slice(&line[..line.len().saturating_sub(1)]).map_err(|_| {
        RestoreImportError::InvalidRow {
            path: path.to_owned(),
        }
    })?;
    let mut canonical = serde_json::to_vec(&value).map_err(|_| RestoreImportError::InvalidRow {
        path: path.to_owned(),
    })?;
    canonical.push(b'\n');
    if canonical != line {
        return Err(RestoreImportError::NonCanonicalRow {
            path: path.to_owned(),
        });
    }
    Ok(value)
}

fn insert_row(
    _transaction: &Transaction<'_>,
    entity: WorkspaceExportEntity,
    result: rusqlite::Result<usize>,
) -> Result<(), RestoreImportError> {
    let changed = result.map_err(|source| RestoreImportError::RowInvariant {
        entity: entity.as_str(),
        source,
    })?;
    if changed != 1 {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

fn require_workspace(actual: WorkspaceId, expected: WorkspaceId) -> Result<(), RestoreImportError> {
    if actual != expected {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>, RestoreImportError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| RestoreImportError::DomainInvariant)
}

fn validate_timestamp(value: &str) -> Result<(), RestoreImportError> {
    let parsed = parse_timestamp(value)?;
    if timestamp(parsed) != value {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

fn validate_claimed_json<T>(value: &str) -> Result<(), RestoreImportError>
where
    T: DeserializeOwned + Serialize,
{
    let parsed: T = serde_json::from_str(value).map_err(|_| RestoreImportError::DomainInvariant)?;
    if serde_json::to_string(&parsed).map_err(|_| RestoreImportError::DomainInvariant)? != value {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

fn validate_namespace(row: &NamespaceRow) -> Result<(), RestoreImportError> {
    let grains = row
        .supported_grains
        .split(',')
        .map(str::parse::<Grain>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RestoreImportError::DomainInvariant)?;
    let definition = NamespaceDefinition::try_new(
        row.namespace.clone(),
        row.label.clone(),
        grains,
        row.id_pattern.clone(),
        row.normalization.clone(),
        row.licence_posture,
    )
    .map_err(|_| RestoreImportError::DomainInvariant)?;
    let canonical_grains = definition
        .grains()
        .iter()
        .map(|grain| grain.as_str())
        .collect::<Vec<_>>()
        .join(",");
    if definition.namespace().as_str() != row.namespace
        || definition.label() != row.label
        || definition.id_pattern() != row.id_pattern
        || definition.normalization() != row.normalization
        || canonical_grains != row.supported_grains
    {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

fn validate_claim(namespace: &str, grain: Grain, value: &str) -> Result<(), RestoreImportError> {
    let claim = ExternalIdentifierClaim::try_new(namespace.to_owned(), grain, value.to_owned())
        .map_err(|_| RestoreImportError::DomainInvariant)?;
    if claim.namespace() != namespace || claim.value() != value {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

fn capability_storage(value: CapabilityKey) -> String {
    serde_json::to_value(value)
        .expect("CapabilityKey always serializes")
        .as_str()
        .expect("CapabilityKey is a string enum")
        .to_owned()
}

fn interpretation_state(value: InterpretationState) -> &'static str {
    match value {
        InterpretationState::Unresolved => "unresolved",
        InterpretationState::Resolved => "resolved",
        InterpretationState::Conflicted => "conflicted",
    }
}

fn review_status(value: ReviewStatus) -> &'static str {
    match value {
        ReviewStatus::Open => "open",
        ReviewStatus::Deferred => "deferred",
        ReviewStatus::Resolved => "resolved",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredClientStatus {
    Active,
    Revoked,
}

impl StoredClientStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Revoked => "revoked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredResolution {
    Unresolved,
    Resolved,
    Conflicted,
}

impl StoredResolution {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unresolved => "unresolved",
            Self::Resolved => "resolved",
            Self::Conflicted => "conflicted",
        }
    }
}

macro_rules! archive_row {
    ($name:ident { $($field:ident: $type:ty),+ $(,)? }) => {
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $name {
            $( $field: $type ),+
        }
    };
}

archive_row!(WorkspaceRow {
    created_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(ProfileRow {
    created_at: String,
    profile_id: ProfileId,
    workspace_id: WorkspaceId,
});
archive_row!(ClientRow {
    client_id: ClientId,
    created_at: String,
    status: StoredClientStatus,
    workspace_id: WorkspaceId,
});
archive_row!(RecordRow {
    created_at: String,
    grain: Grain,
    record_id: RecordId,
    status: RecordStatus,
    workspace_id: WorkspaceId,
});
archive_row!(NamespaceRow {
    created_at: String,
    id_pattern: String,
    label: String,
    licence_posture: NamespaceLicencePosture,
    namespace: String,
    normalization: String,
    supported_grains: String,
    workspace_id: WorkspaceId,
});
archive_row!(ExternalIdentifierRow {
    created_at: String,
    external_identifier_id: ExternalIdentifierId,
    grain: Grain,
    namespace: String,
    record_id: RecordId,
    value: String,
    workspace_id: WorkspaceId,
});
archive_row!(EvidenceRow {
    created_at: String,
    digest: Sha256Digest,
    evidence_id: EvidenceId,
    size_bytes: u64,
    workspace_id: WorkspaceId,
});
archive_row!(ObservationRow {
    created_at: String,
    evidence_id: EvidenceId,
    observation_id: ObservationId,
    observed_at_json: String,
    occurred_at_json: Option<String>,
    profile_id: ProfileId,
    received_at: String,
    source_client_id: ClientId,
    workspace_id: WorkspaceId,
});
archive_row!(ObservationClueRow {
    grain: Grain,
    namespace: String,
    observation_id: ObservationId,
    ordinal: u64,
    value: String,
});
archive_row!(OccurrenceRow {
    created_at: String,
    observation_id: ObservationId,
    occurred_at_json: Option<String>,
    occurrence_id: OccurrenceId,
    profile_id: ProfileId,
    record_id: Option<RecordId>,
    workspace_id: WorkspaceId,
});
archive_row!(InterpretationRow {
    created_at: String,
    interpretation_id: InterpretationId,
    observation_id: ObservationId,
    occurrence_id: OccurrenceId,
    prior_interpretation_id: Option<InterpretationId>,
    record_id: Option<RecordId>,
    state: InterpretationState,
});
archive_row!(ReviewItemRow {
    created_at: String,
    current_interpretation_id: InterpretationId,
    observation_id: ObservationId,
    profile_id: ProfileId,
    review_item_id: ReviewItemId,
    status: ReviewStatus,
    updated_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(ReviewCandidateRow {
    record_id: RecordId,
    review_item_id: ReviewItemId,
});
archive_row!(CorrectionRow {
    actor_client_id: ClientId,
    correction_id: CorrectionId,
    created_at: String,
    observation_id: ObservationId,
    prior_interpretation_id: InterpretationId,
    profile_id: ProfileId,
    reason: String,
    record_id: Option<RecordId>,
    replacement_interpretation_id: InterpretationId,
    workspace_id: WorkspaceId,
});
archive_row!(ReceiptRow {
    capability_key: CapabilityKey,
    client_id: ClientId,
    committed_at: String,
    created_at: String,
    evidence_id: EvidenceId,
    interpretation_id: Option<InterpretationId>,
    observation_id: ObservationId,
    occurrence_id: Option<OccurrenceId>,
    operation_id: OperationId,
    payload_digest: Sha256Digest,
    profile_id: ProfileId,
    receipt_id: ReceiptId,
    received_at: String,
    record_id: Option<RecordId>,
    resolution: StoredResolution,
    review_item_id: Option<ReviewItemId>,
    workspace_id: WorkspaceId,
});
archive_row!(OperationRow {
    capability_key: CapabilityKey,
    client_id: ClientId,
    created_at: String,
    operation_id: OperationId,
    receipt_id: ReceiptId,
    semantic_digest: Sha256Digest,
    workspace_id: WorkspaceId,
});

#[cfg(target_os = "linux")]
fn descriptor_child_path(directory: &File, name: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/{}", directory.as_raw_fd(), name))
}

#[cfg(not(target_os = "linux"))]
fn descriptor_child_path(_directory: &File, _name: &str) -> PathBuf {
    PathBuf::new()
}

#[cfg(target_os = "linux")]
fn verify_database_identity(attempt: &File, created: &File) -> Result<(), RestoreImportError> {
    let current = std::fs::metadata(descriptor_child_path(attempt, DATABASE_NAME))
        .map_err(|source| RestoreImportError::Archive(ArchiveError::Io(source)))?;
    let retained = created
        .metadata()
        .map_err(|source| RestoreImportError::Archive(ArchiveError::Io(source)))?;
    if !current.is_file() || current.dev() != retained.dev() || current.ino() != retained.ino() {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn verify_database_identity(_attempt: &File, _created: &File) -> Result<(), RestoreImportError> {
    Err(RestoreImportError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn cleanup_attempt(
    staging: &File,
    attempt: &File,
    attempt_name: &str,
    blob_digests: &[String],
    blob_prefixes: &BTreeSet<String>,
) -> Result<(), RestoreImportError> {
    let mut first_error = None;
    let payloads = open_cleanup_directory(attempt, "payloads", &mut first_error);
    let sha256 = payloads
        .as_ref()
        .and_then(|payloads| open_cleanup_directory(payloads, "sha256", &mut first_error));
    if let Some(sha256) = sha256.as_ref() {
        for digest in blob_digests {
            let prefix_name = &digest[..2];
            if let Some(prefix) = open_cleanup_directory(sha256, prefix_name, &mut first_error) {
                remove_child(&prefix, digest, false, &mut first_error);
            }
        }
        for prefix in blob_prefixes {
            remove_child(sha256, prefix, true, &mut first_error);
        }
    }
    if let Some(payloads) = payloads.as_ref() {
        remove_child(payloads, "sha256", true, &mut first_error);
    }
    for (name, directory) in [
        ("payloads", true),
        ("fasti.sqlite3-shm", false),
        ("fasti.sqlite3-wal", false),
        ("fasti.sqlite3-journal", false),
        (DATABASE_NAME, false),
    ] {
        remove_child(attempt, name, directory, &mut first_error);
    }
    for name in RESTORE_STATE_FILES {
        remove_child(attempt, name, false, &mut first_error);
    }
    remove_child(staging, attempt_name, true, &mut first_error);
    if let Err(error) = sync_open_handle(staging) {
        first_error.get_or_insert(RestoreImportError::Sync(error));
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(target_os = "linux")]
fn open_cleanup_directory(
    parent: &File,
    name: &str,
    first_error: &mut Option<RestoreImportError>,
) -> Option<File> {
    match crate::archive::open_private_directory(parent, name) {
        Ok(directory) => Some(directory),
        Err(ArchiveError::Io(error)) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            if first_error.is_none() {
                *first_error = Some(RestoreImportError::Archive(error));
            }
            None
        }
    }
}

#[cfg(target_os = "linux")]
fn remove_child(
    parent: &File,
    name: &str,
    directory: bool,
    first_error: &mut Option<RestoreImportError>,
) {
    let flags = if directory {
        rustix::fs::AtFlags::REMOVEDIR
    } else {
        rustix::fs::AtFlags::empty()
    };
    match rustix::fs::unlinkat(parent, name, flags) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => {}
        Err(error) if first_error.is_none() => {
            *first_error = Some(RestoreImportError::Archive(ArchiveError::Io(
                io::Error::from_raw_os_error(error.raw_os_error()),
            )));
        }
        Err(_) => {}
    }
}

#[cfg(not(target_os = "linux"))]
fn cleanup_attempt(
    _staging: &File,
    _attempt: &File,
    _attempt_name: &str,
    _blob_digests: &[String],
    _blob_prefixes: &BTreeSet<String>,
) -> Result<(), RestoreImportError> {
    Err(RestoreImportError::UnsupportedPlatform)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::archive::ArchiveWriter;
    use crate::kernel::scope_storage_key;
    use crate::online_archive::export_online_workspace_archive;
    use crate::test_support::TestNode;
    use fasti_application::{
        AppendCorrectionCommand, CancellationSignal, CompleteRecoveryBootstrapRequest,
        CorrectionPort, CorrectionTarget, CreateRecordCommand, ExportWorkspaceQuery,
        ExportWorkspaceRequest, IdentityPort, ObservationAcceptancePort,
        PrepareRecoveryBootstrapRequest, RegisterNamespaceDefinitionCommand, ResolveReviewCommand,
        RestoreWorkspaceRequest, ReviewPort, ReviewResolutionTarget, ScopeKey, SecretMaterial,
        WorkspaceArchiveDestination, WorkspaceManifest, WorkspaceStreamDescriptor,
    };
    use fasti_contracts::CanonicalWorkspaceManifestProjection;
    use fasti_domain::{ClaimedTrust, ExternalIdentifierClaim, ObservedAt};
    use std::io::Cursor;
    use std::num::NonZeroU64;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct DestinationState {
        bytes: Vec<u8>,
        completed: bool,
        aborted: bool,
    }

    struct MemoryDestination(Arc<Mutex<DestinationState>>);

    impl Write for MemoryDestination {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0
                .lock()
                .expect("destination state")
                .bytes
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl WorkspaceArchiveDestination for MemoryDestination {
        fn preflight(&self, _required_bytes: u64) -> io::Result<()> {
            Ok(())
        }

        fn complete(
            self: Box<Self>,
            _archive_digest: &Sha256Digest,
            _manifest_digest: &Sha256Digest,
        ) -> io::Result<()> {
            self.0.lock().expect("destination state").completed = true;
            Ok(())
        }

        fn abort(self: Box<Self>) -> io::Result<()> {
            self.0.lock().expect("destination state").aborted = true;
            Ok(())
        }
    }

    fn nonzero(value: u64) -> NonZeroU64 {
        NonZeroU64::new(value).expect("non-zero test limit")
    }

    fn limits() -> PortabilityLimits {
        PortabilityLimits {
            max_snapshot_bytes: nonzero(32 * 1024 * 1024),
            max_wal_growth_bytes: nonzero(8 * 1024 * 1024),
            max_archive_bytes: nonzero(64 * 1024 * 1024),
            max_uncompressed_bytes: nonzero(32 * 1024 * 1024),
            max_entry_bytes: nonzero(8 * 1024 * 1024),
            max_entries: nonzero(64),
            max_rows_per_stream: nonzero(1_024),
            max_path_bytes: nonzero(100),
            max_path_depth: nonzero(8),
            max_decompression_ratio: nonzero(1_024),
            scratch_ceiling_bytes: nonzero(64 * 1024 * 1024),
            cleanup_reserve_bytes: nonzero(1024 * 1024),
            backup_step_pages: nonzero(64),
            backup_step_millis: nonzero(1_000),
        }
    }

    fn grant_export(node: &TestNode) {
        node.kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection")
            .execute(
                "INSERT OR IGNORE INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                params![
                    node.access.grant_id().to_string(),
                    scope_storage_key(ScopeKey::WorkspaceExport)
                ],
            )
            .expect("grant workspace export");
    }

    struct FullFixture {
        node: TestNode,
        archive: Vec<u8>,
        archive_revision: u64,
        evidence_digest: Sha256Digest,
        evidence_bytes: Vec<u8>,
    }

    fn full_fixture() -> FullFixture {
        let node = TestNode::new();
        let first = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Release,
            ))
            .expect("first record")
            .record_id();
        let second = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Release,
            ))
            .expect("second record")
            .record_id();
        node.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                NamespaceDefinition::try_new(
                    "fixture",
                    "Fixture identifier",
                    [Grain::Release],
                    "^[ab]$",
                    "identity",
                    NamespaceLicencePosture::Unknown,
                )
                .expect("fixture namespace"),
            ))
            .expect("register fixture namespace");
        let first_claim =
            ExternalIdentifierClaim::try_new("fixture", Grain::Release, "a").expect("first claim");
        let second_claim =
            ExternalIdentifierClaim::try_new("fixture", Grain::Release, "b").expect("second claim");
        node.kernel
            .attach_identifier(fasti_application::AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                first,
                first_claim.clone(),
            ))
            .expect("attach first identifier");
        node.kernel
            .attach_identifier(fasti_application::AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                second,
                second_claim.clone(),
            ))
            .expect("attach second identifier");

        let evidence_bytes = b"strict pass-two fixture evidence".to_vec();
        let evidence = node.upload(&evidence_bytes);
        let accepted = node
            .kernel
            .authorize_and_accept(
                fasti_application::AcceptObservationCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    OperationId::new_v7(),
                    None,
                    ObservedAt::parse("2026-08-24T12:00:00.000000Z", ClaimedTrust::DeviceObserved)
                        .expect("observed timestamp"),
                    evidence.clone(),
                )
                .with_identity_clues(
                    vec![first_claim.clone(), second_claim.clone()],
                    Some(Grain::Release),
                ),
            )
            .expect("accept conflicted observation");
        let receipt = accepted.receipt();
        let review_item_id = receipt.review_item_id().expect("conflicted review item");
        let observation_id = receipt.observation_id();
        node.kernel
            .resolve_review(ResolveReviewCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                review_item_id,
                ReviewResolutionTarget::Existing(first),
                Vec::new(),
            ))
            .expect("resolve fixture review");
        node.kernel
            .append_correction(AppendCorrectionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                observation_id,
                CorrectionTarget::Record(second),
                "corrected fixture identity",
            ))
            .expect("append fixture correction");
        grant_export(&node);

        let destination = Arc::new(Mutex::new(DestinationState::default()));
        let outcome = export_online_workspace_archive(
            &node.kernel,
            ExportWorkspaceRequest::new(
                ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), node.access),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(MemoryDestination(Arc::clone(&destination))),
        )
        .expect("export full fixture");
        let destination = destination.lock().expect("destination state");
        assert!(destination.completed);
        assert!(!destination.aborted);
        FullFixture {
            node,
            archive: destination.bytes.clone(),
            archive_revision: outcome.workspace_revision(),
            evidence_digest: evidence.digest().clone(),
            evidence_bytes,
        }
    }

    fn archive_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                .expect("archive limits");
        let mut entries = Vec::new();
        visit_archive_entries(Cursor::new(bytes), archive_limits, |path, _size, reader| {
            let mut content = Vec::new();
            reader.read_to_end(&mut content)?;
            entries.push((path.to_owned(), content));
            Ok::<(), ArchiveError>(())
        })
        .expect("archive entries");
        entries
    }

    fn digest(bytes: &[u8]) -> Sha256Digest {
        Sha256Digest::parse(format!("sha256:{:x}", Sha256::digest(bytes)))
            .expect("canonical digest")
    }

    fn rewrite_stream(
        archive: &[u8],
        entity: WorkspaceExportEntity,
        mutate: impl FnOnce(&mut Vec<u8>),
    ) -> Vec<u8> {
        let mut entries = archive_entries(archive);
        let manifest_bytes = entries.pop().expect("manifest entry").1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&manifest_bytes, limits())
                .expect("verified fixture manifest");
        let entry = entries
            .iter_mut()
            .find(|(path, _)| path == &format!("{}.ndjson", entity.as_str()))
            .expect("stream entry");
        mutate(&mut entry.1);

        let mut streams = verified.manifest().streams().to_vec();
        streams[entity.index()] = WorkspaceStreamDescriptor::new(
            entity,
            entry.1.iter().filter(|byte| **byte == b'\n').count() as u64,
            entry.1.len() as u64,
            digest(&entry.1),
        );
        let manifest = verified.manifest();
        let rebuilt = WorkspaceManifest::try_new(
            manifest.workspace_id(),
            manifest.workspace_revision(),
            manifest.contract_version().to_owned(),
            manifest.migration_version(),
            manifest.migration_digest().clone(),
            streams,
            manifest.blobs().to_vec(),
        )
        .expect("rebuilt application manifest");
        let projection = CanonicalWorkspaceManifestProjection::try_from_application(rebuilt)
            .expect("rebuilt wire manifest");
        entries.push((
            "manifest.json".to_owned(),
            projection.canonical_json_bytes().to_vec(),
        ));

        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                .expect("archive limits");
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).expect("archive writer");
        for (path, bytes) in entries {
            writer
                .append(&path, bytes.len() as u64, Cursor::new(bytes))
                .expect("append rewritten entry");
        }
        writer.finish().expect("finish rewritten archive")
    }

    fn assert_attempt_removed(root: &Path, attempt_id: RestoreAttemptId) {
        assert!(
            !root
                .join(RESTORE_STAGING_DIRECTORY)
                .join(attempt_id.to_string())
                .exists(),
            "failed import must remove its staging attempt"
        );
    }

    #[test]
    fn full_archive_stages_all_frozen_streams_and_blob_without_local_authority() {
        let fixture = full_fixture();
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let mut source = Cursor::new(fixture.archive);

        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut source,
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage verified fixture");
        assert_eq!(staged.workspace_id(), fixture.node.access.workspace_id());
        assert_eq!(staged.workspace_revision(), fixture.archive_revision);

        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open staged database");
        let expected_counts = [1_i64, 1, 1, 2, 1, 2, 1, 1, 2, 1, 3, 1, 2, 1, 1, 1];
        for (table, expected) in [
            "workspaces",
            "profiles",
            "clients",
            "records",
            "namespace_definitions",
            "external_identifiers",
            "evidence",
            "observations",
            "observation_clues",
            "occurrences",
            "interpretations",
            "review_items",
            "review_candidates",
            "corrections",
            "receipts",
            "operations",
        ]
        .into_iter()
        .zip(expected_counts)
        {
            let count: i64 = database
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("staged row count");
            assert_eq!(count, expected, "{table}");
        }
        let local_rows: i64 = database
            .query_row(
                "SELECT (SELECT COUNT(*) FROM node_state) + (SELECT COUNT(*) FROM credentials) + (SELECT COUNT(*) FROM profile_grants) + (SELECT COUNT(*) FROM grant_scopes)",
                [],
                |row| row.get(0),
            )
            .expect("node-local row count");
        assert_eq!(local_rows, 0);
        drop(database);

        let digest_hex = canonical_digest_hex(fixture.evidence_digest.as_str())
            .expect("canonical fixture digest");
        let blob_path = descriptor_child_path(
            &staged.attempt,
            &path_to_storage_value(&relative_evidence_path(digest_hex)),
        );
        assert_eq!(
            std::fs::read(blob_path).expect("staged evidence"),
            fixture.evidence_bytes
        );
        assert!(!descriptor_child_path(&staged.attempt, "COMPLETE").exists());
        assert!(!restore_root.path().join("current").exists());

        staged.cleanup().expect("remove staged attempt");
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn hostile_cross_workspace_reference_is_rejected_and_cleaned() {
        let fixture = full_fixture();
        let hostile = rewrite_stream(&fixture.archive, WorkspaceExportEntity::Profiles, |bytes| {
            let mut row: serde_json::Value =
                serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                    .expect("profile row");
            row["workspace_id"] = serde_json::Value::String(WorkspaceId::new_v7().to_string());
            *bytes = serde_json::to_vec(&row).expect("mutated row");
            bytes.push(b'\n');
        });
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();

        let error = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(hostile),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .expect("cross-workspace row must fail");
        assert!(
            matches!(error, RestoreImportError::DomainInvariant),
            "unexpected error: {error:?}"
        );
        assert_attempt_removed(restore_root.path(), attempt_id);
        assert!(!restore_root.path().join("current").exists());
    }

    #[test]
    fn hostile_stream_order_is_rejected_and_cleaned() {
        let fixture = full_fixture();
        let hostile = rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::ObservationClues,
            |bytes| {
                let mut rows = bytes
                    .split_inclusive(|byte| *byte == b'\n')
                    .map(<[u8]>::to_vec)
                    .collect::<Vec<_>>();
                assert_eq!(rows.len(), 2);
                rows.reverse();
                *bytes = rows.concat();
            },
        );
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();

        let error = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(hostile),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .expect("out-of-order stream rows must fail");
        assert!(
            matches!(error, RestoreImportError::RowOrder { .. }),
            "unexpected error: {error:?}"
        );
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn hostile_missing_foreign_reference_is_rejected_and_cleaned() {
        let fixture = full_fixture();
        let hostile = rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::ReviewCandidates,
            |bytes| {
                let mut rows = bytes
                    .split_inclusive(|byte| *byte == b'\n')
                    .map(|line| {
                        serde_json::from_slice::<serde_json::Value>(
                            &line[..line.len().saturating_sub(1)],
                        )
                        .expect("candidate row")
                    })
                    .collect::<Vec<_>>();
                let mut missing = [
                    RecordId::new_v7().to_string(),
                    RecordId::new_v7().to_string(),
                ];
                missing.sort();
                let mut replacement = Vec::new();
                for (row, missing_record) in rows.iter_mut().zip(missing) {
                    row["record_id"] = serde_json::Value::String(missing_record);
                    replacement.extend_from_slice(
                        &serde_json::to_vec(row).expect("missing reference row"),
                    );
                    replacement.push(b'\n');
                }
                *bytes = replacement;
            },
        );
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();

        let error = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(hostile),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .expect("missing record reference must fail");
        assert!(
            matches!(error, RestoreImportError::DomainInvariant),
            "unexpected error: {error:?}"
        );
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn hostile_typed_row_is_rejected_and_cleaned() {
        let fixture = full_fixture();
        let hostile = rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::ObservationClues,
            |bytes| {
                let newline = bytes
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .expect("first row");
                let mut first: serde_json::Value =
                    serde_json::from_slice(&bytes[..newline]).expect("first clue");
                first["ordinal"] = serde_json::Value::String("0".to_owned());
                let mut replacement = serde_json::to_vec(&first).expect("typed hostile row");
                replacement.push(b'\n');
                replacement.extend_from_slice(&bytes[newline + 1..]);
                *bytes = replacement;
            },
        );
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();

        let error = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(hostile),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .expect("typed row mismatch must fail");
        assert!(
            matches!(error, RestoreImportError::InvalidRow { .. }),
            "unexpected error: {error:?}"
        );
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn canceled_restore_and_failed_capacity_admission_do_not_create_staging() {
        let fixture = full_fixture();
        let canceled_root = tempfile::tempdir().expect("canceled restore root");
        let canceled_lock =
            LockedDataRoot::acquire(canceled_root.path()).expect("exclusive canceled root");
        let cancellation = CancellationSignal::new();
        cancellation.cancel();
        let canceled = stage_workspace_archive_pass_two(
            &canceled_lock,
            &mut Cursor::new(fixture.archive.clone()),
            RestoreAttemptId::new_v7(),
            RequestCorrelationId::new_v7(),
            limits(),
            &cancellation,
        )
        .err()
        .expect("canceled restore");
        assert!(matches!(canceled, RestoreImportError::Canceled));
        assert!(!canceled_root.path().join("staging").exists());
        assert!(!canceled_root.path().join("current").exists());

        let capacity_root = tempfile::tempdir().expect("capacity restore root");
        let capacity_lock =
            LockedDataRoot::acquire(capacity_root.path()).expect("exclusive capacity root");
        let mut configured = limits();
        configured.scratch_ceiling_bytes = nonzero(
            configured.max_snapshot_bytes.get() + configured.cleanup_reserve_bytes.get() - 1,
        );
        let capacity = stage_workspace_archive_pass_two(
            &capacity_lock,
            &mut Cursor::new(fixture.archive),
            RestoreAttemptId::new_v7(),
            RequestCorrelationId::new_v7(),
            configured,
            &CancellationSignal::new(),
        )
        .err()
        .expect("capacity admission");
        assert!(matches!(capacity, RestoreImportError::CapacityExceeded));
        assert!(!capacity_root.path().join("staging").exists());
        assert!(!capacity_root.path().join("current").exists());
    }

    #[test]
    fn clean_restore_coordinator_returns_typed_outcome_and_refuses_replacement() {
        let fixture = full_fixture();
        let workspace_id = fixture.node.access.workspace_id();
        let archive = fixture.archive;
        let restore_root = tempfile::tempdir().expect("restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let outcome = crate::restore_coordinator::restore_clean_workspace(
            restore_root.path(),
            RestoreWorkspaceRequest::new(
                attempt_id,
                RequestCorrelationId::new_v7(),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(Cursor::new(archive.clone())),
        )
        .expect("clean restore");
        assert_eq!(outcome.restore_attempt_id(), attempt_id);
        assert_eq!(outcome.workspace_id(), workspace_id);

        let refused = crate::restore_coordinator::restore_clean_workspace(
            restore_root.path(),
            RestoreWorkspaceRequest::new(
                RestoreAttemptId::new_v7(),
                RequestCorrelationId::new_v7(),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(Cursor::new(archive)),
        )
        .expect_err("clean restore cannot replace current");
        assert_eq!(
            refused.problem().code(),
            fasti_application::ProblemCode::ValidationFailed
        );
    }

    #[test]
    fn full_import_activation_survives_owner_drop_and_opens_exact_database() {
        let fixture = full_fixture();
        let workspace_id = fixture.node.access.workspace_id();
        let profile_id = fixture.node.access.profile_id();
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let mut source = Cursor::new(fixture.archive);
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut source,
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage verified fixture");
        let expected_marker = staged.marker().clone();
        let root = lock.anchored_directory().expect("anchored restore root");
        let marker = staged.activate(root).expect("activate verified restore");
        assert_eq!(marker, expected_marker);
        assert!(restore_root.path().join("current/fasti.sqlite3").is_file());
        assert!(!restore_root
            .path()
            .join("staging")
            .join(attempt_id.to_string())
            .exists());

        let kernel = crate::SqliteKernel::open_locked(lock).expect("open activated kernel");
        let connection = kernel.inner.connection.lock().expect("restored database");
        let workspace_rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM workspaces WHERE workspace_id = ?1",
                [workspace_id.to_string()],
                |row| row.get(0),
            )
            .expect("restored workspace");
        let local_rows: i64 = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM node_state) + (SELECT COUNT(*) FROM credentials) + (SELECT COUNT(*) FROM profile_grants) + (SELECT COUNT(*) FROM grant_scopes)",
                [],
                |row| row.get(0),
            )
            .expect("node-local state");
        assert_eq!(workspace_rows, 1);
        assert_eq!(local_rows, 0);
        drop(connection);
        drop(kernel);

        let prepared = crate::recovery_coordinator::prepare_recovery_bootstrap(
            restore_root.path(),
            PrepareRecoveryBootstrapRequest::new(
                attempt_id,
                RequestCorrelationId::new_v7(),
                workspace_id,
                profile_id,
                false,
            ),
        )
        .expect("prepare recovery bootstrap after COMPLETE proof");
        let proof = SecretMaterial::from_bytes(*prepared.initialization_proof().expose_bytes());
        let completed = crate::recovery_coordinator::complete_recovery_bootstrap(
            restore_root.path(),
            CompleteRecoveryBootstrapRequest::new(
                attempt_id,
                RequestCorrelationId::new_v7(),
                workspace_id,
                profile_id,
                prepared.client_id(),
                proof,
                SecretMaterial::from_bytes([7_u8; 32]),
            ),
        )
        .expect("complete recovery bootstrap after COMPLETE proof");
        assert_eq!(completed.restore_attempt_id(), attempt_id);
        assert_eq!(completed.access().workspace_id(), workspace_id);
        assert_eq!(completed.access().profile_id(), profile_id);
    }

    #[test]
    fn typed_row_codec_rejects_unknown_fields_and_alternate_key_order() {
        let path = "workspaces.ndjson";
        let workspace = WorkspaceId::new_v7();
        let unknown = format!(
            "{{\"created_at\":\"2026-08-24T00:00:00.000000Z\",\"unknown\":true,\"workspace_id\":\"{workspace}\"}}\n"
        );
        assert!(matches!(
            decode_row::<WorkspaceRow>(unknown.as_bytes(), path),
            Err(RestoreImportError::InvalidRow { .. })
        ));

        let reordered = format!(
            "{{\"workspace_id\":\"{workspace}\",\"created_at\":\"2026-08-24T00:00:00.000000Z\"}}\n"
        );
        assert!(matches!(
            decode_row::<WorkspaceRow>(reordered.as_bytes(), path),
            Err(RestoreImportError::NonCanonicalRow { .. })
        ));
    }
}
