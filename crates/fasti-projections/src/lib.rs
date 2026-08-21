//! Fasti Projections Engine.
//!
//! Rebuilds materialised views (resume position, chronological history, library indexes)
//! deterministically from the immutable event ledger.

use chrono::{DateTime, Utc};
use fasti_activity::ActivityEvent;
use fasti_core::{MediaReference, Progress};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Materialised state of a user's current progress on a media item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaProgressState {
    pub media: MediaReference,
    pub progress: Progress,
    pub completed: bool,
    pub last_played_at: DateTime<Utc>,
    pub play_count: u64,
}

/// In-memory projection builder for the Continue/Resume queue.
#[derive(Default)]
pub struct ResumeProjection {
    items: HashMap<String, MediaProgressState>,
}

impl ResumeProjection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies an activity event to update the projected resume queue.
    pub fn apply_event(&mut self, event: &ActivityEvent) {
        let key = format!("{}:{}", event.media.source, event.media.id);
        if let Some(progress) = &event.progress {
            let is_completed = event.kind == "media.completed" 
                || (progress.total.is_some() && progress.value >= progress.total.unwrap());

            self.items.entry(key).and_modify(|entry| {
                entry.progress = progress.clone();
                entry.completed = is_completed;
                entry.last_played_at = event.timestamps.occurred_at;
                entry.play_count += 1;
            }).or_insert_with(|| MediaProgressState {
                media: event.media.clone(),
                progress: progress.clone(),
                completed: is_completed,
                last_played_at: event.timestamps.occurred_at,
                play_count: 1,
            });
        }
    }

    /// Returns uncompleted items sorted by most recently active.
    pub fn continue_queue(&self) -> Vec<MediaProgressState> {
        let mut list: Vec<_> = self.items.values().filter(|i| !i.completed).cloned().collect();
        list.sort_by(|a, b| b.last_played_at.cmp(&a.last_played_at));
        list
    }
}
