//! Fasti application capabilities and ports.
//!
//! This layer coordinates domain work and owns public capability and problem
//! semantics. It does not import Axum, rusqlite, Tokio, provider, or UI types.

pub mod capabilities;
pub mod problems;
pub mod scopes;

pub use capabilities::*;
pub use problems::*;
pub use scopes::*;
