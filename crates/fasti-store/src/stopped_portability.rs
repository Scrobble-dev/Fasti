//! Stopped-daemon portability adapter owning only a configured data-root path.

use crate::online_archive::{export_stopped_node_workspace_archive, online_receipt};
use crate::recovery_coordinator::{complete_recovery_bootstrap, prepare_recovery_bootstrap};
use crate::restore_coordinator::restore_clean_workspace;
use fasti_application::{
    CapabilityKey, CompleteRecoveryBootstrapOutcome, CompleteRecoveryBootstrapRequest,
    ExportWorkspaceRequest, FastiProblem, PortabilityResult, PrepareRecoveryBootstrapOutcome,
    PrepareRecoveryBootstrapRequest, ReadSeek, RecoveryBootstrapPort, RestoreWorkspaceOutcome,
    RestoreWorkspaceRequest, StoppedNodeExportRequest, WorkspaceArchiveDestination,
    WorkspaceArchiveExportOutcome, WorkspaceArchiveExportPort, WorkspaceRestorePort,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct StoppedNodePortabilityAdapter {
    data_root: PathBuf,
}

impl StoppedNodePortabilityAdapter {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            data_root: data_root.as_ref().to_path_buf(),
        }
    }
}

impl WorkspaceArchiveExportPort for StoppedNodePortabilityAdapter {
    fn export_workspace_archive(
        &self,
        request: ExportWorkspaceRequest,
        destination: Box<dyn WorkspaceArchiveDestination>,
    ) -> PortabilityResult<WorkspaceArchiveExportOutcome> {
        let correlation_id = request.query().correlation_id();
        let problem = if destination.abort().is_ok() {
            Box::new(FastiProblem::stopped_node_export_required(correlation_id))
        } else {
            Box::new(FastiProblem::storage_unavailable(
                CapabilityKey::ExportWorkspace,
                correlation_id,
            ))
        };
        Err(online_receipt(&request, problem))
    }

    fn export_stopped_node_workspace_archive(
        &self,
        request: StoppedNodeExportRequest,
        destination: Box<dyn WorkspaceArchiveDestination>,
    ) -> PortabilityResult<WorkspaceArchiveExportOutcome> {
        export_stopped_node_workspace_archive(&self.data_root, request, destination)
    }
}

impl WorkspaceRestorePort for StoppedNodePortabilityAdapter {
    fn restore_workspace(
        &self,
        request: RestoreWorkspaceRequest,
        archive: Box<dyn ReadSeek + Send>,
    ) -> PortabilityResult<RestoreWorkspaceOutcome> {
        restore_clean_workspace(&self.data_root, request, archive)
    }
}

impl RecoveryBootstrapPort for StoppedNodePortabilityAdapter {
    fn prepare_recovery_bootstrap(
        &self,
        request: PrepareRecoveryBootstrapRequest,
    ) -> PortabilityResult<PrepareRecoveryBootstrapOutcome> {
        prepare_recovery_bootstrap(&self.data_root, request)
    }

    fn complete_recovery_bootstrap(
        &self,
        request: CompleteRecoveryBootstrapRequest,
    ) -> PortabilityResult<CompleteRecoveryBootstrapOutcome> {
        complete_recovery_bootstrap(&self.data_root, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_application::{
        CancellationSignal, ExportWorkspaceQuery, PortabilityLimits, RequestAccessContext,
    };
    use fasti_domain::{
        ClientId, CredentialId, ProfileGrantId, ProfileId, RequestCorrelationId, WorkspaceId,
    };
    use std::io::{self, Write};
    use std::num::NonZeroU64;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct DestinationState {
        aborted: bool,
        bytes: usize,
    }

    struct Destination(Arc<Mutex<DestinationState>>);

    impl Write for Destination {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("destination").bytes += bytes.len();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl WorkspaceArchiveDestination for Destination {
        fn preflight(&self, _required_bytes: u64) -> io::Result<()> {
            Ok(())
        }

        fn complete(
            self: Box<Self>,
            _archive_digest: &fasti_domain::Sha256Digest,
            _manifest_digest: &fasti_domain::Sha256Digest,
        ) -> io::Result<()> {
            Ok(())
        }

        fn abort(self: Box<Self>) -> io::Result<()> {
            self.0.lock().expect("destination").aborted = true;
            Ok(())
        }
    }

    fn nonzero(value: u64) -> NonZeroU64 {
        NonZeroU64::new(value).expect("non-zero")
    }

    fn limits() -> PortabilityLimits {
        PortabilityLimits {
            max_snapshot_bytes: nonzero(1024),
            max_wal_growth_bytes: nonzero(1024),
            max_archive_bytes: nonzero(1024),
            max_uncompressed_bytes: nonzero(1024),
            max_entry_bytes: nonzero(1024),
            max_entries: nonzero(32),
            max_rows_per_stream: nonzero(32),
            max_path_bytes: nonzero(100),
            max_path_depth: nonzero(8),
            max_decompression_ratio: nonzero(32),
            scratch_ceiling_bytes: nonzero(4096),
            cleanup_reserve_bytes: nonzero(1024),
            backup_step_pages: nonzero(1),
            backup_step_millis: nonzero(1),
        }
    }

    fn request() -> ExportWorkspaceRequest {
        ExportWorkspaceRequest::new(
            ExportWorkspaceQuery::new(
                RequestCorrelationId::new_v7(),
                RequestAccessContext::new(
                    WorkspaceId::new_v7(),
                    ProfileId::new_v7(),
                    ClientId::new_v7(),
                    CredentialId::new_v7(),
                    ProfileGrantId::new_v7(),
                    1,
                ),
            ),
            limits(),
            CancellationSignal::new(),
        )
    }

    #[test]
    fn stopped_adapter_aborts_an_online_destination_with_typed_mode_identity() {
        let adapter = StoppedNodePortabilityAdapter::new("unused");
        let request = request();
        let correlation_id = request.query().correlation_id();
        let workspace_id = request.query().access().workspace_id();
        let state = Arc::new(Mutex::new(DestinationState::default()));
        let failure = adapter
            .export_workspace_archive(request, Box::new(Destination(Arc::clone(&state))))
            .expect_err("online export requires the live adapter");

        assert_eq!(
            failure.problem().code(),
            fasti_application::ProblemCode::StoppedNodeExportRequired
        );
        assert_eq!(
            failure.operation(),
            fasti_application::PortabilityFailureOperation::OnlineExport {
                correlation_id,
                workspace_id,
            }
        );
        let state = state.lock().expect("destination");
        assert!(state.aborted);
        assert_eq!(state.bytes, 0);
    }
}
