//! Bounded application inputs shared by every delivery adapter.
//!
//! These limits keep one request from turning a small local write into an
//! unbounded lookup or transaction. Adapters may reject earlier, but they may
//! not accept a larger value than the application boundary.

/// Maximum exact identity claims accepted by one operation.
pub const MAX_IDENTITY_CLAIMS: usize = 32;

/// Maximum records described by one identity-seed application.
pub const MAX_IDENTITY_SEED_ENTRIES: usize = 1_000;

/// Maximum UTF-8 bytes in one stable identity-seed entry key.
pub const MAX_IDENTITY_SEED_KEY_BYTES: usize = 128;

/// Maximum UTF-8 bytes in one identity-seed version label.
pub const MAX_IDENTITY_SEED_VERSION_BYTES: usize = 64;
