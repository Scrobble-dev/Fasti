use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    Work,
    Series,
    Release,
    Edition,
    Season,
    Segment,
    Episode,
    Film,
    Recording,
    AlbumRelease,
    Track,
    Chapter,
    PodcastFeed,
    PodcastEpisode,
    GameRelease,
    Custom,
}

/// A profile's current tracking disposition for one media Record.
///
/// This is not record identity, watchlist intent, completion history, or
/// measured progress. Absence means the profile has not chosen a disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackingDisposition {
    Watching,
    OnHold,
    Dropped,
}

impl TrackingDisposition {
    pub const ALL: &'static [Self] = &[Self::Watching, Self::OnHold, Self::Dropped];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Watching => "watching",
            Self::OnHold => "on_hold",
            Self::Dropped => "dropped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackingDispositionParseError;

impl fmt::Display for TrackingDispositionParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("tracking disposition is not registered")
    }
}

impl std::error::Error for TrackingDispositionParseError {}

impl FromStr for TrackingDisposition {
    type Err = TrackingDispositionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|disposition| disposition.as_str() == value)
            .ok_or(TrackingDispositionParseError)
    }
}

impl Grain {
    pub const ALL: &'static [Self] = &[
        Self::Work,
        Self::Series,
        Self::Release,
        Self::Edition,
        Self::Season,
        Self::Segment,
        Self::Episode,
        Self::Film,
        Self::Recording,
        Self::AlbumRelease,
        Self::Track,
        Self::Chapter,
        Self::PodcastFeed,
        Self::PodcastEpisode,
        Self::GameRelease,
        Self::Custom,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Series => "series",
            Self::Release => "release",
            Self::Edition => "edition",
            Self::Season => "season",
            Self::Segment => "segment",
            Self::Episode => "episode",
            Self::Film => "film",
            Self::Recording => "recording",
            Self::AlbumRelease => "album_release",
            Self::Track => "track",
            Self::Chapter => "chapter",
            Self::PodcastFeed => "podcast_feed",
            Self::PodcastEpisode => "podcast_episode",
            Self::GameRelease => "game_release",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrainParseError;

impl fmt::Display for GrainParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("grain is not registered")
    }
}

impl std::error::Error for GrainParseError {}

impl FromStr for Grain {
    type Err = GrainParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|grain| grain.as_str() == value)
            .ok_or(GrainParseError)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssertionRelation {
    Exact,
    SubsetOf,
    SupersetOf,
    Overlaps,
    AlternateCutOf,
    Related,
    NotSameAs,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_grain_has_one_stable_storage_spelling() {
        for grain in Grain::ALL {
            assert_eq!(grain.as_str().parse::<Grain>(), Ok(*grain));
        }
        assert!("movie".parse::<Grain>().is_err());
    }

    #[test]
    fn every_tracking_disposition_has_one_stable_storage_spelling() {
        for disposition in TrackingDisposition::ALL {
            assert_eq!(
                disposition.as_str().parse::<TrackingDisposition>(),
                Ok(*disposition)
            );
        }
        assert!("completed".parse::<TrackingDisposition>().is_err());
        assert!("plan_to_watch".parse::<TrackingDisposition>().is_err());
    }
}
