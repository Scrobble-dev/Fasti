<script lang="ts">
  import type {
    MediaRecord,
    MetadataProjectionResponse,
    WatchStatus,
    ExternalId,
    ChronicleOccurrence,
    ContextMenuItemConfig,
    ProviderCredentialStatus,
    ProviderSearchCandidate,
    ProviderSelection,
    TrackingDispositionUpdate,
  } from "./types.js";
  import {
    IconArrowLeft,
    IconStarFilled,
    IconCheck,
    IconBookmark,
    IconCalendar,
    IconRepeat,
    IconExternalLink,
    IconShieldCheck,
    IconNotes,
    IconListNumbers,
    IconHistory,
    IconAdjustments,
    IconFolderPlus,
    IconMessage,
    IconDotsVertical,
    IconX,
    IconEdit,
    IconClock,
    IconDeviceTv,
    IconPhoto,
    IconRefresh,
  } from "@tabler/icons-svelte";
  import ProgressModal from "./progress-modal.svelte";
  import RatingReviewModal from "./rating-review-modal.svelte";
  import CollectionModal from "./collection-modal.svelte";
  import ArtworkModal, { type ArtworkCandidate } from "./artwork-modal.svelte";
  import ContextMenu, { type ContextMenuItem } from "./context-menu.svelte";
  import DiscoverView from "./discover-view.svelte";
  import { hostProblemText } from "./host-problem.js";
  import TmdbAttribution from "./tmdb-attribution.svelte";
  import { recordContextMenuItems } from "./record-actions.js";

  /** Namespaces whose external ID value is a real key an image CDN can
   * resolve directly, and the resolver for that namespace. Providers whose
   * poster lives behind an opaque image path/hash (TMDB, TVDB, AniList, MAL,
   * Kitsu, IGDB, ...) are deliberately absent: there's no live provider
   * search wired here to fetch that path, so fabricating a guessed URL would
   * be dishonest. Add a resolver here once a provider integration can supply
   * the real path. */
  const ARTWORK_URL_RESOLVERS: Record<
    string,
    (value: string) => string | null
  > = {
    openlibrary: (value) =>
      /^OL\d+[MW]$/i.test(value)
        ? `https://covers.openlibrary.org/b/olid/${value}-L.jpg`
        : null,
  };

  interface Props {
    record: MediaRecord;
    availableCollections: string[];
    occurrences?: ChronicleOccurrence[];
    initialTab?: "overview" | "sources";
    contextMenuConfigs?: ContextMenuItemConfig[];
    providerCredentials?: ProviderCredentialStatus[];
    providerLoading?: boolean;
    providerHostProblem?: string;
    metadataProjection?: MetadataProjectionResponse;
    metadataProjectionLoading?: boolean;
    metadataProjectionProblem?: string;
    metadataRefreshUnavailableFieldGroups?: string[];
    metadataRefreshableFieldGroupCount?: number;
    onBack: () => void;
    onSearchMetadata?: (
      provider: string,
      query: string,
    ) => Promise<ProviderSearchCandidate[]>;
    onApplyMetadata?: (
      recordId: string,
      selection: ProviderSelection,
    ) => Promise<void>;
    onOpenProviderSettings?: () => void;
    onRetryProviders?: () => void;
    onRetryMetadataProjection?: () => void;
    onRefreshMetadataClaims?: (providerId: string) => Promise<void>;
    onSetTrackingDisposition?: (
      recordId: string,
      disposition: TrackingDispositionUpdate,
    ) => void;
    onOpenReconciliation?: () => void;
    onUpdateStatus?: (recordId: string, status: WatchStatus) => void;
    onUpdateRating?: (recordId: string, rating: number) => void;
    onToggleEpisode?: (recordId: string, episodeId: string) => void;
    onUpdateProgress?: (
      recordId: string,
      episodes: number,
      seconds: number,
      status: WatchStatus,
    ) => void;
    onSaveReview?: (recordId: string, rating: number, notes: string) => void;
    onSaveCollection?: (recordId: string, collections: string[]) => void;
    onUpdateNotes?: (recordId: string, notes: string) => void;
    onAddTag?: (recordId: string, tag: string) => void;
    onRemoveTag?: (recordId: string, tag: string) => void;
    onUpdatePoster?: (
      recordId: string,
      posterUrl: string,
      backdropUrl?: string,
    ) => void;
  }

  let {
    record,
    availableCollections,
    occurrences = [],
    initialTab = "overview",
    contextMenuConfigs,
    providerCredentials,
    providerLoading = false,
    providerHostProblem,
    metadataProjection,
    metadataProjectionLoading = false,
    metadataProjectionProblem,
    metadataRefreshUnavailableFieldGroups = [],
    metadataRefreshableFieldGroupCount = 0,
    onBack,
    onSearchMetadata,
    onApplyMetadata,
    onOpenProviderSettings,
    onRetryProviders,
    onRetryMetadataProjection,
    onRefreshMetadataClaims,
    onSetTrackingDisposition,
    onOpenReconciliation,
    onUpdateStatus,
    onUpdateRating,
    onToggleEpisode,
    onUpdateProgress,
    onSaveReview,
    onSaveCollection,
    onUpdateNotes,
    onAddTag,
    onRemoveTag,
    onUpdatePoster,
  }: Props = $props();

  let activeTab: "overview" | "actions" | "history" | "sources" | "reviews" =
    $state("overview");
  let selectedSeasonIndex = $state(0);
  let isEditingNotes = $state(false);
  let editedNotesText = $state("");
  let newTagInput = $state("");
  let syncedRecordId = $state("");

  // Modal Dialog States
  let showProgressModal = $state(false);
  let showReviewModal = $state(false);
  let showCollectionModal = $state(false);
  let showArtworkModal = $state(false);

  let metadataRefreshingId = $state("");
  let metadataProblem = $state("");
  let metadataNotice = $state("");
  let projectedMetadataRefreshingProvider = $state("");

  // Context Menu State
  let contextMenuState = $state<{
    x: number;
    y: number;
    items: ContextMenuItem[];
  } | null>(null);

  $effect(() => {
    if (record.id !== syncedRecordId) {
      syncedRecordId = record.id;
      activeTab = initialTab;
      editedNotesText = record.userNotes ?? "";
      isEditingNotes = false;
      metadataProblem = "";
      metadataNotice = "";
    }
    const seasonCount = record.seasons?.length ?? 0;
    if (seasonCount === 0) {
      selectedSeasonIndex = 0;
    } else if (selectedSeasonIndex >= seasonCount) {
      selectedSeasonIndex = seasonCount - 1;
    }
  });

  const recordOccurrences = $derived(
    occurrences.filter((occ) => occ.recordId === record.id),
  );
  const isSummary = $derived(record.detailLevel === "summary");
  const hasRecordMutations = $derived(
    Boolean(
      onUpdateStatus ||
      onUpdateRating ||
      onToggleEpisode ||
      onUpdateProgress ||
      onSaveReview ||
      onSaveCollection ||
      onUpdateNotes ||
      onAddTag ||
      onRemoveTag ||
      onUpdatePoster,
    ),
  );

  const candidatePosters = $derived(
    record.externalIds
      .map((xid): ArtworkCandidate | null => {
        const resolver = ARTWORK_URL_RESOLVERS[xid.namespace.toLowerCase()];
        const url = resolver?.(xid.value) ?? null;
        return url
          ? {
              id: `${xid.namespace}:${xid.value}`,
              namespace: xid.namespace,
              url,
            }
          : null;
      })
      .filter((c): c is ArtworkCandidate => c !== null),
  );

  const trackingOptions: Array<{
    id: TrackingDispositionUpdate;
    label: string;
  }> = [
    { id: "unset", label: "Automatic from activity" },
    { id: "watching", label: "In Progress / Watching" },
    { id: "on_hold", label: "On Hold" },
    { id: "dropped", label: "Dropped" },
  ];

  const selectedTrackingDisposition = $derived<TrackingDispositionUpdate>(
    record.trackingDisposition ?? "unset",
  );
  const projectionProviders = $derived(
    Array.from(
      new Set([
        ...(metadataProjection?.policy.preferred_provider_id
          ? [metadataProjection.policy.preferred_provider_id]
          : []),
        ...(metadataProjection?.attributions.map(
          (attribution) => attribution.provider_id,
        ) ?? []),
        ...(metadataProjection?.fields.flatMap((field) =>
          field.provenance?.provider_id ? [field.provenance.provider_id] : [],
        ) ?? []),
        ...(metadataProjection?.ratings.flatMap((rating) =>
          rating.provenance.provider_id ? [rating.provenance.provider_id] : [],
        ) ?? []),
      ]),
    ),
  );
  const offlineCacheEntries = $derived(
    metadataProjection?.cache_entries.filter(
      (entry) => entry.key.purpose === "offline_read",
    ) ?? [],
  );

  function metadataTimestamp(value: string | null | undefined): string {
    if (!value) return "Not declared";
    const timestamp = new Date(value);
    return Number.isNaN(timestamp.getTime())
      ? value
      : new Intl.DateTimeFormat(undefined, {
          dateStyle: "medium",
          timeStyle: "short",
        }).format(timestamp);
  }

  function ratingValue(
    value: number,
    scale: { minimum_millis: number; maximum_millis: number },
  ): string {
    return `${(value / 1000).toLocaleString()} on ${(
      scale.minimum_millis / 1000
    ).toLocaleString()}–${(scale.maximum_millis / 1000).toLocaleString()}`;
  }

  async function refreshProjectedMetadata(providerId: string): Promise<void> {
    if (!onRefreshMetadataClaims || projectedMetadataRefreshingProvider) return;
    projectedMetadataRefreshingProvider = providerId;
    metadataProblem = "";
    metadataNotice = "";
    try {
      await onRefreshMetadataClaims(providerId);
      metadataNotice = `Refreshed governed claims from ${providerLabel(providerId)}.`;
    } catch (error) {
      metadataProblem = hostProblemText(
        error,
        "Governed metadata claim refresh failed.",
      );
    } finally {
      projectedMetadataRefreshingProvider = "";
    }
  }

  function handleSaveNotes(): void {
    if (!onUpdateNotes) return;
    onUpdateNotes(record.id, editedNotesText);
    isEditingNotes = false;
  }

  function providerKindForRecord(): "book" | "movie" | "show" | null {
    if (record.mediaKind === "book") return "book";
    if (record.mediaKind === "movie") return "movie";
    if (record.mediaKind === "show" || record.mediaKind === "anime") {
      return "show";
    }
    return null;
  }

  function providerLabel(provider: string): string {
    return (
      providerCredentials?.find(
        (credential) => credential.provider === provider,
      )?.label ??
      (provider === "google-books"
        ? "Google Books"
        : provider === "tmdb"
          ? "TMDB"
          : provider)
    );
  }

  function providerMappingForIdentifier(
    xid: ExternalId,
  ): Omit<ProviderSelection, "provider_id"> | null {
    const namespace = xid.namespace.toLowerCase();
    if (namespace === "googlebooks.volume" || namespace === "google-books") {
      return { provider: "google-books", kind: "book" };
    }
    if (namespace === "tmdb.movie") return { provider: "tmdb", kind: "movie" };
    if (namespace === "tmdb.tv") return { provider: "tmdb", kind: "show" };
    const currentKind = providerKindForRecord();
    return namespace === "tmdb" &&
      (currentKind === "movie" || currentKind === "show")
      ? { provider: "tmdb", kind: currentKind }
      : null;
  }

  function refreshSelection(xid: ExternalId): ProviderSelection | null {
    const mapping = providerMappingForIdentifier(xid);
    return mapping?.kind === providerKindForRecord()
      ? { ...mapping, provider_id: xid.value }
      : null;
  }

  async function searchCompatibleMetadata(
    provider: string,
    query: string,
  ): Promise<ProviderSearchCandidate[]> {
    if (!onSearchMetadata) return [];
    const kind = providerKindForRecord();
    if (!kind) return [];
    const candidates = await onSearchMetadata(provider, query);
    return candidates.filter((candidate) => candidate.kind === kind);
  }

  async function chooseMetadata(
    candidate: ProviderSearchCandidate,
  ): Promise<void> {
    if (!onApplyMetadata) return;
    await onApplyMetadata(record.id, {
      provider: candidate.provider,
      provider_id: candidate.provider_id,
      kind: candidate.kind,
    });
    metadataNotice = `Applied metadata from ${candidate.title}.`;
  }

  async function refreshMetadata(xid: ExternalId): Promise<void> {
    const selection = refreshSelection(xid);
    if (!selection || !onApplyMetadata || metadataRefreshingId) return;
    metadataRefreshingId = `${xid.namespace}:${xid.value}`;
    metadataProblem = "";
    metadataNotice = "";
    try {
      await onApplyMetadata(record.id, selection);
      metadataNotice = `Refreshed metadata from ${providerLabel(selection.provider)}.`;
    } catch (error) {
      metadataProblem = hostProblemText(error, "Metadata refresh failed.");
    } finally {
      metadataRefreshingId = "";
    }
  }

  function handleAddTagSubmit(e: Event): void {
    e.preventDefault();
    if (onAddTag && newTagInput.trim().length > 0) {
      onAddTag(record.id, newTagInput.trim());
      newTagInput = "";
    }
  }

  function handleMarkSeasonWatched(seasonIndex: number): void {
    if (!onToggleEpisode) return;
    const season = record.seasons?.[seasonIndex];
    if (!season) return;
    for (const ep of season.episodes ?? []) {
      if (!ep.watched) onToggleEpisode(record.id, ep.id);
    }
  }

  function handleMarkSeasonUnwatched(seasonIndex: number): void {
    if (!onToggleEpisode) return;
    const season = record.seasons?.[seasonIndex];
    if (!season) return;
    for (const ep of season.episodes ?? []) {
      if (ep.watched) onToggleEpisode(record.id, ep.id);
    }
  }

  function handleMarkPreviousEpisodesWatched(
    seasonIndex: number,
    upToEpisodeNumber: number,
  ): void {
    if (!onToggleEpisode) return;
    const season = record.seasons?.[seasonIndex];
    if (!season) return;
    for (const ep of season.episodes ?? []) {
      if (ep.number < upToEpisodeNumber && !ep.watched) {
        onToggleEpisode(record.id, ep.id);
      }
    }
  }

  function handleOpenContextMenu(e: MouseEvent): void {
    e.preventDefault();
    contextMenuState = {
      x: e.clientX,
      y: e.clientY,
      items: recordContextMenuItems(
        record,
        {
          onView: () => (activeTab = "overview"),
          onSetTrackingDisposition: onSetTrackingDisposition
            ? (disposition) => onSetTrackingDisposition(record.id, disposition)
            : undefined,
          onMarkCompleted: onUpdateStatus
            ? () =>
                onUpdateStatus(
                  record.id,
                  record.status === "completed" ? "watching" : "completed",
                )
            : undefined,
          onUpdateProgress: onUpdateProgress
            ? () => (showProgressModal = true)
            : undefined,
          onToggleWatchlist: onUpdateStatus
            ? () =>
                onUpdateStatus(
                  record.id,
                  record.status === "plan_to_watch"
                    ? "watching"
                    : "plan_to_watch",
                )
            : undefined,
          onOpenCollection: onSaveCollection
            ? () => (showCollectionModal = true)
            : undefined,
          onOpenReview: onSaveReview
            ? () => (showReviewModal = true)
            : undefined,
          onEditTags:
            onAddTag || onRemoveTag
              ? () => (activeTab = "overview")
              : undefined,
          onInspectIds: () => (activeTab = "sources"),
          onReconcile: onOpenReconciliation,
          onCopyId:
            typeof navigator !== "undefined" && navigator.clipboard
              ? () =>
                  void navigator.clipboard.writeText(record.id).catch(() => {})
              : undefined,
        },
        contextMenuConfigs,
      ),
    };
  }
</script>

<div class="detail-container">
  <!-- Top Navigation Bar -->
  <div class="top-nav-bar">
    <button type="button" class="back-btn" onclick={onBack}>
      <IconArrowLeft size={16} stroke={2} />
      <span>Back to Library</span>
    </button>

    <div class="top-id-badge">
      <span class="id-label">Fasti Entity ID:</span>
      <code>{record.id}</code>
    </div>
  </div>

  <!-- Main Media Header (Floppy / Yamtrack / Ryot Editorial Style) -->
  <header class="media-main-header">
    <!-- Left Poster Column -->
    <div class="poster-column">
      <div class="poster-frame">
        {#if record.posterUrl}
          <img
            src={record.posterUrl}
            alt="{record.title} Poster"
            class="main-poster"
          />
        {:else}
          <div class="fallback-poster">{record.mediaKind}</div>
        {/if}
      </div>
    </div>

    <!-- Center/Right Details Column -->
    <div class="header-details-column">
      <div class="title-row">
        <h1 class="main-title">{record.title}</h1>
      </div>

      {#if record.originalTitle}
        <h2 class="original-title">{record.originalTitle}</h2>
      {/if}

      <!-- Progress & Watched Stats Meta Row -->
      <div class="meta-strip">
        {#if record.collectionName}
          <span class="collection-pill">{record.collectionName}</span>
          <span class="bullet">·</span>
        {/if}
        {#if record.progressEpisodes !== undefined && record.totalEpisodes !== undefined}
          <span class="meta-item">
            <strong>Progress:</strong>
            {record.progressEpisodes} of {record.totalEpisodes} eps
          </span>
          <span class="bullet">·</span>
        {/if}
        {#if record.airDates}
          <span class="meta-item">{record.airDates}</span>
          <span class="bullet">·</span>
        {/if}
        {#if record.runtimeMinutes}
          <span class="meta-item"
            >{Math.floor(record.runtimeMinutes / 60)}h {record.runtimeMinutes %
              60}m</span
          >
        {/if}
      </div>

      <!-- Ratings & Action Button Row -->
      <div class="action-strip">
        <!-- Community Score Badge -->
        {#if record.communityRating}
          <div class="score-badge community">
            <span class="score-num">{record.communityRating.score}</span>
            <span class="score-meta"
              >{record.communityRating.votes.toLocaleString()}
              {record.communityRating.source} ratings</span
            >
          </div>
        {/if}

        <!-- User Rating Picker -->
        <div class="user-rating-box">
          <span class="user-star">★</span>
          <select
            class="rating-select"
            value={record.userRating ?? 0}
            disabled={!onUpdateRating}
            onchange={(e) =>
              onUpdateRating?.(record.id, Number(e.currentTarget.value))}
            aria-label={onUpdateRating
              ? "User rating"
              : "User rating unavailable"}
            title={onUpdateRating
              ? "User rating"
              : "Ratings are not available on this host"}
          >
            <option value="0">Unrated</option>
            {#each [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as num}
              <option value={num}>{num} ★ ({num}/10)</option>
            {/each}
          </select>
        </div>

        <!-- Profile-owned tracking state. Completion and watchlist intent are
             separate domains and are not folded into this selector. -->
        <div class="status-select-wrap">
          <select
            class="status-select {record.status}"
            value={selectedTrackingDisposition}
            disabled={!onSetTrackingDisposition}
            onchange={(e) =>
              onSetTrackingDisposition?.(
                record.id,
                e.currentTarget.value as TrackingDispositionUpdate,
              )}
            aria-label="Profile tracking state"
          >
            {#each trackingOptions as opt}
              <option value={opt.id}>{opt.label}</option>
            {/each}
          </select>
        </div>

        <!-- Quick Action Buttons -->
        <button
          type="button"
          class="icon-action-btn"
          disabled={!onSaveCollection}
          onclick={() => (showCollectionModal = true)}
          title={onSaveCollection
            ? "Add to Collection"
            : "Collections are not active on this host"}
        >
          <IconBookmark size={18} />
        </button>

        <button
          type="button"
          class="icon-action-btn"
          disabled={!onUpdatePoster}
          onclick={() => (showArtworkModal = true)}
          title={onUpdatePoster
            ? "Edit Artwork"
            : "Artwork editing is not active on this host"}
        >
          <IconPhoto size={18} />
        </button>

        <button
          type="button"
          class="icon-action-btn"
          disabled={!onUpdateProgress}
          onclick={() => (showProgressModal = true)}
          title={onUpdateProgress
            ? "Update Progress"
            : "Progress editing is not active on this host"}
        >
          <IconAdjustments size={18} />
        </button>

        <button
          type="button"
          class="icon-action-btn"
          onclick={handleOpenContextMenu}
          title="More actions"
          aria-label="More actions"
        >
          <IconDotsVertical size={18} />
        </button>
      </div>

      <!-- Synopsis -->
      <p class="synopsis-prose">
        {record.overview ??
          (isSummary
            ? "A synopsis is not included in this record summary."
            : "No synopsis is recorded for this record.")}
      </p>
    </div>
  </header>

  <!-- Two-Column Layout: Left Details Sidebar | Right Tabbed Content -->
  <div class="details-body-grid">
    <!-- Left Metadata Sidebar -->
    <aside class="sidebar-details-card">
      <h2 class="details-sidebar-heading">Details</h2>

      <dl class="sidebar-meta-list">
        {#if record.format}
          <div class="meta-pair">
            <dt>Format</dt>
            <dd>{record.format}</dd>
          </div>
        {/if}

        {#if record.airDates}
          <div class="meta-pair">
            <dt>Air Dates</dt>
            <dd>{record.airDates}</dd>
          </div>
        {/if}

        {#if record.statusText}
          <div class="meta-pair">
            <dt>Status</dt>
            <dd><span class="status-pill">{record.statusText}</span></dd>
          </div>
        {/if}

        {#if record.runtimeMinutes}
          <div class="meta-pair">
            <dt>Runtime</dt>
            <dd>
              {Math.floor(record.runtimeMinutes / 60)}h {record.runtimeMinutes %
                60}m ({record.totalEpisodes ?? 1} total)
            </dd>
          </div>
        {/if}

        {#if record.country}
          <div class="meta-pair">
            <dt>Country</dt>
            <dd>{record.country}</dd>
          </div>
        {/if}

        {#if record.languages && record.languages.length > 0}
          <div class="meta-pair">
            <dt>Languages</dt>
            <dd>{record.languages.join(", ")}</dd>
          </div>
        {/if}
      </dl>

      <!-- Genres -->
      {#if record.genres && record.genres.length > 0}
        <div class="sidebar-section">
          <h3 class="sidebar-subheading">Genres</h3>
          <div class="chips-row">
            {#each record.genres as g}
              <span class="genre-chip">{g}</span>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Studios & Production -->
      {#if record.studios && record.studios.length > 0}
        <div class="sidebar-section">
          <h3 class="sidebar-subheading">Studios & Networks</h3>
          <ul class="studios-list">
            {#each record.studios as st}
              <li>{st}</li>
            {/each}
          </ul>
        </div>
      {/if}

      <!-- External Links -->
      <div class="sidebar-section">
        <h3 class="sidebar-subheading">External Identifiers</h3>
        {#if isSummary}
          <p class="empty-custom-fields-hint">
            External identifiers are not included in this record summary.
          </p>
        {:else if record.externalIds.length === 0}
          <p class="empty-custom-fields-hint">
            No external identifiers are recorded.
          </p>
        {:else}
          <div class="xid-links">
            {#each record.externalIds as xid}
              <a
                href={xid.url ?? "#"}
                target="_blank"
                rel="noopener noreferrer"
                class="xid-link"
                title="Open in {xid.namespace}"
              >
                <span class="ns-tag">{xid.namespace}</span>
                <span class="ns-val">{xid.value}</span>
                <IconExternalLink size={12} class="link-icon" />
              </a>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Custom Fields -->
      <div class="sidebar-section">
        <h3 class="sidebar-subheading">Custom Fields</h3>
        {#if isSummary}
          <p class="empty-custom-fields-hint">
            Custom fields are not included in this record summary.
          </p>
        {:else if record.customFields && Object.keys(record.customFields).length > 0}
          <dl class="sidebar-meta-list">
            {#each Object.entries(record.customFields) as [key, value]}
              <div class="meta-pair">
                <dt>{key}</dt>
                <dd>{value}</dd>
              </div>
            {/each}
          </dl>
        {:else}
          <p class="empty-custom-fields-hint">No custom fields are recorded.</p>
        {/if}
      </div>
    </aside>

    <!-- Right Main Tabbed Content Area (Ryot 5-Tab System) -->
    <section class="main-content-pane" aria-label="Media record sections">
      <!-- Section Tabs -->
      <nav class="content-tabs" aria-label="Media section tabs">
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "overview"}
          onclick={() => (activeTab = "overview")}
          aria-pressed={activeTab === "overview"}
        >
          <IconListNumbers size={16} /> Overview & Seasons
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "actions"}
          onclick={() => (activeTab = "actions")}
          aria-pressed={activeTab === "actions"}
        >
          <IconAdjustments size={16} /> Actions & Progress
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "history"}
          onclick={() => (activeTab = "history")}
          aria-pressed={activeTab === "history"}
        >
          <IconHistory size={16} /> History{isSummary
            ? ""
            : ` (${recordOccurrences.length})`}
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "sources"}
          onclick={() => (activeTab = "sources")}
          aria-pressed={activeTab === "sources"}
        >
          <IconShieldCheck size={16} /> Sources & Identity{isSummary
            ? ""
            : ` (${record.externalIds.length})`}
        </button>

        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === "reviews"}
          onclick={() => (activeTab = "reviews")}
          aria-pressed={activeTab === "reviews"}
        >
          <IconNotes size={16} /> Notes & Reviews
        </button>
      </nav>

      <!-- TAB 1: OVERVIEW & SEASONS -->
      {#if activeTab === "overview"}
        <section class="tab-pane">
          {#if record.seasons && record.seasons.length > 0}
            <!-- Horizontal Season Selector Cards -->
            <div class="seasons-cards-row">
              {#each record.seasons as season, sIdx}
                <button
                  type="button"
                  class="season-card-btn"
                  class:active={selectedSeasonIndex === sIdx}
                  onclick={() => (selectedSeasonIndex = sIdx)}
                  aria-pressed={selectedSeasonIndex === sIdx}
                >
                  {#if season.posterUrl}
                    <img src={season.posterUrl} alt="" class="season-thumb" />
                  {/if}
                  <div class="season-card-info">
                    <span class="season-name">{season.title}</span>
                    <span class="season-count"
                      >{season.episodeCount} Episodes</span
                    >
                  </div>
                </button>
              {/each}
            </div>

            <!-- Episode Checklist -->
            {#if record.seasons[selectedSeasonIndex]}
              <div class="episodes-deck">
                <div class="deck-header-row">
                  <h3 class="deck-title">
                    Episodes — {record.seasons[selectedSeasonIndex].title}
                  </h3>
                  <div class="season-bulk-actions">
                    <button
                      type="button"
                      class="btn-secondary-sm"
                      onclick={() =>
                        handleMarkSeasonWatched(selectedSeasonIndex)}
                      title={onToggleEpisode
                        ? "Mark season watched"
                        : "Episode changes are not available on this host"}
                      disabled={!onToggleEpisode}
                    >
                      Mark Watched
                    </button>
                    <button
                      type="button"
                      class="btn-secondary-sm"
                      onclick={() =>
                        handleMarkSeasonUnwatched(selectedSeasonIndex)}
                      title={onToggleEpisode
                        ? "Mark season unwatched"
                        : "Episode changes are not available on this host"}
                      disabled={!onToggleEpisode}
                    >
                      Mark Unwatched
                    </button>
                  </div>
                </div>
                <div class="episodes-table-wrap">
                  {#each record.seasons[selectedSeasonIndex].episodes ?? [] as ep (ep.id)}
                    <div class="episode-item-row" class:watched={ep.watched}>
                      <button
                        type="button"
                        class="ep-check-btn"
                        class:checked={ep.watched}
                        onclick={() => onToggleEpisode?.(record.id, ep.id)}
                        aria-label={onToggleEpisode
                          ? `Toggle watched for episode ${ep.number}`
                          : `Episode ${ep.number} changes unavailable`}
                        title={onToggleEpisode
                          ? `Toggle watched for episode ${ep.number}`
                          : "Episode changes are not available on this host"}
                        disabled={!onToggleEpisode}
                      >
                        {#if ep.watched}
                          <IconCheck size={16} stroke={3} />
                        {/if}
                      </button>

                      <span class="ep-num">#{ep.number}</span>

                      <div class="ep-main-details">
                        <div class="ep-header-line">
                          <h4 class="ep-title">{ep.title}</h4>
                          {#if ep.durationSeconds}
                            <span class="ep-duration"
                              >{Math.round(ep.durationSeconds / 60)} min</span
                            >
                          {/if}
                          {#if ep.airDate}
                            <span class="ep-air-date">{ep.airDate}</span>
                          {/if}
                        </div>
                        {#if ep.overview}
                          <p class="ep-overview">{ep.overview}</p>
                        {/if}
                      </div>

                      {#if ep.watchedAt}
                        <div class="ep-watched-pill">
                          Watched {new Date(ep.watchedAt).toLocaleDateString(
                            "en-IE",
                            { month: "short", day: "numeric" },
                          )}
                        </div>
                      {/if}

                      {#if ep.number > 1 && !ep.watched}
                        <button
                          type="button"
                          class="mark-prev-btn"
                          onclick={() =>
                            handleMarkPreviousEpisodesWatched(
                              selectedSeasonIndex,
                              ep.number,
                            )}
                          title={onToggleEpisode
                            ? `Mark episodes 1 to ${ep.number - 1} seen`
                            : "Episode changes are not available on this host"}
                          disabled={!onToggleEpisode}
                        >
                          Mark 1–{ep.number - 1} Seen
                        </button>
                      {/if}
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}

          <!-- Cast & Crew Section -->
          {#if record.cast && record.cast.length > 0}
            <h3 class="pane-heading mt-4">Top Billed Cast</h3>
            <div class="cast-grid">
              {#each record.cast as actor (actor.id)}
                <div class="cast-card">
                  <div class="cast-avatar">
                    {#if actor.profileUrl}
                      <img
                        src={actor.profileUrl}
                        alt={actor.name}
                        class="avatar-img"
                      />
                    {:else}
                      <div class="avatar-fallback">{actor.name.charAt(0)}</div>
                    {/if}
                  </div>
                  <div class="cast-text">
                    <h4 class="actor-name">{actor.name}</h4>
                    <p class="character-name">{actor.characterName}</p>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <!-- TAB 2: RYOT-STYLE ACTIONS DASHBOARD -->
      {:else if activeTab === "actions"}
        <section class="tab-pane">
          <h3 class="pane-heading">Media Action Center</h3>
          <p class="pane-sub">
            {hasRecordMutations
              ? "Use the changes supported by the active host."
              : "This host exposes this record as read-only. Each unavailable action is disabled."}
          </p>

          <div class="ryot-actions-grid">
            <button
              type="button"
              class="ryot-action-btn"
              disabled={!onUpdateProgress}
              title={onUpdateProgress
                ? "Update progress"
                : "Progress editing is not active on this host"}
              onclick={() => (showProgressModal = true)}
            >
              <div class="action-btn-icon"><IconAdjustments size={22} /></div>
              <div class="action-btn-text">
                <strong>Update Progress</strong>
                <span>Set current episode, timestamp, or percentage</span>
              </div>
            </button>

            <button
              type="button"
              class="ryot-action-btn"
              disabled={!onSaveReview}
              title={onSaveReview
                ? "Post a review"
                : "Personal ratings and reviews are not active on this host"}
              onclick={() => (showReviewModal = true)}
            >
              <div class="action-btn-icon"><IconMessage size={22} /></div>
              <div class="action-btn-text">
                <strong>Post a Review</strong>
                <span>Submit 10-star rating and personal critique</span>
              </div>
            </button>

            <button
              type="button"
              class="ryot-action-btn"
              disabled={!onSaveCollection}
              title={onSaveCollection
                ? "Add to collection"
                : "Collections are not active on this host"}
              onclick={() => (showCollectionModal = true)}
            >
              <div class="action-btn-icon"><IconFolderPlus size={22} /></div>
              <div class="action-btn-text">
                <strong>Add to Collection</strong>
                <span>Add to custom lists, favorites, or franchise</span>
              </div>
            </button>

            <button
              type="button"
              class="ryot-action-btn"
              disabled
              title="Occurrence logging is not available in this build"
            >
              <div class="action-btn-icon"><IconRepeat size={22} /></div>
              <div class="action-btn-text">
                <strong>Log Rewatch Occurrence</strong>
                <span>Record a new chronological consumption timestamp</span>
              </div>
            </button>
          </div>
        </section>

        <!-- TAB 3: OCCURRENCE & PROGRESS HISTORY -->
      {:else if activeTab === "history"}
        <section class="tab-pane">
          <h3 class="pane-heading">Chronicle History for this Entity</h3>
          <p class="pane-sub">
            {isSummary
              ? "History is not included in the Records summary."
              : `Every recorded occurrence available for ${record.title}.`}
          </p>

          {#if isSummary}
            <div class="empty-history-box">
              <IconClock size={32} class="empty-icon" />
              <h4>History is unavailable in this view</h4>
              <p>The active host returned only record summary fields.</p>
            </div>
          {:else if recordOccurrences.length === 0}
            <div class="empty-history-box">
              <IconClock size={32} class="empty-icon" />
              <h4>No occurrences recorded yet</h4>
              <p>
                When you watch or scrobble this title, every occurrence will be
                preserved here.
              </p>
              <button
                type="button"
                class="btn-primary"
                disabled
                title="Occurrence logging is not available in this build"
              >
                Log First Occurrence
              </button>
            </div>
          {:else}
            <div class="history-timeline">
              {#each recordOccurrences as occ (occ.id)}
                <div class="history-item-card">
                  <div class="history-left">
                    <span class="hist-time"
                      >{new Date(occ.timestamp).toLocaleDateString("en-IE", {
                        month: "short",
                        day: "numeric",
                        hour: "2-digit",
                        minute: "2-digit",
                      })}</span
                    >
                    <span class="hist-client">{occ.clientName}</span>
                  </div>
                  <div class="history-center">
                    <h4 class="hist-title">{occ.episodeTitle ?? occ.title}</h4>
                    <span class="hist-device"
                      ><IconDeviceTv size={14} /> {occ.deviceName}</span
                    >
                  </div>
                  <div class="history-right">
                    <span class="hist-dur">{occ.durationMinutes} min</span>
                    {#if occ.isRewatch}
                      <span class="rewatch-pill">Rewatch</span>
                    {/if}
                    {#if occ.userRating}
                      <span class="hist-star">★ {occ.userRating}/10</span>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <!-- TAB 4: SOURCES & PROVIDER IDENTITY (UAT ID-001–ID-030) -->
      {:else if activeTab === "sources"}
        <section class="tab-pane">
          <div class="provider-banner">
            <IconShieldCheck size={24} class="verified-icon" />
            <div>
              <h4 class="banner-title">
                Provider-Neutral Identity Architecture
              </h4>
              <p class="banner-desc">
                Fasti maintains a stable, immutable identity (<code
                  >{record.id}</code
                >) independent of TMDB, TVDB, or MyAnimeList. Provider claims
                remain evidence. Refresh appends a new provider claim; it does
                not replace the record identity.
              </p>
            </div>
          </div>

          <section
            class="metadata-projection"
            data-testid="metadata-projection"
            aria-labelledby="metadata-projection-title"
          >
            <div class="metadata-projection-heading">
              <div>
                <h3 id="metadata-projection-title">Metadata provenance</h3>
                <p>
                  The active profile selects display fields from immutable
                  claims. Cache state and attribution remain visible.
                </p>
              </div>
            </div>

            {#if metadataProjectionLoading}
              <p role="status">Loading metadata provenance…</p>
            {:else if metadataProjectionProblem}
              <div class="metadata-projection-problem" role="alert">
                <p>{metadataProjectionProblem}</p>
                {#if onRetryMetadataProjection}
                  <button
                    id="retry-metadata-projection"
                    type="button"
                    class="btn btn-outline-secondary"
                    onclick={onRetryMetadataProjection}
                  >
                    Retry projection read
                  </button>
                {/if}
              </div>
            {:else if metadataProjection}
              <div class="metadata-projection-summary">
                <span>Profile <code>{metadataProjection.profile_id}</code></span
                >
                <span
                  >{metadataProjection.fields.length.toLocaleString()} projected fields</span
                >
                <span
                  >{metadataProjection.ratings.length.toLocaleString()} rating claims</span
                >
              </div>

              <section
                class="metadata-evidence-section"
                data-testid="metadata-field-provenance"
                aria-labelledby="metadata-field-provenance-title"
              >
                <h4 id="metadata-field-provenance-title">Projected fields</h4>
                <!-- svelte-ignore a11y_no_noninteractive_tabindex (the overflow region must accept keyboard scrolling) -->
                <div
                  class="metadata-table-wrap table-responsive"
                  role="region"
                  tabindex="0"
                  aria-label="Projected field provenance"
                >
                  <table class="table table-vcenter">
                    <thead>
                      <tr>
                        <th scope="col">Field</th>
                        <th scope="col">Selected value</th>
                        <th scope="col">Selection</th>
                        <th scope="col">Provenance</th>
                      </tr>
                    </thead>
                    <tbody>
                      {#each metadataProjection.fields as field (field.field_key)}
                        <tr>
                          <th scope="row" class="mono">{field.field_key}</th>
                          <td class="metadata-value"
                            >{field.value ?? "No selected value"}</td
                          >
                          <td>
                            <span class="badge bg-blue-lt text-blue"
                              >{field.tier.replaceAll("_", " ")}</span
                            >
                            {#if field.is_stale}
                              <span class="badge bg-yellow-lt text-yellow"
                                >stale</span
                              >
                            {:else}
                              <span class="badge bg-green-lt text-green"
                                >fresh</span
                              >
                            {/if}
                            <span class="metadata-secondary"
                              >Projected {metadataTimestamp(
                                field.projected_at,
                              )}</span
                            >
                          </td>
                          <td>
                            {#if field.provenance}
                              <dl class="metadata-provenance-list">
                                <div>
                                  <dt>Source</dt>
                                  <dd>{field.provenance.source_namespace}</dd>
                                </div>
                                <div>
                                  <dt>Provider</dt>
                                  <dd>
                                    {field.provenance.provider_id ??
                                      "Not declared"}
                                  </dd>
                                </div>
                                <div>
                                  <dt>Fetched</dt>
                                  <dd>
                                    {metadataTimestamp(
                                      field.provenance.fetched_at,
                                    )}
                                  </dd>
                                </div>
                                <div>
                                  <dt>Claim state</dt>
                                  <dd>{field.provenance.status}</dd>
                                </div>
                              </dl>
                            {:else}
                              <span class="muted"
                                >No provider claim selected</span
                              >
                            {/if}
                          </td>
                        </tr>
                      {:else}
                        <tr>
                          <td colspan="4" class="muted"
                            >No projected metadata fields were returned.</td
                          >
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              </section>

              <section
                class="metadata-evidence-section"
                data-testid="metadata-rating-provenance"
                aria-labelledby="metadata-rating-provenance-title"
              >
                <h4 id="metadata-rating-provenance-title">Rating claims</h4>
                <div class="metadata-rating-list">
                  {#each metadataProjection.ratings as rating (rating.claim_id)}
                    <article>
                      <strong
                        >{ratingValue(
                          rating.value_millis,
                          rating.scale,
                        )}</strong
                      >
                      <span
                        >{rating.provenance.source_namespace} · {rating
                          .provenance.provider_id ??
                          "provider not declared"}</span
                      >
                      <span
                        >{rating.provenance.status} · fetched {metadataTimestamp(
                          rating.provenance.fetched_at,
                        )}</span
                      >
                    </article>
                  {:else}
                    <p class="muted">No rating claims were returned.</p>
                  {/each}
                </div>
              </section>

              <div class="metadata-state-grid">
                <section
                  class="metadata-evidence-section"
                  data-testid="metadata-cache-state"
                  aria-labelledby="metadata-cache-state-title"
                >
                  <h4 id="metadata-cache-state-title">Cache freshness</h4>
                  <ul class="metadata-state-list">
                    {#each metadataProjection.cache_entries as entry}
                      <li>
                        <strong
                          >{entry.key.field_group.replaceAll("_", " ")}</strong
                        >
                        <span>{entry.read_state.replaceAll("_", " ")}</span>
                        <span
                          >Fresh until {metadataTimestamp(
                            entry.fresh_until,
                          )}</span
                        >
                      </li>
                    {:else}
                      <li>No cache partitions were returned.</li>
                    {/each}
                  </ul>
                </section>

                <section
                  class="metadata-evidence-section"
                  data-testid="metadata-offline-state"
                  aria-labelledby="metadata-offline-state-title"
                >
                  <h4 id="metadata-offline-state-title">Offline read state</h4>
                  {#if offlineCacheEntries.length === 0}
                    <p>
                      Unavailable. This projection did not return an
                      offline-read cache partition.
                    </p>
                  {:else}
                    <ul class="metadata-state-list">
                      {#each offlineCacheEntries as entry}
                        <li>
                          <strong
                            >{entry.key.field_group.replaceAll(
                              "_",
                              " ",
                            )}</strong
                          >
                          <span>{entry.read_state.replaceAll("_", " ")}</span>
                          <span
                            >Stale-on-error limit {metadataTimestamp(
                              entry.stale_on_error_until,
                            )}</span
                          >
                        </li>
                      {/each}
                    </ul>
                    <p class="metadata-secondary">
                      These are server-reported cache states. This view did not
                      request offline fallback.
                    </p>
                  {/if}
                </section>
              </div>

              <section
                class="metadata-evidence-section"
                data-testid="metadata-attributions"
                aria-labelledby="metadata-attributions-title"
              >
                <h4 id="metadata-attributions-title">Attribution</h4>
                <ul class="metadata-attribution-list">
                  {#each metadataProjection.attributions as attribution (attribution.provider_id)}
                    <li>
                      <span>{attribution.text}</span>
                      <a
                        href={attribution.documentation_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        >Provider documentation <IconExternalLink
                          size={14}
                          aria-hidden="true"
                        /></a
                      >
                    </li>
                  {:else}
                    <li>No provider attribution was returned.</li>
                  {/each}
                </ul>
              </section>

              <div
                class="metadata-refresh-actions"
                data-testid="refresh-metadata-claims"
              >
                <h4>Refresh provider claims</h4>
                {#if metadataRefreshUnavailableFieldGroups.length > 0}
                  <div class="alert alert-info mb-0" role="status">
                    <strong>Some policy groups are not refreshed yet.</strong>
                    This refresh skips
                    {metadataRefreshUnavailableFieldGroups
                      .map((group) => group.replaceAll("_", " "))
                      .join(", ")}. The saved profile policy is unchanged.
                  </div>
                {/if}
                {#if projectionProviders.length === 0}
                  <p class="muted">
                    Choose a preferred provider in Settings before the first
                    refresh.
                  </p>
                {:else}
                  {#each projectionProviders as providerId}
                    <button
                      type="button"
                      class="btn btn-outline-secondary"
                      disabled={!onRefreshMetadataClaims ||
                        metadataRefreshableFieldGroupCount === 0 ||
                        Boolean(projectedMetadataRefreshingProvider)}
                      title={!onRefreshMetadataClaims
                        ? "Governed metadata refresh is unavailable on this host"
                        : metadataRefreshableFieldGroupCount === 0
                          ? "Enable at least one currently refreshable field group in Settings"
                          : undefined}
                      onclick={() => refreshProjectedMetadata(providerId)}
                    >
                      <IconRefresh size={16} aria-hidden="true" />
                      {projectedMetadataRefreshingProvider === providerId
                        ? `Refreshing ${providerLabel(providerId)}…`
                        : `Refresh ${providerLabel(providerId)}`}
                    </button>
                  {/each}
                {/if}
              </div>
            {:else}
              <p>
                Metadata provenance is unavailable because this host did not
                return a projection.
              </p>
            {/if}
          </section>

          <!-- svelte-ignore a11y_no_noninteractive_tabindex (the overflow region must accept keyboard scrolling) -->
          <div
            class="assertions-scroll"
            role="region"
            tabindex="0"
            aria-label="External identifiers"
          >
            <table class="assertions-table">
              <thead>
                <tr>
                  <th scope="col">Namespace</th>
                  <th scope="col">Identifier</th>
                  <th scope="col">Status</th>
                  <th scope="col">Provenance Route</th>
                  <th scope="col">Action</th>
                </tr>
              </thead>
              <tbody>
                {#each record.externalIds as xid}
                  {@const selection = refreshSelection(xid)}
                  <tr>
                    <td class="mono">{xid.namespace}</td>
                    <td class="mono"><strong>{xid.value}</strong></td>
                    <td
                      ><span class="status-pill matched"
                        >{xid.status.replaceAll("_", " ")}</span
                      ></td
                    >
                    <td>{xid.source}</td>
                    <td>
                      {#if selection && onApplyMetadata}
                        <button
                          type="button"
                          class="metadata-action"
                          disabled={Boolean(metadataRefreshingId)}
                          onclick={() => refreshMetadata(xid)}
                        >
                          <IconRefresh size={16} aria-hidden="true" />
                          {metadataRefreshingId ===
                          `${xid.namespace}:${xid.value}`
                            ? "Refreshing…"
                            : "Refresh"}
                        </button>
                      {:else}
                        <span class="muted">No live adapter</span>
                      {/if}
                    </td>
                  </tr>
                {:else}
                  <tr>
                    <td colspan="5" class="muted"
                      >No external identifiers are attached.</td
                    >
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>

          <section
            class="metadata-chooser"
            aria-labelledby="metadata-chooser-title"
          >
            <div>
              <h3 id="metadata-chooser-title">Choose metadata</h3>
              <p>
                Search a configured provider. The trusted host fetches the
                selected item again before it stores claims.
              </p>
            </div>

            {#key record.id}
              <DiscoverView
                embedded
                {providerCredentials}
                loading={providerLoading}
                hostProblem={providerHostProblem}
                onSearch={searchCompatibleMetadata}
                onOpenSettings={onOpenProviderSettings ?? (() => {})}
                onRetry={onRetryProviders ?? (() => {})}
                onCandidateAction={onApplyMetadata ? chooseMetadata : undefined}
                actionLabel="Use metadata"
                pendingLabel="Applying…"
                completedLabel="Metadata applied"
                actionProblemFallback="Fasti could not apply metadata to this record."
                actionUnavailableLabel="Metadata changes unavailable"
                actionUnavailableText="Search does not change this Record. Metadata changes require the trusted host to apply the selected provider claims."
              />
            {/key}

            {#if metadataNotice}
              <p class="metadata-notice" role="status">{metadataNotice}</p>
            {/if}
            {#if metadataProblem}
              <p class="metadata-problem" role="alert">{metadataProblem}</p>
            {/if}
            {#if record.externalIds.some((identifier) => providerMappingForIdentifier(identifier)?.provider === "tmdb") || (providerCredentials ?? []).some((provider) => provider.provider === "tmdb")}
              <TmdbAttribution />
            {/if}
          </section>
        </section>

        <!-- TAB 5: PERSONAL REVIEWS & NOTES -->
      {:else if activeTab === "reviews"}
        <section class="tab-pane">
          <div class="notes-header-row">
            <h3 class="pane-heading">Personal Reflections & Review</h3>
            {#if !isEditingNotes}
              <button
                type="button"
                class="edit-notes-btn"
                disabled={!onUpdateNotes}
                title={onUpdateNotes
                  ? "Edit notes"
                  : "Personal notes are not active on this host"}
                onclick={() => {
                  editedNotesText = record.userNotes ?? "";
                  isEditingNotes = true;
                }}
              >
                <IconEdit size={14} /> Edit Notes
              </button>
            {/if}
          </div>

          {#if isEditingNotes}
            <div class="notes-edit-box">
              <textarea
                bind:value={editedNotesText}
                class="notes-textarea"
                rows="6"
                placeholder="Write markdown reflections, favorite scenes, or analysis..."
              ></textarea>
              <div class="notes-actions">
                <button
                  type="button"
                  class="btn-primary"
                  onclick={handleSaveNotes}>Save Reflections</button
                >
                <button
                  type="button"
                  class="btn-secondary"
                  onclick={() => (isEditingNotes = false)}>Cancel</button
                >
              </div>
            </div>
          {:else}
            <div class="notes-display-box">
              {#if record.userNotes}
                <p class="notes-text">{record.userNotes}</p>
              {:else}
                <p class="empty-notes-hint">
                  {onUpdateNotes
                    ? "No personal notes are recorded. Select Edit Notes to add one."
                    : "No personal notes are available. This host exposes notes as read-only."}
                </p>
              {/if}
            </div>
          {/if}

          <!-- Tags Editor -->
          <h3 class="pane-heading mt-4">Personal Tags & Organizers</h3>
          <div class="tags-editor-row">
            {#each record.tags as tag}
              <span class="tag-chip">
                <span>{tag}</span>
                <button
                  type="button"
                  class="tag-delete-btn"
                  disabled={!onRemoveTag}
                  onclick={() => onRemoveTag?.(record.id, tag)}
                  aria-label="Remove tag {tag}"
                  title={onRemoveTag
                    ? `Remove tag ${tag}`
                    : "Tags are not editable on this host"}
                >
                  <IconX size={12} />
                </button>
              </span>
            {/each}

            <form onsubmit={handleAddTagSubmit} class="add-tag-form">
              <input
                type="text"
                disabled={!onAddTag}
                placeholder="+ Add tag..."
                bind:value={newTagInput}
                class="add-tag-input"
                aria-label="New tag name"
                title={onAddTag
                  ? "Add tag"
                  : "Tags are not editable on this host"}
              />
            </form>
          </div>
        </section>
      {/if}
    </section>
  </div>
</div>

<!-- Modal Dialogs -->
{#if showProgressModal && onUpdateProgress}
  <ProgressModal
    {record}
    onClose={() => (showProgressModal = false)}
    onSaveProgress={(recId, eps, sec, st) =>
      onUpdateProgress(recId, eps, sec, st)}
  />
{/if}

{#if showReviewModal && onSaveReview}
  <RatingReviewModal
    {record}
    onClose={() => (showReviewModal = false)}
    onSaveReview={(recId, r, n) => onSaveReview(recId, r, n)}
  />
{/if}

{#if showCollectionModal && onSaveCollection}
  <CollectionModal
    {record}
    collections={availableCollections}
    onClose={() => (showCollectionModal = false)}
    onSaveCollection={(recId, colls) => onSaveCollection(recId, colls)}
  />
{/if}

{#if showArtworkModal && onUpdatePoster}
  <ArtworkModal
    {record}
    candidates={candidatePosters}
    onClose={() => (showArtworkModal = false)}
    onSave={onUpdatePoster}
  />
{/if}

{#if contextMenuState}
  <ContextMenu
    x={contextMenuState.x}
    y={contextMenuState.y}
    items={contextMenuState.items}
    onClose={() => (contextMenuState = null)}
  />
{/if}

<style>
  .detail-container {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    max-width: 1200px;
    margin: 0 auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .top-nav-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .back-btn {
    display: inline-flex;
    align-items: center;
    min-height: 44px;
    gap: 6px;
    background: transparent;
    border: none;
    color: var(--fasti-text-primary);
    font-weight: 600;
    font-size: 0.92rem;
    cursor: pointer;
    padding: 6px 0;
  }
  .top-id-badge {
    min-width: 0;
    overflow-wrap: anywhere;
    text-align: right;
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }

  .media-main-header {
    display: grid;
    grid-template-columns: 240px 1fr;
    gap: 32px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(8px * var(--tblr-border-radius-scale, 1));
    padding: 28px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.04);
  }

  .poster-frame {
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    overflow: hidden;
    background: var(--fasti-surface-archive);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.12);
  }
  .main-poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .header-details-column {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .main-title {
    font-family: var(--fasti-font-display);
    font-size: 2.6rem;
    font-weight: 600;
    margin: 0;
    line-height: 1.1;
    color: var(--fasti-text-primary);
  }
  .original-title {
    font-family: var(--fasti-font-display);
    font-size: 1.25rem;
    font-style: italic;
    color: var(--fasti-text-muted);
    margin: -4px 0 0;
  }

  .meta-strip {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
  }
  .collection-pill {
    padding: 2px 8px;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
    background: color-mix(in srgb, var(--fasti-brand-gold) 20%, transparent);
    color: var(--fasti-brand-gold);
    font-weight: 700;
    font-size: 0.75rem;
  }

  .action-strip {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
    padding: 12px 0;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .score-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    background: #eab308;
    color: #1c1917;
    font-family: var(--fasti-font-mono);
    font-weight: 700;
  }
  .score-num {
    font-size: 1.05rem;
  }
  .score-meta {
    font-size: 0.72rem;
    font-weight: 500;
  }

  .user-rating-box {
    display: flex;
    align-items: center;
    background: var(--fasti-surface-archive);
    border: 1px solid var(--fasti-brand-gold);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    padding: 4px 8px;
  }
  .user-star {
    color: var(--fasti-brand-gold);
    margin-right: 4px;
  }
  .rating-select {
    border: none;
    background: transparent;
    font-family: var(--fasti-font-mono);
    font-size: 0.88rem;
    font-weight: 700;
    color: var(--fasti-text-primary);
    cursor: pointer;
  }

  .status-select {
    padding: 7px 12px;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-weight: 600;
    font-size: 0.88rem;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .status-select.watching {
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
  }
  .status-select.completed {
    background: var(--fasti-state-verified);
    color: var(--fasti-verified-contrast);
  }

  .icon-action-btn {
    width: var(--fasti-touch-target-min);
    height: var(--fasti-touch-target-min);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    background: var(--fasti-surface-archive);
    display: grid;
    place-items: center;
    color: var(--fasti-text-primary);
    cursor: pointer;
  }
  .icon-action-btn.active {
    color: #e11d48;
    border-color: #e11d48;
  }

  :is(button, select, input, textarea):disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .synopsis-prose {
    font-size: 0.95rem;
    line-height: 1.7;
    color: var(--fasti-text-primary);
    margin: 4px 0 0;
    max-width: 85ch;
  }

  .details-body-grid {
    display: grid;
    grid-template-columns: 280px 1fr;
    gap: 24px;
  }
  .sidebar-details-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    height: fit-content;
  }
  .details-sidebar-heading {
    font-family: var(--fasti-font-display);
    font-size: 1.3rem;
    margin: 0;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 8px;
  }
  .sidebar-meta-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin: 0;
  }
  .meta-pair dt {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }
  .meta-pair dd {
    font-size: 0.9rem;
    font-weight: 600;
    margin: 2px 0 0;
  }
  .sidebar-subheading {
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    margin: 0 0 8px;
  }
  .chips-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .genre-chip {
    padding: 3px 8px;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-archive);
    font-size: 0.8rem;
    font-weight: 500;
  }
  .studios-list {
    margin: 0;
    padding-left: 18px;
    font-size: 0.85rem;
    color: var(--fasti-text-primary);
  }
  .xid-links {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .xid-link {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    background: var(--fasti-surface-archive);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    color: var(--fasti-text-primary);
    text-decoration: none;
  }
  .xid-link:hover {
    color: var(--fasti-action-primary);
  }
  .empty-custom-fields-hint {
    font-size: 0.82rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .main-content-pane {
    min-width: 0;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    overflow: hidden;
  }
  .content-tabs {
    display: flex;
    background: var(--fasti-surface-archive);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    overflow-x: auto;
  }
  .tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 12px 18px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
    cursor: pointer;
    white-space: nowrap;
  }
  .tab-btn.active {
    color: var(--fasti-action-primary);
    border-bottom-color: var(--fasti-action-primary);
    background: var(--fasti-surface-paper);
  }
  .tab-pane {
    padding: 24px;
  }

  /* Ryot Actions Grid */
  .ryot-actions-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 14px;
    margin-top: 14px;
  }
  .ryot-action-btn {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 16px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    cursor: pointer;
    text-align: left;
    transition: all 120ms ease;
  }
  .ryot-action-btn:hover {
    border-color: var(--fasti-action-primary);
    background: color-mix(in srgb, var(--fasti-action-primary) 8%, transparent);
  }
  .action-btn-icon {
    width: 44px;
    height: 44px;
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
    display: grid;
    place-items: center;
    color: var(--fasti-action-primary);
  }
  .action-btn-text strong {
    display: block;
    font-size: 0.95rem;
    margin-bottom: 2px;
  }
  .action-btn-text span {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }

  /* History Timeline */
  .history-timeline {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 14px;
  }
  .history-item-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: var(--fasti-surface-archive);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
  }
  .history-left {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 140px;
  }
  .hist-time {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
    font-weight: 700;
  }
  .hist-client {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    color: var(--fasti-text-muted);
  }
  .history-center {
    flex: 1;
    margin: 0 16px;
  }
  .hist-title {
    margin: 0;
    font-size: 0.92rem;
  }
  .hist-device {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-top: 2px;
  }
  .history-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .hist-dur {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
    font-weight: 600;
  }
  .rewatch-pill {
    font-size: 0.7rem;
    padding: 2px 6px;
    background: var(--fasti-brand-gold);
    color: black;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
    font-weight: 700;
  }
  .hist-star {
    font-size: 0.8rem;
    color: var(--fasti-brand-gold);
    font-weight: 700;
  }

  .empty-history-box {
    text-align: center;
    padding: 40px 20px;
    background: var(--fasti-surface-archive);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
  }
  :global(.empty-icon) {
    color: var(--fasti-text-muted);
    margin-bottom: 8px;
  }

  .seasons-cards-row {
    display: flex;
    gap: 12px;
    margin-bottom: 24px;
    overflow-x: auto;
  }
  .season-card-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    cursor: pointer;
  }
  .season-card-btn.active {
    border-color: var(--fasti-action-primary);
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 10%,
      transparent
    );
  }
  .season-thumb {
    width: 36px;
    height: 52px;
    object-fit: cover;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
  }
  .season-name {
    font-weight: 700;
    font-size: 0.92rem;
    display: block;
  }
  .season-count {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }

  .deck-header-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 12px;
  }
  .season-bulk-actions {
    display: flex;
    gap: 8px;
  }
  .btn-secondary-sm {
    padding: 6px 12px;
    min-height: 44px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }

  .episodes-table-wrap {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .episode-item-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px;
    background: var(--fasti-surface-archive);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
  }
  .episode-item-row.watched {
    opacity: 0.75;
  }
  .ep-check-btn {
    width: var(--fasti-touch-target-min);
    height: var(--fasti-touch-target-min);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    border: 2px solid var(--fasti-text-muted);
    background: transparent;
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 0;
  }
  .ep-check-btn.checked {
    background: var(--fasti-state-verified);
    border-color: var(--fasti-state-verified);
    color: var(--fasti-verified-contrast);
  }
  .ep-num {
    font-family: var(--fasti-font-mono);
    font-weight: 700;
    color: var(--fasti-text-muted);
  }
  .ep-main-details {
    flex: 1;
  }
  .ep-header-line {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .ep-title {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
  }
  .ep-duration,
  .ep-air-date {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }
  .ep-overview {
    font-size: 0.82rem;
    color: var(--fasti-text-muted);
    margin: 4px 0 0;
  }
  .mark-prev-btn {
    flex-shrink: 0;
    padding: 5px 10px;
    min-height: 44px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-size: 0.72rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
    cursor: pointer;
    white-space: nowrap;
  }
  .mark-prev-btn:hover {
    color: var(--fasti-action-primary);
    border-color: var(--fasti-action-primary);
  }

  :is(.btn-secondary-sm, .mark-prev-btn):focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .cast-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 16px;
    margin-top: 14px;
  }
  .cast-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 12px;
    background: var(--fasti-surface-archive);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
  }
  .cast-avatar {
    width: 72px;
    height: 72px;
    border-radius: 50%;
    overflow: hidden;
    background: color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    margin-bottom: 8px;
  }
  .avatar-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .actor-name {
    font-size: 0.88rem;
    font-weight: 600;
    margin: 0 0 2px;
  }
  .character-name {
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .provider-banner {
    display: flex;
    gap: 14px;
    padding: 14px 18px;
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 12%,
      transparent
    );
    border: 1px solid
      color-mix(in srgb, var(--fasti-state-verified) 32%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    margin-bottom: 20px;
  }
  .provider-banner > div {
    min-width: 0;
  }
  .provider-banner code {
    overflow-wrap: anywhere;
  }
  .banner-title {
    font-size: 0.95rem;
    font-weight: 700;
    margin: 0 0 2px;
  }
  .banner-desc {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .metadata-projection {
    display: grid;
    gap: 24px;
    margin-block: 24px 32px;
    padding-block: 24px;
    border-block: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .metadata-projection-heading h3,
  .metadata-projection-heading p,
  .metadata-evidence-section h4,
  .metadata-refresh-actions h4 {
    margin: 0;
  }

  .metadata-projection-heading p,
  .metadata-secondary {
    color: var(--fasti-text-muted);
  }

  .metadata-projection-summary {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 20px;
    padding-block: 12px;
    border-block: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 18%, transparent);
  }

  .metadata-projection-summary code,
  .metadata-value,
  .metadata-provenance-list dd {
    overflow-wrap: anywhere;
  }

  .metadata-evidence-section {
    min-width: 0;
  }

  .metadata-evidence-section > h4 {
    margin-bottom: 10px;
  }

  .metadata-table-wrap {
    overflow-x: auto;
  }

  .metadata-table-wrap:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .metadata-value {
    max-width: 44ch;
    white-space: pre-wrap;
  }

  .metadata-secondary {
    display: block;
    margin-top: 6px;
    font-size: 0.78rem;
  }

  .metadata-provenance-list {
    display: grid;
    gap: 4px;
    margin: 0;
  }

  .metadata-provenance-list div {
    display: grid;
    grid-template-columns: minmax(70px, auto) minmax(0, 1fr);
    gap: 8px;
  }

  .metadata-provenance-list dt {
    color: var(--fasti-text-muted);
    font-weight: 600;
  }

  .metadata-provenance-list dd {
    margin: 0;
  }

  .metadata-rating-list,
  .metadata-state-list,
  .metadata-attribution-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .metadata-rating-list article,
  .metadata-state-list li,
  .metadata-attribution-list li {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    padding-block: 10px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .metadata-rating-list article span,
  .metadata-state-list li span {
    color: var(--fasti-text-muted);
  }

  .metadata-state-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr));
    gap: 28px;
  }

  .metadata-attribution-list a {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .metadata-refresh-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .metadata-refresh-actions h4 {
    flex-basis: 100%;
  }

  .metadata-projection-problem {
    color: var(--fasti-state-problem, #b42318);
  }

  .assertions-table {
    width: 100%;
    min-width: 720px;
    border-collapse: collapse;
    font-size: 0.88rem;
    text-align: left;
  }
  .assertions-scroll {
    overflow-x: auto;
  }

  .assertions-scroll:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }
  .assertions-table th,
  .assertions-table td {
    padding: 10px 14px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }
  .mono {
    font-family: var(--fasti-font-mono);
  }
  .status-pill.matched {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 15%,
      transparent
    );
    color: var(--fasti-state-verified);
  }
  .metadata-chooser {
    display: grid;
    gap: 16px;
    margin-top: 28px;
    padding-top: 24px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }
  .metadata-chooser h3,
  .metadata-chooser p {
    margin: 0;
  }
  .metadata-chooser > div > p,
  .muted {
    color: var(--fasti-text-muted);
  }
  .metadata-action {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .metadata-action {
    min-height: 44px;
    justify-content: center;
    padding: 8px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .metadata-action:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }
  .metadata-notice {
    color: var(--fasti-state-verified);
  }
  .metadata-problem {
    color: var(--fasti-state-problem, #b42318);
  }

  .notes-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  .edit-notes-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 6px 12px;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
  }
  .notes-textarea {
    width: 100%;
    padding: 12px;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    font-family: var(--fasti-font-body);
    font-size: 0.95rem;
    background: var(--fasti-surface-paper);
    box-sizing: border-box;
  }
  .notes-actions {
    display: flex;
    gap: 10px;
    margin-top: 10px;
  }
  .btn-primary {
    padding: 8px 16px;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    border: none;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-weight: 600;
    cursor: pointer;
  }
  .btn-secondary {
    padding: 8px 16px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    cursor: pointer;
  }
  .notes-display-box {
    padding: 18px;
    background: var(--fasti-surface-archive);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    font-size: 0.95rem;
    line-height: 1.6;
  }
  .tags-editor-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }
  .tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 20px;
    font-size: 0.85rem;
  }
  .tag-delete-btn {
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    display: grid;
    place-items: center;
    color: var(--fasti-text-muted);
  }
  .tag-delete-btn:hover {
    color: #e11d48;
  }
  .add-tag-input {
    height: 30px;
    padding: 4px 10px;
    border: 1px dashed
      color-mix(in srgb, var(--fasti-text-muted) 40%, transparent);
    border-radius: 20px;
    background: transparent;
    font-size: 0.85rem;
  }

  @media (max-width: 47.99rem) {
    .media-main-header,
    .details-body-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
