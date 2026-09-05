//! Shared governed provider registry and concrete metadata adapters.

mod metadata;
mod platform_vault;
mod providers;
mod transport;

use fasti_application::ProblemCode;
pub use metadata::*;
pub use platform_vault::*;
pub use providers::*;
pub use transport::{
    bounded_body, configuration_digest, pinned_client, pinned_client_with_timeouts, resolve_once,
    AuthorizedClient, GovernedTransport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderRuntimeErrorKind {
    Configuration,
    Credential,
    Network,
    Provider,
    Vault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRuntimeError {
    kind: ProviderRuntimeErrorKind,
    problem_code: ProblemCode,
    detail: String,
}

impl ProviderRuntimeError {
    pub fn configuration(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Configuration,
            ProblemCode::ProviderRouteUnavailable,
            detail,
        )
    }

    pub fn credential(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Credential,
            ProblemCode::ProviderCredentialInvalid,
            detail,
        )
    }

    pub fn credential_missing(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Credential,
            ProblemCode::ProviderCredentialMissing,
            detail,
        )
    }

    pub fn credential_expired(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Credential,
            ProblemCode::ProviderCredentialExpired,
            detail,
        )
    }

    pub fn network(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Network,
            ProblemCode::ProviderUnavailable,
            detail,
        )
    }

    pub fn provider(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Provider,
            ProblemCode::ProviderUnavailable,
            detail,
        )
    }

    pub fn rate_limited(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Provider,
            ProblemCode::ProviderRateLimited,
            detail,
        )
    }

    pub fn response_invalid(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Provider,
            ProblemCode::ProviderResponseInvalid,
            detail,
        )
    }

    pub fn vault(detail: impl Into<String>) -> Self {
        Self::new(
            ProviderRuntimeErrorKind::Vault,
            ProblemCode::ProviderUnavailable,
            detail,
        )
    }

    fn new(
        kind: ProviderRuntimeErrorKind,
        problem_code: ProblemCode,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            problem_code,
            detail: detail.into(),
        }
    }

    pub const fn kind(&self) -> ProviderRuntimeErrorKind {
        self.kind
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub const fn problem_code(&self) -> ProblemCode {
        self.problem_code
    }
}

impl std::fmt::Display for ProviderRuntimeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for ProviderRuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_errors_expose_exact_safe_problem_codes() {
        let cases = [
            (
                ProviderRuntimeError::credential_missing("safe"),
                ProblemCode::ProviderCredentialMissing,
            ),
            (
                ProviderRuntimeError::credential("safe"),
                ProblemCode::ProviderCredentialInvalid,
            ),
            (
                ProviderRuntimeError::credential_expired("safe"),
                ProblemCode::ProviderCredentialExpired,
            ),
            (
                ProviderRuntimeError::rate_limited("safe"),
                ProblemCode::ProviderRateLimited,
            ),
            (
                ProviderRuntimeError::response_invalid("safe"),
                ProblemCode::ProviderResponseInvalid,
            ),
            (
                ProviderRuntimeError::configuration("safe"),
                ProblemCode::ProviderRouteUnavailable,
            ),
            (
                ProviderRuntimeError::provider("safe"),
                ProblemCode::ProviderUnavailable,
            ),
            (
                ProviderRuntimeError::vault("safe"),
                ProblemCode::ProviderUnavailable,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.problem_code(), expected);
        }
    }
}
