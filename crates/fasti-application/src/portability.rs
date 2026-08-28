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
use std::io::{self, Read, Seek, Write};
use std::num::NonZeroU64;
use std::sync::{
    atomic::{AtomicBool, Ordering as AtomicOrdering},
    Arc,
};

/// Frozen first archive format retained for restore compatibility.
pub const WORKSPACE_ARCHIVE_V1_FORMAT_VERSION: u32 = 1;

/// Internal staged archive format version written by the export adapter.
///
/// A restore implementation must reject any version it does not understand
/// rather than guessing at the framing. The archive-v1 stream inventory is
/// frozen, but this does not activate a public format, capability, or route.
pub const WORKSPACE_ARCHIVE_FORMAT_VERSION: u32 = 2;

/// The sole archive-v1 contract version understood by this executable.
///
/// Restore rejects other values during hostile manifest conversion. Schema
/// migration compatibility remains a separate import-pass decision.
pub const WORKSPACE_ARCHIVE_CONTRACT_VERSION: &str = "1.0.0";

/// Largest integer with one interoperable RFC 8785/I-JSON representation.
pub const MAX_PORTABLE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

/// Transitional name used by the staged NDJSON stream writer.
///
/// The stream is one component of the archive profile. It is not a complete
/// `.fasti` archive by itself.
pub const WORKSPACE_EXPORT_FORMAT_VERSION: u32 = WORKSPACE_ARCHIVE_FORMAT_VERSION;

/// Cloneable, monotonic cancellation shared by a caller and portability work.
///
/// Adapters poll this signal at bounded work boundaries. Cancellation never
/// authorizes a partial success: export aborts its destination before returning
/// `export_canceled`, and restore rejects or removes staging before returning
/// `operation_canceled`.
#[derive(Debug, Clone, Default)]
pub struct CancellationSignal {
    cancelled: Arc<AtomicBool>,
}

impl CancellationSignal {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, AtomicOrdering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(AtomicOrdering::SeqCst)
    }
}

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
    MetadataFieldClaims,
    MetadataFieldOverrides,
    ProfileRecordTrackingDispositions,
}

impl WorkspaceExportEntity {
    /// Every archive-v1 entity, in its frozen section order.
    pub const V1: [Self; 16] = [
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

    /// Every archive-v2 entity. The frozen v1 order is an exact prefix.
    ///
    /// Freezing these archive bytes does not activate the staged public
    /// export capability or any runtime route.
    pub const ALL: [Self; 19] = [
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
        Self::MetadataFieldClaims,
        Self::MetadataFieldOverrides,
        Self::ProfileRecordTrackingDispositions,
    ];

    pub const fn for_format(format_version: u32) -> Option<&'static [Self]> {
        match format_version {
            WORKSPACE_ARCHIVE_V1_FORMAT_VERSION => Some(&Self::V1),
            WORKSPACE_ARCHIVE_FORMAT_VERSION => Some(&Self::ALL),
            _ => None,
        }
    }

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
            Self::MetadataFieldClaims => "metadata_field_claims",
            Self::MetadataFieldOverrides => "metadata_field_overrides",
            Self::ProfileRecordTrackingDispositions => "profile_record_tracking_dispositions",
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

impl PortabilityLimits {
    /// Conservative ceiling for the expanded USTAR archive.
    ///
    /// Every admitted entry can add a 512-byte header and at most 511 bytes
    /// of padding. The archive then ends with two 512-byte zero records.
    pub fn archive_expanded_ceiling(self) -> Option<u64> {
        self.max_entries
            .get()
            .checked_mul(512 + 511)
            .and_then(|overhead| overhead.checked_add(1024))
            .and_then(|overhead| self.max_uncompressed_bytes.get().checked_add(overhead))
    }
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
    UnsupportedFormatVersion,
    EmptyContractVersion,
    ContractVersionTooLong,
    PortableIntegerOutOfRange,
    IncompleteStreamSet,
    NonCanonicalBlobOrder,
    DuplicateEvidenceId,
    DuplicateBlobDigest,
}

/// Full-workspace manifest body before contract-owned wire projection.
///
/// Canonical serialization and its checksum stay in `fasti-contracts`; the
/// application layer owns only the representation-independent manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceManifest {
    format_version: u32,
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
        Self::try_new_for_format(
            WORKSPACE_ARCHIVE_FORMAT_VERSION,
            workspace_id,
            workspace_revision,
            contract_version,
            migration_version,
            migration_digest,
            streams,
            blobs,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_new_for_format(
        format_version: u32,
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
        let Some(expected_streams) = WorkspaceExportEntity::for_format(format_version) else {
            return Err(WorkspaceManifestError::UnsupportedFormatVersion);
        };
        if streams.len() != expected_streams.len()
            || streams
                .iter()
                .map(WorkspaceStreamDescriptor::entity)
                .ne(expected_streams.iter().copied())
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
            format_version,
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
        self.format_version
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

#[derive(Debug, Clone)]
pub struct ExportWorkspaceRequest {
    query: ExportWorkspaceQuery,
    limits: PortabilityLimits,
    cancellation: CancellationSignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceExportMode {
    Online,
    StoppedNode,
}

/// Explicit offline export after the caller stops the daemon.
///
/// The stopped-node adapter owns the shared data-root lock, opens the stopped
/// database, and applies the same grant rules from [`ExportWorkspaceQuery`].
/// Workspace identity is derived from that access context; callers cannot
/// supply a second workspace value.
#[derive(Debug, Clone)]
pub struct StoppedNodeExportRequest {
    query: ExportWorkspaceQuery,
    limits: PortabilityLimits,
    cancellation: CancellationSignal,
}

impl StoppedNodeExportRequest {
    pub fn new(
        query: ExportWorkspaceQuery,
        limits: PortabilityLimits,
        cancellation: CancellationSignal,
    ) -> Self {
        Self {
            query,
            limits,
            cancellation,
        }
    }

    pub const fn query(&self) -> &ExportWorkspaceQuery {
        &self.query
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.query.access().workspace_id()
    }

    pub const fn mode(&self) -> WorkspaceExportMode {
        WorkspaceExportMode::StoppedNode
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

    pub const fn cancellation(&self) -> &CancellationSignal {
        &self.cancellation
    }
}

/// Operation identity retained when a portability failure is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortabilityFailureOperation {
    OnlineExport {
        correlation_id: RequestCorrelationId,
        workspace_id: WorkspaceId,
    },
    StoppedNodeExport {
        correlation_id: RequestCorrelationId,
        workspace_id: WorkspaceId,
    },
    CleanRestore {
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
    },
    RecoveryBootstrapPrepare {
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
    },
    RecoveryBootstrapComplete {
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: ClientId,
    },
}

impl PortabilityFailureOperation {
    pub const fn correlation_id(self) -> RequestCorrelationId {
        match self {
            Self::OnlineExport { correlation_id, .. }
            | Self::StoppedNodeExport { correlation_id, .. }
            | Self::CleanRestore { correlation_id, .. }
            | Self::RecoveryBootstrapPrepare { correlation_id, .. }
            | Self::RecoveryBootstrapComplete { correlation_id, .. } => correlation_id,
        }
    }

    pub const fn restore_attempt_id(self) -> Option<RestoreAttemptId> {
        match self {
            Self::OnlineExport { .. } | Self::StoppedNodeExport { .. } => None,
            Self::CleanRestore {
                restore_attempt_id, ..
            }
            | Self::RecoveryBootstrapPrepare {
                restore_attempt_id, ..
            }
            | Self::RecoveryBootstrapComplete {
                restore_attempt_id, ..
            } => Some(restore_attempt_id),
        }
    }

    pub const fn workspace_id(self) -> Option<WorkspaceId> {
        match self {
            Self::OnlineExport { workspace_id, .. }
            | Self::StoppedNodeExport { workspace_id, .. }
            | Self::RecoveryBootstrapPrepare { workspace_id, .. }
            | Self::RecoveryBootstrapComplete { workspace_id, .. } => Some(workspace_id),
            Self::CleanRestore { .. } => None,
        }
    }

    pub const fn profile_id(self) -> Option<ProfileId> {
        match self {
            Self::RecoveryBootstrapPrepare { profile_id, .. }
            | Self::RecoveryBootstrapComplete { profile_id, .. } => Some(profile_id),
            Self::OnlineExport { .. }
            | Self::StoppedNodeExport { .. }
            | Self::CleanRestore { .. } => None,
        }
    }

    pub const fn client_id(self) -> Option<ClientId> {
        match self {
            Self::RecoveryBootstrapComplete { client_id, .. } => Some(client_id),
            Self::OnlineExport { .. }
            | Self::StoppedNodeExport { .. }
            | Self::CleanRestore { .. }
            | Self::RecoveryBootstrapPrepare { .. } => None,
        }
    }

    pub const fn export_mode(self) -> Option<WorkspaceExportMode> {
        match self {
            Self::OnlineExport { .. } => Some(WorkspaceExportMode::Online),
            Self::StoppedNodeExport { .. } => Some(WorkspaceExportMode::StoppedNode),
            Self::CleanRestore { .. }
            | Self::RecoveryBootstrapPrepare { .. }
            | Self::RecoveryBootstrapComplete { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortabilityFailureReceiptError {
    CapabilityMismatch,
    OperationProblemMismatch,
}

/// State of the caller-owned archive destination after an export failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailedArchiveDestinationState {
    /// No archive remains available at the requested destination.
    Discarded,
    /// Pre-publication cleanup failed. The caller must inspect the destination
    /// before choosing a recovery action.
    PartialCleanupIndeterminate,
    /// A complete, digest-verified archive was linked, but directory durability
    /// could not be confirmed. The caller must inspect the destination before
    /// choosing a recovery action.
    PublishedDurabilityIndeterminate,
}

/// Typed failure returned after the destination has left the caller's control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortabilityFailureReceipt {
    operation: PortabilityFailureOperation,
    problem: Box<FastiProblem>,
    archive_destination_state: Option<FailedArchiveDestinationState>,
}

fn validate_failure_problem(
    problem: &FastiProblem,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    operation_allows_code: bool,
) -> Result<(), PortabilityFailureReceiptError> {
    if problem.capability() != capability {
        return Err(PortabilityFailureReceiptError::CapabilityMismatch);
    }
    if problem.correlation_id() != correlation_id || !operation_allows_code {
        return Err(PortabilityFailureReceiptError::OperationProblemMismatch);
    }
    Ok(())
}

impl PortabilityFailureReceipt {
    pub fn try_online_export(
        request: &ExportWorkspaceRequest,
        problem: Box<FastiProblem>,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        Self::try_online_export_with_destination_state(
            request,
            problem,
            FailedArchiveDestinationState::Discarded,
        )
    }

    pub fn try_online_export_with_destination_state(
        request: &ExportWorkspaceRequest,
        problem: Box<FastiProblem>,
        archive_destination_state: FailedArchiveDestinationState,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        let destination_state_is_valid = archive_destination_state
            == FailedArchiveDestinationState::Discarded
            || problem.code() == crate::ProblemCode::StorageUnavailable;
        validate_failure_problem(
            &problem,
            CapabilityKey::ExportWorkspace,
            request.query().correlation_id(),
            destination_state_is_valid
                && matches!(
                    problem.code(),
                    crate::ProblemCode::CapabilityUnavailable
                        | crate::ProblemCode::Forbidden
                        | crate::ProblemCode::CapacityExceeded
                        | crate::ProblemCode::ExportCanceled
                        | crate::ProblemCode::IntegrityFailed
                        | crate::ProblemCode::StoppedNodeExportRequired
                        | crate::ProblemCode::StorageUnavailable
                        | crate::ProblemCode::UnsupportedPlatform
                ),
        )?;
        let operation = PortabilityFailureOperation::OnlineExport {
            correlation_id: request.query().correlation_id(),
            workspace_id: request.query().access().workspace_id(),
        };
        Ok(Self {
            operation,
            problem,
            archive_destination_state: Some(archive_destination_state),
        })
    }

    pub fn try_clean_restore(
        request: &RestoreWorkspaceRequest,
        problem: Box<FastiProblem>,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        validate_failure_problem(
            &problem,
            CapabilityKey::RestoreWorkspace,
            request.correlation_id(),
            matches!(
                problem.code(),
                crate::ProblemCode::CapabilityUnavailable
                    | crate::ProblemCode::Forbidden
                    | crate::ProblemCode::ValidationFailed
                    | crate::ProblemCode::CapacityExceeded
                    | crate::ProblemCode::DataRootLocked
                    | crate::ProblemCode::IntegrityFailed
                    | crate::ProblemCode::OperationCanceled
                    | crate::ProblemCode::StorageUnavailable
                    | crate::ProblemCode::UnsupportedPlatform
            ),
        )?;
        let operation = PortabilityFailureOperation::CleanRestore {
            restore_attempt_id: request.restore_attempt_id(),
            correlation_id: request.correlation_id(),
        };
        Ok(Self {
            operation,
            problem,
            archive_destination_state: None,
        })
    }

    pub fn try_stopped_node_export(
        request: &StoppedNodeExportRequest,
        problem: Box<FastiProblem>,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        Self::try_stopped_node_export_with_destination_state(
            request,
            problem,
            FailedArchiveDestinationState::Discarded,
        )
    }

    pub fn try_stopped_node_export_with_destination_state(
        request: &StoppedNodeExportRequest,
        problem: Box<FastiProblem>,
        archive_destination_state: FailedArchiveDestinationState,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        let destination_state_is_valid = archive_destination_state
            == FailedArchiveDestinationState::Discarded
            || problem.code() == crate::ProblemCode::StorageUnavailable;
        validate_failure_problem(
            &problem,
            CapabilityKey::ExportWorkspace,
            request.query().correlation_id(),
            destination_state_is_valid
                && matches!(
                    problem.code(),
                    crate::ProblemCode::CapabilityUnavailable
                        | crate::ProblemCode::Forbidden
                        | crate::ProblemCode::CapacityExceeded
                        | crate::ProblemCode::DataRootLocked
                        | crate::ProblemCode::ExportCanceled
                        | crate::ProblemCode::IntegrityFailed
                        | crate::ProblemCode::StorageUnavailable
                        | crate::ProblemCode::UnsupportedPlatform
                ),
        )?;
        let operation = PortabilityFailureOperation::StoppedNodeExport {
            correlation_id: request.query().correlation_id(),
            workspace_id: request.workspace_id(),
        };
        Ok(Self {
            operation,
            problem,
            archive_destination_state: Some(archive_destination_state),
        })
    }

    pub fn try_recovery_bootstrap_prepare(
        request: &PrepareRecoveryBootstrapRequest,
        problem: Box<FastiProblem>,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        validate_failure_problem(
            &problem,
            CapabilityKey::RestoreWorkspace,
            request.correlation_id(),
            matches!(
                problem.code(),
                crate::ProblemCode::CapabilityUnavailable
                    | crate::ProblemCode::DataRootLocked
                    | crate::ProblemCode::Forbidden
                    | crate::ProblemCode::IntegrityFailed
                    | crate::ProblemCode::RecoveryBootstrapPending
                    | crate::ProblemCode::StorageUnavailable
                    | crate::ProblemCode::UnsupportedPlatform
                    | crate::ProblemCode::ValidationFailed
            ),
        )?;
        let operation = PortabilityFailureOperation::RecoveryBootstrapPrepare {
            restore_attempt_id: request.restore_attempt_id(),
            correlation_id: request.correlation_id(),
            workspace_id: request.workspace_id(),
            profile_id: request.profile_id(),
        };
        Ok(Self {
            operation,
            problem,
            archive_destination_state: None,
        })
    }

    pub fn try_recovery_bootstrap_complete(
        request: &CompleteRecoveryBootstrapRequest,
        problem: Box<FastiProblem>,
    ) -> Result<Self, PortabilityFailureReceiptError> {
        validate_failure_problem(
            &problem,
            CapabilityKey::RestoreWorkspace,
            request.correlation_id(),
            matches!(
                problem.code(),
                crate::ProblemCode::CapabilityUnavailable
                    | crate::ProblemCode::BootstrapClosed
                    | crate::ProblemCode::DataRootLocked
                    | crate::ProblemCode::Forbidden
                    | crate::ProblemCode::IntegrityFailed
                    | crate::ProblemCode::StorageUnavailable
                    | crate::ProblemCode::UnsupportedPlatform
                    | crate::ProblemCode::ValidationFailed
            ),
        )?;
        let operation = PortabilityFailureOperation::RecoveryBootstrapComplete {
            restore_attempt_id: request.restore_attempt_id(),
            correlation_id: request.correlation_id(),
            workspace_id: request.workspace_id(),
            profile_id: request.profile_id(),
            client_id: request.client_id(),
        };
        Ok(Self {
            operation,
            problem,
            archive_destination_state: None,
        })
    }

    pub const fn operation(&self) -> PortabilityFailureOperation {
        self.operation
    }

    pub fn problem(&self) -> &FastiProblem {
        &self.problem
    }

    pub const fn archive_destination_state(&self) -> Option<FailedArchiveDestinationState> {
        self.archive_destination_state
    }
}

pub type PortabilityResult<T> = Result<T, PortabilityFailureReceipt>;

#[derive(Debug)]
pub enum WorkspaceArchiveCompletionError {
    Discarded(io::Error),
    PartialCleanupIndeterminate(io::Error),
    PublishedDurabilityIndeterminate(io::Error),
}

impl WorkspaceArchiveCompletionError {
    pub const fn destination_state(&self) -> FailedArchiveDestinationState {
        match self {
            Self::Discarded(_) => FailedArchiveDestinationState::Discarded,
            Self::PartialCleanupIndeterminate(_) => {
                FailedArchiveDestinationState::PartialCleanupIndeterminate
            }
            Self::PublishedDurabilityIndeterminate(_) => {
                FailedArchiveDestinationState::PublishedDurabilityIndeterminate
            }
        }
    }

    pub fn kind(&self) -> io::ErrorKind {
        match self {
            Self::Discarded(error)
            | Self::PartialCleanupIndeterminate(error)
            | Self::PublishedDurabilityIndeterminate(error) => error.kind(),
        }
    }
}

impl From<io::Error> for WorkspaceArchiveCompletionError {
    fn from(error: io::Error) -> Self {
        Self::Discarded(error)
    }
}

impl std::fmt::Display for WorkspaceArchiveCompletionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Discarded(error)
            | Self::PartialCleanupIndeterminate(error)
            | Self::PublishedDurabilityIndeterminate(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for WorkspaceArchiveCompletionError {}

impl ExportWorkspaceRequest {
    pub fn new(
        query: ExportWorkspaceQuery,
        limits: PortabilityLimits,
        cancellation: CancellationSignal,
    ) -> Self {
        Self {
            query,
            limits,
            cancellation,
        }
    }

    pub const fn query(&self) -> &ExportWorkspaceQuery {
        &self.query
    }

    pub const fn mode(&self) -> WorkspaceExportMode {
        WorkspaceExportMode::Online
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

    pub const fn cancellation(&self) -> &CancellationSignal {
        &self.cancellation
    }
}

/// Owned partial-archive lifecycle.
///
/// Consuming completion and abort methods ensure the caller cannot reuse a
/// published or discarded destination. Implementations own synchronization,
/// no-replace publication, and partial-file removal.
pub trait WorkspaceArchiveDestination: Write + Send {
    /// Check capacity on the destination filesystem before any source or
    /// snapshot mutation.
    ///
    /// The caller supplies a conservative no-compression output bound: stream
    /// bytes, referenced blob bytes, container and final-manifest overhead, and
    /// the applicable reserve. It must not credit expected compression. The
    /// destination owns this check because it alone knows which filesystem
    /// will hold partial and completed archive bytes.
    fn preflight(&self, required_bytes: u64) -> std::io::Result<()>;

    /// Publish the completed artifact atomically.
    ///
    /// This method consumes the destination. A pre-publication failure must
    /// return [`WorkspaceArchiveCompletionError::Discarded`] after making the
    /// partial artifact unavailable. A cleanup failure must return
    /// [`WorkspaceArchiveCompletionError::PartialCleanupIndeterminate`]. If the
    /// verified artifact was linked but its directory could not be synchronized, return
    /// [`WorkspaceArchiveCompletionError::PublishedDurabilityIndeterminate`]
    /// without attempting a racy rollback.
    fn complete(
        self: Box<Self>,
        archive_digest: &Sha256Digest,
        manifest_digest: &Sha256Digest,
    ) -> Result<(), WorkspaceArchiveCompletionError>;

    fn abort(self: Box<Self>) -> std::io::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceArchiveExportOutcome {
    workspace_id: WorkspaceId,
    workspace_revision: u64,
    manifest_digest: Sha256Digest,
    archive_bytes: u64,
    archive_digest: Sha256Digest,
}

impl WorkspaceArchiveExportOutcome {
    pub const fn new(
        workspace_id: WorkspaceId,
        workspace_revision: u64,
        manifest_digest: Sha256Digest,
        archive_bytes: u64,
        archive_digest: Sha256Digest,
    ) -> Self {
        Self {
            workspace_id,
            workspace_revision,
            manifest_digest,
            archive_bytes,
            archive_digest,
        }
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn workspace_revision(&self) -> u64 {
        self.workspace_revision
    }

    pub const fn manifest_digest(&self) -> &Sha256Digest {
        &self.manifest_digest
    }

    pub const fn archive_bytes(&self) -> u64 {
        self.archive_bytes
    }

    pub const fn archive_digest(&self) -> &Sha256Digest {
        &self.archive_digest
    }
}

/// Complete `.fasti` export boundary above the staged entity-stream writer.
pub trait WorkspaceArchiveExportPort: Send + Sync {
    fn export_workspace_archive(
        &self,
        request: ExportWorkspaceRequest,
        destination: Box<dyn WorkspaceArchiveDestination>,
    ) -> PortabilityResult<WorkspaceArchiveExportOutcome>;

    fn export_stopped_node_workspace_archive(
        &self,
        request: StoppedNodeExportRequest,
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

#[derive(Debug, Clone)]
pub struct RestoreWorkspaceRequest {
    restore_attempt_id: RestoreAttemptId,
    correlation_id: RequestCorrelationId,
    limits: PortabilityLimits,
    cancellation: CancellationSignal,
}

impl RestoreWorkspaceRequest {
    pub fn new(
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
        limits: PortabilityLimits,
        cancellation: CancellationSignal,
    ) -> Self {
        Self {
            restore_attempt_id,
            correlation_id,
            limits,
            cancellation,
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

    pub const fn cancellation(&self) -> &CancellationSignal {
        &self.cancellation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreWorkspaceOutcome {
    restore_attempt_id: RestoreAttemptId,
    workspace_id: WorkspaceId,
    manifest_digest: Sha256Digest,
}

impl RestoreWorkspaceOutcome {
    pub const fn complete(
        restore_attempt_id: RestoreAttemptId,
        workspace_id: WorkspaceId,
        manifest_digest: Sha256Digest,
    ) -> Self {
        Self {
            restore_attempt_id,
            workspace_id,
            manifest_digest,
        }
    }

    pub const fn restore_attempt_id(&self) -> RestoreAttemptId {
        self.restore_attempt_id
    }

    pub const fn status(&self) -> RestoreStatus {
        RestoreStatus::Complete
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn manifest_digest(&self) -> &Sha256Digest {
        &self.manifest_digest
    }

    pub const fn recovery_grant_policy(&self) -> RecoveryGrantPolicy {
        RecoveryGrantPolicy::RequireFreshBootstrap
    }
}

/// One opened archive source that supports preflight and restore passes.
///
/// Restore adapters must preflight format, paths, headers, the final manifest,
/// checksums, references, and bounds before destination mutation. They then
/// rewind this same opened source for the restore pass. Reopening by path would
/// create a time-of-check/time-of-use race; buffering the whole archive would
/// violate the bounded-memory contract.
pub trait ReadSeek: Read + Seek {}

impl<T: Read + Seek + ?Sized> ReadSeek for T {}

pub trait WorkspaceRestorePort: Send + Sync {
    fn restore_workspace(
        &self,
        request: RestoreWorkspaceRequest,
        archive: Box<dyn ReadSeek + Send>,
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
    replace_pending: bool,
}

impl PrepareRecoveryBootstrapRequest {
    pub const fn new(
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        replace_pending: bool,
    ) -> Self {
        Self {
            restore_attempt_id,
            correlation_id,
            workspace_id,
            profile_id,
            replace_pending,
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

    pub const fn replace_pending(&self) -> bool {
        self.replace_pending
    }

    pub const fn recovery_grant_policy(&self) -> RecoveryGrantPolicy {
        RecoveryGrantPolicy::RequireFreshBootstrap
    }
}

/// Fresh provisional node-local identity for the existing restored profile.
///
/// Non-secret imported client provenance remains required for audit and
/// Chronicle references. A successful adapter creates a distinct node-local
/// recovery client and one-time proof after proving the restore/workspace/profile
/// relation. Completion consumes that proof and an already-owned final
/// credential supplied by the caller. Imported client authentication,
/// credentials, grants, scopes, and node state are never reused.
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

/// Caller-owned proof and final credential for atomic recovery completion.
///
/// The final credential exists before the transaction starts. A lost response
/// can therefore retry the same proof/credential pair without the store
/// generating a second secret after commit.
pub struct CompleteRecoveryBootstrapRequest {
    restore_attempt_id: RestoreAttemptId,
    correlation_id: RequestCorrelationId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: ClientId,
    initialization_proof: SecretMaterial,
    credential: SecretMaterial,
}

impl CompleteRecoveryBootstrapRequest {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        restore_attempt_id: RestoreAttemptId,
        correlation_id: RequestCorrelationId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: ClientId,
        initialization_proof: SecretMaterial,
        credential: SecretMaterial,
    ) -> Self {
        Self {
            restore_attempt_id,
            correlation_id,
            workspace_id,
            profile_id,
            client_id,
            initialization_proof,
            credential,
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

    pub const fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub const fn initialization_proof(&self) -> &SecretMaterial {
        &self.initialization_proof
    }

    pub const fn credential(&self) -> &SecretMaterial {
        &self.credential
    }
}

/// Non-secret metadata for one committed recovery-bootstrap completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompleteRecoveryBootstrapOutcome {
    restore_attempt_id: RestoreAttemptId,
    access: RequestAccessContext,
}

impl CompleteRecoveryBootstrapOutcome {
    pub const fn new(restore_attempt_id: RestoreAttemptId, access: RequestAccessContext) -> Self {
        Self {
            restore_attempt_id,
            access,
        }
    }

    pub const fn restore_attempt_id(&self) -> RestoreAttemptId {
        self.restore_attempt_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }
}

/// Offline recovery-bootstrap step after verified clean activation.
pub trait RecoveryBootstrapPort: Send + Sync {
    fn prepare_recovery_bootstrap(
        &self,
        request: PrepareRecoveryBootstrapRequest,
    ) -> PortabilityResult<PrepareRecoveryBootstrapOutcome>;

    fn complete_recovery_bootstrap(
        &self,
        request: CompleteRecoveryBootstrapRequest,
    ) -> PortabilityResult<CompleteRecoveryBootstrapOutcome>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId};
    use std::io::{Cursor, SeekFrom};

    struct BoundedTestDestination {
        capacity_bytes: u64,
    }

    impl Write for BoundedTestDestination {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl WorkspaceArchiveDestination for BoundedTestDestination {
        fn preflight(&self, required_bytes: u64) -> std::io::Result<()> {
            if required_bytes > self.capacity_bytes {
                return Err(std::io::Error::other(
                    "archive destination capacity is insufficient",
                ));
            }
            Ok(())
        }

        fn complete(
            self: Box<Self>,
            _archive_digest: &Sha256Digest,
            _manifest_digest: &Sha256Digest,
        ) -> Result<(), WorkspaceArchiveCompletionError> {
            Ok(())
        }

        fn abort(self: Box<Self>) -> std::io::Result<()> {
            Ok(())
        }
    }

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
    fn archive_expanded_ceiling_accounts_for_headers_padding_and_trailer() {
        assert_eq!(limits().archive_expanded_ceiling(), Some(1 + 1023 + 1024));

        let mut overflowing = limits();
        overflowing.max_uncompressed_bytes =
            NonZeroU64::new(u64::MAX).expect("maximum is non-zero");
        assert_eq!(overflowing.archive_expanded_ceiling(), None);
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
        let export_cancellation = CancellationSignal::new();
        let export = ExportWorkspaceRequest::new(
            ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), access),
            limits(),
            export_cancellation.clone(),
        );
        assert_eq!(export.scope(), ExportScope::FullWorkspace);
        assert_eq!(export.archive_profile(), ArchiveProfile::ZstdL3W22);
        assert_eq!(export.mode(), WorkspaceExportMode::Online);
        assert!(!export.cancellation().is_cancelled());
        export_cancellation.cancel();
        assert!(export.cancellation().is_cancelled());

        let stopped_cancellation = CancellationSignal::new();
        let stopped = StoppedNodeExportRequest::new(
            ExportWorkspaceQuery::new(RequestCorrelationId::new_v7(), access),
            limits(),
            stopped_cancellation.clone(),
        );
        assert_eq!(stopped.workspace_id(), workspace_id);
        assert_eq!(
            stopped.workspace_id(),
            stopped.query().access().workspace_id()
        );
        assert_eq!(stopped.mode(), WorkspaceExportMode::StoppedNode);
        assert_eq!(stopped.scope(), ExportScope::FullWorkspace);
        assert_eq!(stopped.archive_profile(), ArchiveProfile::ZstdL3W22);
        stopped_cancellation.cancel();
        assert!(stopped.cancellation().is_cancelled());

        let attempt_id = RestoreAttemptId::new_v7();
        let restore_cancellation = CancellationSignal::new();
        let restore = RestoreWorkspaceRequest::new(
            attempt_id,
            RequestCorrelationId::new_v7(),
            limits(),
            restore_cancellation.clone(),
        );
        assert_eq!(restore.policy(), RestorePolicy::CleanOnly);
        assert_eq!(restore.archive_profile(), ArchiveProfile::ZstdL3W22);
        assert_eq!(
            restore.recovery_grant_policy(),
            RecoveryGrantPolicy::RequireFreshBootstrap
        );
        assert!(!restore.cancellation().is_cancelled());
        restore_cancellation.cancel();
        assert!(restore.cancellation().is_cancelled());

        let complete = RestoreWorkspaceOutcome::complete(attempt_id, workspace_id, digest(3));
        assert_eq!(complete.status(), RestoreStatus::Complete);
        assert_eq!(complete.workspace_id(), workspace_id);
        assert_eq!(complete.manifest_digest(), &digest(3));

        let archive =
            WorkspaceArchiveExportOutcome::new(workspace_id, 42, digest(4), 2_048, digest(5));
        assert_eq!(archive.workspace_id(), workspace_id);
        assert_eq!(archive.workspace_revision(), 42);
        assert_eq!(archive.manifest_digest(), &digest(4));
        assert_eq!(archive.archive_bytes(), 2_048);
        assert_eq!(archive.archive_digest(), &digest(5));
    }

    #[test]
    fn archive_destination_owns_capacity_preflight() {
        let destination = BoundedTestDestination {
            capacity_bytes: 1024,
        };

        assert!(destination.preflight(1024).is_ok());
        assert!(destination.preflight(1025).is_err());
    }

    #[test]
    fn restore_source_supports_two_pass_preflight_without_reopening() {
        let mut archive: Box<dyn ReadSeek + Send> =
            Box::new(Cursor::new(b"header-payload-manifest".to_vec()));
        let mut preflight = Vec::new();
        archive
            .read_to_end(&mut preflight)
            .expect("preflight reads the complete opened archive");
        archive
            .seek(SeekFrom::Start(0))
            .expect("same archive rewinds after preflight");
        let mut restore = Vec::new();
        archive
            .read_to_end(&mut restore)
            .expect("restore rereads the same opened archive");

        assert_eq!(preflight, restore);
    }

    #[test]
    fn failures_are_returned_outside_destination_with_typed_operation_identity() {
        let workspace_id = WorkspaceId::new_v7();
        let profile_id = ProfileId::new_v7();
        let access = RequestAccessContext::new(
            workspace_id,
            profile_id,
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let export_correlation = RequestCorrelationId::new_v7();
        let export_request = ExportWorkspaceRequest::new(
            ExportWorkspaceQuery::new(export_correlation, access),
            limits(),
            CancellationSignal::new(),
        );
        let export = PortabilityFailureReceipt::try_online_export(
            &export_request,
            Box::new(FastiProblem::export_canceled(export_correlation)),
        )
        .expect("export failure receipt");
        assert_eq!(export.operation().correlation_id(), export_correlation);
        assert_eq!(export.operation().restore_attempt_id(), None);
        assert_eq!(export.operation().workspace_id(), Some(workspace_id));
        assert_eq!(export.operation().profile_id(), None);
        assert_eq!(
            export.operation().export_mode(),
            Some(WorkspaceExportMode::Online)
        );
        assert_eq!(export.problem().code(), crate::ProblemCode::ExportCanceled);
        assert_eq!(
            export.archive_destination_state(),
            Some(FailedArchiveDestinationState::Discarded)
        );

        let indeterminate = PortabilityFailureReceipt::try_online_export_with_destination_state(
            &export_request,
            Box::new(FastiProblem::storage_unavailable(
                CapabilityKey::ExportWorkspace,
                export_correlation,
            )),
            FailedArchiveDestinationState::PublishedDurabilityIndeterminate,
        )
        .expect("storage failure can report indeterminate publication durability");
        assert_eq!(
            indeterminate.archive_destination_state(),
            Some(FailedArchiveDestinationState::PublishedDurabilityIndeterminate)
        );
        assert_eq!(
            PortabilityFailureReceipt::try_online_export_with_destination_state(
                &export_request,
                Box::new(FastiProblem::export_canceled(export_correlation)),
                FailedArchiveDestinationState::PublishedDurabilityIndeterminate,
            ),
            Err(PortabilityFailureReceiptError::OperationProblemMismatch)
        );

        let online_unsupported = PortabilityFailureReceipt::try_online_export(
            &export_request,
            Box::new(FastiProblem::unsupported_platform(
                CapabilityKey::ExportWorkspace,
                export_correlation,
            )),
        )
        .expect("online export can fail closed on an unsupported platform");
        assert_eq!(
            online_unsupported.problem().code(),
            crate::ProblemCode::UnsupportedPlatform
        );

        let stopped_node = PortabilityFailureReceipt::try_online_export(
            &export_request,
            Box::new(FastiProblem::stopped_node_export_required(
                export_correlation,
            )),
        )
        .expect("stopped-node export fallback receipt");
        assert_eq!(
            stopped_node.problem().code(),
            crate::ProblemCode::StoppedNodeExportRequired
        );
        assert_eq!(
            stopped_node.operation().export_mode(),
            Some(WorkspaceExportMode::Online),
            "the online operation reports the stopped-node next action"
        );

        let stopped_correlation = RequestCorrelationId::new_v7();
        let stopped_request = StoppedNodeExportRequest::new(
            ExportWorkspaceQuery::new(stopped_correlation, access),
            limits(),
            CancellationSignal::new(),
        );
        let stopped_failure = PortabilityFailureReceipt::try_stopped_node_export(
            &stopped_request,
            Box::new(FastiProblem::data_root_locked(
                CapabilityKey::ExportWorkspace,
                stopped_correlation,
            )),
        )
        .expect("stopped-node failure receipt");
        assert_eq!(
            stopped_failure.operation().export_mode(),
            Some(WorkspaceExportMode::StoppedNode)
        );
        assert_eq!(
            stopped_failure.operation().workspace_id(),
            Some(workspace_id)
        );
        let stopped_unsupported = PortabilityFailureReceipt::try_stopped_node_export(
            &stopped_request,
            Box::new(FastiProblem::unsupported_platform(
                CapabilityKey::ExportWorkspace,
                stopped_correlation,
            )),
        )
        .expect("stopped-node export can fail closed on an unsupported platform");
        assert_eq!(
            stopped_unsupported.problem().code(),
            crate::ProblemCode::UnsupportedPlatform
        );
        let stopped_canceled = PortabilityFailureReceipt::try_stopped_node_export(
            &stopped_request,
            Box::new(FastiProblem::export_canceled(stopped_correlation)),
        )
        .expect("mode-neutral cancellation applies to stopped-node export");
        let canceled_detail = stopped_canceled
            .problem()
            .contract()
            .detail(CapabilityKey::ExportWorkspace);
        assert!(canceled_detail.contains("workspace export"));
        assert!(!canceled_detail.contains("online export"));

        let restore_correlation = RequestCorrelationId::new_v7();
        let attempt_id = RestoreAttemptId::new_v7();
        let restore_request = RestoreWorkspaceRequest::new(
            attempt_id,
            restore_correlation,
            limits(),
            CancellationSignal::new(),
        );
        let restore = PortabilityFailureReceipt::try_clean_restore(
            &restore_request,
            Box::new(FastiProblem::restore_canceled(restore_correlation)),
        )
        .expect("restore failure receipt");
        assert_eq!(restore.operation().correlation_id(), restore_correlation);
        assert_eq!(restore.operation().restore_attempt_id(), Some(attempt_id));
        assert_eq!(restore.archive_destination_state(), None);

        let bootstrap_correlation = RequestCorrelationId::new_v7();
        let bootstrap_request = PrepareRecoveryBootstrapRequest::new(
            attempt_id,
            bootstrap_correlation,
            workspace_id,
            profile_id,
            false,
        );
        let bootstrap = PortabilityFailureReceipt::try_recovery_bootstrap_prepare(
            &bootstrap_request,
            Box::new(FastiProblem::recovery_bootstrap_pending(
                bootstrap_correlation,
            )),
        )
        .expect("recovery-bootstrap failure receipt");
        assert!(matches!(
            bootstrap.operation(),
            PortabilityFailureOperation::RecoveryBootstrapPrepare { .. }
        ));
        assert_eq!(bootstrap.operation().restore_attempt_id(), Some(attempt_id));
        assert_eq!(bootstrap.operation().workspace_id(), Some(workspace_id));
        assert_eq!(bootstrap.operation().profile_id(), Some(profile_id));
        assert_eq!(bootstrap.operation().client_id(), None);
        assert_eq!(
            bootstrap.problem().safe_state(),
            crate::SafeState::RestoredDataActiveBootstrapPending
        );
        assert_eq!(
            bootstrap.problem().next_actions()[0].id(),
            "retry_recovery_bootstrap"
        );
        assert_eq!(
            PortabilityFailureReceipt::try_recovery_bootstrap_prepare(
                &bootstrap_request,
                Box::new(FastiProblem::restore_canceled(bootstrap_correlation))
            ),
            Err(PortabilityFailureReceiptError::OperationProblemMismatch),
            "a pre-activation cancellation cannot be mislabeled as recovery bootstrap"
        );
        for code in [
            crate::ProblemCode::DataRootLocked,
            crate::ProblemCode::UnsupportedPlatform,
            crate::ProblemCode::ValidationFailed,
        ] {
            let receipt = PortabilityFailureReceipt::try_recovery_bootstrap_prepare(
                &bootstrap_request,
                Box::new(FastiProblem::from_code(
                    code,
                    CapabilityKey::RestoreWorkspace,
                    bootstrap_correlation,
                )),
            )
            .expect("prepare owns offline prerequisites and identity validation");
            assert_eq!(receipt.problem().code(), code);
        }

        let recovery_client_id = ClientId::new_v7();
        let complete_request = CompleteRecoveryBootstrapRequest::new(
            attempt_id,
            bootstrap_correlation,
            workspace_id,
            profile_id,
            recovery_client_id,
            SecretMaterial::from_bytes([3; 32]),
            SecretMaterial::from_bytes([4; 32]),
        );
        let complete = PortabilityFailureReceipt::try_recovery_bootstrap_complete(
            &complete_request,
            Box::new(FastiProblem::from_code(
                crate::ProblemCode::BootstrapClosed,
                CapabilityKey::RestoreWorkspace,
                bootstrap_correlation,
            )),
        )
        .expect("completion failure receipt");
        assert!(matches!(
            complete.operation(),
            PortabilityFailureOperation::RecoveryBootstrapComplete { .. }
        ));
        assert_eq!(complete.operation().restore_attempt_id(), Some(attempt_id));
        assert_eq!(complete.operation().workspace_id(), Some(workspace_id));
        assert_eq!(complete.operation().profile_id(), Some(profile_id));
        assert_eq!(complete.operation().client_id(), Some(recovery_client_id));
        assert_eq!(
            PortabilityFailureReceipt::try_recovery_bootstrap_complete(
                &complete_request,
                Box::new(FastiProblem::recovery_bootstrap_pending(
                    bootstrap_correlation,
                )),
            ),
            Err(PortabilityFailureReceiptError::OperationProblemMismatch),
            "pending is a prepare result, not a bad completion proof"
        );
        assert_eq!(
            PortabilityFailureReceipt::try_recovery_bootstrap_complete(
                &complete_request,
                Box::new(FastiProblem::from_code(
                    crate::ProblemCode::BootstrapClosed,
                    CapabilityKey::RestoreWorkspace,
                    RequestCorrelationId::new_v7(),
                )),
            ),
            Err(PortabilityFailureReceiptError::OperationProblemMismatch),
            "a completion problem from another request cannot be rebound"
        );

        assert_eq!(
            PortabilityFailureReceipt::try_online_export(
                &export_request,
                Box::new(FastiProblem::restore_canceled(export_correlation)),
            ),
            Err(PortabilityFailureReceiptError::CapabilityMismatch)
        );

        assert_eq!(
            PortabilityFailureReceipt::try_online_export(
                &export_request,
                Box::new(FastiProblem::export_canceled(RequestCorrelationId::new_v7())),
            ),
            Err(PortabilityFailureReceiptError::OperationProblemMismatch),
            "a problem from another request cannot be rebound to this export"
        );

        assert_eq!(
            PortabilityFailureReceipt::try_stopped_node_export(
                &stopped_request,
                Box::new(FastiProblem::stopped_node_export_required(
                    stopped_correlation
                )),
            ),
            Err(PortabilityFailureReceiptError::OperationProblemMismatch),
            "stopped-node export cannot request its own fallback"
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
            false,
        );
        assert_eq!(request.workspace_id(), workspace_id);
        assert_eq!(request.profile_id(), profile_id);
        assert!(!request.replace_pending());
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

        let credential_id = CredentialId::new_v7();
        let grant_id = ProfileGrantId::new_v7();
        let complete_request = CompleteRecoveryBootstrapRequest::new(
            attempt_id,
            RequestCorrelationId::new_v7(),
            workspace_id,
            profile_id,
            client_id,
            SecretMaterial::from_bytes([7; 32]),
            SecretMaterial::from_bytes([8; 32]),
        );
        assert_eq!(complete_request.restore_attempt_id(), attempt_id);
        assert_eq!(complete_request.workspace_id(), workspace_id);
        assert_eq!(complete_request.profile_id(), profile_id);
        assert_eq!(complete_request.client_id(), client_id);
        assert_eq!(
            complete_request.initialization_proof().expose_bytes(),
            &[7; 32]
        );
        assert_eq!(complete_request.credential().expose_bytes(), &[8; 32]);

        let access = RequestAccessContext::new(
            workspace_id,
            profile_id,
            client_id,
            credential_id,
            grant_id,
            1,
        );
        let completed = CompleteRecoveryBootstrapOutcome::new(attempt_id, access);
        assert_eq!(completed.restore_attempt_id(), attempt_id);
        assert_eq!(completed.access(), &access);
    }
}
