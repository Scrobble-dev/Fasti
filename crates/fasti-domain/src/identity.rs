use crate::{ExternalIdentifierId, Grain, RecordId, WorkspaceId};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

pub const MAX_NAMESPACE_BYTES: usize = 64;
pub const MAX_EXTERNAL_IDENTIFIER_BYTES: usize = 256;
pub const MAX_NAMESPACE_LABEL_BYTES: usize = 128;
pub const MAX_NAMESPACE_PATTERN_BYTES: usize = 256;
pub const MAX_NAMESPACE_NORMALIZATION_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct NamespaceKey(String);

impl NamespaceKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, NamespaceKeyError> {
        let value = value.into().trim().to_ascii_lowercase();
        let mut bytes = value.bytes();
        let valid = (2..=MAX_NAMESPACE_BYTES).contains(&value.len())
            && bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
            && bytes.all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            });
        if !valid {
            return Err(NamespaceKeyError);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NamespaceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for NamespaceKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("namespace key must be 2 to 64 ASCII characters and start with a letter")]
pub struct NamespaceKeyError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamespaceLicencePosture {
    Open,
    IdentifiersOnly,
    IndirectOnly,
    Excluded,
    Unknown,
}

impl NamespaceLicencePosture {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::IdentifiersOnly => "identifiers_only",
            Self::IndirectOnly => "indirect_only",
            Self::Excluded => "excluded",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NamespaceDefinitionError {
    #[error(transparent)]
    InvalidNamespace(#[from] NamespaceKeyError),
    #[error("namespace label must be non-empty, bounded, and contain no control characters")]
    InvalidLabel,
    #[error("namespace must declare at least one supported grain")]
    InvalidGrains,
    #[error("identifier pattern must be non-empty, bounded, and contain no control characters")]
    InvalidPattern,
    #[error("normalization rule must be non-empty, bounded, and contain no control characters")]
    InvalidNormalization,
}

/// One governed external-identifier comparison space.
///
/// This B2 value owns the active runtime seed fields. Pattern and normalization
/// are stored as declarations; B6 owns provider conformance and execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NamespaceDefinition {
    namespace: NamespaceKey,
    label: String,
    grains: Vec<Grain>,
    id_pattern: String,
    normalization: String,
    licence_posture: NamespaceLicencePosture,
}

impl<'de> Deserialize<'de> for NamespaceDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireDefinition {
            namespace: String,
            label: String,
            grains: Vec<Grain>,
            id_pattern: String,
            normalization: String,
            licence_posture: NamespaceLicencePosture,
        }

        let wire = WireDefinition::deserialize(deserializer)?;
        Self::try_new(
            wire.namespace,
            wire.label,
            wire.grains,
            wire.id_pattern,
            wire.normalization,
            wire.licence_posture,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl NamespaceDefinition {
    pub fn try_new(
        namespace: impl Into<String>,
        label: impl Into<String>,
        grains: impl IntoIterator<Item = Grain>,
        id_pattern: impl Into<String>,
        normalization: impl Into<String>,
        licence_posture: NamespaceLicencePosture,
    ) -> Result<Self, NamespaceDefinitionError> {
        let namespace = NamespaceKey::try_new(namespace)?;
        let label = label.into().trim().to_owned();
        let id_pattern = id_pattern.into();
        let normalization = normalization.into();
        let mut grains = grains.into_iter().collect::<Vec<_>>();
        grains.sort_by_key(|grain| grain.as_str());
        grains.dedup();

        if !valid_text(&label, MAX_NAMESPACE_LABEL_BYTES) {
            return Err(NamespaceDefinitionError::InvalidLabel);
        }
        if grains.is_empty() {
            return Err(NamespaceDefinitionError::InvalidGrains);
        }
        if !valid_text(&id_pattern, MAX_NAMESPACE_PATTERN_BYTES) {
            return Err(NamespaceDefinitionError::InvalidPattern);
        }
        if !valid_text(&normalization, MAX_NAMESPACE_NORMALIZATION_BYTES) {
            return Err(NamespaceDefinitionError::InvalidNormalization);
        }

        Ok(Self {
            namespace,
            label,
            grains,
            id_pattern,
            normalization,
            licence_posture,
        })
    }

    pub const fn namespace(&self) -> &NamespaceKey {
        &self.namespace
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn grains(&self) -> &[Grain] {
        &self.grains
    }

    pub fn supports(&self, grain: Grain) -> bool {
        self.grains.contains(&grain)
    }

    pub fn id_pattern(&self) -> &str {
        &self.id_pattern
    }

    pub fn normalization(&self) -> &str {
        &self.normalization
    }

    pub const fn licence_posture(&self) -> NamespaceLicencePosture {
        self.licence_posture
    }
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

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
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalIdentifierClaim {
    namespace: NamespaceKey,
    grain: Grain,
    value: String,
}

impl<'de> Deserialize<'de> for ExternalIdentifierClaim {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireClaim {
            namespace: String,
            grain: Grain,
            value: String,
        }

        let wire = WireClaim::deserialize(deserializer)?;
        Self::try_new(wire.namespace, wire.grain, wire.value).map_err(serde::de::Error::custom)
    }
}

impl ExternalIdentifierClaim {
    pub fn try_new(
        namespace: impl Into<String>,
        grain: Grain,
        value: impl Into<String>,
    ) -> Result<Self, ExternalIdentifierError> {
        let namespace = NamespaceKey::try_new(namespace)
            .map_err(|_| ExternalIdentifierError::InvalidNamespace)?;
        let value = value.into().trim().to_owned();

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
        self.namespace.as_str()
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

/// The operation for which an external identifier may be used.
///
/// Route permission is purpose-specific: an alias accepted for a metadata
/// read is not automatically safe for a tracker write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionIntent {
    MetadataSearch,
    MetadataLookup,
    MetadataEnrichment,
    RatingLookup,
    CatalogLookup,
    DisplayProjection,
    NuvioExport,
    NuvioImportAttachment,
    TrackerRead,
    TrackerWrite,
    SegmentTranslation,
    DeduplicationReview,
}

impl ResolutionIntent {
    pub const ALL: &'static [Self] = &[
        Self::MetadataSearch,
        Self::MetadataLookup,
        Self::MetadataEnrichment,
        Self::RatingLookup,
        Self::CatalogLookup,
        Self::DisplayProjection,
        Self::NuvioExport,
        Self::NuvioImportAttachment,
        Self::TrackerRead,
        Self::TrackerWrite,
        Self::SegmentTranslation,
        Self::DeduplicationReview,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MetadataSearch => "metadata_search",
            Self::MetadataLookup => "metadata_lookup",
            Self::MetadataEnrichment => "metadata_enrichment",
            Self::RatingLookup => "rating_lookup",
            Self::CatalogLookup => "catalog_lookup",
            Self::DisplayProjection => "display_projection",
            Self::NuvioExport => "nuvio_export",
            Self::NuvioImportAttachment => "nuvio_import_attachment",
            Self::TrackerRead => "tracker_read",
            Self::TrackerWrite => "tracker_write",
            Self::SegmentTranslation => "segment_translation",
            Self::DeduplicationReview => "deduplication_review",
        }
    }

    pub const fn requires_provider_native_route(self) -> bool {
        matches!(self, Self::TrackerWrite)
    }
}

/// How an operation reached the selected provider coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRouteKind {
    ProviderNative,
    VerifiedAlias,
    AcceptedCrosswalk,
}

/// Why a coordinate is eligible for route planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRouteEvidenceKind {
    Direct,
    AcceptedCrosswalk,
}

/// A profile or connection's anime grouping and export preference.
///
/// This value selects an outward projection only. It never identifies or
/// re-keys a Fasti Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimeGroupingPreference {
    GroupByTvWork,
    KeepMalReleasesSeparate,
    KeepKitsuReleasesSeparate,
    Automatic,
}

impl AnimeGroupingPreference {
    pub const ALL: &'static [Self] = &[
        Self::GroupByTvWork,
        Self::KeepMalReleasesSeparate,
        Self::KeepKitsuReleasesSeparate,
        Self::Automatic,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GroupByTvWork => "group_by_tv_work",
            Self::KeepMalReleasesSeparate => "keep_mal_releases_separate",
            Self::KeepKitsuReleasesSeparate => "keep_kitsu_releases_separate",
            Self::Automatic => "automatic",
        }
    }
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
    fn namespace_definition_canonicalizes_key_and_grain_order() {
        let definition = NamespaceDefinition::try_new(
            " IMDb_Title ",
            "IMDb title",
            [Grain::Series, Grain::Film, Grain::Film],
            "^tt[0-9]+$",
            "trim",
            NamespaceLicencePosture::IdentifiersOnly,
        )
        .expect("valid definition");

        assert_eq!(definition.namespace().as_str(), "imdb_title");
        assert_eq!(definition.grains(), &[Grain::Film, Grain::Series]);
        assert!(definition.supports(Grain::Film));
        assert!(!definition.supports(Grain::Release));
    }

    #[test]
    fn namespace_definition_rejects_missing_governance_fields() {
        assert!(NamespaceDefinition::try_new(
            "imdb",
            "",
            [Grain::Film],
            "^tt[0-9]+$",
            "trim",
            NamespaceLicencePosture::Unknown,
        )
        .is_err());
        assert!(NamespaceDefinition::try_new(
            "imdb",
            "IMDb",
            [],
            "^tt[0-9]+$",
            "trim",
            NamespaceLicencePosture::Unknown,
        )
        .is_err());
    }

    #[test]
    fn hostile_namespace_json_cannot_bypass_domain_validation() {
        for json in [
            r#"{"namespace":"1imdb","label":"IMDb","grains":["film"],"id_pattern":".+","normalization":"identity","licence_posture":"unknown"}"#,
            r#"{"namespace":"imdb","label":"IMDb","grains":[],"id_pattern":".+","normalization":"identity","licence_posture":"unknown"}"#,
            "{\"namespace\":\"imdb\",\"label\":\"IMDb\",\"grains\":[\"film\"],\"id_pattern\":\".+\",\"normalization\":\"bad\\nrule\",\"licence_posture\":\"unknown\"}",
            r#"{"namespace":"imdb","label":"IMDb","grains":["film"],"id_pattern":".+","normalization":"identity","licence_posture":"unknown","extra":true}"#,
        ] {
            assert!(serde_json::from_str::<NamespaceDefinition>(json).is_err());
        }
        assert!(serde_json::from_str::<NamespaceKey>(r#""1imdb""#).is_err());
        assert!(serde_json::from_str::<ExternalIdentifierClaim>(
            "{\"namespace\":\"imdb\",\"grain\":\"film\",\"value\":\"bad\\nvalue\"}"
        )
        .is_err());
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

    #[test]
    fn resolution_intents_keep_read_aliases_out_of_tracker_writes() {
        assert_eq!(ResolutionIntent::ALL.len(), 12);
        assert!(ResolutionIntent::TrackerWrite.requires_provider_native_route());
        assert!(!ResolutionIntent::MetadataLookup.requires_provider_native_route());
        assert_eq!(ResolutionIntent::NuvioExport.as_str(), "nuvio_export");
    }

    #[test]
    fn anime_grouping_preferences_use_the_approved_public_vocabulary() {
        assert_eq!(AnimeGroupingPreference::ALL.len(), 4);
        assert_eq!(
            AnimeGroupingPreference::GroupByTvWork.as_str(),
            "group_by_tv_work"
        );
        assert_eq!(
            AnimeGroupingPreference::KeepMalReleasesSeparate.as_str(),
            "keep_mal_releases_separate"
        );
        assert_eq!(
            AnimeGroupingPreference::KeepKitsuReleasesSeparate.as_str(),
            "keep_kitsu_releases_separate"
        );
        assert_eq!(AnimeGroupingPreference::Automatic.as_str(), "automatic");
    }
}
