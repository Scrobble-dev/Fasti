use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{RecordId, RequestCorrelationId, TrackingDisposition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListTrackingDispositionsQuery {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
}

impl ListTrackingDispositionsQuery {
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
pub struct SetTrackingDispositionCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    disposition: Option<TrackingDisposition>,
}

impl SetTrackingDispositionCommand {
    pub const fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        disposition: Option<TrackingDisposition>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            disposition,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn disposition(&self) -> Option<TrackingDisposition> {
        self.disposition
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackingDispositionView {
    record_id: RecordId,
    disposition: TrackingDisposition,
}

impl TrackingDispositionView {
    pub const fn new(record_id: RecordId, disposition: TrackingDisposition) -> Self {
        Self {
            record_id,
            disposition,
        }
    }

    pub const fn record_id(self) -> RecordId {
        self.record_id
    }

    pub const fn disposition(self) -> TrackingDisposition {
        self.disposition
    }
}

pub trait ProfileRecordStatePort: Send + Sync {
    fn list_tracking_dispositions(
        &self,
        query: ListTrackingDispositionsQuery,
    ) -> ApplicationResult<Vec<TrackingDispositionView>>;

    fn set_tracking_disposition(
        &self,
        command: SetTrackingDispositionCommand,
    ) -> ApplicationResult<Option<TrackingDispositionView>>;
}
