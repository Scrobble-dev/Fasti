use serde::{Deserialize, Serialize};

macro_rules! define_scope_keys {
    ($($variant:ident),+ $(,)?) => {
        /// Internal application permission keys. The versioned contract
        /// registry owns their external stable strings and surface bindings.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum ScopeKey {
            $($variant),+
        }

        impl ScopeKey {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

define_scope_keys!(
    NodeInitialize,
    CapabilityRead,
    ClientEnroll,
    ProfileSelect,
    CredentialManage,
    ListenerConfigure,
    ObservationAccept,
    ReceiptRead,
    IdentityWrite,
    ReviewRead,
    ReviewWrite,
    CorrectionRead,
    CorrectionWrite,
    WorkspaceExport,
    WorkspaceRestore,
    WorkspaceVerify,
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
}
