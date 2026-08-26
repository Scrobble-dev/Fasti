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
    LocalOperator,
    Scoped,
}

#[derive(Debug, Clone, Copy)]
pub struct CapabilityProblemPolicy {
    public: &'static [ProblemCode],
    staged: &'static [ProblemCode],
}

impl CapabilityProblemPolicy {
    const fn new(public: &'static [ProblemCode], staged: &'static [ProblemCode]) -> Self {
        Self { public, staged }
    }

    pub const fn public(self) -> &'static [ProblemCode] {
        self.public
    }

    pub const fn staged(self) -> &'static [ProblemCode] {
        self.staged
    }

    pub fn contains(self, code: &ProblemCode) -> bool {
        self.public().contains(code) || self.staged().contains(code)
    }

    pub fn iter(self) -> std::slice::Iter<'static, ProblemCode> {
        self.public().iter()
    }
}

macro_rules! define_capabilities {
    ($(($variant:ident, $contract_body:ident, $runtime_body:ident, $contract_state:ident, $runtime_availability:ident, $authorization:ident, [$($scope:ident),*], [$($problem:ident),*], [$($staged_problem:ident),*])),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum CapabilityKey { $($variant),+ }

        impl CapabilityKey {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn contract_body(self) -> CapabilityBody { match self { $(Self::$variant => CapabilityBody::$contract_body),+ } }
            pub const fn runtime_body(self) -> CapabilityBody { match self { $(Self::$variant => CapabilityBody::$runtime_body),+ } }
            pub const fn contract_state(self) -> ContractState { match self { $(Self::$variant => ContractState::$contract_state),+ } }
            pub const fn runtime_availability(self) -> RuntimeAvailability { match self { $(Self::$variant => RuntimeAvailability::$runtime_availability),+ } }
            pub const fn authorization_kind(self) -> AuthorizationKind { match self { $(Self::$variant => AuthorizationKind::$authorization),+ } }
            pub const fn required_scopes(self) -> &'static [ScopeKey] { match self { $(Self::$variant => &[$(ScopeKey::$scope),*]),+ } }
            pub const fn allowed_problem_codes(self) -> CapabilityProblemPolicy {
                match self {
                    $(Self::$variant => CapabilityProblemPolicy::new(&[$(ProblemCode::$problem),*], &[$(ProblemCode::$staged_problem),*])),+
                }
            }
            pub const fn is_production_executable(self) -> bool {
                matches!(self.runtime_availability(), RuntimeAvailability::Implemented)
            }
        }
    };
}

define_capabilities!(
    (SystemHealth, B1, B0, Finalized, Implemented, Unauthenticated, [], [], []),
    (DiscoverCapabilities, B1, B1, Finalized, FixtureOnly, Scoped, [CapabilityRead], [Forbidden], [AuthenticationFailed, IntegrityFailed, StorageUnavailable]),
    (InitializeNode, B1, B2, Finalized, Implemented, BootstrapOnly, [], [Forbidden, MalformedJson, PayloadTooLarge, UnsupportedMediaType, ValidationFailed, AlreadyInitialized, IntegrityFailed, StorageUnavailable], []),
    (EnrollFirstClient, B1, B2, Finalized, Implemented, Scoped, [ClientEnroll], [Forbidden, MalformedJson, PayloadTooLarge, UnsupportedMediaType, ValidationFailed, BootstrapClosed, IntegrityFailed, StorageUnavailable], []),
    (SelectProfile, B1, B2, Finalized, FixtureOnly, Scoped, [ProfileSelect], [CapabilityUnavailable, Forbidden], [AuthenticationFailed, IntegrityFailed, StorageUnavailable]),
    (RotateCredential, B1, B2, Finalized, FixtureOnly, Scoped, [CredentialManage], [CapabilityUnavailable, Forbidden], [AuthenticationFailed, IntegrityFailed, StorageUnavailable]),
    (RevokeCredential, B1, B2, Finalized, FixtureOnly, Scoped, [CredentialManage], [CapabilityUnavailable, Forbidden], [AuthenticationFailed, IntegrityFailed, StorageUnavailable]),
    (ConfigureListener, B1, B2, Finalized, FixtureOnly, Scoped, [ListenerConfigure], [CapabilityUnavailable, Forbidden], [AuthenticationFailed, IntegrityFailed, StorageUnavailable, UnsupportedListener]),
    (
        AcceptObservation,
        B1,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ObservationAccept],
        [CapacityExceeded, Forbidden, IdempotencyConflict, InvalidObservation, MalformedJson, PayloadTooLarge, UnsupportedMediaType, ValidationFailed],
        [AuthenticationFailed, EvidenceNotFound, IntegrityFailed, StorageUnavailable]
    ),
    (ReplayReceipt, B1, B2, Finalized, FixtureOnly, Scoped, [ReceiptRead], [Forbidden, ReceiptNotFound], [AuthenticationFailed, IntegrityFailed, StorageUnavailable]),
    (StreamReceipts, B1, B2, Finalized, FixtureOnly, Scoped, [ReceiptRead], [Forbidden, ReceiptNotFound], [AuthenticationFailed, CursorExpired, IntegrityFailed, StorageUnavailable]),
    (CreateRecord, B2, B2, Reserved, LaterBody, Scoped, [IdentityWrite], [CapabilityUnavailable, InvalidIdentifier, ValidationFailed], [AuthenticationFailed, Forbidden, IntegrityFailed, StorageUnavailable]),
    (AttachIdentifier, B2, B2, Reserved, LaterBody, Scoped, [IdentityWrite], [CapabilityUnavailable, InvalidIdentifier, ValidationFailed], [AuthenticationFailed, Forbidden, IdentityConflict, IntegrityFailed, RecordNotFound, StorageUnavailable]),
    (InspectReview, B2, B2, Reserved, LaterBody, Scoped, [ReviewRead], [CapabilityUnavailable, Forbidden], [AuthenticationFailed, IntegrityFailed, StorageUnavailable]),
    (DeferReview, B2, B2, Reserved, LaterBody, Scoped, [ReviewWrite], [CapabilityUnavailable, Forbidden, ValidationFailed], [AuthenticationFailed, IntegrityFailed, ReviewNotFound, StorageUnavailable]),
    (ResumeReview, B2, B2, Reserved, LaterBody, Scoped, [ReviewWrite], [CapabilityUnavailable, Forbidden, ValidationFailed], [AuthenticationFailed, IntegrityFailed, ReviewNotFound, StorageUnavailable]),
    (ResolveReview, B2, B2, Reserved, LaterBody, Scoped, [ReviewWrite], [CapabilityUnavailable, Forbidden, ValidationFailed], [AuthenticationFailed, IdentityConflict, IntegrityFailed, InvalidIdentifier, RecordNotFound, ReviewNotFound, StorageUnavailable]),
    (AppendCorrection, B3, B3, Reserved, LaterBody, Scoped, [CorrectionWrite], [CapabilityUnavailable, Forbidden, ValidationFailed], [AuthenticationFailed, IntegrityFailed, RecordNotFound, StorageUnavailable]),
    (InspectCorrectionChain, B3, B3, Reserved, LaterBody, Scoped, [CorrectionRead], [CapabilityUnavailable, Forbidden], [AuthenticationFailed, ValidationFailed, IntegrityFailed, StorageUnavailable]),
    (ExportWorkspace, B3, B3, Reserved, Guarded, Scoped, [WorkspaceExport], [CapabilityUnavailable, Forbidden], [AuthenticationFailed, CapacityExceeded, DataRootLocked, ExportCanceled, IntegrityFailed, StoppedNodeExportRequired, StorageUnavailable, UnsupportedPlatform]),
    (RestoreWorkspace, B3, B3, Reserved, Guarded, LocalOperator, [], [CapabilityUnavailable, Forbidden, ValidationFailed], [BootstrapClosed, CapacityExceeded, DataRootLocked, IntegrityFailed, OperationCanceled, RecoveryBootstrapPending, StorageUnavailable, UnsupportedPlatform]),
    (VerifyWorkspace, B3, B3, Reserved, Guarded, Scoped, [WorkspaceVerify], [CapabilityUnavailable, Forbidden, ValidationFailed], [AuthenticationFailed, DataRootLocked, IntegrityFailed, StorageUnavailable]),
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
    fn durable_local_activation_is_narrow() {
        for capability in [
            CapabilityKey::InitializeNode,
            CapabilityKey::EnrollFirstClient,
            CapabilityKey::AcceptObservation,
        ] {
            assert_eq!(capability.runtime_body(), CapabilityBody::B2);
            assert!(capability.is_production_executable());
        }

        for capability in [CapabilityKey::CreateRecord, CapabilityKey::ResolveReview] {
            assert_eq!(capability.runtime_body(), CapabilityBody::B2);
            assert!(!capability.is_production_executable());
        }

        assert_eq!(
            CapabilityKey::InitializeNode.runtime_availability(),
            RuntimeAvailability::Implemented
        );
    }
}
