//! Private recovery-bootstrap composition bound to a durable restore marker.

use crate::kernel::{LockedDataRoot, SqliteKernel};
use crate::portability::map_offline_open_error;
use crate::restore_activation::verify_complete_restore;
use crate::restore_coordinator::activation_problem;
use fasti_application::{
    CapabilityKey, CompleteRecoveryBootstrapOutcome, CompleteRecoveryBootstrapRequest,
    FastiProblem, PortabilityFailureReceipt, PortabilityResult, PrepareRecoveryBootstrapOutcome,
    PrepareRecoveryBootstrapRequest,
};
use fasti_domain::{RequestCorrelationId, RestoreAttemptId, WorkspaceId};
use std::path::Path;

#[allow(dead_code)] // activated with the coordinated store adapter
pub(crate) fn prepare_recovery_bootstrap(
    data_root: impl AsRef<Path>,
    request: PrepareRecoveryBootstrapRequest,
) -> PortabilityResult<PrepareRecoveryBootstrapOutcome> {
    let kernel = match open_verified_recovery_kernel(
        data_root.as_ref(),
        request.restore_attempt_id(),
        request.workspace_id(),
        request.correlation_id(),
    ) {
        Ok(kernel) => kernel,
        Err(problem) => {
            return Err(PortabilityFailureReceipt::try_recovery_bootstrap_prepare(
                &request, problem,
            )
            .expect("recovery prepare problems are owned by RestoreWorkspace"));
        }
    };
    kernel.prepare_recovery_bootstrap_after_verified_activation(request)
}

#[allow(dead_code)] // activated with the coordinated store adapter
pub(crate) fn complete_recovery_bootstrap(
    data_root: impl AsRef<Path>,
    request: CompleteRecoveryBootstrapRequest,
) -> PortabilityResult<CompleteRecoveryBootstrapOutcome> {
    let kernel = match open_verified_recovery_kernel(
        data_root.as_ref(),
        request.restore_attempt_id(),
        request.workspace_id(),
        request.correlation_id(),
    ) {
        Ok(kernel) => kernel,
        Err(problem) => {
            return Err(PortabilityFailureReceipt::try_recovery_bootstrap_complete(
                &request, problem,
            )
            .expect("recovery completion problems are owned by RestoreWorkspace"));
        }
    };
    kernel.complete_recovery_bootstrap_transaction(request)
}

fn open_verified_recovery_kernel(
    data_root: &Path,
    restore_attempt_id: RestoreAttemptId,
    workspace_id: WorkspaceId,
    correlation_id: RequestCorrelationId,
) -> Result<SqliteKernel, Box<FastiProblem>> {
    let capability = CapabilityKey::RestoreWorkspace;
    let locked = LockedDataRoot::acquire(data_root)
        .map_err(|error| map_offline_open_error(error, capability, correlation_id))?;
    let root = locked.anchored_directory().ok_or_else(|| {
        Box::new(FastiProblem::unsupported_platform(
            capability,
            correlation_id,
        ))
    })?;
    let before = verify_complete_restore(root, restore_attempt_id, workspace_id)
        .map_err(|error| activation_problem(error, correlation_id))?;
    let kernel = SqliteKernel::open_locked(locked)
        .map_err(|error| map_offline_open_error(error, capability, correlation_id))?;
    let after_root = kernel.inner.data_root.anchored_directory().ok_or_else(|| {
        Box::new(FastiProblem::unsupported_platform(
            capability,
            correlation_id,
        ))
    })?;
    let after = verify_complete_restore(after_root, restore_attempt_id, workspace_id)
        .map_err(|error| activation_problem(error, correlation_id))?;
    if after != before {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    Ok(kernel)
}
