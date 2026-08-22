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
    CapabilityUnavailable => "capability_unavailable",
    ContractDrift => "contract_drift",
    Forbidden => "forbidden",
    InvalidIdentifier => "invalid_identifier",
    InvalidTime => "invalid_time",
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
        actual: Option<String>,
        echo_actual: bool,
    ) -> Result<Self, ProblemBuildError> {
        if actual.is_some() && !echo_actual {
            return Err(ProblemBuildError::ActualEchoForbidden);
        }
        let code = code.into();
        let pointer = pointer.into();
        let reason = reason.into();
        let expected = expected.into();
        if code.trim().is_empty()
            || !pointer.starts_with('/')
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
            actual,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemBuildError {
    EmptyAction,
    TooManyActions,
    TooManyViolations,
    InvalidViolation,
    ActualEchoForbidden,
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
            Self::ActualEchoForbidden => "the field policy forbids echoing the actual value",
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
    pub fn capability_unavailable(
        capability: CapabilityKey,
        owner: &str,
        correlation_id: RequestCorrelationId,
    ) -> Self {
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
            documentation_path: Some("problems/capability-unavailable"),
            violations: Vec::new(),
        }
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
            "B3",
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
            "B3",
            RequestCorrelationId::new_v7(),
        );
        assert_eq!(
            problem.try_with_next_actions(vec![action(), action(), action(), action()]),
            Err(ProblemBuildError::TooManyActions)
        );
    }

    #[test]
    fn violation_actual_is_blocked_when_field_policy_forbids_echo() {
        assert_eq!(
            Violation::try_new(
                "invalid_secret",
                "/credential",
                "credential is invalid",
                "a valid credential",
                Some("do-not-echo".to_owned()),
                false,
            ),
            Err(ProblemBuildError::ActualEchoForbidden)
        );
    }
}
