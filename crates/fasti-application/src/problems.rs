use crate::CapabilityKey;
use fasti_domain::RequestCorrelationId;
use serde::Serialize;
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

define_string_enum!(ProblemCode {
    CapacityExceeded => "capacity_exceeded",
    CapabilityUnavailable => "capability_unavailable",
    ContractDrift => "contract_drift",
    Forbidden => "forbidden",
    IdempotencyConflict => "idempotency_conflict",
    InvalidIdentifier => "invalid_identifier",
    InvalidObservation => "invalid_observation",
    InvalidTime => "invalid_time",
    ReceiptNotFound => "receipt_not_found",
    ValidationFailed => "validation_failed",
});

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextAction {
    id: String,
    label: String,
}

impl NextAction {
    pub fn try_new(
        id: impl Into<String>,
        label: impl Into<String>,
    ) -> Result<Self, ProblemBuildError> {
        let id = id.into();
        let label = label.into();
        if id.trim().is_empty() || label.trim().is_empty() {
            return Err(ProblemBuildError::EmptyAction);
        }
        Ok(Self { id, label })
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
    actual: Option<String>,
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
            actual: None,
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
        self.actual.as_deref()
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
    EmptyAction,
    TooManyActions,
    TooManyViolations,
    InvalidViolation,
}

impl fmt::Display for ProblemBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::EmptyAction => "problem action ID and label must not be empty",
            Self::TooManyActions => "a problem may expose no more than three ordered next actions",
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
    message: String,
    safe_state: SafeState,
    retryability: Retryability,
    next_actions: Vec<NextAction>,
    correlation_id: RequestCorrelationId,
    param: Option<String>,
    actual: Option<String>,
    documentation_path: Option<&'static str>,
    violations: Vec<Violation>,
}

impl FastiProblem {
    /// The bounded application resource cannot accept another distinct item.
    /// Callers may retry only after capacity has been released; no mutation has
    /// occurred for the rejected request.
    pub fn capacity_exceeded(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self {
            code: ProblemCode::CapacityExceeded,
            capability,
            message: "bounded application capacity has been reached".to_owned(),
            safe_state: SafeState::NoMutation,
            retryability: Retryability::RetryAfterCorrection,
            next_actions: vec![NextAction {
                id: "release_capacity".to_owned(),
                label: "Release retained capacity before retrying".to_owned(),
            }],
            correlation_id,
            param: None,
            actual: None,
            documentation_path: Some("v1/problems/capacity-exceeded"),
            violations: Vec::new(),
        }
    }

    pub fn capability_unavailable(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        let owner = capability.runtime_body().as_str();
        Self {
            code: ProblemCode::CapabilityUnavailable,
            capability,
            message: format!(
                "requested capability is not available in this body; it is owned by {owner}"
            ),
            safe_state: SafeState::NoMutation,
            retryability: Retryability::NotRetryable,
            next_actions: vec![NextAction {
                id: "review_capability_status".to_owned(),
                label: "Review the local capability registry".to_owned(),
            }],
            correlation_id,
            param: None,
            actual: None,
            documentation_path: Some("v1/problems/capability-unavailable"),
            violations: Vec::new(),
        }
    }

    pub fn forbidden(capability: CapabilityKey, correlation_id: RequestCorrelationId) -> Self {
        Self {
            code: ProblemCode::Forbidden,
            capability,
            message: "request is not authorized for this capability".to_owned(),
            safe_state: SafeState::NoMutation,
            retryability: Retryability::NotRetryable,
            next_actions: vec![NextAction {
                id: "verify_request_authorization".to_owned(),
                label: "Verify the request context and local grant".to_owned(),
            }],
            correlation_id,
            param: None,
            actual: None,
            documentation_path: Some("v1/problems/forbidden"),
            violations: Vec::new(),
        }
    }

    pub fn idempotency_conflict(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> Self {
        Self {
            code: ProblemCode::IdempotencyConflict,
            capability,
            message: "operation ID was already used with different request semantics".to_owned(),
            safe_state: SafeState::PriorStateRetained,
            retryability: Retryability::RetryAfterCorrection,
            next_actions: vec![NextAction {
                id: "use_new_operation_id".to_owned(),
                label: "Use a new operation ID for a distinct observation".to_owned(),
            }],
            correlation_id,
            param: Some("/operation_id".to_owned()),
            actual: None,
            documentation_path: Some("v1/problems/idempotency-conflict"),
            violations: Vec::new(),
        }
    }

    pub fn receipt_not_found(correlation_id: RequestCorrelationId) -> Self {
        Self {
            code: ProblemCode::ReceiptNotFound,
            capability: CapabilityKey::ReplayReceipt,
            message: "no receipt is available for the requested identifier".to_owned(),
            safe_state: SafeState::NoMutation,
            retryability: Retryability::NotRetryable,
            next_actions: vec![NextAction {
                id: "verify_receipt_id".to_owned(),
                label: "Verify the receipt ID and request context".to_owned(),
            }],
            correlation_id,
            param: Some("/receipt_id".to_owned()),
            actual: None,
            documentation_path: Some("v1/problems/receipt-not-found"),
            violations: Vec::new(),
        }
    }

    pub fn invalid_observation(
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self {
            code: ProblemCode::InvalidObservation,
            capability: CapabilityKey::AcceptObservation,
            message: "observation does not satisfy the governed contract".to_owned(),
            safe_state: SafeState::NoMutation,
            retryability: Retryability::RetryAfterCorrection,
            next_actions: vec![NextAction {
                id: "correct_observation".to_owned(),
                label: "Correct the reported fields and submit again".to_owned(),
            }],
            correlation_id,
            param: None,
            actual: None,
            documentation_path: Some("v1/problems/invalid-observation"),
            violations: Vec::new(),
        }
        .try_with_violations(violations)
    }

    /// Transport or representation validation failed before the capability
    /// could mutate application state.
    pub fn validation_failed(
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
        violations: Vec<Violation>,
    ) -> Result<Self, ProblemBuildError> {
        Self {
            code: ProblemCode::ValidationFailed,
            capability,
            message: "request representation does not satisfy the governed contract".to_owned(),
            safe_state: SafeState::NoMutation,
            retryability: Retryability::RetryAfterCorrection,
            next_actions: vec![NextAction {
                id: "correct_request".to_owned(),
                label: "Correct the request representation and retry".to_owned(),
            }],
            correlation_id,
            param: None,
            actual: None,
            documentation_path: Some("v1/problems/validation-failed"),
            violations: Vec::new(),
        }
        .try_with_violations(violations)
    }

    pub fn try_with_next_actions(
        mut self,
        next_actions: Vec<NextAction>,
    ) -> Result<Self, ProblemBuildError> {
        if next_actions.len() > 3 {
            return Err(ProblemBuildError::TooManyActions);
        }
        self.next_actions = next_actions;
        Ok(self)
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
    pub fn capability(&self) -> CapabilityKey {
        self.capability
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn safe_state(&self) -> SafeState {
        self.safe_state
    }
    pub fn retryability(&self) -> Retryability {
        self.retryability
    }
    pub fn next_actions(&self) -> &[NextAction] {
        &self.next_actions
    }
    pub fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }
    pub fn param(&self) -> Option<&str> {
        self.param.as_deref()
    }
    pub fn actual(&self) -> Option<&str> {
        self.actual.as_deref()
    }
    pub fn documentation_path(&self) -> Option<&str> {
        self.documentation_path
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
    fn construction_enforces_action_and_violation_limits() {
        let action = || NextAction::try_new("review", "Review status").expect("valid action");
        let problem = FastiProblem::capability_unavailable(
            CapabilityKey::RestoreWorkspace,
            RequestCorrelationId::new_v7(),
        );
        assert_eq!(
            problem.try_with_next_actions(vec![action(), action(), action(), action()]),
            Err(ProblemBuildError::TooManyActions)
        );
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
}
