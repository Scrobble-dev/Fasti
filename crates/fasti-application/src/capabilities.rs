use crate::ScopeKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityBody {
    B0,
    B1,
    B2,
    B3,
}

impl CapabilityBody {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::B0 => "B0",
            Self::B1 => "B1",
            Self::B2 => "B2",
            Self::B3 => "B3",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractState {
    Finalized,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAvailability {
    Implemented,
    FixtureOnly,
    Guarded,
    LaterBody,
}

macro_rules! define_capabilities {
    ($(($variant:ident, $contract_body:ident, $runtime_body:ident, $contract_state:ident, $runtime_availability:ident, [$($scope:ident),*])),+ $(,)?) => {
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

            pub const fn contract_state(self) -> ContractState {
                match self {
                    $(Self::$variant => ContractState::$contract_state),+
                }
            }

            pub const fn runtime_availability(self) -> RuntimeAvailability {
                match self {
                    $(Self::$variant => RuntimeAvailability::$runtime_availability),+
                }
            }

            pub const fn required_scopes(self) -> &'static [ScopeKey] {
                match self {
                    $(Self::$variant => &[$(ScopeKey::$scope),*]),+
                }
            }

            pub const fn is_production_executable(self) -> bool {
                matches!(self.runtime_availability(), RuntimeAvailability::Implemented)
            }
        }
    };
}

// This single table owns application capability meaning. The versioned B1
// registry maps each key to one external stable ID and all surface metadata;
// application code never owns adapter-facing route or schema strings.
define_capabilities!(
    (SystemHealth, B1, B0, Finalized, Implemented, []),
    (
        DiscoverCapabilities,
        B1,
        B1,
        Finalized,
        FixtureOnly,
        [CapabilityRead]
    ),
    (
        InitializeNode,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        [NodeInitialize]
    ),
    (
        EnrollFirstClient,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        [ClientEnroll]
    ),
    (
        SelectProfile,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        [ProfileSelect]
    ),
    (
        RotateCredential,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        [CredentialManage]
    ),
    (
        RevokeCredential,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        [CredentialManage]
    ),
    (
        ConfigureListener,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        [ListenerConfigure]
    ),
    (
        AcceptObservation,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        [ObservationAccept]
    ),
    (ReplayReceipt, B1, B2, Finalized, FixtureOnly, [ReceiptRead]),
    (CreateRecord, B2, B2, Reserved, LaterBody, [IdentityWrite]),
    (
        AttachIdentifier,
        B2,
        B2,
        Reserved,
        LaterBody,
        [IdentityWrite]
    ),
    (InspectReview, B2, B2, Reserved, LaterBody, [ReviewRead]),
    (DeferReview, B2, B2, Reserved, LaterBody, [ReviewWrite]),
    (ResumeReview, B2, B2, Reserved, LaterBody, [ReviewWrite]),
    (ResolveReview, B2, B2, Reserved, LaterBody, [ReviewWrite]),
    (
        AppendCorrection,
        B3,
        B3,
        Reserved,
        LaterBody,
        [CorrectionWrite]
    ),
    (
        InspectCorrectionChain,
        B3,
        B3,
        Reserved,
        LaterBody,
        [CorrectionRead]
    ),
    (
        ExportWorkspace,
        B3,
        B3,
        Reserved,
        Guarded,
        [WorkspaceExport]
    ),
    (
        RestoreWorkspace,
        B3,
        B3,
        Reserved,
        Guarded,
        [WorkspaceRestore]
    ),
    (
        VerifyWorkspace,
        B3,
        B3,
        Reserved,
        Guarded,
        [WorkspaceVerify]
    ),
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
            CapabilityKey::AcceptObservation.contract_state(),
            ContractState::Finalized
        );
        assert_eq!(
            CapabilityKey::AcceptObservation.runtime_availability(),
            RuntimeAvailability::FixtureOnly
        );
        assert_eq!(
            CapabilityKey::AcceptObservation.required_scopes(),
            &[ScopeKey::ObservationAccept]
        );
    }

    #[test]
    fn later_body_ids_do_not_freeze_early_contracts() {
        assert_eq!(
            CapabilityKey::ResolveReview.contract_body(),
            CapabilityBody::B2
        );
        assert_eq!(
            CapabilityKey::ResolveReview.contract_state(),
            ContractState::Reserved
        );
        assert_eq!(
            CapabilityKey::ResolveReview.runtime_availability(),
            RuntimeAvailability::LaterBody
        );
    }

    #[test]
    fn runtime_availability_assignments_are_exact() {
        let implemented = CapabilityKey::ALL
            .iter()
            .copied()
            .filter(|key| key.runtime_availability() == RuntimeAvailability::Implemented)
            .collect::<HashSet<_>>();
        let fixture_only = CapabilityKey::ALL
            .iter()
            .copied()
            .filter(|key| key.runtime_availability() == RuntimeAvailability::FixtureOnly)
            .collect::<HashSet<_>>();
        let guarded = CapabilityKey::ALL
            .iter()
            .copied()
            .filter(|key| key.runtime_availability() == RuntimeAvailability::Guarded)
            .collect::<HashSet<_>>();
        let later_body = CapabilityKey::ALL
            .iter()
            .copied()
            .filter(|key| key.runtime_availability() == RuntimeAvailability::LaterBody)
            .collect::<HashSet<_>>();

        assert_eq!(implemented, HashSet::from([CapabilityKey::SystemHealth]));
        assert_eq!(
            fixture_only,
            HashSet::from([
                CapabilityKey::DiscoverCapabilities,
                CapabilityKey::InitializeNode,
                CapabilityKey::EnrollFirstClient,
                CapabilityKey::SelectProfile,
                CapabilityKey::RotateCredential,
                CapabilityKey::RevokeCredential,
                CapabilityKey::ConfigureListener,
                CapabilityKey::AcceptObservation,
                CapabilityKey::ReplayReceipt,
            ])
        );
        assert_eq!(
            guarded,
            HashSet::from([
                CapabilityKey::ExportWorkspace,
                CapabilityKey::RestoreWorkspace,
                CapabilityKey::VerifyWorkspace,
            ])
        );
        assert_eq!(
            later_body,
            HashSet::from([
                CapabilityKey::CreateRecord,
                CapabilityKey::AttachIdentifier,
                CapabilityKey::InspectReview,
                CapabilityKey::DeferReview,
                CapabilityKey::ResumeReview,
                CapabilityKey::ResolveReview,
                CapabilityKey::AppendCorrection,
                CapabilityKey::InspectCorrectionChain,
            ])
        );
        assert!(CapabilityKey::SystemHealth.is_production_executable());
        assert!(CapabilityKey::ALL
            .iter()
            .copied()
            .filter(|key| *key != CapabilityKey::SystemHealth)
            .all(|key| !key.is_production_executable()));
    }
}
