use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
