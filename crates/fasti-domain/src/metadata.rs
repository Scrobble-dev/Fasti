//! Provider-owned metadata claims and their resolution to one displayed value.
//!
//! A Fasti Record's identity never depends on a provider. Its displayed
//! metadata does, and providers disagree, go stale, or go silent. This module
//! keeps every claim a provider ever supplied and resolves them to one value
//! deterministically, so the same inputs always produce the same answer and a
//! caller can explain why a value is showing.
//!
//! What this module deliberately does not yet do: workspace-scoped overrides
//! (only profile/user overrides exist here), and falling back to the original
//! observed value once every claim is gone. Both are real tiers in the wider
//! plan; neither is proven necessary by a UAT case yet, so they are not built
//! speculatively. Add them when a case requires them.

use crate::NamespaceKey;
use chrono::{DateTime, Utc};
use serde::Serialize;

pub const MAX_FIELD_KEY_BYTES: usize = 64;
pub const MAX_FIELD_VALUE_BYTES: usize = 4096;
pub const MAX_LOCALE_BYTES: usize = 16;

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
pub enum FieldClaimError {
    #[error("field claim value must be non-empty, bounded, and contain no control characters")]
    InvalidValue,
    #[error("locale must be 2 to 16 ASCII letters, digits, or hyphens")]
    InvalidLocale,
    #[error("expires_at cannot be at or before fetched_at")]
    ExpiryNotAfterFetch,
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
    source: NamespaceKey,
    value: String,
    locale: Option<String>,
    fetched_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
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
                let valid = (2..=MAX_LOCALE_BYTES).contains(&locale.len())
                    && locale
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
                valid
                    .then_some(locale)
                    .ok_or(FieldClaimError::InvalidLocale)
            })
            .transpose()?;
        let fetched_at = fetched_at.value();
        if let Some(expires_at) = expires_at {
            if expires_at <= fetched_at {
                return Err(FieldClaimError::ExpiryNotAfterFetch);
            }
        }
        Ok(Self {
            source,
            value,
            locale,
            fetched_at,
            expires_at,
        })
    }

    pub fn source(&self) -> &NamespaceKey {
        &self.source
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    pub const fn fetched_at(&self) -> DateTime<Utc> {
        self.fetched_at
    }

    /// A claim with no declared expiry never goes stale on its own; absence
    /// of a cache directive is not absence of validity.
    pub fn is_fresh(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_none_or(|expires_at| now < expires_at)
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
        if value.is_empty()
            || value.len() > MAX_FIELD_VALUE_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(FieldClaimError::InvalidValue);
        }
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

    if let (Some(preferred_source), Some(preferred_locale)) = (preferred_source, preferred_locale) {
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
            };
        }
    }

    if let Some(claim) = most_recent(&fresh) {
        return ResolvedField {
            tier: FieldResolutionTier::FallbackProviderClaim,
            value: Some(claim.value().to_owned()),
            source: Some(claim.source().clone()),
            is_stale: false,
        };
    }

    let all: Vec<&FieldClaim> = claims.iter().collect();
    if let Some(claim) = most_recent(&all) {
        return ResolvedField {
            tier: FieldResolutionTier::LastKnownGood,
            value: Some(claim.value().to_owned()),
            source: Some(claim.source().clone()),
            is_stale: true,
        };
    }

    ResolvedField {
        tier: FieldResolutionTier::Empty,
        value: None,
        source: None,
        is_stale: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReceivedAt;
    use chrono::TimeZone;

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
        let claims = [claim("tvdb", "Fallback Title", 100, None), {
            let mut c = claim("tmdb", "Preferred Title", 100, None);
            c.locale = Some("en".to_owned());
            c
        }];
        let resolved = resolve_field(None, &claims, Some(&ns("tmdb")), Some("en"), at(200));
        assert_eq!(resolved.tier(), FieldResolutionTier::PreferredProviderClaim);
        assert_eq!(resolved.value(), Some("Preferred Title"));
        assert_eq!(resolved.source().map(NamespaceKey::as_str), Some("tmdb"));
    }

    #[test]
    fn expired_preferred_claim_falls_back_to_a_fresh_claim() {
        let claims = [
            {
                let mut c = claim("tmdb", "Expired Preferred", 0, Some(50));
                c.locale = Some("en".to_owned());
                c
            },
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
}
