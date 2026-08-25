#[cfg(not(target_os = "android"))]
pub(crate) use keyring::{Entry, Error};
#[cfg(target_os = "android")]
pub(crate) use keyring_core::{Entry, Error};

#[cfg(not(target_os = "android"))]
pub(crate) fn initialize() -> Result<(), Error> {
    Ok(())
}

#[cfg(target_os = "android")]
pub(crate) fn initialize() -> Result<(), Error> {
    keyring_core::set_default_store(android_native_keyring_store::Store::new()?);
    Ok(())
}
