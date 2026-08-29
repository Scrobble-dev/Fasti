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

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::fd::AsRawFd;
#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::unix::fs::MetadataExt;

pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_EVIDENCE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_TEMP_EVIDENCE_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum prepared, not-yet-accepted evidence retained by one workspace.
///
/// Accepted Chronicle evidence is not counted. This bounds abandoned upload
/// state before later retention and operator cleanup capabilities exist.
pub const MAX_PREPARED_EVIDENCE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_CONCURRENT_UPLOADS: usize = 4;
const DATA_ROOT_NONCE_BYTES: usize = 32;

#[derive(Debug, Error)]
#[non_exhaustive]
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

/// Stable identity of one opened physical data root.
///
/// The value comes from the retained directory descriptor, not from the
/// configured pathname. It is suitable for local account scoping but is not a
/// portable workspace identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DataRootIdentity([u8; 16 + DATA_ROOT_NONCE_BYTES]);

impl DataRootIdentity {
    pub fn as_bytes(&self) -> &[u8; 16 + DATA_ROOT_NONCE_BYTES] {
        &self.0
    }
}

/// Exclusive access to one data root without creating or opening `current/`.
///
/// The daemon kernel and stopped-node CLI use this same guard so restore can
/// prove the daemon is stopped before it inspects staging or active data.
/// On Linux and Android, the lock follows the opened physical root across a
/// rename; a replacement at the configured pathname is a distinct root.
#[derive(Debug)]
pub struct LockedDataRoot {
    path: PathBuf,
    identity: DataRootIdentity,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    root_directory: File,
    _lock: File,
}

impl LockedDataRoot {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self, StoreOpenError> {
        let path = path.as_ref().to_path_buf();
        if path.file_name().is_none() {
            return Err(unsafe_path(
                &path,
                "data root must have a final path component",
            ));
        }
        prepare_private_directory(&path)?;
        let root_directory = open_data_root_directory(&path)?;
        let path = fs::read_link(format!("/proc/self/fd/{}", root_directory.as_raw_fd()))?;
        let mut lock = acquire_data_root_lock(&path, &root_directory)?;
        let identity = data_root_identity(&path, &root_directory, &mut lock)?;
        Ok(Self {
            path,
            identity,
            root_directory,
            _lock: lock,
        })
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub fn acquire(_path: impl AsRef<Path>) -> Result<Self, StoreOpenError> {
        Err(StoreOpenError::UnsupportedPlatform)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn identity(&self) -> DataRootIdentity {
        self.identity
    }

    /// Returns an anchored data-root directory handle where descriptor-relative
    /// filesystem operations are supported.
    ///
    /// Linux restore and recovery code must use this handle instead of
    /// resolving child paths from [`Self::path`] again. Android retains a
    /// directory handle for kernel storage but does not expose Linux-only B3
    /// restore behavior through this method.
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
}

#[derive(Debug)]
pub(crate) struct KernelInner {
    pub(crate) current_root: PathBuf,
    pub(crate) scratch_root: PathBuf,
    pub(crate) connection: Mutex<Connection>,
    pub(crate) upload_budget: Mutex<UploadBudget>,
    // Serializes ensure_bootstrap_secret's read-validate-recover sequence.
    // Without it, two concurrent callers can each see the same malformed
    // file, and the one that loses the race can delete a secret the other
    // just published, then republish a different one -- see
    // ensure_bootstrap_secret's recovery loop.
    pub(crate) bootstrap_secret: Mutex<()>,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    _current_directory: File,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    payload_directory: File,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    _scratch_directory: File,
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
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let current_directory =
            open_or_create_kernel_directory(&data_root.root_directory, "current")?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let payloads_directory = open_or_create_kernel_directory(&current_directory, "payloads")?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let payload_directory = open_or_create_kernel_directory(&payloads_directory, "sha256")?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let scratch_base = open_or_create_kernel_directory(&current_directory, "scratch")?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let scratch_directory = open_or_create_kernel_directory(&scratch_base, "uploads")?;

        #[cfg(any(target_os = "linux", target_os = "android"))]
        let current_root = descriptor_path(&current_directory);
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let scratch_root = descriptor_path(&scratch_directory);

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let current_root = data_root.path.join("current");
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let payload_root = current_root.join("payloads").join("sha256");
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let scratch_root = current_root.join("scratch").join("uploads");
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            prepare_private_directory(&current_root)?;
            prepare_private_directory(&payload_root)?;
            prepare_private_directory(&scratch_root)?;
        }

        let database_path = current_root.join("fasti.sqlite3");
        // Open the final path component atomically relative to the held
        // current-directory descriptor (NOFOLLOW + BENEATH + NO_SYMLINKS) on
        // Linux/Android, so a same-user process cannot replace fasti.sqlite3
        // with a symlink between a path-based check and SQLite's own open.
        // SQLITE_OPEN_NOFOLLOW is intentionally not set on those platforms:
        // bundled SQLite >= 3.39.3 canonicalizes the whole path before
        // opening, which rejects the /proc/self/fd/<fd> descriptor path this
        // guard hands it. The openat2 resolve flags already give the
        // NOFOLLOW guarantee for the real filesystem entry.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let database_file = open_kernel_database_file(&current_directory, "fasti.sqlite3")?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let connection = {
            set_owner_only_open_file_permissions(&database_file)?;
            Connection::open_with_flags(descriptor_path(&database_file), OpenFlags::default())?
        };
        // database_file stays alive until here so /proc/self/fd/<fd> stays
        // resolvable for the open() call above; SQLite holds its own fd
        // after that, so database_file can now be dropped normally.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        drop(database_file);

        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        reject_unsafe_existing_file(&database_path)?;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        let flags = OpenFlags::default() | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
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
                scratch_root,
                connection: Mutex::new(connection),
                upload_budget: Mutex::new(UploadBudget::default()),
                bootstrap_secret: Mutex::new(()),
                #[cfg(any(target_os = "linux", target_os = "android"))]
                _current_directory: current_directory,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                payload_directory,
                #[cfg(any(target_os = "linux", target_os = "android"))]
                _scratch_directory: scratch_directory,
                data_root,
            }),
        })
    }

    pub fn data_root(&self) -> &Path {
        self.inner.data_root.path()
    }

    pub fn data_root_identity(&self) -> DataRootIdentity {
        self.inner.data_root.identity()
    }

    pub fn database_path(&self) -> PathBuf {
        self.inner.current_root.join("fasti.sqlite3")
    }

    pub(crate) fn prepare_evidence_destination(
        &self,
        digest_hex: &str,
    ) -> Result<(File, PathBuf), StoreOpenError> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            validate_evidence_digest(digest_hex)?;
            let (prefix, created) =
                open_or_create_private_directory(&self.inner.payload_directory, &digest_hex[..2])?;
            if created {
                self.inner.payload_directory.sync_all()?;
            }
            let destination = descriptor_path(&prefix).join(digest_hex);
            Ok((prefix, destination))
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let _ = digest_hex;
            Err(StoreOpenError::UnsupportedPlatform)
        }
    }

    pub(crate) fn open_evidence_file(&self, digest_hex: &str) -> Result<File, StoreOpenError> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            validate_evidence_digest(digest_hex)?;
            let prefix = open_kernel_directory(&self.inner.payload_directory, &digest_hex[..2])?;
            self.open_evidence_file_at(&prefix, digest_hex)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let _ = digest_hex;
            Err(StoreOpenError::UnsupportedPlatform)
        }
    }

    pub(crate) fn open_evidence_file_at(
        &self,
        prefix_directory: &File,
        digest_hex: &str,
    ) -> Result<File, StoreOpenError> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            validate_evidence_digest(digest_hex)?;
            open_kernel_regular_file(prefix_directory, digest_hex)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            let _ = (prefix_directory, digest_hex);
            Err(StoreOpenError::UnsupportedPlatform)
        }
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

    pub(crate) fn lock_bootstrap_secret(
        &self,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<MutexGuard<'_, ()>> {
        self.inner.bootstrap_secret.lock().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn acquire_data_root_lock(data_root: &Path, root_directory: &File) -> Result<File, StoreOpenError> {
    let file = open_data_root_lock(root_directory)?;
    finish_data_root_lock(data_root, file)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_data_root_directory(data_root: &Path) -> Result<File, StoreOpenError> {
    let directory = open_directory(data_root)?;
    set_owner_only_open_directory_permissions(&directory)?;
    Ok(directory)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_directory(path: &Path) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    Ok(File::from(fd))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[cfg(any(target_os = "linux", target_os = "android"))]
fn data_root_identity(
    data_root: &Path,
    root_directory: &File,
    lock: &mut File,
) -> Result<DataRootIdentity, StoreOpenError> {
    let metadata = root_directory.metadata()?;
    let nonce = data_root_nonce(data_root, root_directory, lock)?;
    Ok(data_root_identity_from_parts(
        metadata.dev(),
        metadata.ino(),
        nonce,
    ))
}

fn data_root_identity_from_parts(
    device: u64,
    inode: u64,
    nonce: [u8; DATA_ROOT_NONCE_BYTES],
) -> DataRootIdentity {
    let mut value = [0_u8; 16 + DATA_ROOT_NONCE_BYTES];
    value[..8].copy_from_slice(&device.to_be_bytes());
    value[8..16].copy_from_slice(&inode.to_be_bytes());
    value[16..].copy_from_slice(&nonce);
    DataRootIdentity(value)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn data_root_nonce(
    data_root: &Path,
    root_directory: &File,
    lock: &mut File,
) -> Result<[u8; DATA_ROOT_NONCE_BYTES], StoreOpenError> {
    let lock_path = data_root.join("fasti.lock");
    let length = lock.metadata()?.len();
    let mut nonce = [0_u8; DATA_ROOT_NONCE_BYTES];
    lock.seek(SeekFrom::Start(0))?;
    match length {
        0 => {
            getrandom::fill(&mut nonce).map_err(|_| {
                StoreOpenError::Io(std::io::Error::other(
                    "the operating system random source is unavailable",
                ))
            })?;
            lock.write_all(&nonce)?;
            lock.sync_all()?;
            root_directory.sync_all()?;
        }
        length if length == DATA_ROOT_NONCE_BYTES as u64 => lock.read_exact(&mut nonce)?,
        _ => {
            return Err(unsafe_path(
                &lock_path,
                "expected an empty legacy lock or a 32-byte data-root nonce",
            ))
        }
    }
    Ok(nonce)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn descriptor_path(directory: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_or_create_kernel_directory(parent: &File, name: &str) -> Result<File, StoreOpenError> {
    open_or_create_private_directory(parent, name).map(|(directory, _created)| directory)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn open_or_create_private_directory(
    parent: &File,
    name: &str,
) -> Result<(File, bool), StoreOpenError> {
    let created = match rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o700)) {
        Ok(()) => true,
        Err(rustix::io::Errno::EXIST) => false,
        Err(error) => {
            return Err(StoreOpenError::Io(std::io::Error::from_raw_os_error(
                error.raw_os_error(),
            )))
        }
    };
    let directory = open_kernel_directory(parent, name)?;
    set_owner_only_open_directory_permissions(&directory)?;
    Ok((directory, created))
}

fn validate_evidence_digest(digest_hex: &str) -> Result<(), StoreOpenError> {
    if digest_hex.len() == 64
        && digest_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(unsafe_path(
            Path::new(digest_hex),
            "expected a canonical SHA-256 digest",
        ))
    }
}

#[cfg(target_os = "linux")]
fn open_kernel_directory(parent: &File, name: &str) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat2(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_MAGICLINKS
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_XDEV,
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    Ok(File::from(fd))
}

#[cfg(target_os = "linux")]
fn open_kernel_regular_file(parent: &File, name: &str) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat2(
        parent,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_MAGICLINKS
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_XDEV,
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    finish_regular_file(File::from(fd), name)
}

#[cfg(target_os = "android")]
fn open_kernel_directory(parent: &File, name: &str) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    Ok(File::from(fd))
}

#[cfg(target_os = "android")]
fn open_kernel_regular_file(parent: &File, name: &str) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    finish_regular_file(File::from(fd), name)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn finish_regular_file(file: File, name: &str) -> Result<File, StoreOpenError> {
    if file.metadata()?.is_file() {
        Ok(file)
    } else {
        Err(unsafe_path(Path::new(name), "expected a regular file"))
    }
}

#[cfg(target_os = "linux")]
fn open_kernel_database_file(parent: &File, name: &str) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat2(
        parent,
        name,
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
    finish_regular_file(File::from(fd), name)
}

#[cfg(target_os = "android")]
fn open_kernel_database_file(parent: &File, name: &str) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map_err(|error| StoreOpenError::Io(std::io::Error::from_raw_os_error(error.raw_os_error())))?;
    finish_regular_file(File::from(fd), name)
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

#[cfg(target_os = "android")]
fn open_data_root_lock(root_directory: &File) -> Result<File, StoreOpenError> {
    open_data_root_lock_with_openat(root_directory)
}

#[cfg(any(target_os = "android", all(test, target_os = "linux")))]
fn open_data_root_lock_with_openat(root_directory: &File) -> Result<File, StoreOpenError> {
    let fd = rustix::fs::openat(
        root_directory,
        "fasti.lock",
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_owner_only_directory_permissions(path: &Path) -> Result<(), StoreOpenError> {
    let directory = open_directory(path)?;
    set_owner_only_open_directory_permissions(&directory)
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn set_owner_only_open_file_permissions(file: &File) -> Result<(), StoreOpenError> {
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
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
        ScopeKey::IdentityRead => "identity_read",
        ScopeKey::ProfileStateRead => "profile_state_read",
        ScopeKey::ProfileStateWrite => "profile_state_write",
        ScopeKey::ReviewRead => "review_read",
        ScopeKey::ReviewWrite => "review_write",
        ScopeKey::CorrectionRead => "correction_read",
        ScopeKey::CorrectionWrite => "correction_write",
        ScopeKey::WorkspaceExport => "workspace_export",
        ScopeKey::WorkspaceVerify => "workspace_verify",
        ScopeKey::BrowserUserManage => "browser_user_manage",
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
    fn data_root_identity_includes_the_persisted_nonce() {
        let first = data_root_identity_from_parts(7, 11, [1; DATA_ROOT_NONCE_BYTES]);
        let second = data_root_identity_from_parts(7, 11, [2; DATA_ROOT_NONCE_BYTES]);

        assert_ne!(first, second);
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    #[test]
    fn unsupported_platform_fails_before_touching_the_data_root() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");

        assert!(matches!(
            LockedDataRoot::acquire(&root),
            Err(StoreOpenError::UnsupportedPlatform)
        ));
        assert!(!root.exists());
    }

    #[cfg(target_os = "linux")]
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

    #[cfg(target_os = "linux")]
    #[test]
    fn data_root_reports_the_opened_physical_path() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let configured = temporary.path().join(".").join("fasti-data");
        assert!(!configured.exists());

        let guard = LockedDataRoot::acquire(&configured).expect("offline data-root guard");
        assert_eq!(
            guard.path(),
            configured.canonicalize().expect("physical path")
        );
    }

    #[cfg(target_os = "linux")]
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

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn android_compatible_data_root_is_locked_and_descriptor_anchored() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let moved = temporary.path().join("moved-fasti-data");
        prepare_private_directory(&root).expect("private data root");
        let root_directory = open_data_root_directory(&root).expect("data-root directory");

        let first = open_data_root_lock_with_openat(&root_directory).expect("first lock file");
        let mut first = finish_data_root_lock(&root, first).expect("first lock");
        let guard = LockedDataRoot {
            path: root.canonicalize().expect("physical path"),
            identity: data_root_identity(&root, &root_directory, &mut first)
                .expect("root identity"),
            root_directory,
            _lock: first,
        };
        let second =
            open_data_root_lock_with_openat(&guard.root_directory).expect("second lock file");
        assert!(matches!(
            finish_data_root_lock(&root, second),
            Err(StoreOpenError::DataRootLocked)
        ));

        fs::rename(&root, &moved).expect("rename data root");
        fs::create_dir(&root).expect("replacement data root");
        let kernel = SqliteKernel::open_locked(guard).expect("descriptor-rooted kernel");
        assert!(moved.join("current").is_dir());
        assert!(moved.join("current/fasti.sqlite3").is_file());
        assert!(!root.join("current").exists());

        drop(kernel);
        let moved_directory = open_data_root_directory(&moved).expect("moved data-root directory");
        let released =
            open_data_root_lock_with_openat(&moved_directory).expect("released lock file");
        let released = finish_data_root_lock(&moved, released).expect("released lock");
        drop(released);

        fs::remove_file(moved.join("fasti.lock")).expect("remove lock file");
        let outside = temporary.path().join("outside");
        fs::write(&outside, b"unchanged").expect("outside file");
        std::os::unix::fs::symlink(&outside, moved.join("fasti.lock"))
            .expect("hostile lock symlink");
        assert!(open_data_root_lock_with_openat(&moved_directory).is_err());
        assert_eq!(fs::read(outside).expect("outside bytes"), b"unchanged");
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn kernel_directory_creation_rejects_an_intermediate_symlink() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(root.join("current")).expect("current directory");
        fs::create_dir(&outside).expect("outside directory");
        std::os::unix::fs::symlink(&outside, root.join("current/payloads"))
            .expect("hostile payloads symlink");

        assert!(SqliteKernel::open(&root).is_err());
        assert!(!outside.join("sha256").exists());
        assert!(!outside.join("fasti.sqlite3").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn database_open_rejects_a_symlinked_final_component() {
        // Regression: kernel.rs open_locked previously checked
        // fasti.sqlite3 via a path-based reject_unsafe_existing_file, then
        // opened it via a second, separate path lookup -- a same-user
        // process could replace the file with a symlink between the two.
        // The fix opens the final component atomically (openat2 with
        // NOFOLLOW|NO_SYMLINKS|BENEATH) relative to a held directory
        // descriptor. This test pre-plants the hostile symlink before any
        // open happens, so it fails closed whether the guard is atomic or
        // just an early check -- it exists to catch a future refactor that
        // drops back to a plain path-based Connection::open_with_flags.
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let outside = temporary.path().join("outside.sqlite3");
        fs::create_dir_all(root.join("current")).expect("current directory");
        fs::write(&outside, b"not a fasti database").expect("outside file");
        std::os::unix::fs::symlink(&outside, root.join("current/fasti.sqlite3"))
            .expect("hostile database symlink");

        assert!(SqliteKernel::open(&root).is_err());
        assert_eq!(
            fs::read(&outside).expect("outside bytes"),
            b"not a fasti database"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn evidence_prefix_and_file_symlinks_are_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let outside = temporary.path().join("outside");
        let kernel = SqliteKernel::open(&root).expect("kernel");
        let digest = format!("ab{}", "0".repeat(62));
        fs::create_dir(&outside).expect("outside directory");
        std::os::unix::fs::symlink(&outside, root.join("current/payloads/sha256/ab"))
            .expect("hostile prefix symlink");

        assert!(kernel.prepare_evidence_destination(&digest).is_err());
        assert!(!outside.join(&digest).exists());

        fs::remove_file(root.join("current/payloads/sha256/ab")).expect("remove prefix symlink");
        fs::create_dir(root.join("current/payloads/sha256/ab")).expect("prefix directory");
        let outside_file = outside.join("payload");
        fs::write(&outside_file, b"unchanged").expect("outside payload");
        std::os::unix::fs::symlink(
            &outside_file,
            root.join("current/payloads/sha256/ab").join(&digest),
        )
        .expect("hostile evidence symlink");

        assert!(kernel.open_evidence_file(&digest).is_err());
        assert_eq!(
            fs::read(outside_file).expect("outside payload"),
            b"unchanged"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn data_root_rejects_a_path_without_a_final_component() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let child = temporary.path().join("child");
        assert!(matches!(
            SqliteKernel::open(child.join("..")),
            Err(StoreOpenError::UnsafePath { .. })
        ));
        assert!(!child.exists());
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
        assert!(matches!(
            LockedDataRoot::acquire(&moved),
            Err(StoreOpenError::DataRootLocked)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sqlite_sidecar_reopen_fails_closed_when_the_database_path_is_replaced() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("fasti-data");
        let moved = temporary.path().join("moved-fasti-data");
        let kernel = SqliteKernel::open(&root).expect("kernel");
        let connection = kernel.inner.connection.lock().expect("connection");
        let mode: String = connection
            .query_row("PRAGMA journal_mode = DELETE", [], |row| row.get(0))
            .expect("close WAL sidecars");
        assert!(mode.eq_ignore_ascii_case("delete"));

        fs::rename(&root, &moved).expect("rename data root");
        fs::create_dir_all(root.join("current")).expect("replacement current directory");

        let error = connection
            .query_row("PRAGMA journal_mode = WAL", [], |row| {
                row.get::<_, String>(0)
            })
            .expect_err("moved database must reject a sidecar reopen");
        assert!(matches!(
            error,
            rusqlite::Error::SqliteFailure(error, _)
                if error.extended_code == rusqlite::ffi::SQLITE_READONLY_DBMOVED
        ));
        assert!(!root.join("current/fasti.sqlite3-wal").exists());
        assert!(!root.join("current/fasti.sqlite3-shm").exists());
        assert!(!root.join("current/fasti.sqlite3-journal").exists());
        assert!(moved.join("current/fasti.sqlite3").is_file());
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

    #[cfg(target_os = "linux")]
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

        for file in [root.join("fasti.lock"), kernel.database_path()] {
            let mode = fs::metadata(file)
                .expect("private file metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
        assert_eq!(
            fs::metadata(root.join("fasti.lock"))
                .expect("lock metadata")
                .len(),
            DATA_ROOT_NONCE_BYTES as u64
        );
    }
}
