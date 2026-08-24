//! Private pass-one verification for hostile B3 workspace archives.
//!
//! This module does not create staging state or activate restored data. It
//! consumes one already-open seekable source through the archive module's sole
//! parser, retains only compact descriptors plus the bounded final manifest,
//! and rewinds that same source for the later import pass.

use crate::archive::{
    visit_archive_entries, ArchiveEntryReader, ArchiveError, ArchiveLimits, ArchiveSummary,
    MAX_IO_CHUNK_BYTES,
};
use crate::crypto::encode_hex;
use crate::evidence::{canonical_digest_hex, path_to_storage_value, relative_evidence_path};
use fasti_application::{PortabilityLimits, ReadSeek, WorkspaceExportEntity};
use fasti_contracts::{VerifiedInboundWorkspaceManifest, WorkspaceManifestConversionError};
use fasti_domain::Sha256Digest;
use sha2::{Digest, Sha256};
use std::io::{self, Read, SeekFrom};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum RestorePreflightError {
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error(transparent)]
    Manifest(#[from] WorkspaceManifestConversionError),
    #[error("archive source could not seek to its first byte")]
    InitialSeek(#[source] io::Error),
    #[error("archive source could not be rewound for restore pass two")]
    Rewind(#[source] io::Error),
    #[error("archive entry {actual} is out of order; expected {expected}")]
    EntryOrder { expected: String, actual: String },
    #[error("archive entry path {path} exceeds configured byte limit {limit}")]
    PathBytesExceeded { path: String, limit: u64 },
    #[error("archive entry path {path} exceeds configured depth limit {limit}")]
    PathDepthExceeded { path: String, limit: u64 },
    #[error("archive manifest size cannot be represented on this platform")]
    ManifestSizeUnsupported,
    #[error("bounded archive manifest memory could not be reserved")]
    ManifestAllocationFailed,
    #[error("configured expanded archive ceiling overflows u64")]
    ExpandedCeilingOverflow,
    #[error("archive entry {path} could not be read")]
    EntryRead {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("archive entry {path} byte count overflowed")]
    EntryByteCountOverflow { path: String },
    #[error("archive stream {path} row count exceeds {limit}")]
    StreamRowCountExceeded { path: String, limit: u64 },
    #[error("archive stream {path} contains a blank NDJSON line")]
    BlankNdjsonLine { path: String },
    #[error("archive stream {path} is not newline-terminated")]
    NonTerminatedNdjson { path: String },
    #[error("archive preflight did not retain manifest.json")]
    MissingVerifiedManifest,
    #[error("archive stream count does not match the frozen inventory")]
    StreamCountMismatch,
    #[error("archive stream descriptor does not match {path}")]
    StreamDescriptorMismatch { path: String },
    #[error("archive blob count does not match manifest.json")]
    BlobCountMismatch,
    #[error("archive blob descriptor does not match {path}")]
    BlobDescriptorMismatch { path: String },
    #[error(
        "archive expansion ratio exceeds {limit}: {expanded_bytes} expanded bytes from {compressed_bytes} compressed bytes"
    )]
    DecompressionRatioExceeded {
        expanded_bytes: u64,
        compressed_bytes: u64,
        limit: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedStream {
    path: String,
    byte_length: u64,
    row_count: u64,
    digest: Sha256Digest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedBlob {
    path: String,
    byte_length: u64,
    digest: Sha256Digest,
}

/// Verified pass-one state retained for the later import pass.
#[allow(dead_code)] // consumed by the next B3 restore slice
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedArchivePreflight {
    manifest: VerifiedInboundWorkspaceManifest,
    archive_digest: Sha256Digest,
    archive_bytes: u64,
    archive_summary: ArchiveSummary,
}

#[allow(dead_code)] // consumed by the next B3 restore slice
impl VerifiedArchivePreflight {
    pub(crate) const fn manifest(&self) -> &VerifiedInboundWorkspaceManifest {
        &self.manifest
    }

    pub(crate) const fn archive_digest(&self) -> &Sha256Digest {
        &self.archive_digest
    }

    pub(crate) const fn archive_bytes(&self) -> u64 {
        self.archive_bytes
    }

    pub(crate) const fn archive_summary(&self) -> ArchiveSummary {
        self.archive_summary
    }
}

struct DigestingReader<R> {
    inner: R,
    hasher: Sha256,
    bytes_read: u64,
}

impl<R> DigestingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes_read: 0,
        }
    }

    fn digest(&self) -> Sha256Digest {
        digest_from_bytes(&self.hasher.clone().finalize())
    }
}

impl<R: Read> Read for DigestingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.hasher.update(&buffer[..read]);
        self.bytes_read = self
            .bytes_read
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("compressed archive byte count overflow"))?;
        Ok(read)
    }
}

/// Strictly verify pass one without creating or changing destination state.
///
/// The source is sought to byte zero before parsing and is returned at byte
/// zero on every post-start outcome. A rewind failure replaces the parse
/// result because pass two must never proceed from an unknown cursor.
#[allow(dead_code)] // activated by the next B3 restore orchestration slice
pub(crate) fn preflight_workspace_archive(
    source: &mut dyn ReadSeek,
    limits: PortabilityLimits,
) -> Result<VerifiedArchivePreflight, RestorePreflightError> {
    let expanded_ceiling = limits
        .archive_expanded_ceiling()
        .ok_or(RestorePreflightError::ExpandedCeilingOverflow)?;
    let archive_limits = ArchiveLimits::new(
        limits.max_archive_bytes.get(),
        limits.max_entries.get(),
        limits.max_entry_bytes.get(),
        expanded_ceiling,
    )?;
    source
        .seek(SeekFrom::Start(0))
        .map_err(RestorePreflightError::InitialSeek)?;

    let result = {
        let mut source = DigestingReader::new(&mut *source);
        let mut streams = Vec::with_capacity(WorkspaceExportEntity::ALL.len());
        let mut blobs = Vec::new();
        let mut manifest = None;

        let archive_summary =
            visit_archive_entries(&mut source, archive_limits, |path, size, reader| {
                enforce_configured_path(path, limits)?;
                if streams.len() < WorkspaceExportEntity::ALL.len() {
                    let entity = WorkspaceExportEntity::ALL[streams.len()];
                    let expected = format!("{}.ndjson", entity.as_str());
                    if path != expected {
                        return Err(RestorePreflightError::EntryOrder {
                            expected,
                            actual: path.to_owned(),
                        });
                    }
                    streams.push(inspect_stream(
                        path,
                        size,
                        reader,
                        limits.max_rows_per_stream.get(),
                    )?);
                } else if path == "manifest.json" {
                    let bytes = read_manifest(path, size, reader)?;
                    manifest = Some(VerifiedInboundWorkspaceManifest::try_from_canonical_json(
                        &bytes, limits,
                    )?);
                } else {
                    blobs.push(inspect_blob(path, size, reader)?);
                }
                Ok(())
            });
        let archive_bytes = source.bytes_read;
        let archive_digest = source.digest();
        archive_summary.and_then(|archive_summary| {
            let manifest = manifest.ok_or(RestorePreflightError::MissingVerifiedManifest)?;
            enforce_ratio(archive_summary, archive_bytes, limits)?;
            verify_observations(&manifest, &streams, &blobs)?;
            Ok(VerifiedArchivePreflight {
                manifest,
                archive_digest,
                archive_bytes,
                archive_summary,
            })
        })
    };

    source
        .seek(SeekFrom::Start(0))
        .map_err(RestorePreflightError::Rewind)?;
    result
}

fn inspect_stream(
    path: &str,
    declared_size: u64,
    reader: &mut ArchiveEntryReader<'_>,
    max_rows: u64,
) -> Result<ObservedStream, RestorePreflightError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; MAX_IO_CHUNK_BYTES];
    let mut byte_length = 0_u64;
    let mut row_count = 0_u64;
    let mut line_has_bytes = false;
    let mut line_has_non_whitespace = false;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| RestorePreflightError::EntryRead {
                path: path.to_owned(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        byte_length = byte_length.checked_add(read as u64).ok_or_else(|| {
            RestorePreflightError::EntryByteCountOverflow {
                path: path.to_owned(),
            }
        })?;
        for byte in &buffer[..read] {
            if *byte == b'\n' {
                if !line_has_non_whitespace {
                    return Err(RestorePreflightError::BlankNdjsonLine {
                        path: path.to_owned(),
                    });
                }
                row_count = row_count.checked_add(1).ok_or_else(|| {
                    RestorePreflightError::StreamRowCountExceeded {
                        path: path.to_owned(),
                        limit: max_rows,
                    }
                })?;
                if row_count > max_rows {
                    return Err(RestorePreflightError::StreamRowCountExceeded {
                        path: path.to_owned(),
                        limit: max_rows,
                    });
                }
                line_has_bytes = false;
                line_has_non_whitespace = false;
            } else {
                line_has_bytes = true;
                if !matches!(*byte, b' ' | b'\t' | b'\r') {
                    line_has_non_whitespace = true;
                }
            }
        }
    }
    if line_has_bytes {
        return Err(RestorePreflightError::NonTerminatedNdjson {
            path: path.to_owned(),
        });
    }
    if byte_length != declared_size {
        return Err(ArchiveError::TruncatedEntry {
            path: path.to_owned(),
        }
        .into());
    }
    Ok(ObservedStream {
        path: path.to_owned(),
        byte_length,
        row_count,
        digest: digest_from_bytes(&hasher.finalize()),
    })
}

fn inspect_blob(
    path: &str,
    declared_size: u64,
    reader: &mut ArchiveEntryReader<'_>,
) -> Result<ObservedBlob, RestorePreflightError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; MAX_IO_CHUNK_BYTES];
    let mut byte_length = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| RestorePreflightError::EntryRead {
                path: path.to_owned(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        byte_length = byte_length.checked_add(read as u64).ok_or_else(|| {
            RestorePreflightError::EntryByteCountOverflow {
                path: path.to_owned(),
            }
        })?;
    }
    if byte_length != declared_size {
        return Err(ArchiveError::TruncatedEntry {
            path: path.to_owned(),
        }
        .into());
    }
    Ok(ObservedBlob {
        path: path.to_owned(),
        byte_length,
        digest: digest_from_bytes(&hasher.finalize()),
    })
}

fn read_manifest(
    path: &str,
    size: u64,
    reader: &mut ArchiveEntryReader<'_>,
) -> Result<Vec<u8>, RestorePreflightError> {
    let capacity =
        usize::try_from(size).map_err(|_| RestorePreflightError::ManifestSizeUnsupported)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| RestorePreflightError::ManifestAllocationFailed)?;
    let mut buffer = [0_u8; MAX_IO_CHUNK_BYTES];
    while bytes.len() < capacity {
        let remaining = capacity - bytes.len();
        let read = reader
            .read(&mut buffer[..remaining.min(MAX_IO_CHUNK_BYTES)])
            .map_err(|source| RestorePreflightError::EntryRead {
                path: path.to_owned(),
                source,
            })?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    if bytes.len() != capacity {
        return Err(ArchiveError::TruncatedEntry {
            path: path.to_owned(),
        }
        .into());
    }
    Ok(bytes)
}

fn verify_observations(
    verified: &VerifiedInboundWorkspaceManifest,
    streams: &[ObservedStream],
    blobs: &[ObservedBlob],
) -> Result<(), RestorePreflightError> {
    let manifest = verified.manifest();
    if streams.len() != manifest.streams().len() {
        return Err(RestorePreflightError::StreamCountMismatch);
    }
    for (observed, expected) in streams.iter().zip(manifest.streams()) {
        let expected_path = format!("{}.ndjson", expected.entity().as_str());
        if observed.path != expected_path
            || observed.byte_length != expected.byte_length()
            || observed.row_count != expected.row_count()
            || &observed.digest != expected.digest()
        {
            return Err(RestorePreflightError::StreamDescriptorMismatch {
                path: expected_path,
            });
        }
    }

    if blobs.len() != manifest.blobs().len() {
        return Err(RestorePreflightError::BlobCountMismatch);
    }
    for (observed, expected) in blobs.iter().zip(manifest.blobs()) {
        let digest_hex = canonical_digest_hex(expected.digest().as_str())
            .expect("verified contract digest has canonical SHA-256 syntax");
        let expected_path = path_to_storage_value(&relative_evidence_path(digest_hex));
        if observed.path != expected_path
            || observed.byte_length != expected.byte_length()
            || &observed.digest != expected.digest()
        {
            return Err(RestorePreflightError::BlobDescriptorMismatch {
                path: expected_path,
            });
        }
    }
    Ok(())
}

fn enforce_ratio(
    summary: ArchiveSummary,
    compressed_bytes: u64,
    limits: PortabilityLimits,
) -> Result<(), RestorePreflightError> {
    let limit = limits.max_decompression_ratio.get();
    if compressed_bytes == 0 || summary.expanded_bytes > compressed_bytes.saturating_mul(limit) {
        return Err(RestorePreflightError::DecompressionRatioExceeded {
            expanded_bytes: summary.expanded_bytes,
            compressed_bytes,
            limit,
        });
    }
    Ok(())
}

fn enforce_configured_path(
    path: &str,
    limits: PortabilityLimits,
) -> Result<(), RestorePreflightError> {
    if u64::try_from(path.len()).map_or(true, |length| length > limits.max_path_bytes.get()) {
        return Err(RestorePreflightError::PathBytesExceeded {
            path: path.to_owned(),
            limit: limits.max_path_bytes.get(),
        });
    }
    let depth = u64::try_from(path.split('/').count()).unwrap_or(u64::MAX);
    if depth > limits.max_path_depth.get() {
        return Err(RestorePreflightError::PathDepthExceeded {
            path: path.to_owned(),
            limit: limits.max_path_depth.get(),
        });
    }
    Ok(())
}

fn digest_from_bytes(bytes: &[u8]) -> Sha256Digest {
    Sha256Digest::parse(format!("sha256:{}", encode_hex(bytes)))
        .expect("SHA-256 output is canonical lowercase hexadecimal")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_application::{
        WorkspaceBlobDescriptor, WorkspaceManifest, WorkspaceStreamDescriptor,
        WORKSPACE_ARCHIVE_CONTRACT_VERSION,
    };
    use fasti_contracts::CanonicalWorkspaceManifestProjection;
    use fasti_domain::{EvidenceId, WorkspaceId};
    use std::io::{Cursor, Write};
    use std::num::NonZeroU64;

    struct Fixture {
        entries: Vec<(String, Vec<u8>)>,
    }

    fn limits() -> PortabilityLimits {
        let archive = NonZeroU64::new(16 * 1024 * 1024).expect("archive limit");
        let uncompressed = NonZeroU64::new(32 * 1024 * 1024).expect("expanded limit");
        let entry = NonZeroU64::new(16 * 1024 * 1024).expect("entry limit");
        let entries = NonZeroU64::new(64).expect("entry-count limit");
        let rows = NonZeroU64::new(1_000_000).expect("row limit");
        let path_bytes = NonZeroU64::new(100).expect("path byte limit");
        let path_depth = NonZeroU64::new(8).expect("path depth limit");
        let ratio = NonZeroU64::new(10_000).expect("ratio limit");
        let one = NonZeroU64::new(1).expect("unit limit");
        PortabilityLimits {
            max_snapshot_bytes: uncompressed,
            max_wal_growth_bytes: uncompressed,
            max_archive_bytes: archive,
            max_uncompressed_bytes: uncompressed,
            max_entry_bytes: entry,
            max_entries: entries,
            max_rows_per_stream: rows,
            max_path_bytes: path_bytes,
            max_path_depth: path_depth,
            max_decompression_ratio: ratio,
            scratch_ceiling_bytes: uncompressed,
            cleanup_reserve_bytes: archive,
            backup_step_pages: one,
            backup_step_millis: one,
        }
    }

    fn fixture(workspaces: Vec<u8>, declared_workspace_rows: Option<u64>) -> Fixture {
        let streams = WorkspaceExportEntity::ALL
            .into_iter()
            .map(|entity| {
                let bytes = if entity == WorkspaceExportEntity::Workspaces {
                    workspaces.as_slice()
                } else {
                    &[]
                };
                WorkspaceStreamDescriptor::new(
                    entity,
                    if entity == WorkspaceExportEntity::Workspaces {
                        declared_workspace_rows.unwrap_or_else(|| {
                            bytes.iter().filter(|byte| **byte == b'\n').count() as u64
                        })
                    } else {
                        0
                    },
                    bytes.len() as u64,
                    digest_from_bytes(&Sha256::digest(bytes)),
                )
            })
            .collect::<Vec<_>>();
        let blob_bytes = b"canonical evidence payload".to_vec();
        let blob_digest = digest_from_bytes(&Sha256::digest(&blob_bytes));
        let blobs = vec![WorkspaceBlobDescriptor::new(
            EvidenceId::new_v7(),
            blob_bytes.len() as u64,
            blob_digest.clone(),
        )];
        let manifest = WorkspaceManifest::try_new(
            WorkspaceId::new_v7(),
            7,
            WORKSPACE_ARCHIVE_CONTRACT_VERSION.to_owned(),
            3,
            digest_from_bytes(&Sha256::digest(b"schema-v3")),
            streams,
            blobs,
        )
        .expect("valid archive-v1 fixture manifest");
        let manifest_bytes = CanonicalWorkspaceManifestProjection::try_from_application(manifest)
            .expect("canonical fixture projection")
            .canonical_json_bytes()
            .to_vec();

        let mut entries = WorkspaceExportEntity::ALL
            .into_iter()
            .map(|entity| {
                let bytes = if entity == WorkspaceExportEntity::Workspaces {
                    workspaces.clone()
                } else {
                    Vec::new()
                };
                (format!("{}.ndjson", entity.as_str()), bytes)
            })
            .collect::<Vec<_>>();
        let blob_path = path_to_storage_value(&relative_evidence_path(
            canonical_digest_hex(blob_digest.as_str()).expect("canonical fixture blob digest"),
        ));
        entries.push((blob_path, blob_bytes));
        entries.push(("manifest.json".to_owned(), manifest_bytes));
        Fixture { entries }
    }

    fn archive(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
        compress_tar(tar_bytes(entries), 22)
    }

    fn tar_bytes(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut bytes);
            builder.mode(tar::HeaderMode::Deterministic);
            for (path, payload) in entries {
                let mut header = tar::Header::new_ustar();
                header.set_path(path).expect("fixture path fits USTAR");
                header.set_size(payload.len() as u64);
                header.set_mode(0o600);
                header.set_uid(0);
                header.set_gid(0);
                header.set_mtime(0);
                header.set_entry_type(tar::EntryType::Regular);
                header.set_cksum();
                builder
                    .append(&header, Cursor::new(payload))
                    .expect("append fixture entry");
            }
            builder.finish().expect("finish fixture tar");
        }
        bytes
    }

    fn compress_tar(bytes: Vec<u8>, window_log: u32) -> Vec<u8> {
        let length = bytes.len() as u64;
        let mut encoder =
            zstd::stream::write::Encoder::new(Vec::new(), 3).expect("fixture zstd encoder");
        encoder.window_log(window_log).expect("fixture window log");
        encoder.include_checksum(true).expect("fixture checksum");
        encoder
            .set_pledged_src_size(Some(length))
            .expect("fixture pledged size");
        encoder.write_all(&bytes).expect("compress fixture tar");
        encoder.finish().expect("finish fixture zstd frame")
    }

    fn assert_rejected(entries: &[(String, Vec<u8>)]) -> RestorePreflightError {
        let mut source = Cursor::new(archive(entries));
        preflight_workspace_archive(&mut source, limits()).expect_err("hostile archive must fail")
    }

    #[test]
    fn canonical_preflight_hashes_the_complete_archive_and_rewinds_the_same_source() {
        let fixture = fixture(b"{\"workspace\":1}\n".to_vec(), None);
        let bytes = archive(&fixture.entries);
        let expected_digest = digest_from_bytes(&Sha256::digest(&bytes));
        let mut source = Cursor::new(bytes.clone());
        source.set_position(bytes.len() as u64);

        let first = preflight_workspace_archive(&mut source, limits())
            .expect("canonical preflight succeeds");
        assert_eq!(first.archive_bytes(), bytes.len() as u64);
        assert_eq!(first.archive_digest(), &expected_digest);
        assert_eq!(
            first.archive_summary().entries,
            (WorkspaceExportEntity::ALL.len() + 2) as u64
        );
        assert_eq!(source.position(), 0);

        let second = preflight_workspace_archive(&mut source, limits())
            .expect("same already-open source succeeds again");
        assert_eq!(second, first);
        assert_eq!(source.position(), 0);
    }

    #[test]
    fn manifest_json_must_be_complete_canonical_jcs_without_duplicate_fields() {
        let mut noncanonical = fixture(Vec::new(), None);
        noncanonical
            .entries
            .last_mut()
            .expect("manifest entry")
            .1
            .insert(0, b' ');
        assert!(matches!(
            assert_rejected(&noncanonical.entries),
            RestorePreflightError::Manifest(WorkspaceManifestConversionError::NonCanonicalJson)
        ));

        let mut duplicate = fixture(Vec::new(), None);
        let manifest = duplicate.entries.last_mut().expect("manifest entry");
        let canonical = std::str::from_utf8(&manifest.1).expect("canonical JSON is UTF-8");
        manifest.1 = canonical
            .replacen("{\"manifest\":", "{\"manifest\":null,\"manifest\":", 1)
            .into_bytes();
        assert!(matches!(
            assert_rejected(&duplicate.entries),
            RestorePreflightError::Manifest(
                WorkspaceManifestConversionError::InvalidJson
                    | WorkspaceManifestConversionError::NonCanonicalJson
            )
        ));
    }

    #[test]
    fn frozen_entry_order_rejects_reordered_extra_missing_and_duplicate_entries() {
        let fixture = fixture(Vec::new(), None);

        let mut reordered = fixture.entries.iter().map(Clone::clone).collect::<Vec<_>>();
        reordered.swap(0, 1);
        assert!(matches!(
            assert_rejected(&reordered),
            RestorePreflightError::EntryOrder { .. }
        ));

        for forbidden in [
            "credentials.ndjson",
            "profile_grants.ndjson",
            "grant_scopes.ndjson",
            "node_state.ndjson",
        ] {
            let mut extra = fixture.entries.clone();
            extra.insert(
                extra.len() - 1,
                (forbidden.to_owned(), b"secret\n".to_vec()),
            );
            assert!(matches!(
                assert_rejected(&extra),
                RestorePreflightError::BlobCountMismatch
            ));
        }

        let mut missing = fixture.entries.clone();
        missing.remove(0);
        assert!(matches!(
            assert_rejected(&missing),
            RestorePreflightError::EntryOrder { .. }
        ));

        let mut duplicate = fixture.entries.clone();
        duplicate[1].0 = duplicate[0].0.clone();
        assert!(matches!(
            assert_rejected(&duplicate),
            RestorePreflightError::Archive(ArchiveError::DuplicateEntry(_))
        ));
    }

    #[test]
    fn stream_size_digest_and_row_count_must_match_the_manifest() {
        let baseline = fixture(b"a\n".to_vec(), None);

        let mut size = baseline.entries.clone();
        size[0].1 = b"aa\n".to_vec();
        assert!(matches!(
            assert_rejected(&size),
            RestorePreflightError::StreamDescriptorMismatch { .. }
        ));

        let mut digest = baseline.entries.clone();
        digest[0].1 = b"b\n".to_vec();
        assert!(matches!(
            assert_rejected(&digest),
            RestorePreflightError::StreamDescriptorMismatch { .. }
        ));

        let rows = fixture(b"a\n".to_vec(), Some(2));
        assert!(matches!(
            assert_rejected(&rows.entries),
            RestorePreflightError::StreamDescriptorMismatch { .. }
        ));
    }

    #[test]
    fn blob_path_size_and_digest_must_match_the_manifest() {
        let fixture = fixture(Vec::new(), None);
        let blob_index = WorkspaceExportEntity::ALL.len();

        let mut path = fixture.entries.clone();
        path[blob_index].0 = format!("payloads/sha256/00/{}", "00".repeat(32));
        assert!(matches!(
            assert_rejected(&path),
            RestorePreflightError::BlobDescriptorMismatch { .. }
        ));

        let mut size = fixture.entries.clone();
        size[blob_index].1.push(b'x');
        assert!(matches!(
            assert_rejected(&size),
            RestorePreflightError::BlobDescriptorMismatch { .. }
        ));

        let mut digest = fixture.entries.clone();
        digest[blob_index].1[0] ^= 1;
        assert!(matches!(
            assert_rejected(&digest),
            RestorePreflightError::BlobDescriptorMismatch { .. }
        ));
    }

    #[test]
    fn ndjson_rejects_blank_lines_and_nonterminated_rows_but_accepts_empty_streams() {
        let blank = fixture(b" \t\r\n".to_vec(), None);
        assert!(matches!(
            assert_rejected(&blank.entries),
            RestorePreflightError::BlankNdjsonLine { .. }
        ));

        let nonterminated = fixture(b"row".to_vec(), Some(1));
        assert!(matches!(
            assert_rejected(&nonterminated.entries),
            RestorePreflightError::NonTerminatedNdjson { .. }
        ));

        let empty = fixture(Vec::new(), Some(0));
        let mut source = Cursor::new(archive(&empty.entries));
        preflight_workspace_archive(&mut source, limits()).expect("empty streams have zero rows");
    }

    #[test]
    fn tighter_configured_path_byte_and_depth_limits_are_enforced() {
        let fixture = fixture(Vec::new(), None);
        let bytes = archive(&fixture.entries);

        let mut byte_limits = limits();
        byte_limits.max_path_bytes = NonZeroU64::new(5).expect("path byte limit");
        let mut source = Cursor::new(bytes.clone());
        assert!(matches!(
            preflight_workspace_archive(&mut source, byte_limits),
            Err(RestorePreflightError::PathBytesExceeded { .. })
        ));
        assert_eq!(source.position(), 0);

        let mut depth_limits = limits();
        depth_limits.max_path_depth = NonZeroU64::new(3).expect("path depth limit");
        let mut source = Cursor::new(bytes);
        assert!(matches!(
            preflight_workspace_archive(&mut source, depth_limits),
            Err(RestorePreflightError::PathDepthExceeded { .. })
        ));
        assert_eq!(source.position(), 0);
    }

    #[test]
    fn configured_ratio_and_compressed_limits_fail_closed() {
        let bomb = fixture([vec![b'a'; 2 * 1024 * 1024], vec![b'\n']].concat(), Some(1));
        let mut source = Cursor::new(archive(&bomb.entries));
        let mut ratio_limits = limits();
        ratio_limits.max_decompression_ratio = NonZeroU64::new(2).expect("ratio limit");
        assert!(matches!(
            preflight_workspace_archive(&mut source, ratio_limits),
            Err(RestorePreflightError::DecompressionRatioExceeded { .. })
        ));
        assert_eq!(source.position(), 0);

        let canonical = fixture(Vec::new(), None);
        let bytes = archive(&canonical.entries);
        let mut source = Cursor::new(bytes.clone());
        let mut compressed_limits = limits();
        compressed_limits.max_archive_bytes =
            NonZeroU64::new(bytes.len() as u64 - 1).expect("compressed limit");
        assert!(matches!(
            preflight_workspace_archive(&mut source, compressed_limits),
            Err(RestorePreflightError::Archive(
                ArchiveError::CompressedSizeExceeded { .. }
            ))
        ));
        assert_eq!(source.position(), 0);
    }

    #[test]
    fn noncanonical_trailer_and_oversized_zstd_window_are_rejected() {
        let canonical_fixture = fixture(Vec::new(), None);
        let canonical = archive(&canonical_fixture.entries);
        let mut expanded =
            zstd::stream::decode_all(Cursor::new(canonical)).expect("decode canonical fixture");
        expanded.extend_from_slice(&[0; 512]);
        let mut source = Cursor::new(compress_tar(expanded, 22));
        assert!(matches!(
            preflight_workspace_archive(&mut source, limits()),
            Err(RestorePreflightError::Archive(ArchiveError::TrailingData))
        ));
        assert_eq!(source.position(), 0);

        let large = fixture([vec![b'a'; 8 * 1024 * 1024], vec![b'\n']].concat(), Some(1));
        let mut source = Cursor::new(compress_tar(tar_bytes(&large.entries), 23));
        assert!(matches!(
            preflight_workspace_archive(&mut source, limits()),
            Err(RestorePreflightError::Archive(ArchiveError::Io(_)))
        ));
        assert_eq!(source.position(), 0);
    }
}
