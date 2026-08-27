//! Application clock types used by delivery adapters.
//!
//! Transport crates should not add a second time-library dependency merely to
//! stamp provider ingress. The application layer already owns server-clock
//! semantics, so adapters import the narrow clock surface from here.

pub use chrono::{TimeZone, Utc};
