use crate::CapabilityKey;
use fasti_domain::RequestCorrelationId;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt;

macro_rules! define_string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
        pub enum $name {
            $(#[serde(rename = $value)] $variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
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

/// Canonical transport-representation violation metadata. Adapters select the
/// problem code from their transport rejection and consume this descriptor;
/// they do not own alternate user-facing strings.
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
        pub enum ProblemCode {
            $(#[serde(rename = $code)] $variant),+
        }

        impl ProblemCode {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $code),+
                }
            }

            pub fn from_code(value: &str) -> Option<Self> {
                match value {
                    $($code => Some(Self::$variant)),+,
                    _ => None,
                }
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
                        default_next_action: NextActionContract {
                            id: $action_id,
                            label: $action_label,
                        },
                        param_policy: $param_policy,
                    }),+
                }
            }

            pub const fn representation_violation(self) -> Option<RepresentationViolationContract> {
                match self {
                    Self::MalformedJson => Some(RepresentationViolationContract {
                        code: "invalid_representation",
                        pointer: "/",
                        reason: "request JSON is malformed",
                        expected: "well-formed JSON",
                    }),
                    Self::PayloadTooLarge => Some(RepresentationViolationContract {
                        code: "invalid_representation",
                        pointer: "/",
                        reason: "request body exceeds the bounded fixture limit",
                        expected: "a JSON body no larger than 4096 bytes",
                    }),
                    Self::UnsupportedMediaType => Some(RepresentationViolationContract {
                        code: "invalid_representation",
                        pointer: "/",
                        reason: "request media type is unsupported",
                        expected: "Content-Type application/json",
                    }),
                    Self::ValidationFailed => Some(RepresentationViolationContract {
                        code: "invalid_representation",
                        pointer: "/",
                        reason: "request JSON does not match the governed schema",
                        expected: "the documented request schema",
                    }),
                    _ => None,
                }
            }
        }
    };
}

define_problem_catalog!(
    CapacityExceeded => "capacity_exceeded" {
        title: "Capacity exceeded",
        status: 507,
        detail: ProblemDetail::Static("bounded application capacity has been reached"),
        documentation_path: "v1/problems/capacity-exceeded",
        safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("release_capacity", "Release retained capacity before retrying"),
        param_policy: ProblemParamPolicy::None
    },
    CapabilityUnavailable => "capability_unavailable" {
        title: "Capability unavailable",
        status: 501,
        detail: ProblemDetail::RuntimeBodyOwner,
        documentation_path: "v1/problems/capability-unavailable",
        safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("review_capability_status", "Review the local capability registry"),
        param_policy: ProblemParamPolicy::None
    },
    Forbidden => "forbidden" {
        title: "Forbidden",
        status: 403,
        detail: ProblemDetail::Static("request is not authorized for this capability"),
        documentation_path: "v1/problems/forbidden",
        safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("verify_request_authorization", "Verify the request context and local grant"),
        param_policy: ProblemParamPolicy::None
    },
    IdempotencyConflict => "idempotency_conflict" {
        title: "Idempotency conflict",
        status: 409,
        detail: ProblemDetail::Static("operation ID was already used with different request semantics"),
        documentation_path: "v1/problems/idempotency-conflict",
        safe_state: PriorStateRetained,
        retryability: RetryAfterCorrection,
        default_next_action: ("use_new_operation_id", "Use a new operation ID for a distinct observation"),
        param_policy: ProblemParamPolicy::Fixed("/operation_id")
    },
    InvalidIdentifier => "invalid_identifier" {
        title: "Invalid identifier",
        status: 422,
        detail: ProblemDetail::Static("identifier does not satisfy the governed format"),
        documentation_path: "v1/problems/invalid-identifier",
        safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_identifier", "Correct the identifier and retry"),
        param_policy: ProblemParamPolicy::None
    },
    MalformedJson => "malformed_json" {
        title: "Malformed JSON",
        status: 400,
        detail: ProblemDetail::Static("request JSON is malformed"),
        documentation_path: "v1/problems/malformed-json",
        safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_json", "Correct the JSON syntax and retry"),
        param_policy: ProblemParamPolicy::None
    },
    InvalidObservation => "invalid_observation" {
        title: "Invalid observation",
        status: 422,
        detail: ProblemDetail::Static("observation does not satisfy the governed contract"),
        documentation_path: "v1/problems/invalid-observation",
        safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_observation", "Correct the reported fields and submit again"),
        param_policy: ProblemParamPolicy::None
    },
    PayloadTooLarge => "payload_too_large" {
        title: "Payload too large",
        status: 413,
        detail: ProblemDetail::Static("request body exceeds the bounded transport limit"),
        documentation_path: "v1/problems/payload-too-large",
        safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("reduce_request_body", "Reduce the request body and retry"),
        param_policy: ProblemParamPolicy::None
    },
    ReceiptNotFound => "receipt_not_found" {
        title: "Receipt not found",
        status: 404,
        detail: ProblemDetail::Static("no receipt is available for the requested identifier"),
        documentation_path: "v1/problems/receipt-not-found",
        safe_state: NoMutation,
        retryability: NotRetryable,
        default_next_action: ("verify_receipt_id", "Verify the receipt ID and request context"),
        param_policy: ProblemParamPolicy::ReceiptIdentifierByCapability
    },
    ValidationFailed => "validation_failed" {
        title: "Validation failed",
        status: 422,
        detail: ProblemDetail::Static("request representation does not satisfy the governed contract"),
        documentation_path: "v1/problems/validation-failed",
        safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("correct_request", "Correct the request representation and retry"),
        param_policy: ProblemParamPolicy::None
    },
    UnsupportedMediaType => "unsupported_media_type" {
        title: "Unsupported media type",
        status: 415,
        detail: ProblemDetail::Static("request media type is unsupported"),
        documentation_path: "v1/problems/unsupported-media-type",
        safe_state: NoMutation,
        retryability: RetryAfterCorrection,
        default_next_action: ("use_json_media_type", "Use Content-Type application/json and retry"),
        param_policy: ProblemParamPolicy::None
    }
);

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::TooManyViolations => "a problem may expose no more than 32 validation violations",
            Self::InvalidViolation => {
                "violation fields must be non-empty and pointer must be a JSON Pointer"
            }
        };
        f.write_str(message)
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
        let default_action = code.contract().default_next_action();
        Self {
            code,
            capability,
            next_actions: vec![NextAction::from_contract(default_action)],
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

    /// The bounded application resource cannot accept another distinct item.
    /// Callers may retry only after capacity has been released; no mutation has
    /// occurred for the rejected request.
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

    pub fn idempotency_conflict(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self::new(ProblemCode::IdempotencyConflict, capability, correlation_id)
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

    /// Transport or representation validation failed before the capability
    /// could mutate application state.
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
    fn semantic_values_have_one_stable_contract_spelling() {
        assert_eq!(ProblemCode::CapacityExceeded.as_str(), "capacity_exceeded");
        assert_eq!(
            ProblemCode::CapabilityUnavailable.as_str(),
            "capability_unavailable"
        );
        assert_eq!(
            SafeState::PriorStateRetained.as_str(),
            "prior_state_retained"
        );
        assert_eq!(
            Retryability::RetryAfterCorrection.as_str(),
            "retry_after_correction"
        );
        assert_eq!(
            ProblemCode::from_code("unsupported_media_type"),
            Some(ProblemCode::UnsupportedMediaType)
        );
        assert_eq!(ProblemCode::from_code("unknown"), None);
    }

    #[test]
    fn every_problem_code_has_one_complete_canonical_descriptor() {
        let mut codes = std::collections::BTreeSet::new();
        let mut documentation_paths = std::collections::BTreeSet::new();
        for code in ProblemCode::ALL {
            let contract = code.contract();
            assert!(codes.insert(code.as_str()));
            assert!(documentation_paths.insert(contract.documentation_path()));
            assert!((400..=599).contains(&contract.status()));
            assert!(!contract.title().is_empty());
            assert!(!contract.detail(CapabilityKey::AcceptObservation).is_empty());
            assert!(!contract.default_next_action().id().is_empty());
            assert!(!contract.default_next_action().label().is_empty());
        }
    }

    #[test]
    fn representation_failures_preserve_distinct_http_semantics() {
        let correlation_id = RequestCorrelationId::new_v7();
        let violation = || {
            Violation::try_new(
                "invalid_representation",
                "/",
                "representation is invalid",
                "a governed JSON representation",
            )
            .expect("valid violation")
        };
        let cases = [
            (
                FastiProblem::malformed_json(
                    CapabilityKey::InitializeNode,
                    correlation_id,
                    vec![violation()],
                )
                .expect("bounded problem"),
                ProblemCode::MalformedJson,
                400,
            ),
            (
                FastiProblem::payload_too_large(
                    CapabilityKey::InitializeNode,
                    correlation_id,
                    vec![violation()],
                )
                .expect("bounded problem"),
                ProblemCode::PayloadTooLarge,
                413,
            ),
            (
                FastiProblem::unsupported_media_type(
                    CapabilityKey::InitializeNode,
                    correlation_id,
                    vec![violation()],
                )
                .expect("bounded problem"),
                ProblemCode::UnsupportedMediaType,
                415,
            ),
            (
                FastiProblem::validation_failed(
                    CapabilityKey::InitializeNode,
                    correlation_id,
                    vec![violation()],
                )
                .expect("bounded problem"),
                ProblemCode::ValidationFailed,
                422,
            ),
        ];
        for (problem, code, status) in cases {
            assert_eq!(problem.code(), code);
            assert_eq!(problem.status(), status);
            assert_eq!(problem.violations().len(), 1);
        }
    }

    #[test]
    fn unavailable_problem_preserves_safe_state_and_one_repair() {
        let problem = FastiProblem::capability_unavailable(
            CapabilityKey::ExportWorkspace,
            RequestCorrelationId::new_v7(),
        );
        assert_eq!(problem.safe_state(), SafeState::NoMutation);
        assert_eq!(problem.next_actions().len(), 1);
    }

    #[test]
    fn violations_cannot_echo_actual_values_before_a_field_policy_exists() {
        let violation = Violation::try_new(
            "invalid_secret",
            "/credential",
            "credential is invalid",
            "a valid credential",
        )
        .expect("valid redacted violation");
        assert_eq!(violation.actual(), None);
    }

    #[test]
    fn capacity_failure_is_retryable_only_after_correction_and_never_mutates() {
        let problem = FastiProblem::capacity_exceeded(
            CapabilityKey::AcceptObservation,
            RequestCorrelationId::new_v7(),
        );
        assert_eq!(problem.safe_state(), SafeState::NoMutation);
        assert_eq!(problem.retryability(), Retryability::RetryAfterCorrection);
        assert_eq!(problem.next_actions()[0].id(), "release_capacity");
    }

    #[test]
    fn receipt_absence_keeps_replay_and_stream_capabilities_distinct() {
        let correlation_id = RequestCorrelationId::new_v7();
        let replay = FastiProblem::receipt_not_found(CapabilityKey::ReplayReceipt, correlation_id);
        let stream = FastiProblem::receipt_not_found(CapabilityKey::StreamReceipts, correlation_id);
        assert_eq!(replay.capability(), CapabilityKey::ReplayReceipt);
        assert_eq!(replay.param(), Some("/receipt_id"));
        assert_eq!(stream.capability(), CapabilityKey::StreamReceipts);
        assert_eq!(stream.param(), Some("/last_event_id"));
    }

    #[test]
    fn violation_pointer_must_be_a_nonempty_rfc_6901_pointer() {
        for invalid in ["", "credential", "/field~", "/field~2name"] {
            assert_eq!(
                Violation::try_new("invalid", invalid, "invalid field", "valid field"),
                Err(ProblemBuildError::InvalidViolation)
            );
        }
        for valid in ["/credential", "/field~0name", "/field~1name"] {
            assert!(Violation::try_new("invalid", valid, "invalid field", "valid field").is_ok());
        }
    }

    #[test]
    fn authorization_problem_does_not_enumerate_the_failed_predicate() {
        let problem = FastiProblem::forbidden(
            CapabilityKey::AcceptObservation,
            RequestCorrelationId::new_v7(),
        );
        assert_eq!(problem.code(), ProblemCode::Forbidden);
        assert_eq!(problem.actual(), None);
        assert_eq!(problem.violations(), &[]);
        assert!(!problem.message().contains("credential"));
        assert!(!problem.message().contains("scope"));
    }

    #[test]
    fn operation_conflict_preserves_prior_state() {
        let problem = FastiProblem::idempotency_conflict(
            CapabilityKey::AcceptObservation,
            RequestCorrelationId::new_v7(),
        );
        assert_eq!(problem.safe_state(), SafeState::PriorStateRetained);
        assert_eq!(problem.param(), Some("/operation_id"));
        assert_eq!(problem.actual(), None);
    }

    #[test]
    #[should_panic(expected = "problem forbidden is not allowed for capability SystemHealth")]
    fn problem_construction_rejects_cross_capability_semantics() {
        let _ =
            FastiProblem::forbidden(CapabilityKey::SystemHealth, RequestCorrelationId::new_v7());
    }
}
