//! Provider metadata application commands.
//!
//! Provider adapters fetch outside the local transaction, then hand validated
//! claims to this port. The provider coordinate remains evidence attached to a
//! Fasti Record; it never becomes the Record identity.

use crate::{ApplicationResult, RequestAccessContext};
use fasti_domain::{
    ExternalIdentifierClaim, ExternalIdentifierError, FieldClaim, FieldKey, Grain,
    NamespaceDefinition, NamespaceDefinitionError, NamespaceLicencePosture, RecordId,
    RequestCorrelationId, MAX_EXTERNAL_IDENTIFIER_BYTES,
};

pub const MAX_PROVIDER_METADATA_FIELDS: usize = 16;
pub const GOOGLE_BOOKS_PROVIDER_ID: &str = "google-books";
pub const TMDB_PROVIDER_ID: &str = "tmdb";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderIdentifierValueKind {
    PositiveDecimal,
    AsciiToken,
}

impl ProviderIdentifierValueKind {
    const fn pattern(self) -> &'static str {
        match self {
            Self::PositiveDecimal => "^[1-9][0-9]*$",
            Self::AsciiToken => "[A-Za-z0-9_-]+",
        }
    }

    fn accepts(self, value: &str) -> bool {
        let trimmed = value.trim();
        value == trimmed
            && !trimmed.is_empty()
            && trimmed.len() <= MAX_EXTERNAL_IDENTIFIER_BYTES
            && match self {
                Self::PositiveDecimal => {
                    let mut bytes = trimmed.bytes();
                    bytes.next().is_some_and(|byte| matches!(byte, b'1'..=b'9'))
                        && bytes.all(|byte| byte.is_ascii_digit())
                }
                Self::AsciiToken => trimmed
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIdentityMapping {
    provider: &'static str,
    kind: &'static str,
    namespace: &'static str,
    label: &'static str,
    grain: Grain,
    value_kind: ProviderIdentifierValueKind,
}

impl ProviderIdentityMapping {
    pub const fn namespace(self) -> &'static str {
        self.namespace
    }

    pub const fn grain(self) -> Grain {
        self.grain
    }

    pub fn identifier(
        self,
        value: impl Into<String>,
    ) -> Result<ExternalIdentifierClaim, ExternalIdentifierError> {
        let value = value.into();
        if !self.accepts_value(&value) {
            return Err(ExternalIdentifierError::InvalidValue);
        }
        ExternalIdentifierClaim::try_new(self.namespace, self.grain, value)
    }

    pub fn accepts_value(self, value: &str) -> bool {
        self.value_kind.accepts(value)
    }

    pub fn namespace_definition(self) -> Result<NamespaceDefinition, NamespaceDefinitionError> {
        NamespaceDefinition::try_new(
            self.namespace,
            self.label,
            [self.grain],
            self.value_kind.pattern(),
            "identity",
            NamespaceLicencePosture::IdentifiersOnly,
        )
    }
}

const PROVIDER_IDENTITY_MAPPINGS: &[ProviderIdentityMapping] = &[
    ProviderIdentityMapping {
        provider: GOOGLE_BOOKS_PROVIDER_ID,
        kind: "book",
        namespace: "googlebooks.volume",
        label: "Google Books Volume",
        grain: Grain::Edition,
        value_kind: ProviderIdentifierValueKind::AsciiToken,
    },
    ProviderIdentityMapping {
        provider: TMDB_PROVIDER_ID,
        kind: "movie",
        namespace: "tmdb.movie",
        label: "TMDB Movie",
        grain: Grain::Film,
        value_kind: ProviderIdentifierValueKind::PositiveDecimal,
    },
    ProviderIdentityMapping {
        provider: TMDB_PROVIDER_ID,
        kind: "show",
        namespace: "tmdb.tv",
        label: "TMDB TV",
        grain: Grain::Series,
        value_kind: ProviderIdentifierValueKind::PositiveDecimal,
    },
];

pub fn provider_identity_mapping(provider: &str, kind: &str) -> Option<ProviderIdentityMapping> {
    PROVIDER_IDENTITY_MAPPINGS
        .iter()
        .copied()
        .find(|mapping| mapping.provider == provider && mapping.kind == kind)
}

pub fn provider_identity_mapping_for_grain(
    provider: &str,
    grain: Grain,
) -> Option<ProviderIdentityMapping> {
    PROVIDER_IDENTITY_MAPPINGS
        .iter()
        .copied()
        .find(|mapping| mapping.provider == provider && mapping.grain == grain)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderMetadataField {
    field_key: FieldKey,
    claim: FieldClaim,
}

impl ProviderMetadataField {
    pub const fn new(field_key: FieldKey, claim: FieldClaim) -> Self {
        Self { field_key, claim }
    }

    pub const fn field_key(&self) -> &FieldKey {
        &self.field_key
    }

    pub const fn claim(&self) -> &FieldClaim {
        &self.claim
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateProviderRecordCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    grain: Grain,
    identifier: ExternalIdentifierClaim,
    fields: Vec<ProviderMetadataField>,
}

impl CreateProviderRecordCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        grain: Grain,
        identifier: ExternalIdentifierClaim,
        fields: Vec<ProviderMetadataField>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            grain,
            identifier,
            fields,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn fields(&self) -> &[ProviderMetadataField] {
        &self.fields
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyProviderMetadataCommand {
    correlation_id: RequestCorrelationId,
    access: RequestAccessContext,
    record_id: RecordId,
    identifier: ExternalIdentifierClaim,
    fields: Vec<ProviderMetadataField>,
}

impl ApplyProviderMetadataCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: RequestAccessContext,
        record_id: RecordId,
        identifier: ExternalIdentifierClaim,
        fields: Vec<ProviderMetadataField>,
    ) -> Self {
        Self {
            correlation_id,
            access,
            record_id,
            identifier,
            fields,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &RequestAccessContext {
        &self.access
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn fields(&self) -> &[ProviderMetadataField] {
        &self.fields
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateProviderRecordOutcome {
    record_id: RecordId,
    grain: Grain,
}

impl CreateProviderRecordOutcome {
    pub const fn new(record_id: RecordId, grain: Grain) -> Self {
        Self { record_id, grain }
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }
}

pub trait ProviderMetadataPort: Send + Sync {
    fn create_provider_record(
        &self,
        command: CreateProviderRecordCommand,
    ) -> ApplicationResult<CreateProviderRecordOutcome>;

    fn apply_provider_metadata(
        &self,
        command: ApplyProviderMetadataCommand,
    ) -> ApplicationResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_identity_coordinates_are_exact_and_shared_by_grain() {
        for (provider, kind, namespace, grain, pattern) in [
            (
                GOOGLE_BOOKS_PROVIDER_ID,
                "book",
                "googlebooks.volume",
                Grain::Edition,
                "[A-Za-z0-9_-]+",
            ),
            (
                TMDB_PROVIDER_ID,
                "movie",
                "tmdb.movie",
                Grain::Film,
                "^[1-9][0-9]*$",
            ),
            (
                TMDB_PROVIDER_ID,
                "show",
                "tmdb.tv",
                Grain::Series,
                "^[1-9][0-9]*$",
            ),
        ] {
            let by_kind = provider_identity_mapping(provider, kind).expect("mapped provider kind");
            let by_grain = provider_identity_mapping_for_grain(provider, grain)
                .expect("mapped provider grain");
            assert_eq!(by_kind, by_grain);
            assert_eq!(by_kind.namespace(), namespace);
            assert_eq!(by_kind.grain(), grain);
            let identifier = by_kind.identifier("42").expect("valid provider identifier");
            assert_eq!(identifier.namespace(), namespace);
            assert_eq!(identifier.grain(), grain);
            assert_eq!(
                by_kind
                    .namespace_definition()
                    .expect("provider namespace")
                    .id_pattern(),
                pattern
            );
        }

        assert!(provider_identity_mapping(GOOGLE_BOOKS_PROVIDER_ID, "chapter").is_none());
        assert!(provider_identity_mapping_for_grain(TMDB_PROVIDER_ID, Grain::Episode).is_none());
        assert!(provider_identity_mapping_for_grain(TMDB_PROVIDER_ID, Grain::Track).is_none());
        assert!(provider_identity_mapping(TMDB_PROVIDER_ID, "movie")
            .expect("TMDB movie mapping")
            .identifier("not-a-number")
            .is_err());
        for value in ["0", "00042", " 42 "] {
            assert!(provider_identity_mapping(TMDB_PROVIDER_ID, "movie")
                .expect("TMDB movie mapping")
                .identifier(value)
                .is_err());
        }
        assert!(provider_identity_mapping(GOOGLE_BOOKS_PROVIDER_ID, "book")
            .expect("Google Books mapping")
            .identifier("bad/value")
            .is_err());
    }
}
