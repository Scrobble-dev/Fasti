use crate::kernel::{harden_private_regular_file, reject_unsafe_existing_file, SqliteKernel};
use crate::schema::SCHEMA_VERSION;
use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};
use std::fs::{self, OpenOptions};
use std::num::NonZeroU32;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const TRANSIENT_RETRY_DELAY: Duration = Duration::from_millis(10);
const INTEGRITY_PROGRESS_OPS: i32 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotLimits {
    pages_per_step: NonZeroU32,
    max_step_time: Duration,
    max_total_time: Duration,
}

impl SnapshotLimits {
    pub fn new(
        pages_per_step: NonZeroU32,
        max_step_time: Duration,
        max_total_time: Duration,
    ) -> Result<Self, SnapshotError> {
        if pages_per_step.get() > i32::MAX as u32 {
            return Err(SnapshotError::InvalidLimit(
                "pages_per_step exceeds i32::MAX",
            ));
        }
        if max_step_time.is_zero() {
            return Err(SnapshotError::InvalidLimit("max_step_time must be nonzero"));
        }
        if max_total_time.is_zero() {
            return Err(SnapshotError::InvalidLimit(
                "max_total_time must be nonzero",
            ));
        }
        Ok(Self {
            pages_per_step,
            max_step_time,
            max_total_time,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotProgress {
    pub remaining_pages: u32,
    pub total_pages: u32,
    pub elapsed: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotMetadata {
    pub page_count: u32,
    pub byte_len: u64,
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("invalid snapshot limit: {0}")]
    InvalidLimit(&'static str),
    #[error("snapshot destination already exists: {0:?}")]
    DestinationExists(PathBuf),
    #[error("snapshot was cancelled")]
    Cancelled,
    #[error("snapshot backup step returned {0}")]
    Busy(&'static str),
    #[error("snapshot backup step exceeded its time limit")]
    StepTimeout,
    #[error("snapshot exceeded its overall time limit")]
    OverallTimeout,
    #[error("snapshot schema version {actual} does not match expected version {expected}")]
    SchemaVersion { expected: i64, actual: i64 },
    #[error("snapshot integrity check failed: {0}")]
    Integrity(String),
    #[error("snapshot progress was invalid")]
    InvalidProgress,
    #[error("snapshot filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to remove incomplete snapshot {path:?}: {source}")]
    Cleanup {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("snapshot SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("snapshot store path preparation failed: {0}")]
    Store(#[from] crate::StoreOpenError),
}

impl SqliteKernel {
    /// Copies the live database into a new SQLite file without holding the
    /// kernel writer mutex while the backup runs.
    ///
    /// Deadlines are checked between backup steps and by SQLite during the
    /// integrity check. One native backup step or filesystem call cannot be
    /// preempted, so callers must also keep the page limit small.
    pub fn snapshot_database<F>(
        &self,
        destination: impl AsRef<Path>,
        limits: SnapshotLimits,
        mut between_steps: F,
    ) -> Result<SnapshotMetadata, SnapshotError>
    where
        F: FnMut(SnapshotProgress) -> ControlFlow<()>,
    {
        let source_path = self.database_path();
        reject_unsafe_existing_file(&source_path)?;

        let destination = destination.as_ref();
        let mut created = IncompleteDestination::create(destination)?;
        let result = (|| {
            let started = Instant::now();
            let source = Connection::open_with_flags(
                &source_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            source.busy_timeout(limits.max_step_time.min(limits.max_total_time))?;
            source.pragma_update(None, "query_only", "ON")?;

            let mut target = Connection::open_with_flags(
                destination,
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            target.busy_timeout(limits.max_step_time.min(limits.max_total_time))?;

            let page_count = {
                let backup = Backup::new(&source, &mut target)?;
                loop {
                    if started.elapsed() >= limits.max_total_time {
                        return Err(SnapshotError::OverallTimeout);
                    }

                    let step_started = Instant::now();
                    let result = backup.step(limits.pages_per_step.get() as i32)?;
                    let step_elapsed = step_started.elapsed();
                    if step_elapsed >= limits.max_step_time {
                        return Err(SnapshotError::StepTimeout);
                    }
                    if started.elapsed() >= limits.max_total_time {
                        return Err(SnapshotError::OverallTimeout);
                    }

                    let progress = checked_progress(backup.progress(), started.elapsed())?;
                    match result {
                        StepResult::Done => break progress.total_pages,
                        result => {
                            monitor_incomplete_step(result, progress, &mut between_steps)?;
                            let elapsed = started.elapsed();
                            if elapsed >= limits.max_total_time {
                                return Err(SnapshotError::OverallTimeout);
                            }
                            if let Some(delay) = transient_retry_delay(
                                result,
                                limits.max_total_time.saturating_sub(elapsed),
                            ) {
                                thread::sleep(delay);
                            }
                        }
                    }
                }
            };

            verify_snapshot(&target, started, limits.max_total_time)?;
            drop(target);
            Ok(SnapshotMetadata {
                page_count,
                byte_len: fs::metadata(destination)?.len(),
            })
        })();

        match result {
            Ok(metadata) => {
                created.keep();
                Ok(metadata)
            }
            Err(error) => match created.remove() {
                Ok(()) => Err(error),
                Err(source) => Err(SnapshotError::Cleanup {
                    path: destination.to_path_buf(),
                    source,
                }),
            },
        }
    }
}

fn monitor_incomplete_step<F>(
    result: StepResult,
    progress: SnapshotProgress,
    between_steps: &mut F,
) -> Result<(), SnapshotError>
where
    F: FnMut(SnapshotProgress) -> ControlFlow<()>,
{
    if !matches!(
        result,
        StepResult::More | StepResult::Busy | StepResult::Locked
    ) {
        return Err(SnapshotError::Busy("unknown transient state"));
    }
    if between_steps(progress).is_break() {
        return Err(SnapshotError::Cancelled);
    }
    Ok(())
}

fn transient_retry_delay(result: StepResult, remaining: Duration) -> Option<Duration> {
    matches!(result, StepResult::Busy | StepResult::Locked)
        .then(|| TRANSIENT_RETRY_DELAY.min(remaining))
}

fn checked_progress(
    progress: rusqlite::backup::Progress,
    elapsed: Duration,
) -> Result<SnapshotProgress, SnapshotError> {
    let remaining_pages =
        u32::try_from(progress.remaining).map_err(|_| SnapshotError::InvalidProgress)?;
    let total_pages =
        u32::try_from(progress.pagecount).map_err(|_| SnapshotError::InvalidProgress)?;
    if remaining_pages > total_pages {
        return Err(SnapshotError::InvalidProgress);
    }
    Ok(SnapshotProgress {
        remaining_pages,
        total_pages,
        elapsed,
    })
}

fn verify_snapshot(
    connection: &Connection,
    started: Instant,
    max_total_time: Duration,
) -> Result<(), SnapshotError> {
    connection.progress_handler(
        INTEGRITY_PROGRESS_OPS,
        Some(move || started.elapsed() >= max_total_time),
    );
    let verification = verify_snapshot_inner(connection);
    connection.progress_handler(0, None::<fn() -> bool>);
    if started.elapsed() >= max_total_time {
        return Err(SnapshotError::OverallTimeout);
    }
    verification
}

fn verify_snapshot_inner(connection: &Connection) -> Result<(), SnapshotError> {
    let actual = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if actual != SCHEMA_VERSION {
        return Err(SnapshotError::SchemaVersion {
            expected: SCHEMA_VERSION,
            actual,
        });
    }
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(SnapshotError::Integrity(integrity));
    }
    Ok(())
}

struct IncompleteDestination {
    path: PathBuf,
    keep: bool,
}

impl IncompleteDestination {
    fn create(path: &Path) -> Result<Self, SnapshotError> {
        for sidecar in destination_files(path).into_iter().skip(1) {
            match fs::symlink_metadata(&sidecar) {
                Ok(_) => return Err(SnapshotError::DestinationExists(sidecar)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(file) => drop(file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(SnapshotError::DestinationExists(path.to_path_buf()));
            }
            Err(error) => return Err(error.into()),
        }
        let created = Self {
            path: path.to_path_buf(),
            keep: false,
        };
        harden_private_regular_file(path)?;
        Ok(created)
    }

    fn keep(&mut self) {
        self.keep = true;
    }

    fn remove(&mut self) -> std::io::Result<()> {
        let mut first_error = None;
        for path in destination_files(&self.path).into_iter().rev() {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        self.keep = true;
        Ok(())
    }
}

impl Drop for IncompleteDestination {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        let _ = self.remove();
    }
}

fn destination_files(path: &Path) -> [PathBuf; 4] {
    fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
        let mut value = path.as_os_str().to_os_string();
        value.push(suffix);
        PathBuf::from(value)
    }
    [
        path.to_path_buf(),
        with_suffix(path, "-journal"),
        with_suffix(path, "-wal"),
        with_suffix(path, "-shm"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroU32;
    use std::sync::mpsc;
    use std::thread;

    fn limits() -> SnapshotLimits {
        SnapshotLimits::new(
            NonZeroU32::new(1).expect("nonzero"),
            Duration::from_secs(2),
            Duration::from_secs(10),
        )
        .expect("snapshot limits")
    }

    fn loaded_kernel() -> (tempfile::TempDir, SqliteKernel) {
        let root = tempfile::tempdir().expect("temporary root");
        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
        let connection = kernel.inner.connection.lock().expect("writer connection");
        connection
            .execute_batch(
                "CREATE TABLE snapshot_load(value BLOB NOT NULL);\n\
                 INSERT INTO snapshot_load(value) VALUES (zeroblob(2097152));",
            )
            .expect("seed snapshot pages");
        drop(connection);
        (root, kernel)
    }

    #[test]
    fn online_snapshot_allows_a_concurrent_writer_between_steps() {
        let (root, kernel) = loaded_kernel();
        let destination = root.path().join("snapshot.sqlite3");
        let writer_kernel = kernel.clone();
        let (start_tx, start_rx) = mpsc::sync_channel(0);
        let (done_tx, done_rx) = mpsc::sync_channel(0);
        let writer = thread::spawn(move || {
            start_rx.recv().expect("backup started");
            let connection = writer_kernel
                .inner
                .connection
                .lock()
                .expect("writer connection");
            connection
                .execute("INSERT INTO snapshot_load(value) VALUES (X'01')", [])
                .expect("concurrent write");
            done_tx.send(()).expect("report completed write");
        });

        let mut notified = false;
        let metadata = kernel
            .snapshot_database(&destination, limits(), |_| {
                if !notified {
                    notified = true;
                    start_tx.send(()).expect("start writer");
                    done_rx
                        .recv_timeout(Duration::from_secs(2))
                        .expect("writer was not blocked by the kernel mutex");
                }
                ControlFlow::Continue(())
            })
            .expect("online snapshot");
        writer.join().expect("writer thread");

        assert!(notified);
        assert!(metadata.page_count > 1);
        assert!(metadata.byte_len > 0);
        let snapshot = Connection::open(&destination).expect("open snapshot");
        let integrity: String = snapshot
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check");
        assert_eq!(integrity, "ok");
    }

    #[test]
    fn transient_steps_call_the_monitor_and_honor_cancellation() {
        let progress = SnapshotProgress {
            remaining_pages: 2,
            total_pages: 3,
            elapsed: Duration::from_millis(1),
        };
        let mut calls = 0;
        for result in [StepResult::Busy, StepResult::Locked] {
            monitor_incomplete_step(result, progress, &mut |_| {
                calls += 1;
                ControlFlow::Continue(())
            })
            .expect("retry transient step");
        }
        assert_eq!(calls, 2);
        assert_eq!(
            transient_retry_delay(StepResult::Busy, Duration::from_millis(3)),
            Some(Duration::from_millis(3))
        );
        assert_eq!(
            transient_retry_delay(StepResult::Locked, Duration::from_secs(1)),
            Some(TRANSIENT_RETRY_DELAY)
        );
        assert_eq!(
            transient_retry_delay(StepResult::More, Duration::from_secs(1)),
            None
        );

        let error =
            monitor_incomplete_step(StepResult::Busy, progress, &mut |_| ControlFlow::Break(()))
                .expect_err("cancel transient retry");
        assert!(matches!(error, SnapshotError::Cancelled));
    }

    #[test]
    fn snapshot_verification_honors_an_expired_deadline() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .pragma_update(None, "user_version", SCHEMA_VERSION)
            .expect("schema version");

        let error = verify_snapshot(&connection, Instant::now(), Duration::ZERO)
            .expect_err("expired verification deadline");

        assert!(matches!(error, SnapshotError::OverallTimeout));
    }

    #[test]
    fn cancellation_removes_the_incomplete_destination() {
        let (root, kernel) = loaded_kernel();
        let destination = root.path().join("cancelled.sqlite3");
        let mut callbacks = 0;
        let mut injected_sidecar = None;

        let error = kernel
            .snapshot_database(&destination, limits(), |_| {
                callbacks += 1;
                let sidecar = destination_files(&destination)
                    .into_iter()
                    .skip(1)
                    .find(|path| !path.exists())
                    .expect("available SQLite sidecar name");
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&sidecar)
                    .expect("create incomplete sidecar");
                injected_sidecar = Some(sidecar);
                ControlFlow::Break(())
            })
            .expect_err("cancel snapshot");

        assert!(matches!(error, SnapshotError::Cancelled));
        assert_eq!(callbacks, 1);
        assert!(injected_sidecar.is_some());
        for path in destination_files(&destination) {
            assert!(!path.exists(), "incomplete snapshot remained at {path:?}");
        }
    }

    #[test]
    fn overall_timeout_removes_the_incomplete_destination() {
        let (root, kernel) = loaded_kernel();
        let destination = root.path().join("timed-out.sqlite3");
        let limits = SnapshotLimits::new(
            NonZeroU32::new(1).expect("nonzero"),
            Duration::from_secs(1),
            Duration::from_millis(100),
        )
        .expect("snapshot limits");

        let error = kernel
            .snapshot_database(&destination, limits, |_| {
                thread::sleep(Duration::from_millis(150));
                ControlFlow::Continue(())
            })
            .expect_err("time out snapshot");

        assert!(matches!(error, SnapshotError::OverallTimeout));
        assert!(!destination.exists());
    }

    #[test]
    fn limits_reject_zero_durations_and_unrepresentable_page_counts() {
        let page = NonZeroU32::new(1).expect("nonzero");
        assert!(SnapshotLimits::new(page, Duration::ZERO, Duration::from_secs(1)).is_err());
        assert!(SnapshotLimits::new(page, Duration::from_secs(1), Duration::ZERO).is_err());
        assert!(SnapshotLimits::new(
            NonZeroU32::new(i32::MAX as u32 + 1).expect("nonzero"),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .is_err());
    }
}
