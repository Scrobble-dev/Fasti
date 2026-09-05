// @ts-check

/** @param {import("./types.js").MediaRecord} record */
export function recordProgressPercent(record) {
  if (record.status === "completed") return 100;
  const episodeProgress =
    typeof record.progressEpisodes === "number" &&
    Number.isFinite(record.progressEpisodes) &&
    typeof record.totalEpisodes === "number" &&
    Number.isFinite(record.totalEpisodes) &&
    record.totalEpisodes > 0;
  const current = episodeProgress
    ? record.progressEpisodes
    : record.progressSeconds;
  const total = episodeProgress
    ? record.totalEpisodes
    : record.totalDurationSeconds;
  if (
    current === undefined ||
    total === undefined ||
    !Number.isFinite(current) ||
    !Number.isFinite(total) ||
    total <= 0
  )
    return 0;
  return Math.min(100, Math.max(0, Math.round((current / total) * 100)));
}
