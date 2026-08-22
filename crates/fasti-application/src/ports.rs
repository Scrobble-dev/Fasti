//! Atomic application ports for observation acceptance and receipt replay.

use crate::{
    AcceptObservationCommand, AcceptObservationOutcome, AcceptObservationReceipt, FastiProblem,
    ReplayReceiptQuery,
};

pub type ApplicationResult<T> = Result<T, Box<FastiProblem>>;

/// Adapter boundary for the acceptance transaction.
///
/// Implementations must re-read the current access snapshot and apply the
/// application authorization evaluator inside the same transaction as the
/// idempotency lookup and receipt commit. A pre-authorized token is not enough:
/// revocation or epoch advancement may race the request.
///
/// For one operation ID, same capability plus digest returns the exact original
/// receipt; changed capability or digest returns an idempotency conflict without
/// changing that receipt. B1 supplies only a non-production conformance adapter.
pub trait ObservationAcceptancePort {
    fn authorize_and_accept(
        &self,
        command: AcceptObservationCommand,
    ) -> ApplicationResult<AcceptObservationOutcome>;

    fn authorize_and_replay(
        &self,
        query: ReplayReceiptQuery,
    ) -> ApplicationResult<AcceptObservationReceipt>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_object_safe(_: &dyn ObservationAcceptancePort) {}

    #[test]
    fn port_can_be_supplied_as_one_atomic_adapter_boundary() {
        struct CompileOnly;

        impl ObservationAcceptancePort for CompileOnly {
            fn authorize_and_accept(
                &self,
                _command: AcceptObservationCommand,
            ) -> ApplicationResult<AcceptObservationOutcome> {
                unreachable!("compile-only port proof")
            }

            fn authorize_and_replay(
                &self,
                _query: ReplayReceiptQuery,
            ) -> ApplicationResult<AcceptObservationReceipt> {
                unreachable!("compile-only port proof")
            }
        }

        assert_object_safe(&CompileOnly);
    }
}
