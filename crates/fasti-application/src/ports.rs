//! Atomic application ports for observation acceptance and receipt replay.

use crate::{
    AcceptObservationCommand, AcceptObservationOutcome, AcceptObservationReceipt, FastiProblem,
    ReplayReceiptQuery, RequestAccessContext,
};
use fasti_domain::{ReceiptId, RequestCorrelationId};

pub const MAX_RECEIPT_STREAM_REPLAY: usize = 100;

pub type ApplicationResult<T> = Result<T, Box<FastiProblem>>;

/// Adapter boundary for the acceptance transaction.
///
/// Implementations must re-read the current access snapshot and apply the
/// application authorization evaluator inside the same transaction as the
/// idempotency lookup and receipt commit. A pre-authorized token is not enough:
/// revocation or epoch advancement may race the request.
pub trait ObservationAcceptancePort: Send + Sync {
    fn authorize_and_accept(
        &self,
        command: AcceptObservationCommand,
    ) -> ApplicationResult<AcceptObservationOutcome>;

    fn authorize_and_replay(
        &self,
        query: ReplayReceiptQuery,
    ) -> ApplicationResult<AcceptObservationReceipt>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamReceiptsQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    last_event_id: Option<String>,
}

impl StreamReceiptsQuery {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        last_event_id: Option<String>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            last_event_id,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptStreamEvent {
    correlation_id: RequestCorrelationId,
    cursor: ReceiptId,
    receipt: AcceptObservationReceipt,
}

impl ReceiptStreamEvent {
    pub fn new(correlation_id: RequestCorrelationId, receipt: AcceptObservationReceipt) -> Self {
        Self {
            correlation_id,
            cursor: receipt.receipt_id(),
            receipt,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn cursor(&self) -> ReceiptId {
        self.cursor
    }

    pub const fn receipt(&self) -> &AcceptObservationReceipt {
        &self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptStreamBatch {
    events: Vec<ReceiptStreamEvent>,
}

impl ReceiptStreamBatch {
    pub fn new(events: Vec<ReceiptStreamEvent>) -> Self {
        assert!(events.len() <= MAX_RECEIPT_STREAM_REPLAY);
        Self { events }
    }

    pub fn events(&self) -> &[ReceiptStreamEvent] {
        &self.events
    }
}

/// Short-lived authorization and bounded replay boundary for receipt streams.
/// Implementations return available events and release storage locks before a
/// delivery adapter starts SSE serialization or waits for a reconnect.
pub trait ReceiptStreamPort: Send + Sync {
    fn authorize_and_stream(
        &self,
        query: StreamReceiptsQuery,
    ) -> ApplicationResult<ReceiptStreamBatch>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_object_safe(_: &dyn ObservationAcceptancePort) {}
    fn assert_stream_object_safe(_: &dyn ReceiptStreamPort) {}

    #[test]
    fn ports_are_object_safe() {
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

        struct StreamCompileOnly;
        impl ReceiptStreamPort for StreamCompileOnly {
            fn authorize_and_stream(
                &self,
                _query: StreamReceiptsQuery,
            ) -> ApplicationResult<ReceiptStreamBatch> {
                unreachable!("compile-only port proof")
            }
        }
        assert_stream_object_safe(&StreamCompileOnly);
    }
}
