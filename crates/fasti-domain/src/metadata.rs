//! Provider-owned metadata claims and their resolution to one displayed value.
//!
//! A Fasti Record's identity never depends on a provider. Its displayed
//! metadata does, and providers disagree, go stale, or go silent. This module
//! keeps every claim a provider ever supplied and resolves them to one value
//! deterministically, so the same inputs always produce the same answer and a
//! caller can explain why a value is showing.
//!
//! Overrides are profile-owned. Resolution ends in an explicit empty state;
//! an original-observed fallback is not part of the binding resolution order.
//! The unscoped `FieldOverride` remains only for pre-M2 API compatibility.

use crate::{Grain, MetadataClaimId, NamespaceKey, ProfileId, RecordId, Sha256Digest};
use chrono::{DateTime, Utc};
use serde::Serialize;

pub const MAX_FIELD_KEY_BYTES: usize = 64;
pub const MAX_FIELD_VALUE_BYTES: usize = 4096;
pub const MAX_LOCALE_BYTES: usize = 16;
pub const MAX_REGION_BYTES: usize = 8;
pub const MAX_METADATA_PROVIDER_ID_BYTES: usize = 128;
pub const MAX_SOURCE_IDENTIFIER_BYTES: usize = 512;
pub const MAX_SOURCE_VERSION_BYTES: usize = 128;

/// Canonical field key for a Record's display title.
pub const TITLE_FIELD_KEY: &str = "core.title";
/// Canonical field key for a Record's poster/artwork URL.
pub const POSTER_FIELD_KEY: &str = "core.poster_url";
/// Canonical field key for a Record's original provider title.
pub const ORIGINAL_TITLE_FIELD_KEY: &str = "core.original_title";
/// Canonical field key for a Record's provider synopsis.
pub const OVERVIEW_FIELD_KEY: &str = "core.overview";
/// Canonical field key for a Record's release year.
pub const RELEASE_YEAR_FIELD_KEY: &str = "core.release_year";

/// A dotted field identity such as `core.title` or `book.authors`.
///
/// Not an enum: record types and providers both add fields, and the domain
/// must not require a code change to accept a new one. Shape validation is
/// the only guarantee; a field a caller does not recognize is simply not
/// resolved for, not rejected here.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct FieldKey(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("field key must be 1 to 64 ASCII characters, dot-separated lowercase segments")]
pub struct FieldKeyError;

impl FieldKey {
    pub fn try_new(value: impl Into<String>) -> Result<Self, FieldKeyError> {
        let value = value.into();
        let valid = (1..=MAX_FIELD_KEY_BYTES).contains(&value.len())
            && value.split('.').all(is_lowercase_ascii_segment);
        if !valid {
            return Err(FieldKeyError);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_lowercase_ascii_segment(segment: &str) -> bool {
    let mut bytes = segment.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MetadataValueError {
    #[error("metadata provider ID must be a bounded canonical lowercase identifier")]
    InvalidProviderId,
    #[error("locale must be 2 to 16 ASCII letters, digits, or non-empty hyphenated segments")]
    InvalidLocale,
    #[error("region must be 2 to 8 ASCII letters or digits")]
    InvalidRegion,
    #[error("source identifier must be non-empty, bounded, and contain no control characters")]
    InvalidSourceIdentifier,
    #[error("source version must be bounded and contain no control characters")]
    InvalidSourceVersion,
}

/// Stable provider identity inside metadata provenance.
///
/// This is distinct from a source namespace. One provider can emit identifiers
/// in several namespaces, and a namespace can be understood by several
/// providers.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MetadataProviderId(String);

impl MetadataProviderId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, MetadataValueError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= MAX_METADATA_PROVIDER_ID_BYTES
            && value.trim() == value
            && value.bytes().all(|byte| {
                !byte.is_ascii_uppercase()
                    && (byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
            });
        valid
            .then_some(Self(value))
            .ok_or(MetadataValueError::InvalidProviderId)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Canonical, comparison-safe metadata locale.
///
/// The domain validates a deliberately bounded BCP-47-shaped subset and
/// normalizes it to lowercase. Provider adapters retain any richer upstream
/// representation in evidence rather than weakening deterministic comparison.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MetadataLocale(String);

impl MetadataLocale {
    pub fn try_new(value: impl Into<String>) -> Result<Self, MetadataValueError> {
        let value = value.into();
        let valid = (2..=MAX_LOCALE_BYTES).contains(&value.len())
            && value.split('-').all(|segment| {
                !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
            && value
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic());
        valid
            .then(|| Self(value.to_ascii_lowercase()))
            .ok_or(MetadataValueError::InvalidLocale)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn language(&self) -> &str {
        self.0
            .split_once('-')
            .map_or(self.as_str(), |(language, _)| language)
    }
}

/// Canonical region used by metadata routing and projection policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MetadataRegion(String);

impl MetadataRegion {
    pub fn try_new(value: impl Into<String>) -> Result<Self, MetadataValueError> {
        let value = value.into();
        let valid = (2..=MAX_REGION_BYTES).contains(&value.len())
            && value.bytes().all(|byte| byte.is_ascii_alphanumeric());
        valid
            .then(|| Self(value.to_ascii_uppercase()))
            .ok_or(MetadataValueError::InvalidRegion)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Immutable provider-response provenance attached to one field claim.
///
/// `legacy` exists only so pre-M2 claims remain readable during their additive
/// migration. New provider responses must use `try_new`, which requires the
/// provider, source identifier, and accepted evidence digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldClaimProvenance {
    provider_id: Option<MetadataProviderId>,
    source_namespace: NamespaceKey,
    source_identifier: Option<String>,
    locale: Option<MetadataLocale>,
    region: Option<MetadataRegion>,
    source_version: Option<String>,
    evidence_digest: Option<Sha256Digest>,
}

impl FieldClaimProvenance {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        provider_id: MetadataProviderId,
        source_namespace: NamespaceKey,
        source_identifier: impl Into<String>,
        locale: Option<MetadataLocale>,
        region: Option<MetadataRegion>,
        source_version: Option<String>,
        evidence_digest: Sha256Digest,
    ) -> Result<Self, MetadataValueError> {
        let source_identifier = source_identifier.into();
        if source_identifier.is_empty()
            || source_identifier.len() > MAX_SOURCE_IDENTIFIER_BYTES
            || source_identifier.trim() != source_identifier
            || source_identifier.chars().any(char::is_control)
        {
            return Err(MetadataValueError::InvalidSourceIdentifier);
        }
        if source_version.as_ref().is_some_and(|version| {
            version.is_empty()
                || version.len() > MAX_SOURCE_VERSION_BYTES
                || version.trim() != version
                || version.chars().any(char::is_control)
        }) {
            return Err(MetadataValueError::InvalidSourceVersion);
        }
        Ok(Self {
            provider_id: Some(provider_id),
            source_namespace,
            source_identifier: Some(source_identifier),
            locale,
            region,
            source_version,
            evidence_digest: Some(evidence_digest),
        })
    }

    pub fn legacy(source_namespace: NamespaceKey, locale: Option<MetadataLocale>) -> Self {
        Self {
            provider_id: None,
            source_namespace,
            source_identifier: None,
            locale,
            region: None,
            source_version: None,
            evidence_digest: None,
        }
    }

    pub fn provider_id(&self) -> Option<&MetadataProviderId> {
        self.provider_id.as_ref()
    }

    pub fn source_namespace(&self) -> &NamespaceKey {
        &self.source_namespace
    }

    pub fn source_identifier(&self) -> Option<&str> {
        self.source_identifier.as_deref()
    }

    pub fn locale(&self) -> Option<&MetadataLocale> {
        self.locale.as_ref()
    }

    pub fn region(&self) -> Option<&MetadataRegion> {
        self.region.as_ref()
    }

    pub fn source_version(&self) -> Option<&str> {
        self.source_version.as_deref()
    }

    pub fn evidence_digest(&self) -> Option<&Sha256Digest> {
        self.evidence_digest.as_ref()
    }

    pub const fn is_complete(&self) -> bool {
        self.provider_id.is_some()
            && self.source_identifier.is_some()
            && self.evidence_digest.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldClaimStatus {
    Fresh,
    Stale,
    Invalid,
    Revoked,
    Superseded,
    Unavailable,
}

impl FieldClaimStatus {
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (
                Self::Fresh,
                Self::Stale | Self::Invalid | Self::Revoked | Self::Superseded | Self::Unavailable
            ) | (
                Self::Stale,
                Self::Invalid | Self::Revoked | Self::Superseded | Self::Unavailable
            ) | (
                Self::Unavailable,
                Self::Stale | Self::Invalid | Self::Revoked | Self::Superseded
            )
        )
    }

    pub const fn can_project_fresh(self) -> bool {
        matches!(self, Self::Fresh)
    }

    pub const fn can_project_last_known_good(self) -> bool {
        matches!(self, Self::Stale | Self::Unavailable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldClaimError {
    #[error("field claim value must be non-empty, bounded, and contain no control characters")]
    InvalidValue,
    #[error("locale must be 2 to 16 ASCII letters, digits, or hyphens")]
    InvalidLocale,
    #[error("expires_at cannot be at or before fetched_at")]
    ExpiryNotAfterFetch,
    #[error("a complete provider claim requires complete provenance")]
    IncompleteProvenance,
    #[error("record and field targets must either both be present or both be absent")]
    IncompleteTarget,
}

/// One provider's claim about one field's value, as it was fetched.
///
/// `fetched_at` is `ReceivedAt`, the same server-owned-ingress type used
/// elsewhere in the domain, reused rather than duplicated: a metadata fetch
/// is exactly that shape of event. It is not `Deserialize` for the same
/// reason `ReceivedAt` is not — a claim's arrival time is not something the
/// wire gets to assert; the layer that actually received the response
/// supplies it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldClaim {
    claim_id: MetadataClaimId,
    record_id: Option<RecordId>,
    field_key: Option<FieldKey>,
    value: String,
    provenance: FieldClaimProvenance,
    fetched_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    initial_status: FieldClaimStatus,
}

impl FieldClaim {
    pub fn try_new(
        source: NamespaceKey,
        value: impl Into<String>,
        locale: Option<String>,
        fetched_at: crate::ReceivedAt,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, FieldClaimError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_FIELD_VALUE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(FieldClaimError::InvalidValue);
        }
        let locale = locale
            .map(|locale| {
                MetadataLocale::try_new(locale).map_err(|_| FieldClaimError::InvalidLocale)
            })
            .transpose()?;
        let fetched_at = fetched_at.value();
        if let Some(expires_at) = expires_at {
            if expires_at <= fetched_at {
                return Err(FieldClaimError::ExpiryNotAfterFetch);
            }
        }
        Ok(Self {
            claim_id: MetadataClaimId::new_v7(),
            record_id: None,
            field_key: None,
            value,
            provenance: FieldClaimProvenance::legacy(source, locale),
            fetched_at,
            expires_at,
            initial_status: FieldClaimStatus::Fresh,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_new_provider(
        claim_id: MetadataClaimId,
        record_id: RecordId,
        field_key: FieldKey,
        value: impl Into<String>,
        provenance: FieldClaimProvenance,
        fetched_at: crate::ReceivedAt,
        expires_at: Option<DateTime<Utc>>,
        initial_status: FieldClaimStatus,
    ) -> Result<Self, FieldClaimError> {
        if !provenance.is_complete() {
            return Err(FieldClaimError::IncompleteProvenance);
        }
        let value = value.into();
        validate_field_value(&value)?;
        let fetched_at = fetched_at.value();
        if expires_at.is_some_and(|expires_at| expires_at <= fetched_at) {
            return Err(FieldClaimError::ExpiryNotAfterFetch);
        }
        Ok(Self {
            claim_id,
            record_id: Some(record_id),
            field_key: Some(field_key),
            value,
            provenance,
            fetched_at,
            expires_at,
            initial_status,
        })
    }

    /// Builds a complete provider claim before a newly discovered candidate
    /// has been attached to a Fasti Record. The application field wrapper
    /// carries the field key; persistence supplies the atomic Record target.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new_unbound_provider(
        claim_id: MetadataClaimId,
        value: impl Into<String>,
        provenance: FieldClaimProvenance,
        fetched_at: crate::ReceivedAt,
        expires_at: Option<DateTime<Utc>>,
        initial_status: FieldClaimStatus,
    ) -> Result<Self, FieldClaimError> {
        if !provenance.is_complete() {
            return Err(FieldClaimError::IncompleteProvenance);
        }
        Self::try_from_persisted(
            claim_id,
            None,
            None,
            value,
            provenance,
            fetched_at,
            expires_at,
            initial_status,
        )
    }

    /// Reconstruct a validated claim from durable state.
    ///
    /// This is the migration-safe constructor: it retains the persisted claim
    /// ID and permits explicitly incomplete legacy provenance without
    /// pretending that missing upstream evidence was observed.
    #[allow(clippy::too_many_arguments)]
    pub fn try_from_persisted(
        claim_id: MetadataClaimId,
        record_id: Option<RecordId>,
        field_key: Option<FieldKey>,
        value: impl Into<String>,
        provenance: FieldClaimProvenance,
        fetched_at: crate::ReceivedAt,
        expires_at: Option<DateTime<Utc>>,
        initial_status: FieldClaimStatus,
    ) -> Result<Self, FieldClaimError> {
        if record_id.is_some() != field_key.is_some() {
            return Err(FieldClaimError::IncompleteTarget);
        }
        let value = value.into();
        validate_field_value(&value)?;
        let fetched_at = fetched_at.value();
        if expires_at.is_some_and(|expires_at| expires_at <= fetched_at) {
            return Err(FieldClaimError::ExpiryNotAfterFetch);
        }
        Ok(Self {
            claim_id,
            record_id,
            field_key,
            value,
            provenance,
            fetched_at,
            expires_at,
            initial_status,
        })
    }

    pub fn claim_id(&self) -> MetadataClaimId {
        self.claim_id
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }

    pub fn field_key(&self) -> Option<&FieldKey> {
        self.field_key.as_ref()
    }

    pub fn source(&self) -> &NamespaceKey {
        self.provenance.source_namespace()
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn locale(&self) -> Option<&str> {
        self.provenance.locale().map(MetadataLocale::as_str)
    }

    pub const fn provenance(&self) -> &FieldClaimProvenance {
        &self.provenance
    }

    pub const fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }

    pub const fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    pub const fn initial_status(&self) -> FieldClaimStatus {
        self.initial_status
    }

    /// A claim with no declared expiry never goes stale on its own; absence
    /// of a cache directive is not absence of validity.
    pub fn is_fresh(&self, now: DateTime<Utc>) -> bool {
        self.initial_status.can_project_fresh()
            && self.expires_at.is_none_or(|expires_at| now < expires_at)
    }

    pub fn status_at(&self, now: DateTime<Utc>) -> FieldClaimStatus {
        if self.initial_status == FieldClaimStatus::Fresh && !self.is_fresh(now) {
            FieldClaimStatus::Stale
        } else {
            self.initial_status
        }
    }
}

fn validate_field_value(value: &str) -> Result<(), FieldClaimError> {
    if value.is_empty()
        || value.len() > MAX_FIELD_VALUE_BYTES
        || value.chars().any(char::is_control)
    {
        Err(FieldClaimError::InvalidValue)
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldClaimLifecycleError {
    #[error("field claim lifecycle sequence must start at one")]
    InvalidSequence,
    #[error("field claim lifecycle transition is not permitted")]
    InvalidTransition,
    #[error("invalid, revoked, and superseded transitions require evidence")]
    MissingEvidence,
}

/// One append-only state transition for an immutable claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldClaimLifecycleEvent {
    claim_id: MetadataClaimId,
    sequence: u32,
    previous_status: FieldClaimStatus,
    status: FieldClaimStatus,
    occurred_at: DateTime<Utc>,
    evidence_digest: Option<Sha256Digest>,
}

impl FieldClaimLifecycleEvent {
    pub fn try_new(
        claim_id: MetadataClaimId,
        sequence: u32,
        previous_status: FieldClaimStatus,
        status: FieldClaimStatus,
        occurred_at: crate::ReceivedAt,
        evidence_digest: Option<Sha256Digest>,
    ) -> Result<Self, FieldClaimLifecycleError> {
        if sequence == 0 {
            return Err(FieldClaimLifecycleError::InvalidSequence);
        }
        if !previous_status.can_transition_to(status) {
            return Err(FieldClaimLifecycleError::InvalidTransition);
        }
        if matches!(
            status,
            FieldClaimStatus::Invalid | FieldClaimStatus::Revoked | FieldClaimStatus::Superseded
        ) && evidence_digest.is_none()
        {
            return Err(FieldClaimLifecycleError::MissingEvidence);
        }
        Ok(Self {
            claim_id,
            sequence,
            previous_status,
            status,
            occurred_at: occurred_at.value(),
            evidence_digest,
        })
    }

    pub const fn claim_id(&self) -> MetadataClaimId {
        self.claim_id
    }

    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    pub const fn previous_status(&self) -> FieldClaimStatus {
        self.previous_status
    }

    pub const fn status(&self) -> FieldClaimStatus {
        self.status
    }

    pub const fn occurred_at(&self) -> DateTime<Utc> {
        self.occurred_at
    }

    pub const fn evidence_digest(&self) -> Option<&Sha256Digest> {
        self.evidence_digest.as_ref()
    }
}

/// A user-owned value for one field. First-class, never silently overwritten
/// by a provider refresh — the constitution's rule that provider metadata is
/// not user-owned truth, applied to one field at a time.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldOverride {
    value: String,
    created_at: DateTime<Utc>,
}

impl FieldOverride {
    pub fn try_new(
        value: impl Into<String>,
        created_at: crate::ReceivedAt,
    ) -> Result<Self, FieldClaimError> {
        let value = value.into();
        validate_field_value(&value)?;
        Ok(Self {
            value,
            created_at: created_at.value(),
        })
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

/// A profile-owned field decision.
///
/// Unlike the pre-M2 [`FieldOverride`], this value carries its complete owner
/// and target. Provider refresh and another profile's policy cannot change it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProfileFieldOverride {
    profile_id: ProfileId,
    record_id: RecordId,
    field_key: FieldKey,
    value: String,
    created_at: DateTime<Utc>,
}

impl ProfileFieldOverride {
    pub fn try_new(
        profile_id: ProfileId,
        record_id: RecordId,
        field_key: FieldKey,
        value: impl Into<String>,
        created_at: crate::ReceivedAt,
    ) -> Result<Self, FieldClaimError> {
        let value = value.into();
        validate_field_value(&value)?;
        Ok(Self {
            profile_id,
            record_id,
            field_key,
            value,
            created_at: created_at.value(),
        })
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub fn field_key(&self) -> &FieldKey {
        &self.field_key
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LastKnownGoodPolicy {
    Allow,
    Deny,
}

/// Profile-owned policy for selecting one visible value from shared claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataProjectionPolicy {
    profile_id: ProfileId,
    preferred_provider_id: Option<MetadataProviderId>,
    preferred_locale: Option<MetadataLocale>,
    original_locale: Option<MetadataLocale>,
    allow_english_fallback: bool,
    last_known_good: LastKnownGoodPolicy,
}

impl MetadataProjectionPolicy {
    pub fn new(
        profile_id: ProfileId,
        preferred_provider_id: Option<MetadataProviderId>,
        preferred_locale: Option<MetadataLocale>,
        original_locale: Option<MetadataLocale>,
        allow_english_fallback: bool,
        last_known_good: LastKnownGoodPolicy,
    ) -> Self {
        Self {
            profile_id,
            preferred_provider_id,
            preferred_locale,
            original_locale,
            allow_english_fallback,
            last_known_good,
        }
    }

    pub fn default_for_profile(profile_id: ProfileId) -> Self {
        Self::new(
            profile_id,
            None,
            None,
            None,
            false,
            LastKnownGoodPolicy::Allow,
        )
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn preferred_provider_id(&self) -> Option<&MetadataProviderId> {
        self.preferred_provider_id.as_ref()
    }

    pub fn preferred_locale(&self) -> Option<&MetadataLocale> {
        self.preferred_locale.as_ref()
    }

    pub fn original_locale(&self) -> Option<&MetadataLocale> {
        self.original_locale.as_ref()
    }

    pub const fn allow_english_fallback(&self) -> bool {
        self.allow_english_fallback
    }

    pub const fn last_known_good(&self) -> LastKnownGoodPolicy {
        self.last_known_good
    }

    fn locale_rank(&self, locale: Option<&MetadataLocale>) -> Option<u8> {
        let Some(preferred) = self.preferred_locale() else {
            return Some(0);
        };
        match locale {
            Some(locale) if locale == preferred => Some(0),
            Some(locale) if self.original_locale() == Some(locale) => Some(1),
            Some(locale) if self.allow_english_fallback && locale.language() == "en" => Some(2),
            None => Some(3),
            Some(_) => None,
        }
    }
}

/// Which tier of the resolution order actually supplied the displayed value.
///
/// Exists so a caller can render "why this value" without re-deriving the
/// answer from raw claims, matching the constitution's requirement that a
/// user can see why a record matched without opening logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldResolutionTier {
    UserOverride,
    PreferredProviderClaim,
    FallbackProviderClaim,
    LastKnownGood,
    Empty,
}

/// Exact immutable claim evidence behind a selected projected value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedFieldProvenance {
    claim_id: MetadataClaimId,
    record_id: Option<RecordId>,
    field_key: Option<FieldKey>,
    provenance: FieldClaimProvenance,
    fetched_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    status: FieldClaimStatus,
}

impl ResolvedFieldProvenance {
    fn from_claim(claim: &FieldClaim, status: FieldClaimStatus) -> Self {
        Self {
            claim_id: claim.claim_id(),
            record_id: claim.record_id(),
            field_key: claim.field_key().cloned(),
            provenance: claim.provenance().clone(),
            fetched_at: claim.fetched_at(),
            expires_at: claim.expires_at(),
            status,
        }
    }

    pub const fn claim_id(&self) -> MetadataClaimId {
        self.claim_id
    }

    pub const fn record_id(&self) -> Option<RecordId> {
        self.record_id
    }

    pub fn field_key(&self) -> Option<&FieldKey> {
        self.field_key.as_ref()
    }

    pub const fn claim_provenance(&self) -> &FieldClaimProvenance {
        &self.provenance
    }

    pub const fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }

    pub const fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    pub const fn status(&self) -> FieldClaimStatus {
        self.status
    }
}

/// The outcome of resolving one field from its override and claims.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedField {
    tier: FieldResolutionTier,
    value: Option<String>,
    source: Option<NamespaceKey>,
    /// True only in the `LastKnownGood` tier: every claim considered had
    /// expired, and the most recently fetched one was used anyway rather
    /// than showing nothing. Absence, timeout, and expiry are not deletion.
    is_stale: bool,
    /// Kept out of the legacy direct serialization shape. Contract adapters
    /// project this explicitly into the versioned M2 DTO.
    #[serde(skip)]
    provenance: Option<ResolvedFieldProvenance>,
}

impl ResolvedField {
    pub const fn tier(&self) -> FieldResolutionTier {
        self.tier
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn source(&self) -> Option<&NamespaceKey> {
        self.source.as_ref()
    }

    pub const fn is_stale(&self) -> bool {
        self.is_stale
    }

    pub const fn provenance(&self) -> Option<&ResolvedFieldProvenance> {
        self.provenance.as_ref()
    }
}

/// Resolve one field to the value that should be displayed.
///
/// Order: user override, then a fresh claim from `preferred_source` matching
/// `preferred_locale` (when both are given), then any other fresh claim, then
/// the most recently fetched claim regardless of freshness, then empty.
///
/// `now` is supplied by the caller rather than read from an ambient clock, so
/// the function is deterministic and testable: the same inputs always
/// produce the same resolution.
///
/// Tie-breaking within a tier is always by most-recent `fetched_at`, then by
/// source namespace, so two callers resolving the same claim set never
/// disagree on which provider wins.
/// True when `candidate` should replace `current` as the tracked winner.
///
/// Matches `Iterator::max_by`'s documented tie-break exactly: forward
/// iteration, and on a full tie the LAST element wins. Preserved deliberately
/// so the single-pass selection below is behaviorally identical to the
/// two-pass `Vec`-based version it replaces, not merely similar.
fn prefer(current: Option<&FieldClaim>, candidate: &FieldClaim) -> bool {
    match current {
        None => true,
        Some(current) => match candidate.fetched_at().cmp(&current.fetched_at()) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Equal => candidate.source() >= current.source(),
            std::cmp::Ordering::Less => false,
        },
    }
}

pub fn resolve_field(
    override_: Option<&FieldOverride>,
    claims: &[FieldClaim],
    preferred_source: Option<&NamespaceKey>,
    preferred_locale: Option<&str>,
    now: DateTime<Utc>,
) -> ResolvedField {
    if let Some(override_) = override_ {
        return ResolvedField {
            tier: FieldResolutionTier::UserOverride,
            value: Some(override_.value().to_owned()),
            source: None,
            is_stale: false,
            provenance: None,
        };
    }

    // Single pass, O(1) extra space regardless of claim count. The prior
    // version built up to three `Vec<&FieldClaim>` proportional to the input
    // slice, which has no declared upper bound; a long claim history could
    // grow past the 192 MiB process ceiling during resolution alone. This
    // tracks only the current winner per tier.
    let mut best_preferred: Option<&FieldClaim> = None;
    let mut best_fallback: Option<&FieldClaim> = None;
    let mut best_any: Option<&FieldClaim> = None;

    for claim in claims {
        let status = claim.status_at(now);
        if status.can_project_last_known_good() && prefer(best_any, claim) {
            best_any = Some(claim);
        }
        if !status.can_project_fresh() {
            continue;
        }
        if prefer(best_fallback, claim) {
            best_fallback = Some(claim);
        }
        if let (Some(preferred_source), Some(preferred_locale)) =
            (preferred_source, preferred_locale)
        {
            if claim.source() == preferred_source
                && claim.locale() == Some(preferred_locale)
                && prefer(best_preferred, claim)
            {
                best_preferred = Some(claim);
            }
        }
    }

    if let Some(claim) = best_preferred {
        return ResolvedField {
            tier: FieldResolutionTier::PreferredProviderClaim,
            value: Some(claim.value().to_owned()),
            source: Some(claim.source().clone()),
            is_stale: false,
            provenance: Some(ResolvedFieldProvenance::from_claim(
                claim,
                FieldClaimStatus::Fresh,
            )),
        };
    }

    if let Some(claim) = best_fallback {
        return ResolvedField {
            tier: FieldResolutionTier::FallbackProviderClaim,
            value: Some(claim.value().to_owned()),
            source: Some(claim.source().clone()),
            is_stale: false,
            provenance: Some(ResolvedFieldProvenance::from_claim(
                claim,
                FieldClaimStatus::Fresh,
            )),
        };
    }

    if let Some(claim) = best_any {
        return ResolvedField {
            tier: FieldResolutionTier::LastKnownGood,
            value: Some(claim.value().to_owned()),
            source: Some(claim.source().clone()),
            is_stale: true,
            provenance: Some(ResolvedFieldProvenance::from_claim(
                claim,
                claim.status_at(now),
            )),
        };
    }

    ResolvedField {
        tier: FieldResolutionTier::Empty,
        value: None,
        source: None,
        is_stale: false,
        provenance: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldResolutionError {
    #[error("field claim IDs must be unique within one resolution input")]
    DuplicateClaimId,
    #[error("field claims in one resolution input must have one record and field target")]
    MixedClaimTarget,
    #[error("profile override does not belong to the projection policy profile")]
    OverrideProfileMismatch,
    #[error("profile override target does not match the claim target")]
    OverrideTargetMismatch,
    #[error("field claim lifecycle event references an unknown claim")]
    UnknownLifecycleClaim,
    #[error("field claim lifecycle sequence is incomplete or out of order")]
    InvalidLifecycleSequence,
    #[error("field claim lifecycle transition chain is invalid")]
    InvalidLifecycleTransition,
    #[error("field claim lifecycle event predates its claim or prior event")]
    InvalidLifecycleTime,
}

fn effective_status(
    claim: &FieldClaim,
    lifecycle_events: &[FieldClaimLifecycleEvent],
    now: DateTime<Utc>,
) -> Result<FieldClaimStatus, FieldResolutionError> {
    let mut status = claim.initial_status();
    let mut expected_sequence = 1_u32;
    let mut prior_time = claim.fetched_at();
    for event in lifecycle_events
        .iter()
        .filter(|event| event.claim_id() == claim.claim_id())
    {
        if event.sequence() != expected_sequence {
            return Err(FieldResolutionError::InvalidLifecycleSequence);
        }
        if event.previous_status() != status || !status.can_transition_to(event.status()) {
            return Err(FieldResolutionError::InvalidLifecycleTransition);
        }
        if event.occurred_at() < prior_time {
            return Err(FieldResolutionError::InvalidLifecycleTime);
        }
        status = event.status();
        prior_time = event.occurred_at();
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or(FieldResolutionError::InvalidLifecycleSequence)?;
    }
    if status == FieldClaimStatus::Fresh
        && claim
            .expires_at()
            .is_some_and(|expires_at| now >= expires_at)
    {
        status = FieldClaimStatus::Stale;
    }
    Ok(status)
}

fn validate_resolution_input(
    override_: Option<&ProfileFieldOverride>,
    claims: &[FieldClaim],
    lifecycle_events: &[FieldClaimLifecycleEvent],
    policy: &MetadataProjectionPolicy,
) -> Result<(), FieldResolutionError> {
    let mut record_id = None;
    let mut field_key: Option<&FieldKey> = None;
    for (index, claim) in claims.iter().enumerate() {
        if claims[..index]
            .iter()
            .any(|prior| prior.claim_id() == claim.claim_id())
        {
            return Err(FieldResolutionError::DuplicateClaimId);
        }
        if let Some(candidate) = claim.record_id() {
            if record_id.is_some_and(|record_id| record_id != candidate) {
                return Err(FieldResolutionError::MixedClaimTarget);
            }
            record_id = Some(candidate);
        }
        if let Some(candidate) = claim.field_key() {
            if field_key.is_some_and(|field_key| field_key != candidate) {
                return Err(FieldResolutionError::MixedClaimTarget);
            }
            field_key = Some(candidate);
        }
    }
    if lifecycle_events.iter().any(|event| {
        !claims
            .iter()
            .any(|claim| claim.claim_id() == event.claim_id())
    }) {
        return Err(FieldResolutionError::UnknownLifecycleClaim);
    }
    if let Some(override_) = override_ {
        if override_.profile_id() != policy.profile_id() {
            return Err(FieldResolutionError::OverrideProfileMismatch);
        }
        if record_id.is_some_and(|record_id| record_id != override_.record_id())
            || field_key.is_some_and(|field_key| field_key != override_.field_key())
        {
            return Err(FieldResolutionError::OverrideTargetMismatch);
        }
    }
    Ok(())
}

fn provider_is_preferred(claim: &FieldClaim, policy: &MetadataProjectionPolicy) -> bool {
    policy
        .preferred_provider_id()
        .is_some_and(|preferred| claim.provenance().provider_id() == Some(preferred))
}

fn prefer_for_policy(
    current: Option<&FieldClaim>,
    candidate: &FieldClaim,
    policy: &MetadataProjectionPolicy,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    let candidate_locale = policy
        .locale_rank(candidate.provenance().locale())
        .expect("only compatible claims are ranked");
    let current_locale = policy
        .locale_rank(current.provenance().locale())
        .expect("only compatible claims are ranked");
    candidate_locale
        .cmp(&current_locale)
        .reverse()
        .then_with(|| {
            provider_is_preferred(candidate, policy).cmp(&provider_is_preferred(current, policy))
        })
        .then_with(|| candidate.fetched_at().cmp(&current.fetched_at()))
        .then_with(|| candidate.source().cmp(current.source()))
        .then_with(|| {
            candidate
                .provenance()
                .source_identifier()
                .cmp(&current.provenance().source_identifier())
        })
        .then_with(|| candidate.claim_id().uuid().cmp(&current.claim_id().uuid()))
        == std::cmp::Ordering::Greater
}

fn resolved_claim(
    tier: FieldResolutionTier,
    claim: &FieldClaim,
    status: FieldClaimStatus,
) -> ResolvedField {
    ResolvedField {
        tier,
        value: Some(claim.value().to_owned()),
        source: Some(claim.source().clone()),
        is_stale: status != FieldClaimStatus::Fresh,
        provenance: Some(ResolvedFieldProvenance::from_claim(claim, status)),
    }
}

/// Resolve a profile's visible field from immutable claims and lifecycle.
///
/// Claims must all address one Record field. Lifecycle events must be supplied
/// in ascending sequence order for each claim. The resolver validates those
/// invariants before applying an override, so corrupt hidden evidence cannot
/// be masked by a valid-looking projection.
pub fn resolve_profile_field(
    override_: Option<&ProfileFieldOverride>,
    claims: &[FieldClaim],
    lifecycle_events: &[FieldClaimLifecycleEvent],
    policy: &MetadataProjectionPolicy,
    now: DateTime<Utc>,
) -> Result<ResolvedField, FieldResolutionError> {
    validate_resolution_input(override_, claims, lifecycle_events, policy)?;

    // Validate every lifecycle before an override can hide corrupt evidence.
    // Keep resolution O(1) in additional space even for long claim histories.
    for claim in claims {
        effective_status(claim, lifecycle_events, now)?;
    }

    if let Some(override_) = override_ {
        return Ok(ResolvedField {
            tier: FieldResolutionTier::UserOverride,
            value: Some(override_.value().to_owned()),
            source: None,
            is_stale: false,
            provenance: None,
        });
    }

    let mut preferred = None;
    let mut fallback = None;
    let mut last_known_good = None;
    for claim in claims {
        let status = effective_status(claim, lifecycle_events, now)?;
        if policy.locale_rank(claim.provenance().locale()).is_none() {
            continue;
        }
        if status.can_project_fresh() {
            let exact_preference = provider_is_preferred(claim, policy)
                && policy.locale_rank(claim.provenance().locale()) == Some(0);
            let winner = if exact_preference {
                &mut preferred
            } else {
                &mut fallback
            };
            if prefer_for_policy(*winner, claim, policy) {
                *winner = Some(claim);
            }
        } else if status.can_project_last_known_good()
            && prefer_for_policy(last_known_good, claim, policy)
        {
            last_known_good = Some(claim);
        }
    }

    if let Some(claim) = preferred {
        return Ok(resolved_claim(
            FieldResolutionTier::PreferredProviderClaim,
            claim,
            FieldClaimStatus::Fresh,
        ));
    }
    if let Some(claim) = fallback {
        return Ok(resolved_claim(
            FieldResolutionTier::FallbackProviderClaim,
            claim,
            FieldClaimStatus::Fresh,
        ));
    }
    if policy.last_known_good() == LastKnownGoodPolicy::Allow {
        if let Some(claim) = last_known_good {
            let status = effective_status(claim, lifecycle_events, now)?;
            return Ok(resolved_claim(
                FieldResolutionTier::LastKnownGood,
                claim,
                status,
            ));
        }
    }
    Ok(ResolvedField {
        tier: FieldResolutionTier::Empty,
        value: None,
        source: None,
        is_stale: false,
        provenance: None,
    })
}

pub const RATING_FIXED_POINT_SCALE: u32 = 1_000;
pub const MAX_RATING_MILLIS: u32 = 1_000_000;

/// One provider rating scale in thousandths.
///
/// Fixed-point values avoid platform-dependent floating-point serialization.
/// For example, `7.8 / 10` is value `7_800` on scale `0..=10_000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RatingScale {
    minimum_millis: u32,
    maximum_millis: u32,
}

impl RatingScale {
    pub fn try_new(minimum_millis: u32, maximum_millis: u32) -> Result<Self, RatingClaimError> {
        if minimum_millis >= maximum_millis || maximum_millis > MAX_RATING_MILLIS {
            return Err(RatingClaimError::InvalidScale);
        }
        Ok(Self {
            minimum_millis,
            maximum_millis,
        })
    }

    pub const fn minimum_millis(self) -> u32 {
        self.minimum_millis
    }

    pub const fn maximum_millis(self) -> u32 {
        self.maximum_millis
    }

    pub const fn contains(self, value_millis: u32) -> bool {
        value_millis >= self.minimum_millis && value_millis <= self.maximum_millis
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RatingClaimError {
    #[error("rating scale must have an increasing bounded range")]
    InvalidScale,
    #[error("rating value is outside its declared scale")]
    ValueOutsideScale,
    #[error("a provider rating requires complete provenance")]
    IncompleteProvenance,
    #[error("rating expires_at cannot be at or before fetched_at")]
    ExpiryNotAfterFetch,
}

/// One provider's independently retained score for one Record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RatingClaim {
    claim_id: MetadataClaimId,
    record_id: RecordId,
    value_millis: u32,
    scale: RatingScale,
    provenance: FieldClaimProvenance,
    fetched_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    initial_status: FieldClaimStatus,
}

impl RatingClaim {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        claim_id: MetadataClaimId,
        record_id: RecordId,
        value_millis: u32,
        scale: RatingScale,
        provenance: FieldClaimProvenance,
        fetched_at: crate::ReceivedAt,
        expires_at: Option<DateTime<Utc>>,
        initial_status: FieldClaimStatus,
    ) -> Result<Self, RatingClaimError> {
        if !scale.contains(value_millis) {
            return Err(RatingClaimError::ValueOutsideScale);
        }
        if !provenance.is_complete() {
            return Err(RatingClaimError::IncompleteProvenance);
        }
        let fetched_at = fetched_at.value();
        if expires_at.is_some_and(|expires_at| expires_at <= fetched_at) {
            return Err(RatingClaimError::ExpiryNotAfterFetch);
        }
        Ok(Self {
            claim_id,
            record_id,
            value_millis,
            scale,
            provenance,
            fetched_at,
            expires_at,
            initial_status,
        })
    }

    pub const fn claim_id(&self) -> MetadataClaimId {
        self.claim_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub const fn value_millis(&self) -> u32 {
        self.value_millis
    }

    pub const fn scale(&self) -> RatingScale {
        self.scale
    }

    pub const fn provenance(&self) -> &FieldClaimProvenance {
        &self.provenance
    }

    pub const fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }

    pub const fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.expires_at
    }

    pub const fn initial_status(&self) -> FieldClaimStatus {
        self.initial_status
    }

    pub fn status_at(&self, now: DateTime<Utc>) -> FieldClaimStatus {
        if self.initial_status == FieldClaimStatus::Fresh
            && self.expires_at.is_some_and(|expires_at| now >= expires_at)
        {
            FieldClaimStatus::Stale
        } else {
            self.initial_status
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataFieldGroup {
    Artwork,
    BasicInfo,
    Details,
    ReleaseDates,
    Credits,
    ProductionCompanies,
    Networks,
    Episodes,
    SeasonArtwork,
    Recommendations,
    Collections,
    Trailers,
    WatchProviders,
}

impl MetadataFieldGroup {
    pub const ALL: &'static [Self] = &[
        Self::Artwork,
        Self::BasicInfo,
        Self::Details,
        Self::ReleaseDates,
        Self::Credits,
        Self::ProductionCompanies,
        Self::Networks,
        Self::Episodes,
        Self::SeasonArtwork,
        Self::Recommendations,
        Self::Collections,
        Self::Trailers,
        Self::WatchProviders,
    ];
}

/// Profile-owned enrichment configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EnrichmentPolicy {
    projection_policy: MetadataProjectionPolicy,
    region: Option<MetadataRegion>,
    enabled_field_groups: Vec<MetadataFieldGroup>,
}

impl EnrichmentPolicy {
    pub fn new(
        projection_policy: MetadataProjectionPolicy,
        region: Option<MetadataRegion>,
        mut enabled_field_groups: Vec<MetadataFieldGroup>,
    ) -> Self {
        enabled_field_groups.sort_unstable();
        enabled_field_groups.dedup();
        Self {
            projection_policy,
            region,
            enabled_field_groups,
        }
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.projection_policy.profile_id()
    }

    pub const fn projection_policy(&self) -> &MetadataProjectionPolicy {
        &self.projection_policy
    }

    pub fn locale(&self) -> Option<&MetadataLocale> {
        self.projection_policy.preferred_locale()
    }

    pub fn region(&self) -> Option<&MetadataRegion> {
        self.region.as_ref()
    }

    pub fn enabled_field_groups(&self) -> &[MetadataFieldGroup] {
        &self.enabled_field_groups
    }

    pub fn field_group_is_enabled(&self, field_group: MetadataFieldGroup) -> bool {
        self.enabled_field_groups
            .binary_search(&field_group)
            .is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("resolved claim provenance does not match the projection target")]
pub struct MetadataProjectionError;

/// One profile's selected projection for one Record field.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetadataProjection {
    profile_id: ProfileId,
    record_id: RecordId,
    field_key: FieldKey,
    resolved_field: ResolvedField,
    projected_at: DateTime<Utc>,
}

impl MetadataProjection {
    pub fn try_new(
        profile_id: ProfileId,
        record_id: RecordId,
        field_key: FieldKey,
        resolved_field: ResolvedField,
        projected_at: crate::ReceivedAt,
    ) -> Result<Self, MetadataProjectionError> {
        if resolved_field.provenance().is_some_and(|provenance| {
            provenance
                .record_id()
                .is_some_and(|claim_record| claim_record != record_id)
                || provenance
                    .field_key()
                    .is_some_and(|claim_field| claim_field != &field_key)
        }) {
            return Err(MetadataProjectionError);
        }
        Ok(Self {
            profile_id,
            record_id,
            field_key,
            resolved_field,
            projected_at: projected_at.value(),
        })
    }

    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub fn field_key(&self) -> &FieldKey {
        &self.field_key
    }

    pub const fn resolved_field(&self) -> &ResolvedField {
        &self.resolved_field
    }

    pub const fn projected_at(&self) -> DateTime<Utc> {
        self.projected_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataDataClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
}

impl MetadataDataClassification {
    pub const fn is_within(self, maximum: Self) -> bool {
        (self as u8) <= (maximum as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataCachePurpose {
    MetadataEnrichment,
    DisplayProjection,
    RatingLookup,
    OfflineRead,
}

pub const MAX_CACHE_ROUTE_BYTES: usize = 512;
pub const MAX_TERMS_REVISION_BYTES: usize = 128;
pub const MAX_CACHE_CLAIM_IDS: usize = 256;
pub const METADATA_FRESH_SECONDS: i64 = 24 * 60 * 60;
pub const METADATA_STALE_WHILE_REFRESHING_SECONDS: i64 = 12 * 60 * 60;
pub const METADATA_STALE_ON_ERROR_SECONDS: i64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MetadataCacheKeyError {
    #[error("credential-reference version must be at least one when present")]
    InvalidCredentialReferenceVersion,
    #[error("provider route must be a bounded canonical non-secret route")]
    InvalidRoute,
    #[error("cache source identifier must be bounded and contain no control characters")]
    InvalidSourceIdentifier,
    #[error("terms revision must be a bounded canonical identifier")]
    InvalidTermsRevision,
    #[error("cache schema version must be at least one")]
    InvalidSchemaVersion,
}

/// Complete non-secret partition key for one metadata cache entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct MetadataCacheKey {
    provider_id: MetadataProviderId,
    credential_reference_version: Option<u64>,
    record_id: RecordId,
    resolved_provider_route: String,
    grain: Grain,
    source_namespace: NamespaceKey,
    source_identifier: String,
    locale: Option<MetadataLocale>,
    region: Option<MetadataRegion>,
    field_group: MetadataFieldGroup,
    settings_fingerprint: Sha256Digest,
    configuration_digest: Sha256Digest,
    schema_version: u32,
    purpose: MetadataCachePurpose,
    terms_revision: String,
    classification: MetadataDataClassification,
}

impl MetadataCacheKey {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        provider_id: MetadataProviderId,
        credential_reference_version: Option<u64>,
        record_id: RecordId,
        resolved_provider_route: impl Into<String>,
        grain: Grain,
        source_namespace: NamespaceKey,
        source_identifier: impl Into<String>,
        locale: Option<MetadataLocale>,
        region: Option<MetadataRegion>,
        field_group: MetadataFieldGroup,
        settings_fingerprint: Sha256Digest,
        configuration_digest: Sha256Digest,
        schema_version: u32,
        purpose: MetadataCachePurpose,
        terms_revision: impl Into<String>,
        classification: MetadataDataClassification,
    ) -> Result<Self, MetadataCacheKeyError> {
        if credential_reference_version == Some(0) {
            return Err(MetadataCacheKeyError::InvalidCredentialReferenceVersion);
        }
        let resolved_provider_route = resolved_provider_route.into();
        if resolved_provider_route.is_empty()
            || resolved_provider_route.len() > MAX_CACHE_ROUTE_BYTES
            || resolved_provider_route.trim() != resolved_provider_route
            || !resolved_provider_route.bytes().all(|byte| {
                !byte.is_ascii_uppercase()
                    && (byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
            })
        {
            return Err(MetadataCacheKeyError::InvalidRoute);
        }
        let source_identifier = source_identifier.into();
        if source_identifier.is_empty()
            || source_identifier.len() > MAX_SOURCE_IDENTIFIER_BYTES
            || source_identifier.trim() != source_identifier
            || source_identifier.chars().any(char::is_control)
        {
            return Err(MetadataCacheKeyError::InvalidSourceIdentifier);
        }
        let terms_revision = terms_revision.into();
        if terms_revision.is_empty()
            || terms_revision.len() > MAX_TERMS_REVISION_BYTES
            || terms_revision.trim() != terms_revision
            || !terms_revision.bytes().all(|byte| {
                !byte.is_ascii_uppercase()
                    && (byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
            })
        {
            return Err(MetadataCacheKeyError::InvalidTermsRevision);
        }
        if schema_version == 0 {
            return Err(MetadataCacheKeyError::InvalidSchemaVersion);
        }
        Ok(Self {
            provider_id,
            credential_reference_version,
            record_id,
            resolved_provider_route,
            grain,
            source_namespace,
            source_identifier,
            locale,
            region,
            field_group,
            settings_fingerprint,
            configuration_digest,
            schema_version,
            purpose,
            terms_revision,
            classification,
        })
    }

    pub fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }

    pub const fn credential_reference_version(&self) -> Option<u64> {
        self.credential_reference_version
    }

    pub const fn record_id(&self) -> RecordId {
        self.record_id
    }

    pub fn resolved_provider_route(&self) -> &str {
        &self.resolved_provider_route
    }

    pub const fn grain(&self) -> Grain {
        self.grain
    }

    pub fn source_namespace(&self) -> &NamespaceKey {
        &self.source_namespace
    }

    pub fn source_identifier(&self) -> &str {
        &self.source_identifier
    }

    pub fn locale(&self) -> Option<&MetadataLocale> {
        self.locale.as_ref()
    }

    pub fn region(&self) -> Option<&MetadataRegion> {
        self.region.as_ref()
    }

    pub const fn field_group(&self) -> MetadataFieldGroup {
        self.field_group
    }

    pub const fn settings_fingerprint(&self) -> &Sha256Digest {
        &self.settings_fingerprint
    }

    pub const fn configuration_digest(&self) -> &Sha256Digest {
        &self.configuration_digest
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn purpose(&self) -> MetadataCachePurpose {
        self.purpose
    }

    pub fn terms_revision(&self) -> &str {
        &self.terms_revision
    }

    pub const fn classification(&self) -> MetadataDataClassification {
        self.classification
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataCacheInvalidationReason {
    ProviderConfigurationChanged,
    CredentialRotated,
    ProjectionPolicyChanged,
    TermsChanged,
    ExplicitRetraction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataCacheInvalidation {
    reason: MetadataCacheInvalidationReason,
    invalidated_at: DateTime<Utc>,
}

impl MetadataCacheInvalidation {
    pub const fn reason(&self) -> MetadataCacheInvalidationReason {
        self.reason
    }

    pub const fn invalidated_at(&self) -> DateTime<Utc> {
        self.invalidated_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataCacheReadState {
    Fresh,
    StaleWhileRefreshing,
    StaleOnError,
    Expired,
    Invalidated,
    PartitionDenied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MetadataCacheEntryError {
    #[error("metadata cache contains too many claim references")]
    TooManyClaims,
    #[error("metadata cache claim references must be unique")]
    DuplicateClaim,
    #[error("metadata freshness windows are out of order or exceed policy caps")]
    InvalidFreshnessWindow,
    #[error("metadata cache invalidation cannot predate entry creation")]
    InvalidInvalidationTime,
}

/// Durable cache metadata referencing immutable claims, never raw provider
/// response bodies or credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataCacheEntry {
    key: MetadataCacheKey,
    claim_ids: Vec<MetadataClaimId>,
    created_at: DateTime<Utc>,
    fresh_until: DateTime<Utc>,
    stale_while_refreshing_until: DateTime<Utc>,
    stale_on_error_until: DateTime<Utc>,
    invalidation: Option<MetadataCacheInvalidation>,
}

impl MetadataCacheEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        key: MetadataCacheKey,
        claim_ids: Vec<MetadataClaimId>,
        created_at: crate::ReceivedAt,
        fresh_until: DateTime<Utc>,
        stale_while_refreshing_until: DateTime<Utc>,
        stale_on_error_until: DateTime<Utc>,
    ) -> Result<Self, MetadataCacheEntryError> {
        if claim_ids.len() > MAX_CACHE_CLAIM_IDS {
            return Err(MetadataCacheEntryError::TooManyClaims);
        }
        for (index, claim_id) in claim_ids.iter().enumerate() {
            if claim_ids[..index].contains(claim_id) {
                return Err(MetadataCacheEntryError::DuplicateClaim);
            }
        }
        let created_at = created_at.value();
        let fresh_cap = created_at + chrono::Duration::seconds(METADATA_FRESH_SECONDS);
        let refreshing_cap =
            fresh_until + chrono::Duration::seconds(METADATA_STALE_WHILE_REFRESHING_SECONDS);
        let stale_error_cap =
            created_at + chrono::Duration::seconds(METADATA_STALE_ON_ERROR_SECONDS);
        if fresh_until < created_at
            || fresh_until > fresh_cap
            || stale_while_refreshing_until < fresh_until
            || stale_while_refreshing_until > refreshing_cap
            || stale_on_error_until < stale_while_refreshing_until
            || stale_on_error_until > stale_error_cap
        {
            return Err(MetadataCacheEntryError::InvalidFreshnessWindow);
        }
        Ok(Self {
            key,
            claim_ids,
            created_at,
            fresh_until,
            stale_while_refreshing_until,
            stale_on_error_until,
            invalidation: None,
        })
    }

    pub const fn key(&self) -> &MetadataCacheKey {
        &self.key
    }

    pub fn claim_ids(&self) -> &[MetadataClaimId] {
        &self.claim_ids
    }

    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub const fn fresh_until(&self) -> DateTime<Utc> {
        self.fresh_until
    }

    pub const fn stale_while_refreshing_until(&self) -> DateTime<Utc> {
        self.stale_while_refreshing_until
    }

    pub const fn stale_on_error_until(&self) -> DateTime<Utc> {
        self.stale_on_error_until
    }

    pub const fn invalidation(&self) -> Option<&MetadataCacheInvalidation> {
        self.invalidation.as_ref()
    }

    pub fn invalidated(
        &self,
        reason: MetadataCacheInvalidationReason,
        invalidated_at: crate::ReceivedAt,
    ) -> Result<Self, MetadataCacheEntryError> {
        let invalidated_at = invalidated_at.value();
        if invalidated_at < self.created_at {
            return Err(MetadataCacheEntryError::InvalidInvalidationTime);
        }
        let mut next = self.clone();
        next.invalidation = Some(MetadataCacheInvalidation {
            reason,
            invalidated_at,
        });
        Ok(next)
    }

    /// Decide whether this exact partition can serve an online or offline
    /// caller. Offline reads may use the stale-on-error window but never cross
    /// a purpose or classification partition.
    pub fn read_state(
        &self,
        now: DateTime<Utc>,
        purpose: MetadataCachePurpose,
        maximum_classification: MetadataDataClassification,
        offline: bool,
    ) -> MetadataCacheReadState {
        if purpose != self.key.purpose()
            || !self.key.classification().is_within(maximum_classification)
        {
            return MetadataCacheReadState::PartitionDenied;
        }
        if self
            .invalidation
            .as_ref()
            .is_some_and(|invalidation| invalidation.invalidated_at() <= now)
        {
            return MetadataCacheReadState::Invalidated;
        }
        if now < self.fresh_until {
            MetadataCacheReadState::Fresh
        } else if now < self.stale_while_refreshing_until {
            MetadataCacheReadState::StaleWhileRefreshing
        } else if offline && now < self.stale_on_error_until {
            MetadataCacheReadState::StaleOnError
        } else {
            MetadataCacheReadState::Expired
        }
    }
}

pub const MAX_ATTRIBUTION_TEXT_BYTES: usize = 256;
pub const MAX_ATTRIBUTION_URL_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MetadataAttributionError {
    #[error("attribution text must be non-empty, bounded, and contain no control characters")]
    InvalidText,
    #[error("attribution URL must be a bounded HTTPS URL without control characters")]
    InvalidUrl,
}

/// Provider attribution that must travel with a projected provider field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MetadataAttribution {
    provider_id: MetadataProviderId,
    text: String,
    documentation_url: String,
}

impl MetadataAttribution {
    pub fn try_new(
        provider_id: MetadataProviderId,
        text: impl Into<String>,
        documentation_url: impl Into<String>,
    ) -> Result<Self, MetadataAttributionError> {
        let text = text.into();
        if text.is_empty()
            || text.len() > MAX_ATTRIBUTION_TEXT_BYTES
            || text.trim() != text
            || text.chars().any(char::is_control)
        {
            return Err(MetadataAttributionError::InvalidText);
        }
        let documentation_url = documentation_url.into();
        if !is_safe_attribution_url(&documentation_url) {
            return Err(MetadataAttributionError::InvalidUrl);
        }
        Ok(Self {
            provider_id,
            text,
            documentation_url,
        })
    }

    pub fn provider_id(&self) -> &MetadataProviderId {
        &self.provider_id
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn documentation_url(&self) -> &str {
        &self.documentation_url
    }
}

fn is_safe_attribution_url(value: &str) -> bool {
    if value.len() > MAX_ATTRIBUTION_URL_BYTES
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || value.contains('#')
    {
        return false;
    }
    let Some(remainder) = value.strip_prefix("https://") else {
        return false;
    };
    let authority = remainder
        .split_once(['/', '?'])
        .map_or(remainder, |(authority, _)| authority);
    if authority.is_empty() || authority.contains('@') {
        return false;
    }

    // Attribution links are public documentation links, not arbitrary URL
    // transports. A bounded DNS host (plus an optional numeric port) keeps
    // the display boundary deterministic without adding a parser dependency.
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    if host.is_empty()
        || host.len() > 253
        || host.starts_with('.')
        || host.ends_with('.')
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return false;
    }
    port.is_none_or(|port| {
        !port.is_empty()
            && port.bytes().all(|byte| byte.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|port| port != 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReceivedAt;
    use chrono::TimeZone;
    use proptest::prelude::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0)
            .single()
            .expect("valid instant")
    }

    fn received(seconds: i64) -> ReceivedAt {
        ReceivedAt::from_application_clock(at(seconds))
    }

    fn ns(value: &str) -> NamespaceKey {
        NamespaceKey::try_new(value).expect("valid namespace")
    }

    fn claim(source: &str, value: &str, fetched: i64, expires: Option<i64>) -> FieldClaim {
        FieldClaim::try_new(ns(source), value, None, received(fetched), expires.map(at))
            .expect("valid claim")
    }

    fn localized_claim(
        source: &str,
        value: &str,
        locale: &str,
        fetched: i64,
        expires: Option<i64>,
    ) -> FieldClaim {
        FieldClaim::try_new(
            ns(source),
            value,
            Some(locale.to_owned()),
            received(fetched),
            expires.map(at),
        )
        .expect("valid localized claim")
    }

    fn provider(value: &str) -> MetadataProviderId {
        MetadataProviderId::try_new(value).expect("valid provider")
    }

    fn locale(value: &str) -> MetadataLocale {
        MetadataLocale::try_new(value).expect("valid locale")
    }

    fn digest(byte: u8) -> Sha256Digest {
        Sha256Digest::from_bytes(&[byte; 32])
    }

    fn provenance(
        provider_value: &str,
        source: &str,
        source_identifier: &str,
        locale_value: Option<&str>,
    ) -> FieldClaimProvenance {
        FieldClaimProvenance::try_new(
            provider(provider_value),
            ns(source),
            source_identifier,
            locale_value.map(locale),
            Some(MetadataRegion::try_new("ie").expect("valid region")),
            Some("v1".to_owned()),
            digest(7),
        )
        .expect("complete provenance")
    }

    #[allow(clippy::too_many_arguments)]
    fn provider_claim(
        record_id: RecordId,
        field_key: &FieldKey,
        provider_value: &str,
        source: &str,
        source_identifier: &str,
        value: &str,
        locale_value: Option<&str>,
        fetched: i64,
        expires: Option<i64>,
        status: FieldClaimStatus,
    ) -> FieldClaim {
        FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record_id,
            field_key.clone(),
            value,
            provenance(provider_value, source, source_identifier, locale_value),
            received(fetched),
            expires.map(at),
            status,
        )
        .expect("valid provider claim")
    }

    fn projection_policy(
        profile_id: ProfileId,
        preferred_provider: Option<&str>,
        preferred_locale: Option<&str>,
        original_locale: Option<&str>,
        allow_english_fallback: bool,
        last_known_good: LastKnownGoodPolicy,
    ) -> MetadataProjectionPolicy {
        MetadataProjectionPolicy::new(
            profile_id,
            preferred_provider.map(provider),
            preferred_locale.map(locale),
            original_locale.map(locale),
            allow_english_fallback,
            last_known_good,
        )
    }

    fn cache_key(
        record_id: RecordId,
        purpose: MetadataCachePurpose,
        classification: MetadataDataClassification,
    ) -> MetadataCacheKey {
        MetadataCacheKey::try_new(
            provider("tmdb"),
            Some(2),
            record_id,
            "movie/details",
            Grain::Film,
            ns("tmdb.movie"),
            "550",
            Some(locale("en-ie")),
            Some(MetadataRegion::try_new("ie").expect("valid region")),
            MetadataFieldGroup::BasicInfo,
            digest(1),
            digest(2),
            1,
            purpose,
            "2026-08",
            classification,
        )
        .expect("valid cache key")
    }

    // ---------------------------------------------------------------------
    // Differential proof: the single-pass resolve_field must be exactly
    // equivalent to the Vec-based version it replaced, not merely similar.
    // The reference below is a deliberate frozen copy of the pre-rewrite
    // logic, kept test-only so the production code has no unbounded
    // allocation, while this proves the rewrite changed nothing observable.
    // ---------------------------------------------------------------------

    fn reference_resolve_field(
        override_: Option<&FieldOverride>,
        claims: &[FieldClaim],
        preferred_source: Option<&NamespaceKey>,
        preferred_locale: Option<&str>,
        now: DateTime<Utc>,
    ) -> ResolvedField {
        if let Some(override_) = override_ {
            return ResolvedField {
                tier: FieldResolutionTier::UserOverride,
                value: Some(override_.value().to_owned()),
                source: None,
                is_stale: false,
                provenance: None,
            };
        }

        let most_recent = |claims: &[&FieldClaim]| -> Option<FieldClaim> {
            claims
                .iter()
                .max_by(|left, right| {
                    left.fetched_at()
                        .cmp(&right.fetched_at())
                        .then_with(|| left.source().cmp(right.source()))
                })
                .map(|claim| (*claim).clone())
        };

        let fresh: Vec<&FieldClaim> = claims.iter().filter(|claim| claim.is_fresh(now)).collect();

        if let (Some(preferred_source), Some(preferred_locale)) =
            (preferred_source, preferred_locale)
        {
            let preferred: Vec<&FieldClaim> = fresh
                .iter()
                .copied()
                .filter(|claim| {
                    claim.source() == preferred_source && claim.locale() == Some(preferred_locale)
                })
                .collect();
            if let Some(claim) = most_recent(&preferred) {
                return ResolvedField {
                    tier: FieldResolutionTier::PreferredProviderClaim,
                    value: Some(claim.value().to_owned()),
                    source: Some(claim.source().clone()),
                    is_stale: false,
                    provenance: Some(ResolvedFieldProvenance::from_claim(
                        &claim,
                        FieldClaimStatus::Fresh,
                    )),
                };
            }
        }

        if let Some(claim) = most_recent(&fresh) {
            return ResolvedField {
                tier: FieldResolutionTier::FallbackProviderClaim,
                value: Some(claim.value().to_owned()),
                source: Some(claim.source().clone()),
                is_stale: false,
                provenance: Some(ResolvedFieldProvenance::from_claim(
                    &claim,
                    FieldClaimStatus::Fresh,
                )),
            };
        }

        let all: Vec<&FieldClaim> = claims.iter().collect();
        if let Some(claim) = most_recent(&all) {
            return ResolvedField {
                tier: FieldResolutionTier::LastKnownGood,
                value: Some(claim.value().to_owned()),
                source: Some(claim.source().clone()),
                is_stale: true,
                provenance: Some(ResolvedFieldProvenance::from_claim(
                    &claim,
                    claim.status_at(now),
                )),
            };
        }

        ResolvedField {
            tier: FieldResolutionTier::Empty,
            value: None,
            source: None,
            is_stale: false,
            provenance: None,
        }
    }

    fn arb_claim() -> impl Strategy<Value = FieldClaim> {
        // A small alphabet for source and fetched_at deliberately produces
        // real ties, which is exactly where a refactor of this kind breaks.
        // `value` must vary independently of (source, fetched_at): two claims
        // that fully tie on both still need to be distinguishable, or a wrong
        // tie-break pick and a right one produce identical output and the
        // property can never observe the difference.
        (
            prop::sample::select(vec!["tmdb", "tvdb", "imdb"]),
            "[a-z]{1,4}",
            0i64..5,
            prop::option::of(5i64..10),
            prop::option::of(prop::sample::select(vec!["en", "fr"])),
        )
            .prop_map(|(source, value, fetched, expires, locale)| {
                FieldClaim::try_new(
                    ns(source),
                    value,
                    locale.map(str::to_owned),
                    received(fetched),
                    expires.map(at),
                )
                .expect("valid generated claim")
            })
    }

    proptest! {
        #[test]
        fn single_pass_resolution_matches_the_reference_implementation(
            claims in prop::collection::vec(arb_claim(), 0..8),
            has_override in any::<bool>(),
            preferred_source in prop::option::of(prop::sample::select(vec!["tmdb", "tvdb", "imdb"])),
            preferred_locale in prop::option::of(prop::sample::select(vec!["en", "fr"])),
            now_secs in 0i64..10,
        ) {
            let override_ = has_override
                .then(|| FieldOverride::try_new("override", received(0)).expect("valid override"));
            let preferred_source = preferred_source.map(ns);
            let now = at(now_secs);

            let fast = resolve_field(
                override_.as_ref(),
                &claims,
                preferred_source.as_ref(),
                preferred_locale,
                now,
            );
            let reference = reference_resolve_field(
                override_.as_ref(),
                &claims,
                preferred_source.as_ref(),
                preferred_locale,
                now,
            );
            prop_assert_eq!(fast, reference);
        }
    }

    #[test]
    fn field_key_rejects_uppercase_and_empty_segments() {
        assert!(FieldKey::try_new("core.title").is_ok());
        assert!(FieldKey::try_new("Core.Title").is_err());
        assert!(FieldKey::try_new("core..title").is_err());
        assert!(FieldKey::try_new("").is_err());
    }

    #[test]
    fn claim_construction_rejects_control_characters_and_bad_expiry() {
        assert!(FieldClaim::try_new(ns("tmdb"), "Example\0Film", None, received(0), None).is_err());
        assert!(
            FieldClaim::try_new(ns("tmdb"), "Example", None, received(100), Some(at(50))).is_err(),
            "expiry before fetch must be rejected"
        );
    }

    #[test]
    fn user_override_wins_over_every_claim() {
        let override_ = FieldOverride::try_new("My Title", received(0)).expect("valid override");
        let claims = [claim("tmdb", "Provider Title", 100, None)];
        let resolved = resolve_field(Some(&override_), &claims, None, None, at(200));
        assert_eq!(resolved.tier(), FieldResolutionTier::UserOverride);
        assert_eq!(resolved.value(), Some("My Title"));
    }

    #[test]
    fn provider_refresh_does_not_overwrite_the_override() {
        // ID-064: the override must keep winning even after a NEWER claim
        // arrives. resolve_field takes no special path for "newer" -- an
        // override always wins regardless of claim recency.
        let override_ = FieldOverride::try_new("My Title", received(0)).expect("valid override");
        let claims = [claim("tmdb", "Refreshed Title", 999_999, None)];
        let resolved = resolve_field(Some(&override_), &claims, None, None, at(1_000_000));
        assert_eq!(resolved.value(), Some("My Title"));
    }

    #[test]
    fn preferred_source_and_locale_wins_over_fallback() {
        let claims = [
            claim("tvdb", "Fallback Title", 100, None),
            localized_claim("tmdb", "Preferred Title", "en", 100, None),
        ];
        let resolved = resolve_field(None, &claims, Some(&ns("tmdb")), Some("en"), at(200));
        assert_eq!(resolved.tier(), FieldResolutionTier::PreferredProviderClaim);
        assert_eq!(resolved.value(), Some("Preferred Title"));
        assert_eq!(resolved.source().map(NamespaceKey::as_str), Some("tmdb"));
    }

    #[test]
    fn expired_preferred_claim_falls_back_to_a_fresh_claim() {
        let claims = [
            localized_claim("tmdb", "Expired Preferred", "en", 0, Some(50)),
            claim("tvdb", "Fresh Fallback", 100, Some(1_000)),
        ];
        let resolved = resolve_field(None, &claims, Some(&ns("tmdb")), Some("en"), at(200));
        assert_eq!(resolved.tier(), FieldResolutionTier::FallbackProviderClaim);
        assert_eq!(resolved.value(), Some("Fresh Fallback"));
    }

    #[test]
    fn every_claim_expired_falls_back_to_last_known_good_and_is_marked_stale() {
        // ID-065: preferred claim expires; offline; last-known-good displays
        // as stale with its source.
        let claims = [
            claim("tmdb", "Older", 100, Some(150)),
            claim("tvdb", "Newer", 120, Some(150)),
        ];
        let resolved = resolve_field(None, &claims, None, None, at(200));
        assert_eq!(resolved.tier(), FieldResolutionTier::LastKnownGood);
        assert!(resolved.is_stale());
        assert_eq!(resolved.value(), Some("Newer"));
        assert_eq!(resolved.source().map(NamespaceKey::as_str), Some("tvdb"));
    }

    #[test]
    fn a_claim_is_not_fresh_at_the_exact_instant_it_expires() {
        // Boundary case: freshness is a STRICT upper bound. now == expires_at
        // must already be treated as expired, not as the last fresh instant.
        let expiring = claim("tmdb", "Right At Expiry", 0, Some(100));
        assert!(
            expiring.is_fresh(at(99)),
            "one second before expiry is fresh"
        );
        assert!(
            !expiring.is_fresh(at(100)),
            "the exact expiry instant must not be fresh"
        );
    }

    #[test]
    fn no_claims_and_no_override_resolves_to_empty() {
        let resolved = resolve_field(None, &[], None, None, at(0));
        assert_eq!(resolved.tier(), FieldResolutionTier::Empty);
        assert_eq!(resolved.value(), None);
        assert!(!resolved.is_stale());
    }

    #[test]
    fn a_failed_refresh_does_not_erase_the_prior_valid_claim() {
        // ID-066: absence is not deletion. A failed fetch simply adds no new
        // claim; the set handed to resolve_field is unchanged, and the prior
        // claim keeps winning.
        let claims = [claim("tmdb", "Still Here", 100, Some(1_000))];
        let resolved = resolve_field(None, &claims, None, None, at(200));
        assert_eq!(resolved.value(), Some("Still Here"));
        assert!(!resolved.is_stale());
    }

    #[test]
    fn tie_break_within_a_tier_is_deterministic_by_source_when_fetched_at_ties() {
        let claims = [
            claim("tvdb", "From TVDB", 100, None),
            claim("tmdb", "From TMDB", 100, None),
        ];
        let first = resolve_field(None, &claims, None, None, at(200));
        let second = resolve_field(None, &claims, None, None, at(200));
        assert_eq!(first, second, "resolution must be deterministic");
        // "tvdb" > "tmdb" lexically, so it wins the tie by source ordering.
        assert_eq!(first.source().map(NamespaceKey::as_str), Some("tvdb"));
    }

    #[test]
    fn persisted_claim_retains_id_target_status_and_legacy_provenance() {
        let claim_id = MetadataClaimId::new_v7();
        let record_id = RecordId::new_v7();
        let field_key = FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field");
        let provenance = FieldClaimProvenance::legacy(ns("tmdb.movie"), Some(locale("en")));
        let reconstructed = FieldClaim::try_from_persisted(
            claim_id,
            Some(record_id),
            Some(field_key.clone()),
            "Migrated title",
            provenance,
            received(100),
            Some(at(200)),
            FieldClaimStatus::Stale,
        )
        .expect("legacy state remains readable");

        assert_eq!(reconstructed.claim_id(), claim_id);
        assert_eq!(reconstructed.record_id(), Some(record_id));
        assert_eq!(reconstructed.field_key(), Some(&field_key));
        assert_eq!(reconstructed.initial_status(), FieldClaimStatus::Stale);
        assert!(!reconstructed.provenance().is_complete());
        assert!(FieldClaim::try_from_persisted(
            MetadataClaimId::new_v7(),
            Some(record_id),
            None,
            "broken target",
            FieldClaimProvenance::legacy(ns("tmdb.movie"), None),
            received(100),
            None,
            FieldClaimStatus::Fresh,
        )
        .is_err());
    }

    #[test]
    fn lifecycle_changes_resolution_without_mutating_claim_provenance() {
        let record_id = RecordId::new_v7();
        let field_key = FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field");
        let claim = provider_claim(
            record_id,
            &field_key,
            "tmdb",
            "tmdb.movie",
            "550",
            "Provider title",
            Some("en"),
            100,
            None,
            FieldClaimStatus::Fresh,
        );
        let original_provenance = claim.provenance().clone();
        let revoked = FieldClaimLifecycleEvent::try_new(
            claim.claim_id(),
            1,
            FieldClaimStatus::Fresh,
            FieldClaimStatus::Revoked,
            received(150),
            Some(digest(9)),
        )
        .expect("valid revocation");
        let policy = MetadataProjectionPolicy::default_for_profile(ProfileId::new_v7());
        let resolved = resolve_profile_field(
            None,
            std::slice::from_ref(&claim),
            &[revoked],
            &policy,
            at(200),
        )
        .expect("valid lifecycle");

        assert_eq!(resolved.tier(), FieldResolutionTier::Empty);
        assert_eq!(claim.initial_status(), FieldClaimStatus::Fresh);
        assert_eq!(claim.provenance(), &original_provenance);
        assert!(FieldClaimLifecycleEvent::try_new(
            claim.claim_id(),
            1,
            FieldClaimStatus::Fresh,
            FieldClaimStatus::Superseded,
            received(150),
            None,
        )
        .is_err());

        let skipped_sequence = FieldClaimLifecycleEvent::try_new(
            claim.claim_id(),
            2,
            FieldClaimStatus::Fresh,
            FieldClaimStatus::Stale,
            received(150),
            None,
        )
        .expect("event is individually valid");
        assert_eq!(
            resolve_profile_field(None, &[claim], &[skipped_sequence], &policy, at(200)),
            Err(FieldResolutionError::InvalidLifecycleSequence)
        );
    }

    #[test]
    fn policy_resolves_preferred_then_original_then_english_locale() {
        let profile_id = ProfileId::new_v7();
        let record_id = RecordId::new_v7();
        let field_key = FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field");
        let claims = [
            provider_claim(
                record_id,
                &field_key,
                "tvdb",
                "tvdb.series",
                "1",
                "newer English",
                Some("en"),
                400,
                None,
                FieldClaimStatus::Fresh,
            ),
            provider_claim(
                record_id,
                &field_key,
                "tmdb",
                "tmdb.tv",
                "2",
                "preferred French",
                Some("fr"),
                100,
                None,
                FieldClaimStatus::Fresh,
            ),
            provider_claim(
                record_id,
                &field_key,
                "tvdb",
                "tvdb.series",
                "3",
                "original Japanese",
                Some("ja"),
                300,
                None,
                FieldClaimStatus::Fresh,
            ),
        ];
        let preferred = projection_policy(
            profile_id,
            Some("tmdb"),
            Some("fr"),
            Some("ja"),
            true,
            LastKnownGoodPolicy::Allow,
        );
        let resolved = resolve_profile_field(None, &claims, &[], &preferred, at(500))
            .expect("valid resolution");
        assert_eq!(resolved.tier(), FieldResolutionTier::PreferredProviderClaim);
        assert_eq!(resolved.value(), Some("preferred French"));

        let original_fallback = projection_policy(
            profile_id,
            None,
            Some("de"),
            Some("ja"),
            true,
            LastKnownGoodPolicy::Allow,
        );
        let resolved = resolve_profile_field(None, &claims, &[], &original_fallback, at(500))
            .expect("valid resolution");
        assert_eq!(resolved.value(), Some("original Japanese"));

        let english_fallback = projection_policy(
            profile_id,
            None,
            Some("de"),
            None,
            true,
            LastKnownGoodPolicy::Allow,
        );
        let resolved = resolve_profile_field(None, &claims, &[], &english_fallback, at(500))
            .expect("valid resolution");
        assert_eq!(resolved.value(), Some("newer English"));
    }

    #[test]
    fn last_known_good_policy_never_revives_invalid_revoked_or_superseded_claims() {
        let profile_id = ProfileId::new_v7();
        let record_id = RecordId::new_v7();
        let field_key = FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field");
        let stale = provider_claim(
            record_id,
            &field_key,
            "tmdb",
            "tmdb.movie",
            "1",
            "safe stale value",
            Some("en"),
            100,
            Some(150),
            FieldClaimStatus::Fresh,
        );
        let invalid = provider_claim(
            record_id,
            &field_key,
            "tvdb",
            "tvdb.movie",
            "2",
            "invalid newer value",
            Some("en"),
            190,
            None,
            FieldClaimStatus::Invalid,
        );
        let revoked = provider_claim(
            record_id,
            &field_key,
            "imdb",
            "imdb.title",
            "3",
            "revoked value",
            Some("en"),
            195,
            None,
            FieldClaimStatus::Revoked,
        );
        let superseded = provider_claim(
            record_id,
            &field_key,
            "mdblist",
            "tmdb.movie",
            "4",
            "superseded value",
            Some("en"),
            199,
            None,
            FieldClaimStatus::Superseded,
        );
        let claims = [stale, invalid, revoked, superseded];
        let allow = projection_policy(
            profile_id,
            None,
            Some("en"),
            None,
            true,
            LastKnownGoodPolicy::Allow,
        );
        let resolved =
            resolve_profile_field(None, &claims, &[], &allow, at(200)).expect("valid resolution");
        assert_eq!(resolved.tier(), FieldResolutionTier::LastKnownGood);
        assert_eq!(resolved.value(), Some("safe stale value"));
        assert_eq!(
            resolved.provenance().map(ResolvedFieldProvenance::status),
            Some(FieldClaimStatus::Stale)
        );

        let deny = projection_policy(
            profile_id,
            None,
            Some("en"),
            None,
            true,
            LastKnownGoodPolicy::Deny,
        );
        let resolved =
            resolve_profile_field(None, &claims, &[], &deny, at(200)).expect("valid resolution");
        assert_eq!(resolved.tier(), FieldResolutionTier::Empty);
    }

    #[test]
    fn profile_override_is_owned_and_resolution_is_input_order_independent() {
        let profile_id = ProfileId::new_v7();
        let record_id = RecordId::new_v7();
        let field_key = FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field");
        let first_claim = provider_claim(
            record_id,
            &field_key,
            "tmdb",
            "tmdb.movie",
            "550",
            "TMDB title",
            Some("en"),
            100,
            None,
            FieldClaimStatus::Fresh,
        );
        let second_claim = provider_claim(
            record_id,
            &field_key,
            "tvdb",
            "tvdb.movie",
            "550",
            "TVDB title",
            Some("en"),
            100,
            None,
            FieldClaimStatus::Fresh,
        );
        let policy = projection_policy(
            profile_id,
            None,
            Some("en"),
            None,
            true,
            LastKnownGoodPolicy::Allow,
        );
        let forward = resolve_profile_field(
            None,
            &[first_claim.clone(), second_claim.clone()],
            &[],
            &policy,
            at(200),
        )
        .expect("valid resolution");
        let reverse =
            resolve_profile_field(None, &[second_claim, first_claim], &[], &policy, at(200))
                .expect("valid resolution");
        assert_eq!(forward, reverse);

        let override_ = ProfileFieldOverride::try_new(
            profile_id,
            record_id,
            field_key.clone(),
            "My title",
            received(200),
        )
        .expect("valid override");
        let overridden = resolve_profile_field(Some(&override_), &[], &[], &policy, at(300))
            .expect("owned override resolves");
        assert_eq!(overridden.tier(), FieldResolutionTier::UserOverride);
        assert_eq!(overridden.value(), Some("My title"));

        let other_policy = MetadataProjectionPolicy::default_for_profile(ProfileId::new_v7());
        assert_eq!(
            resolve_profile_field(Some(&override_), &[], &[], &other_policy, at(300)),
            Err(FieldResolutionError::OverrideProfileMismatch)
        );
    }

    #[test]
    fn metadata_value_validation_is_bounded_and_canonical() {
        assert_eq!(locale("EN-ie").as_str(), "en-ie");
        assert_eq!(
            MetadataRegion::try_new("ie")
                .expect("valid region")
                .as_str(),
            "IE"
        );
        assert!(MetadataLocale::try_new("e").is_err());
        assert!(MetadataLocale::try_new("en--IE").is_err());
        assert!(MetadataRegion::try_new("I!").is_err());
        assert!(MetadataProviderId::try_new("TMDB").is_err());
        assert!(FieldClaimProvenance::try_new(
            provider("tmdb"),
            ns("tmdb.movie"),
            " ",
            None,
            None,
            None,
            digest(1),
        )
        .is_err());
    }

    #[test]
    fn rating_claim_uses_fixed_point_scale_and_complete_provenance() {
        let record_id = RecordId::new_v7();
        let scale = RatingScale::try_new(0, 10_000).expect("valid ten point scale");
        let rating = RatingClaim::try_new(
            MetadataClaimId::new_v7(),
            record_id,
            7_800,
            scale,
            provenance("mdblist", "imdb.title", "tt0137523", None),
            received(100),
            Some(at(200)),
            FieldClaimStatus::Fresh,
        )
        .expect("valid rating");
        assert_eq!(rating.record_id(), record_id);
        assert_eq!(rating.value_millis(), 7_800);
        assert_eq!(rating.scale().maximum_millis(), 10_000);
        assert_eq!(rating.status_at(at(200)), FieldClaimStatus::Stale);
        assert!(RatingClaim::try_new(
            MetadataClaimId::new_v7(),
            record_id,
            10_001,
            scale,
            provenance("mdblist", "imdb.title", "tt0137523", None),
            received(100),
            None,
            FieldClaimStatus::Fresh,
        )
        .is_err());
    }

    #[test]
    fn cache_partitions_freshness_offline_reads_and_safe_invalidation() {
        let key = cache_key(
            RecordId::new_v7(),
            MetadataCachePurpose::DisplayProjection,
            MetadataDataClassification::Internal,
        );
        let entry = MetadataCacheEntry::try_new(
            key,
            vec![MetadataClaimId::new_v7()],
            received(0),
            at(10),
            at(20),
            at(30),
        )
        .expect("valid cache entry");
        assert_eq!(
            entry.read_state(
                at(5),
                MetadataCachePurpose::DisplayProjection,
                MetadataDataClassification::Internal,
                false,
            ),
            MetadataCacheReadState::Fresh
        );
        assert_eq!(
            entry.read_state(
                at(15),
                MetadataCachePurpose::DisplayProjection,
                MetadataDataClassification::Internal,
                false,
            ),
            MetadataCacheReadState::StaleWhileRefreshing
        );
        assert_eq!(
            entry.read_state(
                at(25),
                MetadataCachePurpose::DisplayProjection,
                MetadataDataClassification::Internal,
                true,
            ),
            MetadataCacheReadState::StaleOnError
        );
        assert_eq!(
            entry.read_state(
                at(25),
                MetadataCachePurpose::DisplayProjection,
                MetadataDataClassification::Internal,
                false,
            ),
            MetadataCacheReadState::Expired
        );
        assert_eq!(
            entry.read_state(
                at(5),
                MetadataCachePurpose::OfflineRead,
                MetadataDataClassification::Restricted,
                true,
            ),
            MetadataCacheReadState::PartitionDenied
        );
        assert_eq!(
            entry.read_state(
                at(5),
                MetadataCachePurpose::DisplayProjection,
                MetadataDataClassification::Public,
                false,
            ),
            MetadataCacheReadState::PartitionDenied
        );

        let invalidated = entry
            .invalidated(
                MetadataCacheInvalidationReason::CredentialRotated,
                received(12),
            )
            .expect("safe immutable invalidation");
        assert!(entry.invalidation().is_none());
        assert_eq!(
            invalidated.read_state(
                at(12),
                MetadataCachePurpose::DisplayProjection,
                MetadataDataClassification::Internal,
                false,
            ),
            MetadataCacheReadState::Invalidated
        );
    }

    #[test]
    fn cache_key_enrichment_projection_and_attribution_validate_boundaries() {
        let record_id = RecordId::new_v7();
        assert!(MetadataCacheKey::try_new(
            provider("tmdb"),
            None,
            record_id,
            "movie/details?api_key=secret",
            Grain::Film,
            ns("tmdb.movie"),
            "550",
            None,
            None,
            MetadataFieldGroup::BasicInfo,
            digest(1),
            digest(2),
            1,
            MetadataCachePurpose::MetadataEnrichment,
            "2026-08",
            MetadataDataClassification::Public,
        )
        .is_err());

        let profile_id = ProfileId::new_v7();
        let projection_policy = MetadataProjectionPolicy::default_for_profile(profile_id);
        let enrichment = EnrichmentPolicy::new(
            projection_policy,
            None,
            vec![
                MetadataFieldGroup::Details,
                MetadataFieldGroup::Artwork,
                MetadataFieldGroup::Details,
            ],
        );
        assert_eq!(
            enrichment.enabled_field_groups(),
            &[MetadataFieldGroup::Artwork, MetadataFieldGroup::Details]
        );

        let projection = MetadataProjection::try_new(
            profile_id,
            record_id,
            FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field"),
            resolve_field(None, &[], None, None, at(0)),
            received(1),
        )
        .expect("empty projection has an explicit target");
        assert_eq!(projection.record_id(), record_id);

        let field_key = FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field");
        let targeted_claim = provider_claim(
            record_id,
            &field_key,
            "tmdb",
            "tmdb.movie",
            "550",
            "Targeted title",
            Some("en"),
            1,
            None,
            FieldClaimStatus::Fresh,
        );
        let targeted = resolve_profile_field(
            None,
            &[targeted_claim],
            &[],
            enrichment.projection_policy(),
            at(2),
        )
        .expect("valid targeted projection");
        assert!(MetadataProjection::try_new(
            profile_id,
            RecordId::new_v7(),
            field_key,
            targeted,
            received(2),
        )
        .is_err());

        let attribution = MetadataAttribution::try_new(
            provider("tmdb"),
            "Metadata provided by TMDB",
            "https://developer.themoviedb.org/docs",
        )
        .expect("safe attribution");
        assert_eq!(attribution.provider_id().as_str(), "tmdb");
        for unsafe_url in [
            "http://example.com/docs",
            "https:///docs",
            "https://user@example.com/docs",
            "https://example.com/docs#fragment",
            "https://example.com/white space",
        ] {
            assert!(MetadataAttribution::try_new(
                provider("tmdb"),
                "Provider attribution",
                unsafe_url,
            )
            .is_err());
        }
    }
}
