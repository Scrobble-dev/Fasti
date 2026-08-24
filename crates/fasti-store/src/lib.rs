//! SQLite and content-addressed filesystem adapters for the Fasti local kernel.

mod access;
pub mod archive;
mod correction;
mod crypto;
mod evidence;
mod identity;
mod kernel;
mod observation;
mod portability;
mod review;
mod schema;
mod seed;
mod snapshot;
#[cfg(test)]
mod test_support;

pub use kernel::{LockedDataRoot, SqliteKernel, StoreOpenError};
pub use snapshot::{SnapshotError, SnapshotLimits, SnapshotMetadata, SnapshotProgress};
