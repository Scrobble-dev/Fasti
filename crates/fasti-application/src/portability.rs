//! B3 workspace portability capabilities and adapter ports.
//!
//! Verification and export report bounded summary counts. Neither exposes
//! SQLite, filesystem, transport, provider, or UI details to callers.
//!
//! Export writes to a caller-supplied [`std::io::Write`] sink so the adapter
//! can stream bounded pages instead of materializing a workspace in memory.
//! `std::io::Write` is a standard-library boundary, not an adapter type, so
//! the domain-inward dependency rule holds.

use crate::{ApplicationResult, CapabilityKey, FastiProblem, RequestAccessContext, SecretMaterial};
use fasti_domain::{
    ArchiveProfile, ClientId, EvidenceId, ExportScope, ProfileId, RecoveryGrantPolicy,
    RequestCorrelationId, RestoreAttemptId, RestorePolicy, RestoreStatus, Sha256Digest,
    WorkspaceId,
};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::num::NonZeroU64;

/// Internal draft archive format version written by the export adapter.
///
/// A restore implementation must reject any version it does not understand
/// rather than guessing at the framing. This is not a public format activation,
/// and its stream inventory remains unfrozen pending namespace ownership.
pub const WORKSPACE_ARCHIVE_FORMAT_VERSION: u32 = 1;

/// Largest integer with one interoperable RFC 8785/I-JSON representation.
pub const MAX_PORTABLE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

/// Transitional name used by the staged NDJSON stream writer.
///
/// The stream is one component of the archive profile. It is not a complete
/// `.fasti` archive by itself.
pub const WORKSPACE_EXPORT_FORMAT_VERSION: u32 = WORKSPACE_ARCHIVE_FORMAT_VERSION;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyWorkspaceQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl VerifyWorkspaceQuery {
    pub const fn new(correlation_id: RequestCorrelationId, access: RequestAccessContext) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceVerificationOutcome {
    workspace_id: WorkspaceId,
    observations_verified: u64,
    evidence_verified: u64,
    corrections_verified: u64,
}

impl WorkspaceVerificationOutcome {
    pub const fn new(
        workspace_id: WorkspaceId,
        observations_verified: u64,
        evidence_verified: u64,
        corrections_verified: u64,
    ) -> Self {
        Self {
            workspace_id,
            observations_verified,
            evidence_verified,
            corrections_verified,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn observations_verified(&self) -> u64 {
        self.observations_verified
    }

    pub const fn evidence_verified(&self) -> u64 {
        self.evidence_verified
    }

    pub const fn corrections_verified(&self) -> u64 {
        self.corrections_verified
    }
}

/// Read-only B3 integrity-verification boundary.
///
/// Implementations must re-authorize against current durable state and verify
/// persisted Chronicle relations and evidence bytes before returning success.
pub trait WorkspaceVerificationPort: Send + Sync {
    fn verify_workspace(
        &self,
        query: VerifyWorkspaceQuery,
    ) -> ApplicationResult<WorkspaceVerificationOutcome>;
}

/// One durable entity stream in a workspace export archive.
///
/// The order of [`WorkspaceExportEntity::ALL`] is the order sections are
/// written. It is part of the archive format: changing it changes the bytes
/// and therefore requires a format-version change.
///
/// Excluded by policy, and deliberately absent from this enum: credential
/// secrets, initialization proof material, active authorization grants and
/// their scopes, and node-local listener configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkspaceExportEntity {
    Workspaces,
    Profiles,
    Clients,
    Records,
    NamespaceDefinitions,
    ExternalIdentifiers,
    Evidence,
    Observations,
    ObservationClues,
    Occurrences,
    Interpretations,
    ReviewItems,
    ReviewCandidates,
    Corrections,
    Receipts,
    Operations,
}

impl WorkspaceExportEntity {
    /// Every exported entity, in the frozen archive-v1 section order.
    ///
    /// Freezing these archive bytes does not activate the staged public
    /// export capability or any runtime route.
    pub const ALL: [Self; 16] = [
        Self::Workspaces,
        Self::Profiles,
        Self::Clients,
        Self::Records,
        Self::NamespaceDefinitions,
        Self::ExternalIdentifiers,
        Self::Evidence,
        Self::Observations,
        Self::ObservationClues,
        Self::Occurrences,
        Self::Interpretations,
        Self::ReviewItems,
        Self::ReviewCandidates,
        Self::Corrections,
        Self::Receipts,
        Self::Operations,
    ];

    /// Stable section name written into the archive and the manifest.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Workspaces => "workspaces",
            Self::Profiles => "profiles",
            Self::Clients => "clients",
            Self::Records => "records",
            Self::NamespaceDefinitions => "namespaces",
            Self::ExternalIdentifiers => "external_identifiers",
            Self::Evidence => "evidence",
            Self::Observations => "observations",
            Self::ObservationClues => "observation_clues",
            Self::Occurrences => "occurrences",
            Self::Interpretations => "interpretations",
            Self::ReviewItems => "review_items",
            Self::ReviewCandidates => "review_candidates",
            Self::Corrections => "corrections",
            Self::Receipts => "receipts",
            Self::Operations => "operations",
        }
    }

    /// Position of this entity in [`WorkspaceExportEntity::ALL`].
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Every configured ceiling needed by export admission and restore preflight.
///
/// The composition root supplies deployment values. Non-zero types prevent an
/// adapter from silently treating an unset ceiling as unbounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortabilityLimits {
    pub max_snapshot_bytes: NonZeroU64,
    pub max_wal_growth_bytes: NonZeroU64,
    pub max_archive_bytes: NonZeroU64,
    pub max_uncompressed_bytes: NonZeroU64,
    pub max_entry_bytes: NonZeroU64,
    pub max_entries: NonZeroU64,
    pub max_rows_per_stream: NonZeroU64,
    pub max_path_bytes: NonZeroU64,
    pub max_path_depth: NonZeroU64,
    pub max_decompression_ratio: NonZeroU64,
    pub scratch_ceiling_bytes: NonZeroU64,
    pub cleanup_reserve_bytes: NonZeroU64,
    pub backup_step_pages: NonZeroU64,
    pub backup_step_millis: NonZeroU64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceStreamDescriptor {
    entity: WorkspaceExportEntity,
    row_count: u64,
    byte_length: u64,
    digest: Sha256Digest,
}

impl WorkspaceStreamDescriptor {
    pub const fn new(
        entity: WorkspaceExportEntity,
        row_count: u64,
        byte_length: u64,
        digest: Sha256Digest,
    ) -> Self {
        Self {
            entity,
            row_count,
            byte_length,
            digest,
        }
    }

    pub const fn entity(&self) -> WorkspaceExportEntity {
        self.entity
    }

    pub const fn row_count(&self) -> u64 {
        self.row_count
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBlobDescriptor {
    evidence_id: EvidenceId,
    byte_length: u64,
    digest: Sha256Digest,
}

impl WorkspaceBlobDescriptor {
    pub const fn new(evidence_id: EvidenceId, byte_length: u64, digest: Sha256Digest) -> Self {
        Self {
            evidence_id,
            byte_length,
            digest,
        }
    }

    pub const fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceManifestError {
    EmptyContractVersion,
    ContractVersionTooLong,
    PortableIntegerOutOfRange,
    IncompleteStreamSet,
    NonCanonicalBlobOrder,
    DuplicateEvidenceId,
    DuplicateBlobDigest,
}

/// Checksummed full-workspace manifest body.
///
/// The checksum is deliberately held by [`ChecksummedWorkspaceManifest`], so
/// it covers canonical bytes of this body without a self-reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceManifest {
    workspace_id: WorkspaceId,
    workspace_revision: u64,
    contract_version: String,
    migration_version: u32,
    migration_digest: Sha256Digest,
    streams: Vec<WorkspaceStreamDescriptor>,
    blobs: Vec<WorkspaceBlobDescriptor>,
}

impl WorkspaceManifest {
    pub fn try_new(
        workspace_id: WorkspaceId,
        workspace_revision: u64,
        contract_version: String,
        migration_version: u32,
        migration_digest: Sha256Digest,
        streams: Vec<WorkspaceStreamDescriptor>,
        blobs: Vec<WorkspaceBlobDescriptor>,
    ) -> Result<Self, WorkspaceManifestError> {
        if contract_version.is_empty() {
            return Err(WorkspaceManifestError::EmptyContractVersion);
        }
        if contract_version.chars().count() > 64 {
            return Err(WorkspaceManifestError::ContractVersionTooLong);
        }
        if workspace_revision > MAX_PORTABLE_JSON_INTEGER
            || streams.iter().any(|stream| {
                stream.row_count() > MAX_PORTABLE_JSON_INTEGER
                    || stream.byte_length() > MAX_PORTABLE_JSON_INTEGER
            })
            || blobs
                .iter()
                .any(|blob| blob.byte_length() > MAX_PORTABLE_JSON_INTEGER)
        {
            return Err(WorkspaceManifestError::PortableIntegerOutOfRange);
        }
        if streams.len() != WorkspaceExportEntity::ALL.len()
            || streams
                .iter()
                .map(WorkspaceStreamDescriptor::entity)
                .ne(WorkspaceExportEntity::ALL)
        {
            return Err(WorkspaceManifestError::IncompleteStreamSet);
        }
        for adjacent in blobs.windows(2) {
            match adjacent[0]
                .evidence_id()
                .uuid()
                .cmp(&adjacent[1].evidence_id().uuid())
            {
                Ordering::Less => {}
                Ordering::Equal => return Err(WorkspaceManifestError::DuplicateEvidenceId),
                Ordering::Greater => {
                    return Err(WorkspaceManifestError::NonCanonicalBlobOrder);
                }
            }
        }
        let mut blob_digests = HashSet::with_capacity(blobs.len());
        if blobs.iter().any(|blob| !blob_digests.insert(blob.digest())) {
            return Err(WorkspaceManifestError::DuplicateBlobDigest);
        }

        Ok(Self {
            workspace_id,
            workspace_revision,
            contract_version,
            migration_version,
            migration_digest,
            streams,
            blobs,
        })
    }

    pub const fn format_version(&self) -> u32 {
        WORKSPACE_ARCHIVE_FORMAT_VERSION
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn export_scope(&self) -> ExportScope {
        ExportScope::FullWorkspace
    }

    pub const fn archive_profile(&self) -> ArchiveProfile {
        ArchiveProfile::ZstdL3W22
    }

    pub const fn restore_policy(&self) -> RestorePolicy {
        RestorePolicy::CleanOnly
    }

    pub const fn recovery_grant_policy(&self) -> RecoveryGrantPolicy {
        RecoveryGrantPolicy::RequireFreshBootstrap
    }

    pub const fn workspace_revision(&self) -> u64 {
        self.workspace_revision
    }

    pub fn contract_version(&self) -> &str {
        &self.contract_version
    }

    pub const fn migration_version(&self) -> u32 {
        self.migration_version
    }

    pub const fn migration_digest(&self) -> &Sha256Digest {
        &self.migration_digest
    }

    pub fn streams(&self) -> &[WorkspaceStreamDescriptor] {
        &self.streams
    }

    pub fn blobs(&self) -> &[WorkspaceBlobDescriptor] {
        &self.blobs
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksummedWorkspaceManifest {
    manifest: WorkspaceManifest,
    digest: Sha256Digest,
}

impl ChecksummedWorkspaceManifest {
    pub const fn new(manifest: WorkspaceManifest, digest: Sha256Digest) -> Self {
        Self { manifest, digest }
    }

    pub const fn manifest(&self) -> &WorkspaceManifest {
        &self.manifest
    }

    pub const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportWorkspaceQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl ExportWorkspaceQuery {
    pub const fn new(correlation_id: RequestCorrelationId, access: RequestAccessContext) -> Self {
        Self {
            correlation_id,
            access,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportWorkspaceRequest {
    query: ExportWorkspaceQuery,
    limits: PortabilityLimits,
}

/// Operation identity retained when a portability failure is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortabilityFailureOperation {
    OnlineExport {
        correlation_id: RequestCorrelationId,
    },
    CleanRestore {
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
    },
}

impl PortabilityFailureOperation {
    pub const fn correlation_id(self) -> RequestCorrelationId {
        match self {
            Self::OnlineExport { correlation_id } | Self::CleanRestore { correlation_id, .. } => {
                correlation_id
            }
        }
    }

    pub const fn restore_attempt_id(self) -> Option<RestoreAttemptId> {
        match self {
            Self::OnlineExport { .. } => None,
            Self::CleanRestore {
                restore_attempt_id, ..
            } => Some(restore_attempt_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortabilityFailureReceiptError {
    CapabilityMismatch,
}

/// Typed failure returned outside a partial archive destination.
///
/// An adapter must abort and discard the destination before returning this
/// receipt. The receipt therefore remains available when partial bytes do not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortabilityFailureReceipt {
    operation: PortabilityFailureOperation,
    problem: Box<FastiProblem>,
}

impl PortabilityFailureReceipt {
    pub fn try_online_export(
        problem: Box<FastiProblem>,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        if problem.capability() != CapabilityKey::ExportWorkspace {
            return Err(PortabilityFailureReceiptError::CapabilityMismatch);
        }
        let operation = PortabilityFailureOperation::OnlineExport {
            correlation_id: problem.correlation_id(),
        };
        Ok(Self { operation, problem })
    }

    pub fn try_clean_restore(
        restore_attempt_id: RestoreAttemptId,
        problem: Box<FastiProblem>,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        if problem.capability() != CapabilityKey::RestoreWorkspace {
            return Err(PortabilityFailureReceiptError::CapabilityMismatch);
        }
        let operation = PortabilityFailureOperation::CleanRestore {
            restore_attempt_id,
            correlation_id: problem.correlation_id(),
        };
        Ok(Self { operation, problem })
    }

    pub const fn operation(&self) -> PortabilityFailureOperation {
        self.operation
    }

    pub fn problem(&self) -> &FastiProblem {
        &self.problem
    }
}

pub type PortabilityResult<T> = Result<T, PortabilityFailureReceipt>;

impl ExportWorkspaceRequest {
    pub const fn new(query: ExportWorkspaceQuery, limits: PortabilityLimits) -> Self {
        Self { query, limits }
    }

    pub const fn query(&self) -> &ExportWorkspaceQuery {
        &self.query
    }

    pub const fn scope(&self) -> ExportScope {
        ExportScope::FullWorkspace
    }

    pub const fn archive_profile(&self) -> ArchiveProfile {
        ArchiveProfile::ZstdL3W22
    }

    pub const fn limits(&self) -> PortabilityLimits {
        self.limits
    }
}

/// Owned partial-archive lifecycle.
///
/// Consuming completion and abort methods ensure the caller cannot reuse a
/// published or discarded destination. Implementations own synchronization,
/// no-replace publication, and partial-file removal.
pub trait WorkspaceArchiveDestination: Write + Send {
    fn complete(
        self: Box<Self>,
        archive_digest: &Sha256Digest,
        manifest_digest: &Sha256Digest,
    ) -> std::io::Result<()>;

    fn abort(self: Box<Self>) -> std::io::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceArchiveExportOutcome {
    archive_bytes: u64,
    archive_digest: Sha256Digest,
    manifest: ChecksummedWorkspaceManifest,
}

impl WorkspaceArchiveExportOutcome {
    pub const fn new(
        archive_bytes: u64,
        archive_digest: Sha256Digest,
        manifest: ChecksummedWorkspaceManifest,
    ) -> Self {
        Self {
            archive_bytes,
            archive_digest,
            manifest,
        }
    }

    pub const fn archive_bytes(&self) -> u64 {
        self.archive_bytes
    }

    pub const fn archive_digest(&self) -> &Sha256Digest {
        &self.archive_digest
    }

    pub const fn manifest(&self) -> &ChecksummedWorkspaceManifest {
        &self.manifest
    }
}

/// Complete `.fasti` export boundary above the staged entity-stream writer.
pub trait WorkspaceArchiveExportPort: Send + Sync {
    fn export_workspace_archive(
        &self,
        request: ExportWorkspaceRequest,
        destination: Box<dyn WorkspaceArchiveDestination>,
    ) -> PortabilityResult<WorkspaceArchiveExportOutcome>;
}

/// Bounded summary of one completed workspace export.
///
/// Counts are per entity so a restore can assert that it consumed exactly
/// what the export produced. The digest covers every byte written to the
/// sink, so it cannot be embedded in the archive itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceExportOutcome {
    workspace_id: WorkspaceId,
    format_version: u32,
    counts: [u64; WorkspaceExportEntity::ALL.len()],
    bytes_written: u64,
    archive_digest: String,
}

impl WorkspaceExportOutcome {
    pub const fn new(
        workspace_id: WorkspaceId,
        format_version: u32,
        counts: [u64; WorkspaceExportEntity::ALL.len()],
        bytes_written: u64,
        archive_digest: String,
    ) -> Self {
        Self {
            workspace_id,
            format_version,
            counts,
            bytes_written,
            archive_digest,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn format_version(&self) -> u32 {
        self.format_version
    }

    /// Rows written for one entity section.
    pub const fn count(&self, entity: WorkspaceExportEntity) -> u64 {
        self.counts[entity.index()]
    }

    /// Every per-entity count in archive section order.
    pub fn counts(&self) -> impl Iterator<Item = (WorkspaceExportEntity, u64)> + '_ {
        WorkspaceExportEntity::ALL
            .into_iter()
            .map(|entity| (entity, self.counts[entity.index()]))
    }

    pub const fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// `sha256:<64 lowercase hex>` over every byte written to the sink.
    pub fn archive_digest(&self) -> &str {
        &self.archive_digest
    }
}

/// Staged deterministic entity-stream boundary.
///
/// Implementations must:
///
/// - re-authorize against current durable state for every bounded page, so a
///   revocation part-way through a long export stops further disclosure;
/// - write deterministically, so the same durable state produces identical
///   bytes across processes and hosts;
/// - stream bounded pages rather than materializing the workspace;
/// - exclude credential secrets, initialization proof material, and active
///   authorization bindings.
///
/// A partially written sink is not a valid archive. Callers must treat any
/// error as "discard the destination", because bytes already handed to the
/// sink cannot be recalled.
pub trait WorkspaceExportPort: Send + Sync {
    fn export_workspace(
        &self,
        query: ExportWorkspaceQuery,
        sink: &mut dyn Write,
    ) -> ApplicationResult<WorkspaceExportOutcome>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreWorkspaceRequest {
    restore_attempt_id: RestoreAttemptId,
    correlation_id: RequestCorrelationId,
    limits: PortabilityLimits,
}

impl RestoreWorkspaceRequest {
    pub const fn new(
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
        limits: PortabilityLimits,
    ) -> Self {
        Self {
            restore_attempt_id,
            correlation_id,
            limits,
        }
    }

    pub const fn restore_attempt_id(&self) -> RestoreAttemptId {
        self.restore_attempt_id
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn policy(&self) -> RestorePolicy {
        RestorePolicy::CleanOnly
    }

    pub const fn archive_profile(&self) -> ArchiveProfile {
        ArchiveProfile::ZstdL3W22
    }

    pub const fn recovery_grant_policy(&self) -> RecoveryGrantPolicy {
        RecoveryGrantPolicy::RequireFreshBootstrap
    }

    pub const fn limits(&self) -> PortabilityLimits {
        self.limits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreWorkspaceOutcome {
    restore_attempt_id: RestoreAttemptId,
    status: RestoreStatus,
    workspace_id: Option<WorkspaceId>,
    manifest_digest: Option<Sha256Digest>,
}

impl RestoreWorkspaceOutcome {
    pub const fn complete(
        restore_attempt_id: RestoreAttemptId,
        workspace_id: WorkspaceId,
        manifest_digest: Sha256Digest,
    ) -> Self {
        Self {
            restore_attempt_id,
            status: RestoreStatus::Complete,
            workspace_id: Some(workspace_id),
            manifest_digest: Some(manifest_digest),
        }
    }

    pub const fn rejected(restore_attempt_id: RestoreAttemptId) -> Self {
        Self {
            restore_attempt_id,
            status: RestoreStatus::Rejected,
            workspace_id: None,
            manifest_digest: None,
        }
    }

    pub const fn restore_attempt_id(&self) -> RestoreAttemptId {
        self.restore_attempt_id
    }

    pub const fn status(&self) -> RestoreStatus {
        self.status
    }

    pub const fn workspace_id(&self) -> Option<WorkspaceId> {
        self.workspace_id
    }

    pub const fn manifest_digest(&self) -> Option<&Sha256Digest> {
        self.manifest_digest.as_ref()
    }

    pub const fn recovery_grant_policy(&self) -> RecoveryGrantPolicy {
        RecoveryGrantPolicy::RequireFreshBootstrap
    }
}

pub trait WorkspaceRestorePort: Send + Sync {
    fn restore_workspace(
        &self,
        request: RestoreWorkspaceRequest,
        archive: Box<dyn Read + Send>,
    ) -> PortabilityResult<RestoreWorkspaceOutcome>;
}

/// Explicit profile selection for node-local authorization after clean restore.
///
/// The selected profile must already belong to the restored workspace. There
/// is no default-profile or first-profile behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrepareRecoveryBootstrapRequest {
    restore_attempt_id: RestoreAttemptId,
    correlation_id: RequestCorrelationId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
}

impl PrepareRecoveryBootstrapRequest {
    pub const fn new(
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
    ) -> Self {
        Self {
            restore_attempt_id,
            correlation_id,
            workspace_id,
            profile_id,
        }
    }

    pub const fn restore_attempt_id(&self) -> RestoreAttemptId {
        self.restore_attempt_id
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn recovery_grant_policy(&self) -> RecoveryGrantPolicy {
        RecoveryGrantPolicy::RequireFreshBootstrap
    }
}

/// Fresh provisional node-local identity for the existing restored profile.
///
/// A successful adapter creates this client and one-time proof after proving
/// the workspace/profile relation. The normal enrollment exchange replaces
/// the proof with a fresh credential and fresh grants. Imported credentials,
/// clients, grants, scopes, and node state are never reused.
pub struct PrepareRecoveryBootstrapOutcome {
    restore_attempt_id: RestoreAttemptId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: ClientId,
    initialization_proof: SecretMaterial,
}

impl PrepareRecoveryBootstrapOutcome {
    pub const fn new(
        restore_attempt_id: RestoreAttemptId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: ClientId,
        initialization_proof: SecretMaterial,
    ) -> Self {
        Self {
            restore_attempt_id,
            workspace_id,
            profile_id,
            client_id,
            initialization_proof,
        }
    }

    pub const fn restore_attempt_id(&self) -> RestoreAttemptId {
        self.restore_attempt_id
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub const fn initialization_proof(&self) -> &SecretMaterial {
        &self.initialization_proof
    }
}

/// Offline recovery-bootstrap step after verified clean activation.
pub trait RecoveryBootstrapPort: Send + Sync {
    fn prepare_recovery_bootstrap(
        &self,
        request: PrepareRecoveryBootstrapRequest,
    ) -> PortabilityResult<PrepareRecoveryBootstrapOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId};

    fn digest(byte: u8) -> Sha256Digest {
        Sha256Digest::parse(format!("sha256:{}", format!("{byte:02x}").repeat(32)))
            .expect("test digest")
    }

    fn limits() -> PortabilityLimits {
        let bounded = NonZeroU64::new(1).expect("non-zero test limit");
        PortabilityLimits {
            max_snapshot_bytes: bounded,
            max_wal_growth_bytes: bounded,
            max_archive_bytes: bounded,
            max_uncompressed_bytes: bounded,
            max_entry_bytes: bounded,
            max_entries: bounded,
            max_rows_per_stream: bounded,
            max_path_bytes: bounded,
            max_path_depth: bounded,
            max_decompression_ratio: bounded,
            scratch_ceiling_bytes: bounded,
            cleanup_reserve_bytes: bounded,
            backup_step_pages: bounded,
            backup_step_millis: bounded,
        }
    }

    fn streams() -> Vec<WorkspaceStreamDescriptor> {
        WorkspaceExportEntity::ALL
            .into_iter()
            .enumerate()
            .map(|(index, entity)| {
                WorkspaceStreamDescriptor::new(entity, index as u64, index as u64, digest(1))
            })
            .collect()
    }

    #[test]
    fn verification_query_derives_workspace_from_access_context() {
        let workspace_id = WorkspaceId::new_v7();
        let access = RequestAccessContext::new(
            workspace_id,
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let query = VerifyWorkspaceQuery::new(RequestCorrelationId::new_v7(), access);

        assert_eq!(query.access().workspace_id(), workspace_id);
    }

    #[test]
    fn export_entity_index_matches_declared_section_order() {
        // `index()` is `self as usize`, so a reordered or partially updated
        // `ALL` would silently attribute counts to the wrong entity. Nothing
        // else in the archive would look wrong.
        for (position, entity) in WorkspaceExportEntity::ALL.into_iter().enumerate() {
            assert_eq!(entity.index(), position, "{entity:?} index drifted");
        }
    }

    #[test]
    fn export_entity_section_names_are_unique_and_stable() {
        let mut names: Vec<&str> = WorkspaceExportEntity::ALL
            .into_iter()
            .map(WorkspaceExportEntity::as_str)
            .collect();
        let declared = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), declared, "duplicate export section name");
        assert!(names.iter().all(|name| !name.is_empty()));
    }

    #[test]
    fn export_entity_excludes_secret_and_authorization_bindings() {
        // These tables exist in the store and must never gain a section.
        for forbidden in [
            "credentials",
            "profile_grants",
            "grant_scopes",
            "node_state",
            "listener_configuration",
        ] {
            assert!(
                !WorkspaceExportEntity::ALL
                    .into_iter()
                    .any(|entity| entity.as_str() == forbidden),
                "{forbidden} must not be exported"
            );
        }
    }

    #[test]
    fn export_outcome_reports_counts_in_section_order() {
        let mut counts = [0u64; WorkspaceExportEntity::ALL.len()];
        counts[WorkspaceExportEntity::Observations.index()] = 7;
        counts[WorkspaceExportEntity::Corrections.index()] = 3;
        let outcome = WorkspaceExportOutcome::new(
            WorkspaceId::new_v7(),
            WORKSPACE_EXPORT_FORMAT_VERSION,
            counts,
            2048,
            "sha256:00".to_owned(),
        );

        assert_eq!(outcome.count(WorkspaceExportEntity::Observations), 7);
        assert_eq!(outcome.count(WorkspaceExportEntity::Corrections), 3);
        assert_eq!(outcome.count(WorkspaceExportEntity::Receipts), 0);
        assert_eq!(outcome.bytes_written(), 2048);
        let reported: Vec<_> = outcome.counts().map(|(entity, _)| entity).collect();
        assert_eq!(reported, WorkspaceExportEntity::ALL.to_vec());
    }

    #[test]
    fn manifest_requires_the_complete_ordered_stream_set() {
        let manifest = WorkspaceManifest::try_new(
            WorkspaceId::new_v7(),
            42,
            "1.0.0".to_owned(),
            2,
            digest(2),
            streams(),
            Vec::new(),
        )
        .expect("valid manifest");

        assert_eq!(manifest.export_scope(), ExportScope::FullWorkspace);
        assert_eq!(manifest.archive_profile(), ArchiveProfile::ZstdL3W22);
        assert_eq!(manifest.restore_policy(), RestorePolicy::CleanOnly);
        assert_eq!(manifest.workspace_revision(), 42);
        assert_eq!(
            manifest.recovery_grant_policy(),
            RecoveryGrantPolicy::RequireFreshBootstrap
        );

        let mut reordered = streams();
        reordered.swap(0, 1);
        assert_eq!(
            WorkspaceManifest::try_new(
                WorkspaceId::new_v7(),
                42,
                "1.0.0".to_owned(),
                2,
                digest(2),
                reordered,
                Vec::new(),
            ),
            Err(WorkspaceManifestError::IncompleteStreamSet)
        );
    }

    #[test]
    fn manifest_rejects_unbounded_contract_metadata() {
        assert_eq!(
            WorkspaceManifest::try_new(
                WorkspaceId::new_v7(),
                42,
                String::new(),
                2,
                digest(2),
                streams(),
                Vec::new(),
            ),
            Err(WorkspaceManifestError::EmptyContractVersion)
        );
        assert_eq!(
            WorkspaceManifest::try_new(
                WorkspaceId::new_v7(),
                42,
                "x".repeat(65),
                2,
                digest(2),
                streams(),
                Vec::new(),
            ),
            Err(WorkspaceManifestError::ContractVersionTooLong)
        );
        assert_eq!(
            WorkspaceManifest::try_new(
                WorkspaceId::new_v7(),
                MAX_PORTABLE_JSON_INTEGER + 1,
                "1.0.0".to_owned(),
                2,
                digest(2),
                streams(),
                Vec::new(),
            ),
            Err(WorkspaceManifestError::PortableIntegerOutOfRange)
        );
    }

    #[test]
    fn manifest_requires_canonical_unique_blob_identity_and_content() {
        let mut evidence_ids = [EvidenceId::new_v7(), EvidenceId::new_v7()];
        evidence_ids.sort_by_key(|evidence_id| evidence_id.uuid());
        let [first, second] = evidence_ids;

        let valid = vec![
            WorkspaceBlobDescriptor::new(first, 1, digest(4)),
            WorkspaceBlobDescriptor::new(second, 1, digest(5)),
        ];
        WorkspaceManifest::try_new(
            WorkspaceId::new_v7(),
            42,
            "1.0.0".to_owned(),
            2,
            digest(2),
            streams(),
            valid,
        )
        .expect("canonical blobs");

        let reversed = vec![
            WorkspaceBlobDescriptor::new(second, 1, digest(5)),
            WorkspaceBlobDescriptor::new(first, 1, digest(4)),
        ];
        assert_eq!(
            WorkspaceManifest::try_new(
                WorkspaceId::new_v7(),
                42,
                "1.0.0".to_owned(),
                2,
                digest(2),
                streams(),
                reversed,
            ),
            Err(WorkspaceManifestError::NonCanonicalBlobOrder)
        );

        let duplicate_id = vec![
            WorkspaceBlobDescriptor::new(first, 1, digest(4)),
            WorkspaceBlobDescriptor::new(first, 1, digest(5)),
        ];
        assert_eq!(
            WorkspaceManifest::try_new(
                WorkspaceId::new_v7(),
                42,
                "1.0.0".to_owned(),
                2,
                digest(2),
                streams(),
                duplicate_id,
            ),
            Err(WorkspaceManifestError::DuplicateEvidenceId)
        );

        let duplicate_digest = vec![
            WorkspaceBlobDescriptor::new(first, 1, digest(4)),
            WorkspaceBlobDescriptor::new(second, 1, digest(4)),
        ];
        assert_eq!(
            WorkspaceManifest::try_new(
                WorkspaceId::new_v7(),
                42,
                "1.0.0".to_owned(),
                2,
                digest(2),
                streams(),
                duplicate_digest,
            ),
            Err(WorkspaceManifestError::DuplicateBlobDigest)
        );
    }

    #[test]
    fn owned_requests_fix_archive_and_recovery_policy() {
        let workspace_id = WorkspaceId::new_v7();
        let access = RequestAccessContext::new(
            workspace_id,
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let export = ExportWorkspaceRequest::new(
            ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), access),
            limits(),
        );
        assert_eq!(export.scope(), ExportScope::FullWorkspace);
        assert_eq!(export.archive_profile(), ArchiveProfile::ZstdL3W22);

        let attempt_id = RestoreAttemptId::new_v7();
        let restore =
            RestoreWorkspaceRequest::new(attempt_id, RequestCorrelationId::new_v7(), limits());
        assert_eq!(restore.policy(), RestorePolicy::CleanOnly);
        assert_eq!(restore.archive_profile(), ArchiveProfile::ZstdL3W22);
        assert_eq!(
            restore.recovery_grant_policy(),
            RecoveryGrantPolicy::RequireFreshBootstrap
        );

        let complete = RestoreWorkspaceOutcome::complete(attempt_id, workspace_id, digest(3));
        assert_eq!(complete.status(), RestoreStatus::Complete);
        assert_eq!(complete.workspace_id(), Some(workspace_id));
        assert!(complete.manifest_digest().is_some());

        let rejected = RestoreWorkspaceOutcome::rejected(attempt_id);
        assert_eq!(rejected.status(), RestoreStatus::Rejected);
        assert_eq!(rejected.workspace_id(), None);
        assert_eq!(rejected.manifest_digest(), None);
    }

    #[test]
    fn failures_are_returned_outside_destination_with_typed_operation_identity() {
        let export_correlation = RequestCorrelationId::new_v7();
        let export = PortabilityFailureReceipt::try_online_export(Box::new(
            FastiProblem::export_canceled(export_correlation),
        ))
        .expect("export failure receipt");
        assert_eq!(export.operation().correlation_id(), export_correlation);
        assert_eq!(export.operation().restore_attempt_id(), None);
        assert_eq!(export.problem().code(), crate::ProblemCode::ExportCanceled);

        let stopped_node = PortabilityFailureReceipt::try_online_export(Box::new(
            FastiProblem::stopped_node_export_required(RequestCorrelationId::new_v7()),
        ))
        .expect("stopped-node export fallback receipt");
        assert_eq!(
            stopped_node.problem().code(),
            crate::ProblemCode::StoppedNodeExportRequired
        );

        let restore_correlation = RequestCorrelationId::new_v7();
        let attempt_id = RestoreAttemptId::new_v7();
        let restore = PortabilityFailureReceipt::try_clean_restore(
            attempt_id,
            Box::new(FastiProblem::restore_canceled(restore_correlation)),
        )
        .expect("restore failure receipt");
        assert_eq!(restore.operation().correlation_id(), restore_correlation);
        assert_eq!(restore.operation().restore_attempt_id(), Some(attempt_id));

        assert_eq!(
            PortabilityFailureReceipt::try_online_export(Box::new(FastiProblem::restore_canceled(
                RequestCorrelationId::new_v7()
            ),)),
            Err(PortabilityFailureReceiptError::CapabilityMismatch)
        );
    }

    #[test]
    fn recovery_bootstrap_requires_an_explicit_existing_profile() {
        let attempt_id = RestoreAttemptId::new_v7();
        let workspace_id = WorkspaceId::new_v7();
        let profile_id = ProfileId::new_v7();
        let request = PrepareRecoveryBootstrapRequest::new(
            attempt_id,
            RequestCorrelationId::new_v7(),
            workspace_id,
            profile_id,
        );
        assert_eq!(request.workspace_id(), workspace_id);
        assert_eq!(request.profile_id(), profile_id);
        assert_eq!(
            request.recovery_grant_policy(),
            RecoveryGrantPolicy::RequireFreshBootstrap
        );

        let client_id = ClientId::new_v7();
        let outcome = PrepareRecoveryBootstrapOutcome::new(
            attempt_id,
            workspace_id,
            profile_id,
            client_id,
            SecretMaterial::from_bytes([7; 32]),
        );
        assert_eq!(outcome.workspace_id(), workspace_id);
        assert_eq!(outcome.profile_id(), profile_id);
        assert_eq!(outcome.client_id(), client_id);
        assert_eq!(outcome.initialization_proof().expose_bytes(), &[7; 32]);
    }
}
