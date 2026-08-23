use crate::{ProblemCode, ScopeKey};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationKind {
    Unauthenticated,
    BootstrapOnly,
    Scoped,
}

/// Problem policy for one capability.
///
/// Iteration exposes only the finalized public contract. Runtime validation also
/// accepts staged failures that are implemented in the current local kernel but
/// remain reserved until their owning contract body is activated.
#[derive(Debug, Clone, Copy)]
pub struct CapabilityProblemPolicy {
    public: &'static [ProblemCode],
    staged: &'static [ProblemCode],
}

impl CapabilityProblemPolicy {
    const fn new(public: &'static [ProblemCode], staged: &'static [ProblemCode]) -> Self {
        Self { public, staged }
    }

    pub fn contains(self, code: &ProblemCode) -> bool {
        self.public.contains(code) || self.staged.contains(code)
    }

    pub fn iter(self) -> std::slice::Iter<'static, ProblemCode> {
        self.public.iter()
    }
}

macro_rules! define_capabilities {
    ($(($variant:ident, $contract_body:ident, $runtime_body:ident, $contract_state:ident, $runtime_availability:ident, $authorization:ident, [$($scope:ident),*], [$($problem:ident),)], [$($staged_problem:ident),)])),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum CapabilityKey {
            $($variant),+
        }

        impl CapabilityKey {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn contract_body(self) -> CapabilityBody {
                match self { $(Self::$variant => CapabilityBody::$contract_body),+ }
            }

            pub const fn runtime_body(self) -> CapabilityBody {
                match self { $(Self::$variant => CapabilityBody::$runtime_body),+ }
            }

            pub const fn contract_state(self) -> ContractState {
                match self { $(Self::$variant => ContractState::$contract_state),+ }
            }

            pub const fn runtime_availability(self) -> RuntimeAvailability {
                match self { $(Self::$variant => RuntimeAvailability::$runtime_availability),+ }
            }

            pub const fn authorization_kind(self) -> AuthorizationKind {
                match self { $(Self::$variant => AuthorizationKind::$authorization),+ }
            }

            pub const fn required_scopes(self) -> &'static [ScopeKey] {
                match self { $(Self::$variant => &[$(ScopeKey::$scope),*]),+ }
            }

            pub const fn allowed_problem_codes(self) -> CapabilityProblemPolicy {
                match self {
                    $(Self::$variant => CapabilityProblemPolicy::new(
                        &[$(ProblemCode::$problem),*],
                        &[$(ProblemCode::$staged_problem),*],
                    )),+
                }
            }

            pub const fn is_production_executable(self) -> bool {
                matches!(self.runtime_availability(), RuntimeAvailability::Implemented)
            }
        }
    };
}

// One application table owns capability lifecycle, authorization, scope, and
// problem semantics. Transport bindings map stable public IDs onto these keys.
define_capabilities!(
    (
        SystemHealth,
        B1,
        B0,
        Finalized,
        Implemented,
        Unauthenticated,
        [],
        [],
        []
    ),
    (
        DiscoverCapabilities,
        B1,
        B1,
        Finalized,
        FixtureOnly,
        Scoped,
        [CapabilityRead],
        [Forbidden],
        [AuthenticationFailed, IntegrityFailed, StorageUnavailable]
    ),
    (
        InitializeNode,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        BootstrapOnly,
        [],
        [
            Forbidden,
            MalformedJson,
            PayloadTooLarge,
            UnsupportedMediaType,
            ValidationFailed
        ],
        [AlreadyInitialized, IntegrityFailed, StorageUnavailable]
    ),
    (
        EnrollFirstClient,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [ClientEnroll],
        [
            Forbidden,
            MalformedJson,
            PayloadTooLarge,
            UnsupportedMediaType,
            ValidationFailed
        ],
        [BootstrapClosed, IntegrityFailed, StorageUnavailable]
    ),
    (
        SelectProfile,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [ProfileSelect],
        [CapabilityUnavailable, Forbidden],
        [IntegrityFailed, StorageUnavailable]
    ),
    (
        RotateCredential,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [CredentialManage],
        [CapabilityUnavailable, Forbidden],
        [IntegrityFailed, StorageUnavailable]
    ),
    (
        RevokeCredential,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [CredentialManage],
        [CapabilityUnavailable, Forbidden],
        [AuthenticationFailed, IntegrityFailed, StorageUnavailable]
    ),
    (
        ConfigureListener,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [ListenerConfigure],
        [CapabilityUnavailable, Forbidden],
        [IntegrityFailed, StorageUnavailable, UnsupportedListener]
    ),
    (
        AcceptObservation,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [ObservationAccept],
        [
            CapacityExceeded,
            Forbidden,
            IdempotencyConflict,
            InvalidObservation,
            MalformedJson,
            PayloadTooLarge,
            UnsupportedMediaType,
            ValidationFailed
        ],
        [EvidenceNotFound, IntegrityFailed, StorageUnavailable]
    ),
    (
        ReplayReceipt,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [ReceiptRead],
        [Forbidden, ReceiptNotFound],
        [IntegrityFailed, StorageUnavailable]
    ),
    (
        StreamReceipts,
        B1,
        B2,
        Finalized,
        FixtureOnly,
        Scoped,
        [ReceiptRead],
        [Forbidden, ReceiptNotFound],
        [CursorExpired, IntegrityFailed, StorageUnavailable]
    ),
    (
        CreateRecord,
        B2,
        B2,
        Reserved,
        LaterBody,
        Scoped,
        [IdentityWrite],
        [CapabilityUnavaile, InvalidIdentifier, ValidationFailed],
        [Forbidden, IntegrityFailed, StorageUnavailable]
    ),
    (
        AttachIdentifier,
        B2,
        B2,
        Reserved,
        LaterBody,
        Scoped,
        [IdentityWrite],
        [CapabilityUnavailable, InvalidIdentifier, ValidationFailed],
        [
            Forbidden,
            IdentityConflict,
            IntegrityFailed,
            RecordNotFound,
            StorageUnavailable
        ]
    ),
    (
        InspectReview,
        B2,
        B2,
        Reserved,
        LaterBody,
        Scoped,
        [ReviewRead],
        [CapabilityUnavailable, Forbidden],
        [IntegrityFailed, StorageUnavailable]
    ),
    (
        DeferReview,
        B2,
        B2,
        Reserved,
        LaterBody,
        Scoped,
        [ReviewWrite],
        [CapabilityUnavailable, Forbidden, ValidationFailed],
        [IntegrityFailed, ReviewNotFound, StorageUnavailable]
    ),
    (
        ResumeReview,
        B2,
        B2,
        Reserved,
        LaterBody,
        Scoped,
        [ReviewWrite],
        [CapabilityUnavailable, Forbidden, ValidationFailed],
        [IntegrityFailed, ReviewNotFound, StorageUnavailable]
    ),
    (
        ResolveReview,
        B2,
        B2,
        Reserved,
        LaterBody,
        Scoped,
        [ReviewWrite],
        [CapabilityUnavailable, Forbidden, ValidationFailed],
        [
            IdentityConflict,
            IntegrityFailed,
            InvalidIdentifier,
            RecordNotFound,
            ReviewNotFound,
            StorageUnavailable
        ]
    ),
    (
        AppendCorrection,
        B3,
        B3,
        Reserved,
        LaterBody,
        Scoped,
        [CorrectionWrite],
        [CapabilityUnavailable, Forbidden, ValidationFailed],
        []
    ),
    (
        InspectCorrectionChain,
        B3,
        B3,
        Reserved,
        LaterBody,
        Scoped,
        [CorrectionRead],
        [CapabilityUnavailable, Forbidden],
        []
    ),
    (
        ExportWorkspace,
        B3,
        B3,
        Reserved,
        Guarded,
        Scoped,
        [WorkspaceExport],
        [CapabilityUnavailable, Forbidden],
        []
    ),
    (
        RestoreWorkspace,
        B3,
        B3,
        Reserved,
        Guarded,
        Scoped,
        [WorkspaceRestore],
        [CapabilityUnavailable, Forbidden, ValidationFailed],
        []
    ),
    (
        VerifyWorkspace,
        B3,
        B3,
        Reserved,
        Guarded,
        Scoped,
        [WorkspaceVerify],
        [CapabilityUnavailable, Forbidden, ValidationFailed],
        []
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
    fn b2_runtime_capabilities_remain_non_production_until_activation() {
        for capability in [
            CapabilityKey::InitializeNode,
            CapabilityKey::AcceptObservation,
            CapabilityKey::CreateRecord,
            CapabilityKey::ResolveReview,
        ] {
            assert_eq!(capability.runtime_body(), CapabilityBody::B2);
            assert!(!capability.is_production_executable());
        }

        assert_eq!(
            CapabilityKey::InitializeNode.runtime_availability(),
            RuntimeAvailability::FixtureOnly
        );
        assert_eq!(
            CapabilityKey::CreateRecord.contract_state(),
            ContractState::Reserved
        );
        assert_eq!(
            CapabilityKey::CreateRecord.runtime_availability(),
            RuntimeAvailability::LaterBody
        );
    }

    #[test]
    fn b3_contracts_remain_reserved_or_guarded() {
        assert_eq!(
            CapabilityKey::AppendCorrection.runtime_availability(),
            RuntimeAvailability::LaterBody
        );
        assert_eq!(
            CapabilityKey::ExportWorkspace.runtime_availability(),
            RuntimeAvaility::Guarded
        );
    }

    #[test]
    fn staged_runtime_failures_are_checked_but_not_published() {
        let discovery = CapabilityKey::DiscoverCapabilities.allowed_problem_codes();
        assert!(discovery.contains(&ProblemCode::AuthenticationFailed));
        assert!(!discovery
            .iter()
            .any(|code| *code == ProblemCode::AuthenticationFailed));

        let enrollment = CapabilityKey::EnrollFirstClient.allowed_problem_codes();
        assert!(enrollment.contains(&ProblemCode::BootstrapClosed));
        assert!(!enrollment
            .iter()
            .any(|code| *code == ProblemCode::BootstrapClosed));

        let review = CapabilityKey::ResolveReview.allowed_problem_codes();
        assert!(review.contains(&ProblemCode::ReviewNotFound));
        assert!(!review
            .iter()
            .any(|code| code.contract_state() == ContractState::Reserved));
    }
}
