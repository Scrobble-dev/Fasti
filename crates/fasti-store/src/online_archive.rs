//! Store production of the staged B3 `.fasti` archive.
//!
//! The live kernel owns online export. A distinct stopped-node adapter owns
//! offline export so neither mode can resolve paths through the wrong owner.

use crate::archive::{ArchiveError, ArchiveLimits, ArchiveWriter};
use crate::crypto::encode_hex;
use crate::kernel::{authorize_transaction, map_sql, LockedDataRoot, SqliteKernel};
use crate::portability::{
    map_offline_open_error, schema_fingerprint, snapshot_evidence_blobs, stream_archive_entity,
    SnapshotEvidenceBlob,
};
use crate::schema::workspace_revision;
use crate::{SnapshotError, SnapshotLimits};
use fasti_application::{
    ApplicationResult, CapabilityKey, ExportWorkspaceRequest, FailedArchiveDestinationState,
    FastiProblem, PortabilityFailureReceipt, PortabilityLimits, PortabilityResult, ProblemCode,
    StoppedNodeExportRequest, WorkspaceArchiveCompletionError, WorkspaceArchiveDestination,
    WorkspaceArchiveExportOutcome, WorkspaceArchiveExportPort, WorkspaceExportEntity,
    WorkspaceManifest, MAX_PORTABLE_JSON_INTEGER, WORKSPACE_ARCHIVE_CONTRACT_VERSION,
};
use fasti_contracts::CanonicalWorkspaceManifestProjection;
use fasti_domain::{RequestCorrelationId, Sha256Digest, ARCHIVE_MAX_IO_CHUNK_BYTES};
use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::num::NonZeroU32;
use std::ops::ControlFlow;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

const SQLITE_MINIMUM_PAGE_BYTES: u64 = 512;

/// Produce one complete online archive.
///
/// The destination remains guarded until its consuming `complete` call begins.
/// Any earlier failure explicitly invokes `abort` and replaces the failure with
/// `storage_unavailable` if cleanup fails. Because `complete` consumes the
/// destination, its implementation owns cleanup and exact destination-state
/// reporting if publication fails.
pub(crate) fn export_online_workspace_archive(
    kernel: &SqliteKernel,
    request: ExportWorkspaceRequest,
    destination: Box<dyn WorkspaceArchiveDestination>,
) -> PortabilityResult<WorkspaceArchiveExportOutcome> {
    let correlation_id = request.query().correlation_id();
    let mut destination = DestinationGuard::new(destination);
    let completed = AnchoredOnlineRoot::open(kernel, correlation_id)
        .and_then(|root| build_online_archive(kernel, &root, &request, &mut destination));

    match completed {
        Ok((archive_bytes, archive_digest, projection)) => {
            let workspace_id = projection.application_manifest().workspace_id();
            let workspace_revision = projection.application_manifest().workspace_revision();
            let manifest_digest = projection.manifest_digest().clone();
            if let Err(problem) = monitor_export(kernel, &request, true) {
                let (problem, state) = destination.abort_problem(problem, correlation_id);
                return Err(online_receipt_with_destination_state(
                    &request, problem, state,
                ));
            }
            if let Err(error) = destination.complete(&archive_digest, &manifest_digest) {
                return Err(online_receipt_with_destination_state(
                    &request,
                    Box::new(FastiProblem::storage_unavailable(
                        CapabilityKey::ExportWorkspace,
                        correlation_id,
                    )),
                    error.destination_state(),
                ));
            }
            Ok(WorkspaceArchiveExportOutcome::new(
                workspace_id,
                workspace_revision,
                manifest_digest,
                archive_bytes,
                archive_digest,
            ))
        }
        Err(problem) => {
            let (problem, state) = destination.abort_problem(problem, correlation_id);
            Err(online_receipt_with_destination_state(
                &request, problem, state,
            ))
        }
    }
}

impl WorkspaceArchiveExportPort for SqliteKernel {
    fn export_workspace_archive(
        &self,
        request: ExportWorkspaceRequest,
        destination: Box<dyn WorkspaceArchiveDestination>,
    ) -> PortabilityResult<WorkspaceArchiveExportOutcome> {
        export_online_workspace_archive(self, request, destination)
    }

    fn export_stopped_node_workspace_archive(
        &self,
        request: StoppedNodeExportRequest,
        destination: Box<dyn WorkspaceArchiveDestination>,
    ) -> PortabilityResult<WorkspaceArchiveExportOutcome> {
        let correlation_id = request.query().correlation_id();
        let (problem, state) = abort_destination_problem(
            destination,
            Box::new(FastiProblem::data_root_locked(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            )),
            correlation_id,
        );
        Err(stopped_receipt_with_destination_state(
            &request, problem, state,
        ))
    }
}

/// Produce one complete stopped-node archive for the stopped-node adapter.
///
/// This seam owns the shared data-root lock for the complete operation. It is
/// crate-private so callers cannot bypass the adapter's ownership split.
pub(crate) fn export_stopped_node_workspace_archive(
    data_root: impl AsRef<Path>,
    request: StoppedNodeExportRequest,
    destination: Box<dyn WorkspaceArchiveDestination>,
) -> PortabilityResult<WorkspaceArchiveExportOutcome> {
    let mut destination = DestinationGuard::new(destination);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = data_root;
        let correlation_id = request.query().correlation_id();
        let (problem, state) =
            destination.abort_problem(unsupported_platform_problem(correlation_id), correlation_id);
        return Err(stopped_receipt_with_destination_state(
            &request, problem, state,
        ));
    }
    #[cfg(target_os = "linux")]
    {
        match build_stopped_node_archive(data_root.as_ref(), &request, &mut destination) {
            Ok(outcome) => Ok(outcome),
            Err(failure) => {
                let problem = match failure.destination_state {
                    FailedArchiveDestinationState::Discarded => {
                        let (problem, state) = destination
                            .abort_problem(failure.problem, request.query().correlation_id());
                        return Err(stopped_receipt_with_destination_state(
                            &request, problem, state,
                        ));
                    }
                    FailedArchiveDestinationState::PartialCleanupIndeterminate
                    | FailedArchiveDestinationState::PublishedDurabilityIndeterminate => {
                        failure.problem
                    }
                };
                Err(stopped_receipt_with_destination_state(
                    &request,
                    problem,
                    failure.destination_state,
                ))
            }
        }
    }
}

trait ExportMonitor {
    fn check(&self, started: bool) -> ApplicationResult<()>;
}

struct OnlineExportMonitor<'a> {
    kernel: &'a SqliteKernel,
    request: &'a ExportWorkspaceRequest,
}

impl ExportMonitor for OnlineExportMonitor<'_> {
    fn check(&self, started: bool) -> ApplicationResult<()> {
        monitor_export(self.kernel, self.request, started)
    }
}

struct StoppedNodeExportMonitor<'a> {
    connection: &'a Connection,
    request: &'a StoppedNodeExportRequest,
}

impl ExportMonitor for StoppedNodeExportMonitor<'_> {
    fn check(&self, _started: bool) -> ApplicationResult<()> {
        let capability = CapabilityKey::ExportWorkspace;
        let correlation_id = self.request.query().correlation_id();
        if self.request.cancellation().is_cancelled() {
            return Err(Box::new(FastiProblem::export_canceled(correlation_id)));
        }
        let transaction = map_sql(
            self.connection.unchecked_transaction(),
            capability,
            correlation_id,
        )?;
        authorize_transaction(
            &transaction,
            capability,
            self.request.query().access(),
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)
    }
}

fn build_online_archive(
    kernel: &SqliteKernel,
    root: &AnchoredOnlineRoot,
    request: &ExportWorkspaceRequest,
    destination: &mut DestinationGuard,
) -> ApplicationResult<(u64, Sha256Digest, CanonicalWorkspaceManifestProjection)> {
    let capability = CapabilityKey::ExportWorkspace;
    let correlation_id = request.query().correlation_id();
    let limits = request.limits();

    monitor_export(kernel, request, false)?;
    let bounds = AdmissionBounds::try_new(limits, correlation_id)?;
    destination
        .preflight(bounds.destination_bytes)
        .map_err(|_| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
    monitor_export(kernel, request, true)?;
    let source_connection = root.open_source_connection(limits, correlation_id)?;
    if database_logical_bytes(&source_connection, correlation_id)? > limits.max_snapshot_bytes.get()
    {
        return capacity_failure(correlation_id);
    }

    let mut scratch = root.create_scratch(correlation_id, &bounds)?;
    let snapshot_path = scratch.snapshot_path(correlation_id)?;
    let initial_wal_bytes = root.wal_bytes(correlation_id)?;
    monitor_snapshot_resources(
        root,
        &scratch,
        initial_wal_bytes,
        limits,
        correlation_id,
        false,
        true,
    )?;

    let snapshot_limits = snapshot_limits(limits, correlation_id)?;
    monitor_export(kernel, request, true)?;
    let stopped_problem = Rc::new(RefCell::new(None));
    let callback_problem = Rc::clone(&stopped_problem);
    let snapshot = kernel.snapshot_database_from_connection(
        &source_connection,
        &snapshot_path,
        snapshot_limits,
        |_| {
            let result = monitor_export(kernel, request, true).and_then(|()| {
                monitor_snapshot_resources(
                    root,
                    &scratch,
                    initial_wal_bytes,
                    limits,
                    correlation_id,
                    true,
                    true,
                )
            });
            match result {
                Ok(()) => ControlFlow::Continue(()),
                Err(problem) => {
                    *callback_problem.borrow_mut() = Some(problem);
                    ControlFlow::Break(())
                }
            }
        },
    );
    drop(source_connection);
    let snapshot = match snapshot {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if let Some(problem) = stopped_problem.borrow_mut().take() {
                return Err(problem);
            }
            return Err(map_snapshot_error(error, correlation_id));
        }
    };
    if snapshot.byte_len > limits.max_snapshot_bytes.get() {
        return stopped_node_failure(correlation_id);
    }
    monitor_export(kernel, request, true)?;
    monitor_snapshot_resources(
        root,
        &scratch,
        initial_wal_bytes,
        limits,
        correlation_id,
        true,
        false,
    )?;

    // The live source connection is released before archive generation. Rows
    // are read through a separate connection to the immutable snapshot; the
    // kernel connection is borrowed only for short reauthorization transactions.
    let snapshot_file = scratch.open_snapshot_file(correlation_id)?;
    let snapshot_connection = open_read_only_snapshot(&snapshot_file, correlation_id)?;
    let monitor = OnlineExportMonitor { kernel, request };
    let completed = assemble_workspace_archive(
        root,
        &snapshot_connection,
        request.query().access().workspace_id(),
        limits,
        bounds.archive_expanded_bytes,
        &mut scratch,
        destination,
        &monitor,
        correlation_id,
    );
    drop(snapshot_connection);
    drop(snapshot_file);
    let completed = completed?;
    scratch.cleanup(correlation_id)?;
    Ok(completed)
}

#[cfg(target_os = "linux")]
struct StoppedArchiveBuildFailure {
    problem: Box<FastiProblem>,
    destination_state: FailedArchiveDestinationState,
}

#[cfg(target_os = "linux")]
impl From<Box<FastiProblem>> for StoppedArchiveBuildFailure {
    fn from(problem: Box<FastiProblem>) -> Self {
        Self {
            problem,
            destination_state: FailedArchiveDestinationState::Discarded,
        }
    }
}

#[cfg(target_os = "linux")]
fn build_stopped_node_archive(
    data_root: &Path,
    request: &StoppedNodeExportRequest,
    destination: &mut DestinationGuard,
) -> Result<WorkspaceArchiveExportOutcome, StoppedArchiveBuildFailure> {
    let capability = CapabilityKey::ExportWorkspace;
    let correlation_id = request.query().correlation_id();
    let limits = request.limits();
    let locked = LockedDataRoot::acquire(data_root)
        .map_err(|error| map_offline_open_error(error, capability, correlation_id))?;
    let root = AnchoredOnlineRoot::open_locked(&locked, correlation_id)?;
    let authorization_connection = root.open_authorization_connection(limits, correlation_id)?;
    let monitor = StoppedNodeExportMonitor {
        connection: &authorization_connection,
        request,
    };
    monitor.check(false)?;
    let bounds = AdmissionBounds::try_stopped(limits, correlation_id)?;
    destination
        .preflight(bounds.destination_bytes)
        .map_err(|_| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
    monitor.check(true)?;

    let source_connection = root.open_source_connection(limits, correlation_id)?;
    let mut scratch = root.create_scratch(correlation_id, &bounds)?;
    let (archive_bytes, archive_digest, projection) = assemble_workspace_archive(
        &root,
        &source_connection,
        request.workspace_id(),
        limits,
        bounds.archive_expanded_bytes,
        &mut scratch,
        destination,
        &monitor,
        correlation_id,
    )?;
    drop(source_connection);
    scratch.cleanup(correlation_id)?;

    let workspace_id = projection.application_manifest().workspace_id();
    let workspace_revision = projection.application_manifest().workspace_revision();
    let manifest_digest = projection.manifest_digest().clone();
    monitor.check(true)?;
    destination
        .complete(&archive_digest, &manifest_digest)
        .map_err(|error| StoppedArchiveBuildFailure {
            problem: storage_failure(correlation_id),
            destination_state: error.destination_state(),
        })?;
    drop(authorization_connection);
    drop(locked);
    Ok(WorkspaceArchiveExportOutcome::new(
        workspace_id,
        workspace_revision,
        manifest_digest,
        archive_bytes,
        archive_digest,
    ))
}

#[allow(clippy::too_many_arguments)]
fn assemble_workspace_archive(
    root: &AnchoredOnlineRoot,
    source_connection: &Connection,
    workspace_id: fasti_domain::WorkspaceId,
    limits: PortabilityLimits,
    archive_expanded_bytes: u64,
    scratch: &mut ExportScratch,
    destination: &mut DestinationGuard,
    monitor: &dyn ExportMonitor,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<(u64, Sha256Digest, CanonicalWorkspaceManifestProjection)> {
    let capability = CapabilityKey::ExportWorkspace;
    monitor.check(true)?;
    let revision = map_sql(
        workspace_revision(source_connection, &workspace_id.to_string()),
        capability,
        correlation_id,
    )?;
    let revision = u64::try_from(revision)
        .ok()
        .filter(|revision| *revision <= MAX_PORTABLE_JSON_INTEGER)
        .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    monitor.check(true)?;
    let fingerprint = schema_fingerprint(source_connection, correlation_id)?;
    monitor.check(true)?;
    let inventory =
        snapshot_evidence_blobs(source_connection, workspace_id, limits, correlation_id)?;
    let entry_count = u64::try_from(WorkspaceExportEntity::ALL.len())
        .ok()
        .and_then(|count| count.checked_add(u64::try_from(inventory.len()).ok()?))
        .and_then(|count| count.checked_add(1))
        .filter(|count| *count <= limits.max_entries.get())
        .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
    debug_assert!(entry_count >= 1);

    let archive_limits = ArchiveLimits::new(
        limits.max_archive_bytes.get(),
        limits.max_entries.get(),
        limits.max_entry_bytes.get(),
        archive_expanded_bytes,
    )
    .map_err(|error| map_archive_error(error, None, correlation_id))?;
    let monitor_problem = Rc::new(RefCell::new(None));
    let output = MonitoredArchiveOutput::new(
        destination,
        monitor,
        limits.max_archive_bytes.get(),
        Rc::clone(&monitor_problem),
    );
    let mut archive = ArchiveWriter::new(output, archive_limits)
        .map_err(|error| map_archive_error(error, Some(&monitor_problem), correlation_id))?;

    let mut streams = Vec::with_capacity(WorkspaceExportEntity::ALL.len());
    let mut content_bytes = 0_u64;
    for entity in WorkspaceExportEntity::ALL {
        monitor.check(true)?;
        let mut stream_file = scratch.create_stream_file(correlation_id)?;
        let descriptor = stream_archive_entity(
            source_connection,
            workspace_id,
            entity,
            limits,
            &mut stream_file,
            &mut || monitor.check(true),
            correlation_id,
        )?;
        stream_file
            .flush()
            .map_err(|_| storage_failure(correlation_id))?;
        let stream_size = stream_file
            .metadata()
            .map_err(|_| storage_failure(correlation_id))?
            .len();
        if stream_size != descriptor.byte_length() {
            return integrity_failure(correlation_id);
        }
        stream_file
            .seek(SeekFrom::Start(0))
            .map_err(|_| storage_failure(correlation_id))?;
        content_bytes = add_content_bytes(
            content_bytes,
            descriptor.byte_length(),
            limits,
            correlation_id,
        )?;
        let mut reader = CheckedEntryReader::new(
            stream_file,
            descriptor.byte_length(),
            descriptor.digest().clone(),
        );
        append_entry(
            &mut archive,
            &format!("{}.ndjson", entity.as_str()),
            descriptor.byte_length(),
            &mut reader,
            &monitor_problem,
            correlation_id,
        )?;
        reader.finish(correlation_id)?;
        drop(reader);
        scratch.remove_stream_file(correlation_id)?;
        streams.push(descriptor);
    }

    let mut blobs = Vec::with_capacity(inventory.len());
    for blob in &inventory {
        monitor.check(true)?;
        content_bytes = append_blob(
            &mut archive,
            root,
            blob,
            content_bytes,
            limits,
            &monitor_problem,
            correlation_id,
        )?;
        blobs.push(blob.descriptor().clone());
    }

    monitor.check(true)?;
    let manifest = WorkspaceManifest::try_new(
        workspace_id,
        revision,
        WORKSPACE_ARCHIVE_CONTRACT_VERSION.to_owned(),
        fingerprint.migration_version(),
        fingerprint.digest().clone(),
        streams,
        blobs,
    )
    .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let projection = CanonicalWorkspaceManifestProjection::try_from_application(manifest)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let manifest_bytes = projection.canonical_json_bytes();
    let manifest_size = u64::try_from(manifest_bytes.len())
        .map_err(|_| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
    if manifest_size > limits.max_entry_bytes.get() {
        return capacity_failure(correlation_id);
    }
    add_content_bytes(content_bytes, manifest_size, limits, correlation_id)?;
    append_entry(
        &mut archive,
        "manifest.json",
        manifest_size,
        manifest_bytes,
        &monitor_problem,
        correlation_id,
    )?;

    let output = archive
        .finish()
        .map_err(|error| map_archive_error(error, Some(&monitor_problem), correlation_id))?;
    let (archive_bytes, archive_digest) = output.finish(correlation_id)?;
    Ok((archive_bytes, archive_digest, projection))
}

#[derive(Clone, Copy)]
struct AdmissionBounds {
    archive_expanded_bytes: u64,
    destination_bytes: u64,
    scratch_bytes: u64,
}

impl AdmissionBounds {
    fn try_new(
        limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Self> {
        Self::try_for(limits, correlation_id, true)
    }

    fn try_stopped(
        limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Self> {
        Self::try_for(limits, correlation_id, false)
    }

    fn try_for(
        limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
        needs_snapshot: bool,
    ) -> ApplicationResult<Self> {
        let capability = CapabilityKey::ExportWorkspace;
        let minimum_entries = u64::try_from(WorkspaceExportEntity::ALL.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        if limits.max_entries.get() < minimum_entries {
            return capacity_failure(correlation_id);
        }
        let archive_expanded_bytes = limits
            .archive_expanded_ceiling()
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        let scratch_ceiling_bytes = limits
            .max_entry_bytes
            .get()
            .checked_add(limits.cleanup_reserve_bytes.get())
            .and_then(|bytes| {
                if needs_snapshot {
                    bytes.checked_add(limits.max_snapshot_bytes.get())
                } else {
                    Some(bytes)
                }
            })
            .filter(|bytes| *bytes <= limits.scratch_ceiling_bytes.get())
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        let scratch_bytes = if needs_snapshot {
            scratch_ceiling_bytes
                .checked_add(limits.max_wal_growth_bytes.get())
                .ok_or_else(|| {
                    Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
                })?
        } else {
            scratch_ceiling_bytes
        };
        let destination_bytes = archive_expanded_bytes
            .checked_add(limits.cleanup_reserve_bytes.get())
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;
        Ok(Self {
            archive_expanded_bytes,
            destination_bytes,
            scratch_bytes,
        })
    }
}

#[cfg(target_os = "linux")]
struct AnchoredOnlineRoot {
    current: File,
    scratch: File,
}

#[cfg(not(target_os = "linux"))]
struct AnchoredOnlineRoot;

#[cfg(target_os = "linux")]
impl AnchoredOnlineRoot {
    fn open(
        kernel: &SqliteKernel,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Self> {
        Self::open_locked(&kernel.inner.data_root, correlation_id)
    }

    fn open_locked(
        locked: &LockedDataRoot,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Self> {
        let data_root = locked
            .anchored_directory()
            .ok_or_else(|| unsupported_platform_problem(correlation_id))?;
        let current = open_directory_beneath(data_root, "current", correlation_id)?;
        let scratch = open_directory_beneath(&current, "scratch", correlation_id)?;
        // SQLite needs a filename so it can discover WAL state. A procfs path
        // rooted in this retained directory descriptor stays bound to the same
        // current/ inode even if the configured data-root pathname is replaced.
        let descriptor = PathBuf::from(format!("/proc/self/fd/{}", current.as_raw_fd()));
        if !fs::metadata(descriptor).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(unsupported_platform_problem(correlation_id));
        }
        Ok(Self { current, scratch })
    }

    fn open_source_connection(
        &self,
        limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Connection> {
        self.open_database_connection(limits, correlation_id, true)
    }

    fn open_authorization_connection(
        &self,
        limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Connection> {
        self.open_database_connection(limits, correlation_id, false)
    }

    fn open_database_connection(
        &self,
        limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
        pin_read_transaction: bool,
    ) -> ApplicationResult<Connection> {
        let retained =
            open_regular_beneath(&self.current, Path::new("fasti.sqlite3"), correlation_id)?;
        let retained_metadata = retained
            .metadata()
            .map_err(|_| storage_failure(correlation_id))?;
        let path = descriptor_child_path(&self.current, "fasti.sqlite3");
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| storage_failure(correlation_id))?;
        connection
            .busy_timeout(Duration::from_millis(limits.backup_step_millis.get()))
            .map_err(|_| storage_failure(correlation_id))?;
        connection
            .pragma_update(None, "query_only", "ON")
            .map_err(|_| storage_failure(correlation_id))?;
        let opened_path = connection
            .path()
            .ok_or_else(|| integrity_problem(correlation_id))?;
        let opened_metadata =
            fs::metadata(opened_path).map_err(|_| integrity_problem(correlation_id))?;
        if opened_metadata.dev() != retained_metadata.dev()
            || opened_metadata.ino() != retained_metadata.ino()
        {
            return integrity_failure(correlation_id);
        }
        if pin_read_transaction {
            connection
                .execute_batch("BEGIN DEFERRED")
                .map_err(|_| storage_failure(correlation_id))?;
        }
        connection
            .query_row("SELECT COUNT(*) FROM sqlite_schema", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|_| storage_failure(correlation_id))?;
        Ok(connection)
    }

    fn wal_bytes(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<u64> {
        match open_regular_beneath_raw(&self.current, Path::new("fasti.sqlite3-wal")) {
            Ok(file) => {
                let metadata = file
                    .metadata()
                    .map_err(|_| storage_failure(correlation_id))?;
                if !metadata.is_file() {
                    return integrity_failure(correlation_id);
                }
                Ok(metadata.len())
            }
            Err(error) if error == rustix::io::Errno::NOENT => Ok(0),
            Err(_) => Err(integrity_problem(correlation_id)),
        }
    }

    fn open_evidence_file(
        &self,
        relative_path: &Path,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<File> {
        open_regular_beneath(&self.current, relative_path, correlation_id)
    }

    fn create_scratch(
        &self,
        correlation_id: RequestCorrelationId,
        bounds: &AdmissionBounds,
    ) -> ApplicationResult<ExportScratch> {
        match rustix::fs::mkdirat(&self.scratch, "exports", rustix::fs::Mode::RWXU) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(_) => return Err(storage_failure(correlation_id)),
        }
        let exports = open_directory_beneath(&self.scratch, "exports", correlation_id)?;
        rustix::fs::fchmod(&exports, rustix::fs::Mode::RWXU)
            .map_err(|_| storage_failure(correlation_id))?;
        let sweep_lock = open_private_lock(&exports, ".sweep.lock", false, correlation_id)?;
        sweep_lock
            .lock()
            .map_err(|_| storage_failure(correlation_id))?;
        sweep_stale_exports(&exports, correlation_id)?;
        require_available_handle_bytes(&exports, bounds.scratch_bytes, correlation_id, false)?;

        for _ in 0..8 {
            let mut random = [0_u8; 8];
            getrandom::fill(&mut random).map_err(|_| storage_failure(correlation_id))?;
            let name = format!("{}-{}", correlation_id, encode_hex(&random));
            match rustix::fs::mkdirat(&exports, name.as_str(), rustix::fs::Mode::RWXU) {
                Ok(()) => {
                    let root = open_directory_beneath(&exports, name.as_str(), correlation_id)?;
                    rustix::fs::fchmod(&root, rustix::fs::Mode::RWXU)
                        .map_err(|_| storage_failure(correlation_id))?;
                    let owner = open_private_lock(&root, ".owner", true, correlation_id)?;
                    owner.lock().map_err(|_| storage_failure(correlation_id))?;
                    return Ok(ExportScratch {
                        exports,
                        root,
                        name,
                        _owner: owner,
                        correlation_id,
                        cleaned: false,
                    });
                }
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(_) => return Err(storage_failure(correlation_id)),
            }
        }
        Err(storage_failure(correlation_id))
    }

    fn require_available_bytes(
        &self,
        required: u64,
        correlation_id: RequestCorrelationId,
        started: bool,
    ) -> ApplicationResult<()> {
        require_available_handle_bytes(&self.scratch, required, correlation_id, started)
    }
}

#[cfg(not(target_os = "linux"))]
impl AnchoredOnlineRoot {
    fn open(
        _kernel: &SqliteKernel,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Self> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn open_locked(
        _locked: &LockedDataRoot,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Self> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn open_source_connection(
        &self,
        _limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Connection> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn open_authorization_connection(
        &self,
        _limits: PortabilityLimits,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<Connection> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn wal_bytes(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<u64> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn open_evidence_file(
        &self,
        _relative_path: &Path,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<File> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn create_scratch(
        &self,
        correlation_id: RequestCorrelationId,
        _bounds: &AdmissionBounds,
    ) -> ApplicationResult<ExportScratch> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn require_available_bytes(
        &self,
        _required: u64,
        correlation_id: RequestCorrelationId,
        _started: bool,
    ) -> ApplicationResult<()> {
        Err(unsupported_platform_problem(correlation_id))
    }
}

#[cfg(target_os = "linux")]
struct ExportScratch {
    exports: File,
    root: File,
    name: String,
    _owner: File,
    correlation_id: RequestCorrelationId,
    cleaned: bool,
}

#[cfg(not(target_os = "linux"))]
struct ExportScratch;

#[cfg(target_os = "linux")]
impl ExportScratch {
    fn snapshot_path(&self, _correlation_id: RequestCorrelationId) -> ApplicationResult<PathBuf> {
        Ok(descriptor_child_path(&self.root, "snapshot.sqlite3"))
    }

    fn snapshot_bytes(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<u64> {
        child_file_bytes(&self.root, "snapshot.sqlite3", correlation_id)
    }

    fn current_bytes(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<u64> {
        [
            "snapshot.sqlite3",
            "snapshot.sqlite3-journal",
            "snapshot.sqlite3-wal",
            "snapshot.sqlite3-shm",
            "stream.ndjson",
        ]
        .into_iter()
        .try_fold(0_u64, |total, name| {
            total
                .checked_add(child_file_bytes(&self.root, name, correlation_id)?)
                .ok_or_else(|| {
                    Box::new(FastiProblem::capacity_exceeded(
                        CapabilityKey::ExportWorkspace,
                        correlation_id,
                    ))
                })
        })
    }

    fn create_stream_file(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<File> {
        let fd = rustix::fs::openat2(
            &self.root,
            "stream.ndjson",
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            secure_resolve_flags(),
        )
        .map_err(|_| storage_failure(correlation_id))?;
        Ok(File::from(fd))
    }

    fn open_snapshot_file(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<File> {
        open_regular_beneath(&self.root, Path::new("snapshot.sqlite3"), correlation_id)
    }

    fn remove_stream_file(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<()> {
        rustix::fs::unlinkat(&self.root, "stream.ndjson", rustix::fs::AtFlags::empty())
            .map_err(|_| storage_failure(correlation_id))
    }

    fn cleanup(&mut self, correlation_id: RequestCorrelationId) -> ApplicationResult<()> {
        cleanup_export_attempt(&self.exports, &self.root, &self.name, correlation_id)?;
        self.cleaned = true;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for ExportScratch {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ =
                cleanup_export_attempt(&self.exports, &self.root, &self.name, self.correlation_id);
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl ExportScratch {
    fn snapshot_path(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<PathBuf> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn snapshot_bytes(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<u64> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn current_bytes(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<u64> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn create_stream_file(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<File> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn open_snapshot_file(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<File> {
        Err(unsupported_platform_problem(correlation_id))
    }

    fn remove_stream_file(&self, correlation_id: RequestCorrelationId) -> ApplicationResult<()> {
        Err(unsupported_platform_problem(correlation_id))
    }
}

#[cfg(target_os = "linux")]
fn secure_resolve_flags() -> rustix::fs::ResolveFlags {
    rustix::fs::ResolveFlags::BENEATH
        | rustix::fs::ResolveFlags::NO_MAGICLINKS
        | rustix::fs::ResolveFlags::NO_SYMLINKS
        | rustix::fs::ResolveFlags::NO_XDEV
}

#[cfg(target_os = "linux")]
fn open_directory_beneath(
    parent: &File,
    relative: &str,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<File> {
    let fd = rustix::fs::openat2(
        parent,
        relative,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
        secure_resolve_flags(),
    )
    .map_err(|error| {
        if error == rustix::io::Errno::NOSYS {
            unsupported_platform_problem(correlation_id)
        } else {
            integrity_problem(correlation_id)
        }
    })?;
    Ok(File::from(fd))
}

#[cfg(target_os = "linux")]
fn open_regular_beneath_raw(parent: &File, relative: &Path) -> Result<File, rustix::io::Errno> {
    let fd = rustix::fs::openat2(
        parent,
        relative,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
        secure_resolve_flags(),
    )?;
    Ok(File::from(fd))
}

#[cfg(target_os = "linux")]
fn open_regular_beneath(
    parent: &File,
    relative: &Path,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<File> {
    open_regular_beneath_raw(parent, relative).map_err(|error| {
        if error == rustix::io::Errno::NOSYS {
            unsupported_platform_problem(correlation_id)
        } else {
            integrity_problem(correlation_id)
        }
    })
}

#[cfg(target_os = "linux")]
fn open_private_lock(
    parent: &File,
    name: &str,
    exclusive: bool,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<File> {
    let mut flags = rustix::fs::OFlags::RDWR
        | rustix::fs::OFlags::CREATE
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::NOFOLLOW;
    if exclusive {
        flags |= rustix::fs::OFlags::EXCL;
    }
    let fd = rustix::fs::openat2(
        parent,
        name,
        flags,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        secure_resolve_flags(),
    )
    .map_err(|_| storage_failure(correlation_id))?;
    let file = File::from(fd);
    if !file
        .metadata()
        .map_err(|_| storage_failure(correlation_id))?
        .is_file()
    {
        return integrity_failure(correlation_id);
    }
    Ok(file)
}

#[cfg(target_os = "linux")]
fn sweep_stale_exports(
    exports: &File,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let directory = PathBuf::from(format!("/proc/self/fd/{}", exports.as_raw_fd()));
    for entry in fs::read_dir(directory).map_err(|_| storage_failure(correlation_id))? {
        let entry = entry.map_err(|_| storage_failure(correlation_id))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| integrity_problem(correlation_id))?;
        if name == ".sweep.lock" {
            continue;
        }
        let attempt = open_directory_beneath(exports, &name, correlation_id)?;
        let owner = open_private_lock(&attempt, ".owner", false, correlation_id)?;
        match owner.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => continue,
            Err(std::fs::TryLockError::Error(_)) => {
                return Err(storage_failure(correlation_id));
            }
        }
        cleanup_export_attempt(exports, &attempt, &name, correlation_id)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn cleanup_export_attempt(
    exports: &File,
    attempt: &File,
    name: &str,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    for child in [
        "stream.ndjson",
        "snapshot.sqlite3-shm",
        "snapshot.sqlite3-wal",
        "snapshot.sqlite3-journal",
        "snapshot.sqlite3",
        ".owner",
    ] {
        match rustix::fs::unlinkat(attempt, child, rustix::fs::AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(_) => return Err(storage_failure(correlation_id)),
        }
    }
    rustix::fs::unlinkat(exports, name, rustix::fs::AtFlags::REMOVEDIR)
        .map_err(|_| storage_failure(correlation_id))
}

#[cfg(target_os = "linux")]
fn child_file_bytes(
    parent: &File,
    name: &str,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<u64> {
    match open_regular_beneath_raw(parent, Path::new(name)) {
        Ok(file) => {
            let metadata = file
                .metadata()
                .map_err(|_| storage_failure(correlation_id))?;
            if !metadata.is_file() {
                return integrity_failure(correlation_id);
            }
            Ok(metadata.len())
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(0),
        Err(_) => Err(integrity_problem(correlation_id)),
    }
}

#[cfg(target_os = "linux")]
fn descriptor_child_path(directory: &File, name: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}/{}", directory.as_raw_fd(), name))
}

#[cfg(target_os = "linux")]
fn require_available_handle_bytes(
    directory: &File,
    required: u64,
    correlation_id: RequestCorrelationId,
    started: bool,
) -> ApplicationResult<()> {
    let stats = rustix::fs::fstatvfs(directory).map_err(|_| storage_failure(correlation_id))?;
    let available = stats.f_bavail.checked_mul(stats.f_frsize).ok_or_else(|| {
        Box::new(FastiProblem::capacity_exceeded(
            CapabilityKey::ExportWorkspace,
            correlation_id,
        ))
    })?;
    if available < required {
        return if started {
            stopped_node_failure(correlation_id)
        } else {
            capacity_failure(correlation_id)
        };
    }
    Ok(())
}

struct DestinationGuard {
    destination: Option<Box<dyn WorkspaceArchiveDestination>>,
}

impl DestinationGuard {
    fn new(destination: Box<dyn WorkspaceArchiveDestination>) -> Self {
        Self {
            destination: Some(destination),
        }
    }

    fn preflight(&self, required_bytes: u64) -> io::Result<()> {
        self.destination
            .as_ref()
            .ok_or_else(|| io::Error::other("archive destination is no longer available"))?
            .preflight(required_bytes)
    }

    fn complete(
        &mut self,
        archive_digest: &Sha256Digest,
        manifest_digest: &Sha256Digest,
    ) -> Result<(), WorkspaceArchiveCompletionError> {
        self.destination
            .take()
            .ok_or_else(|| {
                WorkspaceArchiveCompletionError::Discarded(io::Error::other(
                    "archive destination is no longer available",
                ))
            })?
            .complete(archive_digest, manifest_digest)
    }

    fn abort_problem(
        &mut self,
        problem: Box<FastiProblem>,
        correlation_id: RequestCorrelationId,
    ) -> (Box<FastiProblem>, FailedArchiveDestinationState) {
        match self.destination.take() {
            Some(destination) => abort_destination_problem(destination, problem, correlation_id),
            None => (problem, FailedArchiveDestinationState::Discarded),
        }
    }
}

pub(crate) fn abort_destination_problem(
    destination: Box<dyn WorkspaceArchiveDestination>,
    problem: Box<FastiProblem>,
    correlation_id: RequestCorrelationId,
) -> (Box<FastiProblem>, FailedArchiveDestinationState) {
    if destination.abort().is_ok() {
        (problem, FailedArchiveDestinationState::Discarded)
    } else {
        (
            storage_failure(correlation_id),
            FailedArchiveDestinationState::PartialCleanupIndeterminate,
        )
    }
}

impl Write for DestinationGuard {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.destination
            .as_mut()
            .ok_or_else(|| io::Error::other("archive destination is no longer available"))?
            .write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.destination
            .as_mut()
            .ok_or_else(|| io::Error::other("archive destination is no longer available"))?
            .flush()
    }
}

impl Drop for DestinationGuard {
    fn drop(&mut self) {
        if let Some(destination) = self.destination.take() {
            let _ = destination.abort();
        }
    }
}

struct MonitoredArchiveOutput<'a> {
    destination: &'a mut DestinationGuard,
    monitor: &'a dyn ExportMonitor,
    max_archive_bytes: u64,
    problem: Rc<RefCell<Option<Box<FastiProblem>>>>,
    hasher: Sha256,
    bytes: u64,
}

impl<'a> MonitoredArchiveOutput<'a> {
    fn new(
        destination: &'a mut DestinationGuard,
        monitor: &'a dyn ExportMonitor,
        max_archive_bytes: u64,
        problem: Rc<RefCell<Option<Box<FastiProblem>>>>,
    ) -> Self {
        Self {
            destination,
            monitor,
            max_archive_bytes,
            problem,
            hasher: Sha256::new(),
            bytes: 0,
        }
    }

    fn finish(
        self,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<(u64, Sha256Digest)> {
        let digest: [u8; 32] = self.hasher.finalize().into();
        let digest =
            Sha256Digest::parse(format!("sha256:{}", encode_hex(&digest))).map_err(|_| {
                Box::new(FastiProblem::integrity_failed(
                    CapabilityKey::ExportWorkspace,
                    correlation_id,
                ))
            })?;
        Ok((self.bytes, digest))
    }
}

impl Write for MonitoredArchiveOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > ARCHIVE_MAX_IO_CHUNK_BYTES {
            return Err(io::Error::other(
                "archive writer exceeded the bounded I/O chunk",
            ));
        }
        if let Err(problem) = self.monitor.check(true) {
            *self.problem.borrow_mut() = Some(problem);
            return Err(io::Error::other("archive output was canceled"));
        }
        let next = self
            .bytes
            .checked_add(
                u64::try_from(bytes.len())
                    .map_err(|_| io::Error::other("archive byte count overflow"))?,
            )
            .ok_or_else(|| io::Error::other("archive byte count overflow"))?;
        if next > self.max_archive_bytes {
            return Err(io::Error::other("archive byte ceiling exceeded"));
        }
        self.destination.write_all(bytes)?;
        self.hasher.update(bytes);
        self.bytes = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.destination.flush()
    }
}

struct CheckedEntryReader {
    file: File,
    expected_bytes: u64,
    expected_digest: Sha256Digest,
    bytes: u64,
    hasher: Sha256,
}

impl CheckedEntryReader {
    fn new(file: File, expected_bytes: u64, expected_digest: Sha256Digest) -> Self {
        Self {
            file,
            expected_bytes,
            expected_digest,
            bytes: 0,
            hasher: Sha256::new(),
        }
    }

    fn finish(&mut self, correlation_id: RequestCorrelationId) -> ApplicationResult<()> {
        let mut extra = [0_u8; 1];
        if self
            .file
            .read(&mut extra)
            .map_err(|_| storage_failure(correlation_id))?
            != 0
            || self.bytes != self.expected_bytes
        {
            return integrity_failure(correlation_id);
        }
        let digest: [u8; 32] = self.hasher.clone().finalize().into();
        let actual = Sha256Digest::parse(format!("sha256:{}", encode_hex(&digest)))
            .map_err(|_| integrity_problem(correlation_id))?;
        if actual != self.expected_digest {
            return integrity_failure(correlation_id);
        }
        Ok(())
    }
}

impl Read for CheckedEntryReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.file.read(bytes)?;
        self.bytes = self
            .bytes
            .checked_add(
                u64::try_from(count).map_err(|_| io::Error::other("entry byte count overflow"))?,
            )
            .ok_or_else(|| io::Error::other("entry byte count overflow"))?;
        self.hasher.update(&bytes[..count]);
        Ok(count)
    }
}

fn append_blob<W: Write>(
    archive: &mut ArchiveWriter<W>,
    root: &AnchoredOnlineRoot,
    blob: &SnapshotEvidenceBlob,
    content_bytes: u64,
    limits: PortabilityLimits,
    monitor_problem: &Rc<RefCell<Option<Box<FastiProblem>>>>,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<u64> {
    let file = root.open_evidence_file(blob.relative_path(), correlation_id)?;
    let metadata = file
        .metadata()
        .map_err(|_| storage_failure(correlation_id))?;
    if !metadata.is_file() || metadata.len() != blob.descriptor().byte_length() {
        return integrity_failure(correlation_id);
    }
    let mut reader = CheckedEntryReader::new(
        file,
        blob.descriptor().byte_length(),
        blob.descriptor().digest().clone(),
    );
    append_entry(
        archive,
        &path_to_archive_value(blob.relative_path(), correlation_id)?,
        blob.descriptor().byte_length(),
        &mut reader,
        monitor_problem,
        correlation_id,
    )?;
    reader.finish(correlation_id)?;
    add_content_bytes(
        content_bytes,
        blob.descriptor().byte_length(),
        limits,
        correlation_id,
    )
}

fn append_entry<W: Write, R: Read>(
    archive: &mut ArchiveWriter<W>,
    path: &str,
    size: u64,
    reader: R,
    monitor_problem: &Rc<RefCell<Option<Box<FastiProblem>>>>,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    archive
        .append(path, size, reader)
        .map_err(|error| map_archive_error(error, Some(monitor_problem), correlation_id))
}

fn add_content_bytes(
    current: u64,
    entry: u64,
    limits: PortabilityLimits,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<u64> {
    current
        .checked_add(entry)
        .filter(|bytes| *bytes <= limits.max_uncompressed_bytes.get())
        .ok_or_else(|| {
            Box::new(FastiProblem::capacity_exceeded(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            ))
        })
}

fn monitor_export(
    kernel: &SqliteKernel,
    request: &ExportWorkspaceRequest,
    started: bool,
) -> ApplicationResult<()> {
    let capability = CapabilityKey::ExportWorkspace;
    let correlation_id = request.query().correlation_id();
    if request.cancellation().is_cancelled() {
        return Err(Box::new(FastiProblem::export_canceled(correlation_id)));
    }
    let mut connection = kernel.lock_connection(capability, correlation_id)?;
    let transaction = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Deferred),
        capability,
        correlation_id,
    )?;
    if let Err(problem) = authorize_transaction(
        &transaction,
        capability,
        request.query().access(),
        correlation_id,
    ) {
        if started && problem.code() == ProblemCode::Forbidden {
            return stopped_node_failure(correlation_id);
        }
        return Err(problem);
    }
    map_sql(transaction.commit(), capability, correlation_id)?;
    Ok(())
}

fn monitor_snapshot_resources(
    root: &AnchoredOnlineRoot,
    scratch: &ExportScratch,
    initial_wal_bytes: u64,
    limits: PortabilityLimits,
    correlation_id: RequestCorrelationId,
    started: bool,
    snapshot_can_grow: bool,
) -> ApplicationResult<()> {
    let snapshot_bytes = scratch.snapshot_bytes(correlation_id)?;
    let current_wal_bytes = root.wal_bytes(correlation_id)?;
    let wal_growth = current_wal_bytes.saturating_sub(initial_wal_bytes);
    if snapshot_bytes > limits.max_snapshot_bytes.get()
        || wal_growth > limits.max_wal_growth_bytes.get()
    {
        return if started {
            stopped_node_failure(correlation_id)
        } else {
            capacity_failure(correlation_id)
        };
    }
    let pending_snapshot = if snapshot_can_grow {
        limits.max_snapshot_bytes.get() - snapshot_bytes
    } else {
        0
    };
    let pending_wal = if snapshot_can_grow {
        limits.max_wal_growth_bytes.get() - wal_growth
    } else {
        0
    };
    let pending_scratch = pending_snapshot
        .checked_add(limits.max_entry_bytes.get())
        .and_then(|bytes| bytes.checked_add(limits.cleanup_reserve_bytes.get()))
        .ok_or_else(|| {
            Box::new(FastiProblem::capacity_exceeded(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            ))
        })?;
    let pending_filesystem = pending_scratch.checked_add(pending_wal).ok_or_else(|| {
        Box::new(FastiProblem::capacity_exceeded(
            CapabilityKey::ExportWorkspace,
            correlation_id,
        ))
    })?;
    let scratch_bytes = scratch
        .current_bytes(correlation_id)?
        .checked_add(pending_scratch)
        .filter(|bytes| *bytes <= limits.scratch_ceiling_bytes.get())
        .ok_or_else(|| {
            if started {
                Box::new(FastiProblem::stopped_node_export_required(correlation_id))
            } else {
                Box::new(FastiProblem::capacity_exceeded(
                    CapabilityKey::ExportWorkspace,
                    correlation_id,
                ))
            }
        })?;
    debug_assert!(scratch_bytes >= pending_scratch);
    root.require_available_bytes(pending_filesystem, correlation_id, started)
}

fn snapshot_limits(
    limits: PortabilityLimits,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<SnapshotLimits> {
    let pages = u32::try_from(limits.backup_step_pages.get())
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or_else(|| {
            Box::new(FastiProblem::capacity_exceeded(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            ))
        })?;
    let max_steps = limits
        .max_snapshot_bytes
        .get()
        .div_ceil(SQLITE_MINIMUM_PAGE_BYTES)
        .div_ceil(u64::from(pages.get()))
        .checked_add(1)
        .ok_or_else(|| {
            Box::new(FastiProblem::capacity_exceeded(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            ))
        })?;
    let total_millis = limits
        .backup_step_millis
        .get()
        .checked_mul(max_steps)
        .ok_or_else(|| {
            Box::new(FastiProblem::capacity_exceeded(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            ))
        })?;
    SnapshotLimits::new(
        pages,
        Duration::from_millis(limits.backup_step_millis.get()),
        Duration::from_millis(total_millis),
    )
    .map_err(|_| {
        Box::new(FastiProblem::capacity_exceeded(
            CapabilityKey::ExportWorkspace,
            correlation_id,
        ))
    })
}

fn database_logical_bytes(
    connection: &Connection,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<u64> {
    let capability = CapabilityKey::ExportWorkspace;
    let page_count = map_sql(
        connection.query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0)),
        capability,
        correlation_id,
    )?;
    let page_size = map_sql(
        connection.query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0)),
        capability,
        correlation_id,
    )?;
    u64::try_from(page_count)
        .ok()
        .and_then(|count| count.checked_mul(u64::try_from(page_size).ok()?))
        .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

fn open_read_only_snapshot(
    file: &File,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Connection> {
    #[cfg(target_os = "linux")]
    let path = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
    #[cfg(not(target_os = "linux"))]
    let path: PathBuf = {
        let _ = file;
        return Err(unsupported_platform_problem(correlation_id));
    };
    let connection = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| storage_failure(correlation_id))?;
    connection
        .pragma_update(None, "query_only", "ON")
        .map_err(|_| storage_failure(correlation_id))?;
    Ok(connection)
}

fn path_to_archive_value(
    path: &Path,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<String> {
    let value = path
        .to_str()
        .ok_or_else(|| integrity_problem(correlation_id))?;
    if value.as_bytes().contains(&b'\\') {
        return integrity_failure(correlation_id);
    }
    Ok(value.to_owned())
}

fn map_snapshot_error(
    error: SnapshotError,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    match error {
        SnapshotError::Cancelled
        | SnapshotError::Busy(_)
        | SnapshotError::StepTimeout
        | SnapshotError::OverallTimeout => {
            Box::new(FastiProblem::stopped_node_export_required(correlation_id))
        }
        SnapshotError::SchemaVersion { .. }
        | SnapshotError::Integrity(_)
        | SnapshotError::InvalidProgress => integrity_problem(correlation_id),
        SnapshotError::InvalidLimit(_) => Box::new(FastiProblem::capacity_exceeded(
            CapabilityKey::ExportWorkspace,
            correlation_id,
        )),
        SnapshotError::DestinationExists(_)
        | SnapshotError::Io(_)
        | SnapshotError::Cleanup { .. }
        | SnapshotError::Sqlite(_)
        | SnapshotError::Store(_) => storage_failure(correlation_id),
    }
}

fn map_archive_error(
    error: ArchiveError,
    monitor_problem: Option<&Rc<RefCell<Option<Box<FastiProblem>>>>>,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    if let Some(problem) = monitor_problem.and_then(|problem| problem.borrow_mut().take()) {
        return problem;
    }
    match error {
        ArchiveError::EntryCountExceeded { .. }
        | ArchiveError::EntrySizeExceeded { .. }
        | ArchiveError::ExpandedSizeExceeded { .. }
        | ArchiveError::CompressedSizeExceeded { .. }
        | ArchiveError::InvalidLimits => Box::new(FastiProblem::capacity_exceeded(
            CapabilityKey::ExportWorkspace,
            correlation_id,
        )),
        ArchiveError::Io(_) => storage_failure(correlation_id),
        _ => integrity_problem(correlation_id),
    }
}

pub(crate) fn online_receipt_with_destination_state(
    request: &ExportWorkspaceRequest,
    problem: Box<FastiProblem>,
    destination_state: FailedArchiveDestinationState,
) -> PortabilityFailureReceipt {
    PortabilityFailureReceipt::try_online_export_with_destination_state(
        request,
        problem,
        destination_state,
    )
    .expect("online archive failures always name ExportWorkspace")
}

fn stopped_receipt_with_destination_state(
    request: &StoppedNodeExportRequest,
    problem: Box<FastiProblem>,
    destination_state: FailedArchiveDestinationState,
) -> PortabilityFailureReceipt {
    PortabilityFailureReceipt::try_stopped_node_export_with_destination_state(
        request,
        problem,
        destination_state,
    )
    .expect("stopped-node archive failures always name ExportWorkspace")
}

fn capacity_failure<T>(correlation_id: RequestCorrelationId) -> ApplicationResult<T> {
    Err(Box::new(FastiProblem::capacity_exceeded(
        CapabilityKey::ExportWorkspace,
        correlation_id,
    )))
}

fn stopped_node_failure<T>(correlation_id: RequestCorrelationId) -> ApplicationResult<T> {
    Err(Box::new(FastiProblem::stopped_node_export_required(
        correlation_id,
    )))
}

fn integrity_failure<T>(correlation_id: RequestCorrelationId) -> ApplicationResult<T> {
    Err(integrity_problem(correlation_id))
}

fn integrity_problem(correlation_id: RequestCorrelationId) -> Box<FastiProblem> {
    Box::new(FastiProblem::integrity_failed(
        CapabilityKey::ExportWorkspace,
        correlation_id,
    ))
}

fn storage_failure(correlation_id: RequestCorrelationId) -> Box<FastiProblem> {
    Box::new(FastiProblem::storage_unavailable(
        CapabilityKey::ExportWorkspace,
        correlation_id,
    ))
}

fn unsupported_platform_problem(correlation_id: RequestCorrelationId) -> Box<FastiProblem> {
    Box::new(FastiProblem::unsupported_platform(
        CapabilityKey::ExportWorkspace,
        correlation_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{visit_archive_entries, FilesystemArchiveDestination};
    use crate::kernel::{prepare_private_directory, scope_storage_key};
    use crate::test_support::TestNode;
    use fasti_application::{
        CancellationSignal, ExportWorkspaceQuery, ScopeKey, StoppedNodeExportRequest,
    };
    use fasti_contracts::ChecksummedWorkspaceManifestDto;
    use fasti_domain::RecordId;
    use rusqlite::params;
    use std::io::Cursor;
    use std::num::NonZeroU64;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct DestinationState {
        required_bytes: Option<u64>,
        bytes: Vec<u8>,
        maximum_write: usize,
        completed: bool,
        aborted: bool,
        archive_digest: Option<Sha256Digest>,
        manifest_digest: Option<Sha256Digest>,
    }

    struct TestDestination {
        state: Arc<Mutex<DestinationState>>,
        preflight_action: Mutex<Option<Box<dyn FnOnce() + Send>>>,
        write_action: Option<Box<dyn FnOnce() + Send>>,
        flush_action: Option<Box<dyn FnOnce() + Send>>,
        abort_fails: bool,
        completion_indeterminate: bool,
    }

    struct CleanupOnFailedCompleteDestination {
        path: PathBuf,
        file: Option<File>,
    }

    impl CleanupOnFailedCompleteDestination {
        fn create(path: PathBuf) -> Self {
            let file = File::create(&path).expect("partial destination");
            Self {
                path,
                file: Some(file),
            }
        }

        fn cleanup(&mut self) -> io::Result<()> {
            self.file.take();
            match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }
    }

    impl Write for CleanupOnFailedCompleteDestination {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.file
                .as_mut()
                .ok_or_else(|| io::Error::other("partial destination is closed"))?
                .write(bytes)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.file
                .as_mut()
                .ok_or_else(|| io::Error::other("partial destination is closed"))?
                .flush()
        }
    }

    impl WorkspaceArchiveDestination for CleanupOnFailedCompleteDestination {
        fn preflight(&self, _required_bytes: u64) -> io::Result<()> {
            Ok(())
        }

        fn complete(
            mut self: Box<Self>,
            _archive_digest: &Sha256Digest,
            _manifest_digest: &Sha256Digest,
        ) -> Result<(), fasti_application::WorkspaceArchiveCompletionError> {
            self.cleanup().map_err(
                fasti_application::WorkspaceArchiveCompletionError::PartialCleanupIndeterminate,
            )?;
            Err(io::Error::other("simulated publication failure").into())
        }

        fn abort(mut self: Box<Self>) -> io::Result<()> {
            self.cleanup()
        }
    }

    impl TestDestination {
        fn new(state: Arc<Mutex<DestinationState>>) -> Self {
            Self {
                state,
                preflight_action: Mutex::new(None),
                write_action: None,
                flush_action: None,
                abort_fails: false,
                completion_indeterminate: false,
            }
        }

        fn on_preflight(mut self, action: impl FnOnce() + Send + 'static) -> Self {
            self.preflight_action = Mutex::new(Some(Box::new(action)));
            self
        }

        fn on_first_write(mut self, action: impl FnOnce() + Send + 'static) -> Self {
            self.write_action = Some(Box::new(action));
            self
        }

        fn on_flush(mut self, action: impl FnOnce() + Send + 'static) -> Self {
            self.flush_action = Some(Box::new(action));
            self
        }

        fn with_failed_abort(mut self) -> Self {
            self.abort_fails = true;
            self
        }

        fn with_indeterminate_completion(mut self) -> Self {
            self.completion_indeterminate = true;
            self
        }
    }

    impl Write for TestDestination {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Some(action) = self.write_action.take() {
                action();
            }
            let mut state = self.state.lock().expect("destination state");
            state.maximum_write = state.maximum_write.max(bytes.len());
            state.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if let Some(action) = self.flush_action.take() {
                action();
            }
            Ok(())
        }
    }

    impl WorkspaceArchiveDestination for TestDestination {
        fn preflight(&self, required_bytes: u64) -> io::Result<()> {
            self.state.lock().expect("destination state").required_bytes = Some(required_bytes);
            let action = self
                .preflight_action
                .lock()
                .expect("preflight action")
                .take();
            if let Some(action) = action {
                action();
            }
            Ok(())
        }

        fn complete(
            self: Box<Self>,
            archive_digest: &Sha256Digest,
            manifest_digest: &Sha256Digest,
        ) -> Result<(), fasti_application::WorkspaceArchiveCompletionError> {
            let mut state = self.state.lock().expect("destination state");
            state.completed = true;
            state.archive_digest = Some(archive_digest.clone());
            state.manifest_digest = Some(manifest_digest.clone());
            if self.completion_indeterminate {
                Err(
                    WorkspaceArchiveCompletionError::PublishedDurabilityIndeterminate(
                        io::Error::other("simulated directory sync failure"),
                    ),
                )
            } else {
                Ok(())
            }
        }

        fn abort(self: Box<Self>) -> io::Result<()> {
            self.state.lock().expect("destination state").aborted = true;
            if self.abort_fails {
                Err(io::Error::other("simulated abort failure"))
            } else {
                Ok(())
            }
        }
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

    fn nonzero(value: u64) -> NonZeroU64 {
        NonZeroU64::new(value).expect("non-zero test limit")
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
            .expect("grant staged export scope");
    }

    fn request(node: &TestNode, cancellation: CancellationSignal) -> ExportWorkspaceRequest {
        request_with_limits(node, cancellation, limits())
    }

    fn request_with_limits(
        node: &TestNode,
        cancellation: CancellationSignal,
        limits: PortabilityLimits,
    ) -> ExportWorkspaceRequest {
        ExportWorkspaceRequest::new(
            ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), node.access),
            limits,
            cancellation,
        )
    }

    fn stopped_request(
        access: fasti_application::RequestAccessContext,
        cancellation: CancellationSignal,
        limits: PortabilityLimits,
    ) -> StoppedNodeExportRequest {
        StoppedNodeExportRequest::new(
            ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), access),
            limits,
            cancellation,
        )
    }

    fn export(
        node: &TestNode,
        destination: TestDestination,
    ) -> PortabilityResult<WorkspaceArchiveExportOutcome> {
        export_online_workspace_archive(
            &node.kernel,
            request(node, CancellationSignal::new()),
            Box::new(destination),
        )
    }

    fn archive_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let limits = ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
            .expect("archive limits");
        let mut entries = Vec::new();
        visit_archive_entries(Cursor::new(bytes), limits, |path, _size, reader| {
            let mut content = Vec::new();
            reader.read_to_end(&mut content)?;
            entries.push((path.to_owned(), content));
            Ok::<(), ArchiveError>(())
        })
        .expect("valid produced archive");
        entries
    }

    fn digest(bytes: &[u8]) -> Sha256Digest {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        Sha256Digest::parse(format!("sha256:{}", encode_hex(&digest))).expect("digest")
    }

    fn assert_scratch_clean(node: &TestNode) {
        assert_scratch_path_clean(&node.kernel.inner.current_root);
    }

    fn assert_scratch_path_clean(current_root: &Path) {
        let exports = current_root.join("scratch").join("exports");
        if exports.exists() {
            let mut attempts = fs::read_dir(exports)
                .expect("export scratch directory")
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name() != ".sweep.lock");
            assert!(attempts.next().is_none());
        }
    }

    #[test]
    fn real_archive_is_deterministic_and_binds_every_descriptor() {
        let node = TestNode::new();
        grant_export(&node);
        let evidence = b"portable evidence bytes";
        node.upload(evidence);

        let first_state = Arc::new(Mutex::new(DestinationState::default()));
        let first =
            export(&node, TestDestination::new(Arc::clone(&first_state))).expect("first archive");
        let second_state = Arc::new(Mutex::new(DestinationState::default()));
        let second =
            export(&node, TestDestination::new(Arc::clone(&second_state))).expect("second archive");
        let first_state = first_state.lock().expect("first destination state");
        let second_state = second_state.lock().expect("second destination state");

        assert_eq!(first_state.bytes, second_state.bytes);
        assert_eq!(first.archive_digest(), second.archive_digest());
        assert_eq!(first.archive_bytes(), first_state.bytes.len() as u64);
        assert_eq!(first.archive_digest(), &digest(&first_state.bytes));
        assert_eq!(
            first_state.archive_digest.as_ref(),
            Some(first.archive_digest())
        );
        assert_eq!(
            first_state.manifest_digest.as_ref(),
            Some(first.manifest_digest())
        );
        assert!(first_state.completed);
        assert!(!first_state.aborted);
        assert!(first_state.required_bytes.is_some());
        assert!(first_state.maximum_write <= ARCHIVE_MAX_IO_CHUNK_BYTES);

        let entries = archive_entries(&first_state.bytes);
        let expected_stream_paths: Vec<String> = WorkspaceExportEntity::ALL
            .into_iter()
            .map(|entity| format!("{}.ndjson", entity.as_str()))
            .collect();
        assert_eq!(
            entries[..WorkspaceExportEntity::ALL.len()]
                .iter()
                .map(|(path, _)| path)
                .cloned()
                .collect::<Vec<_>>(),
            expected_stream_paths
        );
        assert!(entries[WorkspaceExportEntity::ALL.len()]
            .0
            .starts_with("payloads/sha256/"));
        assert_eq!(
            entries.last().map(|entry| entry.0.as_str()),
            Some("manifest.json")
        );

        let manifest_bytes = &entries.last().expect("manifest entry").1;
        let manifest: ChecksummedWorkspaceManifestDto =
            serde_json::from_slice(manifest_bytes).expect("manifest DTO");
        assert_eq!(
            manifest.manifest.contract_version,
            WORKSPACE_ARCHIVE_CONTRACT_VERSION
        );
        assert_eq!(
            manifest.manifest.workspace_revision,
            first.workspace_revision()
        );
        assert_eq!(
            manifest.manifest.streams.len(),
            WorkspaceExportEntity::ALL.len()
        );
        for (descriptor, (_, stream_bytes)) in manifest
            .manifest
            .streams
            .iter()
            .zip(entries.iter().take(WorkspaceExportEntity::ALL.len()))
        {
            assert_eq!(descriptor.byte_length, stream_bytes.len() as u64);
            assert_eq!(descriptor.digest, digest(stream_bytes).as_str());
            assert_eq!(
                descriptor.row_count,
                stream_bytes.iter().filter(|byte| **byte == b'\n').count() as u64
            );
        }
        assert_eq!(manifest.manifest.blobs.len(), 1);
        assert_eq!(
            manifest.manifest.blobs[0].byte_length,
            evidence.len() as u64
        );
        assert_eq!(manifest.manifest.blobs[0].digest, digest(evidence).as_str());
        assert_eq!(entries[WorkspaceExportEntity::ALL.len()].1, evidence);
        assert_eq!(manifest.manifest_digest, first.manifest_digest().as_str());
        manifest
            .try_into_application(limits())
            .expect("strict manifest conversion");
        assert_scratch_clean(&node);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn real_filesystem_destination_publishes_one_verified_archive() {
        let node = TestNode::new();
        grant_export(&node);
        let root = tempfile::tempdir().expect("archive destination root");
        let path = root.path().join("workspace.fasti");
        let outcome = export_online_workspace_archive(
            &node.kernel,
            request(&node, CancellationSignal::new()),
            Box::new(FilesystemArchiveDestination::new(&path).expect("filesystem destination")),
        )
        .expect("filesystem export");

        let bytes = fs::read(&path).expect("published archive");
        assert_eq!(outcome.archive_bytes(), bytes.len() as u64);
        assert_eq!(outcome.archive_digest(), &digest(&bytes));
        assert!(!archive_entries(&bytes).is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stopped_node_archive_is_byte_identical_to_online_for_the_same_state() {
        let node = TestNode::new();
        grant_export(&node);
        node.upload(b"same immutable evidence");
        let online_state = Arc::new(Mutex::new(DestinationState::default()));
        let online =
            export(&node, TestDestination::new(Arc::clone(&online_state))).expect("online archive");
        let online_bytes = online_state
            .lock()
            .expect("online destination")
            .bytes
            .clone();
        let (root, access) = node.into_stopped();
        let current = root.path().join("current");

        let stale = current.join("scratch/exports/stale-completed-process");
        prepare_private_directory(&stale).expect("stale stopped export");
        fs::write(stale.join("stream.ndjson"), b"stale stream").expect("stale stream");

        let stopped_state = Arc::new(Mutex::new(DestinationState::default()));
        let stopped = export_stopped_node_workspace_archive(
            root.path(),
            stopped_request(access, CancellationSignal::new(), limits()),
            Box::new(TestDestination::new(Arc::clone(&stopped_state))),
        )
        .expect("stopped-node archive");
        let stopped_state = stopped_state.lock().expect("stopped destination");

        assert_eq!(stopped_state.bytes, online_bytes);
        assert_eq!(stopped.archive_digest(), online.archive_digest());
        assert_eq!(stopped.manifest_digest(), online.manifest_digest());
        assert_eq!(stopped.workspace_revision(), online.workspace_revision());
        assert_eq!(stopped.archive_bytes(), online.archive_bytes());
        assert!(stopped_state.completed);
        assert!(!stopped_state.aborted);
        assert!(stopped_state.maximum_write <= ARCHIVE_MAX_IO_CHUNK_BYTES);
        let expected_destination = limits().archive_expanded_ceiling().expect("archive bound")
            + limits().cleanup_reserve_bytes.get();
        assert_eq!(stopped_state.required_bytes, Some(expected_destination));
        assert!(!stale.exists(), "owner-free stale scratch was swept");
        drop(stopped_state);
        assert_scratch_path_clean(&current);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stopped_node_export_refuses_a_live_kernel_lock() {
        let node = TestNode::new();
        grant_export(&node);
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let failure = export_stopped_node_workspace_archive(
            node.kernel.data_root(),
            stopped_request(node.access, CancellationSignal::new(), limits()),
            Box::new(TestDestination::new(Arc::clone(&state))),
        )
        .expect_err("live daemon lock must exclude stopped export");

        assert_eq!(failure.problem().code(), ProblemCode::DataRootLocked);
        assert_eq!(
            failure.archive_destination_state(),
            Some(FailedArchiveDestinationState::Discarded)
        );
        let state = state.lock().expect("destination state");
        assert!(state.aborted);
        assert!(!state.completed);
        assert!(state.bytes.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn live_archive_port_aborts_stopped_mode_without_resolving_its_path() {
        let node = TestNode::new();
        grant_export(&node);
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let failure = WorkspaceArchiveExportPort::export_stopped_node_workspace_archive(
            &node.kernel,
            stopped_request(node.access, CancellationSignal::new(), limits()),
            Box::new(TestDestination::new(Arc::clone(&state))),
        )
        .expect_err("live port cannot run stopped-node export");

        assert_eq!(failure.problem().code(), ProblemCode::DataRootLocked);
        let state = state.lock().expect("destination state");
        assert!(state.aborted);
        assert!(!state.completed);
        assert!(state.bytes.is_empty());
        drop(state);

        let failed_abort_state = Arc::new(Mutex::new(DestinationState::default()));
        let failure = WorkspaceArchiveExportPort::export_stopped_node_workspace_archive(
            &node.kernel,
            stopped_request(node.access, CancellationSignal::new(), limits()),
            Box::new(TestDestination::new(Arc::clone(&failed_abort_state)).with_failed_abort()),
        )
        .expect_err("failed wrong-mode abort must preserve uncertain cleanup state");
        assert_eq!(failure.problem().code(), ProblemCode::StorageUnavailable);
        assert_eq!(
            failure.archive_destination_state(),
            Some(FailedArchiveDestinationState::PartialCleanupIndeterminate)
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stopped_node_cancellation_aborts_partial_output_and_cleans_scratch() {
        let node = TestNode::new();
        grant_export(&node);
        node.upload(b"checked evidence");
        let (root, access) = node.into_stopped();
        let current = root.path().join("current");
        let cancellation = CancellationSignal::new();
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let destination = TestDestination::new(Arc::clone(&state)).on_first_write({
            let cancellation = cancellation.clone();
            move || cancellation.cancel()
        });

        let failure = export_stopped_node_workspace_archive(
            root.path(),
            stopped_request(access, cancellation, limits()),
            Box::new(destination),
        )
        .expect_err("stopped export canceled after output begins");

        assert_eq!(failure.problem().code(), ProblemCode::ExportCanceled);
        let state = state.lock().expect("destination state");
        assert!(state.aborted);
        assert!(!state.completed);
        assert!(!state.bytes.is_empty());
        assert!(state.maximum_write <= ARCHIVE_MAX_IO_CHUNK_BYTES);
        drop(state);
        assert_scratch_path_clean(&current);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stopped_node_admission_reserves_entry_and_cleanup_capacity_without_snapshot_credit() {
        let node = TestNode::new();
        grant_export(&node);
        let (root, access) = node.into_stopped();
        let mut configured = limits();
        configured.scratch_ceiling_bytes =
            nonzero(configured.max_entry_bytes.get() + configured.cleanup_reserve_bytes.get() - 1);
        let state = Arc::new(Mutex::new(DestinationState::default()));

        let failure = export_stopped_node_workspace_archive(
            root.path(),
            stopped_request(access, CancellationSignal::new(), configured),
            Box::new(TestDestination::new(Arc::clone(&state))),
        )
        .expect_err("insufficient stopped scratch reserve");

        assert_eq!(failure.problem().code(), ProblemCode::CapacityExceeded);
        let state = state.lock().expect("destination state");
        assert!(state.aborted);
        assert!(!state.completed);
        assert!(state.bytes.is_empty());
        assert_eq!(state.required_bytes, None);
        drop(state);

        let mut unavailable = limits();
        unavailable.max_entry_bytes = nonzero(1_u64 << 50);
        unavailable.scratch_ceiling_bytes =
            nonzero(unavailable.max_entry_bytes.get() + unavailable.cleanup_reserve_bytes.get());
        let unavailable_state = Arc::new(Mutex::new(DestinationState::default()));
        let failure = export_stopped_node_workspace_archive(
            root.path(),
            stopped_request(access, CancellationSignal::new(), unavailable),
            Box::new(TestDestination::new(Arc::clone(&unavailable_state))),
        )
        .expect_err("filesystem cannot satisfy the conservative scratch reservation");

        assert_eq!(failure.problem().code(), ProblemCode::CapacityExceeded);
        let unavailable_state = unavailable_state.lock().expect("destination state");
        assert!(unavailable_state.aborted);
        assert!(!unavailable_state.completed);
        assert!(unavailable_state.required_bytes.is_some());
        assert!(unavailable_state.bytes.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stopped_node_final_reauthorization_prevents_publication() {
        let node = TestNode::new();
        grant_export(&node);
        let (root, access) = node.into_stopped();
        let current = root.path().join("current");
        let database = current.join("fasti.sqlite3");
        let grant_id = access.grant_id().to_string();
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let destination = TestDestination::new(Arc::clone(&state)).on_flush(move || {
            Connection::open(&database)
                .expect("stopped database writer")
                .execute(
                    "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = ?2",
                    params![grant_id, scope_storage_key(ScopeKey::WorkspaceExport)],
                )
                .expect("revoke stopped export scope before publication");
        });

        let failure = export_stopped_node_workspace_archive(
            root.path(),
            stopped_request(access, CancellationSignal::new(), limits()),
            Box::new(destination),
        )
        .expect_err("revoked stopped export cannot publish");

        assert_eq!(failure.problem().code(), ProblemCode::Forbidden);
        let state = state.lock().expect("destination state");
        assert!(state.aborted);
        assert!(!state.completed);
        assert!(!state.bytes.is_empty());
        drop(state);
        assert_scratch_path_clean(&current);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn failed_abort_masks_prepublication_failure_in_both_export_modes() {
        let node = TestNode::new();
        grant_export(&node);
        let online_state = Arc::new(Mutex::new(DestinationState::default()));
        let kernel = node.kernel.clone();
        let grant_id = node.access.grant_id().to_string();
        let online_destination = TestDestination::new(Arc::clone(&online_state))
            .on_flush(move || {
                kernel
                    .inner
                    .connection
                    .lock()
                    .expect("SQLite connection")
                    .execute(
                        "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = ?2",
                        params![grant_id, scope_storage_key(ScopeKey::WorkspaceExport)],
                    )
                    .expect("revoke online export scope before publication");
            })
            .with_failed_abort();
        let online_failure = export(&node, online_destination)
            .expect_err("failed online abort must replace the prior failure");
        assert_eq!(
            online_failure.problem().code(),
            ProblemCode::StorageUnavailable
        );
        assert_eq!(
            online_failure.archive_destination_state(),
            Some(FailedArchiveDestinationState::PartialCleanupIndeterminate)
        );
        let online_state = online_state.lock().expect("online destination state");
        assert!(online_state.aborted);
        assert!(!online_state.completed);
        assert!(!online_state.bytes.is_empty());
        drop(online_state);
        assert_scratch_clean(&node);

        let node = TestNode::new();
        grant_export(&node);
        let (root, access) = node.into_stopped();
        let current = root.path().join("current");
        let database = current.join("fasti.sqlite3");
        let grant_id = access.grant_id().to_string();
        let stopped_state = Arc::new(Mutex::new(DestinationState::default()));
        let stopped_destination = TestDestination::new(Arc::clone(&stopped_state))
            .on_flush(move || {
                Connection::open(&database)
                    .expect("stopped database writer")
                    .execute(
                        "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = ?2",
                        params![grant_id, scope_storage_key(ScopeKey::WorkspaceExport)],
                    )
                    .expect("revoke stopped export scope before publication");
            })
            .with_failed_abort();
        let stopped_failure = export_stopped_node_workspace_archive(
            root.path(),
            stopped_request(access, CancellationSignal::new(), limits()),
            Box::new(stopped_destination),
        )
        .expect_err("failed stopped abort must replace the prior failure");
        assert_eq!(
            stopped_failure.problem().code(),
            ProblemCode::StorageUnavailable
        );
        assert_eq!(
            stopped_failure.archive_destination_state(),
            Some(FailedArchiveDestinationState::PartialCleanupIndeterminate)
        );
        let stopped_state = stopped_state.lock().expect("stopped destination state");
        assert!(stopped_state.aborted);
        assert!(!stopped_state.completed);
        assert!(!stopped_state.bytes.is_empty());
        drop(stopped_state);
        assert_scratch_path_clean(&current);
    }

    #[test]
    fn destination_preflight_is_the_exact_uncompressed_archive_bound() {
        let node = TestNode::new();
        grant_export(&node);
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let mut configured = limits();
        configured.max_archive_bytes = nonzero(1024 * 1024);
        assert!(configured.max_uncompressed_bytes > configured.max_archive_bytes);

        export_online_workspace_archive(
            &node.kernel,
            request_with_limits(&node, CancellationSignal::new(), configured),
            Box::new(TestDestination::new(Arc::clone(&state))),
        )
        .expect("small compressed archive within its independent ceiling");

        let expected = configured
            .archive_expanded_ceiling()
            .expect("archive ceiling")
            + configured.cleanup_reserve_bytes.get();
        assert_eq!(
            state.lock().expect("destination state").required_bytes,
            Some(expected)
        );
        assert_scratch_clean(&node);
    }

    #[test]
    fn cancellation_and_reauthorization_failure_abort_the_destination() {
        let node = TestNode::new();
        grant_export(&node);

        let cancellation = CancellationSignal::new();
        let canceled_state = Arc::new(Mutex::new(DestinationState::default()));
        let canceled_destination =
            TestDestination::new(Arc::clone(&canceled_state)).on_preflight({
                let cancellation = cancellation.clone();
                move || cancellation.cancel()
            });
        let canceled = export_online_workspace_archive(
            &node.kernel,
            request(&node, cancellation),
            Box::new(canceled_destination),
        )
        .expect_err("canceled export");
        assert_eq!(canceled.problem().code(), ProblemCode::ExportCanceled);
        let canceled_state = canceled_state.lock().expect("canceled state");
        assert!(canceled_state.aborted);
        assert!(!canceled_state.completed);
        assert!(canceled_state.bytes.is_empty());
        drop(canceled_state);

        grant_export(&node);
        let revoked_state = Arc::new(Mutex::new(DestinationState::default()));
        let kernel = node.kernel.clone();
        let grant_id = node.access.grant_id().to_string();
        let revoked_destination =
            TestDestination::new(Arc::clone(&revoked_state)).on_preflight(move || {
                kernel
                    .inner
                    .connection
                    .lock()
                    .expect("SQLite connection")
                    .execute(
                        "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = ?2",
                        params![grant_id, scope_storage_key(ScopeKey::WorkspaceExport)],
                    )
                    .expect("revoke export scope");
            });
        let revoked = export(&node, revoked_destination).expect_err("revoked export");
        assert_eq!(
            revoked.problem().code(),
            ProblemCode::StoppedNodeExportRequired
        );
        let revoked_state = revoked_state.lock().expect("revoked state");
        assert!(revoked_state.aborted);
        assert!(!revoked_state.completed);
        assert!(revoked_state.bytes.is_empty());
        drop(revoked_state);
        assert_scratch_clean(&node);
    }

    #[test]
    fn final_reauthorization_prevents_publication_after_archive_flush() {
        let node = TestNode::new();
        grant_export(&node);
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let kernel = node.kernel.clone();
        let grant_id = node.access.grant_id().to_string();
        let destination = TestDestination::new(Arc::clone(&state)).on_flush(move || {
            kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection")
                .execute(
                    "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = ?2",
                    params![grant_id, scope_storage_key(ScopeKey::WorkspaceExport)],
                )
                .expect("revoke export scope after final flush");
        });

        let failure = export(&node, destination).expect_err("revoked before publication");
        assert_eq!(
            failure.problem().code(),
            ProblemCode::StoppedNodeExportRequired
        );
        let state = state.lock().expect("destination state");
        assert!(state.aborted);
        assert!(!state.completed);
        assert!(!state.bytes.is_empty());
        drop(state);
        assert_scratch_clean(&node);
    }

    #[test]
    fn admission_reserves_wal_growth_outside_the_scratch_ceiling() {
        let mut configured = limits();
        let scratch_ceiling = configured.max_snapshot_bytes.get()
            + configured.max_entry_bytes.get()
            + configured.cleanup_reserve_bytes.get();
        configured.scratch_ceiling_bytes = nonzero(scratch_ceiling);
        let bounds = AdmissionBounds::try_new(configured, RequestCorrelationId::new_v7())
            .expect("WAL is filesystem headroom, not scratch content");
        assert_eq!(
            bounds.scratch_bytes,
            scratch_ceiling + configured.max_wal_growth_bytes.get()
        );
    }

    #[test]
    fn failed_consuming_completion_removes_its_partial_artifact() {
        let node = TestNode::new();
        grant_export(&node);
        let destination_root = tempfile::tempdir().expect("destination root");
        let partial = destination_root.path().join("workspace.fasti.partial");
        let failure = export_online_workspace_archive(
            &node.kernel,
            request(&node, CancellationSignal::new()),
            Box::new(CleanupOnFailedCompleteDestination::create(partial.clone())),
        )
        .expect_err("publication failure");
        assert_eq!(failure.problem().code(), ProblemCode::StorageUnavailable);
        assert!(!partial.exists());
        assert_scratch_clean(&node);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn indeterminate_completion_state_reaches_both_export_receipts() {
        let node = TestNode::new();
        grant_export(&node);
        let online_state = Arc::new(Mutex::new(DestinationState::default()));
        let online_failure = export(
            &node,
            TestDestination::new(Arc::clone(&online_state)).with_indeterminate_completion(),
        )
        .expect_err("directory sync failure cannot return an export success");
        assert_eq!(
            online_failure.archive_destination_state(),
            Some(FailedArchiveDestinationState::PublishedDurabilityIndeterminate)
        );
        assert_eq!(
            online_failure.problem().code(),
            ProblemCode::StorageUnavailable
        );
        assert!(online_state.lock().expect("online state").completed);
        assert_scratch_clean(&node);

        let node = TestNode::new();
        grant_export(&node);
        let (root, access) = node.into_stopped();
        let stopped_state = Arc::new(Mutex::new(DestinationState::default()));
        let stopped_failure = export_stopped_node_workspace_archive(
            root.path(),
            stopped_request(access, CancellationSignal::new(), limits()),
            Box::new(
                TestDestination::new(Arc::clone(&stopped_state)).with_indeterminate_completion(),
            ),
        )
        .expect_err("directory sync failure cannot return a stopped export success");
        assert_eq!(
            stopped_failure.archive_destination_state(),
            Some(FailedArchiveDestinationState::PublishedDurabilityIndeterminate)
        );
        assert_eq!(
            stopped_failure.problem().code(),
            ProblemCode::StorageUnavailable
        );
        assert!(stopped_state.lock().expect("stopped state").completed);
        assert_scratch_path_clean(&root.path().join("current"));
    }

    #[test]
    fn stale_correlation_directory_does_not_block_a_fresh_scratch_attempt() {
        let node = TestNode::new();
        grant_export(&node);
        let correlation_id = RequestCorrelationId::new_v7();
        let exports = node
            .kernel
            .inner
            .current_root
            .join("scratch")
            .join("exports");
        prepare_private_directory(&exports).expect("exports scratch root");
        let stale = exports.join(correlation_id.to_string());
        prepare_private_directory(&stale).expect("stale prior scratch directory");
        fs::write(stale.join("snapshot.sqlite3"), b"stale plaintext snapshot")
            .expect("stale snapshot");

        let request = ExportWorkspaceRequest::new(
            ExportWorkspaceQuery::new(correlation_id, node.access),
            limits(),
            CancellationSignal::new(),
        );
        let state = Arc::new(Mutex::new(DestinationState::default()));
        export_online_workspace_archive(
            &node.kernel,
            request,
            Box::new(TestDestination::new(state)),
        )
        .expect("fresh random scratch suffix");

        assert!(!stale.exists(), "the stale attempt was reclaimed");
        assert_scratch_clean(&node);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stale_sweep_preserves_an_owner_locked_attempt() {
        let node = TestNode::new();
        grant_export(&node);
        let exports = node
            .kernel
            .inner
            .current_root
            .join("scratch")
            .join("exports");
        prepare_private_directory(&exports).expect("exports scratch root");
        let active = exports.join("active-attempt");
        prepare_private_directory(&active).expect("active attempt");
        let owner = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(active.join(".owner"))
            .expect("owner lock file");
        owner.lock().expect("hold owner lock");
        fs::write(active.join("snapshot.sqlite3"), b"active snapshot").expect("active snapshot");

        let state = Arc::new(Mutex::new(DestinationState::default()));
        export(&node, TestDestination::new(state)).expect("concurrent export");
        assert!(active.join("snapshot.sqlite3").is_file());

        drop(owner);
        fs::remove_file(active.join("snapshot.sqlite3")).expect("remove active snapshot");
        fs::remove_file(active.join(".owner")).expect("remove owner file");
        fs::remove_dir(active).expect("remove active attempt");
        assert_scratch_clean(&node);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn source_database_child_symlink_is_rejected() {
        let node = TestNode::new();
        let correlation_id = RequestCorrelationId::new_v7();
        let root = AnchoredOnlineRoot::open(&node.kernel, correlation_id).expect("anchored root");
        let database = node.kernel.inner.current_root.join("fasti.sqlite3");
        let retained = node
            .kernel
            .inner
            .current_root
            .join("fasti-retained.sqlite3");
        fs::rename(&database, &retained).expect("retain database inode");
        std::os::unix::fs::symlink(&retained, &database).expect("substitute database symlink");

        let problem = root
            .open_source_connection(limits(), correlation_id)
            .expect_err("SQLite must not follow the final symlink");
        assert_eq!(problem.code(), ProblemCode::IntegrityFailed);

        fs::remove_file(&database).expect("remove symlink");
        fs::rename(&retained, &database).expect("restore database path");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn data_root_path_replacement_cannot_redirect_snapshot_or_evidence() {
        let node = TestNode::new();
        let evidence = b"evidence from the anchored original root";
        node.upload(evidence);
        grant_export(&node);
        let original_root = node.kernel.data_root().to_path_buf();
        let moved_root = original_root.with_file_name(format!(
            "{}-moved",
            original_root
                .file_name()
                .and_then(|name| name.to_str())
                .expect("UTF-8 test root")
        ));
        let replacement_marker = original_root.join("replacement-marker");
        let destination_state = Arc::new(Mutex::new(DestinationState::default()));
        let destination = TestDestination::new(Arc::clone(&destination_state)).on_preflight({
            let original_root = original_root.clone();
            let moved_root = moved_root.clone();
            let replacement_marker = replacement_marker.clone();
            move || {
                fs::rename(&original_root, &moved_root).expect("rename original data root");
                fs::create_dir(&original_root).expect("replacement data root");
                fs::write(&replacement_marker, b"must remain untouched")
                    .expect("replacement marker");
            }
        });

        let outcome = export(&node, destination).expect("anchored export after path replacement");
        let replacement_entries = fs::read_dir(&original_root)
            .expect("replacement root")
            .map(|entry| entry.expect("replacement entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            replacement_entries,
            vec![replacement_marker
                .file_name()
                .expect("marker name")
                .to_owned()]
        );
        let state = destination_state.lock().expect("destination state");
        let entries = archive_entries(&state.bytes);
        assert!(entries
            .iter()
            .any(|(path, bytes)| { path.starts_with("payloads/sha256/") && bytes == evidence }));
        assert!(outcome.archive_bytes() > 0);
        drop(state);

        fs::remove_dir_all(&original_root).expect("remove replacement root");
        fs::rename(&moved_root, &original_root).expect("restore original root for cleanup");
        assert_scratch_clean(&node);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn multistep_backup_does_not_hold_the_live_writer_mutex() {
        let node = TestNode::new();
        let correlation_id = RequestCorrelationId::new_v7();
        let root = AnchoredOnlineRoot::open(&node.kernel, correlation_id).expect("anchored root");
        let request = request(&node, CancellationSignal::new());
        let bounds =
            AdmissionBounds::try_new(request.limits(), correlation_id).expect("admission bounds");
        let scratch = root
            .create_scratch(correlation_id, &bounds)
            .expect("export scratch");
        let source = root
            .open_source_connection(request.limits(), correlation_id)
            .expect("separate source connection");
        let snapshot_limits = SnapshotLimits::new(
            NonZeroU32::new(1).expect("one page"),
            Duration::from_secs(1),
            Duration::from_secs(10),
        )
        .expect("snapshot limits");
        let mut writer_committed = false;

        node.kernel
            .snapshot_database_from_connection(
                &source,
                scratch
                    .snapshot_path(correlation_id)
                    .expect("snapshot path"),
                snapshot_limits,
                |_| {
                    if !writer_committed {
                        let connection = node
                            .kernel
                            .inner
                            .connection
                            .try_lock()
                            .expect("backup must not hold the live writer mutex");
                        connection
                            .execute(
                                "INSERT INTO records(record_id, workspace_id, grain, status, created_at) \
                                 VALUES (?1, ?2, 'film', 'active', '2026-08-24T00:00:00.000000Z')",
                                params![
                                    RecordId::new_v7().to_string(),
                                    node.access.workspace_id().to_string()
                                ],
                            )
                            .expect("concurrent live writer commit");
                        writer_committed = true;
                    }
                    ControlFlow::Continue(())
                },
            )
            .expect("multistep snapshot");
        assert!(
            writer_committed,
            "one-page backup must invoke its step monitor"
        );
        drop(scratch);
        assert_scratch_clean(&node);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn evidence_symlink_substitution_is_rejected_from_the_anchored_root() {
        let node = TestNode::new();
        node.upload(b"anchored evidence");
        grant_export(&node);
        let relative_path = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection")
            .query_row("SELECT relative_path FROM evidence", [], |row| {
                row.get::<_, String>(0)
            })
            .expect("evidence path");
        let evidence_path = node.kernel.inner.current_root.join(relative_path);
        let original_path = evidence_path.with_extension("original");
        fs::rename(&evidence_path, &original_path).expect("move original evidence");
        std::os::unix::fs::symlink(&original_path, &evidence_path)
            .expect("substitute evidence symlink");

        let state = Arc::new(Mutex::new(DestinationState::default()));
        let failure = export(&node, TestDestination::new(Arc::clone(&state)))
            .expect_err("anchored open must reject symlink substitution");
        assert_eq!(failure.problem().code(), ProblemCode::IntegrityFailed);
        assert!(state.lock().expect("destination state").aborted);
        assert_scratch_clean(&node);
    }

    #[test]
    fn archive_reads_only_the_snapshot_after_live_mutation() {
        let node = TestNode::new();
        grant_export(&node);
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let kernel = node.kernel.clone();
        let workspace_id = node.access.workspace_id().to_string();
        let destination = TestDestination::new(Arc::clone(&state)).on_first_write(move || {
            kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection")
                .execute(
                    "INSERT INTO records(record_id, workspace_id, grain, status, created_at) \
                     VALUES (?1, ?2, 'film', 'active', '2026-08-24T00:00:00.000000Z')",
                    params![RecordId::new_v7().to_string(), workspace_id],
                )
                .expect("live mutation after snapshot");
        });

        let outcome = export(&node, destination).expect("snapshot archive");
        let state = state.lock().expect("destination state");
        let entries = archive_entries(&state.bytes);
        let manifest: ChecksummedWorkspaceManifestDto =
            serde_json::from_slice(&entries.last().expect("manifest").1).expect("manifest DTO");
        let record_descriptor = &manifest.manifest.streams[WorkspaceExportEntity::Records.index()];
        assert_eq!(record_descriptor.row_count, 0);
        assert!(
            node.kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection")
                .query_row("SELECT COUNT(*) FROM records", [], |row| row
                    .get::<_, i64>(0))
                .expect("live record count")
                > 0
        );
        let live_revision = workspace_revision(
            &node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection"),
            &node.access.workspace_id().to_string(),
        )
        .expect("live revision");
        assert!(u64::try_from(live_revision).expect("non-negative") > outcome.workspace_revision());
        assert_scratch_clean(&node);
    }

    #[test]
    fn archive_contract_version_tracks_the_governed_registry() {
        let registry: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/generated/v1/capabilities.json"
        ))
        .expect("generated capability registry");
        assert_eq!(
            registry
                .get("contract_version")
                .and_then(serde_json::Value::as_str),
            Some(WORKSPACE_ARCHIVE_CONTRACT_VERSION)
        );
    }
}
