use crate::registry;
use fasti_application::{
    CredentialReference, CredentialSecret, CredentialVaultError, CredentialVaultPort,
    CredentialVaultSource, StoredCredential,
};
#[cfg(not(target_os = "android"))]
use keyring::{Entry, Error as KeyringError};
#[cfg(target_os = "android")]
use keyring_core::{Entry, Error as KeyringError};
#[cfg(target_os = "android")]
use std::sync::Arc;
use zeroize::Zeroize;

pub const PLATFORM_CREDENTIAL_SERVICE: &str = "dev.scrobble.fasti.desktop";

/// OS credential-store adapter scoped to one physical Fasti data root.
pub struct PlatformCredentialVault {
    service: String,
    data_root_scope: String,
}

impl PlatformCredentialVault {
    pub fn new(service: impl Into<String>, data_root_scope: impl Into<String>) -> Self {
        Self {
            service: service.into(),
            data_root_scope: data_root_scope.into(),
        }
    }

    pub fn initialize() -> Result<(), CredentialVaultError> {
        #[cfg(target_os = "android")]
        {
            let store = android_native_keyring_store::Store::new()
                .map_err(|_| CredentialVaultError::Unavailable)?;
            keyring_core::set_default_store(Arc::new(store));
        }
        Ok(())
    }

    fn entry(&self, reference: &CredentialReference) -> Result<Entry, CredentialVaultError> {
        Entry::new(&self.service, &self.account(reference)?)
            .map_err(|_| CredentialVaultError::Unavailable)
    }

    fn account(&self, reference: &CredentialReference) -> Result<String, CredentialVaultError> {
        self.spec(reference)?;
        Ok(format!("{}-{}", reference.as_str(), self.data_root_scope))
    }

    fn spec(
        &self,
        reference: &CredentialReference,
    ) -> Result<&'static crate::ProviderSpec, CredentialVaultError> {
        registry()
            .iter()
            .find(|spec| spec.runtime_available && spec.account == reference.as_str())
            .ok_or(CredentialVaultError::Rejected)
    }

    fn environment_secret(
        &self,
        reference: &CredentialReference,
    ) -> Result<Option<CredentialSecret>, CredentialVaultError> {
        let environment = self.spec(reference)?.environment;
        match std::env::var(environment) {
            Ok(mut value) => {
                if !valid_provider_credential(value.as_bytes()) {
                    value.zeroize();
                    return Err(CredentialVaultError::Rejected);
                }
                let secret = CredentialSecret::try_from_bytes(value.as_bytes().to_vec())
                    .map_err(|_| CredentialVaultError::Rejected);
                value.zeroize();
                secret.map(Some)
            }
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(CredentialVaultError::Rejected),
        }
    }

    fn write(
        &self,
        reference: &CredentialReference,
        secret: CredentialSecret,
    ) -> Result<StoredCredential, CredentialVaultError> {
        if self.environment_secret(reference)?.is_some()
            || !valid_provider_credential(secret.expose())
        {
            return Err(CredentialVaultError::Rejected);
        }
        let entry = self.entry(reference)?;
        entry
            .set_secret(secret.expose())
            .map_err(|_| CredentialVaultError::Unavailable)?;
        let mut stored = entry
            .get_secret()
            .map_err(|_| CredentialVaultError::Unavailable)?;
        let retained = stored == secret.expose();
        stored.zeroize();
        if !retained {
            return Err(CredentialVaultError::Unavailable);
        }
        StoredCredential::try_new(reference.clone(), 1).map_err(|_| CredentialVaultError::Rejected)
    }
}

impl CredentialVaultPort for PlatformCredentialVault {
    fn source(
        &self,
        reference: &CredentialReference,
    ) -> Result<CredentialVaultSource, CredentialVaultError> {
        if self.environment_secret(reference)?.is_some() {
            return Ok(CredentialVaultSource::Environment);
        }
        match self.entry(reference)?.get_secret() {
            Ok(mut secret) => {
                let valid = valid_provider_credential(&secret);
                secret.zeroize();
                if valid {
                    Ok(CredentialVaultSource::CredentialStore)
                } else {
                    Err(CredentialVaultError::Rejected)
                }
            }
            Err(KeyringError::NoEntry) => Ok(CredentialVaultSource::None),
            Err(_) => Err(CredentialVaultError::Unavailable),
        }
    }

    fn store(
        &self,
        reference: &CredentialReference,
        secret: CredentialSecret,
    ) -> Result<StoredCredential, CredentialVaultError> {
        self.write(reference, secret)
    }

    fn replace(
        &self,
        reference: &CredentialReference,
        secret: CredentialSecret,
    ) -> Result<StoredCredential, CredentialVaultError> {
        self.write(reference, secret)
    }

    fn load(
        &self,
        reference: &CredentialReference,
    ) -> Result<CredentialSecret, CredentialVaultError> {
        if let Some(secret) = self.environment_secret(reference)? {
            return Ok(secret);
        }
        let mut bytes = self.entry(reference)?.get_secret().map_err(|error| {
            if matches!(error, KeyringError::NoEntry) {
                CredentialVaultError::Missing
            } else {
                CredentialVaultError::Unavailable
            }
        })?;
        if !valid_provider_credential(&bytes) {
            bytes.zeroize();
            return Err(CredentialVaultError::Rejected);
        }
        CredentialSecret::try_from_bytes(bytes).map_err(|_| CredentialVaultError::Rejected)
    }

    fn revoke(&self, reference: &CredentialReference) -> Result<(), CredentialVaultError> {
        if self.environment_secret(reference)?.is_some() {
            return Err(CredentialVaultError::Rejected);
        }
        match self.entry(reference)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(_) => Err(CredentialVaultError::Unavailable),
        }
    }
}

fn valid_provider_credential(value: &[u8]) -> bool {
    !value.is_empty() && value.len() <= 512 && value.iter().all(|byte| byte.is_ascii_graphic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_keep_the_existing_data_root_suffix_shape() {
        let vault = PlatformCredentialVault::new(PLATFORM_CREDENTIAL_SERVICE, "abc123");
        let reference = CredentialReference::try_new("provider/tmdb/read-access-token")
            .expect("credential reference");
        assert_eq!(
            vault.account(&reference).expect("platform account"),
            "provider/tmdb/read-access-token-abc123"
        );
    }
}
