//! Private crash-safe activation marker for B3 clean restore.
//!
//! Import and equality verification happen before this module is called. Phase
//! files are immutable create-new sentinels, so recovery never interprets a
//! torn in-place state update. The verified marker moves with the staged
//! directory inode; `restore.complete` is created only after the no-replace
//! activation and data-root sync are durable.

#![allow(dead_code)] // some marker inspection accessors await store-adapter activation

use crate::archive::{
    activate_no_replace, open_existing_file_beneath, open_new_file_beneath, open_private_directory,
    sync_open_handle, ArchiveError,
};
use crate::restore::VerifiedArchivePreflight;
use fasti_domain::{RestoreAttemptId, RestoreStatus, Sha256Digest, WorkspaceId};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Write};
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use thiserror::Error;

const MARKER_FORMAT_VERSION: u32 = 1;
pub(crate) const RESTORE_STAGING_DIRECTORY: &str = "staging";
pub(crate) const MARKER_FILE: &str = "restore.marker.json";
const COMPLETE_PENDING_FILE: &str = "restore.complete.pending";
const REJECTED_PENDING_FILE: &str = "restore.rejected.pending";
pub(crate) const RESTORE_STATE_FILES: [&str; 7] = [
    "restore.received",
    "restore.staging",
    "restore.verified",
    "restore.activating",
    "restore.complete",
    "restore.rejected",
    MARKER_FILE,
];
const MAX_MARKER_BYTES: u64 = 1024;

#[derive(Debug, Error)]
pub(crate) enum RestoreActivationError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("restore activation marker I/O failed")]
    Io(#[from] io::Error),
    #[error("restore activation marker is invalid")]
    InvalidMarker,
    #[error("restore activation marker does not match the requested restore")]
    MarkerMismatch,
    #[error("restore activation phase is missing or invalid")]
    InvalidPhase,
    #[error("an incomplete restore staging attempt requires offline cleanup")]
    IncompleteStaging,
    #[error("clean restore cannot replace an existing current workspace")]
    CurrentExists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestoreActivationMarker {
    restore_attempt_id: RestoreAttemptId,
    workspace_id: WorkspaceId,
    workspace_revision: u64,
    archive_digest: Sha256Digest,
    manifest_digest: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreActivationMarkerDto {
    archive_digest: String,
    format_version: u32,
    manifest_digest: String,
    restore_attempt_id: String,
    status: RestoreStatus,
    workspace_id: String,
    workspace_revision: u64,
}

impl RestoreActivationMarker {
    pub(crate) fn new(
        restore_attempt_id: RestoreAttemptId,
        workspace_id: WorkspaceId,
        workspace_revision: u64,
        archive_digest: Sha256Digest,
        manifest_digest: Sha256Digest,
    ) -> Self {
        Self {
            restore_attempt_id,
            workspace_id,
            workspace_revision,
            archive_digest,
            manifest_digest,
        }
    }

    pub(crate) fn from_preflight(
        restore_attempt_id: RestoreAttemptId,
        preflight: &VerifiedArchivePreflight,
    ) -> Self {
        let manifest = preflight.manifest().manifest();
        Self::new(
            restore_attempt_id,
            manifest.workspace_id(),
            manifest.workspace_revision(),
            preflight.archive_digest().clone(),
            preflight.manifest().manifest_digest().clone(),
        )
    }

    pub(crate) const fn restore_attempt_id(&self) -> RestoreAttemptId {
        self.restore_attempt_id
    }

    pub(crate) const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub(crate) const fn workspace_revision(&self) -> u64 {
        self.workspace_revision
    }

    pub(crate) const fn archive_digest(&self) -> &Sha256Digest {
        &self.archive_digest
    }

    pub(crate) const fn manifest_digest(&self) -> &Sha256Digest {
        &self.manifest_digest
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, RestoreActivationError> {
        serde_json::to_vec(&RestoreActivationMarkerDto {
            archive_digest: self.archive_digest.to_string(),
            format_version: MARKER_FORMAT_VERSION,
            manifest_digest: self.manifest_digest.to_string(),
            restore_attempt_id: self.restore_attempt_id.to_string(),
            status: RestoreStatus::Verified,
            workspace_id: self.workspace_id.to_string(),
            workspace_revision: self.workspace_revision,
        })
        .map_err(|_| RestoreActivationError::InvalidMarker)
    }

    fn parse_canonical(bytes: &[u8]) -> Result<Self, RestoreActivationError> {
        let dto: RestoreActivationMarkerDto =
            serde_json::from_slice(bytes).map_err(|_| RestoreActivationError::InvalidMarker)?;
        if dto.format_version != MARKER_FORMAT_VERSION || dto.status != RestoreStatus::Verified {
            return Err(RestoreActivationError::InvalidMarker);
        }
        let marker = Self {
            restore_attempt_id: dto
                .restore_attempt_id
                .parse()
                .map_err(|_| RestoreActivationError::InvalidMarker)?,
            workspace_id: dto
                .workspace_id
                .parse()
                .map_err(|_| RestoreActivationError::InvalidMarker)?,
            workspace_revision: dto.workspace_revision,
            archive_digest: Sha256Digest::parse(dto.archive_digest)
                .map_err(|_| RestoreActivationError::InvalidMarker)?,
            manifest_digest: Sha256Digest::parse(dto.manifest_digest)
                .map_err(|_| RestoreActivationError::InvalidMarker)?,
        };
        if marker.canonical_bytes()?.as_slice() != bytes {
            return Err(RestoreActivationError::InvalidMarker);
        }
        Ok(marker)
    }
}

fn phase(status: RestoreStatus) -> (&'static str, &'static str) {
    match status {
        RestoreStatus::Received => ("restore.received", "received"),
        RestoreStatus::Staging => ("restore.staging", "staging"),
        RestoreStatus::Verified => ("restore.verified", "verified"),
        RestoreStatus::Activating => ("restore.activating", "activating"),
        RestoreStatus::Complete => ("restore.complete", "complete"),
        RestoreStatus::Rejected => ("restore.rejected", "rejected"),
    }
}

fn terminal_pending_file(status: RestoreStatus) -> Option<&'static str> {
    match status {
        RestoreStatus::Complete => Some(COMPLETE_PENDING_FILE),
        RestoreStatus::Rejected => Some(REJECTED_PENDING_FILE),
        _ => None,
    }
}

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn crash_test_point(scope: &str, operation: &str) {
    let expected = format!("{scope}.{operation}");
    if std::env::var("FASTI_TEST_RESTORE_CRASH_POINT").as_deref() == Ok(expected.as_str()) {
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .expect("send SIGKILL to restore crash worker");
    }
}

#[cfg(not(all(test, target_os = "linux")))]
#[inline(always)]
pub(crate) fn crash_test_point(_scope: &str, _operation: &str) {}

pub(crate) fn write_restore_phase(
    attempt: &File,
    status: RestoreStatus,
) -> Result<(), RestoreActivationError> {
    let (name, value) = phase(status);
    if let Some(pending) = terminal_pending_file(status) {
        let mut file = open_new_file_beneath(attempt, Path::new(pending))?;
        crash_test_point(value, "created");
        file.write_all(value.as_bytes())?;
        file.write_all(b"\n")?;
        crash_test_point(value, "written");
        sync_open_handle(&file)?;
        crash_test_point(value, "file_synced");
        activate_no_replace(attempt, pending, attempt, name)?;
        crash_test_point(value, "renamed");
        sync_open_handle(attempt)?;
        crash_test_point(value, "directory_synced");
        return Ok(());
    }
    let mut file = open_new_file_beneath(attempt, Path::new(name))?;
    crash_test_point(value, "created");
    file.write_all(value.as_bytes())?;
    file.write_all(b"\n")?;
    crash_test_point(value, "written");
    sync_open_handle(&file)?;
    crash_test_point(value, "file_synced");
    sync_open_handle(attempt)?;
    crash_test_point(value, "directory_synced");
    Ok(())
}

pub(crate) fn require_restore_phase(
    directory: &File,
    status: RestoreStatus,
) -> Result<(), RestoreActivationError> {
    let (name, value) = phase(status);
    let file = open_existing_file_beneath(directory, Path::new(name))?;
    require_private_file(&file)?;
    let mut bytes = Vec::new();
    file.take(32).read_to_end(&mut bytes)?;
    let expected = format!("{value}\n");
    if bytes != expected.as_bytes() {
        return Err(RestoreActivationError::InvalidPhase);
    }
    Ok(())
}

fn write_marker(
    attempt: &File,
    marker: &RestoreActivationMarker,
) -> Result<(), RestoreActivationError> {
    let mut file = open_new_file_beneath(attempt, Path::new(MARKER_FILE))?;
    crash_test_point("marker", "created");
    file.write_all(&marker.canonical_bytes()?)?;
    crash_test_point("marker", "written");
    sync_open_handle(&file)?;
    crash_test_point("marker", "file_synced");
    sync_open_handle(attempt)?;
    crash_test_point("marker", "directory_synced");
    Ok(())
}

fn read_marker(directory: &File) -> Result<RestoreActivationMarker, RestoreActivationError> {
    let file = open_existing_file_beneath(directory, Path::new(MARKER_FILE))?;
    require_private_file(&file)?;
    if file.metadata()?.len() > MAX_MARKER_BYTES {
        return Err(RestoreActivationError::InvalidMarker);
    }
    let mut bytes = Vec::new();
    file.take(MAX_MARKER_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MARKER_BYTES {
        return Err(RestoreActivationError::InvalidMarker);
    }
    RestoreActivationMarker::parse_canonical(&bytes)
}

fn require_private_file(file: &File) -> Result<(), RestoreActivationError> {
    #[cfg(target_os = "linux")]
    if file.metadata()?.permissions().mode() & 0o777 != 0o600 {
        return Err(RestoreActivationError::InvalidMarker);
    }
    Ok(())
}

/// Durably activate one already-imported and verified staging attempt.
pub(crate) fn activate_verified_restore(
    data_root: &File,
    staging: &File,
    attempt: &File,
    attempt_name: &str,
    marker: &RestoreActivationMarker,
) -> Result<(), RestoreActivationError> {
    require_restore_phase(attempt, RestoreStatus::Received)?;
    require_restore_phase(attempt, RestoreStatus::Staging)?;
    require_restore_phase(attempt, RestoreStatus::Verified)?;
    write_marker(attempt, marker)?;
    write_restore_phase(attempt, RestoreStatus::Activating)?;
    sync_open_handle(attempt)?;
    crash_test_point("activation", "attempt_synced");
    sync_open_handle(staging)?;
    crash_test_point("activation", "staging_synced");
    activate_no_replace(staging, attempt_name, data_root, "current")?;
    crash_test_point("activation", "renamed");
    sync_open_handle(data_root)?;
    crash_test_point("activation", "root_synced");
    write_restore_phase(attempt, RestoreStatus::Complete)?;
    sync_open_handle(data_root)?;
    crash_test_point("activation", "complete_root_synced");
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationRecovery {
    AlreadyComplete,
    CompletedAfterRename,
}

/// Resolve all restore filesystem state that is safe before SQLite opens.
///
/// Pre-rename staging is never opened as a database. The daemon fails closed
/// until the offline restore owner rejects that attempt. A renamed verified
/// directory is completed through its descriptor-rooted marker first.
pub(crate) fn recover_activation_before_database_open(
    data_root: &File,
) -> Result<Option<ActivationRecovery>, RestoreActivationError> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = data_root;
        return Ok(None);
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(staging) = open_optional_directory(data_root, RESTORE_STAGING_DIRECTORY)? {
            let path = format!("/proc/self/fd/{}", staging.as_raw_fd());
            if std::fs::read_dir(path)?.next().transpose()?.is_some() {
                return Err(RestoreActivationError::IncompleteStaging);
            }
        }
        let Some(current) = open_optional_directory(data_root, "current")? else {
            return Ok(None);
        };
        let mut managed = false;
        for name in RESTORE_STATE_FILES {
            match open_existing_file_beneath(&current, Path::new(name)) {
                Ok(_) => managed = true,
                Err(ArchiveError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        if managed {
            recover_current_activation(data_root).map(Some)
        } else {
            Ok(None)
        }
    }
}

fn open_optional_directory(
    parent: &File,
    name: &str,
) -> Result<Option<File>, RestoreActivationError> {
    match open_private_directory(parent, name) {
        Ok(directory) => Ok(Some(directory)),
        Err(ArchiveError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(target_os = "linux")]
pub(crate) fn discard_pending_restore_phase(
    directory: &File,
    status: RestoreStatus,
) -> Result<(), RestoreActivationError> {
    let pending = terminal_pending_file(status).ok_or(RestoreActivationError::InvalidPhase)?;
    match rustix::fs::unlinkat(directory, pending, rustix::fs::AtFlags::empty()) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(error) => Err(io::Error::from_raw_os_error(error.raw_os_error()).into()),
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn discard_pending_restore_phase(
    _directory: &File,
    _status: RestoreStatus,
) -> Result<(), RestoreActivationError> {
    Err(ArchiveError::UnsupportedPlatform.into())
}

pub(crate) fn require_clean_restore_target(data_root: &File) -> Result<(), RestoreActivationError> {
    if open_optional_directory(data_root, "current")?.is_some() {
        return Err(RestoreActivationError::CurrentExists);
    }
    if let Some(staging) = open_optional_directory(data_root, RESTORE_STAGING_DIRECTORY)? {
        #[cfg(target_os = "linux")]
        {
            let path = format!("/proc/self/fd/{}", staging.as_raw_fd());
            if std::fs::read_dir(path)?.next().transpose()?.is_some() {
                return Err(RestoreActivationError::IncompleteStaging);
            }
        }
    }
    Ok(())
}

/// Finish the only recoverable crash state: verified directory renamed to
/// `current`, but the immutable COMPLETE sentinel was not yet durable.
pub(crate) fn recover_current_activation(
    data_root: &File,
) -> Result<ActivationRecovery, RestoreActivationError> {
    let current = open_private_directory(data_root, "current")?;
    require_restore_phase(&current, RestoreStatus::Received)?;
    require_restore_phase(&current, RestoreStatus::Staging)?;
    require_restore_phase(&current, RestoreStatus::Verified)?;
    require_restore_phase(&current, RestoreStatus::Activating)?;
    read_marker(&current)?;
    match require_restore_phase(&current, RestoreStatus::Complete) {
        Ok(()) => {
            sync_open_handle(&current)?;
            sync_open_handle(data_root)?;
            Ok(ActivationRecovery::AlreadyComplete)
        }
        Err(RestoreActivationError::Archive(ArchiveError::Io(error)))
            if error.kind() == io::ErrorKind::NotFound =>
        {
            discard_pending_restore_phase(&current, RestoreStatus::Complete)?;
            write_restore_phase(&current, RestoreStatus::Complete)?;
            sync_open_handle(data_root)?;
            Ok(ActivationRecovery::CompletedAfterRename)
        }
        Err(error) => Err(error),
    }
}

/// Descriptor-verify the durable COMPLETE marker before recovery bootstrap.
pub(crate) fn verify_complete_restore(
    data_root: &File,
    restore_attempt_id: RestoreAttemptId,
    workspace_id: WorkspaceId,
) -> Result<RestoreActivationMarker, RestoreActivationError> {
    let current = open_private_directory(data_root, "current")?;
    require_restore_phase(&current, RestoreStatus::Received)?;
    require_restore_phase(&current, RestoreStatus::Staging)?;
    require_restore_phase(&current, RestoreStatus::Verified)?;
    require_restore_phase(&current, RestoreStatus::Activating)?;
    require_restore_phase(&current, RestoreStatus::Complete)?;
    let marker = read_marker(&current)?;
    if marker.restore_attempt_id != restore_attempt_id || marker.workspace_id != workspace_id {
        return Err(RestoreActivationError::MarkerMismatch);
    }
    Ok(marker)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::create_staging_attempt;

    fn digest(byte: u8) -> Sha256Digest {
        Sha256Digest::parse(format!("sha256:{}", format!("{byte:02x}").repeat(32))).expect("digest")
    }

    fn marker() -> RestoreActivationMarker {
        RestoreActivationMarker {
            restore_attempt_id: RestoreAttemptId::new_v7(),
            workspace_id: WorkspaceId::new_v7(),
            workspace_revision: 17,
            archive_digest: digest(1),
            manifest_digest: digest(2),
        }
    }

    fn root() -> (tempfile::TempDir, File) {
        let temporary = tempfile::tempdir().expect("temporary data root");
        let root = File::open(temporary.path()).expect("data-root handle");
        (temporary, root)
    }

    #[test]
    fn verified_attempt_moves_with_its_marker_and_completes_after_parent_sync() {
        let (temporary, root) = root();
        let (staging, attempt) =
            create_staging_attempt(&root, RESTORE_STAGING_DIRECTORY, "attempt-one")
                .expect("staging attempt");
        for status in [
            RestoreStatus::Received,
            RestoreStatus::Staging,
            RestoreStatus::Verified,
        ] {
            write_restore_phase(&attempt, status).expect("phase");
        }
        let marker = marker();

        activate_verified_restore(&root, &staging, &attempt, "attempt-one", &marker)
            .expect("activation");
        let verified =
            verify_complete_restore(&root, marker.restore_attempt_id(), marker.workspace_id())
                .expect("complete marker");
        assert_eq!(verified, marker);
        assert!(temporary.path().join("current").is_dir());
        assert!(!temporary.path().join("restore/attempt-one").exists());
    }

    #[test]
    fn recovery_only_completes_a_digest_proven_post_rename_attempt() {
        let (_temporary, root) = root();
        let (staging, attempt) =
            create_staging_attempt(&root, RESTORE_STAGING_DIRECTORY, "attempt-two")
                .expect("staging attempt");
        for status in [
            RestoreStatus::Received,
            RestoreStatus::Staging,
            RestoreStatus::Verified,
        ] {
            write_restore_phase(&attempt, status).expect("phase");
        }
        write_marker(&attempt, &marker()).expect("marker");
        write_restore_phase(&attempt, RestoreStatus::Activating).expect("activating");
        activate_no_replace(&staging, "attempt-two", &root, "current").expect("rename");
        sync_open_handle(&root).expect("durable rename");

        assert_eq!(
            recover_current_activation(&root).expect("recover"),
            ActivationRecovery::CompletedAfterRename
        );
        assert_eq!(
            recover_current_activation(&root).expect("idempotent recovery"),
            ActivationRecovery::AlreadyComplete
        );
    }

    #[test]
    fn marker_parser_rejects_alternate_bytes_and_identity_mismatch() {
        let marker = marker();
        let mut alternate = marker.canonical_bytes().expect("canonical marker");
        alternate.push(b'\n');
        assert!(matches!(
            RestoreActivationMarker::parse_canonical(&alternate),
            Err(RestoreActivationError::InvalidMarker)
        ));

        let (temporary, root) = root();
        let (staging, attempt) =
            create_staging_attempt(&root, RESTORE_STAGING_DIRECTORY, "attempt-three")
                .expect("staging attempt");
        for status in [
            RestoreStatus::Received,
            RestoreStatus::Staging,
            RestoreStatus::Verified,
        ] {
            write_restore_phase(&attempt, status).expect("phase");
        }
        activate_verified_restore(&root, &staging, &attempt, "attempt-three", &marker)
            .expect("activation");
        assert!(matches!(
            verify_complete_restore(&root, RestoreAttemptId::new_v7(), marker.workspace_id()),
            Err(RestoreActivationError::MarkerMismatch)
        ));
        let marker_path = temporary.path().join("current").join(MARKER_FILE);
        let mut permissions = std::fs::metadata(&marker_path)
            .expect("marker metadata")
            .permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(marker_path, permissions).expect("weaken marker permissions");
        assert!(matches!(
            verify_complete_restore(&root, marker.restore_attempt_id(), marker.workspace_id()),
            Err(RestoreActivationError::InvalidMarker)
        ));
    }
}
