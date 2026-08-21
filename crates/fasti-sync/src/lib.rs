//! Fasti Logical Replica Sync Protocol.
//!
//! Synchronises immutable activity ledgers between client nodes and server daemons
//! using opaque cursors, idempotency keys, and sequence gap detection.

use fasti_activity::{ActivityEvent, EventReceipt};
use serde::{Deserialize, Serialize};

/// Batch push request containing offline or queued events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncPushRequest {
    pub client_node_id: String,
    pub events: Vec<ActivityEvent>,
}

/// Batch push response acknowledging accepted events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncPushResponse {
    pub receipts: Vec<EventReceipt>,
    pub server_cursor: String,
}

/// Incremental pull request using an opaque sync cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncPullRequest {
    pub client_node_id: String,
    pub since_cursor: Option<String>,
    pub limit: Option<usize>,
}

/// Incremental pull response containing new ledger entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncPullResponse {
    pub events: Vec<ActivityEvent>,
    pub next_cursor: String,
    pub has_more: bool,
}
