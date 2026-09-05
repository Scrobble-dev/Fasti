//! Trusted response observations shared by Search and metadata. HTTP syntax
//! belongs to the provider adapter; these limits never authorize persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResponseReuse {
    NoStore,
    ValidateEveryReuse,
    ValidateWhenStale,
    Reusable,
}

#[cfg(test)]
include!("response_cache_tests.rs");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResponseCachePolicy {
    reuse: ProviderResponseReuse,
    received_at: DateTime<Utc>,
    corrected_initial_age: Duration,
    source_freshness: Option<Duration>,
    source_stale_if_error: Option<Duration>,
}

impl ProviderResponseCachePolicy {
    /// Private persisted observation bound, not a provider body or public DTO.
    pub const MAX_JSON_BYTES: usize = 1024;

    pub fn to_canonical_json(&self) -> String {
        serde_json::to_string(self).expect("bounded response policy has JSON-safe fields")
    }

    /// Representation validation does not grant storage or reuse permission.
    pub fn from_canonical_json(value: &str) -> Option<Self> {
        if value.len() > Self::MAX_JSON_BYTES {
            return None;
        }
        let policy: Self = serde_json::from_str(value).ok()?;
        (policy.to_canonical_json() == value).then_some(policy)
    }

    pub const fn new(
        reuse: ProviderResponseReuse,
        received_at: DateTime<Utc>,
        corrected_initial_age: Duration,
        source_freshness: Option<Duration>,
        source_stale_if_error: Option<Duration>,
    ) -> Self {
        Self {
            reuse,
            received_at,
            corrected_initial_age,
            source_freshness,
            source_stale_if_error,
        }
    }

    pub const fn reuse(&self) -> ProviderResponseReuse {
        self.reuse
    }

    pub const fn received_at(&self) -> DateTime<Utc> {
        self.received_at
    }

    /// Absolute deadlines. Purpose caps start at response observation, not at
    /// conversion/commit. Explicit source freshness also consumes upstream age.
    /// The stale cap is absolute, never an extra grace added to the fresh cap.
    pub fn deadlines(
        &self,
        fresh_cap: Duration,
        stale_cap: Duration,
    ) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        let fresh_cap = fresh_cap.min(stale_cap);
        if self.reuse == ProviderResponseReuse::NoStore {
            return None;
        }
        if self.reuse == ProviderResponseReuse::ValidateEveryReuse {
            return Some((self.received_at, self.received_at));
        }
        let fresh = self
            .source_freshness
            .map(|duration| duration.saturating_sub(self.corrected_initial_age))
            .unwrap_or(fresh_cap)
            .min(fresh_cap);
        let stale = if self.reuse == ProviderResponseReuse::ValidateWhenStale {
            fresh
        } else {
            self.source_stale_if_error
                .map(|grace| match self.source_freshness {
                    Some(source) => source
                        .saturating_add(grace)
                        .saturating_sub(self.corrected_initial_age),
                    None => fresh.saturating_add(grace),
                })
                .unwrap_or(stale_cap)
                .min(stale_cap)
                .max(fresh)
        };
        let deadline = |duration| {
            chrono::Duration::from_std(duration)
                .ok()
                .and_then(|duration| self.received_at.checked_add_signed(duration))
                .unwrap_or(self.received_at)
        };
        Some((deadline(fresh), deadline(stale)))
    }
}
