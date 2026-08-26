export type MediaKind =
  | "movie"
  | "show"
  | "anime"
  | "manga"
  | "book"
  | "comic"
  | "game"
  | "music"
  | "podcast"
  | "custom";

export type WatchStatus =
  "watching" | "completed" | "plan_to_watch" | "on_hold" | "dropped";

export interface ExternalId {
  readonly namespace: string;
  readonly value: string;
  readonly status: "matched" | "needs_review" | "local_only" | "retired";
  readonly source: string;
  readonly url?: string;
}

export interface CastMember {
  readonly id: string;
  readonly name: string;
  readonly characterName: string;
  readonly profileUrl?: string;
}

export interface CrewMember {
  readonly id: string;
  readonly name: string;
  readonly role: string; // Director, Writer, Composer, Producer
  readonly profileUrl?: string;
}

export interface EpisodeItem {
  readonly id: string;
  readonly number: number;
  readonly seasonNumber: number;
  readonly title: string;
  readonly overview?: string;
  readonly airDate?: string;
  readonly durationSeconds?: number;
  readonly watched: boolean;
  readonly watchedAt?: string;
  readonly userRating?: number;
}

export interface SeasonItem {
  readonly seasonNumber: number;
  readonly title: string;
  readonly posterUrl?: string;
  readonly episodeCount: number;
  readonly episodes: EpisodeItem[];
}

export interface MediaRecord {
  readonly id: string; // rec_01K...
  readonly title: string;
  readonly originalTitle?: string;
  readonly mediaKind: MediaKind;
  readonly customTypeName?: string;
  readonly releaseYear?: number;
  readonly airDates?: string;
  readonly format?: string; // TV, Movie, OVA, Miniseries, Hardcover, LP
  readonly statusText?: string; // Ended, Returning Series, In Production
  readonly country?: string;
  readonly languages?: string[];
  readonly runtimeMinutes?: number;
  readonly overview?: string;
  readonly posterUrl?: string;
  readonly backdropUrl?: string;
  readonly status: WatchStatus;
  readonly userRating?: number; // 1-10
  readonly communityRating?: {
    readonly score: number;
    readonly votes: number;
    readonly source: string;
  };
  readonly progressSeconds?: number;
  readonly totalDurationSeconds?: number;
  readonly progressEpisodes?: number;
  readonly totalEpisodes?: number;
  readonly externalIds: ExternalId[];
  readonly displaySource: string;
  readonly userNotes?: string;
  readonly tags: string[];
  readonly genres: string[];
  readonly studios: string[];
  readonly lastActivityAt?: string;
  readonly seasons?: SeasonItem[];
  readonly cast?: CastMember[];
  readonly crew?: CrewMember[];
  readonly collectionName?: string;
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
  readonly tokenSecret?: string;
  readonly scopes: string[];
  readonly createdAt: string;
  readonly lastUsedAt?: string;
}

export interface ProviderApiKeyConfig {
  readonly provider: string;
  readonly label: string;
  readonly isConfigured: boolean;
  readonly docsUrl: string;
}

export interface OidcConfiguration {
  readonly enabled: boolean;
  readonly issuerUrl: string;
  readonly clientId: string;
  readonly clientSecret: string;
  readonly redirectUri: string;
  readonly autoProvisionUsers: boolean;
}

export interface AppriseNotificationConfig {
  readonly enabled: boolean;
  readonly urls: string[];
  readonly notifyOnReviewRequired: boolean;
  readonly notifyOnSyncError: boolean;
  readonly notifyOnMilestone: boolean;
}

export interface ThemeSettings {
  readonly mode: "light" | "dark" | "night";
  readonly accentColor: string;
  readonly fontFamily?: "sans-serif" | "serif" | "monospace";
  readonly themeBase?: "slate" | "gray" | "zinc" | "neutral" | "stone";
  readonly cornerRadius?: number;
  readonly density: "compact" | "normal" | "spacious";
  readonly fontSize?: "sm" | "md" | "lg";
}

export type ActiveNavSection =
  | "home"
  | "discover"
  | "tv_shows"
  | "tv_seasons"
  | "movies"
  | "anime"
  | "manga"
  | "games"
  | "books"
  | "comics"
  | "board_games"
  | "music"
  | "podcasts"
  | "calendar"
  | "collection"
  | "custom"
  | "history"
  | "lists"
  | "statistics"
  | "tags"
  | "reconciliation"
  | "sources"
  | "connections"
  | "settings"
  | "detail"
  | "chronicle"
  | "library";

export interface NavItemConfig {
  id: ActiveNavSection;
  label: string;
  category: "primary" | "media" | "library" | "utilities";
  visible: boolean;
  pinned: boolean;
  order: number;
}

export interface ContextMenuItemConfig {
  id: string;
  label: string;
  visible: boolean;
  order: number;
}

export interface WorkbenchPreferences {
  sidebarCollapsed: boolean;
  sidebarHidden: boolean;
  navItems: NavItemConfig[];
  contextMenuItems: ContextMenuItemConfig[];
}
