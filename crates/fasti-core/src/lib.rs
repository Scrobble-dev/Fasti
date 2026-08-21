//! Fasti Core domain primitives and invariants.
//!
//! Provides fundamental types for actors, devices, media references,
//! UUIDv7 event identifiers, and multi-timestamp semantics.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an activity event, generated as a UUIDv7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(pub Uuid);

impl EventId {
    /// Generates a new time-ordered UUIDv7 event identifier.
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new_v7()
    }
}

/// Distinct timestamp semantics representing the lifecycle of an observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventTimestamps {
    /// When the activity is reported to have occurred by the source.
    pub occurred_at: DateTime<Utc>,
    /// When the client observer recorded the activity.
    pub observed_at: DateTime<Utc>,
    /// When this Fasti node accepted and committed the record.
    pub received_at: DateTime<Utc>,
}

/// Canonical reference to a media item across sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaReference {
    /// Originating source system (e.g. "local", "plex", "trakt", "spotify", "openlibrary").
    pub source: String,
    /// Canonical identifier within the source system.
    pub id: String,
    /// Media grain / granularity (e.g. "track", "album", "episode", "season", "movie", "book", "chapter", "game").
    pub grain: String,
    /// Optional human-readable title.
    pub title: Option<String>,
}

/// Progress measurement with native units preserved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Current numerical value (seconds, pages, percentage, etc.).
    pub value: f64,
    /// Total length or count if known.
    pub total: Option<f64>,
    /// Native measurement unit (e.g. "seconds", "pages", "chapters", "percent", "sessions").
    pub unit: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_id_v7_generation() {
        let id1 = EventId::new_v7();
        let id2 = EventId::new_v7();
        assert_ne!(id1, id2);
        assert_eq!(id1.0.get_version_num(), 7);
    }
}
