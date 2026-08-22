//! Fasti domain values and invariants.
//!
//! This crate is deliberately free of HTTP, storage, runtime, provider, and UI
//! dependencies. Adapters depend inward on these types; domain policy never
//! depends outward on an adapter representation.

pub mod ids;
pub mod media;
pub mod time;

pub use ids::*;
pub use media::*;
pub use time::*;
