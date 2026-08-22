//! Fasti Activity Event validation and envelope definition.

use chrono::{DateTime, Utc};
use fasti_core::{EventId, EventTimestamps, MediaReference, Progress, RecordId, ResolutionStatus};
use serde::{Deserialize, Serialize};

/// Canonical activity event envelope representing an immutable occurrence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityEvent {
    /// Global UUIDv7 event identifier.
    pub event_id: EventId,
    /// Envelope schema version (e.g. "1.0").
    pub schema_version: String,
    /// Actor or account identifier.
    pub actor_id: String,
    /// Originating device identifier.
    pub device_id: String,
    /// Monotonic per-device sequence number.
    pub device_seq: u64,
    /// Type of activity (e.g. "media.played", "media.progressed", "media.completed", "media.rated").
    pub kind: String,
    /// Canonical media reference.
    pub media: MediaReference,
    /// Progress measurement if applicable.
    pub progress: Option<Progress>,
    /// Multi-timestamp record.
    pub timestamps: EventTimestamps,
    /// Provenance metadata detailing observer and ingestion source.
    pub provenance: Provenance,
    /// Optional Fasti Record binding if resolved.
    pub record_id: Option<RecordId>,
    /// Current resolution status.
    pub resolution_status: Option<ResolutionStatus>,
    /// Event ID this event supersedes/corrects, if any.
    pub correction_of: Option<EventId>,
    /// Event ID this event tombstones/deletes, if any.
    pub tombstone_of: Option<EventId>,
}

/// Provenance metadata recording how and where the activity was captured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Ingestion channel (e.g. "player", "webhook", "import", "manual", "sync").
    pub channel: String,
    /// Specific external client or adapter identifier (for example, "floppy-import/1.0").
    pub client: String,
    /// Optional external source event ID for deduplication.
    pub external_event_id: Option<String>,
}

/// Acknowledgment receipt returned upon idempotent event ingestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventReceipt {
    pub event_id: EventId,
    pub received_at: DateTime<Utc>,
    pub status: ReceiptStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    Committed,
    DuplicateIgnored,
    CorrectionAccepted,
}
