import type { MediaKind, MediaRecord, RecordSummary } from "./types.js";

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
    case "work":
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
 * No watch-state query is wired yet, so the presentation reports that state
 * as unknown instead of inferring it from unrelated activity.
 */
export function projectRecordSummary(summary: RecordSummary): MediaRecord {
  const titleValue =
    summary.title.tier !== "empty" && summary.title.value
      ? summary.title.value
      : `Untitled record (${summary.record_id})`;
  return {
    id: summary.record_id,
    title: titleValue,
    detailLevel: "summary",
    mediaKind: mediaKindForGrain(summary.grain),
    status: "unknown",
    // Record summaries carry an untrusted poster claim, not a governed local
    // media URL. Keep the typographic fallback until a host media path exists.
    posterUrl: undefined,
    externalIds: [],
    displaySource: "Fasti local record",
    tags: [],
    genres: [],
    studios: [],
    lastActivityAt: summary.latest_activity?.occurred_at?.original,
  };
}
