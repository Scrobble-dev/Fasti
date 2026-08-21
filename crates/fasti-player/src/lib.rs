//! Fasti Playback State Machine.
//!
//! Bridges raw player events (mpv, web player, external player) into typed
//! observations before promotion to the immutable activity ledger.

use chrono::{DateTime, Utc};
use fasti_core::{MediaReference, Progress};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlaybackObservation {
    SessionStarted {
        session_id: String,
        media: MediaReference,
        timestamp: DateTime<Utc>,
    },
    ProgressObserved {
        session_id: String,
        progress: Progress,
        timestamp: DateTime<Utc>,
    },
    Paused {
        session_id: String,
        timestamp: DateTime<Utc>,
    },
    Seeked {
        session_id: String,
        from_seconds: f64,
        to_seconds: f64,
        timestamp: DateTime<Utc>,
    },
    SessionEnded {
        session_id: String,
        timestamp: DateTime<Utc>,
    },
}
