use serde::{Deserialize, Serialize};

macro_rules! define_scope_keys {
    ($(($variant:ident, $value:literal)),+ $(,)?) => {
        /// Internal application permission keys. The versioned contract
        /// registry owns their external stable strings and surface bindings.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum ScopeKey {
            $(#[serde(rename = $value)] $variant),+
        }

        impl ScopeKey {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }
    };
}

define_scope_keys!(
    (CapabilityRead, "capability_read"),
    (ClientEnroll, "client_enroll"),
    (ProfileSelect, "profile_select"),
    (CredentialManage, "credential_manage"),
    (ListenerConfigure, "listener_configure"),
    (ObservationAccept, "observation_accept"),
    (ReceiptRead, "receipt_read"),
    (IdentityWrite, "identity_write"),
    (ReviewRead, "review_read"),
    (ReviewWrite, "review_write"),
    (CorrectionRead, "correction_read"),
    (CorrectionWrite, "correction_write"),
    (WorkspaceExport, "workspace_export"),
    (WorkspaceRestore, "workspace_restore"),
    (WorkspaceVerify, "workspace_verify"),
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn scope_keys_are_unique() {
        let unique: HashSet<_> = ScopeKey::ALL.iter().collect();
        assert_eq!(unique.len(), ScopeKey::ALL.len());
    }

    #[test]
    fn scope_storage_strings_are_stable() {
        assert_eq!(ScopeKey::ObservationAccept.as_str(), "observation_accept");
        assert_eq!(ScopeKey::IdentityWrite.as_str(), "identity_write");
    }
}
