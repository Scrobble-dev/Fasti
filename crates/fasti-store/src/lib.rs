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
pub use portability::map_offline_verify_open_error;
pub use snapshot::{SnapshotError, SnapshotLimits, SnapshotMetadata, SnapshotProgress};
