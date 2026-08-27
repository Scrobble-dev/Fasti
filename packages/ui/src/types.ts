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
  | "unknown"
  | "watching"
  | "completed"
  | "plan_to_watch"
  | "on_hold"
  | "dropped";

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
  readonly role: string;
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
  readonly id: string;
  readonly title: string;
  readonly originalTitle?: string;
  readonly mediaKind: MediaKind;
  readonly customTypeName?: string;
  readonly releaseYear?: number;
  readonly airDates?: string;
  readonly format?: string;
  readonly statusText?: string;
  readonly country?: string;
  readonly languages?: string[];
  readonly runtimeMinutes?: number;
  readonly overview?: string;
  readonly posterUrl?: string;
  readonly backdropUrl?: string;
  readonly status: WatchStatus;
  readonly userRating?: number;
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
  /** User-defined key/value metadata registered in Settings → Custom Fields. */
  readonly customFields?: Record<string, string>;
}

export interface ChronicleOccurrence {
  readonly id: string;
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

export interface CustomFieldDefinition {
  readonly key: string;
  readonly label: string;
  readonly targetType: MediaKind | "all";
  readonly valueType:
    "string" | "number" | "boolean" | "date" | "url" | "identifier" | "select";
  readonly registeredNamespace?: string;
  readonly isFilterable: boolean;
  /** Choices for the field when `valueType` is `"select"`. */
  readonly options?: string[];
}

export interface CustomMediaTypeDefinition {
  readonly id: string;
  readonly name: string;
  readonly singular: string;
  readonly plural: string;
  readonly icon: string;
  readonly progressTrackingType:
    "episodes" | "percentage" | "pages" | "sessions" | "none";
}

/** Legacy type retained for compatibility with prototype-only components. */
export interface ScopedApiToken {
  readonly id: string;
  readonly name: string;
  readonly tokenPrefix: string;
  readonly tokenSecret?: string;
  readonly scopes: string[];
  readonly createdAt: string;
  readonly lastUsedAt?: string;
}

export interface ApiClientCredentialSummary {
  readonly client_id: string;
  readonly credential_id: string;
  readonly profile_id: string;
  readonly scopes: string[];
  readonly active: boolean;
  readonly created_at: string;
  readonly revoked_at?: string | null;
}

export interface CreatedApiClientCredential extends ApiClientCredentialSummary {
  /** Returned once by the trusted host. The workbench must never persist this value. */
  readonly credential: string;
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
  listApiClients?(): Promise<ApiClientCredentialSummary[]>;
  createApiClient?(scopes: string[]): Promise<CreatedApiClientCredential>;
  revokeApiClient?(credentialId: string): Promise<ApiClientCredentialSummary[]>;
  clearSearchCache?(): void;
  getSearchCacheSize?(): number;
  listReviews?(): Promise<ReviewItem[]>;
  resolveReview?(input: ResolveReviewInput): Promise<ResolveReviewOutcome>;
  listRecords?(): Promise<RecordSummary[]>;
  createRecord?(grain: string): Promise<CreateRecordResult>;
  attachIdentifier?(
    input: AttachIdentifierInput,
  ): Promise<AttachIdentifierResult>;
  registerNamespace?(
    input: RegisterNamespaceInput,
  ): Promise<RegisterNamespaceResult>;
  setSessionCredential?(credential: string): void;
  clearSessionCredential?(): void;
}

/** Wire shape of the desktop host's `create_record` command output. */
export interface CreateRecordResult {
  readonly record_id: string;
  readonly grain: string;
}

/** Wire shape of the desktop host's `attach_identifier` command input.
 * `grain` must match the target record's own grain -- the host rejects a
 * mismatch, and rejects any namespace that hasn't been registered first
 * (see `registerNamespace`). */
export interface AttachIdentifierInput {
  readonly record_id: string;
  readonly namespace: string;
  readonly grain: string;
  readonly value: string;
}

export interface AttachIdentifierResult {
  readonly external_identifier_id: string;
  readonly record_id: string;
  readonly created: boolean;
}

/** Wire shape of the desktop host's `register_namespace` command input.
 * Declares which grains a provider namespace (e.g. "google_books") may
 * attach claims for -- required once per namespace before the first
 * `attachIdentifier` call under it. */
export interface RegisterNamespaceInput {
  readonly namespace: string;
  readonly label: string;
  readonly grains: string[];
  readonly id_pattern: string;
  readonly normalization: string;
  readonly licence_posture:
    "open" | "identifiers_only" | "indirect_only" | "excluded" | "unknown";
}

export interface RegisterNamespaceResult {
  readonly namespace: string;
  readonly created: boolean;
}

/** Which tier of the resolution order actually supplied a field's displayed value.
 * Mirrors `fasti_domain::FieldResolutionTier`'s serde representation. */
export type FieldResolutionTier =
  | "user_override"
  | "preferred_provider_claim"
  | "fallback_provider_claim"
  | "last_known_good"
  | "empty";

/** Wire projection of `fasti_domain::ResolvedField` / the desktop host's `ResolvedFieldView`. */
export interface ResolvedFieldView {
  readonly tier: FieldResolutionTier;
  readonly value: string | null;
  readonly source: string | null;
  readonly is_stale: boolean;
}

/** Mirrors `fasti_domain::ClaimedTime`'s wire shape (`ClaimedTimeWire`). */
export interface ClaimedTimeView {
  readonly original: string;
  readonly precision:
    "date" | "second" | "millisecond" | "microsecond" | "nanosecond";
  readonly trust:
    "source_claim" | "device_observed" | "user_entered" | "inferred";
}

export interface RecordActivityView {
  readonly occurred_at: ClaimedTimeView | null;
  readonly interpretation_state: "unresolved" | "resolved" | "conflicted";
}

/** Wire shape of the desktop host's `list_records` command output
 * (`apps/desktop/src-tauri/src/records.rs`'s `RecordSummary`). `grain` is
 * identity granularity (`fasti_domain::Grain`), not the frontend's display
 * `MediaKind` — see `projectRecordSummary` in `record-projection.ts` for the
 * mapping. */
export interface RecordSummary {
  readonly record_id: string;
  readonly grain: string;
  readonly status: "active";
  readonly title: ResolvedFieldView;
  readonly poster: ResolvedFieldView;
  readonly latest_activity: RecordActivityView | null;
}

export interface ReviewItem {
  readonly review_item_id: string;
  readonly observation_id: string;
  readonly current_interpretation_id: string;
  readonly status: "open" | "deferred" | "resolved";
  readonly candidate_record_ids: string[];
}

export type ReviewResolutionTargetInput =
  { kind: "existing"; value: string } | { kind: "new"; value: string };

export interface ExternalIdentifierClaimInput {
  readonly namespace: string;
  readonly grain: string;
  readonly value: string;
}

export interface ResolveReviewInput {
  readonly review_item_id: string;
  readonly target: ReviewResolutionTargetInput;
  readonly identifiers: ExternalIdentifierClaimInput[];
}

export interface ResolveReviewOutcome {
  readonly review_item_id: string;
  readonly record_id: string;
  readonly interpretation_id: string;
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

export type TvMetadataProvider = "tmdb" | "tvdb_v4";
export type AnimeMetadataProvider = "mal" | "anilist" | "kitsu";
export type TitleLanguagePreference = "romaji" | "english" | "native";
export type HideCompletedMode = "disabled" | "home_only" | "everywhere";
export type GameLoggingMode = "repeats" | "sessions";
export type ProgressFormat = "percentage" | "time_remaining" | "episodes";

export interface WorkbenchPreferences {
  sidebarCollapsed: boolean;
  sidebarHidden: boolean;
  navItems: NavItemConfig[];
  contextMenuItems: ContextMenuItemConfig[];
  /** ISO 3166-1 alpha-2 region used to bias provider search/availability. */
  providerRegion: string;
  /** BCP 47 language tag used to request localized metadata. */
  metadataLanguage: string;
  tvProvider: TvMetadataProvider;
  animeProvider: AnimeMetadataProvider;
  titleLanguage: TitleLanguagePreference;
  hideCompleted: HideCompletedMode;
  hideZeroRatings: boolean;
  gameLogging: GameLoggingMode;
  progressFormat: ProgressFormat;
  /** Minutes of inactivity before a session is considered stale. */
  sessionDuration: number;
  customFields: CustomFieldDefinition[];
  customMediaTypes: CustomMediaTypeDefinition[];
}
