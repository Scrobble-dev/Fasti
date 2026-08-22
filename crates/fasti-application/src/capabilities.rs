use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityBody {
    B0,
    B1,
    B2,
    B3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityLifecycle {
    Implemented,
    ContractExecutable,
    Guarded,
    LaterBody,
}

macro_rules! define_capabilities {
    ($(($variant:ident, $contract_body:ident, $runtime_body:ident, $lifecycle:ident)),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum CapabilityKey {
            $($variant),+
        }

        impl CapabilityKey {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn contract_body(self) -> CapabilityBody {
                match self {
                    $(Self::$variant => CapabilityBody::$contract_body),+
                }
            }

            pub const fn runtime_body(self) -> CapabilityBody {
                match self {
                    $(Self::$variant => CapabilityBody::$runtime_body),+
                }
            }

            pub const fn lifecycle(self) -> CapabilityLifecycle {
                match self {
                    $(Self::$variant => CapabilityLifecycle::$lifecycle),+
                }
            }
        }
    };
}

// This single table owns application capability meaning. The versioned B1
// registry maps each key to one external stable ID and all surface metadata;
// application code never owns adapter-facing route or schema strings.
define_capabilities!(
    (SystemHealth, B1, B0, Implemented),
    (InitializeNode, B1, B2, ContractExecutable),
    (EnrollFirstClient, B1, B2, ContractExecutable),
    (SelectProfile, B1, B2, ContractExecutable),
    (RotateCredential, B1, B2, ContractExecutable),
    (RevokeCredential, B1, B2, ContractExecutable),
    (ConfigureListener, B1, B2, ContractExecutable),
    (AcceptObservation, B1, B2, ContractExecutable),
    (ReplayReceipt, B1, B2, ContractExecutable),
    (CreateRecord, B2, B2, LaterBody),
    (AttachIdentifier, B2, B2, LaterBody),
    (InspectReview, B2, B2, LaterBody),
    (DeferReview, B2, B2, LaterBody),
    (ResumeReview, B2, B2, LaterBody),
    (ResolveReview, B2, B2, LaterBody),
    (AppendCorrection, B3, B3, LaterBody),
    (InspectCorrectionChain, B3, B3, LaterBody),
    (ExportWorkspace, B3, B3, Guarded),
    (RestoreWorkspace, B3, B3, Guarded),
    (VerifyWorkspace, B3, B3, Guarded),
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn capability_keys_are_unique() {
        let unique: HashSet<_> = CapabilityKey::ALL.iter().collect();
        assert_eq!(unique.len(), CapabilityKey::ALL.len());
    }

    #[test]
    fn b1_contract_fixture_does_not_claim_b2_runtime_implementation() {
        assert_eq!(
            CapabilityKey::AcceptObservation.contract_body(),
            CapabilityBody::B1
        );
        assert_eq!(
            CapabilityKey::AcceptObservation.runtime_body(),
            CapabilityBody::B2
        );
        assert_eq!(
            CapabilityKey::AcceptObservation.lifecycle(),
            CapabilityLifecycle::ContractExecutable
        );
    }

    #[test]
    fn later_body_ids_do_not_freeze_early_contracts() {
        assert_eq!(
            CapabilityKey::ResolveReview.contract_body(),
            CapabilityBody::B2
        );
        assert_eq!(
            CapabilityKey::ResolveReview.lifecycle(),
            CapabilityLifecycle::LaterBody
        );
    }
}
