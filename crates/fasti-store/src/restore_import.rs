//! Private, non-activating pass-two import for verified B3 archives.
//!
//! Pass one and pass two consume the same already-open `Read + Seek` source.
//! This module creates only a fresh descriptor-relative staging attempt. It
//! does not write an activation marker, rename `current`, dispatch recovery
//! bootstrap, or implement an application port.

use crate::archive::{
    create_staging_attempt, open_existing_file_beneath, open_new_file_beneath,
    open_or_create_private_directory, open_private_directory, sync_open_handle,
    visit_archive_entries, ArchiveEntryReader, ArchiveError, ArchiveLimits,
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
    activate_verified_restore, crash_test_point, discard_pending_restore_phase,
    recover_current_activation, require_restore_phase, verify_complete_restore,
    write_restore_phase, RestoreActivationError, RestoreActivationMarker,
    RESTORE_STAGING_DIRECTORY, RESTORE_STATE_FILES,
};
use crate::schema::{
    migrate, migrate_imported_legacy_metadata_v12, repair_legacy_provider_coordinates_v1,
    workspace_revision, SCHEMA_VERSION,
};
use chrono::{DateTime, Utc};
use fasti_application::{
    CancellationSignal, CapabilityKey, FastiProblem, PortabilityLimits, ReadSeek,
    WorkspaceExportEntity, MAX_CORRECTION_REASON_BYTES, WORKSPACE_ARCHIVE_V1_FORMAT_VERSION,
    WORKSPACE_ARCHIVE_V2_FORMAT_VERSION, WORKSPACE_ARCHIVE_V3_FORMAT_VERSION,
    WORKSPACE_ARCHIVE_V4_FORMAT_VERSION, WORKSPACE_ARCHIVE_V5_FORMAT_VERSION,
    WORKSPACE_ARCHIVE_V6_FORMAT_VERSION,
};
use fasti_contracts::VerifiedInboundWorkspaceManifest;
use fasti_domain::{
    AuthSubjectId, ClientId, CorrectionId, EvidenceId, ExternalIdentifierClaim,
    ExternalIdentifierId, FieldClaim, FieldClaimLifecycleEvent, FieldClaimProvenance,
    FieldClaimStatus, FieldKey, FieldOverride, Grain, IdentityAssertionId, InterpretationId,
    InterpretationState, LastKnownGoodPolicy, MetadataAttribution, MetadataClaimId, MetadataLocale,
    MetadataProjectionPolicy, MetadataProviderId, MetadataRegion, NamespaceDefinition,
    NamespaceKey, NamespaceLicencePosture, ObservationId, ObservedAt, OccurredAt, OccurrenceId,
    OperationId, ProfileFieldOverride, ProfileId, RatingClaim, RatingScale, ReceiptId, ReceivedAt,
    RecordId, RecordStatus, RequestCorrelationId, RestoreAttemptId, RestoreStatus, ReviewItemId,
    ReviewStatus, Sha256Digest, TrackingDisposition, WorkspaceId,
};
use rusqlite::{
    params, Connection, OpenFlags, OptionalExtension, Transaction, TransactionBehavior,
};
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
    #[error("staged identity assertions or lifecycle chains are invalid")]
    IdentityRoutingInvariant,
    #[error("staged anime grouping rollback receipts are invalid")]
    PolicyReceiptInvariant,
    #[error("staged SQLite integrity checks failed")]
    SqliteIntegrity,
    #[error("staged cross-table relations are invalid")]
    RelationInvariant,
    #[error("staged domain aggregate relations are invalid")]
    AggregateInvariant,
    #[error("staged interpretation chains are invalid")]
    InterpretationChainInvariant,
    #[error("staged metadata lifecycle chains are invalid")]
    MetadataLifecycleInvariant,
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
        cancellation: &CancellationSignal,
    ) -> Result<RestoreActivationMarker, RestoreImportError> {
        if cancellation.is_cancelled() {
            return match self.cleanup_internal() {
                Ok(()) => Err(RestoreImportError::Canceled),
                Err(cleanup) => Err(RestoreImportError::Cleanup {
                    failure: Box::new(RestoreImportError::Canceled),
                    cleanup: Box::new(cleanup),
                }),
            };
        }
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
#[cfg(test)]
pub(crate) fn stage_workspace_archive_pass_two(
    data_root: &LockedDataRoot,
    source: &mut dyn ReadSeek,
    restore_attempt_id: RestoreAttemptId,
    correlation_id: RequestCorrelationId,
    limits: PortabilityLimits,
    cancellation: &CancellationSignal,
) -> Result<StagedWorkspaceImport, RestoreImportError> {
    let preflight = preflight_restore_source(source, limits, cancellation)?;
    stage_preflighted_workspace_archive_pass_two(
        data_root,
        source,
        restore_attempt_id,
        correlation_id,
        limits,
        cancellation,
        preflight,
    )
}

pub(crate) fn preflight_restore_source(
    source: &mut dyn ReadSeek,
    limits: PortabilityLimits,
    cancellation: &CancellationSignal,
) -> Result<VerifiedArchivePreflight, RestoreImportError> {
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
    Ok(preflight)
}

pub(crate) fn stage_preflighted_workspace_archive_pass_two(
    data_root: &LockedDataRoot,
    source: &mut dyn ReadSeek,
    restore_attempt_id: RestoreAttemptId,
    correlation_id: RequestCorrelationId,
    limits: PortabilityLimits,
    cancellation: &CancellationSignal,
    preflight: VerifiedArchivePreflight,
) -> Result<StagedWorkspaceImport, RestoreImportError> {
    check_cancellation(cancellation)?;
    let mut guarded_source = CancellableSource {
        source,
        cancellation,
    };
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
    crash_test_point("import", "rows_written");

    verify_imported_archive(
        &transaction,
        preflight.manifest(),
        correlation_id,
        &imported_counts,
        limits,
        cancellation,
    )?;
    repair_legacy_provider_coordinates_v1(&transaction).map_err(RestoreImportError::Sqlite)?;
    migrate_imported_legacy_metadata_v12(&transaction).map_err(RestoreImportError::Sqlite)?;
    crate::local_search::rebuild(&transaction).map_err(RestoreImportError::Sqlite)?;
    verify_imported_database(&transaction, preflight.manifest(), correlation_id)?;
    crash_test_point("import", "verified");
    transaction.commit().map_err(RestoreImportError::Sqlite)?;
    crash_test_point("import", "transaction_committed");
    connection
        .close()
        .map_err(|(_, error)| RestoreImportError::Sqlite(error))?;
    crash_test_point("import", "connection_closed");

    if database_file
        .metadata()
        .map_err(|source| RestoreImportError::Archive(ArchiveError::Io(source)))?
        .len()
        > limits.max_snapshot_bytes.get()
    {
        return Err(RestoreImportError::CapacityExceeded);
    }

    sync_open_handle(&database_file).map_err(RestoreImportError::Sync)?;
    crash_test_point("import", "database_synced");
    sync_open_handle(&sha256).map_err(RestoreImportError::Sync)?;
    crash_test_point("import", "sha256_synced");
    sync_open_handle(&payloads).map_err(RestoreImportError::Sync)?;
    crash_test_point("import", "payloads_synced");
    sync_open_handle(&staged.attempt).map_err(RestoreImportError::Sync)?;
    crash_test_point("import", "attempt_synced");
    sync_open_handle(&staged.staging).map_err(RestoreImportError::Sync)?;
    crash_test_point("import", "staging_synced");
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
        if stream_index < manifest.streams().len() {
            let entity = manifest.streams()[stream_index].entity();
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
                manifest.format_version(),
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

    if stream_index != manifest.streams().len()
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
    format_version: u32,
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
        if row_count >= expected.row_count() {
            return Err(RestoreImportError::StreamDescriptor { path: path.clone() });
        }
        let key = import_row(
            transaction,
            entity,
            format_version,
            &line,
            workspace_id,
            &path,
        )?;
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
    let digest_bytes: [u8; 32] = hasher.finalize().into();
    let digest = Sha256Digest::from_bytes(&digest_bytes);
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
    let fingerprint = schema_fingerprint(connection, correlation_id)
        .map_err(|_| RestoreImportError::SchemaMismatch)?;
    if fingerprint.migration_version() != u32::try_from(SCHEMA_VERSION).unwrap_or(u32::MAX)
        || !accepted_archive_schema(
            manifest.format_version(),
            manifest.migration_version(),
            manifest.migration_digest().as_str(),
            fingerprint.digest().as_str(),
        )
    {
        return Err(RestoreImportError::SchemaMismatch);
    }
    Ok(())
}

fn accepted_archive_schema(
    format_version: u32,
    version: u32,
    digest: &str,
    current_digest: &str,
) -> bool {
    if (format_version == WORKSPACE_ARCHIVE_V1_FORMAT_VERSION && version <= 11)
        || (format_version == WORKSPACE_ARCHIVE_V2_FORMAT_VERSION && (7..=11).contains(&version))
        || (format_version == WORKSPACE_ARCHIVE_V3_FORMAT_VERSION && version == 12)
        || (format_version == WORKSPACE_ARCHIVE_V4_FORMAT_VERSION && matches!(version, 13 | 14))
        || (format_version == WORKSPACE_ARCHIVE_V5_FORMAT_VERSION && version == 15)
        || (format_version == WORKSPACE_ARCHIVE_V6_FORMAT_VERSION && version == 16)
    {
        // Continue to the exact historical fingerprint match below.
    } else if format_version == fasti_application::WORKSPACE_ARCHIVE_FORMAT_VERSION
        && version == u32::try_from(SCHEMA_VERSION).unwrap_or(u32::MAX)
    {
        return digest == current_digest;
    } else {
        return false;
    }
    digest
        == match version {
            1 => "sha256:54fdbe7a1abd38a9f3fb528edf7fe18a2086a3465673eeecb37ced6831471eba",
            2 => "sha256:2d758df15b556e8f33ef79f6c0e366e793c81e22775ed6cde2d66222ad1cd51f",
            3 => "sha256:88567141fce9927e6c330f8b93d650b255ba3fd1a6c285df88a8197dc9a2d90d",
            4 => "sha256:51308ec0da45af490fbdec8221b324147d1b3af9c382b519bc4093d98a6e128b",
            5 => "sha256:862bc9f5ca71e1a3ec4fcf46a9312a9b05203a97e8420cb8ec621d31bfd29acc",
            6 => "sha256:9c415c43b39793ec3ac58bd819e6ad8e1c56c096c88fe62aa3e78a50696760aa",
            7 => "sha256:174264c60cee620d31041031f5510336208034318ff17378e01696bd53df27c3",
            8 => "sha256:4c3ecc5db6b6491f3884d56782241aafdca4e1bd6e0eaa58508738fbc8a974c3",
            9 => "sha256:4e0b16c5d4148d1b5e75a176ac1f3e58f6a31c569c0cfb5c6bb7c1de5d11584e",
            10 => "sha256:7c7c93de4419d8a56db8fd2dfb5c239fd3ffa994218b2ec583ec371c463726dd",
            11 => "sha256:c833fb634b64d0b9680e4734b22684e8eab36710fca5c95d4315f3141491687a",
            12 => "sha256:eea7d899b8c257b7bafa359a540bd25ba2cdc4d9ddb7f50ce0ec8f80e251cfb9",
            13 => "sha256:e470f2e8ae2972aa05fecd5b39642b79ef739de89eda204c37bf1d3e48f892c3",
            14 => "sha256:630bc759b1bc6148931fe1b496e6e149553c5c005cf8d5956da683f2872c0375",
            15 => "sha256:36720ca62ef606e52f960e71cb40452323269f14e4a4af984e2fe875279a155e",
            16 => "sha256:d7ae3b1ab15c0223245d1a9008833049e58e9ec882a6e1ba70a2a080fa3fd7a6",
            _ => return false,
        }
}

fn verify_imported_archive(
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
    verify_derived_metadata_state_absent(transaction)?;

    for expected in manifest.streams() {
        let mut sink = io::sink();
        let actual = stream_archive_entity(
            transaction,
            manifest.workspace_id(),
            expected.entity(),
            manifest.format_version(),
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
                RestoreImportError::StreamDescriptor {
                    path: format!("{}.ndjson", expected.entity().as_str()),
                }
            }
        })?;
        if actual != *expected {
            return Err(RestoreImportError::StreamDescriptor {
                path: format!("{}.ndjson", expected.entity().as_str()),
            });
        }
    }

    Ok(())
}

fn verify_imported_database(
    transaction: &Transaction<'_>,
    verified: &VerifiedInboundWorkspaceManifest,
    correlation_id: RequestCorrelationId,
) -> Result<(), RestoreImportError> {
    let manifest = verified.manifest();
    verify_import_domain_invariants(transaction, manifest.workspace_id(), correlation_id)?;
    verify_sqlite_integrity(transaction, CapabilityKey::RestoreWorkspace, correlation_id)
        .map_err(|_| RestoreImportError::SqliteIntegrity)?;
    verify_domain_relations(
        transaction,
        manifest.workspace_id(),
        CapabilityKey::RestoreWorkspace,
        correlation_id,
    )
    .map_err(|_| RestoreImportError::RelationInvariant)?;

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
        "metadata_field_claims",
        "metadata_field_overrides",
        "profile_record_tracking_dispositions",
        "metadata_claims",
        "metadata_claim_provenance",
        "metadata_rating_claims",
        "metadata_claim_lifecycle_events",
        "metadata_projection_policies",
        "metadata_profile_field_overrides",
        "metadata_legacy_override_ownership",
        "metadata_override_migration_receipts",
        "metadata_attributions",
        "metadata_refresh_receipts",
        "identity_assertions",
        "identity_assertion_lifecycle_events",
        "profile_anime_grouping_policies",
        "client_anime_grouping_policies",
        "anime_grouping_policy_receipts",
        "search_action_receipts",
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

const NODE_LOCAL_STATE_COUNT_SQL: &str = r#"
    SELECT (SELECT COUNT(*) FROM node_state)
         + (SELECT COUNT(*) FROM credentials)
         + (SELECT COUNT(*) FROM profile_grants)
         + (SELECT COUNT(*) FROM grant_scopes)
         + (SELECT COUNT(*) FROM auth_subjects)
         + (SELECT COUNT(*) FROM auth_subject_profile_grants)
         + (SELECT COUNT(*) FROM fasti_browser_sessions)
         + (SELECT COUNT(*) FROM fasti_browser_session_grants)
         + (SELECT COUNT(*) FROM trailbase_installation)
         + (SELECT COUNT(*) FROM trailbase_auth_anchors)
         + (SELECT COUNT(*) FROM workspace_memberships)
         + (SELECT COUNT(*) FROM auth_ceremonies)
         + (SELECT COUNT(*) FROM fasti_browser_session_authentication)
         + (SELECT COUNT(*) FROM access_audit_events)
"#;

fn verify_node_local_state_absent(transaction: &Transaction<'_>) -> Result<(), RestoreImportError> {
    let count: i64 = transaction
        .query_row(NODE_LOCAL_STATE_COUNT_SQL, [], |row| row.get(0))
        .map_err(RestoreImportError::Sqlite)?;
    if count != 0 {
        return Err(RestoreImportError::NodeLocalStatePresent);
    }
    Ok(())
}

fn verify_derived_metadata_state_absent(
    transaction: &Transaction<'_>,
) -> Result<(), RestoreImportError> {
    let count: i64 = transaction
        .query_row(
            r#"
            SELECT (SELECT COUNT(*) FROM metadata_projections)
                 + (SELECT COUNT(*) FROM metadata_cache_entries)
                 + (SELECT COUNT(*) FROM metadata_cache_claims)
                 + (SELECT COUNT(*) FROM search_pages)
                 + (SELECT COUNT(*) FROM search_candidate_receipts)
            "#,
            [],
            |row| row.get(0),
        )
        .map_err(RestoreImportError::Sqlite)?;
    if count != 0 {
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(())
}

fn verify_import_domain_invariants(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    correlation_id: RequestCorrelationId,
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
        return Err(RestoreImportError::AggregateInvariant);
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
        return Err(RestoreImportError::InterpretationChainInvariant);
    }

    let invalid_lifecycle: i64 = transaction
        .query_row(
            r#"
            WITH initial AS (
                SELECT claim_id, initial_status, fetched_at AS initial_at
                FROM metadata_claim_provenance
                WHERE workspace_id = ?1
                UNION ALL
                SELECT claim_id, initial_status, fetched_at AS initial_at
                FROM metadata_rating_claims
                WHERE workspace_id = ?1
            ), ordered AS (
                SELECT event.claim_id,
                       event.sequence,
                       event.previous_status,
                       event.status,
                       event.occurred_at,
                       initial.claim_id AS initial_claim_id,
                       initial.initial_status,
                       initial.initial_at,
                       ROW_NUMBER() OVER (
                           PARTITION BY event.claim_id ORDER BY event.sequence
                       ) AS expected_sequence,
                       LAG(event.status) OVER (
                           PARTITION BY event.claim_id ORDER BY event.sequence
                       ) AS prior_status,
                       LAG(event.occurred_at) OVER (
                           PARTITION BY event.claim_id ORDER BY event.sequence
                       ) AS prior_occurred_at
                FROM metadata_claim_lifecycle_events event
                LEFT JOIN initial ON initial.claim_id = event.claim_id
                WHERE event.workspace_id = ?1
            )
            SELECT COUNT(*)
            FROM ordered
            WHERE initial_claim_id IS NULL
               OR sequence <> expected_sequence
               OR previous_status <> COALESCE(prior_status, initial_status)
               OR occurred_at < COALESCE(prior_occurred_at, initial_at)
            "#,
            [workspace_id.to_string()],
            |row| row.get(0),
        )
        .map_err(RestoreImportError::Sqlite)?;
    if invalid_lifecycle != 0 {
        return Err(RestoreImportError::MetadataLifecycleInvariant);
    }
    crate::identity_routing::validate_workspace_identity_routing_state(
        transaction,
        workspace_id,
        correlation_id,
    )
    .map_err(|_| RestoreImportError::IdentityRoutingInvariant)?;

    crate::identity_routing::validate_workspace_anime_grouping_policy_receipts(
        transaction,
        workspace_id,
        correlation_id,
    )
    .map_err(|_| RestoreImportError::PolicyReceiptInvariant)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum RowKey {
    One(String),
    Two(String, String),
    Three(String, String, String),
    Four(String, String, String, String),
    TextInteger(String, u64),
}

fn import_row(
    transaction: &Transaction<'_>,
    entity: WorkspaceExportEntity,
    format_version: u32,
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
        WorkspaceExportEntity::MetadataFieldClaims => {
            let row: MetadataFieldClaimRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            let field_key = FieldKey::try_new(row.field_key.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            validate_timestamp(&row.fetched_at)?;
            validate_timestamp(&row.created_at)?;
            let expires_at = row
                .expires_at
                .as_deref()
                .map(|value| {
                    validate_timestamp(value)?;
                    parse_timestamp(value)
                })
                .transpose()?;
            FieldClaim::try_new(
                row.source.clone(),
                row.value.clone(),
                row.locale.clone(),
                ReceivedAt::from_application_clock(parse_timestamp(&row.fetched_at)?),
                expires_at,
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO metadata_field_claims(
                        workspace_id, record_id, field_key, source, value, locale,
                        fetched_at, expires_at, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    "#,
                    params![
                        row.workspace_id.to_string(),
                        row.record_id.to_string(),
                        field_key.as_str(),
                        row.source.as_str(),
                        row.value,
                        row.locale,
                        row.fetched_at,
                        row.expires_at,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::Four(
                row.record_id.to_string(),
                field_key.as_str().to_owned(),
                row.source.as_str().to_owned(),
                row.fetched_at,
            ))
        }
        WorkspaceExportEntity::MetadataFieldOverrides => {
            let row: MetadataFieldOverrideRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            let field_key = FieldKey::try_new(row.field_key.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            validate_timestamp(&row.created_at)?;
            FieldOverride::try_new(
                row.value.clone(),
                ReceivedAt::from_application_clock(parse_timestamp(&row.created_at)?),
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO metadata_field_overrides(
                        workspace_id, record_id, field_key, value, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                    params![
                        row.workspace_id.to_string(),
                        row.record_id.to_string(),
                        field_key.as_str(),
                        row.value,
                        row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::Two(
                row.record_id.to_string(),
                field_key.as_str().to_owned(),
            ))
        }
        WorkspaceExportEntity::ProfileRecordTrackingDispositions => {
            let row: ProfileRecordTrackingDispositionRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.updated_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"
                    INSERT INTO profile_record_tracking_dispositions(
                        workspace_id, profile_id, record_id, disposition, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                    params![
                        row.workspace_id.to_string(),
                        row.profile_id.to_string(),
                        row.record_id.to_string(),
                        row.disposition.as_str(),
                        row.updated_at,
                    ],
                ),
            )?;
            Ok(RowKey::Two(
                row.profile_id.to_string(),
                row.record_id.to_string(),
            ))
        }
        WorkspaceExportEntity::MetadataClaims => {
            let row = if format_version <= WORKSPACE_ARCHIVE_V6_FORMAT_VERSION {
                let legacy: MetadataClaimRow = decode_row(line, path)?;
                MetadataClaimV7Row {
                    claim_id: legacy.claim_id,
                    claim_kind: legacy.claim_kind,
                    created_at: legacy.created_at,
                    record_id: legacy.record_id,
                    response_policy_json: None,
                    workspace_id: legacy.workspace_id,
                }
            } else {
                decode_metadata_claim_v7(line, path)?
            };
            require_workspace(row.workspace_id, workspace_id)?;
            if !matches!(row.claim_kind.as_str(), "field" | "rating") {
                return Err(RestoreImportError::DomainInvariant);
            }
            validate_timestamp(&row.created_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO metadata_claims(claim_id, workspace_id, record_id, claim_kind, created_at, response_policy_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![row.claim_id.to_string(), row.workspace_id.to_string(), row.record_id.to_string(), row.claim_kind, row.created_at, row.response_policy_json],
                ),
            )?;
            Ok(RowKey::One(row.claim_id.to_string()))
        }
        WorkspaceExportEntity::MetadataClaimProvenance => {
            let row: MetadataClaimProvenanceRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            FieldKey::try_new(row.field_key.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            validate_timestamp(&row.fetched_at)?;
            validate_timestamp(&row.created_at)?;
            validate_classification(&row.classification)?;
            let complete = row.provenance_state == "complete"
                && row.provider_id.is_some()
                && row.source_record_id.is_some()
                && row.evidence_digest.is_some();
            let legacy = row.provenance_state == "legacy_incomplete"
                && row.provider_id.is_none()
                && row.source_record_id.is_none()
                && row.evidence_digest.is_none();
            if !(complete || legacy) || parse_claim_status(&row.initial_status).is_none() {
                return Err(RestoreImportError::DomainInvariant);
            }
            if let Some(provider_id) = &row.provider_id {
                MetadataProviderId::try_new(provider_id.clone())
                    .map_err(|_| RestoreImportError::DomainInvariant)?;
            }
            if let Some(region) = &row.region {
                MetadataRegion::try_new(region.clone())
                    .map_err(|_| RestoreImportError::DomainInvariant)?;
            }
            validate_optional_bounded(&row.source_record_id, 512)?;
            validate_optional_bounded(&row.source_version, 128)?;
            validate_optional_bounded(&row.terms_revision, 128)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"INSERT INTO metadata_claim_provenance(
                        claim_id, workspace_id, record_id, field_key, source, fetched_at,
                        provider_id, source_record_id, region, source_version, evidence_digest,
                        classification, terms_revision, provenance_state, initial_status, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
                    params![
                        row.claim_id.to_string(), row.workspace_id.to_string(), row.record_id.to_string(),
                        row.field_key, row.source.as_str(), row.fetched_at, row.provider_id,
                        row.source_record_id, row.region, row.source_version,
                        row.evidence_digest.map(|value| value.to_string()), row.classification,
                        row.terms_revision, row.provenance_state, row.initial_status, row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.claim_id.to_string()))
        }
        WorkspaceExportEntity::MetadataRatingClaims => {
            let row: MetadataRatingClaimRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.fetched_at)?;
            validate_timestamp(&row.created_at)?;
            let fetched_at = parse_timestamp(&row.fetched_at)?;
            let expires_at = row.expires_at.as_deref().map(parse_timestamp).transpose()?;
            if let Some(value) = row.expires_at.as_deref() {
                validate_timestamp(value)?;
            }
            validate_classification(&row.classification)?;
            validate_optional_bounded(&row.terms_revision, 128)?;
            let provider_id = MetadataProviderId::try_new(row.provider_id.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let locale = row
                .locale
                .clone()
                .map(MetadataLocale::try_new)
                .transpose()
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let region = row
                .region
                .clone()
                .map(MetadataRegion::try_new)
                .transpose()
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let provenance = FieldClaimProvenance::try_new(
                provider_id,
                row.source.clone(),
                row.source_record_id.clone(),
                locale,
                region,
                row.source_version.clone(),
                row.evidence_digest.clone(),
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            let value =
                u32::try_from(row.value_millis).map_err(|_| RestoreImportError::DomainInvariant)?;
            let minimum = u32::try_from(row.scale_minimum_millis)
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let maximum = u32::try_from(row.scale_maximum_millis)
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let status = parse_claim_status(&row.initial_status)
                .ok_or(RestoreImportError::DomainInvariant)?;
            RatingClaim::try_new(
                row.claim_id,
                row.record_id,
                value,
                RatingScale::try_new(minimum, maximum)
                    .map_err(|_| RestoreImportError::DomainInvariant)?,
                provenance,
                ReceivedAt::from_application_clock(fetched_at),
                expires_at,
                status,
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"INSERT INTO metadata_rating_claims(
                        claim_id, workspace_id, record_id, value_millis, scale_minimum_millis,
                        scale_maximum_millis, provider_id, source, source_record_id, locale, region,
                        source_version, evidence_digest, classification, terms_revision, fetched_at,
                        expires_at, initial_status, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)"#,
                    params![
                        row.claim_id.to_string(), row.workspace_id.to_string(), row.record_id.to_string(),
                        value, minimum, maximum, row.provider_id, row.source.as_str(),
                        row.source_record_id, row.locale, row.region, row.source_version,
                        row.evidence_digest.to_string(), row.classification, row.terms_revision,
                        row.fetched_at, row.expires_at, row.initial_status, row.created_at,
                    ],
                ),
            )?;
            Ok(RowKey::One(row.claim_id.to_string()))
        }
        WorkspaceExportEntity::MetadataClaimLifecycleEvents => {
            let row: MetadataClaimLifecycleEventRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.occurred_at)?;
            let sequence =
                u32::try_from(row.sequence).map_err(|_| RestoreImportError::DomainInvariant)?;
            let previous_status = parse_claim_status(&row.previous_status)
                .ok_or(RestoreImportError::DomainInvariant)?;
            let status =
                parse_claim_status(&row.status).ok_or(RestoreImportError::DomainInvariant)?;
            FieldClaimLifecycleEvent::try_new(
                row.claim_id,
                sequence,
                previous_status,
                status,
                ReceivedAt::from_application_clock(parse_timestamp(&row.occurred_at)?),
                row.evidence_digest.clone(),
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO metadata_claim_lifecycle_events(claim_id, sequence, workspace_id, previous_status, status, occurred_at, evidence_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![row.claim_id.to_string(), sequence, row.workspace_id.to_string(), row.previous_status, row.status, row.occurred_at, row.evidence_digest.map(|value| value.to_string())],
                ),
            )?;
            Ok(RowKey::TextInteger(
                row.claim_id.to_string(),
                u64::from(sequence),
            ))
        }
        WorkspaceExportEntity::MetadataProjectionPolicies => {
            let row: MetadataProjectionPolicyRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.updated_at)?;
            let preferred_provider = row
                .preferred_provider_id
                .clone()
                .map(MetadataProviderId::try_new)
                .transpose()
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let preferred_locale = row
                .preferred_locale
                .clone()
                .map(MetadataLocale::try_new)
                .transpose()
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let original_locale = row
                .original_locale
                .clone()
                .map(MetadataLocale::try_new)
                .transpose()
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            if let Some(region) = &row.region {
                MetadataRegion::try_new(region.clone())
                    .map_err(|_| RestoreImportError::DomainInvariant)?;
            }
            validate_field_groups(&row.enabled_field_groups)?;
            let allow_english = match row.allow_english_fallback {
                0 => false,
                1 => true,
                _ => return Err(RestoreImportError::DomainInvariant),
            };
            let last_known_good = match row.last_known_good_policy.as_str() {
                "allow" => LastKnownGoodPolicy::Allow,
                "deny" => LastKnownGoodPolicy::Deny,
                _ => return Err(RestoreImportError::DomainInvariant),
            };
            MetadataProjectionPolicy::new(
                row.profile_id,
                preferred_provider,
                preferred_locale,
                original_locale,
                allow_english,
                last_known_good,
            );
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    r#"INSERT INTO metadata_projection_policies(
                        workspace_id, profile_id, preferred_provider_id, preferred_locale,
                        original_locale, region, enabled_field_groups, allow_english_fallback,
                        last_known_good_policy, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
                    params![
                        row.workspace_id.to_string(),
                        row.profile_id.to_string(),
                        row.preferred_provider_id,
                        row.preferred_locale,
                        row.original_locale,
                        row.region,
                        row.enabled_field_groups,
                        i64::try_from(row.allow_english_fallback)
                            .map_err(|_| RestoreImportError::DomainInvariant)?,
                        row.last_known_good_policy,
                        row.updated_at
                    ],
                ),
            )?;
            Ok(RowKey::One(row.profile_id.to_string()))
        }
        WorkspaceExportEntity::MetadataProfileFieldOverrides => {
            let row: MetadataProfileFieldOverrideRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            validate_timestamp(&row.updated_at)?;
            if parse_timestamp(&row.updated_at)? < parse_timestamp(&row.created_at)?
                || !matches!(row.origin.as_str(), "user" | "legacy_migration")
            {
                return Err(RestoreImportError::DomainInvariant);
            }
            let field_key = FieldKey::try_new(row.field_key.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            ProfileFieldOverride::try_new(
                row.profile_id,
                row.record_id,
                field_key,
                row.value.clone(),
                ReceivedAt::from_application_clock(parse_timestamp(&row.created_at)?),
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO metadata_profile_field_overrides(workspace_id, profile_id, record_id, field_key, value, created_at, updated_at, origin) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![row.workspace_id.to_string(), row.profile_id.to_string(), row.record_id.to_string(), row.field_key, row.value, row.created_at, row.updated_at, row.origin],
                ),
            )?;
            Ok(RowKey::Three(
                row.profile_id.to_string(),
                row.record_id.to_string(),
                row.field_key,
            ))
        }
        WorkspaceExportEntity::MetadataLegacyOverrideOwnership => {
            let row: MetadataLegacyOverrideOwnershipRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            FieldKey::try_new(row.field_key.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            validate_timestamp(&row.recorded_at)?;
            let valid = matches!(
                (
                    row.state.as_str(),
                    row.owner_profile_id,
                    row.review_reason.as_deref()
                ),
                ("migrated", Some(_), None)
                    | (
                        "review_required",
                        None,
                        Some("zero_profiles" | "multiple_profiles")
                    )
            );
            if !valid {
                return Err(RestoreImportError::DomainInvariant);
            }
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO metadata_legacy_override_ownership(workspace_id, record_id, field_key, owner_profile_id, state, review_reason, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![row.workspace_id.to_string(), row.record_id.to_string(), row.field_key, row.owner_profile_id.map(|value| value.to_string()), row.state, row.review_reason, row.recorded_at],
                ),
            )?;
            Ok(RowKey::Two(row.record_id.to_string(), row.field_key))
        }
        WorkspaceExportEntity::MetadataOverrideMigrationReceipts => {
            let row: MetadataOverrideMigrationReceiptRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            if row.receipt_id.is_empty() || row.receipt_id.len() > 512 {
                return Err(RestoreImportError::DomainInvariant);
            }
            FieldKey::try_new(row.field_key.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            validate_timestamp(&row.source_created_at)?;
            validate_timestamp(&row.migrated_at)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO metadata_override_migration_receipts(receipt_id, workspace_id, record_id, field_key, profile_id, source_created_at, migrated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![row.receipt_id, row.workspace_id.to_string(), row.record_id.to_string(), row.field_key, row.profile_id.to_string(), row.source_created_at, row.migrated_at],
                ),
            )?;
            Ok(RowKey::One(row.receipt_id))
        }
        WorkspaceExportEntity::MetadataAttributions => {
            let row: MetadataAttributionRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.updated_at)?;
            MetadataAttribution::try_new(
                MetadataProviderId::try_new(row.provider_id.clone())
                    .map_err(|_| RestoreImportError::DomainInvariant)?,
                row.attribution_text.clone(),
                row.documentation_url.clone(),
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO metadata_attributions(workspace_id, provider_id, attribution_text, documentation_url, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![row.workspace_id.to_string(), row.provider_id, row.attribution_text, row.documentation_url, row.updated_at],
                ),
            )?;
            Ok(RowKey::One(row.provider_id))
        }
        WorkspaceExportEntity::MetadataRefreshReceipts => {
            let row: MetadataRefreshReceiptRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            let provider_id = MetadataProviderId::try_new(row.provider_id.clone())
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            crate::metadata::decode_refresh_receipt_outcome(
                &row.response_json,
                row.record_id,
                &provider_id,
                row.profile_id,
                CapabilityKey::RefreshMetadataClaims,
                RequestCorrelationId::new_v7(),
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO metadata_refresh_receipts(workspace_id, profile_id, client_id, operation_id, semantic_digest, record_id, provider_id, response_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![row.workspace_id.to_string(), row.profile_id.to_string(), row.client_id.to_string(), row.operation_id.to_string(), row.semantic_digest.to_string(), row.record_id.to_string(), row.provider_id, row.response_json, row.created_at],
                ),
            )?;
            Ok(RowKey::Two(
                row.client_id.to_string(),
                row.operation_id.to_string(),
            ))
        }
        WorkspaceExportEntity::IdentityAssertions => {
            let row: IdentityAssertionRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            validate_claimed_json::<serde_json::Value>(&row.coverage_json)?;
            validate_claimed_json::<serde_json::Value>(&row.episode_links_json)?;
            validate_claimed_json::<serde_json::Value>(&row.evidence_json)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO identity_assertions(assertion_id, workspace_id, record_id, source_external_identifier_id, target_namespace, target_grain, target_value, relation, coverage_json, episode_links_json, evidence_class, evidence_json, id_source, source_version, authority, reasoning, initial_status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                    params![row.assertion_id.to_string(), row.workspace_id.to_string(), row.record_id.to_string(), row.source_external_identifier_id.to_string(), row.target_namespace, row.target_grain, row.target_value, row.relation, row.coverage_json, row.episode_links_json, row.evidence_class, row.evidence_json, row.id_source, row.source_version, row.authority, row.reasoning, row.initial_status, row.created_at],
                ),
            )?;
            Ok(RowKey::One(row.assertion_id.to_string()))
        }
        WorkspaceExportEntity::IdentityAssertionLifecycleEvents => {
            let row: IdentityAssertionLifecycleEventRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.occurred_at)?;
            let sequence =
                i64::try_from(row.sequence).map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO identity_assertion_lifecycle_events(workspace_id, assertion_id, sequence, previous_status, status, reviewer_client_id, occurred_at, evidence_digest) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![row.workspace_id.to_string(), row.assertion_id.to_string(), sequence, row.previous_status, row.status, row.reviewer_client_id.to_string(), row.occurred_at, row.evidence_digest.map(|value| value.to_string())],
                ),
            )?;
            Ok(RowKey::TextInteger(
                row.assertion_id.to_string(),
                row.sequence,
            ))
        }
        WorkspaceExportEntity::ProfileAnimeGroupingPolicies => {
            let row: ProfileAnimeGroupingPolicyRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.updated_at)?;
            let revision =
                i64::try_from(row.revision).map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO profile_anime_grouping_policies(workspace_id, profile_id, preference, revision, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![row.workspace_id.to_string(), row.profile_id.to_string(), row.preference, revision, row.updated_at],
                ),
            )?;
            Ok(RowKey::One(row.profile_id.to_string()))
        }
        WorkspaceExportEntity::ClientAnimeGroupingPolicies => {
            let row: ClientAnimeGroupingPolicyRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.updated_at)?;
            let revision =
                i64::try_from(row.revision).map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO client_anime_grouping_policies(workspace_id, profile_id, client_id, preference, revision, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![row.workspace_id.to_string(), row.profile_id.to_string(), row.client_id.to_string(), row.preference, revision, row.updated_at],
                ),
            )?;
            Ok(RowKey::Two(
                row.profile_id.to_string(),
                row.client_id.to_string(),
            ))
        }
        WorkspaceExportEntity::AnimeGroupingPolicyReceipts => {
            let row: AnimeGroupingPolicyReceiptRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            validate_timestamp(&row.created_at)?;
            let result_revision = i64::try_from(row.result_revision)
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let affected_records = i64::try_from(row.affected_records)
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let unresolved_routes = i64::try_from(row.unresolved_routes)
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            let possible_season_regroupings = i64::try_from(row.possible_season_regroupings)
                .map_err(|_| RestoreImportError::DomainInvariant)?;
            insert_row(
                transaction,
                entity,
                transaction.execute(
                    "INSERT INTO anime_grouping_policy_receipts(workspace_id, profile_id, actor_client_id, scope_kind, scope_client_id, operation_id, semantic_digest, change_kind, requested_preference, rollback_operation_id, previous_preference, previous_source, result_preference, result_source, result_revision, affected_records, unresolved_routes, possible_season_regroupings, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                    params![row.workspace_id.to_string(), row.profile_id.to_string(), row.actor_client_id.to_string(), row.scope_kind, row.scope_client_id.map(|value| value.to_string()), row.operation_id.to_string(), row.semantic_digest.to_string(), row.change_kind, row.requested_preference, row.rollback_operation_id.map(|value| value.to_string()), row.previous_preference, row.previous_source, row.result_preference, row.result_source, result_revision, affected_records, unresolved_routes, possible_season_regroupings, row.created_at],
                ),
            )?;
            Ok(RowKey::Two(
                row.actor_client_id.to_string(),
                row.operation_id.to_string(),
            ))
        }
        WorkspaceExportEntity::SearchActionReceipts => {
            let row: SearchActionReceiptRow = decode_row(line, path)?;
            require_workspace(row.workspace_id, workspace_id)?;
            let receipt = crate::search_actions::decode_receipt(
                &row.receipt_json,
                RequestCorrelationId::new_v7(),
            )
            .map_err(|_| RestoreImportError::DomainInvariant)?;
            if receipt.workspace_id != row.workspace_id
                || receipt.operation_id != row.operation_id
                || receipt.profile_id != row.profile_id
                || receipt.actor_client_id != row.actor_client_id
                || receipt.actor_subject_id != row.actor_subject_id
                || receipt.record_id != row.record_id
                || receipt.semantic_digest() != row.semantic_digest
            {
                return Err(RestoreImportError::DomainInvariant);
            }
            let grain: String = transaction
                .query_row(
                    "SELECT grain FROM records WHERE workspace_id = ?1 AND record_id = ?2",
                    params![workspace_id.to_string(), row.record_id.to_string()],
                    |record| record.get(0),
                )
                .optional()
                .map_err(RestoreImportError::Sqlite)?
                .ok_or(RestoreImportError::DomainInvariant)?;
            if grain != receipt.grain.as_str() {
                return Err(RestoreImportError::DomainInvariant);
            }
            insert_row(transaction, entity, transaction.execute(
                "INSERT INTO search_action_receipts(workspace_id, operation_id, profile_id, actor_client_id, actor_subject_id, record_id, semantic_digest, receipt_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![row.workspace_id.to_string(), row.operation_id.to_string(), row.profile_id.to_string(), row.actor_client_id.to_string(), row.actor_subject_id.map(|value| value.to_string()), row.record_id.to_string(), row.semantic_digest.to_string(), row.receipt_json],
            ))?;
            Ok(RowKey::One(row.operation_id.to_string()))
        }
    }
}

pub(crate) fn validate_metadata_claim_legacy(line: &[u8]) -> bool {
    decode_row::<MetadataClaimRow>(line, "metadata_claims.ndjson").is_ok()
}

pub(crate) fn validate_metadata_claim_v7(line: &[u8]) -> bool {
    decode_metadata_claim_v7(line, "metadata_claims.ndjson").is_ok()
}

fn decode_metadata_claim_v7(
    line: &[u8],
    path: &str,
) -> Result<MetadataClaimV7Row, RestoreImportError> {
    let row: MetadataClaimV7Row = decode_row(line, path)?;
    if let Some(json) = &row.response_policy_json {
        let policy = fasti_application::ProviderResponseCachePolicy::from_canonical_json(json)
            .ok_or(RestoreImportError::DomainInvariant)?;
        if policy.reuse() == fasti_application::ProviderResponseReuse::NoStore {
            return Err(RestoreImportError::DomainInvariant);
        }
    }
    Ok(row)
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

fn parse_claim_status(value: &str) -> Option<FieldClaimStatus> {
    match value {
        "fresh" => Some(FieldClaimStatus::Fresh),
        "stale" => Some(FieldClaimStatus::Stale),
        "invalid" => Some(FieldClaimStatus::Invalid),
        "revoked" => Some(FieldClaimStatus::Revoked),
        "superseded" => Some(FieldClaimStatus::Superseded),
        "unavailable" => Some(FieldClaimStatus::Unavailable),
        _ => None,
    }
}

fn validate_classification(value: &str) -> Result<(), RestoreImportError> {
    if matches!(value, "public" | "internal" | "confidential" | "restricted") {
        Ok(())
    } else {
        Err(RestoreImportError::DomainInvariant)
    }
}

fn validate_optional_bounded(
    value: &Option<String>,
    max_bytes: usize,
) -> Result<(), RestoreImportError> {
    if value.as_ref().is_some_and(|value| {
        value.is_empty()
            || value.len() > max_bytes
            || value.trim() != value
            || value.chars().any(char::is_control)
    }) {
        Err(RestoreImportError::DomainInvariant)
    } else {
        Ok(())
    }
}

fn validate_field_groups(value: &str) -> Result<(), RestoreImportError> {
    let groups: Vec<String> =
        serde_json::from_str(value).map_err(|_| RestoreImportError::DomainInvariant)?;
    if serde_json::to_string(&groups).map_err(|_| RestoreImportError::DomainInvariant)? != value
        || groups.iter().any(|group| {
            !matches!(
                group.as_str(),
                "artwork"
                    | "basic_info"
                    | "details"
                    | "release_dates"
                    | "credits"
                    | "production_companies"
                    | "networks"
                    | "episodes"
                    | "season_artwork"
                    | "recommendations"
                    | "collections"
                    | "trailers"
                    | "watch_providers"
            )
        })
    {
        return Err(RestoreImportError::DomainInvariant);
    }
    let mut canonical = groups.clone();
    canonical.sort_unstable();
    canonical.dedup();
    if canonical != groups {
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
archive_row!(MetadataFieldClaimRow {
    created_at: String,
    expires_at: Option<String>,
    fetched_at: String,
    field_key: String,
    locale: Option<String>,
    record_id: RecordId,
    source: NamespaceKey,
    value: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataFieldOverrideRow {
    created_at: String,
    field_key: String,
    record_id: RecordId,
    value: String,
    workspace_id: WorkspaceId,
});
archive_row!(ProfileRecordTrackingDispositionRow {
    disposition: TrackingDisposition,
    profile_id: ProfileId,
    record_id: RecordId,
    updated_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataClaimRow {
    claim_id: MetadataClaimId,
    claim_kind: String,
    created_at: String,
    record_id: RecordId,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataClaimV7Row {
    claim_id: MetadataClaimId,
    claim_kind: String,
    created_at: String,
    record_id: RecordId,
    response_policy_json: Option<String>,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataClaimProvenanceRow {
    claim_id: MetadataClaimId,
    classification: String,
    created_at: String,
    evidence_digest: Option<Sha256Digest>,
    fetched_at: String,
    field_key: String,
    initial_status: String,
    provenance_state: String,
    provider_id: Option<String>,
    record_id: RecordId,
    region: Option<String>,
    source: NamespaceKey,
    source_record_id: Option<String>,
    source_version: Option<String>,
    terms_revision: Option<String>,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataRatingClaimRow {
    claim_id: MetadataClaimId,
    classification: String,
    created_at: String,
    evidence_digest: Sha256Digest,
    expires_at: Option<String>,
    fetched_at: String,
    initial_status: String,
    locale: Option<String>,
    provider_id: String,
    record_id: RecordId,
    region: Option<String>,
    scale_maximum_millis: u64,
    scale_minimum_millis: u64,
    source: NamespaceKey,
    source_record_id: String,
    source_version: Option<String>,
    terms_revision: Option<String>,
    value_millis: u64,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataClaimLifecycleEventRow {
    claim_id: MetadataClaimId,
    evidence_digest: Option<Sha256Digest>,
    occurred_at: String,
    previous_status: String,
    sequence: u64,
    status: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataProjectionPolicyRow {
    allow_english_fallback: u64,
    enabled_field_groups: String,
    last_known_good_policy: String,
    original_locale: Option<String>,
    preferred_locale: Option<String>,
    preferred_provider_id: Option<String>,
    profile_id: ProfileId,
    region: Option<String>,
    updated_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataProfileFieldOverrideRow {
    created_at: String,
    field_key: String,
    origin: String,
    profile_id: ProfileId,
    record_id: RecordId,
    updated_at: String,
    value: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataLegacyOverrideOwnershipRow {
    field_key: String,
    owner_profile_id: Option<ProfileId>,
    recorded_at: String,
    record_id: RecordId,
    review_reason: Option<String>,
    state: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataOverrideMigrationReceiptRow {
    field_key: String,
    migrated_at: String,
    profile_id: ProfileId,
    receipt_id: String,
    record_id: RecordId,
    source_created_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataAttributionRow {
    attribution_text: String,
    documentation_url: String,
    provider_id: String,
    updated_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(MetadataRefreshReceiptRow {
    client_id: ClientId,
    created_at: String,
    operation_id: OperationId,
    profile_id: ProfileId,
    provider_id: String,
    record_id: RecordId,
    response_json: String,
    semantic_digest: Sha256Digest,
    workspace_id: WorkspaceId,
});
archive_row!(IdentityAssertionRow {
    assertion_id: IdentityAssertionId,
    authority: Option<String>,
    coverage_json: String,
    created_at: String,
    episode_links_json: String,
    evidence_class: String,
    evidence_json: String,
    id_source: String,
    initial_status: String,
    reasoning: Option<String>,
    record_id: RecordId,
    relation: String,
    source_external_identifier_id: ExternalIdentifierId,
    source_version: Option<String>,
    target_grain: String,
    target_namespace: String,
    target_value: String,
    workspace_id: WorkspaceId,
});
archive_row!(IdentityAssertionLifecycleEventRow {
    assertion_id: IdentityAssertionId,
    evidence_digest: Option<Sha256Digest>,
    occurred_at: String,
    previous_status: String,
    reviewer_client_id: ClientId,
    sequence: u64,
    status: String,
    workspace_id: WorkspaceId,
});
archive_row!(ProfileAnimeGroupingPolicyRow {
    preference: String,
    profile_id: ProfileId,
    revision: u64,
    updated_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(ClientAnimeGroupingPolicyRow {
    client_id: ClientId,
    preference: Option<String>,
    profile_id: ProfileId,
    revision: u64,
    updated_at: String,
    workspace_id: WorkspaceId,
});
archive_row!(AnimeGroupingPolicyReceiptRow {
    actor_client_id: ClientId,
    affected_records: u64,
    change_kind: String,
    created_at: String,
    operation_id: OperationId,
    possible_season_regroupings: u64,
    previous_preference: String,
    previous_source: String,
    profile_id: ProfileId,
    requested_preference: Option<String>,
    result_preference: String,
    result_revision: u64,
    result_source: String,
    rollback_operation_id: Option<OperationId>,
    scope_client_id: Option<ClientId>,
    scope_kind: String,
    semantic_digest: Sha256Digest,
    unresolved_routes: u64,
    workspace_id: WorkspaceId,
});

archive_row!(SearchActionReceiptRow {
    actor_client_id: ClientId,
    actor_subject_id: Option<AuthSubjectId>,
    operation_id: OperationId,
    profile_id: ProfileId,
    receipt_json: String,
    record_id: RecordId,
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
pub(crate) fn reject_interrupted_restore(
    data_root: &File,
    max_entries: u64,
) -> Result<(), RestoreImportError> {
    let staging = open_private_directory(data_root, RESTORE_STAGING_DIRECTORY)?;
    let attempt_name = only_child_name(&staging)?;
    attempt_name
        .parse::<RestoreAttemptId>()
        .map_err(|_| RestoreImportError::DomainInvariant)?;
    let attempt = open_private_directory(&staging, &attempt_name)?;

    discard_pending_restore_phase(&attempt, RestoreStatus::Complete)?;
    discard_pending_restore_phase(&attempt, RestoreStatus::Rejected)?;
    match require_restore_phase(&attempt, RestoreStatus::Rejected) {
        Ok(()) => {}
        Err(RestoreActivationError::Archive(ArchiveError::Io(error)))
            if error.kind() == io::ErrorKind::NotFound =>
        {
            write_restore_phase(&attempt, RestoreStatus::Rejected)?;
        }
        Err(error) => return Err(error.into()),
    }

    cleanup_interrupted_blobs(&attempt, max_entries)?;
    cleanup_attempt(&staging, &attempt, &attempt_name, &[], &BTreeSet::new())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn reject_interrupted_restore(
    _data_root: &File,
    _max_entries: u64,
) -> Result<(), RestoreImportError> {
    Err(RestoreImportError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn cleanup_interrupted_blobs(attempt: &File, max_entries: u64) -> Result<(), RestoreImportError> {
    let Some(payloads) = open_optional_cleanup_directory(attempt, "payloads")? else {
        return Ok(());
    };
    let Some(sha256) = open_optional_cleanup_directory(&payloads, "sha256")? else {
        return Ok(());
    };
    let mut digest_count = 0_u64;
    let mut prefix_count = 0_u16;
    for entry in descriptor_read_dir(&sha256)? {
        prefix_count = prefix_count
            .checked_add(1)
            .filter(|count| *count <= 256)
            .ok_or(RestoreImportError::DomainInvariant)?;
        let prefix_name = child_name(entry)?;
        if prefix_name.len() != 2
            || !prefix_name
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(RestoreImportError::DomainInvariant);
        }
        let prefix = open_private_directory(&sha256, &prefix_name)?;
        for entry in descriptor_read_dir(&prefix)? {
            digest_count = digest_count
                .checked_add(1)
                .filter(|count| *count <= max_entries)
                .ok_or(RestoreImportError::DomainInvariant)?;
            let digest = child_name(entry)?;
            if digest.len() != 64
                || !digest.starts_with(&prefix_name)
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(RestoreImportError::DomainInvariant);
            }
            open_existing_file_beneath(&prefix, Path::new(&digest))?;
            rustix::fs::unlinkat(&prefix, digest, rustix::fs::AtFlags::empty()).map_err(
                |error| {
                    RestoreImportError::Archive(ArchiveError::Io(io::Error::from_raw_os_error(
                        error.raw_os_error(),
                    )))
                },
            )?;
        }
        rustix::fs::unlinkat(&sha256, prefix_name, rustix::fs::AtFlags::REMOVEDIR).map_err(
            |error| {
                RestoreImportError::Archive(ArchiveError::Io(io::Error::from_raw_os_error(
                    error.raw_os_error(),
                )))
            },
        )?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn descriptor_read_dir(directory: &File) -> Result<std::fs::ReadDir, RestoreImportError> {
    let path = format!("/proc/self/fd/{}", directory.as_raw_fd());
    std::fs::read_dir(path).map_err(|error| RestoreImportError::Archive(ArchiveError::Io(error)))
}

#[cfg(target_os = "linux")]
fn child_name(entry: io::Result<std::fs::DirEntry>) -> Result<String, RestoreImportError> {
    entry
        .map_err(|error| RestoreImportError::Archive(ArchiveError::Io(error)))?
        .file_name()
        .into_string()
        .map_err(|_| RestoreImportError::DomainInvariant)
}

#[cfg(target_os = "linux")]
fn only_child_name(directory: &File) -> Result<String, RestoreImportError> {
    let mut entries = descriptor_read_dir(directory)?;
    let name = child_name(entries.next().ok_or(RestoreImportError::DomainInvariant)?)?;
    if let Some(entry) = entries.next() {
        child_name(entry)?;
        return Err(RestoreImportError::DomainInvariant);
    }
    Ok(name)
}

#[cfg(target_os = "linux")]
fn open_optional_cleanup_directory(
    parent: &File,
    name: &str,
) -> Result<Option<File>, RestoreImportError> {
    match open_private_directory(parent, name) {
        Ok(directory) => Ok(Some(directory)),
        Err(ArchiveError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
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
    include!("metadata_policy_archive_tests.rs");
    include!("search_action_archive_tests.rs");
    use super::*;
    use crate::archive::ArchiveWriter;
    use crate::kernel::scope_storage_key;
    use crate::online_archive::export_online_workspace_archive;
    use crate::test_support::TestNode;
    use crate::StoreOpenError;
    use fasti_application::{
        AccessAdministrationPort, AnimeGroupingPolicyChange, AnimeGroupingPolicyScope,
        AppendCorrectionCommand, ApplyAnimeGroupingPolicyChangeCommand,
        AuthenticateCredentialQuery, CancellationSignal, CompleteRecoveryBootstrapRequest,
        CorrectionPort, CorrectionTarget, CreateRecordCommand, ExportWorkspaceQuery,
        ExportWorkspaceRequest, IdentityPort, IdentityRoutingPort, ObservationAcceptancePort,
        PrepareRecoveryBootstrapRequest, ProfileRecordStatePort, RecoveryBootstrapPort,
        RefreshMetadataClaimsOutcome, RegisterNamespaceDefinitionCommand, ResolveReviewCommand,
        RestoreWorkspaceRequest, ReviewPort, ReviewResolutionTarget, ScopeKey, SecretMaterial,
        SetTrackingDispositionCommand, VerifyWorkspaceQuery, WorkspaceArchiveDestination,
        WorkspaceManifest, WorkspaceRestorePort, WorkspaceStreamDescriptor,
        WorkspaceVerificationPort,
    };
    use fasti_contracts::CanonicalWorkspaceManifestProjection;
    use fasti_domain::{
        AnimeGroupingPreference, ClaimedTrust, EnrichmentPolicy, ExternalIdentifierClaim,
        MetadataFieldGroup, ObservedAt,
    };
    use std::io::Cursor;
    use std::num::NonZeroU64;
    use std::os::unix::process::ExitStatusExt as _;
    use std::process::Command;
    use std::sync::{Arc, Mutex};

    const CRASH_POINT_ENV: &str = "FASTI_TEST_RESTORE_CRASH_POINT";
    const CRASH_ROOT_ENV: &str = "FASTI_TEST_RESTORE_CRASH_ROOT";
    const CRASH_ATTEMPT_ENV: &str = "FASTI_TEST_RESTORE_CRASH_ATTEMPT";
    const CRASH_ARCHIVE_ENV: &str = "FASTI_TEST_RESTORE_CRASH_ARCHIVE";

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
        ) -> Result<(), fasti_application::WorkspaceArchiveCompletionError> {
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
        source_credential_hex: String,
    }

    fn full_fixture() -> FullFixture {
        let mut node = TestNode::new();
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
        let metadata_time = ReceivedAt::from_application_clock(
            DateTime::parse_from_rfc3339("2026-08-24T11:00:00Z")
                .expect("metadata time")
                .with_timezone(&Utc),
        );
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            crate::metadata::write_field_claim(
                &connection,
                node.access.workspace_id(),
                first,
                &FieldKey::try_new("core.title").expect("field key"),
                &FieldClaim::try_new(
                    NamespaceKey::try_new("fixture").expect("metadata source"),
                    "Provider title",
                    Some("en".to_owned()),
                    metadata_time,
                    None,
                )
                .expect("field claim"),
                CapabilityKey::ExportWorkspace,
                RequestCorrelationId::new_v7(),
            )
            .expect("persist field claim");
            crate::metadata::write_field_override(
                &connection,
                node.access.workspace_id(),
                first,
                &FieldKey::try_new("core.title").expect("field key"),
                &FieldOverride::try_new("Preferred title", metadata_time).expect("field override"),
                CapabilityKey::ExportWorkspace,
                RequestCorrelationId::new_v7(),
            )
            .expect("persist field override");
        }
        node.kernel
            .set_tracking_disposition(SetTrackingDispositionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                first,
                Some(TrackingDisposition::Watching),
            ))
            .expect("persist tracking disposition");
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
        let rotated = node
            .kernel
            .rotate_credential(fasti_application::RotateCredentialCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("rotate source credential");
        let source_credential_hex = rotated.credential().expose_hex();
        node.access = *rotated.access();
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
            source_credential_hex,
        }
    }

    struct IdentityRoutingArchiveFixture {
        archive: Vec<u8>,
        assertion_id: IdentityAssertionId,
        profile_id: ProfileId,
        client_id: ClientId,
        profile_operation_id: OperationId,
    }

    fn identity_routing_archive_fixture() -> IdentityRoutingArchiveFixture {
        let FullFixture { node, .. } = full_fixture();
        let workspace_id = node.access.workspace_id();
        let profile_id = node.access.profile_id();
        let client_id = node.access.client_id();
        let assertion_id = IdentityAssertionId::new_v7();
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            let (record_id, external_identifier_id): (String, String) = connection
                .query_row(
                    "SELECT record_id, external_identifier_id FROM external_identifiers WHERE workspace_id = ?1 AND value = 'a'",
                    [workspace_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("identity source");
            let evidence_json = serde_json::json!([{
                "method": "human_verified",
                "observed_source": "restore fixture",
                "derivation_root": "fixture-root",
                "reviewer": "fixture-reviewer",
                "observed_at": "2026-08-30",
                "evidence_id": null,
            }])
            .to_string();
            connection
                .execute(
                    r#"
                    INSERT INTO identity_assertions(
                        assertion_id, workspace_id, record_id, source_external_identifier_id,
                        target_namespace, target_grain, target_value, relation, coverage_json,
                        episode_links_json, evidence_class, evidence_json, id_source,
                        source_version, authority, reasoning, initial_status, created_at
                    ) VALUES (
                        ?1, ?2, ?3, ?4, 'fixture', 'release', 'b', 'exact', '[]', '[]',
                        'verified', ?5, 'restore-fixture', 'v1', NULL, NULL, 'candidate', ?6
                    )
                    "#,
                    params![
                        assertion_id.to_string(),
                        workspace_id.to_string(),
                        record_id,
                        external_identifier_id,
                        evidence_json,
                        "2026-08-31T12:00:00.000000Z",
                    ],
                )
                .expect("identity assertion");
            connection
                .execute(
                    "INSERT INTO identity_assertion_lifecycle_events(workspace_id, assertion_id, sequence, previous_status, status, reviewer_client_id, occurred_at, evidence_digest) VALUES (?1, ?2, 1, 'candidate', 'accepted', ?3, ?4, NULL)",
                    params![
                        workspace_id.to_string(),
                        assertion_id.to_string(),
                        client_id.to_string(),
                        "2026-08-31T12:00:01.000000Z",
                    ],
                )
                .expect("identity lifecycle event");
        }

        let profile_operation_id = OperationId::new_v7();
        node.kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Profile,
                    profile_operation_id,
                    Sha256Digest::parse(format!("sha256:{}", "1a".repeat(32)))
                        .expect("profile semantic digest"),
                    0,
                    AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::GroupByTvWork),
                )
                .expect("profile policy command"),
            )
            .expect("profile policy");
        node.kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Client(client_id),
                    OperationId::new_v7(),
                    Sha256Digest::parse(format!("sha256:{}", "2b".repeat(32)))
                        .expect("client semantic digest"),
                    1,
                    AnimeGroupingPolicyChange::Set(
                        AnimeGroupingPreference::KeepMalReleasesSeparate,
                    ),
                )
                .expect("client policy command"),
            )
            .expect("client policy");
        node.kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Client(client_id),
                    OperationId::new_v7(),
                    Sha256Digest::parse(format!("sha256:{}", "3c".repeat(32)))
                        .expect("inherit semantic digest"),
                    2,
                    AnimeGroupingPolicyChange::InheritProfile,
                )
                .expect("inherit policy command"),
            )
            .expect("inherit profile policy");
        node.kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Profile,
                    OperationId::new_v7(),
                    Sha256Digest::parse(format!("sha256:{}", "4d".repeat(32)))
                        .expect("advanced profile semantic digest"),
                    1,
                    AnimeGroupingPolicyChange::Set(
                        AnimeGroupingPreference::KeepKitsuReleasesSeparate,
                    ),
                )
                .expect("advanced profile policy command"),
            )
            .expect("profile policy after inherited client revision");
        node.kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Profile,
                    OperationId::new_v7(),
                    Sha256Digest::parse(format!("sha256:{}", "5e".repeat(32)))
                        .expect("rollback semantic digest"),
                    4,
                    AnimeGroupingPolicyChange::Rollback {
                        applied_operation_id: profile_operation_id,
                    },
                )
                .expect("rollback policy command"),
            )
            .expect("rollback profile policy");

        crate::identity_routing::validate_workspace_identity_routing_state(
            &node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection"),
            workspace_id,
            RequestCorrelationId::new_v7(),
        )
        .expect("valid source identity-routing state");

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
        .expect("export identity-routing fixture");
        let archive = destination.lock().expect("destination state").bytes.clone();
        IdentityRoutingArchiveFixture {
            archive,
            assertion_id,
            profile_id,
            client_id,
            profile_operation_id,
        }
    }

    struct MetadataV3Fixture {
        archive: Vec<u8>,
        field_claim_id: MetadataClaimId,
        rating_claim_id: MetadataClaimId,
        first_profile_id: ProfileId,
        second_profile_id: ProfileId,
        record_id: RecordId,
        operation_id: OperationId,
    }

    fn metadata_v3_fixture() -> MetadataV3Fixture {
        let FullFixture { node, .. } = full_fixture();
        let workspace_id = node.access.workspace_id();
        let first_profile_id = node.access.profile_id();
        let second_profile_id = ProfileId::new_v7();
        let operation_id = OperationId::new_v7();
        let record_id = {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                    params![
                        second_profile_id.to_string(),
                        workspace_id.to_string(),
                        timestamp(Utc::now())
                    ],
                )
                .expect("second profile");
            connection
                .query_row(
                    "SELECT record_id FROM records WHERE workspace_id = ?1 ORDER BY record_id LIMIT 1",
                    [workspace_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .expect("fixture record")
                .parse::<RecordId>()
                .expect("record ID")
        };
        let field_key = FieldKey::try_new("core.title").expect("field key");
        let fetched_at = ReceivedAt::from_application_clock(
            DateTime::parse_from_rfc3339("2026-08-30T12:00:00Z")
                .expect("fetched time")
                .with_timezone(&Utc),
        );
        let evidence_digest =
            Sha256Digest::parse(format!("sha256:{}", "a".repeat(64))).expect("evidence digest");
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            NamespaceKey::try_new("tmdb.movie").expect("namespace"),
            "438631",
            Some(MetadataLocale::try_new("en-IE").expect("locale")),
            Some(MetadataRegion::try_new("IE").expect("region")),
            Some("v3".to_owned()),
            evidence_digest.clone(),
        )
        .expect("provenance");
        let field_claim_id = MetadataClaimId::new_v7();
        let field_claim = FieldClaim::try_new_provider(
            field_claim_id,
            record_id,
            field_key.clone(),
            "Archive title",
            provenance.clone(),
            fetched_at,
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("field claim");
        let rating_claim_id = MetadataClaimId::new_v7();
        let rating_claim = RatingClaim::try_new(
            rating_claim_id,
            record_id,
            8_400,
            RatingScale::try_new(0, 10_000).expect("rating scale"),
            provenance,
            fetched_at,
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("rating claim");
        let first_policy = EnrichmentPolicy::new(
            MetadataProjectionPolicy::new(
                first_profile_id,
                Some(MetadataProviderId::try_new("tmdb").expect("provider")),
                Some(MetadataLocale::try_new("en-IE").expect("locale")),
                None,
                true,
                LastKnownGoodPolicy::Allow,
            ),
            Some(MetadataRegion::try_new("IE").expect("region")),
            vec![MetadataFieldGroup::BasicInfo],
        );
        let second_policy = EnrichmentPolicy::new(
            MetadataProjectionPolicy::new(
                second_profile_id,
                None,
                Some(MetadataLocale::try_new("fr-FR").expect("locale")),
                None,
                false,
                LastKnownGoodPolicy::Deny,
            ),
            Some(MetadataRegion::try_new("FR").expect("region")),
            vec![MetadataFieldGroup::BasicInfo, MetadataFieldGroup::Artwork],
        );
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            crate::metadata::write_field_claim(
                &connection,
                workspace_id,
                record_id,
                &field_key,
                &field_claim,
                CapabilityKey::ExportWorkspace,
                RequestCorrelationId::new_v7(),
            )
            .expect("provider field claim");
            crate::metadata::write_rating_claim(
                &connection,
                workspace_id,
                &rating_claim,
                CapabilityKey::ExportWorkspace,
                RequestCorrelationId::new_v7(),
            )
            .expect("rating claim");
            connection
                .execute(
                    "INSERT INTO metadata_claim_lifecycle_events(claim_id, sequence, workspace_id, previous_status, status, occurred_at, evidence_digest) VALUES (?1, 1, ?2, 'fresh', 'stale', ?3, ?4)",
                    params![
                        field_claim_id.to_string(),
                        workspace_id.to_string(),
                        timestamp(Utc::now()),
                        evidence_digest.to_string()
                    ],
                )
                .expect("lifecycle event");
            for policy in [&first_policy, &second_policy] {
                crate::metadata::write_enrichment_policy(
                    &connection,
                    workspace_id,
                    policy,
                    CapabilityKey::ExportWorkspace,
                    RequestCorrelationId::new_v7(),
                )
                .expect("projection policy");
            }
            for override_ in [
                ProfileFieldOverride::try_new(
                    first_profile_id,
                    record_id,
                    field_key.clone(),
                    "First profile title",
                    fetched_at,
                )
                .expect("first override"),
                ProfileFieldOverride::try_new(
                    second_profile_id,
                    record_id,
                    field_key.clone(),
                    "Second profile title",
                    fetched_at,
                )
                .expect("second override"),
            ] {
                crate::metadata::write_profile_field_override(
                    &connection,
                    workspace_id,
                    &override_,
                    CapabilityKey::ExportWorkspace,
                    RequestCorrelationId::new_v7(),
                )
                .expect("profile override");
            }
            crate::metadata::write_metadata_attribution(
                &connection,
                workspace_id,
                &MetadataAttribution::try_new(
                    MetadataProviderId::try_new("tmdb").expect("provider"),
                    "Metadata supplied by TMDB",
                    "https://developer.themoviedb.org/",
                )
                .expect("attribution"),
                CapabilityKey::ExportWorkspace,
                RequestCorrelationId::new_v7(),
            )
            .expect("attribution");
            let provider_id = MetadataProviderId::try_new("tmdb").expect("provider");
            let response_json = crate::metadata::encode_refresh_receipt_outcome(
                record_id,
                &provider_id,
                &RefreshMetadataClaimsOutcome::new(
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
                CapabilityKey::ExportWorkspace,
                RequestCorrelationId::new_v7(),
            )
            .expect("receipt response");
            connection
                .execute(
                    "INSERT INTO metadata_refresh_receipts(workspace_id, profile_id, client_id, operation_id, semantic_digest, record_id, provider_id, response_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![workspace_id.to_string(), first_profile_id.to_string(), node.access.client_id().to_string(), operation_id.to_string(), evidence_digest.to_string(), record_id.to_string(), provider_id.as_str(), response_json, timestamp(Utc::now())],
                )
                .expect("metadata refresh receipt");
        }

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
        .expect("export metadata-v3 fixture");
        let archive = destination.lock().expect("destination state").bytes.clone();
        MetadataV3Fixture {
            archive,
            field_claim_id,
            rating_claim_id,
            first_profile_id,
            second_profile_id,
            record_id,
            operation_id,
        }
    }

    fn legacy_google_books_archive() -> Vec<u8> {
        let fixture = full_fixture();
        fixture
            .node
            .kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                fixture.node.access,
                NamespaceDefinition::try_new(
                    "google-books",
                    "google-books",
                    [Grain::Chapter],
                    ".+",
                    "identity",
                    NamespaceLicencePosture::IdentifiersOnly,
                )
                .expect("legacy Google Books namespace"),
            ))
            .expect("register legacy Google Books namespace");
        let record_id = fixture
            .node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                fixture.node.access,
                Grain::Chapter,
            ))
            .expect("legacy Google Books record")
            .record_id();
        fixture
            .node
            .kernel
            .attach_identifier(fasti_application::AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                fixture.node.access,
                record_id,
                ExternalIdentifierClaim::try_new("google-books", Grain::Chapter, "restore-book")
                    .expect("legacy Google Books identifier"),
            ))
            .expect("attach legacy Google Books identifier");
        {
            let connection = fixture
                .node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            crate::metadata::write_field_claim(
                &connection,
                fixture.node.access.workspace_id(),
                record_id,
                &FieldKey::try_new("core.title").expect("field key"),
                &FieldClaim::try_new(
                    NamespaceKey::try_new("google-books").expect("legacy metadata source"),
                    "Restored book",
                    None,
                    ReceivedAt::from_application_clock(
                        DateTime::parse_from_rfc3339("2026-08-24T11:30:00Z")
                            .expect("metadata time")
                            .with_timezone(&Utc),
                    ),
                    None,
                )
                .expect("legacy provider claim"),
                CapabilityKey::ExportWorkspace,
                RequestCorrelationId::new_v7(),
            )
            .expect("persist legacy provider claim");
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
        .expect("export legacy provider fixture");
        let archive = destination.lock().expect("destination state").bytes.clone();
        archive
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
        let digest_bytes: [u8; 32] = Sha256::digest(bytes).into();
        Sha256Digest::from_bytes(&digest_bytes)
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
        let rebuilt = WorkspaceManifest::try_new_for_format(
            manifest.format_version(),
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

    fn archive_v1_from_v2(archive: &[u8]) -> Vec<u8> {
        let mut entries = archive_entries(archive);
        let manifest_bytes = entries.pop().expect("manifest entry").1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&manifest_bytes, limits())
                .expect("verified fixture manifest");
        entries.retain(|(entry_path, _)| {
            WorkspaceExportEntity::ALL[WorkspaceExportEntity::V1.len()..]
                .iter()
                .all(|entity| entry_path != &format!("{}.ndjson", entity.as_str()))
        });

        let manifest = verified.manifest();
        let rebuilt = WorkspaceManifest::try_new_for_format(
            WORKSPACE_ARCHIVE_V1_FORMAT_VERSION,
            manifest.workspace_id(),
            manifest.workspace_revision(),
            manifest.contract_version().to_owned(),
            manifest.migration_version(),
            manifest.migration_digest().clone(),
            manifest.streams()[..WorkspaceExportEntity::V1.len()].to_vec(),
            manifest.blobs().to_vec(),
        )
        .expect("rebuilt archive-v1 application manifest");
        let projection = CanonicalWorkspaceManifestProjection::try_from_application(rebuilt)
            .expect("rebuilt archive-v1 wire manifest");
        entries.push((
            "manifest.json".to_owned(),
            projection.canonical_json_bytes().to_vec(),
        ));

        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                .expect("archive limits");
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).expect("archive writer");
        for (entry_path, bytes) in entries {
            writer
                .append(&entry_path, bytes.len() as u64, Cursor::new(bytes))
                .expect("append archive-v1 entry");
        }
        writer.finish().expect("finish archive-v1 fixture")
    }

    fn archive_v2_from_v3(archive: &[u8]) -> Vec<u8> {
        let mut entries = archive_entries(archive);
        let manifest_bytes = entries.pop().expect("manifest entry").1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&manifest_bytes, limits())
                .expect("verified fixture manifest");
        entries.retain(|(entry_path, _)| {
            WorkspaceExportEntity::ALL[WorkspaceExportEntity::V2.len()..]
                .iter()
                .all(|entity| entry_path != &format!("{}.ndjson", entity.as_str()))
        });
        let manifest = verified.manifest();
        let rebuilt = WorkspaceManifest::try_new_for_format(
            WORKSPACE_ARCHIVE_V2_FORMAT_VERSION,
            manifest.workspace_id(),
            manifest.workspace_revision(),
            manifest.contract_version().to_owned(),
            manifest.migration_version(),
            manifest.migration_digest().clone(),
            manifest.streams()[..WorkspaceExportEntity::V2.len()].to_vec(),
            manifest.blobs().to_vec(),
        )
        .expect("rebuilt archive-v2 application manifest");
        let projection = CanonicalWorkspaceManifestProjection::try_from_application(rebuilt)
            .expect("rebuilt archive-v2 wire manifest");
        entries.push((
            "manifest.json".to_owned(),
            projection.canonical_json_bytes().to_vec(),
        ));
        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                .expect("archive limits");
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).expect("archive writer");
        for (entry_path, bytes) in entries {
            writer
                .append(&entry_path, bytes.len() as u64, Cursor::new(bytes))
                .expect("append archive-v2 entry");
        }
        writer.finish().expect("finish archive-v2 fixture")
    }

    fn legacy_metadata_claims_fixture(archive: &[u8]) -> Vec<u8> {
        rewrite_stream(archive, WorkspaceExportEntity::MetadataClaims, |bytes| {
            let mut legacy = Vec::new();
            for line in bytes.split_inclusive(|byte| *byte == b'\n') {
                let mut row: serde_json::Value = serde_json::from_slice(line).unwrap();
                if let Some(policy) = row.as_object_mut().unwrap().remove("response_policy_json") {
                    assert!(
                        policy.is_null(),
                        "never strip actual policy from historical fixtures"
                    );
                }
                legacy.extend(serde_json::to_vec(&row).unwrap());
                legacy.push(b'\n');
            }
            *bytes = legacy;
        })
    }

    fn archive_v3_from_v4(archive: &[u8]) -> Vec<u8> {
        let mut entries = archive_entries(&legacy_metadata_claims_fixture(archive));
        let manifest_bytes = entries.pop().expect("manifest entry").1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&manifest_bytes, limits())
                .expect("verified fixture manifest");
        entries.retain(|(entry_path, _)| {
            WorkspaceExportEntity::ALL[WorkspaceExportEntity::V3.len()..]
                .iter()
                .all(|entity| entry_path != &format!("{}.ndjson", entity.as_str()))
        });
        let manifest = verified.manifest();
        let rebuilt = WorkspaceManifest::try_new_for_format(
            WORKSPACE_ARCHIVE_V3_FORMAT_VERSION,
            manifest.workspace_id(),
            manifest.workspace_revision(),
            manifest.contract_version().to_owned(),
            12,
            Sha256Digest::parse(
                "sha256:eea7d899b8c257b7bafa359a540bd25ba2cdc4d9ddb7f50ce0ec8f80e251cfb9",
            )
            .expect("published v12 schema digest"),
            manifest.streams()[..WorkspaceExportEntity::V3.len()].to_vec(),
            manifest.blobs().to_vec(),
        )
        .expect("rebuilt archive-v3 application manifest");
        let projection = CanonicalWorkspaceManifestProjection::try_from_application(rebuilt)
            .expect("rebuilt archive-v3 wire manifest");
        entries.push((
            "manifest.json".to_owned(),
            projection.canonical_json_bytes().to_vec(),
        ));
        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                .expect("archive limits");
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).expect("archive writer");
        for (entry_path, bytes) in entries {
            writer
                .append(&entry_path, bytes.len() as u64, Cursor::new(bytes))
                .expect("append archive-v3 entry");
        }
        writer.finish().expect("finish archive-v3 fixture")
    }

    fn archive_v4_from_v5(archive: &[u8]) -> Vec<u8> {
        let mut entries = archive_entries(&legacy_metadata_claims_fixture(archive));
        let manifest_bytes = entries.pop().expect("manifest entry").1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&manifest_bytes, limits())
                .expect("verified fixture manifest");
        entries.retain(|(entry_path, _)| {
            WorkspaceExportEntity::ALL[WorkspaceExportEntity::V4.len()..]
                .iter()
                .all(|entity| entry_path != &format!("{}.ndjson", entity.as_str()))
        });
        let manifest = verified.manifest();
        let rebuilt = WorkspaceManifest::try_new_for_format(
            WORKSPACE_ARCHIVE_V4_FORMAT_VERSION,
            manifest.workspace_id(),
            manifest.workspace_revision(),
            manifest.contract_version().to_owned(),
            13,
            Sha256Digest::parse(
                "sha256:e470f2e8ae2972aa05fecd5b39642b79ef739de89eda204c37bf1d3e48f892c3",
            )
            .expect("published v13 schema digest"),
            manifest.streams()[..WorkspaceExportEntity::V4.len()].to_vec(),
            manifest.blobs().to_vec(),
        )
        .expect("rebuilt archive-v4 application manifest");
        let projection = CanonicalWorkspaceManifestProjection::try_from_application(rebuilt)
            .expect("rebuilt archive-v4 wire manifest");
        entries.push((
            "manifest.json".to_owned(),
            projection.canonical_json_bytes().to_vec(),
        ));
        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                .expect("archive limits");
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).expect("archive writer");
        for (entry_path, bytes) in entries {
            writer
                .append(&entry_path, bytes.len() as u64, Cursor::new(bytes))
                .expect("append archive-v4 entry");
        }
        writer.finish().expect("finish archive-v4 fixture")
    }

    fn rewrite_manifest_schema(archive: &[u8], version: u32, digest: &str) -> Vec<u8> {
        let mut entries = archive_entries(archive);
        let manifest_bytes = entries.pop().expect("manifest entry").1;
        let verified =
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&manifest_bytes, limits())
                .expect("verified fixture manifest");
        let manifest = verified.manifest();
        let rebuilt = WorkspaceManifest::try_new_for_format(
            manifest.format_version(),
            manifest.workspace_id(),
            manifest.workspace_revision(),
            manifest.contract_version().to_owned(),
            version,
            Sha256Digest::parse(digest).expect("historical schema digest"),
            manifest.streams().to_vec(),
            manifest.blobs().to_vec(),
        )
        .expect("rebuilt historical manifest");
        let projection = CanonicalWorkspaceManifestProjection::try_from_application(rebuilt)
            .expect("rebuilt historical wire manifest");
        entries.push((
            "manifest.json".to_owned(),
            projection.canonical_json_bytes().to_vec(),
        ));
        let archive_limits =
            ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                .expect("archive limits");
        let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).expect("archive writer");
        for (entry_path, bytes) in entries {
            writer
                .append(&entry_path, bytes.len() as u64, Cursor::new(bytes))
                .expect("append historical archive entry");
        }
        writer.finish().expect("finish historical archive")
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
    #[ignore = "subprocess worker invoked by full_restore_sigkill_matrix"]
    fn full_restore_crash_worker() {
        let (Ok(root), Ok(attempt), Ok(archive)) = (
            std::env::var(CRASH_ROOT_ENV),
            std::env::var(CRASH_ATTEMPT_ENV),
            std::env::var(CRASH_ARCHIVE_ENV),
        ) else {
            return;
        };
        let adapter = crate::StoppedNodePortabilityAdapter::new(root);
        WorkspaceRestorePort::restore_workspace(
            &adapter,
            RestoreWorkspaceRequest::new(
                attempt.parse().expect("restore attempt id"),
                RequestCorrelationId::new_v7(),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(Cursor::new(
                std::fs::read(archive).expect("read parent-owned archive"),
            )),
        )
        .expect("configured crash point must terminate before restore returns");
        panic!("configured restore crash point was not reached");
    }

    #[test]
    fn full_restore_sigkill_matrix() {
        #[derive(Clone, Copy)]
        enum ExpectedState {
            PreRename,
            RecoverableCurrent,
            Complete,
        }

        let cases = [
            ("received.created", ExpectedState::PreRename),
            ("received.written", ExpectedState::PreRename),
            ("received.file_synced", ExpectedState::PreRename),
            ("received.directory_synced", ExpectedState::PreRename),
            ("staging.created", ExpectedState::PreRename),
            ("staging.written", ExpectedState::PreRename),
            ("staging.file_synced", ExpectedState::PreRename),
            ("staging.directory_synced", ExpectedState::PreRename),
            ("import.rows_written", ExpectedState::PreRename),
            ("import.verified", ExpectedState::PreRename),
            ("import.transaction_committed", ExpectedState::PreRename),
            ("import.connection_closed", ExpectedState::PreRename),
            ("import.database_synced", ExpectedState::PreRename),
            ("import.sha256_synced", ExpectedState::PreRename),
            ("import.payloads_synced", ExpectedState::PreRename),
            ("import.attempt_synced", ExpectedState::PreRename),
            ("import.staging_synced", ExpectedState::PreRename),
            ("verified.created", ExpectedState::PreRename),
            ("verified.written", ExpectedState::PreRename),
            ("verified.file_synced", ExpectedState::PreRename),
            ("verified.directory_synced", ExpectedState::PreRename),
            ("marker.created", ExpectedState::PreRename),
            ("marker.written", ExpectedState::PreRename),
            ("marker.file_synced", ExpectedState::PreRename),
            ("marker.directory_synced", ExpectedState::PreRename),
            ("activating.created", ExpectedState::PreRename),
            ("activating.written", ExpectedState::PreRename),
            ("activating.file_synced", ExpectedState::PreRename),
            ("activating.directory_synced", ExpectedState::PreRename),
            ("activation.attempt_synced", ExpectedState::PreRename),
            ("activation.staging_synced", ExpectedState::PreRename),
            ("activation.renamed", ExpectedState::RecoverableCurrent),
            ("activation.root_synced", ExpectedState::RecoverableCurrent),
            ("complete.created", ExpectedState::RecoverableCurrent),
            ("complete.written", ExpectedState::RecoverableCurrent),
            ("complete.file_synced", ExpectedState::RecoverableCurrent),
            ("complete.renamed", ExpectedState::Complete),
            ("complete.directory_synced", ExpectedState::Complete),
            ("activation.complete_root_synced", ExpectedState::Complete),
        ];

        let archive_root = tempfile::tempdir().expect("parent-owned crash archive root");
        let archive_path = archive_root.path().join("workspace.fasti");
        std::fs::write(&archive_path, full_fixture().archive).expect("write crash archive");
        for (point, expected) in cases {
            let root = tempfile::tempdir().expect("crash-matrix data root");
            let attempt = RestoreAttemptId::new_v7();
            // nosemgrep: rust.lang.security.current-exe.current-exe -- test-only re-exec worker (#[cfg(test)]), never compiled into a release binary
            let output = Command::new(std::env::current_exe().expect("current test binary"))
                .args([
                    "--exact",
                    "restore_import::tests::full_restore_crash_worker",
                    "--ignored",
                    "--nocapture",
                ])
                .env(CRASH_POINT_ENV, point)
                .env(CRASH_ROOT_ENV, root.path())
                .env(CRASH_ATTEMPT_ENV, attempt.to_string())
                .env(CRASH_ARCHIVE_ENV, &archive_path)
                .output()
                .expect("run restore crash worker");
            assert_eq!(
                output.status.signal(),
                Some(9),
                "{point} did not terminate with SIGKILL; status={:?}; stdout={}; stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );

            drop(
                LockedDataRoot::acquire(root.path())
                    .unwrap_or_else(|error| panic!("{point} leaked the data-root lock: {error}")),
            );
            let staged = root
                .path()
                .join(RESTORE_STAGING_DIRECTORY)
                .join(attempt.to_string());
            let current = root.path().join("current");
            match expected {
                ExpectedState::PreRename => {
                    assert!(!current.exists(), "{point} exposed current before rename");
                    assert!(staged.exists(), "{point} lost interrupted staging");
                    assert!(matches!(
                        crate::SqliteKernel::open(root.path()),
                        Err(StoreOpenError::RestoreActivation)
                    ));
                    let retry_attempt = RestoreAttemptId::new_v7();
                    let adapter = crate::StoppedNodePortabilityAdapter::new(root.path());
                    WorkspaceRestorePort::restore_workspace(
                        &adapter,
                        RestoreWorkspaceRequest::new(
                            retry_attempt,
                            RequestCorrelationId::new_v7(),
                            limits(),
                            CancellationSignal::new(),
                        ),
                        Box::new(Cursor::new(
                            std::fs::read(&archive_path).expect("read retry archive"),
                        )),
                    )
                    .unwrap_or_else(|error| panic!("{point} did not reject and retry: {error:?}"));
                    assert!(!staged.exists(), "{point} retained rejected staging");
                    drop(
                        crate::SqliteKernel::open(root.path()).unwrap_or_else(|error| {
                            panic!("{point} retry did not open complete current: {error}")
                        }),
                    );
                }
                ExpectedState::RecoverableCurrent => {
                    assert!(current.exists(), "{point} lost renamed current");
                    assert!(!staged.exists(), "{point} retained renamed staging");
                    drop(
                        crate::SqliteKernel::open(root.path()).unwrap_or_else(|error| {
                            panic!("{point} did not recover digest-proven current: {error}")
                        }),
                    );
                    assert_eq!(
                        std::fs::read(current.join("restore.complete"))
                            .expect("recovered COMPLETE phase"),
                        b"complete\n"
                    );
                }
                ExpectedState::Complete => {
                    assert!(current.exists(), "{point} lost complete current");
                    drop(
                        crate::SqliteKernel::open(root.path()).unwrap_or_else(|error| {
                            panic!("{point} did not open complete current: {error}")
                        }),
                    );
                }
            }
        }

        for point in [
            "rejected.created",
            "rejected.written",
            "rejected.file_synced",
            "rejected.renamed",
            "rejected.directory_synced",
        ] {
            let root = tempfile::tempdir().expect("rejection crash-matrix data root");
            let root_handle = File::open(root.path()).expect("rejection data-root handle");
            let stale_attempt = RestoreAttemptId::new_v7();
            let (staging, attempt) = create_staging_attempt(
                &root_handle,
                RESTORE_STAGING_DIRECTORY,
                &stale_attempt.to_string(),
            )
            .expect("stale staging attempt");
            write_restore_phase(&attempt, RestoreStatus::Received).expect("stale received phase");
            drop((staging, attempt, root_handle));

            let retry_attempt = RestoreAttemptId::new_v7();
            // nosemgrep: rust.lang.security.current-exe.current-exe -- test-only re-exec worker (#[cfg(test)]), never compiled into a release binary
            let output = Command::new(std::env::current_exe().expect("current test binary"))
                .args([
                    "--exact",
                    "restore_import::tests::full_restore_crash_worker",
                    "--ignored",
                    "--nocapture",
                ])
                .env(CRASH_POINT_ENV, point)
                .env(CRASH_ROOT_ENV, root.path())
                .env(CRASH_ATTEMPT_ENV, retry_attempt.to_string())
                .env(CRASH_ARCHIVE_ENV, &archive_path)
                .output()
                .expect("run rejection crash worker");
            assert_eq!(
                output.status.signal(),
                Some(9),
                "{point} did not terminate with SIGKILL; status={:?}; stdout={}; stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            assert!(!root.path().join("current").exists());

            let adapter = crate::StoppedNodePortabilityAdapter::new(root.path());
            WorkspaceRestorePort::restore_workspace(
                &adapter,
                RestoreWorkspaceRequest::new(
                    RestoreAttemptId::new_v7(),
                    RequestCorrelationId::new_v7(),
                    limits(),
                    CancellationSignal::new(),
                ),
                Box::new(Cursor::new(
                    std::fs::read(&archive_path).expect("read rejection retry archive"),
                )),
            )
            .unwrap_or_else(|error| panic!("{point} rejection did not recover: {error:?}"));
            drop(
                crate::SqliteKernel::open(root.path())
                    .unwrap_or_else(|error| panic!("{point} retry did not open: {error}")),
            );
        }
    }

    #[test]
    fn malformed_retry_preserves_interrupted_staging() {
        let root = tempfile::tempdir().expect("restore root");
        let root_handle = File::open(root.path()).expect("data-root handle");
        let stale_attempt = RestoreAttemptId::new_v7();
        let (staging, attempt) = create_staging_attempt(
            &root_handle,
            RESTORE_STAGING_DIRECTORY,
            &stale_attempt.to_string(),
        )
        .expect("stale staging attempt");
        write_restore_phase(&attempt, RestoreStatus::Received).expect("stale received phase");
        drop((staging, attempt, root_handle));

        let stale_path = root
            .path()
            .join(RESTORE_STAGING_DIRECTORY)
            .join(stale_attempt.to_string());
        let adapter = crate::StoppedNodePortabilityAdapter::new(root.path());
        assert!(WorkspaceRestorePort::restore_workspace(
            &adapter,
            RestoreWorkspaceRequest::new(
                RestoreAttemptId::new_v7(),
                RequestCorrelationId::new_v7(),
                limits(),
                CancellationSignal::new(),
            ),
            Box::new(Cursor::new(b"not a fasti archive".to_vec())),
        )
        .is_err());
        assert!(stale_path.is_dir());
        assert!(!stale_path.join("restore.rejected").exists());
        assert!(!root.path().join("current").exists());
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
        let expected_counts = [1_i64, 1, 1, 2, 1, 2, 1, 1, 2, 1, 3, 1, 2, 1, 1, 1, 1, 1, 1];
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
            "metadata_field_claims",
            "metadata_field_overrides",
            "profile_record_tracking_dispositions",
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
            .query_row(NODE_LOCAL_STATE_COUNT_SQL, [], |row| row.get(0))
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
    fn archive_v3_round_trips_two_profile_metadata_without_identity_loss() {
        let fixture = metadata_v3_fixture();
        let archive = archive_v3_from_v4(&fixture.archive);
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(archive),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage metadata-v3 archive");
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open staged database");

        assert_eq!(
            database
                .query_row(
                    "SELECT claim_kind FROM metadata_claims WHERE claim_id = ?1",
                    [fixture.field_claim_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .expect("field claim identity"),
            "field"
        );
        assert_eq!(
            database
                .query_row(
                    "SELECT claim_kind FROM metadata_claims WHERE claim_id = ?1",
                    [fixture.rating_claim_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .expect("rating claim identity"),
            "rating"
        );
        assert_eq!(
            database
                .query_row(
                    "SELECT value_millis FROM metadata_rating_claims WHERE claim_id = ?1",
                    [fixture.rating_claim_id.to_string()],
                    |row| row.get::<_, i64>(0),
                )
                .expect("rating value"),
            8_400
        );
        assert_eq!(
            database
                .query_row(
                    "SELECT previous_status || '>' || status FROM metadata_claim_lifecycle_events WHERE claim_id = ?1 AND sequence = 1",
                    [fixture.field_claim_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .expect("lifecycle event"),
            "fresh>stale"
        );

        let policies = database
            .prepare(
                "SELECT profile_id, preferred_locale, region, enabled_field_groups, last_known_good_policy FROM metadata_projection_policies ORDER BY profile_id",
            )
            .expect("policy statement")
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .expect("policy rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("policies");
        assert_eq!(policies.len(), 2);
        assert!(policies.contains(&(
            fixture.first_profile_id.to_string(),
            Some("en-ie".to_owned()),
            Some("IE".to_owned()),
            "[\"basic_info\"]".to_owned(),
            "allow".to_owned(),
        )));
        assert!(policies.contains(&(
            fixture.second_profile_id.to_string(),
            Some("fr-fr".to_owned()),
            Some("FR".to_owned()),
            "[\"artwork\",\"basic_info\"]".to_owned(),
            "deny".to_owned(),
        )));

        let overrides = database
            .prepare(
                "SELECT profile_id, value FROM metadata_profile_field_overrides WHERE record_id = ?1 ORDER BY profile_id",
            )
            .expect("override statement")
            .query_map([fixture.record_id.to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("override rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("overrides");
        assert!(overrides.contains(&(
            fixture.first_profile_id.to_string(),
            "First profile title".to_owned(),
        )));
        assert!(overrides.contains(&(
            fixture.second_profile_id.to_string(),
            "Second profile title".to_owned(),
        )));
        // The archive contains authoritative metadata, not disposable postings.
        // Rebuild must run after import and preserve private visibility partitions.
        for (profile, gram, expected) in [
            (fixture.first_profile_id, "fir", 1),
            (fixture.second_profile_id, "sec", 1),
            (fixture.second_profile_id, "fir", 0),
        ] {
            let count: i64 = database.query_row(
                "SELECT COUNT(*) FROM local_search_grams WHERE profile_partition=?1 AND gram=?2 AND record_id=?3",
                params![profile.to_string(), gram, fixture.record_id.to_string()], |r| r.get(0),
            ).unwrap();
            assert_eq!(count, expected);
        }
        assert_eq!(
            database
                .query_row(
                    "SELECT attribution_text FROM metadata_attributions WHERE provider_id = 'tmdb'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .expect("attribution"),
            "Metadata supplied by TMDB"
        );

        drop(database);
        staged.cleanup().expect("remove staged attempt");
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn archive_v4_round_trips_immutable_metadata_refresh_receipts() {
        let fixture = metadata_v3_fixture();
        let archive = archive_v4_from_v5(&fixture.archive);
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(archive),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage metadata-v4 archive");
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open staged database");
        let response_json: String = database
            .query_row(
                "SELECT response_json FROM metadata_refresh_receipts WHERE operation_id = ?1",
                [fixture.operation_id.to_string()],
                |row| row.get(0),
            )
            .expect("restored receipt response");
        crate::metadata::decode_refresh_receipt_outcome(
            &response_json,
            fixture.record_id,
            &MetadataProviderId::try_new("tmdb").expect("provider"),
            fixture.first_profile_id,
            CapabilityKey::ExportWorkspace,
            RequestCorrelationId::new_v7(),
        )
        .expect("restored exact outcome");
        assert!(database
            .execute(
                "UPDATE metadata_refresh_receipts SET provider_id = 'mdblist' WHERE operation_id = ?1",
                [fixture.operation_id.to_string()],
            )
            .is_err());
        assert!(database
            .execute(
                "DELETE FROM metadata_refresh_receipts WHERE operation_id = ?1",
                [fixture.operation_id.to_string()],
            )
            .is_err());
        drop(database);

        staged.cleanup().expect("remove staged attempt");
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn archive_v5_round_trips_identity_routing_and_policy_receipts() {
        let fixture = identity_routing_archive_fixture();
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(fixture.archive),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage identity-routing archive-v5");
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open staged database");

        for (table, expected) in [
            ("identity_assertions", 1_i64),
            ("identity_assertion_lifecycle_events", 1),
            ("profile_anime_grouping_policies", 1),
            ("client_anime_grouping_policies", 1),
            ("anime_grouping_policy_receipts", 5),
        ] {
            let count: i64 = database
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("restored M3 count");
            assert_eq!(count, expected, "{table}");
        }
        assert_eq!(
            database
                .query_row(
                    "SELECT preference FROM profile_anime_grouping_policies WHERE profile_id = ?1",
                    [fixture.profile_id.to_string()],
                    |row| row.get::<_, String>(0),
                )
                .expect("profile policy"),
            "automatic"
        );
        assert_eq!(
            database
                .query_row(
                    "SELECT preference FROM client_anime_grouping_policies WHERE client_id = ?1",
                    [fixture.client_id.to_string()],
                    |row| row.get::<_, Option<String>>(0),
                )
                .expect("client policy"),
            None
        );
        assert!(database
            .execute(
                "DELETE FROM identity_assertions WHERE assertion_id = ?1",
                [fixture.assertion_id.to_string()],
            )
            .is_err());
        assert!(database
            .execute(
                "UPDATE anime_grouping_policy_receipts SET affected_records = 0 WHERE operation_id = ?1",
                [fixture.profile_operation_id.to_string()],
            )
            .is_err());
        drop(database);

        staged.cleanup().expect("remove staged attempt");
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn archive_v5_rejects_invalid_identity_lifecycle_and_payloads() {
        let fixture = identity_routing_archive_fixture();
        let maximum_profile_revision = rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::ProfileAnimeGroupingPolicies,
            |bytes| {
                let mut row: serde_json::Value =
                    serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                        .expect("profile anime policy row");
                row["revision"] = serde_json::json!(9_007_199_254_740_991_u64);
                *bytes = serde_json::to_vec(&row).expect("profile policy mutation");
                bytes.push(b'\n');
            },
        );
        let maximum_profile_revision = rewrite_stream(
            &maximum_profile_revision,
            WorkspaceExportEntity::AnimeGroupingPolicyReceipts,
            |bytes| {
                let mut rewritten = Vec::with_capacity(bytes.len());
                for line in bytes
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                {
                    let mut row: serde_json::Value =
                        serde_json::from_slice(line).expect("policy receipt row");
                    if row["scope_kind"] == "profile" && row["change_kind"] == "rollback" {
                        row["result_revision"] = serde_json::json!(9_007_199_254_740_991_u64);
                    }
                    rewritten.extend_from_slice(
                        &serde_json::to_vec(&row).expect("policy receipt mutation"),
                    );
                    rewritten.push(b'\n');
                }
                *bytes = rewritten;
            },
        );
        let duplicate_profile_revision = rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::ProfileAnimeGroupingPolicies,
            |bytes| {
                let mut row: serde_json::Value =
                    serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                        .expect("profile anime policy row");
                row["revision"] = serde_json::json!(4);
                *bytes = serde_json::to_vec(&row).expect("profile policy mutation");
                bytes.push(b'\n');
            },
        );
        let duplicate_profile_revision = rewrite_stream(
            &duplicate_profile_revision,
            WorkspaceExportEntity::AnimeGroupingPolicyReceipts,
            |bytes| {
                let mut rewritten = Vec::with_capacity(bytes.len());
                for line in bytes
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                {
                    let mut row: serde_json::Value =
                        serde_json::from_slice(line).expect("policy receipt row");
                    if row["scope_kind"] == "profile" && row["change_kind"] == "rollback" {
                        row["result_revision"] = serde_json::json!(4);
                    }
                    rewritten.extend_from_slice(
                        &serde_json::to_vec(&row).expect("policy receipt mutation"),
                    );
                    rewritten.push(b'\n');
                }
                *bytes = rewritten;
            },
        );
        let maximum_client_revision = rewrite_stream(
            &fixture.archive,
            WorkspaceExportEntity::ClientAnimeGroupingPolicies,
            |bytes| {
                let mut row: serde_json::Value =
                    serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                        .expect("client anime policy row");
                row["preference"] = serde_json::Value::Null;
                row["revision"] = serde_json::json!(9_007_199_254_740_991_u64);
                *bytes = serde_json::to_vec(&row).expect("client policy mutation");
                bytes.push(b'\n');
            },
        );
        let maximum_client_revision = rewrite_stream(
            &maximum_client_revision,
            WorkspaceExportEntity::AnimeGroupingPolicyReceipts,
            |bytes| {
                let mut rewritten = Vec::with_capacity(bytes.len());
                for line in bytes
                    .split(|byte| *byte == b'\n')
                    .filter(|line| !line.is_empty())
                {
                    let mut row: serde_json::Value =
                        serde_json::from_slice(line).expect("policy receipt row");
                    if row["scope_kind"] == "client" {
                        row["change_kind"] = serde_json::json!("inherit_profile");
                        row["requested_preference"] = serde_json::Value::Null;
                        row["result_preference"] = serde_json::json!("group_by_tv_work");
                        row["result_source"] = serde_json::json!("profile_default");
                        row["result_revision"] = serde_json::json!(9_007_199_254_740_991_u64);
                    }
                    rewritten.extend_from_slice(
                        &serde_json::to_vec(&row).expect("policy receipt mutation"),
                    );
                    rewritten.push(b'\n');
                }
                *bytes = rewritten;
            },
        );
        for (hostile, expected_policy_error) in [
            (
                rewrite_stream(
                    &fixture.archive,
                    WorkspaceExportEntity::IdentityAssertionLifecycleEvents,
                    |bytes| {
                        let mut row: serde_json::Value =
                            serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                                .expect("identity lifecycle row");
                        row["sequence"] = serde_json::json!(2);
                        *bytes = serde_json::to_vec(&row).expect("sequence mutation");
                        bytes.push(b'\n');
                    },
                ),
                false,
            ),
            (
                rewrite_stream(
                    &fixture.archive,
                    WorkspaceExportEntity::IdentityAssertions,
                    |bytes| {
                        let mut row: serde_json::Value =
                            serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                                .expect("identity assertion row");
                        row["evidence_json"] = serde_json::json!(serde_json::json!([{
                            "method": "invented",
                            "observed_source": "hostile",
                            "derivation_root": null,
                            "reviewer": null,
                            "observed_at": "2026-08-30",
                            "evidence_id": null,
                        }])
                        .to_string());
                        *bytes = serde_json::to_vec(&row).expect("payload mutation");
                        bytes.push(b'\n');
                    },
                ),
                false,
            ),
            (
                rewrite_stream(
                    &fixture.archive,
                    WorkspaceExportEntity::AnimeGroupingPolicyReceipts,
                    |bytes| {
                        let line_end = bytes
                            .iter()
                            .position(|byte| *byte == b'\n')
                            .expect("policy receipt line");
                        let mut row: serde_json::Value =
                            serde_json::from_slice(&bytes[..line_end]).expect("policy receipt row");
                        row["previous_preference"] =
                            serde_json::json!("keep_kitsu_releases_separate");
                        let mut replacement =
                            serde_json::to_vec(&row).expect("policy receipt mutation");
                        replacement.push(b'\n');
                        replacement.extend_from_slice(&bytes[line_end + 1..]);
                        *bytes = replacement;
                    },
                ),
                true,
            ),
            (
                rewrite_stream(
                    &fixture.archive,
                    WorkspaceExportEntity::AnimeGroupingPolicyReceipts,
                    |bytes| {
                        let line_end = bytes
                            .iter()
                            .position(|byte| *byte == b'\n')
                            .expect("policy receipt line");
                        let mut row: serde_json::Value =
                            serde_json::from_slice(&bytes[..line_end]).expect("policy receipt row");
                        row["affected_records"] = serde_json::json!(999);
                        let mut replacement =
                            serde_json::to_vec(&row).expect("policy receipt mutation");
                        replacement.push(b'\n');
                        replacement.extend_from_slice(&bytes[line_end + 1..]);
                        *bytes = replacement;
                    },
                ),
                true,
            ),
            (maximum_client_revision, true),
            (maximum_profile_revision, true),
            (duplicate_profile_revision, true),
        ] {
            let restore_root = tempfile::tempdir().expect("restore root");
            let lock =
                LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
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
            .expect("invalid identity-routing state must fail");
            assert!(if expected_policy_error {
                matches!(error, RestoreImportError::PolicyReceiptInvariant)
            } else {
                matches!(error, RestoreImportError::IdentityRoutingInvariant)
            });
            assert_attempt_removed(restore_root.path(), attempt_id);
        }
    }

    #[test]
    fn restore_repairs_legacy_provider_coordinates_before_activation() {
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(legacy_google_books_archive()),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage legacy provider fixture");
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open staged database");
        let restored: (String, String, String, String, i64) = database
            .query_row(
                r#"
                SELECT record.grain, identifier.namespace, identifier.grain, claim.source,
                       (SELECT user_version FROM pragma_user_version)
                FROM records record
                JOIN external_identifiers identifier ON identifier.record_id = record.record_id
                JOIN metadata_field_claims claim ON claim.record_id = record.record_id
                WHERE identifier.value = 'restore-book'
                "#,
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("restored canonical provider coordinate");
        assert_eq!(
            restored,
            (
                "edition".to_owned(),
                "googlebooks.volume".to_owned(),
                "edition".to_owned(),
                "googlebooks.volume".to_owned(),
                SCHEMA_VERSION,
            )
        );

        drop(database);
        staged.cleanup().expect("remove staged attempt");
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn archive_v1_restore_keeps_legacy_rows_and_leaves_v2_tables_empty() {
        let fixture = full_fixture();
        let archive = rewrite_manifest_schema(
            &archive_v1_from_v2(&fixture.archive),
            11,
            "sha256:c833fb634b64d0b9680e4734b22684e8eab36710fca5c95d4315f3141491687a",
        );
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();

        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(archive),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("stage archive-v1 fixture");
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open staged database");
        let record_count: i64 = database
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .expect("restored legacy record count");
        assert_eq!(record_count, 2);
        for table in [
            "metadata_field_claims",
            "metadata_field_overrides",
            "profile_record_tracking_dispositions",
        ] {
            let count: i64 = database
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("archive-v2 table count");
            assert_eq!(count, 0, "{table}");
        }
        drop(database);

        staged.cleanup().expect("remove staged attempt");
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn historical_v11_archive_v1_and_v2_restore_into_the_current_schema() {
        let fixture = full_fixture();
        let v2 = archive_v2_from_v3(&fixture.archive);
        for archive in [archive_v1_from_v2(&v2), v2] {
            let archive = rewrite_manifest_schema(
                &archive,
                11,
                "sha256:c833fb634b64d0b9680e4734b22684e8eab36710fca5c95d4315f3141491687a",
            );
            let restore_root = tempfile::tempdir().expect("restore root");
            let lock =
                LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
            let attempt_id = RestoreAttemptId::new_v7();
            let staged = stage_workspace_archive_pass_two(
                &lock,
                &mut Cursor::new(archive),
                attempt_id,
                RequestCorrelationId::new_v7(),
                limits(),
                &CancellationSignal::new(),
            )
            .expect("historical archive restores into current schema");
            staged.cleanup().expect("remove staged attempt");
            assert_attempt_removed(restore_root.path(), attempt_id);
        }
    }

    #[test]
    fn historical_v13_archive_v4_restores_without_local_access_authority() {
        let fixture = full_fixture();
        let archive = archive_v4_from_v5(&fixture.archive);
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let staged = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(archive),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .expect("historical archive-v4/schema-v13 restores");
        let database = Connection::open_with_flags(
            staged.database_path(),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .expect("open staged database");
        let local_rows: i64 = database
            .query_row(NODE_LOCAL_STATE_COUNT_SQL, [], |row| row.get(0))
            .expect("node-local state");
        assert_eq!(local_rows, 0);
        drop(database);
        staged.cleanup().expect("remove staged attempt");
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn archive_v4_rejects_a_forged_v13_schema_fingerprint() {
        let fixture = full_fixture();
        let v4 = archive_v4_from_v5(&fixture.archive);
        let archive = rewrite_manifest_schema(
            &v4,
            13,
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        );
        let restore_root = tempfile::tempdir().expect("restore root");
        let lock = LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
        let attempt_id = RestoreAttemptId::new_v7();
        let error = stage_workspace_archive_pass_two(
            &lock,
            &mut Cursor::new(archive),
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            &CancellationSignal::new(),
        )
        .err()
        .expect("forged schema fingerprint is rejected");
        assert!(matches!(error, RestoreImportError::SchemaMismatch));
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn archive_v4_accepts_only_the_exact_v14_schema_fingerprint() {
        let digest = "sha256:630bc759b1bc6148931fe1b496e6e149553c5c005cf8d5956da683f2872c0375";
        assert!(accepted_archive_schema(
            WORKSPACE_ARCHIVE_V4_FORMAT_VERSION,
            14,
            digest,
            "unused-current-digest",
        ));
        assert!(!accepted_archive_schema(
            WORKSPACE_ARCHIVE_V4_FORMAT_VERSION,
            14,
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "unused-current-digest",
        ));
    }

    #[test]
    fn archive_v5_retains_only_the_exact_published_v15_fingerprint() {
        let digest = "sha256:36720ca62ef606e52f960e71cb40452323269f14e4a4af984e2fe875279a155e";
        assert!(accepted_archive_schema(5, 15, digest, "current"));
        assert!(!accepted_archive_schema(5, 15, "forged", "current"));
        assert!(!accepted_archive_schema(4, 15, digest, "current"));
        for format in 1..=5 {
            assert!(!accepted_archive_schema(format, 16, "current", "current"));
        }
        assert!(accepted_archive_schema(
            6,
            16,
            "sha256:d7ae3b1ab15c0223245d1a9008833049e58e9ec882a6e1ba70a2a080fa3fd7a6",
            "current"
        ));
        assert!(!accepted_archive_schema(6, 16, "current", "current"));
        assert!(!accepted_archive_schema(6, 16, "forged", "current"));
        assert!(!accepted_archive_schema(6, 15, digest, "current"));
        assert!(accepted_archive_schema(7, 17, "current", "current"));
        assert!(!accepted_archive_schema(7, 17, "forged", "current"));
        assert!(!accepted_archive_schema(7, 16, "current", "current"));
        assert!(!accepted_archive_schema(6, 17, "current", "current"));
    }

    #[test]
    fn archive_v3_cannot_claim_a_pre_v3_schema_fingerprint() {
        let fixture = metadata_v3_fixture();
        let archive = archive_v3_from_v4(&fixture.archive);
        let hostile = rewrite_manifest_schema(
            &archive,
            11,
            "sha256:c833fb634b64d0b9680e4734b22684e8eab36710fca5c95d4315f3141491687a",
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
        .expect("archive-v3 must require the v3 schema");
        assert!(matches!(error, RestoreImportError::SchemaMismatch));
        assert_attempt_removed(restore_root.path(), attempt_id);
    }

    #[test]
    fn hostile_metadata_lifecycle_chains_are_rejected() {
        let fixture = metadata_v3_fixture();
        for hostile in [
            rewrite_stream(
                &fixture.archive,
                WorkspaceExportEntity::MetadataClaimLifecycleEvents,
                |bytes| {
                    let mut row: serde_json::Value =
                        serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                            .expect("lifecycle row");
                    row["sequence"] = serde_json::json!(2);
                    *bytes = serde_json::to_vec(&row).expect("sequence mutation");
                    bytes.push(b'\n');
                },
            ),
            rewrite_stream(
                &fixture.archive,
                WorkspaceExportEntity::MetadataClaimLifecycleEvents,
                |bytes| {
                    let mut row: serde_json::Value =
                        serde_json::from_slice(&bytes[..bytes.len().saturating_sub(1)])
                            .expect("lifecycle row");
                    row["previous_status"] = serde_json::json!("stale");
                    *bytes = serde_json::to_vec(&row).expect("status mutation");
                    bytes.push(b'\n');
                },
            ),
            rewrite_stream(
                &fixture.archive,
                WorkspaceExportEntity::MetadataClaimProvenance,
                Vec::clear,
            ),
        ] {
            let restore_root = tempfile::tempdir().expect("restore root");
            let lock =
                LockedDataRoot::acquire(restore_root.path()).expect("exclusive restore root");
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
            .expect("invalid lifecycle chain must fail");
            assert!(matches!(
                error,
                RestoreImportError::DomainInvariant
                    | RestoreImportError::MetadataLifecycleInvariant
            ));
            assert_attempt_removed(restore_root.path(), attempt_id);
        }
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
            matches!(
                error,
                RestoreImportError::DomainInvariant | RestoreImportError::AggregateInvariant
            ),
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
            matches!(
                error,
                RestoreImportError::DomainInvariant | RestoreImportError::AggregateInvariant
            ),
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
        let archive = fixture.archive;
        let canceled_root = tempfile::tempdir().expect("canceled restore root");
        let canceled_lock =
            LockedDataRoot::acquire(canceled_root.path()).expect("exclusive canceled root");
        let cancellation = CancellationSignal::new();
        cancellation.cancel();
        let canceled = stage_workspace_archive_pass_two(
            &canceled_lock,
            &mut Cursor::new(archive.clone()),
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

        let activation_root = tempfile::tempdir().expect("activation cancellation root");
        let activation_lock = LockedDataRoot::acquire(activation_root.path())
            .expect("exclusive activation cancellation root");
        let activation_cancellation = CancellationSignal::new();
        let staged = stage_workspace_archive_pass_two(
            &activation_lock,
            &mut Cursor::new(archive.clone()),
            RestoreAttemptId::new_v7(),
            RequestCorrelationId::new_v7(),
            limits(),
            &activation_cancellation,
        )
        .expect("stage before cancellation");
        activation_cancellation.cancel();
        let root = activation_lock
            .anchored_directory()
            .expect("anchored activation root");
        let activation = staged
            .activate(root, &activation_cancellation)
            .expect_err("cancellation before marker must reject activation");
        assert!(matches!(activation, RestoreImportError::Canceled));
        assert!(!activation_root.path().join("current").exists());
        assert!(std::fs::read_dir(activation_root.path().join("staging"))
            .expect("empty staging")
            .next()
            .is_none());

        let capacity_root = tempfile::tempdir().expect("capacity restore root");
        let capacity_lock =
            LockedDataRoot::acquire(capacity_root.path()).expect("exclusive capacity root");
        let mut configured = limits();
        configured.scratch_ceiling_bytes = nonzero(
            configured.max_snapshot_bytes.get() + configured.cleanup_reserve_bytes.get() - 1,
        );
        let capacity = stage_workspace_archive_pass_two(
            &capacity_lock,
            &mut Cursor::new(archive),
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
        let adapter = crate::StoppedNodePortabilityAdapter::new(restore_root.path());
        let attempt_id = RestoreAttemptId::new_v7();
        let outcome = WorkspaceRestorePort::restore_workspace(
            &adapter,
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

        let refused = WorkspaceRestorePort::restore_workspace(
            &adapter,
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
        let marker = staged
            .activate(root, &CancellationSignal::new())
            .expect("activate verified restore");
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
            .query_row(NODE_LOCAL_STATE_COUNT_SQL, [], |row| row.get(0))
            .expect("node-local state");
        assert_eq!(workspace_rows, 1);
        assert_eq!(local_rows, 0);
        drop(connection);
        drop(kernel);

        let adapter = crate::StoppedNodePortabilityAdapter::new(restore_root.path());
        let prepared = RecoveryBootstrapPort::prepare_recovery_bootstrap(
            &adapter,
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
        let completed = RecoveryBootstrapPort::complete_recovery_bootstrap(
            &adapter,
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

        let restored = crate::SqliteKernel::open(restore_root.path()).expect("open recovered node");
        let rejected = restored
            .authenticate_credential(AuthenticateCredentialQuery::new(
                RequestCorrelationId::new_v7(),
                CapabilityKey::VerifyWorkspace,
                SecretMaterial::try_from_hex(&fixture.source_credential_hex)
                    .expect("copy source credential"),
            ))
            .expect_err("source credential must not survive restore");
        assert_eq!(
            rejected.code(),
            fasti_application::ProblemCode::AuthenticationFailed
        );
        let recovered_access = restored
            .authenticate_credential(AuthenticateCredentialQuery::new(
                RequestCorrelationId::new_v7(),
                CapabilityKey::VerifyWorkspace,
                SecretMaterial::from_bytes([7_u8; 32]),
            ))
            .expect("recovery credential authenticates");
        assert_eq!(&recovered_access, completed.access());
        let verified = restored
            .verify_workspace(VerifyWorkspaceQuery::new(
                RequestCorrelationId::new_v7(),
                recovered_access,
            ))
            .expect("recovered workspace verifies");
        assert_eq!(verified.workspace_id(), workspace_id);
        assert_eq!(verified.observations_verified(), 1);
        assert_eq!(verified.evidence_verified(), 1);
        assert_eq!(verified.corrections_verified(), 1);
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
