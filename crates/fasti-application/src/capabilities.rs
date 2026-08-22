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

macro_rules! define_capabilities {
    ($(($variant:ident, $contract_body:ident, $runtime_body:ident, $contract_state:ident, $runtime_availability:ident, $authorization:ident, [$($scope:ident),*], [$($problem:ident),*])),+ $(,)?) => {
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

            pub const fn allowed_problem_codes(self) -> &'static [ProblemCode] {
                match self { $(Self::$variant => &[$(ProblemCode::$problem),*]),+ }
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
    (SystemHealth, B1, B0, Finalized, Implemented, Unauthenticated, [], []),
    (DiscoverCapabilities, B1, B2, Finalized, Implemented, Scoped, [CapabilityRead], [AuthenticationFailed, Forbidden, StorageUnavailable]),
    (InitializeNode, B1, B2, Finalized, Implemented, BootstrapOnly, [], [AlreadyInitialized, CapacityExceeded, StorageUnavailable, ValidationFailed]),
    (EnrollFirstClient, B1, B2, Finalized, Implemented, BootstrapOnly, [], [AuthenticationFailed, BootstrapClosed, StorageUnavailable, ValidationFailed]),
    (SelectProfile, B1, B2, Finalized, Implemented, Scoped, [ProfileSelect], [AuthenticationFailed, Forbidden, StorageUnavailable]),
    (RotateCredential, B1, B2, Finalized, Implemented, Scoped, [CredentialManage], [AuthenticationFailed, Forbidden, StorageUnavailable]),
    (RevokeCredential, B1, B2, Finalized, Implemented, Scoped, [CredentialManage], [AuthenticationFailed, Forbidden, StorageUnavailable]),
    (ConfigureListener, B1, B2, Finalized, Implemented, Scoped, [ListenerConfigure], [AuthenticationFailed, Forbidden, StorageUnavailable, UnsupportedListener, ValidationFailed]),
    (UploadEvidence, B2, B2, Finalized, Implemented, Scoped, [ObservationAccept], [AuthenticationFailed, CapacityExceeded, Forbidden, IntegrityFailed, PayloadTooLarge, StorageUnavailable]),
    (AcceptObservation, B1, B2, Finalized, Implemented, Scoped, [ObservationAccept], [AuthenticationFailed, CapacityExceeded, EvidenceNotFound, Forbidden, IdempotencyConflict, IntegrityFailed, InvalidObservation, MalformedJson, PayloadTooLarge, StorageUnavailable, UnsupportedMediaType, ValidationFailed]),
    (ReplayReceipt, B1, B2, Finalized, Implemented, Scoped, [ReceiptRead], [AuthenticationFailed, Forbidden, ReceiptNotFound, StorageUnavailable]),
    (StreamReceipts, B1, B2, Finalized, Implemented, Scoped, [ReceiptRead], [AuthenticationFailed, CursorExpired, Forbidden, ReceiptNotFound, StorageUnavailable]),
    (CreateRecord, B2, B2, Finalized, Implemented, Scoped, [IdentityWrite], [AuthenticationFailed, Forbidden, InvalidIdentifier, StorageUnavailable, ValidationFailed]),
    (AttachIdentifier, B2, B2, Finalized, Implemented, Scoped, [IdentityWrite], [AuthenticationFailed, Forbidden, IdentityConflict, InvalidIdentifier, RecordNotFound, StorageUnavailable, ValidationFailed]),
    (ApplyIdentitySeed, B2, B2, Finalized, Implemented, Scoped, [IdentityWrite], [AuthenticationFailed, Forbidden, IdentityConflict, InvalidIdentifier, StorageUnavailable, ValidationFailed]),
    (InspectReview, B2, B2, Finalized, Implemented, Scoped, [ReviewRead], [AuthenticationFailed, Forbidden, ReviewNotFound, StorageUnavailable]),
    (DeferReview, B2, B2, Finalized, Implemented, Scoped, [ReviewWrite], [AuthenticationFailed, Forbidden, ReviewNotFound, StorageUnavailable, ValidationFailed]),
    (ResumeReview, B2, B2, Finalized, Implemented, Scoped, [ReviewWrite], [AuthenticationFailed, Forbidden, ReviewNotFound, StorageUnavailable, ValidationFailed]),
    (ResolveReview, B2, B2, Finalized, Implemented, Scoped, [ReviewWrite], [AuthenticationFailed, Forbidden, IdentityConflict, InvalidIdentifier, RecordNotFound, ReviewNotFound, StorageUnavailable, ValidationFailed]),
    (AppendCorrection, B3, B3, Reserved, LaterBody, Scoped, [CorrectionWrite], [CapabilityUnavailable, Forbidden, ValidationFailed]),
    (InspectCorrectionChain, B3, B3, Reserved, LaterBody, Scoped, [CorrectionRead], [CapabilityUnavailable, Forbidden]),
    (ExportWorkspace, B3, B3, Reserved, Guarded, Scoped, [WorkspaceExport], [CapabilityUnavailable, Forbidden]),
    (RestoreWorkspace, B3, B3, Reserved, Guarded, Scoped, [WorkspaceRestore], [CapabilityUnavailable, Forbidden, ValidationFailed]),
    (VerifyWorkspace, B3, B3, Reserved, Guarded, Scoped, [WorkspaceVerify], [CapabilityUnavailable, Forbidden, ValidationFailed]),
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
    fn b2_runtime_capabilities_are_explicit() {
        for capability in [
            CapabilityKey::InitializeNode,
            CapabilityKey::UploadEvidence,
            CapabilityKey::AcceptObservation,
            CapabilityKey::CreateRecord,
            CapabilityKey::ResolveReview,
        ] {
            assert_eq!(capability.runtime_body(), CapabilityBody::B2);
            assert_eq!(capability.contract_state(), ContractState::Finalized);
            assert_eq!(
                capability.runtime_availability(),
                RuntimeAvailability::Implemented
            );
        }
    }

    #[test]
    fn b3_contracts_remain_reserved_or_guarded() {
        assert_eq!(
            CapabilityKey::AppendCorrection.runtime_availability(),
            RuntimeAvailability::LaterBody
        );
        assert_eq!(
            CapabilityKey::ExportWorkspace.runtime_availability(),
            RuntimeAvailability::Guarded
        );
    }
}
