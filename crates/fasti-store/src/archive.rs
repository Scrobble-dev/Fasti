//! Strict low-level primitives for the B3 `.fasti` archive profile.
//!
//! This module owns framing, hostile-input checks, and atomic archive-file
//! publication. Database snapshot, restore orchestration, authorization, and
//! startup recovery remain outside this boundary.

use fasti_application::{WorkspaceArchiveCompletionError, WorkspaceArchiveDestination};
use fasti_domain::Sha256Digest;
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::rc::Rc;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use thiserror::Error;

pub const MAX_IO_CHUNK_BYTES: usize = 256 * 1024;
pub const ZSTD_COMPRESSION_LEVEL: i32 = 3;
pub const ZSTD_WINDOW_LOG: u32 = 22;
const USTAR_PATH_BYTES: usize = 100;
const TAR_TRAILER_BYTES: u64 = 1024;
const TAR_REMAINING_ZERO_BYTES: u64 = 512;

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("archive I/O failed")]
    Io(#[from] io::Error),
    #[error("archive limits must all be non-zero")]
    InvalidLimits,
    #[error("archive entry path is not canonical: {0}")]
    InvalidPath(String),
    #[error("archive entry type {0:#04x} is not a regular file")]
    UnsupportedEntryType(u8),
    #[error("archive entry does not use the USTAR header profile")]
    NonUstarHeader,
    #[error("archive entry metadata is not canonical")]
    NonCanonicalHeader,
    #[error("archive contains duplicate entry {0}")]
    DuplicateEntry(String),
    #[error("archive entry count exceeds {limit}")]
    EntryCountExceeded { limit: u64 },
    #[error("archive entry {path} is {size} bytes; limit is {limit}")]
    EntrySizeExceeded { path: String, size: u64, limit: u64 },
    #[error("archive expanded bytes exceed {limit}")]
    ExpandedSizeExceeded { limit: u64 },
    #[error("archive compressed bytes exceed {limit}")]
    CompressedSizeExceeded { limit: u64 },
    #[error("archive entry {path} ended before its declared size")]
    TruncatedEntry { path: String },
    #[error("archive must contain manifest.json")]
    MissingManifest,
    #[error("manifest.json must be the final archive entry")]
    EntryAfterManifest,
    #[error("archive has non-canonical trailing data")]
    TrailingData,
    #[error("safe archive activation is not supported on this platform")]
    UnsupportedPlatform,
    #[error("activation source or destination name is unsafe")]
    UnsafeActivationName,
    #[error("activation destination already exists")]
    DestinationExists,
    #[error("activation crossed a filesystem boundary")]
    CrossFilesystemActivation,
    #[error("activation path is not a regular file")]
    UnsafeActivationFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub max_compressed_bytes: u64,
    pub max_entries: u64,
    pub max_entry_bytes: u64,
    pub max_expanded_bytes: u64,
}

impl ArchiveLimits {
    pub fn new(
        max_compressed_bytes: u64,
        max_entries: u64,
        max_entry_bytes: u64,
        max_expanded_bytes: u64,
    ) -> Result<Self, ArchiveError> {
        if [
            max_compressed_bytes,
            max_entries,
            max_entry_bytes,
            max_expanded_bytes,
        ]
        .contains(&0)
            || max_expanded_bytes < TAR_TRAILER_BYTES
        {
            return Err(ArchiveError::InvalidLimits);
        }
        Ok(Self {
            max_compressed_bytes,
            max_entries,
            max_entry_bytes,
            max_expanded_bytes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveSummary {
    pub entries: u64,
    pub expanded_bytes: u64,
}

#[derive(Debug)]
struct ArchiveBudget {
    limits: ArchiveLimits,
    entries: u64,
    expanded_bytes: u64,
    paths: HashSet<String>,
}

impl ArchiveBudget {
    fn new(limits: ArchiveLimits) -> Self {
        Self {
            limits,
            entries: 0,
            expanded_bytes: TAR_TRAILER_BYTES,
            paths: HashSet::new(),
        }
    }

    fn admit(&mut self, path: &str, size: u64) -> Result<(), ArchiveError> {
        if self.paths.contains(path) {
            return Err(ArchiveError::DuplicateEntry(path.to_owned()));
        }
        if size > self.limits.max_entry_bytes {
            return Err(ArchiveError::EntrySizeExceeded {
                path: path.to_owned(),
                size,
                limit: self.limits.max_entry_bytes,
            });
        }
        let entries = self
            .entries
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_entries)
            .ok_or(ArchiveError::EntryCountExceeded {
                limit: self.limits.max_entries,
            })?;
        let padded_size = size.checked_add(511).map(|bytes| bytes / 512 * 512).ok_or(
            ArchiveError::ExpandedSizeExceeded {
                limit: self.limits.max_expanded_bytes,
            },
        )?;
        let expanded_bytes = self
            .expanded_bytes
            .checked_add(512)
            .and_then(|bytes| bytes.checked_add(padded_size))
            .filter(|bytes| *bytes <= self.limits.max_expanded_bytes)
            .ok_or(ArchiveError::ExpandedSizeExceeded {
                limit: self.limits.max_expanded_bytes,
            })?;

        self.paths.insert(path.to_owned());
        self.entries = entries;
        self.expanded_bytes = expanded_bytes;
        Ok(())
    }

    fn summary(&self) -> ArchiveSummary {
        ArchiveSummary {
            entries: self.entries,
            expanded_bytes: self.expanded_bytes,
        }
    }
}

/// Validates the lexical path and tar type before any entry bytes are read.
///
/// Version 1 uses only lowercase ASCII USTAR names. Directory entries are not
/// needed because extraction creates profile-owned directories explicitly.
pub fn validate_entry(path: &[u8], entry_type: tar::EntryType) -> Result<&str, ArchiveError> {
    if entry_type != tar::EntryType::Regular {
        return Err(ArchiveError::UnsupportedEntryType(entry_type.as_byte()));
    }
    let path = std::str::from_utf8(path)
        .map_err(|_| ArchiveError::InvalidPath("<non-UTF-8>".to_owned()))?;
    validate_canonical_path(path)?;
    Ok(path)
}

fn validate_header(header: &tar::Header) -> Result<(), ArchiveError> {
    if header.as_ustar().is_none() {
        return Err(ArchiveError::NonUstarHeader);
    }
    if header.mode()? != 0o600 || header.uid()? != 0 || header.gid()? != 0 || header.mtime()? != 0 {
        return Err(ArchiveError::NonCanonicalHeader);
    }
    Ok(())
}

fn validate_canonical_path(path: &str) -> Result<(), ArchiveError> {
    let valid = !path.is_empty()
        && path.len() <= USTAR_PATH_BYTES
        && !path.starts_with('/')
        && !path.ends_with('/')
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && component.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
        });
    if valid {
        Ok(())
    } else {
        Err(ArchiveError::InvalidPath(path.to_owned()))
    }
}

/// Caps each call into the wrapped reader at 256 KiB.
pub struct BoundedReader<R> {
    inner: R,
}

impl<R> BoundedReader<R> {
    pub const fn new(inner: R) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let length = buf.len().min(MAX_IO_CHUNK_BYTES);
        self.inner.read(&mut buf[..length])
    }
}

/// A scoped, bounded view of one validated archive entry.
///
/// The parser retains ownership of the underlying tar entry. After a visitor
/// returns successfully, the parser drains this same reader to the declared
/// entry size before it advances to the next header.
pub(crate) struct ArchiveEntryReader<'entry> {
    inner: BoundedReader<&'entry mut dyn Read>,
    bytes_read: u64,
}

impl<'entry> ArchiveEntryReader<'entry> {
    fn new(inner: &'entry mut dyn Read) -> Self {
        Self {
            inner: BoundedReader::new(inner),
            bytes_read: 0,
        }
    }

    fn bytes_read(&self) -> u64 {
        self.bytes_read
    }
}

impl Read for ArchiveEntryReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        self.bytes_read = self
            .bytes_read
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("archive entry byte count overflow"))?;
        Ok(read)
    }
}

/// Caps each call into the wrapped writer at 256 KiB.
pub struct BoundedWriter<W> {
    inner: W,
}

impl<W> BoundedWriter<W> {
    pub const fn new(inner: W) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(&buf[..buf.len().min(MAX_IO_CHUNK_BYTES)])
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct LimitedWriter<W> {
    inner: W,
    remaining: u64,
    limit_hit: Rc<Cell<bool>>,
}

impl<W> LimitedWriter<W> {
    fn new(inner: W, limit: u64) -> (Self, Rc<Cell<bool>>) {
        let limit_hit = Rc::new(Cell::new(false));
        (
            Self {
                inner,
                remaining: limit,
                limit_hit: Rc::clone(&limit_hit),
            },
            limit_hit,
        )
    }

    fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            self.limit_hit.set(true);
            return Err(io::Error::other("compressed archive limit exceeded"));
        }
        let length = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let written = self.inner.write(&buf[..length])?;
        self.remaining -= written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct LimitedReader<R> {
    inner: R,
    remaining: u64,
    limit: u64,
    limit_hit: Rc<Cell<bool>>,
}

impl<R> LimitedReader<R> {
    fn new(inner: R, limit: u64) -> (Self, Rc<Cell<bool>>) {
        let limit_hit = Rc::new(Cell::new(false));
        (
            Self {
                inner,
                remaining: limit,
                limit,
                limit_hit: Rc::clone(&limit_hit),
            },
            limit_hit,
        )
    }
}

impl<R: Read> Read for LimitedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => {
                    self.limit_hit.set(true);
                    Err(io::Error::other(format!(
                        "compressed archive exceeds {} bytes",
                        self.limit
                    )))
                }
            };
        }
        let length = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let read = self.inner.read(&mut buf[..length])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

fn map_archive_io(error: io::Error) -> ArchiveError {
    ArchiveError::Io(error)
}

fn map_validation_io(
    error: io::Error,
    limit_hit: &Cell<bool>,
    compressed_limit: u64,
) -> ArchiveError {
    if limit_hit.get() {
        ArchiveError::CompressedSizeExceeded {
            limit: compressed_limit,
        }
    } else {
        ArchiveError::Io(error)
    }
}

/// Deterministic USTAR + zstd writer for a caller-defined ordered entry list.
pub struct ArchiveWriter<W: Write> {
    builder: tar::Builder<zstd::stream::write::Encoder<'static, BoundedWriter<LimitedWriter<W>>>>,
    budget: ArchiveBudget,
    manifest_written: bool,
    compressed_limit: u64,
    compressed_limit_hit: Rc<Cell<bool>>,
}

impl<W: Write> ArchiveWriter<W> {
    pub fn new(sink: W, limits: ArchiveLimits) -> Result<Self, ArchiveError> {
        let (sink, compressed_limit_hit) = LimitedWriter::new(sink, limits.max_compressed_bytes);
        let mut encoder =
            zstd::stream::write::Encoder::new(BoundedWriter::new(sink), ZSTD_COMPRESSION_LEVEL)?;
        encoder.window_log(ZSTD_WINDOW_LOG)?;
        encoder.include_checksum(true)?;
        let mut builder = tar::Builder::new(encoder);
        builder.mode(tar::HeaderMode::Deterministic);
        Ok(Self {
            builder,
            budget: ArchiveBudget::new(limits),
            manifest_written: false,
            compressed_limit: limits.max_compressed_bytes,
            compressed_limit_hit,
        })
    }

    pub fn append<R: Read>(
        &mut self,
        path: &str,
        size: u64,
        reader: R,
    ) -> Result<(), ArchiveError> {
        if self.manifest_written {
            return Err(ArchiveError::EntryAfterManifest);
        }
        let path = validate_entry(path.as_bytes(), tar::EntryType::Regular)?;
        self.budget.admit(path, size)?;

        let mut header = tar::Header::new_ustar();
        header.set_path(path)?;
        header.set_size(size);
        header.set_mode(0o600);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        self.builder
            .append(&header, BoundedReader::new(reader))
            .map_err(|error| {
                map_validation_io(error, &self.compressed_limit_hit, self.compressed_limit)
            })?;
        self.manifest_written = path == "manifest.json";
        Ok(())
    }

    pub fn finish(mut self) -> Result<W, ArchiveError> {
        if !self.manifest_written {
            return Err(ArchiveError::MissingManifest);
        }
        let limit_hit = Rc::clone(&self.compressed_limit_hit);
        let compressed_limit = self.compressed_limit;
        let map_io = |error| map_validation_io(error, &limit_hit, compressed_limit);
        self.builder.finish().map_err(&map_io)?;
        let encoder = self.builder.into_inner().map_err(&map_io)?;
        let mut sink = encoder.finish().map_err(&map_io)?;
        sink.flush().map_err(map_archive_io)?;
        Ok(sink.into_inner().into_inner())
    }
}

/// Streams validated archive entries through one scoped visitor.
///
/// This is the sole strict parser for `.fasti` archives. The visitor cannot
/// retain the entry reader or access the underlying tar entry. A successful
/// partial read is drained and counted here before parsing continues.
pub(crate) fn visit_archive_entries<R, F, E>(
    source: R,
    limits: ArchiveLimits,
    mut visitor: F,
) -> Result<ArchiveSummary, E>
where
    R: Read,
    F: for<'entry> FnMut(&str, u64, &mut ArchiveEntryReader<'entry>) -> Result<(), E>,
    E: From<ArchiveError>,
{
    let (source, limit_hit) = LimitedReader::new(source, limits.max_compressed_bytes);
    let source = BoundedReader::new(source);
    let map_io = |error| {
        E::from(map_validation_io(
            error,
            &limit_hit,
            limits.max_compressed_bytes,
        ))
    };
    let mut decoder = zstd::stream::read::Decoder::new(source).map_err(&map_io)?;
    decoder.window_log_max(ZSTD_WINDOW_LOG).map_err(&map_io)?;
    let mut archive = tar::Archive::new(BoundedReader::new(decoder));
    let mut budget = ArchiveBudget::new(limits);
    let mut manifest_seen = false;

    {
        let entries = archive.entries().map_err(&map_io)?.raw(true);
        for result in entries {
            if manifest_seen {
                return Err(ArchiveError::EntryAfterManifest.into());
            }
            let mut entry = result.map_err(&map_io)?;
            validate_header(entry.header()).map_err(E::from)?;
            let path_bytes = entry.path_bytes();
            let path = validate_entry(path_bytes.as_ref(), entry.header().entry_type())
                .map_err(E::from)?
                .to_owned();
            let size = entry.header().size().map_err(&map_io)?;
            budget.admit(&path, size).map_err(E::from)?;
            let mut reader = ArchiveEntryReader::new(&mut entry);
            let visit_result = visitor(&path, size, &mut reader);
            if limit_hit.get() {
                return Err(ArchiveError::CompressedSizeExceeded {
                    limit: limits.max_compressed_bytes,
                }
                .into());
            }
            visit_result?;
            io::copy(&mut reader, &mut io::sink()).map_err(&map_io)?;
            if limit_hit.get() {
                return Err(ArchiveError::CompressedSizeExceeded {
                    limit: limits.max_compressed_bytes,
                }
                .into());
            }
            if reader.bytes_read() != size {
                return Err(ArchiveError::TruncatedEntry { path }.into());
            }
            manifest_seen = path == "manifest.json";
        }
    }

    if !manifest_seen {
        return Err(ArchiveError::MissingManifest.into());
    }

    // The canonical builder writes exactly two zero TAR records. The iterator
    // consumes at least the first. Drain the decoder so checksum and later
    // frames cannot hide data after the end marker.
    let mut decoder = archive.into_inner();
    let mut trailer = [0_u8; MAX_IO_CHUNK_BYTES];
    let mut trailer_bytes = 0_u64;
    loop {
        let read = decoder.read(&mut trailer).map_err(&map_io)?;
        if read == 0 {
            break;
        }
        trailer_bytes = trailer_bytes
            .checked_add(read as u64)
            .ok_or_else(|| E::from(ArchiveError::TrailingData))?;
        if trailer_bytes > TAR_REMAINING_ZERO_BYTES || trailer[..read].iter().any(|byte| *byte != 0)
        {
            return Err(ArchiveError::TrailingData.into());
        }
    }
    Ok(budget.summary())
}

/// Parses and drains an archive without extracting any entry.
pub fn validate_archive<R: Read>(
    source: R,
    limits: ArchiveLimits,
) -> Result<ArchiveSummary, ArchiveError> {
    visit_archive_entries(source, limits, |_path, _size, _reader| {
        Ok::<(), ArchiveError>(())
    })
}

#[cfg(target_os = "linux")]
fn validate_activation_name(name: &str) -> Result<(), ArchiveError> {
    if name.contains('/') || name.contains('\\') {
        return Err(ArchiveError::UnsafeActivationName);
    }
    validate_canonical_path(name).map_err(|_| ArchiveError::UnsafeActivationName)
}

/// Linux same-filesystem destination for one atomically published `.fasti` file.
///
/// The partial archive is an unnamed `O_TMPFILE` inode. A crash before
/// publication therefore leaves no pathname to sweep; publication links the
/// verified inode to the final name without replacing an existing artifact.
pub struct FilesystemArchiveDestination {
    parent: File,
    file: Option<File>,
    final_name: OsString,
    admitted_bytes: Cell<Option<u64>>,
    written_bytes: u64,
    hasher: Sha256,
}

impl FilesystemArchiveDestination {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, ArchiveError> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            return Err(ArchiveError::UnsupportedPlatform);
        }
        #[cfg(target_os = "linux")]
        {
            let path = path.as_ref();
            let final_name = path
                .file_name()
                .ok_or(ArchiveError::UnsafeActivationName)?
                .to_owned();
            let parent_path = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            let parent = File::from(
                rustix::fs::open(
                    parent_path,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::CLOEXEC
                        | rustix::fs::OFlags::NOFOLLOW,
                    rustix::fs::Mode::empty(),
                )
                .map_err(errno_to_archive_error)?,
            );
            match rustix::fs::statat(
                &parent,
                final_name.as_os_str(),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Ok(_) => return Err(ArchiveError::DestinationExists),
                Err(rustix::io::Errno::NOENT) => {}
                Err(error) => return Err(errno_to_archive_error(error)),
            }
            let file = File::from(
                rustix::fs::openat(
                    &parent,
                    ".",
                    rustix::fs::OFlags::RDWR
                        | rustix::fs::OFlags::TMPFILE
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                )
                .map_err(errno_to_archive_error)?,
            );
            destination_crash_test_point("created");
            Ok(Self {
                parent,
                file: Some(file),
                final_name,
                admitted_bytes: Cell::new(None),
                written_bytes: 0,
                hasher: Sha256::new(),
            })
        }
    }

    fn file(&mut self) -> io::Result<&mut File> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("archive destination is closed"))
    }
}

impl Write for FilesystemArchiveDestination {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let admitted = self
            .admitted_bytes
            .get()
            .ok_or_else(|| io::Error::other("archive destination was not preflighted"))?;
        let remaining = admitted
            .checked_sub(self.written_bytes)
            .ok_or_else(|| io::Error::other("archive destination capacity was exceeded"))?;
        if remaining == 0 {
            return Err(io::Error::other(
                "archive destination capacity was exceeded",
            ));
        }
        let limit = bytes
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let written = self.file()?.write(&bytes[..limit])?;
        self.hasher.update(&bytes[..written]);
        self.written_bytes = self
            .written_bytes
            .checked_add(written as u64)
            .ok_or_else(|| io::Error::other("archive destination byte count overflow"))?;
        destination_crash_test_point("written");
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file()?.flush()?;
        destination_crash_test_point("flushed");
        Ok(())
    }
}

impl WorkspaceArchiveDestination for FilesystemArchiveDestination {
    fn preflight(&self, required_bytes: u64) -> io::Result<()> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = required_bytes;
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "archive destination is unsupported on this platform",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            if self.admitted_bytes.get().is_some() {
                return Err(io::Error::other(
                    "archive destination was already preflighted",
                ));
            }
            let stats = rustix::fs::fstatvfs(&self.parent).map_err(errno_to_io_error)?;
            let available = stats
                .f_bavail
                .checked_mul(stats.f_frsize)
                .ok_or_else(|| io::Error::other("destination capacity overflow"))?;
            if available < required_bytes {
                return Err(io::Error::other("destination capacity is insufficient"));
            }
            self.admitted_bytes.set(Some(required_bytes));
            destination_crash_test_point("preflighted");
            Ok(())
        }
    }

    fn complete(
        mut self: Box<Self>,
        archive_digest: &Sha256Digest,
        _manifest_digest: &Sha256Digest,
    ) -> Result<(), WorkspaceArchiveCompletionError> {
        self.flush()?;
        let file = self
            .file
            .as_ref()
            .ok_or_else(|| io::Error::other("archive destination is closed"))?;
        sync_open_handle(file).map_err(archive_error_to_io)?;
        destination_crash_test_point("file_synced");
        let actual_digest = format!("sha256:{:x}", self.hasher.clone().finalize());
        if actual_digest != archive_digest.as_str() {
            return Err(io::Error::other("archive destination digest mismatch").into());
        }
        #[cfg(not(target_os = "linux"))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "archive destination is unsupported on this platform",
        ));
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd as _;
            let source = format!("/proc/self/fd/{}", file.as_raw_fd());
            rustix::fs::linkat(
                rustix::fs::CWD,
                source,
                &self.parent,
                self.final_name.as_os_str(),
                rustix::fs::AtFlags::SYMLINK_FOLLOW,
            )
            .map_err(errno_to_io_error)
            .map_err(WorkspaceArchiveCompletionError::Discarded)?;
            destination_crash_test_point("linked");
            if let Err(publication) = sync_destination_parent(&self.parent) {
                self.file.take();
                return Err(
                    WorkspaceArchiveCompletionError::PublishedDurabilityIndeterminate(publication),
                );
            }
            destination_crash_test_point("directory_synced");
            self.file.take();
            Ok(())
        }
    }

    fn abort(mut self: Box<Self>) -> io::Result<()> {
        destination_crash_test_point("abort_started");
        self.file.take();
        destination_crash_test_point("abort_closed");
        Ok(())
    }
}

fn archive_error_to_io(error: ArchiveError) -> io::Error {
    match error {
        ArchiveError::Io(error) => error,
        ArchiveError::DestinationExists => {
            io::Error::new(io::ErrorKind::AlreadyExists, "archive destination exists")
        }
        error => io::Error::other(error),
    }
}

#[cfg(target_os = "linux")]
fn errno_to_io_error(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}

#[cfg(target_os = "linux")]
fn sync_destination_parent(parent: &File) -> io::Result<()> {
    #[cfg(test)]
    {
        static FAILURE_INJECTED: AtomicBool = AtomicBool::new(false);
        if std::env::var("FASTI_TEST_DESTINATION_PARENT_SYNC_FAILURE").as_deref() == Ok("1")
            && !FAILURE_INJECTED.swap(true, Ordering::SeqCst)
        {
            return Err(io::Error::other("injected destination parent sync failure"));
        }
    }
    sync_open_handle(parent).map_err(archive_error_to_io)
}

#[cfg(all(test, target_os = "linux"))]
fn destination_crash_test_point(operation: &str) {
    let expected = format!("destination.{operation}");
    if std::env::var("FASTI_TEST_DESTINATION_CRASH_POINT").as_deref() == Ok(expected.as_str()) {
        rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::KILL)
            .expect("send SIGKILL to destination crash worker");
    }
}

#[cfg(not(all(test, target_os = "linux")))]
#[inline(always)]
fn destination_crash_test_point(_operation: &str) {}

/// Opens or creates the owner-only staging parent, then creates one fresh
/// owner-only attempt directory beneath it.
///
/// Both names are validated before either directory can be created. The
/// returned handles remain anchored if the path used to open the data root is
/// later renamed or replaced. The attempt directory is always create-new;
/// existing restore state is never reused implicitly.
#[cfg(target_os = "linux")]
pub fn create_staging_attempt(
    root: &File,
    staging_name: &str,
    attempt_name: &str,
) -> Result<(File, File), ArchiveError> {
    validate_activation_name(staging_name)?;
    validate_activation_name(attempt_name)?;

    match rustix::fs::mkdirat(root, staging_name, rustix::fs::Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(error) => return Err(errno_to_archive_error(error)),
    }
    let staging = open_private_directory(root, staging_name)?;

    rustix::fs::mkdirat(
        &staging,
        attempt_name,
        rustix::fs::Mode::from_raw_mode(0o700),
    )
    .map_err(errno_to_archive_error)?;
    let attempt = open_private_directory(&staging, attempt_name)?;
    Ok((staging, attempt))
}

#[cfg(target_os = "linux")]
pub(crate) fn open_private_directory(parent: &File, name: &str) -> Result<File, ArchiveError> {
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
    .map_err(errno_to_archive_error)?;
    let directory = File::from(fd);
    rustix::fs::fchmod(&directory, rustix::fs::Mode::from_raw_mode(0o700))
        .map_err(errno_to_archive_error)?;
    Ok(directory)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn open_private_directory(_parent: &File, _name: &str) -> Result<File, ArchiveError> {
    Err(ArchiveError::UnsupportedPlatform)
}

/// Opens or creates one owner-only child directory beneath an anchored parent.
///
/// The name is one validated path component. Existing directories are accepted
/// only so a bounded importer can reuse a digest-prefix directory that it
/// created earlier in the same fresh staging attempt.
#[cfg(target_os = "linux")]
pub(crate) fn open_or_create_private_directory(
    parent: &File,
    name: &str,
) -> Result<File, ArchiveError> {
    validate_activation_name(name)?;
    match rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(error) => return Err(errno_to_archive_error(error)),
    }
    open_private_directory(parent, name)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn open_or_create_private_directory(
    _parent: &File,
    _name: &str,
) -> Result<File, ArchiveError> {
    Err(ArchiveError::UnsupportedPlatform)
}
/// Creates a new owner-only regular file beneath an already-open data root.
#[cfg(target_os = "linux")]
pub fn open_new_file_beneath(root: &File, relative: &Path) -> Result<File, ArchiveError> {
    let relative = relative
        .to_str()
        .ok_or(ArchiveError::UnsafeActivationName)?;
    validate_canonical_path(relative).map_err(|_| ArchiveError::UnsafeActivationName)?;
    let fd = rustix::fs::openat2(
        root,
        relative,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::from_raw_mode(0o600),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_MAGICLINKS
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_XDEV,
    )
    .map_err(errno_to_archive_error)?;
    Ok(File::from(fd))
}

/// Opens one existing regular file beneath an already-open private directory.
#[cfg(target_os = "linux")]
#[allow(dead_code)] // consumed by the private restore activation coordinator
pub(crate) fn open_existing_file_beneath(
    root: &File,
    relative: &Path,
) -> Result<File, ArchiveError> {
    let relative = relative
        .to_str()
        .ok_or(ArchiveError::UnsafeActivationName)?;
    validate_canonical_path(relative).map_err(|_| ArchiveError::UnsafeActivationName)?;
    let fd = rustix::fs::openat2(
        root,
        relative,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
        rustix::fs::Mode::empty(),
        rustix::fs::ResolveFlags::BENEATH
            | rustix::fs::ResolveFlags::NO_MAGICLINKS
            | rustix::fs::ResolveFlags::NO_SYMLINKS
            | rustix::fs::ResolveFlags::NO_XDEV,
    )
    .map_err(errno_to_archive_error)?;
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(ArchiveError::UnsafeActivationFile);
    }
    Ok(file)
}

/// Atomically moves one child between opened parents without replacing a live
/// destination.
#[cfg(target_os = "linux")]
pub fn activate_no_replace(
    source_parent: &File,
    source_name: &str,
    destination_parent: &File,
    destination_name: &str,
) -> Result<(), ArchiveError> {
    validate_activation_name(source_name)?;
    validate_activation_name(destination_name)?;
    rustix::fs::renameat_with(
        source_parent,
        source_name,
        destination_parent,
        destination_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(errno_to_archive_error)
}

/// Flushes one already-open regular file or directory without resolving a
/// pathname again.
#[cfg(target_os = "linux")]
pub fn sync_open_handle(handle: &File) -> Result<(), ArchiveError> {
    rustix::fs::fsync(handle).map_err(errno_to_archive_error)
}

#[cfg(target_os = "linux")]
fn errno_to_archive_error(error: rustix::io::Errno) -> ArchiveError {
    if error == rustix::io::Errno::EXIST {
        ArchiveError::DestinationExists
    } else if error == rustix::io::Errno::XDEV {
        ArchiveError::CrossFilesystemActivation
    } else {
        ArchiveError::Io(io::Error::from_raw_os_error(error.raw_os_error()))
    }
}

#[cfg(not(target_os = "linux"))]
pub fn open_new_file_beneath(_root: &File, _relative: &Path) -> Result<File, ArchiveError> {
    Err(ArchiveError::UnsupportedPlatform)
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)] // consumed by the private restore activation coordinator
pub(crate) fn open_existing_file_beneath(
    _root: &File,
    _relative: &Path,
) -> Result<File, ArchiveError> {
    Err(ArchiveError::UnsupportedPlatform)
}

#[cfg(not(target_os = "linux"))]
pub fn create_staging_attempt(
    _root: &File,
    _staging_name: &str,
    _attempt_name: &str,
) -> Result<(File, File), ArchiveError> {
    Err(ArchiveError::UnsupportedPlatform)
}

#[cfg(not(target_os = "linux"))]
pub fn activate_no_replace(
    _source_parent: &File,
    _source_name: &str,
    _destination_parent: &File,
    _destination_name: &str,
) -> Result<(), ArchiveError> {
    Err(ArchiveError::UnsupportedPlatform)
}

#[cfg(not(target_os = "linux"))]
pub fn sync_open_handle(_handle: &File) -> Result<(), ArchiveError> {
    Err(ArchiveError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io::Cursor;
    #[cfg(target_os = "linux")]
    use std::os::unix::process::ExitStatusExt as _;
    #[cfg(target_os = "linux")]
    use std::process::Command;
    use std::rc::Rc;

    #[cfg(target_os = "linux")]
    const DESTINATION_CRASH_POINT_ENV: &str = "FASTI_TEST_DESTINATION_CRASH_POINT";
    #[cfg(target_os = "linux")]
    const DESTINATION_CRASH_ROOT_ENV: &str = "FASTI_TEST_DESTINATION_CRASH_ROOT";
    #[cfg(target_os = "linux")]
    const DESTINATION_PARENT_SYNC_FAILURE_ENV: &str = "FASTI_TEST_DESTINATION_PARENT_SYNC_FAILURE";
    #[cfg(target_os = "linux")]
    const DESTINATION_BYTES: &[u8] = b"complete deterministic archive bytes";

    #[cfg(target_os = "linux")]
    fn destination_digest(bytes: &[u8]) -> Sha256Digest {
        Sha256Digest::parse(format!("sha256:{:x}", Sha256::digest(bytes)))
            .expect("destination digest")
    }

    #[cfg(target_os = "linux")]
    fn publish_destination(path: &Path) {
        let mut destination = FilesystemArchiveDestination::new(path).expect("destination");
        destination.preflight(1024).expect("capacity preflight");
        destination
            .write_all(DESTINATION_BYTES)
            .expect("write destination");
        let digest = destination_digest(DESTINATION_BYTES);
        Box::new(destination)
            .complete(&digest, &digest)
            .expect("publish destination");
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "subprocess worker invoked by filesystem_destination_sigkill_matrix"]
    fn filesystem_destination_crash_worker() {
        let Ok(root) = std::env::var(DESTINATION_CRASH_ROOT_ENV) else {
            return;
        };
        let point = std::env::var(DESTINATION_CRASH_POINT_ENV).unwrap_or_default();
        let path = Path::new(&root).join("workspace.fasti");
        let mut destination = FilesystemArchiveDestination::new(path).expect("destination");
        destination.preflight(1024).expect("capacity preflight");
        destination
            .write_all(DESTINATION_BYTES)
            .expect("write destination");
        destination.flush().expect("flush destination");
        if point.contains("abort_") {
            Box::new(destination).abort().expect("abort destination");
        } else {
            let digest = destination_digest(DESTINATION_BYTES);
            Box::new(destination)
                .complete(&digest, &digest)
                .expect("complete destination");
        }
        panic!("configured destination crash point was not reached");
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "subprocess worker invoked by filesystem_destination_reports_indeterminate_parent_sync"]
    fn filesystem_destination_parent_sync_failure_worker() {
        let Ok(root) = std::env::var(DESTINATION_CRASH_ROOT_ENV) else {
            return;
        };
        let path = Path::new(&root).join("workspace.fasti");
        let mut destination = FilesystemArchiveDestination::new(&path).expect("destination");
        destination.preflight(1024).expect("capacity preflight");
        destination
            .write_all(DESTINATION_BYTES)
            .expect("write destination");
        let digest = destination_digest(DESTINATION_BYTES);
        let error = Box::new(destination)
            .complete(&digest, &digest)
            .expect_err("directory sync failure must not return success");
        assert!(matches!(
            error,
            WorkspaceArchiveCompletionError::PublishedDurabilityIndeterminate(_)
        ));
        assert_eq!(
            std::fs::read(path).expect("linked complete archive"),
            DESTINATION_BYTES
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn filesystem_destination_is_bounded_digest_bound_and_no_replace() {
        let root = tempfile::tempdir().expect("destination root");
        let path = root.path().join("Backup 2026.fasti");

        let mut bounded = FilesystemArchiveDestination::new(&path).expect("bounded destination");
        bounded.preflight(1).expect("bounded preflight");
        assert!(bounded.preflight(1024).is_err());
        assert!(bounded.write_all(DESTINATION_BYTES).is_err());
        Box::new(bounded)
            .abort()
            .expect("abort bounded destination");
        assert!(!path.exists());

        let mut mismatched =
            FilesystemArchiveDestination::new(&path).expect("mismatched destination");
        mismatched.preflight(1024).expect("mismatch preflight");
        mismatched
            .write_all(DESTINATION_BYTES)
            .expect("write mismatched destination");
        let wrong = destination_digest(b"wrong bytes");
        assert!(Box::new(mismatched).complete(&wrong, &wrong).is_err());
        assert!(!path.exists());

        publish_destination(&path);
        assert_eq!(
            std::fs::read(&path).expect("published bytes"),
            DESTINATION_BYTES
        );
        assert!(matches!(
            FilesystemArchiveDestination::new(&path),
            Err(ArchiveError::DestinationExists)
        ));

        let race_path = root.path().join("race.fasti");
        let mut duplicate =
            FilesystemArchiveDestination::new(&race_path).expect("duplicate destination");
        duplicate.preflight(1024).expect("duplicate preflight");
        duplicate
            .write_all(b"replacement")
            .expect("write duplicate destination");
        std::fs::write(&race_path, DESTINATION_BYTES).expect("concurrent destination");
        let digest = destination_digest(b"replacement");
        let error = Box::new(duplicate)
            .complete(&digest, &digest)
            .expect_err("publication must not replace");
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(&race_path).expect("retained bytes"),
            DESTINATION_BYTES
        );

        let symlink_path = root.path().join("symlink.fasti");
        std::os::unix::fs::symlink(&path, &symlink_path).expect("destination symlink");
        assert!(matches!(
            FilesystemArchiveDestination::new(&symlink_path),
            Err(ArchiveError::DestinationExists)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn filesystem_destination_reports_indeterminate_parent_sync() {
        let root = tempfile::tempdir().expect("sync failure destination root");
        // nosemgrep: rust.lang.security.current-exe.current-exe -- test-only re-exec worker (#[cfg(test)]), never compiled into a release binary
        let output = Command::new(std::env::current_exe().expect("current test binary"))
            .args([
                "--exact",
                "archive::tests::filesystem_destination_parent_sync_failure_worker",
                "--ignored",
                "--nocapture",
            ])
            .env(DESTINATION_CRASH_ROOT_ENV, root.path())
            .env(DESTINATION_PARENT_SYNC_FAILURE_ENV, "1")
            .output()
            .expect("run parent-sync failure worker");
        assert!(
            output.status.success(),
            "parent-sync worker failed; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let path = root.path().join("workspace.fasti");
        assert_eq!(
            std::fs::read(&path).expect("linked complete archive"),
            DESTINATION_BYTES
        );
        assert!(matches!(
            FilesystemArchiveDestination::new(&path),
            Err(ArchiveError::DestinationExists)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn filesystem_destination_sigkill_matrix() {
        for (point, published) in [
            ("destination.created", false),
            ("destination.preflighted", false),
            ("destination.written", false),
            ("destination.flushed", false),
            ("destination.file_synced", false),
            ("destination.linked", true),
            ("destination.directory_synced", true),
            ("destination.abort_started", false),
            ("destination.abort_closed", false),
        ] {
            let root = tempfile::tempdir().expect("crash destination root");
            let path = root.path().join("workspace.fasti");
            // nosemgrep: rust.lang.security.current-exe.current-exe -- test-only re-exec worker (#[cfg(test)]), never compiled into a release binary
            let output = Command::new(std::env::current_exe().expect("current test binary"))
                .args([
                    "--exact",
                    "archive::tests::filesystem_destination_crash_worker",
                    "--ignored",
                    "--nocapture",
                ])
                .env(DESTINATION_CRASH_POINT_ENV, point)
                .env(DESTINATION_CRASH_ROOT_ENV, root.path())
                .output()
                .expect("run destination crash worker");
            assert_eq!(
                output.status.signal(),
                Some(9),
                "{point} did not terminate with SIGKILL; status={:?}; stdout={}; stderr={}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            if published {
                assert_eq!(
                    std::fs::read(&path).expect("crash-published archive"),
                    DESTINATION_BYTES,
                    "{point} published alternate bytes"
                );
            } else {
                assert!(!path.exists(), "{point} exposed an incomplete archive");
                assert_eq!(
                    std::fs::read_dir(root.path())
                        .expect("destination directory")
                        .count(),
                    0,
                    "{point} leaked a named partial"
                );
                publish_destination(&path);
            }
        }
    }

    fn limits() -> ArchiveLimits {
        ArchiveLimits::new(16 * 1024 * 1024, 32, 8 * 1024 * 1024, 16 * 1024 * 1024)
            .expect("valid test limits")
    }

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = ArchiveWriter::new(Vec::new(), limits()).expect("archive writer");
        for (path, bytes) in entries {
            writer
                .append(path, bytes.len() as u64, Cursor::new(*bytes))
                .expect("append entry");
        }
        writer.finish().expect("finish archive")
    }

    #[test]
    fn canonical_paths_are_table_driven() {
        for accepted in [
            "manifest.json",
            "observations.ndjson",
            "attachments/sha256/ab/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
        ] {
            assert!(validate_entry(accepted.as_bytes(), tar::EntryType::Regular).is_ok());
        }
        for rejected in [
            "",
            "/manifest.json",
            "../manifest.json",
            "streams/../manifest.json",
            "streams//observations.ndjson",
            "streams\\observations.ndjson",
            "Manifest.json",
            "méta.json",
            "manifest.json/",
        ] {
            assert!(
                matches!(
                    validate_entry(rejected.as_bytes(), tar::EntryType::Regular),
                    Err(ArchiveError::InvalidPath(_))
                ),
                "accepted hostile path {rejected:?}"
            );
        }
    }

    #[test]
    fn non_regular_tar_types_are_rejected() {
        for entry_type in [
            tar::EntryType::Directory,
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::Fifo,
        ] {
            assert!(matches!(
                validate_entry(b"manifest.json", entry_type),
                Err(ArchiveError::UnsupportedEntryType(_))
            ));
        }
    }

    #[test]
    fn non_ustar_and_variable_metadata_are_rejected() {
        let mut old = tar::Header::new_old();
        old.set_path("manifest.json").expect("old path");
        old.set_size(2);
        old.set_mode(0o600);
        old.set_entry_type(tar::EntryType::Regular);
        old.set_cksum();
        assert!(matches!(
            validate_header(&old),
            Err(ArchiveError::NonUstarHeader)
        ));

        let mut variable = tar::Header::new_ustar();
        variable.set_mode(0o644);
        assert!(matches!(
            validate_header(&variable),
            Err(ArchiveError::NonCanonicalHeader)
        ));
    }

    #[test]
    fn budget_rejects_duplicates_and_checked_limit_overflow() {
        let limits = ArchiveLimits::new(u64::MAX, 1, u64::MAX, 4096).expect("limits");
        let mut budget = ArchiveBudget::new(limits);
        budget.admit("manifest.json", 1).expect("first entry");
        assert!(matches!(
            budget.admit("manifest.json", 0),
            Err(ArchiveError::DuplicateEntry(_))
        ));
        assert!(matches!(
            budget.admit("other.json", 0),
            Err(ArchiveError::EntryCountExceeded { .. })
        ));

        let limits = ArchiveLimits::new(u64::MAX, 2, u64::MAX, u64::MAX).expect("limits");
        let mut budget = ArchiveBudget::new(limits);
        budget.expanded_bytes = u64::MAX;
        assert!(matches!(
            budget.admit("b", 1),
            Err(ArchiveError::ExpandedSizeExceeded { .. })
        ));
    }

    #[derive(Clone)]
    struct MeasuringReader {
        remaining: usize,
        calls: Rc<RefCell<Vec<usize>>>,
    }

    impl Read for MeasuringReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.calls.borrow_mut().push(buf.len());
            let read = self.remaining.min(buf.len());
            buf[..read].fill(0);
            self.remaining -= read;
            Ok(read)
        }
    }

    struct MeasuringWriter(Rc<RefCell<Vec<usize>>>);

    impl Write for MeasuringWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().push(buf.len());
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn bounded_adapters_never_delegate_more_than_256_kib() {
        let read_calls = Rc::new(RefCell::new(Vec::new()));
        let source = MeasuringReader {
            remaining: MAX_IO_CHUNK_BYTES * 3,
            calls: Rc::clone(&read_calls),
        };
        let mut reader = BoundedReader::new(source);
        io::copy(&mut reader, &mut io::sink()).expect("bounded copy");
        assert!(read_calls
            .borrow()
            .iter()
            .all(|size| *size <= MAX_IO_CHUNK_BYTES));

        let write_calls = Rc::new(RefCell::new(Vec::new()));
        let mut writer = BoundedWriter::new(MeasuringWriter(Rc::clone(&write_calls)));
        writer
            .write_all(&vec![0; MAX_IO_CHUNK_BYTES * 3])
            .expect("bounded write");
        assert!(write_calls
            .borrow()
            .iter()
            .all(|size| *size <= MAX_IO_CHUNK_BYTES));
    }

    #[test]
    fn deterministic_archive_bytes_and_headers_are_stable() {
        let entries = [
            ("observations.ndjson", b"{\"id\":1}\n".as_slice()),
            ("manifest.json", b"{\"format_version\":1}\n".as_slice()),
        ];
        let first = archive(&entries);
        let second = archive(&entries);
        assert_eq!(first, second);
        assert_eq!(
            validate_archive(Cursor::new(&first), limits()).expect("valid archive"),
            ArchiveSummary {
                entries: 2,
                expanded_bytes: 3072,
            }
        );

        let tar_bytes = zstd::stream::decode_all(Cursor::new(&first)).expect("decode archive");
        let mut tar = tar::Archive::new(Cursor::new(tar_bytes));
        for entry in tar.entries().expect("tar entries") {
            let entry = entry.expect("tar entry");
            let header = entry.header();
            assert_eq!(header.entry_type(), tar::EntryType::Regular);
            assert_eq!(header.mode().expect("mode"), 0o600);
            assert_eq!(header.uid().expect("uid"), 0);
            assert_eq!(header.gid().expect("gid"), 0);
            assert_eq!(header.mtime().expect("mtime"), 0);
        }

        let compressed_limit = first.len() as u64 - 1;
        let too_small = ArchiveLimits::new(
            compressed_limit,
            limits().max_entries,
            limits().max_entry_bytes,
            limits().max_expanded_bytes,
        )
        .expect("compressed limit");
        assert!(matches!(
            validate_archive(Cursor::new(first), too_small),
            Err(ArchiveError::CompressedSizeExceeded { .. })
        ));
    }

    #[test]
    fn entry_visitor_sees_exact_ordered_bytes_including_final_manifest() {
        let entries = [
            ("observations.ndjson", b"one\ntwo\n".as_slice()),
            ("receipts.ndjson", b"three\n".as_slice()),
            ("manifest.json", b"{\"format_version\":1}\n".as_slice()),
        ];
        let encoded = archive(&entries);
        let mut visited = Vec::new();

        let summary =
            visit_archive_entries(Cursor::new(encoded), limits(), |path, size, reader| {
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                visited.push((path.to_owned(), size, bytes));
                Ok::<(), ArchiveError>(())
            })
            .expect("visit canonical archive");

        assert_eq!(summary.entries, entries.len() as u64);
        assert_eq!(
            visited,
            entries
                .iter()
                .map(|(path, bytes)| (path.to_string(), bytes.len() as u64, bytes.to_vec()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            visited.last().map(|entry| entry.0.as_str()),
            Some("manifest.json")
        );
    }

    #[test]
    fn partial_visitor_reads_are_drained_and_cannot_hide_trailing_data() {
        let entries = [
            ("observations.ndjson", b"abcdef".as_slice()),
            ("manifest.json", b"{}".as_slice()),
        ];
        let encoded = archive(&entries);
        let mut prefixes = Vec::new();
        let summary =
            visit_archive_entries(Cursor::new(&encoded), limits(), |path, size, reader| {
                let mut prefix = [0_u8; 1];
                reader.read_exact(&mut prefix)?;
                prefixes.push((path.to_owned(), size, prefix[0]));
                Ok::<(), ArchiveError>(())
            })
            .expect("parser drains unread entry bytes");
        assert_eq!(summary.entries, 2);
        assert_eq!(
            prefixes,
            vec![
                ("observations.ndjson".to_owned(), 6, b'a'),
                ("manifest.json".to_owned(), 2, b'{'),
            ]
        );

        let mut tar_bytes = zstd::stream::decode_all(Cursor::new(encoded)).expect("decode tar");
        tar_bytes.extend_from_slice(&[0; 512]);
        let with_trailing_data =
            zstd::stream::encode_all(Cursor::new(tar_bytes), ZSTD_COMPRESSION_LEVEL)
                .expect("compress archive with trailing data");
        let mut visited_paths = Vec::new();
        let result = visit_archive_entries(
            Cursor::new(with_trailing_data),
            limits(),
            |path, size, reader| {
                let mut prefix = [0_u8; 1];
                if size != 0 {
                    reader.read_exact(&mut prefix)?;
                }
                visited_paths.push(path.to_owned());
                Ok(())
            },
        );
        assert!(matches!(result, Err(ArchiveError::TrailingData)));
        assert_eq!(
            visited_paths,
            vec!["observations.ndjson".to_owned(), "manifest.json".to_owned()]
        );
    }

    #[test]
    fn compressed_output_limit_is_typed_during_append_and_finish() {
        let mut state = 0x1234_5678_u32;
        let payload: Vec<u8> = (0..512 * 1024)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        let entries = [
            ("observations.ndjson", payload.as_slice()),
            ("manifest.json", b"{}".as_slice()),
        ];
        let baseline = archive(&entries);

        let append_limits = ArchiveLimits::new(
            64,
            limits().max_entries,
            limits().max_entry_bytes,
            limits().max_expanded_bytes,
        )
        .expect("append limits");
        let mut writer = ArchiveWriter::new(Vec::new(), append_limits).expect("archive writer");
        assert!(matches!(
            writer.append(
                "observations.ndjson",
                payload.len() as u64,
                Cursor::new(&payload)
            ),
            Err(ArchiveError::CompressedSizeExceeded { limit: 64 })
        ));

        let finish_limit = baseline.len() as u64 - 1;
        let finish_limits = ArchiveLimits::new(
            finish_limit,
            limits().max_entries,
            limits().max_entry_bytes,
            limits().max_expanded_bytes,
        )
        .expect("finish limits");
        let mut writer = ArchiveWriter::new(Vec::new(), finish_limits).expect("archive writer");
        for (path, bytes) in entries {
            writer
                .append(path, bytes.len() as u64, Cursor::new(bytes))
                .expect("append within pre-finish limit");
        }
        assert!(matches!(
            writer.finish(),
            Err(ArchiveError::CompressedSizeExceeded { limit }) if limit == finish_limit
        ));
    }

    fn hostile_archive(entries: &[(&str, tar::EntryType, &[u8])]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (path, entry_type, bytes) in entries {
                let mut header = tar::Header::new_ustar();
                header.set_path(path).expect("hostile test path fits USTAR");
                header.set_size(bytes.len() as u64);
                header.set_mode(0o600);
                header.set_uid(0);
                header.set_gid(0);
                header.set_mtime(0);
                header.set_entry_type(*entry_type);
                header.set_cksum();
                builder
                    .append(&header, Cursor::new(*bytes))
                    .expect("hostile test tar");
            }
            builder.finish().expect("finish hostile tar");
        }
        zstd::stream::encode_all(Cursor::new(tar_bytes), ZSTD_COMPRESSION_LEVEL)
            .expect("compress hostile tar")
    }

    #[test]
    fn hostile_links_duplicates_and_entry_limits_are_rejected() {
        let linked = hostile_archive(&[
            ("escape", tar::EntryType::Symlink, b"../outside"),
            ("manifest.json", tar::EntryType::Regular, b"{}"),
        ]);
        let linked = validate_archive(Cursor::new(linked), limits());
        assert!(
            matches!(linked, Err(ArchiveError::UnsupportedEntryType(_))),
            "unexpected link result: {linked:?}"
        );

        let duplicate = hostile_archive(&[
            ("same.ndjson", tar::EntryType::Regular, b"a"),
            ("same.ndjson", tar::EntryType::Regular, b"b"),
            ("manifest.json", tar::EntryType::Regular, b"{}"),
        ]);
        assert!(matches!(
            validate_archive(Cursor::new(duplicate), limits()),
            Err(ArchiveError::DuplicateEntry(_))
        ));

        let oversized = hostile_archive(&[
            ("large.ndjson", tar::EntryType::Regular, b"1234"),
            ("manifest.json", tar::EntryType::Regular, b"{}"),
        ]);
        let small = ArchiveLimits::new(1 << 20, 4, 3, 1 << 20).expect("small limits");
        assert!(matches!(
            validate_archive(Cursor::new(oversized), small),
            Err(ArchiveError::EntrySizeExceeded { .. })
        ));

        let canonical = archive(&[("manifest.json", b"{}")]);
        let mut tar_bytes = zstd::stream::decode_all(Cursor::new(canonical)).expect("decode tar");
        tar_bytes.extend_from_slice(&[0; 512]);
        let extra_trailer =
            zstd::stream::encode_all(Cursor::new(tar_bytes), ZSTD_COMPRESSION_LEVEL)
                .expect("compress extra trailer");
        assert!(matches!(
            validate_archive(Cursor::new(extra_trailer), limits()),
            Err(ArchiveError::TrailingData)
        ));
    }

    #[test]
    fn decoder_rejects_frames_advertising_a_larger_window() {
        let input = vec![0_u8; 8 * 1024 * 1024];
        let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), ZSTD_COMPRESSION_LEVEL)
            .expect("large-window encoder");
        encoder.window_log(23).expect("set large window");
        encoder
            .set_pledged_src_size(Some(input.len() as u64))
            .expect("pledge source size");
        encoder.write_all(&input).expect("compress input");
        let frame = encoder.finish().expect("finish frame");

        let mut decoder = zstd::stream::read::Decoder::new(Cursor::new(frame))
            .expect("profile decoder construction");
        decoder
            .window_log_max(ZSTD_WINDOW_LOG)
            .expect("set decoder window limit");
        let error = io::copy(&mut decoder, &mut io::sink())
            .expect_err("8 MiB advertised window must be rejected");
        assert_eq!(error.kind(), io::ErrorKind::Other);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_safe_open_and_no_replace_activation_fail_closed() {
        let root = tempfile::tempdir().expect("temporary root");
        std::fs::create_dir(root.path().join("files")).expect("files directory");
        let root_fd = File::open(root.path()).expect("open data root");
        let file = open_new_file_beneath(&root_fd, Path::new("files/item"))
            .expect("safe create beneath root");
        assert_eq!(file.metadata().expect("metadata").len(), 0);
        assert!(open_new_file_beneath(&root_fd, Path::new("../escape")).is_err());
        std::fs::create_dir(root.path().join("outside")).expect("outside directory");
        std::os::unix::fs::symlink("outside", root.path().join("link")).expect("hostile symlink");
        assert!(open_new_file_beneath(&root_fd, Path::new("link/escape")).is_err());
        assert!(!root.path().join("outside/escape").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nested_staging_activation_and_open_handle_fsync_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("temporary root");
        let root_fd = File::open(root.path()).expect("open data root");
        let (staging, attempt) = create_staging_attempt(&root_fd, "staging", "rst_attempt")
            .expect("create nested staging attempt");

        for directory in [&staging, &attempt] {
            assert_eq!(
                directory
                    .metadata()
                    .expect("directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }

        let mut payload =
            open_new_file_beneath(&attempt, Path::new("payload")).expect("create attempt payload");
        payload.write_all(b"durable").expect("write payload");
        sync_open_handle(&payload).expect("fsync payload");
        sync_open_handle(&attempt).expect("fsync attempt");
        sync_open_handle(&staging).expect("fsync staging parent");
        sync_open_handle(&root_fd).expect("fsync data root before activation");

        activate_no_replace(&staging, "rst_attempt", &root_fd, "current")
            .expect("activate nested attempt");
        sync_open_handle(&attempt).expect("fsync moved attempt handle");
        sync_open_handle(&root_fd).expect("fsync activated data root");

        assert_eq!(
            std::fs::read(root.path().join("current/payload")).expect("active payload"),
            b"durable"
        );
        assert!(!root.path().join("staging/rst_attempt").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nested_activation_never_reuses_an_attempt_or_replaces_current() {
        let root = tempfile::tempdir().expect("temporary root");
        let root_fd = File::open(root.path()).expect("open data root");
        let (staging, _attempt) =
            create_staging_attempt(&root_fd, "staging", "rst_first").expect("create first attempt");
        assert!(matches!(
            create_staging_attempt(&root_fd, "staging", "rst_first"),
            Err(ArchiveError::DestinationExists)
        ));

        activate_no_replace(&staging, "rst_first", &root_fd, "current")
            .expect("activate first attempt");
        let (staging, _next) =
            create_staging_attempt(&root_fd, "staging", "rst_next").expect("create next attempt");
        assert!(matches!(
            activate_no_replace(&staging, "rst_next", &root_fd, "current"),
            Err(ArchiveError::DestinationExists)
        ));
        assert!(root.path().join("current").is_dir());
        assert!(root.path().join("staging/rst_next").is_dir());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn staging_components_and_symlink_parents_fail_before_escape() {
        let root = tempfile::tempdir().expect("temporary root");
        let root_fd = File::open(root.path()).expect("open data root");
        assert!(matches!(
            create_staging_attempt(&root_fd, "staging", "../escape"),
            Err(ArchiveError::UnsafeActivationName)
        ));
        assert!(!root.path().join("staging").exists());

        std::fs::create_dir(root.path().join("outside")).expect("outside directory");
        std::os::unix::fs::symlink("outside", root.path().join("staging"))
            .expect("hostile staging symlink");
        assert!(create_staging_attempt(&root_fd, "staging", "rst_attempt").is_err());
        assert!(!root.path().join("outside/rst_attempt").exists());
        assert!(matches!(
            activate_no_replace(&root_fd, "../source", &root_fd, "current"),
            Err(ArchiveError::UnsafeActivationName)
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn anchored_activation_ignores_a_replaced_root_path() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root_path = temporary.path().join("fasti-data");
        let moved_path = temporary.path().join("moved-fasti-data");
        let guard = crate::LockedDataRoot::acquire(&root_path).expect("locked data root");
        let root_fd = guard
            .anchored_directory()
            .expect("Linux anchored data root");

        std::fs::rename(&root_path, &moved_path).expect("rename locked root");
        std::fs::create_dir(&root_path).expect("replacement root path");

        let (staging, attempt) = create_staging_attempt(root_fd, "staging", "rst_anchored")
            .expect("create attempt under anchored root");
        let mut marker =
            open_new_file_beneath(&attempt, Path::new("marker")).expect("create anchored marker");
        marker.write_all(b"anchored").expect("write marker");
        sync_open_handle(&marker).expect("fsync marker");
        sync_open_handle(&attempt).expect("fsync attempt");
        sync_open_handle(&staging).expect("fsync staging");
        activate_no_replace(&staging, "rst_anchored", root_fd, "current")
            .expect("activate beneath anchored root");
        sync_open_handle(root_fd).expect("fsync anchored root");

        assert_eq!(
            std::fs::read(moved_path.join("current/marker")).expect("anchored active marker"),
            b"anchored"
        );
        assert!(!root_path.join("staging").exists());
        assert!(!root_path.join("current").exists());
    }
}
