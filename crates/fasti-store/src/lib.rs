//! SQLite and content-addressed filesystem adapters for the Fasti local kernel.

mod access;
mod access_projection;
pub mod archive;
mod browser_auth;
mod client_credentials;
mod correction;
mod crypto;
mod evidence;
mod human_access;
mod identity;
mod identity_routing;
mod kernel;
mod local_search;
mod metadata;
mod nuvio_collections;
mod observation;
mod online_archive;
mod portability;
mod profile_state;
mod providers;
mod recovery_coordinator;
mod restore;
mod restore_activation;
mod restore_coordinator;
mod restore_import;
mod review;
mod schema;
mod search;
mod seed;
mod snapshot;
mod stopped_portability;
#[cfg(test)]
mod test_support;

pub use kernel::{DataRootIdentity, LockedDataRoot, SqliteKernel, StoreOpenError};
pub use portability::map_offline_verify_open_error;
pub use snapshot::{SnapshotError, SnapshotLimits, SnapshotMetadata, SnapshotProgress};
pub use stopped_portability::StoppedNodePortabilityAdapter;
