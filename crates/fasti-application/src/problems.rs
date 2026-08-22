use crate::{CapabilityBody, CapabilityKey, ContractState};
use fasti_domain::RequestCorrelationId;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt;

macro_rules! define_string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        pub enum $name { $(#[serde(rename = $value)] $variant),+ }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }
        }
    };
}

define_string_enum!(SafeState {
    NoMutation => "no_mutation",
    PriorStateRetained => "prior_state_retained",
    UnresolvedEvidenceRetained => "unresolved_evidence_retained",
});

define_string_enum!(Retryability {
    NotRetryable => "not_retryable",
    RetryAfterCorrection => "retry_after_correction",
    RetrySafe => "retry_safe",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProblemDetail {
    Static(&'static str),
    RuntimeBodyOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemParamPolicy {
    None,
    Fixed(&'static str),
    ReceiptIdentifierByCapability,
}

impl ProblemParamPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Fixed(_) => "fixed",
            Self::ReceiptIdentifierByCapability => "receipt_identifier_by_capability",
        }
    }

    pub const fn resolve(self, capability: CapabilityKey) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Fixed(pointer) => Some(pointer),
            Self::ReceiptIdentifierByCapability => Some(match capability {
                CapabilityKey::StreamReceipts => "/last_event_id",
                _ => "/receipt_id",
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextActionContract {
    id: &'static str,
    label: &'static str,
}

impl NextActionContract {
    pub const fn id(self) -> &'static str {
        self.id
    }
    pub const fn label(self) -> &'static str {
        self.label
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProblemContract {
    title: &'static str,
    status: u16,
    detail: ProblemDetail,
    documentation_path: &'static str,
    safe_state: SafeState,
    retryability: Retryability,
    default_next_action: NextActionContract,
    param_policy: ProblemParamPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepresentationViolationContract {
    code: &'static str,
    pointer: &'static str,
    reason: &'static str,
    expected: &'static str,
}

impl RepresentationViolationContract {
    pub const fn code(self) -> &'static str {
        self.code
    }
    pub const fn pointer(self) -> &'static str {
        self.pointer
    }
    pub const fn reason(self) -> &'static str {
        self.reason
    }
    pub const fn expected(self) -> &'static str {
        self.expected
    }
}

impl ProblemContract {
    pub const fn title(self) -> &'static str {
        self.title
    }
    pub const fn status(self) -> u16 {
        self.status
    }
    pub fn detail(self, capability: CapabilityKey) -> Cow<'static, str> {
        match self.detail {
            ProblemDetail::Static(detail) => Cow::Borrowed(detail),
            ProblemDetail::RuntimeBodyOwner => Cow::Owned(format!(
                "requested capability is not available in this body; it is owned by {}",
                capability.runtime_body().as_str().to_ascii_lowercase()
            )),
        }
    }
    pub const fn documentation_path(self) -> &'static str {
        self.documentation_path
    }
    pub const fn safe_state(self) -> SafeState {
        self.safe_state
    }
    pub const fn retryability(self) -> Retryability {
        self.retryability
    }
    pub const fn default_next_action(self) -> NextActionContract {
        self.default_next_action
    }
    pub const fn param_policy(self) -> ProblemParamPolicy {
        self.param_policy
    }
}

macro_rules! define_problem_catalog {
    ($($variant:ident => $code:literal {
        title: $title:literal,
        status: $status:literal,
        detail: $detail:expr,
        documentation_path: $documentation_path:literal,
        safe_state: $safe_state:ident,
        retryability: $retryability:ident,
        default_next_action: ($action_id:literal, $action_label:literal),
        param_policy: $param_policy:expr
    }),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        pub enum ProblemCode { $(#[serde(rename = $code)] $variant),+ }

        impl ProblemCode {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
            }
            pub fn from_code(value: &str) -> Option<Self> {
                match value { $($code => Some(Self::$variant)),+, _ => None }
            }
            pub const fn contract(self) -> ProblemContract {
                match self {
                    $(Self::$variant => ProblemContract {
                        title: $title,
                        status: $status,
                        detail: $detail,
                        documentation_path: $documentation_path,
                        safe_state: SafeState::$safe_state,
                        retryability: Retryability::$retryability,
                        default_next_action: NextActionContract { id: $action_id, label: $action_label },
                        param_policy: $param_policy,
                    }),+
                }
            }
            pub const fn representation_violation(self) -> Option<RepresentationViolationContract> {
                match self {
                    Self::MalformedJson => Some(RepresentationViolationContract {
                        code: "invalid_representation", pointer: "/", reason: "request JSON is malformed", expected: "well-formed JSON",
                    }),
                    Self::PayloadTooLarge => Some(RepresentationViolationContract {
                        code: "invalid_representation", pointer: "/", reason: "request body exceeds its bounded limit", expected: "a request within the documented byte limit",
                    }),
                    Self::UnsupportedMediaType => Some(RepresentationViolationContract {
                        code: "invalid_representation", pointer: "/", reason: "request media type is unsupported", expected: "the documented request media type",
                    }),
                    Self::ValidationFailed => Some(RepresentationViolationContract {
                        code: "invalid_representation", pointer: "/", reason: "request JSON does not match the governed schema", expected: "the documented request schema",
                    }),
                    _ => None,
                }
            }
        }
    };
}

define_problem_catalog!(
    AlreadyInitialized => "already_initialized" {
        title: "Node already initialized", status: 409,
        detail: ProblemDetail::Static("the local node has already completed its one-time initialization"),
        documentation_path: "v1/problems/already-initialized", safe_state: PriorStateRetained,
        retryability: NotRetryable,
        default_next_action: ("use_existing_node", "Use the existing node and enrolled client"),
        param_policy: ProblemParamPolicy::None
    },
    AuthenticationFailed => "authentication_failed" {
        title: "Authentication failed", status: 401,
        detail: ProblemDetail::Static("the presented local credential is not active"),
        documentation_path: "v1/problems/authentication-failed", safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("use_active_credential", "Use an active local credential or enroll again"),
        param_policy: ProblemParamPolicy::None
    },
    BootstrapClosed => "bootstrap_closed" {
        title: "Bootstrap closed", status: 409,
        detail: ProblemDetail::Static("the one-time enrollment proof is invalid, expired, or already consumed"),
        documentation_path: "v1/problems/bootstrap-closed", safe_state: PriorStateRetained,
        retryability: NotRetryable,
        default_next_action: ("inspect_node_status", "Inspect the local node status before retrying"),
        param_policy: ProblemParamPolicy::None
    },
    CapacityExceeded => "capacity_exceeded" {
        title: "Capacity exceeded", status: 507,
        detail: ProblemDetail::Static("bounded application capacity has been reached"),
        documentation_path: "v1/problems/capacity-exceeded", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("release_capacity", "Release retained capacity before retrying"),
        param_policy: ProblemParamPolicy::None
    },
    CapabilityUnavailable => "capability_unavailable" {
        title: "Capability unavailable", status: 501,
        detail: ProblemDetail::RuntimeBodyOwner,
        documentation_path: "v1/problems/capability-unavailable", safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("review_capability_status", "Review the local capability registry"),
        param_policy: ProblemParamPolicy::None
    },
    CursorExpired => "cursor_expired" {
        title: "Receipt cursor expired", status: 410,
        detail: ProblemDetail::Static("the receipt cursor is outside the retained authorized range"),
        documentation_path: "v1/problems/cursor-expired", safe_state: PriorStateRetained,
        retryability: RetryAfterCorrection,
        default_next_action: ("restart_receipt_replay", "Start from an available receipt cursor"),
        param_policy: ProblemParamPolicy::Fixed("/last_event_id")
    },
    EvidenceNotFound => "evidence_not_found" {
        title: "Evidence not found", status: 404,
        detail: ProblemDetail::Static("the referenced evidence is not available in this workspace"),
        documentation_path: "v1/problems/evidence-not-found", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("upload_evidence", "Upload the evidence and retry the observation"),
        param_policy: ProblemParamPolicy::Fixed("/evidence/evidence_id")
    },
    Forbidden => "forbidden" {
        title: "Forbidden", status: 403,
        detail: ProblemDetail::Static("request is not authorized for this capability"),
        documentation_path: "v1/problems/forbidden", safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("verify_request_authorization", "Verify the request context and local grant"),
        param_policy: ProblemParamPolicy::None
    },
    IdempotencyConflict => "idempotency_conflict" {
        title: "Idempotency conflict", status: 409,
        detail: ProblemDetail::Static("operation ID was already used with different request semantics"),
        documentation_path: "v1/problems/idempotency-conflict", safe_state: PriorStateRetained,
        retryability: RetryAfterCorrection,
        default_next_action: ("use_new_operation_id", "Use a new operation ID for a distinct observation"),
        param_policy: ProblemParamPolicy::Fixed("/operation_id")
    },
    IdentityConflict => "identity_conflict" {
        title: "Identity conflict", status: 409,
        detail: ProblemDetail::Static("an exact external identifier is already attached to another active Record"),
        documentation_path: "v1/problems/identity-conflict", safe_state: PriorStateRetained,
        retryability: RetryAfterCorrection,
        default_next_action: ("review_identity_conflict", "Review the existing Record before attaching the identifier"),
        param_policy: ProblemParamPolicy::Fixed("/identifier")
    },
    IntegrityFailed => "integrity_failed" {
        title: "Integrity check failed", status: 500,
        detail: ProblemDetail::Static("stored evidence or durable state did not satisfy its recorded digest and reference invariants"),
        documentation_path: "v1/problems/integrity-failed", safe_state: PriorStateRetained,
        retryability: NotRetryable,
        default_next_action: ("run_local_integrity_check", "Stop the mutation and run the local integrity check"),
        param_policy: ProblemParamPolicy::None
    },
    InvalidIdentifier => "invalid_identifier" {
        title: "Invalid identifier", status: 422,
        detail: ProblemDetail::Static("identifier does not satisfy the governed format"),
        documentation_path: "v1/problems/invalid-identifier", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_identifier", "Correct the identifier and retry"),
        param_policy: ProblemParamPolicy::Fixed("/identifier")
    },
    MalformedJson => "malformed_json" {
        title: "Malformed JSON", status: 400,
        detail: ProblemDetail::Static("request JSON is malformed"),
        documentation_path: "v1/problems/malformed-json", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_json", "Correct the JSON syntax and retry"),
        param_policy: ProblemParamPolicy::None
    },
    InvalidObservation => "invalid_observation" {
        title: "Invalid observation", status: 422,
        detail: ProblemDetail::Static("observation does not satisfy the governed contract"),
        documentation_path: "v1/problems/invalid-observation", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_observation", "Correct the reported fields and submit again"),
        param_policy: ProblemParamPolicy::None
    },
    PayloadTooLarge => "payload_too_large" {
        title: "Payload too large", status: 413,
        detail: ProblemDetail::Static("request body exceeds the bounded transport limit"),
        documentation_path: "v1/problems/payload-too-large", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("reduce_request_body", "Reduce the request body and retry"),
        param_policy: ProblemParamPolicy::None
    },
    ReceiptNotFound => "receipt_not_found" {
        title: "Receipt not found", status: 404,
        detail: ProblemDetail::Static("no receipt is available for the requested identifier"),
        documentation_path: "v1/problems/receipt-not-found", safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("verify_receipt_id", "Verify the receipt ID and request context"),
        param_policy: ProblemParamPolicy::ReceiptIdentifierByCapability
    },
    RecordNotFound => "record_not_found" {
        title: "Record not found", status: 404,
        detail: ProblemDetail::Static("no active Record is available for the requested identifier"),
        documentation_path: "v1/problems/record-not-found", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("verify_record_id", "Verify the Record ID or create a new Record"),
        param_policy: ProblemParamPolicy::Fixed("/record_id")
    },
    ReviewNotFound => "review_not_found" {
        title: "Review item not found", status: 404,
        detail: ProblemDetail::Static("no authorized review item is available for the requested identifier"),
        documentation_path: "v1/problems/review-not-found", safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("inspect_review_queue", "Inspect the current review queue"),
        param_policy: ProblemParamPolicy::Fixed("/review_item_id")
    },
    StorageUnavailable => "storage_unavailable" {
        title: "Storage unavailable", status: 503,
        detail: ProblemDetail::Static("the local durability boundary is temporarily unavailable"),
        documentation_path: "v1/problems/storage-unavailable", safe_state: NoMutation,
        retryability: RetrySafe,
        default_next_action: ("retry_local_operation", "Check local storage and retry the same safe operation"),
        param_policy: ProblemParamPolicy::None
    },
    UnsupportedListener => "unsupported_listener" {
        title: "Listener configuration unsupported", status: 422,
        detail: ProblemDetail::Static("the requested listener would cross an unproven remote trust boundary"),
        documentation_path: "v1/problems/unsupported-listener", safe_state: PriorStateRetained,
        retryability: NotRetryable,
        default_next_action: ("use_loopback_listener", "Use the loopback listener until remote transport is proven"),
        param_policy: ProblemParamPolicy::Fixed("/loopback_port")
    },
    ValidationFailed => "validation_failed" {
        title: "Validation failed", status: 422,
        detail: ProblemDetail::Static("request representation does not satisfy the governed contract"),
        documentation_path: "v1/problems/validation-failed", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_request", "Correct the request representation and retry"),
        param_policy: ProblemParamPolicy::None
    },
    UnsupportedMediaType => "unsupported_media_type" {
        title: "Unsupported media type", status: 415,
        detail: ProblemDetail::Static("request media type is unsupported"),
        documentation_path: "v1/problems/unsupported-media-type", safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("use_supported_media_type", "Use the documented media type and retry"),
        param_policy: ProblemParamPolicy::None
    }
);

impl ProblemCode {
    pub const fn introduced_in(self) -> CapabilityBody {
        match self {
            Self::AlreadyInitialized
            | Self::AuthenticationFailed
            | Self::BootstrapClosed
            | Self::CursorExpired
            | Self::EvidenceNotFound
            | Self::IdentityConflict
            | Self::IntegrityFailed
            | Self::RecordNotFound
            | Self::ReviewNotFound
            | Self::StorageUnavailable
            | Self::UnsupportedListener => CapabilityBody::B2,
            _ => CapabilityBody::B1,
        }
    }

    pub const fn contract_state(self) -> ContractState {
        match self.introduced_in() {
            CapabilityBody::B0 | CapabilityBody::B1 => ContractState::Finalized,
            CapabilityBody::B2 | CapabilityBody::B3 => ContractState::Reserved,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextAction {
    id: String,
    label: String,
}
impl NextAction {
    fn from_contract(contract: NextActionContract) -> Self {
        Self {
            id: contract.id().to_owned(),
            label: contract.label().to_owned(),
        }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    code: String,
    pointer: String,
    reason: String,
    expected: String,
}
impl Violation {
    pub fn try_new(
        code: impl Into<String>,
        pointer: impl Into<String>,
        reason: impl Into<String>,
        expected: impl Into<String>,
    ) -> Result<Self, ProblemBuildError> {
        let code = code.into();
        let pointer = pointer.into();
        let reason = reason.into();
        let expected = expected.into();
        if code.trim().is_empty()
            || !is_valid_json_pointer(&pointer)
            || reason.trim().is_empty()
            || expected.trim().is_empty()
        {
            return Err(ProblemBuildError::InvalidViolation);
        }
        Ok(Self {
            code,
            pointer,
            reason,
            expected,
        })
    }
    pub fn code(&self) -> &str {
        &self.code
    }
    pub fn pointer(&self) -> &str {
        &self.pointer
    }
    pub fn reason(&self) -> &str {
        &self.reason
    }
    pub fn expected(&self) -> &str {
        &self.expected
    }
    pub fn actual(&self) -> Option<&str> {
        None
    }
}

fn is_valid_json_pointer(pointer: &str) -> bool {
    if !pointer.starts_with('/') {
        return false;
    }
    let mut characters = pointer.chars();
    while let Some(character) = characters.next() {
        if character == '~' && !matches!(characters.next(), Some('0' | '1')) {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemBuildError {
    TooManyViolations,
    InvalidViolation,
}
impl fmt::Display for ProblemBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooManyViolations => "a problem may expose no more than 32 validation violations",
            Self::InvalidViolation => {
                "violation fields must be non-empty and pointer must be a JSON Pointer"
            }
        })
    }
}
impl std::error::Error for ProblemBuildError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastiProblem {
    code: ProblemCode,
    capability: CapabilityKey,
    next_actions: Vec<NextAction>,
    correlation_id: RequestCorrelationId,
    violations: Vec<Violation>,
}

impl FastiProblem {
    fn new(
        code: ProblemCode,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        assert!(
            capability.allowed_problem_codes().contains(&code),
            "problem {} is not allowed for capability {capability:?}",
            code.as_str()
        );
        let action = code.contract().default_next_action();
        Self {
            code,
            capability,
            next_actions: vec![NextAction::from_contract(action)],
            correlation_id,
            violations: Vec::new(),
        }
    }

    fn with_violations(
        code: ProblemCode,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self::new(code, capability, correlation_id).try_with_violations(violations)
    }

    pub fn from_code(
        code: ProblemCode,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(code, capability, correlation_id)
    }

    pub fn capacity_exceeded(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::CapacityExceeded, capability, correlation_id)
    }
    pub fn capability_unavailable(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(
            ProblemCode::CapabilityUnavailable,
            capability,
            correlation_id,
        )
    }
    pub fn forbidden(capability: CapabilityKey, correlation_id: RequestCorrelationId) -> Self {
        Self::new(ProblemCode::Forbidden, capability, correlation_id)
    }
    pub fn authentication_failed(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(
            ProblemCode::AuthenticationFailed,
            capability,
            correlation_id,
        )
    }
    pub fn already_initialized(correlation_id: RequestCorrelationId) -> Self {
        Self::new(
            ProblemCode::AlreadyInitialized,
            CapabilityKey::InitializeNode,
            correlation_id,
        )
    }
    pub fn bootstrap_closed(correlation_id: RequestCorrelationId) -> Self {
        Self::new(
            ProblemCode::BootstrapClosed,
            CapabilityKey::EnrollFirstClient,
            correlation_id,
        )
    }
    pub fn idempotency_conflict(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::IdempotencyConflict, capability, correlation_id)
    }
    pub fn evidence_not_found(correlation_id: RequestCorrelationId) -> Self {
        Self::new(
            ProblemCode::EvidenceNotFound,
            CapabilityKey::AcceptObservation,
            correlation_id,
        )
    }
    pub fn identity_conflict(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::IdentityConflict, capability, correlation_id)
    }
    pub fn integrity_failed(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::IntegrityFailed, capability, correlation_id)
    }
    pub fn invalid_identifier(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::InvalidIdentifier, capability, correlation_id)
    }
    pub fn record_not_found(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::RecordNotFound, capability, correlation_id)
    }
    pub fn review_not_found(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::ReviewNotFound, capability, correlation_id)
    }
    pub fn storage_unavailable(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::StorageUnavailable, capability, correlation_id)
    }
    pub fn cursor_expired(correlation_id: RequestCorrelationId) -> Self {
        Self::new(
            ProblemCode::CursorExpired,
            CapabilityKey::StreamReceipts,
            correlation_id,
        )
    }
    pub fn unsupported_listener(correlation_id: RequestCorrelationId) -> Self {
        Self::new(
            ProblemCode::UnsupportedListener,
            CapabilityKey::ConfigureListener,
            correlation_id,
        )
    }

    pub fn receipt_not_found(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        debug_assert!(matches!(
            capability,
            CapabilityKey::ReplayReceipt | CapabilityKey::StreamReceipts
        ));
        Self::new(ProblemCode::ReceiptNotFound, capability, correlation_id)
    }

    pub fn invalid_observation(
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self::with_violations(
            ProblemCode::InvalidObservation,
            CapabilityKey::AcceptObservation,
            correlation_id,
            violations,
        )
    }
    pub fn malformed_json(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self::with_violations(
            ProblemCode::MalformedJson,
            capability,
            correlation_id,
            violations,
        )
    }
    pub fn payload_too_large(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self::with_violations(
            ProblemCode::PayloadTooLarge,
            capability,
            correlation_id,
            violations,
        )
    }
    pub fn unsupported_media_type(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self::with_violations(
            ProblemCode::UnsupportedMediaType,
            capability,
            correlation_id,
            violations,
        )
    }
    pub fn validation_failed(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self::with_violations(
            ProblemCode::ValidationFailed,
            capability,
            correlation_id,
            violations,
        )
    }

    pub fn try_with_violations(
        mut self,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        if violations.len() > 32 {
            return Err(ProblemBuildError::TooManyViolations);
        }
        self.violations = violations;
        Ok(self)
    }

    pub fn code(&self) -> ProblemCode {
        self.code
    }
    pub const fn contract(&self) -> ProblemContract {
        self.code.contract()
    }
    pub fn capability(&self) -> CapabilityKey {
        self.capability
    }
    pub fn title(&self) -> &'static str {
        self.contract().title()
    }
    pub fn status(&self) -> u16 {
        self.contract().status()
    }
    pub fn message(&self) -> Cow<'static, str> {
        self.contract().detail(self.capability)
    }
    pub fn safe_state(&self) -> SafeState {
        self.contract().safe_state()
    }
    pub fn retryability(&self) -> Retryability {
        self.contract().retryability()
    }
    pub fn next_actions(&self) -> &[NextAction] {
        &self.next_actions
    }
    pub fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub fn param(&self) -> Option<&str> {
        self.contract().param_policy().resolve(self.capability)
    }
    pub fn actual(&self) -> Option<&str> {
        None
    }
    pub fn documentation_path(&self) -> &'static str {
        self.contract().documentation_path()
    }
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_problem_code_has_one_complete_descriptor() {
        let mut codes = std::collections::BTreeSet::new();
        let mut paths = std::collections::BTreeSet::new();
        for code in ProblemCode::ALL {
            let contract = code.contract();
            assert!(codes.insert(code.as_str()));
            assert!(paths.insert(contract.documentation_path()));
            assert!((400..=599).contains(&contract.status()));
            assert!(!contract.title().is_empty());
            assert!(!contract.detail(CapabilityKey::AcceptObservation).is_empty());
            assert!(!contract.default_next_action().id().is_empty());
        }
    }

    #[test]
    fn authentication_failure_does_not_expose_the_failed_predicate() {
        let contract = ProblemCode::AuthenticationFailed.contract();
        assert_eq!(contract.status(), 401);
        assert_eq!(contract.safe_state(), SafeState::NoMutation);
        assert_eq!(contract.param_policy(), ProblemParamPolicy::None);
        assert_eq!(
            contract.detail(CapabilityKey::AcceptObservation),
            "the presented local credential is not active"
        );
    }

    #[test]
    fn later_problem_contracts_remain_reserved_until_their_body_activates() {
        for code in [
            ProblemCode::AlreadyInitialized,
            ProblemCode::AuthenticationFailed,
            ProblemCode::BootstrapClosed,
            ProblemCode::CursorExpired,
            ProblemCode::EvidenceNotFound,
            ProblemCode::IdentityConflict,
            ProblemCode::IntegrityFailed,
            ProblemCode::RecordNotFound,
            ProblemCode::ReviewNotFound,
            ProblemCode::StorageUnavailable,
            ProblemCode::UnsupportedListener,
        ] {
            assert_eq!(code.introduced_in(), CapabilityBody::B2);
            assert_eq!(code.contract_state(), ContractState::Reserved);
        }

        assert_eq!(ProblemCode::Forbidden.introduced_in(), CapabilityBody::B1);
        assert_eq!(
            ProblemCode::Forbidden.contract_state(),
            ContractState::Finalized
        );
    }

    #[test]
    fn operation_conflict_preserves_prior_state() {
        let problem = FastiProblem::idempotency_conflict(
            CapabilityKey::AcceptObservation,
            RequestCorrelationId::new_v7(),
        );
        assert_eq!(problem.safe_state(), SafeState::PriorStateRetained);
        assert_eq!(problem.param(), Some("/operation_id"));
    }

    #[test]
    fn violation_pointer_must_be_an_rfc_6901_pointer() {
        assert!(Violation::try_new("invalid", "/field", "bad", "good").is_ok());
        assert!(Violation::try_new("invalid", "field", "bad", "good").is_err());
    }
}
