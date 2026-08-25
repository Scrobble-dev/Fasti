export type MediaKind =
  "movie" | "show" | "anime" | "book" | "music" | "podcast" | "game" | "custom";

export type WatchStatus =
  "watching" | "completed" | "plan_to_watch" | "on_hold" | "dropped";

export interface ExternalId {
  readonly namespace: string;
  readonly value: string;
  readonly status: "matched" | "needs_review" | "local_only" | "retired";
  readonly source: string;
}

export interface EpisodeItem {
  readonly id: string;
  readonly number: number;
  readonly seasonNumber: number;
  readonly title: string;
  readonly airDate?: string;
  readonly durationSeconds?: number;
  readonly watched: boolean;
  readonly watchedAt?: string;
}

export interface SeasonItem {
  readonly seasonNumber: number;
  readonly title: string;
  readonly episodes: EpisodeItem[];
}

export interface MediaRecord {
  readonly id: string; // rec_01K...
  readonly title: string;
  readonly originalTitle?: string;
  readonly mediaKind: MediaKind;
  readonly customTypeName?: string;
  readonly releaseYear?: number;
  readonly overview?: string;
  readonly posterUrl?: string;
  readonly backdropUrl?: string;
  readonly status: WatchStatus;
  readonly userRating?: number; // 1-10
  readonly progressSeconds?: number;
  readonly totalDurationSeconds?: number;
  readonly progressEpisodes?: number;
  readonly totalEpisodes?: number;
  readonly externalIds: ExternalId[];
  readonly displaySource: string;
  readonly userNotes?: string;
  readonly tags: string[];
  readonly lastActivityAt?: string;
  readonly seasons?: SeasonItem[];
}

export interface ChronicleOccurrence {
  readonly id: string; // occ_01K...
  readonly recordId: string;
  readonly title: string;
  readonly mediaKind: MediaKind;
  readonly episodeTitle?: string;
  readonly seasonNumber?: number;
  readonly episodeNumber?: number;
  readonly posterUrl?: string;
  readonly timestamp: string;
  readonly progressPercentage: number;
  readonly durationMinutes: number;
  readonly deviceName: string;
  readonly clientName: string;
  readonly isRewatch: boolean;
  readonly userRating?: number;
}

export interface ReconciliationCase {
  readonly id: string;
  readonly recordId: string;
  readonly title: string;
  readonly mediaKind: MediaKind;
  readonly suppliedIds: ExternalId[];
  readonly candidateId: string;
  readonly candidateTitle: string;
  readonly candidateNamespace: string;
  readonly candidateExternalId: string;
  readonly candidatePosterUrl?: string;
  readonly matchingReasons: string[];
  readonly conflictingFactors: string[];
  readonly status: "open" | "resolved" | "deferred";
}

export interface CustomFieldDefinition {
  readonly key: string; // e.g. games.gog_product_id
  readonly label: string;
  readonly targetType: MediaKind | "all";
  readonly valueType:
    "string" | "number" | "boolean" | "date" | "url" | "identifier";
  readonly registeredNamespace?: string;
  readonly isFilterable: boolean;
}

export interface ScopedApiToken {
  readonly id: string;
  readonly name: string;
  readonly tokenPrefix: string;
  readonly scopes: string[];
  readonly createdAt: string;
  readonly lastUsedAt?: string;
}

export type ActiveNavSection =
  | "chronicle"
  | "library"
  | "up_next"
  | "calendar"
  | "reconciliation"
  | "connections"
  | "settings"
  | "detail";
