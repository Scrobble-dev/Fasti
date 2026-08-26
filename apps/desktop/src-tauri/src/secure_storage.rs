#[cfg(not(target_os = "android"))]
pub(crate) use keyring::{Entry, Error};

#[cfg(target_os = "android")]
pub(crate) use keyring_core::{Entry, Error};

use fasti_store::DataRootIdentity;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

pub(crate) fn account_scope(identity: DataRootIdentity) -> String {
    let digest = Sha256::digest(identity.as_bytes());
    let mut account = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(account, "{byte:02x}").expect("writing to a String cannot fail");
    }
    account
}

pub(crate) fn scoped_account(label: &str, identity: DataRootIdentity) -> String {
    format!("{label}-{}", account_scope(identity))
}

pub(crate) fn initialize() -> Result<(), ()> {
    #[cfg(target_os = "android")]
    {
        let store = android_native_keyring_store::Store::new().map_err(|_| ())?;
        keyring_core::set_default_store(store);
    }

    #[cfg(not(target_os = "android"))]
    {}

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn credential_accounts_follow_the_opened_root_across_path_replacement() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let configured = temporary.path().join("fasti-data");
        let moved = temporary.path().join("moved-fasti-data");
        let first = fasti_store::SqliteKernel::open(&configured).expect("first kernel");
        let first_identity = first.data_root_identity();
        let first_provider = scoped_account("provider/google-books/api-key", first_identity);

        std::fs::rename(&configured, &moved).expect("rename first data root");
        drop(first);
        let reopened = fasti_store::SqliteKernel::open(&moved).expect("reopened moved root");
        assert_eq!(
            first_provider,
            scoped_account(
                "provider/google-books/api-key",
                reopened.data_root_identity()
            )
        );

        std::fs::create_dir(&configured).expect("replacement data root");
        let replacement = fasti_store::SqliteKernel::open(&configured).expect("replacement kernel");

        assert_ne!(
            first_provider,
            scoped_account(
                "provider/google-books/api-key",
                replacement.data_root_identity()
            )
        );
        assert_ne!(
            first_provider,
            scoped_account("local-admin-credential-v1", first_identity)
        );
    }
}
