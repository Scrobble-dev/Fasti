use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimedPrecision {
    Date,
    Second,
    Millisecond,
    Microsecond,
    Nanosecond,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimedTrust {
    SourceClaim,
    DeviceObserved,
    UserEntered,
    Inferred,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClaimedTimeError {
    #[error("claimed time must be an ISO date or RFC 3339 timestamp with an explicit offset")]
    InvalidClaim,
    #[error("declared precision contradicts the original claimed value")]
    PrecisionMismatch,
    #[error("observed_at must identify an instant, not only a date")]
    ObservedAtRequiresInstant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedClaimedTime {
    Date(NaiveDate),
    Instant(DateTime<FixedOffset>),
}

/// A client-owned time claim. The lexical source is retained so normalization
/// cannot erase offset or precision. Deserialization reparses and verifies the
/// original; callers cannot inject a contradictory parsed value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedTime {
    original: String,
    parsed: ParsedClaimedTime,
    precision: ClaimedPrecision,
    trust: ClaimedTrust,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimedTimeWire {
    original: String,
    precision: ClaimedPrecision,
    trust: ClaimedTrust,
}

impl ClaimedTime {
    pub fn parse(
        original: impl Into<String>,
        trust: ClaimedTrust,
    ) -> Result<Self, ClaimedTimeError> {
        let original = original.into();
        if let Ok(date) = NaiveDate::parse_from_str(&original, "%Y-%m-%d") {
            return Ok(Self {
                original,
                parsed: ParsedClaimedTime::Date(date),
                precision: ClaimedPrecision::Date,
                trust,
            });
        }

        let instant =
            DateTime::parse_from_rfc3339(&original).map_err(|_| ClaimedTimeError::InvalidClaim)?;
        let precision = infer_fractional_precision(&original)?;
        Ok(Self {
            original,
            parsed: ParsedClaimedTime::Instant(instant),
            precision,
            trust,
        })
    }

    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn instant(&self) -> Option<DateTime<FixedOffset>> {
        match self.parsed {
            ParsedClaimedTime::Date(_) => None,
            ParsedClaimedTime::Instant(value) => Some(value),
        }
    }

    pub fn date(&self) -> NaiveDate {
        match self.parsed {
            ParsedClaimedTime::Date(value) => value,
            ParsedClaimedTime::Instant(value) => value.date_naive(),
        }
    }

    pub fn precision(&self) -> ClaimedPrecision {
        self.precision
    }

    pub fn trust(&self) -> ClaimedTrust {
        self.trust
    }
}

impl Serialize for ClaimedTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ClaimedTimeWire {
            original: self.original.clone(),
            precision: self.precision,
            trust: self.trust,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ClaimedTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ClaimedTimeWire::deserialize(deserializer)?;
        let parsed = Self::parse(wire.original, wire.trust).map_err(serde::de::Error::custom)?;
        if parsed.precision != wire.precision {
            return Err(serde::de::Error::custom(
                ClaimedTimeError::PrecisionMismatch,
            ));
        }
        Ok(parsed)
    }
}

fn infer_fractional_precision(value: &str) -> Result<ClaimedPrecision, ClaimedTimeError> {
    let time_start = value.find('T').ok_or(ClaimedTimeError::InvalidClaim)?;
    let time_and_offset = &value[time_start + 1..];
    let Some(dot) = time_and_offset.find('.') else {
        return Ok(ClaimedPrecision::Second);
    };
    let digits = time_and_offset[dot + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .count();
    match digits {
        1..=3 => Ok(ClaimedPrecision::Millisecond),
        4..=6 => Ok(ClaimedPrecision::Microsecond),
        7..=9 => Ok(ClaimedPrecision::Nanosecond),
        _ => Err(ClaimedTimeError::InvalidClaim),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OccurredAt(ClaimedTime);

impl OccurredAt {
    pub fn parse(value: impl Into<String>, trust: ClaimedTrust) -> Result<Self, ClaimedTimeError> {
        ClaimedTime::parse(value, trust).map(Self)
    }

    pub fn claim(&self) -> &ClaimedTime {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObservedAt(ClaimedTime);

impl ObservedAt {
    pub fn parse(value: impl Into<String>, trust: ClaimedTrust) -> Result<Self, ClaimedTimeError> {
        let value = ClaimedTime::parse(value, trust)?;
        if value.instant().is_none() {
            return Err(ClaimedTimeError::ObservedAtRequiresInstant);
        }
        Ok(Self(value))
    }

    pub fn claim(&self) -> &ClaimedTime {
        &self.0
    }
}

/// Server-owned ingress time. The application clock supplies the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReceivedAt(DateTime<Utc>);

impl ReceivedAt {
    pub fn from_application_clock(value: DateTime<Utc>) -> Self {
        Self(value)
    }

    pub fn value(self) -> DateTime<Utc> {
        self.0
    }
}

/// Server-owned durable commit time. Persistence supplies this only after the
/// durability boundary has been crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommittedAt(DateTime<Utc>);

impl CommittedAt {
    pub fn from_durability_boundary(value: DateTime<Utc>) -> Self {
        Self(value)
    }

    pub fn value(self) -> DateTime<Utc> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claimed_time_preserves_original_offset_and_infers_precision() {
        let value = ClaimedTime::parse("2026-08-21T23:14:15.120+05:30", ClaimedTrust::SourceClaim)
            .expect("valid claimed time");

        assert_eq!(value.original(), "2026-08-21T23:14:15.120+05:30");
        assert_eq!(
            value.instant().expect("instant").offset().local_minus_utc(),
            19_800
        );
        assert_eq!(value.precision(), ClaimedPrecision::Millisecond);
    }

    #[test]
    fn date_only_is_valid_for_occurrence_but_not_observation() {
        assert!(OccurredAt::parse("2026-08-21", ClaimedTrust::SourceClaim).is_ok());
        assert_eq!(
            ObservedAt::parse("2026-08-21", ClaimedTrust::DeviceObserved),
            Err(ClaimedTimeError::ObservedAtRequiresInstant)
        );
    }

    #[test]
    fn datetime_without_explicit_offset_is_rejected() {
        assert_eq!(
            ObservedAt::parse("2026-08-21T23:14:15", ClaimedTrust::DeviceObserved),
            Err(ClaimedTimeError::InvalidClaim)
        );
    }

    #[test]
    fn deserialization_rejects_contradictory_precision() {
        let input = r#"{"original":"2026-08-21T23:14:15.120+05:30","precision":"second","trust":"source_claim"}"#;
        assert!(serde_json::from_str::<ClaimedTime>(input).is_err());
    }
}
