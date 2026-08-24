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
#[cfg(test)]
mod test_support;

pub use kernel::{SqliteKernel, StoreOpenError};
