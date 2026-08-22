use crate::{ExternalIdentifierId, Grain, RecordId, WorkspaceId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_NAMESPACE_BYTES: usize = 64;
pub const MAX_EXTERNAL_IDENTIFIER_BYTES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordStatus {
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    record_id: RecordId,
    workspace_id: WorkspaceId,
    grain: Grain,
    status: RecordStatus,
}

impl Record {
    pub const fn new(record_id: RecordId, workspace_id: WorkspaceId, grain: Grain) -> Self {
        Self {
            record_id,
            workspace_id,
            grain,
            status: RecordStatus::Active,
        }
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub const fn status(&self) -> RecordStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExternalIdentifierError {
    #[error("identifier namespace must be 2 to 64 ASCII characters and start with a letter")]
    InvalidNamespace,
    #[error("identifier value must be non-empty, bounded, and contain no control characters")]
    InvalidValue,
}

/// A provider coordinate supplied as evidence.
///
/// It is never a Fasti Record identity. Namespace normalization is limited to
/// an ASCII lowercase key. The identifier value is trimmed but otherwise kept
/// exact because providers do not share one case or punctuation policy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalIdentifierClaim {
    namespace: String,
    grain: Grain,
    value: String,
}

impl ExternalIdentifierClaim {
    pub fn try_new(
        namespace: impl Into<String>,
        grain: Grain,
        value: impl Into<String>,
    ) -> Result<Self, ExternalIdentifierError> {
        let namespace = namespace.into().trim().to_ascii_lowercase();
        let value = value.into().trim().to_owned();

        let mut namespace_bytes = namespace.bytes();
        let valid_namespace = (2..=MAX_NAMESPACE_BYTES).contains(&namespace.len())
            && namespace_bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
            && namespace_bytes.all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            });
        if !valid_namespace {
            return Err(ExternalIdentifierError::InvalidNamespace);
        }

        if value.is_empty()
            || value.len() > MAX_EXTERNAL_IDENTIFIER_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(ExternalIdentifierError::InvalidValue);
        }

        Ok(Self {
            namespace,
            grain,
            value,
        })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalIdentifier {
    external_identifier_id: ExternalIdentifierId,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    claim: ExternalIdentifierClaim,
}

impl ExternalIdentifier {
    pub const fn new(
        external_identifier_id: ExternalIdentifierId,
        workspace_id: WorkspaceId,
        record_id: RecordId,
        claim: ExternalIdentifierClaim,
    ) -> Self {
        Self {
            external_identifier_id,
            workspace_id,
            record_id,
            claim,
        }
    }

    pub const fn external_identifier_id(&self) -> ExternalIdentifierId {
        self.external_identifier_id
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn claim(&self) -> &ExternalIdentifierClaim {
        &self.claim
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityResolution {
    Unresolved,
    Resolved(RecordId),
    Conflicted(Vec<RecordId>),
}

impl IdentityResolution {
    pub fn conflicted(candidates: impl IntoIterator<Item = RecordId>) -> Self {
        let mut values = candidates.into_iter().collect::<Vec<_>>();
        values.sort_by_key(ToString::to_string);
        values.dedup();
        debug_assert!(values.len() > 1);
        Self::Conflicted(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_is_canonical_but_provider_value_is_not_rewritten() {
        let claim = ExternalIdentifierClaim::try_new("  IMDb ", Grain::Film, " tt0903747 ")
            .expect("valid external identifier");
        assert_eq!(claim.namespace(), "imdb");
        assert_eq!(claim.value(), "tt0903747");
    }

    #[test]
    fn invalid_namespaces_and_control_values_fail() {
        for namespace in ["", "1tmdb", "TM DB", "a"] {
            assert!(ExternalIdentifierClaim::try_new(namespace, Grain::Release, "1").is_err());
        }
        assert!(ExternalIdentifierClaim::try_new("tmdb", Grain::Release, "a\nb").is_err());
    }

    #[test]
    fn record_identity_is_not_a_provider_coordinate() {
        let record = Record::new(RecordId::new_v7(), WorkspaceId::new_v7(), Grain::Release);
        let claim =
            ExternalIdentifierClaim::try_new("kitsu", Grain::Release, "42").expect("valid claim");
        let identifier = ExternalIdentifier::new(
            ExternalIdentifierId::new_v7(),
            record.workspace_id(),
            record.record_id(),
            claim,
        );
        assert_ne!(
            identifier.record_id().to_string(),
            identifier.claim().value()
        );
    }
}
