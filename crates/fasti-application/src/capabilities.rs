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
        Implemented,
        BootstrapOnly,
        [],
        [
            Forbidden,
            MalformedJson,
            PayloadTooLarge,
            UnsupportedMediaType,
            ValidationFailed,
            AlreadyInitialized,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        EnrollFirstClient,
        B1,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ClientEnroll],
        [
            Forbidden,
            MalformedJson,
            PayloadTooLarge,
            UnsupportedMediaType,
            ValidationFailed,
            BootstrapClosed,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
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
        [AuthenticationFailed, IntegrityFailed, StorageUnavailable]
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
        [AuthenticationFailed, IntegrityFailed, StorageUnavailable]
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
        [
            AuthenticationFailed,
            IntegrityFailed,
            StorageUnavailable,
            UnsupportedListener
        ]
    ),
    (
        AcceptObservation,
        B1,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ObservationAccept],
        [
            AuthenticationFailed,
            CapacityExceeded,
            Forbidden,
            IdempotencyConflict,
            IntegrityFailed,
            InvalidObservation,
            MalformedJson,
            PayloadTooLarge,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        [EvidenceNotFound]
    ),
    (
        CreateBrowserSession,
        B2,
        B2,
        Finalized,
        Implemented,
        Unauthenticated,
        [],
        [
            AuthenticationFailed,
            IntegrityFailed,
            MalformedJson,
            PayloadTooLarge,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
    ),
    (
        ReadBrowserSession,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [],
        [
            AuthenticationFailed,
            Forbidden,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        EndBrowserSession,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [],
        [
            AuthenticationFailed,
            Forbidden,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        ListBrowserUsers,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [BrowserUserManage],
        [
            AuthenticationFailed,
            Forbidden,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        UpdateBrowserUser,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [BrowserUserManage],
        [
            AuthenticationFailed,
            Forbidden,
            IntegrityFailed,
            MalformedJson,
            PayloadTooLarge,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
    ),
    (
        DeleteBrowserUser,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [BrowserUserManage],
        [
            AuthenticationFailed,
            Forbidden,
            IntegrityFailed,
            MalformedJson,
            PayloadTooLarge,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
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
        [AuthenticationFailed, IntegrityFailed, StorageUnavailable]
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
        [
            AuthenticationFailed,
            CursorExpired,
            IntegrityFailed,
            StorageUnavailable
        ]
    ),
    (
        CreateRecord,
        B1,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [IdentityWrite],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            InvalidIdentifier,
            MalformedJson,
            PayloadTooLarge,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
    ),
    (
        AttachIdentifier,
        B1,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [IdentityWrite],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IdentityConflict,
            IntegrityFailed,
            InvalidIdentifier,
            MalformedJson,
            PayloadTooLarge,
            RecordNotFound,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
    ),
    (
        ListRecords,
        B1,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [IdentityRead],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        RegisterNamespace,
        B1,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [IdentityWrite],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            MalformedJson,
            PayloadTooLarge,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
    ),
    (
        GetNuvioCollections,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ProfileStateRead],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        ReplaceNuvioCollections,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ProfileStateWrite],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            MalformedJson,
            PayloadTooLarge,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
    ),
    (
        ClearNuvioCollections,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ProfileStateWrite],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        ListTrackingDispositions,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ProfileStateRead],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            StorageUnavailable
        ],
        []
    ),
    (
        SetTrackingDisposition,
        B2,
        B2,
        Finalized,
        Implemented,
        Scoped,
        [ProfileStateWrite],
        [
            AuthenticationFailed,
            CapabilityUnavailable,
            Forbidden,
            IntegrityFailed,
            MalformedJson,
            PayloadTooLarge,
            RecordNotFound,
            StorageUnavailable,
            UnsupportedMediaType,
            ValidationFailed
        ],
        []
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
        [AuthenticationFailed, IntegrityFailed, StorageUnavailable]
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
        [
            AuthenticationFailed,
            IntegrityFailed,
            ReviewNotFound,
            StorageUnavailable
        ]
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
        [
            AuthenticationFailed,
            IntegrityFailed,
            ReviewNotFound,
            StorageUnavailable
        ]
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
            AuthenticationFailed,
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
        [
            AuthenticationFailed,
            IntegrityFailed,
            RecordNotFound,
            StorageUnavailable
        ]
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
        [
            AuthenticationFailed,
            ValidationFailed,
            IntegrityFailed,
            StorageUnavailable
        ]
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
        [
            AuthenticationFailed,
            CapacityExceeded,
            DataRootLocked,
            ExportCanceled,
            IntegrityFailed,
            StoppedNodeExportRequired,
            StorageUnavailable,
            UnsupportedPlatform
        ]
    ),
    (
        RestoreWorkspace,
        B3,
        B3,
        Reserved,
        Guarded,
        LocalOperator,
        [],
        [CapabilityUnavailable, Forbidden, ValidationFailed],
        [
            BootstrapClosed,
            CapacityExceeded,
            DataRootLocked,
            IntegrityFailed,
            OperationCanceled,
            RecoveryBootstrapPending,
            StorageUnavailable,
            UnsupportedPlatform
        ]
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
        [
            AuthenticationFailed,
            DataRootLocked,
            IntegrityFailed,
            StorageUnavailable
        ]
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
    fn durable_local_activation_is_narrow() {
        for capability in [
            CapabilityKey::InitializeNode,
            CapabilityKey::EnrollFirstClient,
            CapabilityKey::AcceptObservation,
            CapabilityKey::CreateRecord,
            CapabilityKey::AttachIdentifier,
            CapabilityKey::ListRecords,
            CapabilityKey::RegisterNamespace,
        ] {
            assert_eq!(capability.runtime_body(), CapabilityBody::B2);
            assert!(capability.is_production_executable());
        }

        let capability = CapabilityKey::ResolveReview;
        assert_eq!(capability.runtime_body(), CapabilityBody::B2);
        assert!(!capability.is_production_executable());

        assert_eq!(
            CapabilityKey::InitializeNode.runtime_availability(),
            RuntimeAvailability::Implemented
        );
        assert_eq!(
            CapabilityKey::CreateRecord.contract_state(),
            ContractState::Finalized
        );
        assert_eq!(
            CapabilityKey::CreateRecord.runtime_availability(),
            RuntimeAvailability::Implemented
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
            RuntimeAvailability::Guarded
        );
        assert_eq!(
            CapabilityKey::RestoreWorkspace.runtime_availability(),
            RuntimeAvailability::Guarded
        );
        assert_eq!(
            CapabilityKey::RestoreWorkspace.authorization_kind(),
            AuthorizationKind::LocalOperator
        );
        assert!(CapabilityKey::RestoreWorkspace.required_scopes().is_empty());
        assert_eq!(
            CapabilityKey::ExportWorkspace.authorization_kind(),
            AuthorizationKind::Scoped
        );
        assert_eq!(
            CapabilityKey::ExportWorkspace.required_scopes(),
            &[ScopeKey::WorkspaceExport]
        );
        assert_eq!(
            CapabilityKey::VerifyWorkspace.authorization_kind(),
            AuthorizationKind::Scoped
        );
        assert_eq!(
            CapabilityKey::VerifyWorkspace.required_scopes(),
            &[ScopeKey::WorkspaceVerify]
        );

        let local_operator_capabilities: Vec<_> = CapabilityKey::ALL
            .iter()
            .copied()
            .filter(|capability| {
                capability.authorization_kind() == AuthorizationKind::LocalOperator
            })
            .collect();
        assert_eq!(
            local_operator_capabilities,
            [CapabilityKey::RestoreWorkspace]
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
        assert!(enrollment
            .iter()
            .any(|code| *code == ProblemCode::BootstrapClosed));

        let observation = CapabilityKey::AcceptObservation.allowed_problem_codes();
        for code in [
            ProblemCode::AuthenticationFailed,
            ProblemCode::IntegrityFailed,
            ProblemCode::StorageUnavailable,
        ] {
            assert!(observation.contains(&code));
            assert!(observation.iter().any(|published| *published == code));
        }
        assert!(observation.contains(&ProblemCode::EvidenceNotFound));
        assert!(!observation
            .iter()
            .any(|published| *published == ProblemCode::EvidenceNotFound));

        let review = CapabilityKey::InspectReview.allowed_problem_codes();
        assert!(review.contains(&ProblemCode::AuthenticationFailed));
        assert!(!review
            .iter()
            .any(|code| *code == ProblemCode::AuthenticationFailed));

        for capability in CapabilityKey::ALL.iter().copied().filter(|capability| {
            capability.authorization_kind() == AuthorizationKind::Scoped
                && *capability != CapabilityKey::EnrollFirstClient
        }) {
            assert!(
                capability
                    .allowed_problem_codes()
                    .contains(&ProblemCode::AuthenticationFailed),
                "{capability:?} must accept route-attributed authentication failures"
            );
        }

        let review = CapabilityKey::ResolveReview.allowed_problem_codes();
        assert!(review.contains(&ProblemCode::ReviewNotFound));
        assert!(!review
            .iter()
            .any(|code| code.contract_state() == ContractState::Reserved));

        let export = CapabilityKey::ExportWorkspace.allowed_problem_codes();
        for code in [
            ProblemCode::CapacityExceeded,
            ProblemCode::DataRootLocked,
            ProblemCode::ExportCanceled,
            ProblemCode::IntegrityFailed,
            ProblemCode::StoppedNodeExportRequired,
            ProblemCode::StorageUnavailable,
            ProblemCode::UnsupportedPlatform,
        ] {
            assert!(export.contains(&code));
            assert!(!export.iter().any(|published| *published == code));
        }

        let restore = CapabilityKey::RestoreWorkspace.allowed_problem_codes();
        for code in [
            ProblemCode::BootstrapClosed,
            ProblemCode::OperationCanceled,
            ProblemCode::RecoveryBootstrapPending,
            ProblemCode::UnsupportedPlatform,
        ] {
            assert!(restore.contains(&code));
            assert!(!restore.iter().any(|published| *published == code));
        }

        let verify = CapabilityKey::VerifyWorkspace.allowed_problem_codes();
        assert!(verify.contains(&ProblemCode::DataRootLocked));
        assert!(!verify
            .iter()
            .any(|published| *published == ProblemCode::DataRootLocked));
    }

    #[test]
    fn every_problem_policy_is_unique_disjoint_and_explicit() {
        fn assert_unique(capability: CapabilityKey, set_name: &str, codes: &[ProblemCode]) {
            for (index, code) in codes.iter().enumerate() {
                assert!(
                    !codes[index + 1..].contains(code),
                    "{capability:?} has duplicate {set_name} problem {}",
                    code.as_str()
                );
            }
        }

        for capability in CapabilityKey::ALL {
            let policy = capability.allowed_problem_codes();
            assert_unique(*capability, "public", policy.public());
            assert_unique(*capability, "staged", policy.staged());

            for code in policy.public() {
                assert!(
                    !policy.staged().contains(code),
                    "{capability:?} exposes problem {} as both public and staged",
                    code.as_str()
                );
                assert!(policy.contains(code));
            }

            for code in policy.staged() {
                assert!(policy.contains(code));
                assert!(
                    !policy.iter().any(|published| published == code),
                    "{capability:?} publishes staged problem {}",
                    code.as_str()
                );
            }

            assert_eq!(
                policy.iter().copied().collect::<Vec<_>>().as_slice(),
                policy.public(),
                "{capability:?} public iteration must remain exact"
            );
        }
    }
}
