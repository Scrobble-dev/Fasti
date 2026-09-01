//! Private clean-restore composition above pass-one, pass-two, and activation.

use crate::archive::ArchiveError;
use crate::kernel::LockedDataRoot;
use crate::portability::map_offline_open_error;
use crate::restore::RestorePreflightError;
use crate::restore_activation::{require_clean_restore_target, RestoreActivationError};
use crate::restore_import::{
    preflight_restore_source, reject_interrupted_restore,
    stage_preflighted_workspace_archive_pass_two, RestoreImportError,
};
use fasti_application::{
    CapabilityKey, FastiProblem, PortabilityFailureReceipt, PortabilityResult, ReadSeek,
    RestoreWorkspaceOutcome, RestoreWorkspaceRequest,
};
use std::path::Path;

#[allow(dead_code)] // activated with the coordinated store adapter
pub(crate) fn restore_clean_workspace(
    data_root: impl AsRef<Path>,
    request: RestoreWorkspaceRequest,
    mut archive: Box<dyn ReadSeek + Send>,
) -> PortabilityResult<RestoreWorkspaceOutcome> {
    let correlation_id = request.correlation_id();
    let result = (|| {
        if request.cancellation().is_cancelled() {
            return Err(Box::new(FastiProblem::restore_canceled(correlation_id)));
        }
        let locked = LockedDataRoot::acquire(data_root).map_err(|error| {
            map_offline_open_error(error, CapabilityKey::RestoreWorkspace, correlation_id)
        })?;
        let root = locked.anchored_directory().ok_or_else(|| {
            Box::new(FastiProblem::unsupported_platform(
                CapabilityKey::RestoreWorkspace,
                correlation_id,
            ))
        })?;
        let preflight =
            preflight_restore_source(archive.as_mut(), request.limits(), request.cancellation())
                .map_err(|error| import_problem(error, correlation_id))?;
        match require_clean_restore_target(root) {
            Ok(()) => {}
            Err(RestoreActivationError::IncompleteStaging) => {
                reject_interrupted_restore(root, request.limits().max_entries.get())
                    .map_err(|error| import_problem(error, correlation_id))?;
                require_clean_restore_target(root)
                    .map_err(|error| activation_problem(error, correlation_id))?;
            }
            Err(error) => return Err(activation_problem(error, correlation_id)),
        }
        let staged = stage_preflighted_workspace_archive_pass_two(
            &locked,
            archive.as_mut(),
            request.restore_attempt_id(),
            correlation_id,
            request.limits(),
            request.cancellation(),
            preflight,
        )
        .map_err(|error| import_problem(error, correlation_id))?;
        let marker = staged
            .activate(root, request.cancellation())
            .map_err(|error| import_problem(error, correlation_id))?;
        Ok(RestoreWorkspaceOutcome::complete(
            marker.restore_attempt_id(),
            marker.workspace_id(),
            marker.manifest_digest().clone(),
        ))
    })();

    result.map_err(|problem| {
        PortabilityFailureReceipt::try_clean_restore(&request, problem)
            .expect("clean restore problems are owned by RestoreWorkspace")
    })
}

fn import_problem(
    error: RestoreImportError,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    let capability = CapabilityKey::RestoreWorkspace;
    match error {
        RestoreImportError::Canceled => Box::new(FastiProblem::restore_canceled(correlation_id)),
        RestoreImportError::CapacityExceeded | RestoreImportError::RowTooLarge { .. } => {
            Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
        }
        RestoreImportError::UnsupportedPlatform => Box::new(FastiProblem::unsupported_platform(
            capability,
            correlation_id,
        )),
        RestoreImportError::Preflight(error) => preflight_problem(error, correlation_id),
        RestoreImportError::Archive(error) => archive_problem(error, correlation_id),
        RestoreImportError::Activation(error) => activation_problem(error, correlation_id),
        RestoreImportError::Rewind(_)
        | RestoreImportError::Sqlite(_)
        | RestoreImportError::Sync(_)
        | RestoreImportError::Cleanup { .. } => Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        )),
        RestoreImportError::ArchiveChanged
        | RestoreImportError::ManifestChanged
        | RestoreImportError::EntryOrder { .. }
        | RestoreImportError::InvalidRow { .. }
        | RestoreImportError::NonCanonicalRow { .. }
        | RestoreImportError::RowOrder { .. }
        | RestoreImportError::StreamDescriptor { .. }
        | RestoreImportError::BlobDescriptor { .. }
        | RestoreImportError::RowInvariant { .. }
        | RestoreImportError::DomainInvariant
        | RestoreImportError::IdentityRoutingInvariant
        | RestoreImportError::PolicyReceiptInvariant
        | RestoreImportError::SqliteIntegrity
        | RestoreImportError::RelationInvariant
        | RestoreImportError::AggregateInvariant
        | RestoreImportError::InterpretationChainInvariant
        | RestoreImportError::MetadataLifecycleInvariant
        | RestoreImportError::SchemaMismatch
        | RestoreImportError::RevisionMismatch
        | RestoreImportError::CountMismatch
        | RestoreImportError::EvidenceMismatch
        | RestoreImportError::NodeLocalStatePresent => {
            Box::new(FastiProblem::integrity_failed(capability, correlation_id))
        }
    }
}

fn preflight_problem(
    error: RestorePreflightError,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    let capability = CapabilityKey::RestoreWorkspace;
    match error {
        RestorePreflightError::Archive(error) => archive_problem(error, correlation_id),
        RestorePreflightError::InitialSeek(_)
        | RestorePreflightError::Rewind(_)
        | RestorePreflightError::EntryRead { .. } => Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        )),
        RestorePreflightError::ManifestAllocationFailed
        | RestorePreflightError::ExpandedCeilingOverflow
        | RestorePreflightError::ManifestSizeUnsupported
        | RestorePreflightError::PathBytesExceeded { .. }
        | RestorePreflightError::PathDepthExceeded { .. }
        | RestorePreflightError::StreamRowCountExceeded { .. }
        | RestorePreflightError::DecompressionRatioExceeded { .. } => {
            Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
        }
        RestorePreflightError::Manifest(_)
        | RestorePreflightError::EntryOrder { .. }
        | RestorePreflightError::EntryByteCountOverflow { .. }
        | RestorePreflightError::BlankNdjsonLine { .. }
        | RestorePreflightError::NonTerminatedNdjson { .. }
        | RestorePreflightError::MissingVerifiedManifest
        | RestorePreflightError::StreamCountMismatch
        | RestorePreflightError::StreamDescriptorMismatch { .. }
        | RestorePreflightError::BlobCountMismatch
        | RestorePreflightError::BlobDescriptorMismatch { .. } => {
            Box::new(FastiProblem::integrity_failed(capability, correlation_id))
        }
    }
}

fn archive_problem(
    error: ArchiveError,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    let capability = CapabilityKey::RestoreWorkspace;
    match error {
        ArchiveError::Io(_) => Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        )),
        ArchiveError::InvalidLimits
        | ArchiveError::EntryCountExceeded { .. }
        | ArchiveError::EntrySizeExceeded { .. }
        | ArchiveError::ExpandedSizeExceeded { .. }
        | ArchiveError::CompressedSizeExceeded { .. } => {
            Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
        }
        ArchiveError::UnsupportedPlatform => Box::new(FastiProblem::unsupported_platform(
            capability,
            correlation_id,
        )),
        ArchiveError::DestinationExists => Box::new(FastiProblem::from_code(
            fasti_application::ProblemCode::ValidationFailed,
            capability,
            correlation_id,
        )),
        ArchiveError::InvalidPath(_)
        | ArchiveError::UnsupportedEntryType(_)
        | ArchiveError::NonUstarHeader
        | ArchiveError::NonCanonicalHeader
        | ArchiveError::DuplicateEntry(_)
        | ArchiveError::TruncatedEntry { .. }
        | ArchiveError::MissingManifest
        | ArchiveError::EntryAfterManifest
        | ArchiveError::TrailingData
        | ArchiveError::UnsafeActivationName
        | ArchiveError::CrossFilesystemActivation
        | ArchiveError::UnsafeActivationFile => {
            Box::new(FastiProblem::integrity_failed(capability, correlation_id))
        }
    }
}

pub(crate) fn activation_problem(
    error: RestoreActivationError,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    let capability = CapabilityKey::RestoreWorkspace;
    match error {
        RestoreActivationError::Archive(error) => archive_problem(error, correlation_id),
        RestoreActivationError::Io(_) => Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        )),
        RestoreActivationError::CurrentExists => Box::new(FastiProblem::from_code(
            fasti_application::ProblemCode::ValidationFailed,
            capability,
            correlation_id,
        )),
        RestoreActivationError::InvalidMarker
        | RestoreActivationError::MarkerMismatch
        | RestoreActivationError::InvalidPhase
        | RestoreActivationError::IncompleteStaging => {
            Box::new(FastiProblem::integrity_failed(capability, correlation_id))
        }
    }
}
