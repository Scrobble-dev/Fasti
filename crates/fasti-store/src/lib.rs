//! SQLite and content-addressed filesystem adapters for the Fasti local kernel.

mod access;
pub mod archive;
mod client_credentials;
mod correction;
mod crypto;
mod evidence;
mod identity;
mod kernel;
mod observation;
mod online_archive;
mod portability;
mod recovery_coordinator;
mod restore;
mod restore_activation;
mod restore_coordinator;
mod restore_import;
mod review;
mod schema;
mod seed;
mod snapshot;
mod stopped_portability;
#[cfg(test)]
mod test_support;

pub use kernel::{DataRootIdentity, LockedDataRoot, SqliteKernel, StoreOpenError};
pub use portability::map_offline_verify_open_error;
pub use snapshot::{SnapshotError, SnapshotLimits, SnapshotMetadata, SnapshotProgress};
pub use stopped_portability::StoppedNodePortabilityAdapter;
