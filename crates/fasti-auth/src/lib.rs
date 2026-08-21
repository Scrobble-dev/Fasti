//! Fasti Authentication and Token Scoping.

use serde::{Deserialize, Serialize};

/// Granular permission scopes for Fasti API tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScope {
    EventsWrite,
    HistoryRead,
    LibraryRead,
    LibraryWrite,
    SyncAdmin,
    SettingsAdmin,
}

/// Token payload representing a client's capability set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedToken {
    pub token_id: String,
    pub actor_id: String,
    pub label: String,
    pub scopes: Vec<AuthScope>,
}
