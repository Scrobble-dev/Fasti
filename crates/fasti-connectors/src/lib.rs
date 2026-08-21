//! Fasti Connectors and Importers.
//!
//! Maps external media tracking data (Trakt, Floppy, Plex, Letterboxd, Last.fm)
//! into canonical ActivityEvents while accounting for data loss during translation.

use fasti_activity::ActivityEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub source_system: String,
    pub total_records: usize,
    pub imported_records: usize,
    pub duplicate_records: usize,
    pub loss_warnings: Vec<String>,
}

pub trait Importer {
    fn name(&self) -> &'static str;
    fn import(&self, raw_input: &[u8]) -> Result<(Vec<ActivityEvent>, ImportReport), String>;
}
