//! Fasti application capabilities and ports.
//!
//! This layer coordinates domain work and owns public capability and problem
//! semantics. It does not import Axum, rusqlite, Tokio, provider, or UI types.

pub mod authorization;
pub mod capabilities;
#[cfg(feature = "conformance-fixture")]
pub mod conformance;
pub mod corrections;
pub mod kernel;
pub mod limits;
pub mod ports;
pub mod problems;
pub mod receipts;
pub mod requests;
pub mod scopes;

pub use authorization::*;
pub use capabilities::*;
pub use corrections::*;
pub use kernel::*;
pub use limits::*;
pub use ports::*;
pub use problems::*;
pub use receipts::*;
pub use requests::*;
pub use scopes::*;
