use crate::crypto::{constant_time_eq, sha256_hex};
use crate::schema::{migrate, SCHEMA_VERSION};
use chrono::{DateTime, Utc};
use fasti_application::{
    authorize, AccessSnapshot, ApplicationResult, AuthorizationRequirement, CapabilityKey,
    CredentialStatus, FastiProblem, GrantStatus, ProblemCode, RequestAccessContext, ScopeKey,
};
use fasti_domain::{
    ClientId, CredentialId, ProfileGrantId, ProfileId, RequestCorrelationId, WorkspaceId,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;

pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_EVIDENCE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_TEMP_EVIDENCE_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum prepared, not-yet-accepted evidence retained by one workspace.
///
/// Accepted Chronicle evidence is not counted. This bounds abandoned upload
/// state before later retention and operator cleanup capabilities exist.
pub const MAX_PREPARED_EVIDENCE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CONCURRENT_UPLOADS: usize = 4;

#[derive(Debug, Error)]
pub enum StoreOpenError {
    #[error("this platform does not provide the required data-root locking semantics")]
    UnsupportedPlatform,
    #[error("failed to prepare the Fasti data root: {0}")]
    Io(#[from] std::io::Error),
    #[error("refusing unsafe data path {path:?}: {reason}")]
    UnsafePath { path: PathBuf, reason: &'static str },
    #[error("failed to open or migrate SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("SQLite returned unsupported journal mode {0:?}; WAL is required")]
    JournalMode(String),
    #[error("SQLite returned synchronous level {0}; FULL is required")]
    SynchronousLevel(i64),
    #[error("SQLite schema version {actual} does not match expected version {expected}")]
    SchemaVersion { expected: i64, actual: i64 },
    #[error("another daemon or offline operation holds the Fasti data-root lock")]
    DataRootLocked,
    #[error("restore activation state is incomplete or invalid")]
    RestoreActivation,
}

#[derive(Debug, Clone)]
pub struct SqliteKernel {
    pub(crate) inner: Arc<KernelInner>,
}

/// Exclusive access to one data root without creating or opening `current/`.
///
/// The daemon kernel and stopped-node CLI use this same guard so restore can
/// prove the daemon is stopped before it inspects staging or active data.
#[derive(Debug)]
pub struct LockedDataRoot {
    path: PathBuf,
    #[cfg(target_os = "linux")]
    root_directory: File,
    _lock: File,
}

impl LockedDataRoot {
    #[cfg(unix)]
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self, StoreOpenError> {
        let path = path.as_ref().to_path_buf();
        prepare_private_directory(&path)?;
        #[cfg(target_os = "linux")]
        let root_directory = open_data_root_directory(&path)?;
        #[cfg(target_os = "linux")]
        let lock = acquire_data_root_lock(&path, &root_directory)?;
        #[cfg(not(target_os = "linux"))]
        let lock = acquire_data_root_lock(&path)?;
        Ok(Self {
            path,
            #[cfg(target_os = "linux")]
            root_directory,
            _lock: lock,
        })
    }

    #[cfg(not(unix))]
    pub fn acquire(_path: impl AsRef<Path>) -> Result<Self, StoreOpenError> {
        Err(StoreOpenError::UnsupportedPlatform)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns an anchored data-root directory handle where descriptor-relative
    /// filesystem operations are supported.
    ///
    /// Linux restore and recovery code must use this handle instead of
    /// resolving child paths from [`Self::path`] again. Other platforms return
    /// `None` until they provide equivalent no-follow activation semantics.
    pub fn anchored_directory(&self) -> Option<&File> {
        #[cfg(target_os = "linux")]
        {
            Some(&self.root_directory)
        }
        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    fn current_path(&self) -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            PathBuf::from(format!(
                "/proc/self/fd/{}/current",
                self.root_directory.as_raw_fd()
            ))
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.path.join("current")
        }
    }
}

#[derive(Debug)]
pub(crate) struct KernelInner {
    pub(crate) current_root: PathBuf,
    pub(crate) payload_root: PathBuf,
    pub(crate) scratch_root: PathBuf,
    pub(crate) connection: Mutex<Connection>,
    pub(crate) upload_budget: Mutex<UploadBudget>,
    // Rust drops fields in declaration order; release this lock last.
    pub(crate) data_root: LockedDataRoot,
}

#[derive(Debug, Default)]
pub(crate) struct UploadBudget {
    pub(crate) active: usize,
    pub(crate) reserved_bytes: u64,
}

impl SqliteKernel {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, StoreOpenError> {
        let data_root = LockedDataRoot::acquire(data_root)?;
        Self::open_locked(data_root)
    }

    /// Opens the local kernel without releasing an already-held data-root lock.
    pub fn open_locked(data_root: LockedDataRoot) -> Result<Self, StoreOpenError> {
        if let Some(root) = data_root.anchored_directory() {
            crate::restore_activation::recover_activation_before_database_open(root)
                .map_err(|_| StoreOpenError::RestoreActivation)?;
        }
        let current_root = data_root.current_path();
        let payload_root = current_root.join("payloads").join("sha256");
        let scratch_root = current_root.join("scratch").join("uploads");

        prepare_private_directory(&current_root)?;
        prepare_private_directory(&payload_root)?;
        prepare_private_directory(&scratch_root)?;

        let database_path = current_root.join("fasti.sqlite3");
        reject_unsafe_existing_file(&database_path)?;
        let flags = OpenFlags::default();
        #[cfg(not(target_os = "linux"))]
        let flags = flags | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        // Linux is already rooted through a retained directory descriptor.
        // SQLite rejects NOFOLLOW when `/proc/self/fd` appears in the path,
        // while its final file open still uses the bundled no-follow guard.
        let connection = Connection::open_with_flags(&database_path, flags)?;
        harden_private_regular_file(&database_path)?;
        connection.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let journal_mode: String =
            connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(StoreOpenError::JournalMode(journal_mode));
        }
        connection.pragma_update(None, "synchronous", "FULL")?;
        let synchronous: i64 = connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        if synchronous != 2 {
            return Err(StoreOpenError::SynchronousLevel(synchronous));
        }
        migrate(&connection)?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            return Err(StoreOpenError::SchemaVersion {
                expected: SCHEMA_VERSION,
                actual: version,
            });
        }

        Ok(Self {
            inner: Arc::new(KernelInner {
                current_root,
                payload_root,
                scratch_root,
                connection: Mutex::new(connection),
                upload_budget: Mutex::new(UploadBudget::default()),
                data_root,
            }),
        })
    }

    pub fn data_root(&self) -> &Path {
        self.inner.data_root.path()
    }

    pub fn database_path(&self) -> PathBuf {
        self.inner.current_root.join("fasti.sqlite3")
    }

    pub(crate) fn lock_connection(
        &self,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<MutexGuard<'_, Connection>> {
        self.inner.connection.lock().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })
    }

    pub(crate) fn lock_upload_budget(
        &self,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<MutexGuard<'_, UploadBudget>> {
        self.inner.upload_budget.lock().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })
    }
}

#[cfg(target_os = "linux")]
fn acquire_data_root_lock(data_root: &Path, root_directory: &File) -> Result<File, StoreOpenError> {
    let file = open_data_root_lock(root_directory)?;
    finish_data_root_lock(data_root, file)
}

#[cfg(not(target_os = "linux"))]
fn acquire_data_root_lock(data_root: &Path) -> Result<File, StoreOpenError> {
    let file = open_data_root_lock(data_root)?;
    finish_data_root_lock(data_root, file)
}

fn finish_data_root_lock(data_root: &Path, file: File) -> Result<File, StoreOpenError> {
    if !file.metadata()?.is_file() {
        return Err(unsafe_path(
            &data_root.join("fasti.lock"),
            "expected a regular file",
        ));
    }
    set_owner_only_open_file_permissions(&file)?;
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => StoreOpenError::DataRootLocked,
        std::fs::TryLockError::Error(error) => StoreOpenError::Io(error),
    })?;
    Ok(file)
}

#[cfg(target_os = "linux")]
fn open_data_root_directory(data_root: &Path) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::open(
        data_root,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    let directory = File::from(fd);
    set_owner_only_open_directory_permissions(&directory)?;
    Ok(directory)
}

#[cfg(target_os = "linux")]
fn open_data_root_lock(root_directory: &File) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat2(
        root_directory,
        "fasti.lock",
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from_raw_mode(0o600),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_MAGICLINKS
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_XDEV,
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    Ok(File::from(fd))
}

#[cfg(all(not(target_os = "linux"), unix))]
fn open_data_root_lock(data_root: &Path) -> Result<File, StoreOpenError> {
    let path = data_root.join("fasti.lock");
    reject_unsafe_existing_file(&path)?;
    let fd = rustix::fs::open(
        &path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    Ok(File::from(fd))
}

fn unsafe_path(path: &Path, reason: &'static str) -> StoreOpenError {
    StoreOpenError::UnsafePath {
        path: path.to_path_buf(),
        reason,
    }
}

pub(crate) fn prepare_private_directory(path: &Path) -> Result<(), StoreOpenError> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(unsafe_path(path, "symbolic links are not accepted"));
    }
    if !metadata.is_dir() {
        return Err(unsafe_path(path, "expected a directory"));
    }
    set_owner_only_directory_permissions(path)?;
    Ok(())
}

pub(crate) fn reject_unsafe_existing_file(path: &Path) -> Result<(), StoreOpenError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        return Err(unsafe_path(path, "symbolic links are not accepted"));
    }
    if !metadata.is_file() {
        return Err(unsafe_path(path, "expected a regular file"));
    }
    Ok(())
}

pub(crate) fn harden_private_regular_file(path: &Path) -> Result<(), StoreOpenError> {
    reject_unsafe_existing_file(path)?;
    set_owner_only_file_permissions(path)?;
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_directory_permissions(path: &Path) -> Result<(), StoreOpenError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_directory_permissions(_path: &Path) -> Result<(), StoreOpenError> {
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_file_permissions(path: &Path) -> Result<(), StoreOpenError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_file_permissions(_path: &Path) -> Result<(), StoreOpenError> {
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_open_file_permissions(file: &File) -> Result<(), StoreOpenError> {
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_open_file_permissions(_file: &File) -> Result<(), StoreOpenError> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_owner_only_open_directory_permissions(file: &File) -> Result<(), StoreOpenError> {
    file.set_permissions(fs::Permissions::from_mode(0o700))?;
    Ok(())
}

pub(crate) fn now() -> DateTime<Utc> {
    Utc::now()
}

pub(crate) fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

pub(crate) fn parse_timestamp(
    value: &str,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

pub(crate) fn map_sql<T>(
    result: rusqlite::Result<T>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<T> {
    result.map_err(|_| {
        Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        ))
    })
}

pub(crate) fn map_json<T>(
    result: serde_json::Result<T>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<T> {
    result.map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

pub(crate) fn random_secret(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<fasti_application::SecretMaterial> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| {
        Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        ))
    })?;
    Ok(fasti_application::SecretMaterial::from_bytes(bytes))
}

pub(crate) fn digest_secret(secret: &fasti_application::SecretMaterial) -> String {
    sha256_hex(secret.expose_bytes())
}

pub(crate) fn verify_digest(stored: &str, presented: &str) -> bool {
    constant_time_eq(stored.as_bytes(), presented.as_bytes())
}

pub(crate) fn fsync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

pub(crate) fn scope_storage_key(scope: ScopeKey) -> &'static str {
    match scope {
        ScopeKey::CapabilityRead => "capability_read",
        ScopeKey::ClientEnroll => "client_enroll",
        ScopeKey::ProfileSelect => "profile_select",
        ScopeKey::CredentialManage => "credential_manage",
        ScopeKey::ListenerConfigure => "listener_configure",
        ScopeKey::ObservationAccept => "observation_accept",
        ScopeKey::ReceiptRead => "receipt_read",
        ScopeKey::IdentityWrite => "identity_write",
        ScopeKey::ReviewRead => "review_read",
        ScopeKey::ReviewWrite => "review_write",
        ScopeKey::CorrectionRead => "correction_read",
        ScopeKey::CorrectionWrite => "correction_write",
        ScopeKey::WorkspaceExport => "workspace_export",
        ScopeKey::WorkspaceVerify => "workspace_verify",
    }
}

pub(crate) fn parse_scope(value: &str) -> Option<ScopeKey> {
    ScopeKey::ALL
        .iter()
        .copied()
        .find(|scope| scope_storage_key(*scope) == value)
}

pub(crate) fn load_access_snapshot(
    connection: &Connection,
    access: &RequestAccessContext,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessSnapshot> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT
                    cr.workspace_id,
                    pg.profile_id,
                    cr.client_id,
                    cr.credential_id,
                    pg.grant_id,
                    cr.status,
                    pg.status,
                    c.current_credential_epoch
                FROM credentials cr
                JOIN clients c ON c.client_id = cr.client_id
                JOIN profile_grants pg
                  ON pg.client_id = cr.client_id
                 AND pg.grant_id = ?2
                WHERE cr.credential_id = ?1
                "#,
                rusqlite::params![
                    access.credential_id().to_string(),
                    access.grant_id().to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;

    let Some((
        workspace,
        profile,
        client,
        credential,
        grant,
        credential_status,
        grant_status,
        epoch,
    )) = row
    else {
        return Ok(AccessSnapshot::bootstrap_closed());
    };

    let mut statement = map_sql(
        connection.prepare("SELECT scope_key FROM grant_scopes WHERE grant_id = ?1"),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([grant.as_str()], |row| row.get::<_, String>(0)),
        capability,
        correlation_id,
    )?;
    let mut scopes = Vec::new();
    for value in rows {
        let value = map_sql(value, capability, correlation_id)?;
        let scope = parse_scope(&value)
            .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        scopes.push(scope);
    }

    let workspace_id = workspace
        .parse::<WorkspaceId>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let profile_id = profile
        .parse::<ProfileId>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let client_id = client
        .parse::<ClientId>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let credential_id = credential
        .parse::<CredentialId>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let grant_id = grant
        .parse::<ProfileGrantId>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let epoch = u64::try_from(epoch)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;

    Ok(AccessSnapshot::established(
        workspace_id,
        profile_id,
        client_id,
        credential_id,
        grant_id,
        if credential_status == "active" {
            CredentialStatus::Active
        } else {
            CredentialStatus::Revoked
        },
        if grant_status == "active" {
            GrantStatus::Active
        } else {
            GrantStatus::Revoked
        },
        epoch,
        scopes,
    ))
}

pub(crate) fn authorize_connection(
    connection: &Connection,
    capability: CapabilityKey,
    access: &RequestAccessContext,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessSnapshot> {
    let snapshot = load_access_snapshot(connection, access, capability, correlation_id)?;
    authorize(
        &AuthorizationRequirement::for_capability(capability),
        Some(access),
        Some(&snapshot),
    )
    .map_err(|_| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;
    Ok(snapshot)
}

pub(crate) fn authorize_transaction(
    transaction: &Transaction<'_>,
    capability: CapabilityKey,
    access: &RequestAccessContext,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessSnapshot> {
    authorize_connection(transaction, capability, access, correlation_id)
}

pub(crate) fn problem(
    code: ProblemCode,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(code, capability, correlation_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_root_rejects_a_symbolic_link() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let target = temporary.path().join("target");
        fs::create_dir(&target).expect("target directory");
        let link = temporary.path().join("fasti-data");

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, &link).expect("symbolic link");
            assert!(matches!(
                SqliteKernel::open(&link),
                Err(StoreOpenError::UnsafePath { .. })
            ));
        }

        #[cfg(not(unix))]
        {
            let _ = link;
        }
    }

    #[test]
    fn data_root_lock_excludes_a_second_kernel_and_releases_on_drop() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let guard = LockedDataRoot::acquire(&root).expect("offline data-root guard");
        assert!(!root.join("current").exists());
        let first = SqliteKernel::open_locked(guard).expect("kernel from held guard");

        assert!(matches!(
            SqliteKernel::open(&root),
            Err(StoreOpenError::DataRootLocked)
        ));

        drop(first);
        SqliteKernel::open(&root).expect("lock released with kernel");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn data_root_directory_handle_remains_anchored_after_a_path_rename() {
        use std::os::unix::fs::MetadataExt;

        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let moved = temporary.path().join("moved-fasti-data");
        let guard = LockedDataRoot::acquire(&root).expect("offline data-root guard");
        let anchored = guard
            .anchored_directory()
            .expect("Linux anchored directory");
        let before = anchored.metadata().expect("anchored metadata");

        fs::rename(&root, &moved).expect("rename data root");
        fs::create_dir(&root).expect("replacement directory");

        let after = anchored.metadata().expect("metadata after rename");
        let moved_metadata = fs::metadata(&moved).expect("renamed directory metadata");
        assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
        assert_eq!(
            (after.dev(), after.ino()),
            (moved_metadata.dev(), moved_metadata.ino())
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn daemon_open_refuses_incomplete_restore_staging_before_creating_current() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let guard = LockedDataRoot::acquire(&root).expect("offline data-root guard");
        let anchored = guard.anchored_directory().expect("anchored data root");
        let (_staging, attempt) = crate::archive::create_staging_attempt(
            anchored,
            crate::restore_activation::RESTORE_STAGING_DIRECTORY,
            "attempt-one",
        )
        .expect("restore staging");
        crate::restore_activation::write_restore_phase(
            &attempt,
            fasti_domain::RestoreStatus::Received,
        )
        .expect("received phase");
        drop(attempt);

        assert!(matches!(
            SqliteKernel::open_locked(guard),
            Err(StoreOpenError::RestoreActivation)
        ));
        assert!(!root.join("current").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn daemon_open_completes_a_verified_post_rename_restore_before_sqlite() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let guard = LockedDataRoot::acquire(&root).expect("offline data-root guard");
        let anchored = guard.anchored_directory().expect("anchored data root");
        let (staging, attempt) = crate::archive::create_staging_attempt(
            anchored,
            crate::restore_activation::RESTORE_STAGING_DIRECTORY,
            "attempt-two",
        )
        .expect("restore staging");
        let database = root.join("staging/attempt-two/fasti.sqlite3");
        let connection = Connection::open(&database).expect("staged database");
        migrate(&connection).expect("staged schema");
        drop(connection);
        for status in [
            fasti_domain::RestoreStatus::Received,
            fasti_domain::RestoreStatus::Staging,
            fasti_domain::RestoreStatus::Verified,
        ] {
            crate::restore_activation::write_restore_phase(&attempt, status).expect("phase");
        }
        let marker = crate::restore_activation::RestoreActivationMarker::new(
            fasti_domain::RestoreAttemptId::new_v7(),
            fasti_domain::WorkspaceId::new_v7(),
            0,
            fasti_domain::Sha256Digest::parse(format!("sha256:{}", "11".repeat(32)))
                .expect("archive digest"),
            fasti_domain::Sha256Digest::parse(format!("sha256:{}", "22".repeat(32)))
                .expect("manifest digest"),
        );
        crate::restore_activation::activate_verified_restore(
            anchored,
            &staging,
            &attempt,
            "attempt-two",
            &marker,
        )
        .expect("activate restore");
        fs::remove_file(root.join("current/restore.complete"))
            .expect("simulate crash before COMPLETE");
        drop(attempt);
        drop(staging);

        let moved = temporary.path().join("moved-fasti-data");
        fs::rename(&root, &moved).expect("rename locked data root");
        fs::create_dir(&root).expect("replacement data root");
        fs::write(root.join("replacement-marker"), b"unchanged").expect("replacement marker");

        let kernel = SqliteKernel::open_locked(guard).expect("recovered kernel");
        assert!(moved.join("current/restore.complete").is_file());
        assert!(!root.join("current").exists());
        assert_eq!(
            fs::read(root.join("replacement-marker")).expect("replacement marker"),
            b"unchanged"
        );
        drop(kernel);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn data_root_lock_rejects_a_symbolic_link() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        fs::create_dir(&root).expect("data root");
        let outside = temporary.path().join("outside");
        fs::write(&outside, b"unchanged").expect("outside file");
        std::os::unix::fs::symlink(&outside, root.join("fasti.lock"))
            .expect("hostile lock symlink");

        assert!(SqliteKernel::open(&root).is_err());
        assert_eq!(fs::read(outside).expect("outside bytes"), b"unchanged");
    }

    #[cfg(unix)]
    #[test]
    fn data_root_and_database_are_owner_only() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let kernel = SqliteKernel::open(&root).expect("secure local kernel");

        for directory in [
            root.clone(),
            root.join("current"),
            root.join("current/payloads/sha256"),
            root.join("current/scratch/uploads"),
        ] {
            let mode = fs::metadata(directory)
                .expect("directory metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700);
        }

        let mode = fs::metadata(kernel.database_path())
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}
