//! Isolated qualification only; no production crypto profile or recovery authority.
use libsodium_rs::crypto_sign::{self, KeyPair, PublicKey};

pub const MAX_MESSAGE_BYTES: usize = 16384;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Length,
    Provider,
}

/// Keeps provider keys out of public fields and diagnostics. Seed-only import.
///
/// ```compile_fail
/// use fasti_c3_sign_probe::SigningKey;
/// let key = SigningKey::generate().unwrap();
/// println!("{key:?}");
/// ```
/// ```compile_fail
/// use fasti_c3_sign_probe::SigningKey;
/// let key = SigningKey::generate().unwrap();
/// let copy = key.clone();
/// ```
/// ```compile_fail
/// use fasti_c3_sign_probe::SigningKey;
/// let raw = libsodium_rs::crypto_sign::KeyPair::generate().unwrap();
/// let key = SigningKey { pair: raw };
/// ```
pub struct SigningKey {
    pair: KeyPair,
}

impl SigningKey {
    pub fn generate() -> Result<Self, Error> {
        KeyPair::generate()
            .map(|pair| Self { pair })
            .map_err(|_| Error::Provider)
    }

    pub fn from_seed(seed: &[u8]) -> Result<Self, Error> {
        if seed.len() != crypto_sign::SEEDBYTES {
            return Err(Error::Length);
        }
        KeyPair::from_seed(seed)
            .map(|pair| Self { pair })
            .map_err(|_| Error::Provider)
    }

    pub fn public_key(&self) -> &[u8; 32] {
        self.pair.public_key.as_bytes()
    }

    pub fn sign(&self, message: &[u8]) -> Result<[u8; 64], Error> {
        if message.len() > MAX_MESSAGE_BYTES {
            return Err(Error::Length);
        }
        crypto_sign::sign_detached(message, &self.pair.secret_key).map_err(|_| Error::Provider)
    }
}

pub fn verify(signature: &[u8], message: &[u8], public_key: &[u8]) -> Result<bool, Error> {
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(Error::Length);
    }
    let signature: &[u8; 64] = signature.try_into().map_err(|_| Error::Length)?;
    let public_key = PublicKey::from_bytes(public_key).map_err(|_| Error::Length)?;
    Ok(crypto_sign::verify_detached(
        signature,
        message,
        &public_key,
    ))
}

#[cfg(test)]
mod tests;
