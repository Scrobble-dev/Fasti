import type {
  MediaKind,
  MediaRecord,
  RecordSummary,
  TrackingDisposition,
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
    case "edition":
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
 * An explicit profile-owned tracking disposition wins. Without one, `status`
 * remains a presentation fallback: "watching" if the record has any recorded
 * activity, else "plan_to_watch". Completion still needs Chronicle history;
 * watchlist intent still needs its own list-membership capability.
 *
 * ponytail: status heuristic ceiling is "activity present or not"; upgrade
 * once occurrence-derived watch status exists.
 */
export function projectRecordSummary(
  summary: RecordSummary,
  disposition?: TrackingDisposition | null,
): MediaRecord {
  const status: WatchStatus =
    disposition ?? (summary.latest_activity ? "watching" : "plan_to_watch");
  const titleValue =
    summary.title.tier !== "empty" && summary.title.value
      ? summary.title.value
      : `Untitled record (${summary.record_id})`;
  const posterValue =
    summary.poster.tier !== "empty" && summary.poster.value
      ? summary.poster.value
      : undefined;
  const originalTitle =
    summary.original_title?.tier !== "empty"
      ? (summary.original_title?.value ?? undefined)
      : undefined;
  const overview =
    summary.overview?.tier !== "empty"
      ? (summary.overview?.value ?? undefined)
      : undefined;
  const releaseYearValue =
    summary.release_year?.tier !== "empty"
      ? summary.release_year?.value
      : undefined;
  const releaseYear = releaseYearValue?.match(/^\d{4}$/)
    ? Number(releaseYearValue)
    : undefined;

  return {
    id: summary.record_id,
    title: titleValue,
    originalTitle,
    mediaKind: mediaKindForGrain(summary.grain),
    releaseYear,
    overview,
    status,
    trackingDisposition: disposition ?? null,
    posterUrl: posterValue,
    externalIds: (summary.identifiers ?? []).map((identifier) => ({
      namespace: identifier.namespace,
      value: identifier.value,
      status: "matched",
      source: identifier.namespace,
    })),
    displaySource: "Fasti local record",
    tags: [],
    genres: [],
    studios: [],
    lastActivityAt: summary.latest_activity?.occurred_at?.original,
  };
}
