import type {
  MediaKind,
  MediaRecord,
  RecordSummary,
  WatchStatus,
} from "./types.js";

/**
 * Coarse `Grain` (identity granularity) -> `MediaKind` (display category)
 * mapping. Grains don't carry genre/format info, so this is a best-effort
 * heuristic, not a real classification.
 *
 * ponytail: coarse heuristic, ceiling is genre-blind (e.g. "series" covers
 * both TV shows and anime). Upgrade when a real media-kind classification
 * lands server-side.
 */
function mediaKindForGrain(grain: string): MediaKind {
  switch (grain) {
    case "film":
      return "movie";
    case "series":
    case "season":
    case "episode":
      return "show";
    case "recording":
    case "album_release":
    case "track":
      return "music";
    case "chapter":
      return "book";
    case "podcast_feed":
    case "podcast_episode":
      return "podcast";
    case "game_release":
      return "game";
    default:
      return "custom";
  }
}

/**
 * Projects the desktop host's real `RecordSummary` (from `list_records`)
 * onto the presentational `MediaRecord` shape the view components expect.
 *
 * No watch-status pipeline is wired yet (that needs Chronicle/occurrence
 * history, which this pass explicitly does not restore), so `status` is a
 * placeholder: "watching" if the record has any recorded activity, else
 * "plan_to_watch". This asserts nothing false, it just can't be precise yet.
 *
 * ponytail: status heuristic ceiling is "activity present or not"; upgrade
 * once occurrence-derived watch status exists.
 */
export function projectRecordSummary(summary: RecordSummary): MediaRecord {
  const status: WatchStatus = summary.latest_activity
    ? "watching"
    : "plan_to_watch";
  const titleValue =
    summary.title.tier !== "empty" && summary.title.value
      ? summary.title.value
      : `Untitled record (${summary.record_id})`;
  const posterValue =
    summary.poster.tier !== "empty" && summary.poster.value
      ? summary.poster.value
      : undefined;

  return {
    id: summary.record_id,
    title: titleValue,
    mediaKind: mediaKindForGrain(summary.grain),
    status,
    posterUrl: posterValue,
    externalIds: [],
    displaySource: "Fasti local record",
    tags: [],
    genres: [],
    studios: [],
    lastActivityAt: summary.latest_activity?.occurred_at?.original,
  };
}
