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
  namespace: string;
  value: string;
  status: "matched" | "needs_review" | "local_only" | "retired";
  source: string;
  url?: string;
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
  title: string;
  overview?: string;
  airDate?: string;
  durationSeconds?: number;
  watched: boolean;
  watchedAt?: string;
  userRating?: number;
}

export interface SeasonItem {
  readonly seasonNumber: number;
  title: string;
  posterUrl?: string;
  episodeCount: number;
  episodes: EpisodeItem[];
}

export interface MediaRecord {
  readonly id: string; // rec_01K...
  title: string;
  originalTitle?: string;
  mediaKind: MediaKind;
  customTypeName?: string;
  releaseYear?: number;
  airDates?: string;
  format?: string; // TV, Movie, OVA, Miniseries, Hardcover, LP
  statusText?: string; // Ended, Returning Series, In Production
  country?: string;
  languages?: string[];
  runtimeMinutes?: number;
  overview?: string;
  posterUrl?: string;
  backdropUrl?: string;
  status: WatchStatus;
  userRating?: number; // 1-10
  communityRating?: {
    score: number;
    votes: number;
    source: string;
  };
  progressSeconds?: number;
  totalDurationSeconds?: number;
  progressEpisodes?: number;
  totalEpisodes?: number;
  externalIds: ExternalId[];
  displaySource: string;
  userNotes?: string;
  tags: string[];
  genres: string[];
  studios: string[];
  lastActivityAt?: string;
  seasons?: SeasonItem[];
  cast?: CastMember[];
  crew?: CrewMember[];
  customFields?: Record<string, any>;
  collectionName?: string;
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

export type ManagedSettingSource =
  "default" | "saved" | "build" | "environment";

export interface ManagedSetting<T> {
  readonly value: T;
  readonly source: ManagedSettingSource;
  readonly managed: boolean;
}

export type NetworkClass =
  | "public"
  | "loopback"
  | "private"
  | "link_local"
  | "multicast"
  | "unspecified"
  | "documentation"
  | "reserved";

export interface OutboundAccessPolicy {
  readonly allow_providers: string[];
  readonly deny_providers: string[];
  readonly allow_capabilities: string[];
  readonly deny_capabilities: string[];
  readonly allow_hosts: string[];
  readonly deny_hosts: string[];
  readonly allow_networks: NetworkClass[];
  readonly deny_networks: NetworkClass[];
}

export interface ConnectionPreferenceView {
  readonly service_url: ManagedSetting<string>;
  readonly public_url: ManagedSetting<string | null>;
}

export interface NetworkConfiguration {
  readonly connection: ConnectionPreferenceView;
  readonly outbound_policy: OutboundAccessPolicy;
}

export interface SaveNetworkConfigurationRequest {
  readonly service_url: string;
  readonly public_url: string | null;
  readonly outbound_policy: OutboundAccessPolicy;
}

export interface EndpointConnectionStatus {
  readonly endpoint: string;
  readonly scheme: "http" | "https";
  readonly status: string;
  readonly version: string;
}

export interface ProviderApiKeyConfig {
  readonly provider: string;
  readonly label: string;
  readonly apiKey?: string;
  readonly isConfigured: boolean;
  readonly docsUrl?: string;
}

export interface ProviderCredentialStatus {
  readonly provider: string;
  readonly label: string;
  readonly configured: boolean;
  readonly source: "none" | "environment" | "credential_store";
  readonly writable: boolean;
  readonly docs_url: string;
}

export interface ProviderSearchCandidate {
  readonly provider: string;
  readonly provider_id: string;
  readonly title: string;
  readonly original_title?: string;
  readonly kind: MediaKind | string;
  readonly release_year?: number;
  readonly authors: string[];
  readonly image_url: string | null;
  readonly overview?: string;
  readonly external_ids?: ExternalId[];
}

export interface WorkbenchHost {
  loadNetworkConfiguration(): Promise<NetworkConfiguration>;
  saveNetworkConfiguration(
    input: SaveNetworkConfigurationRequest,
  ): Promise<NetworkConfiguration>;
  testEndpointConnection(endpoint: string): Promise<EndpointConnectionStatus>;
  providerCredentialStatus(): Promise<ProviderCredentialStatus[]>;
  saveProviderCredential(
    provider: string,
    credential: string,
  ): Promise<ProviderCredentialStatus[]>;
  deleteProviderCredential(
    provider: string,
  ): Promise<ProviderCredentialStatus[]>;
  searchProvider(
    provider: string,
    query: string,
  ): Promise<ProviderSearchCandidate[]>;
  clearSearchCache?(): void;
  getSearchCacheSize?(): number;
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

export interface CustomMetadataField {
  id: string;
  name: string;
  key: string;
  type: "text" | "number" | "date" | "boolean" | "select";
  targetKinds: (MediaKind | "all")[];
  options?: string[];
  description?: string;
}

export interface CustomMediaType {
  id: string;
  name: string;
  singular: string;
  plural: string;
  icon?: string;
  trackProgress: "episodes" | "pages" | "percent" | "duration" | "binary";
}

export interface WorkbenchPreferences {
  sidebarCollapsed: boolean;
  sidebarHidden: boolean;
  titleLanguagePreference?: "english" | "romaji" | "native";
  showOriginalTitleSubtitle?: boolean;
  cacheMetadataLocally?: boolean;
  providerRegion?: string;
  metadataLanguage?: string;
  tvProvider?: "tmdb" | "tvdb";
  animeProvider?: "mal" | "anilist" | "kitsu";
  animeLibrary?: "separate" | "unified";
  hideCompleted?: "disabled" | "home" | "all";
  hideZeroRatings?: boolean;
  homeButtons?: "none" | "quick_track" | "context_only";
  gameLogging?: "repeats" | "sessions";
  progressFormat?: "pages_chapters" | "percent" | "episodes_minutes";
  sessionDuration?: "2_weeks" | "30_days" | "never";
  rpdbApiKey?: string;
  rpdbEnabled?: boolean;
  tvdbApiKey?: string;
  tvdbUserPin?: string;
  collectionsManifestUrl?: string;
  customFields?: CustomMetadataField[];
  customMediaTypes?: CustomMediaType[];
  navItems: NavItemConfig[];
  contextMenuItems: ContextMenuItemConfig[];
}
