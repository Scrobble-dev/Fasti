<script lang="ts">
  import type {
    MediaRecord,
    MediaKind,
    WatchStatus,
    WorkbenchHost,
    ProviderSearchCandidate,
  } from "./types.js";
  import {
    IconCompass,
    IconSearch,
    IconStarFilled,
    IconPlus,
    IconCheck,
    IconFlame,
    IconAward,
    IconRefresh,
    IconAdjustments,
    IconMessage,
    IconFolder,
    IconRepeat,
    IconPlayerPlay,
    IconLoader2,
    IconBookmark,
    IconAlertCircle,
  } from "@tabler/icons-svelte";
  import FastActionBar from "./fast-action-bar.svelte";
  import ProgressModal from "./progress-modal.svelte";
  import RatingReviewModal from "./rating-review-modal.svelte";
  import CollectionModal from "./collection-modal.svelte";
  import ContextMenu, { type ContextMenuItem } from "./context-menu.svelte";

  interface Props {
    trendingRecords: MediaRecord[];
    libraryRecords?: MediaRecord[];
    host?: WorkbenchHost;
    onSelectRecord: (recordId: string) => void;
    onUpdateStatus?: (recordId: string, status: WatchStatus) => void;
    onUpdateProgress?: (
      recordId: string,
      episodes: number,
      seconds: number,
      status: WatchStatus,
    ) => void;
    onSaveReview?: (recordId: string, rating: number, notes: string) => void;
    onSaveCollection?: (recordId: string, collections: string[]) => void;
    onAddRecord?: (rec: MediaRecord, andOcc?: boolean) => void;
  }

  let {
    trendingRecords,
    libraryRecords = [],
    host,
    onSelectRecord,
    onUpdateStatus,
    onUpdateProgress,
    onSaveReview,
    onSaveCollection,
    onAddRecord,
  }: Props = $props();

  let selectedCategory: MediaKind | "all" = $state("all");
  let searchQuery = $state("");
  let isSearchingOnline = $state(false);
  let onlineResults = $state<ProviderSearchCandidate[]>([]);
  let searchError = $state<string | null>(null);
  let addedCandidateIds = $state<string[]>([]);

  // Modal Dialog States
  let activeModalRecord = $state<MediaRecord | null>(null);
  let showProgressModal = $state(false);
  let showReviewModal = $state(false);
  let showCollectionModal = $state(false);

  // Context Menu State
  let contextMenuState = $state<{
    x: number;
    y: number;
    items: ContextMenuItem[];
  } | null>(null);

  const categories: Array<{ id: MediaKind | "all"; label: string }> = [
    { id: "all", label: "All Media" },
    { id: "show", label: "TV Shows" },
    { id: "movie", label: "Movies" },
    { id: "anime", label: "Anime" },
    { id: "manga", label: "Manga" },
    { id: "game", label: "Games" },
    { id: "book", label: "Books" },
    { id: "comic", label: "Comics" },
    { id: "music", label: "Music" },
    { id: "podcast", label: "Podcasts" },
  ];

  let selectedProvider = $state<string>("auto");
  let searchTimeout: any;

  const localMatchingRecords = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (q.length < 2) return [];
    return libraryRecords.filter(
      (r) =>
        r.title.toLowerCase().includes(q) ||
        r.originalTitle?.toLowerCase().includes(q) ||
        r.genres?.some((g) => g.toLowerCase().includes(q)) ||
        r.tags?.some((t) => t.toLowerCase().includes(q)),
    );
  });

  function handleSearchInput(e: Event): void {
    const val = (e.target as HTMLInputElement).value;
    searchQuery = val;
    clearTimeout(searchTimeout);
    if (val.trim().length > 1) {
      searchTimeout = setTimeout(() => {
        void performOnlineSearch(val.trim());
      }, 350);
    } else {
      onlineResults = [];
      searchError = null;
    }
  }

  async function performOnlineSearch(query: string): Promise<void> {
    if (!host?.searchProvider) return;
    isSearchingOnline = true;
    searchError = null;
    try {
      let targetProvider = selectedProvider;
      if (selectedProvider === "auto") {
        if (selectedCategory === "book") targetProvider = "open-library";
        else if (selectedCategory === "anime" || selectedCategory === "manga")
          targetProvider = "kitsu";
        else if (selectedCategory === "game") targetProvider = "steam";
        else if (selectedCategory === "music") targetProvider = "musicbrainz";
        else if (selectedCategory === "show" || selectedCategory === "movie")
          targetProvider = "tmdb";
        else targetProvider = "auto";
      }
      const results = await host.searchProvider(targetProvider, query);
      onlineResults = results;
    } catch (err) {
      searchError = (err as Error).message;
    } finally {
      isSearchingOnline = false;
    }
  }

  function handleTrackCandidate(
    cand: ProviderSearchCandidate,
    status: WatchStatus,
  ): void {
    const newId = `rec_${Date.now()}_${Math.random().toString(36).substring(2, 6)}`;
    const rec: MediaRecord = {
      id: newId,
      title: cand.title,
      originalTitle: cand.original_title,
      mediaKind: (cand.kind as MediaKind) || "book",
      releaseYear: cand.release_year,
      posterUrl: cand.image_url ?? undefined,
      overview: cand.overview,
      status,
      displaySource: cand.provider,
      tags: ["Discovered"],
      genres: [],
      studios: cand.authors || [],
      externalIds: cand.external_ids || [
        {
          namespace: cand.provider,
          value: cand.provider_id,
          status: "matched",
          source: `${cand.provider}_search`,
        },
      ],
    };
    onAddRecord?.(rec, status === "completed");
    addedCandidateIds = [...addedCandidateIds, cand.provider_id];
  }

  const filteredTrending = $derived(
    trendingRecords.filter((rec) => {
      const matchCat =
        selectedCategory === "all" || rec.mediaKind === selectedCategory;
      const matchSearch =
        searchQuery.trim() === "" ||
        rec.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (rec.genres &&
          rec.genres.some((g) =>
            g.toLowerCase().includes(searchQuery.toLowerCase()),
          ));
      return matchCat && matchSearch;
    }),
  );

  function handleToggleWatched(rec: MediaRecord): void {
    const nextStatus: WatchStatus =
      rec.status === "completed" ? "watching" : "completed";
    onUpdateStatus?.(rec.id, nextStatus);
  }

  function handleToggleWatchlist(rec: MediaRecord): void {
    const nextStatus: WatchStatus =
      rec.status === "plan_to_watch" ? "watching" : "plan_to_watch";
    onUpdateStatus?.(rec.id, nextStatus);
  }

  function handleOpenCollection(rec: MediaRecord): void {
    activeModalRecord = rec;
    showCollectionModal = true;
  }

  function handleOpenReview(rec: MediaRecord): void {
    activeModalRecord = rec;
    showReviewModal = true;
  }

  function handleOpenContextMenu(rec: MediaRecord, e: MouseEvent): void {
    e.preventDefault();
    contextMenuState = {
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          id: "open",
          label: "View Details...",
          icon: IconPlayerPlay,
          action: () => onSelectRecord(rec.id),
        },
        {
          id: "prog",
          label: "Update Progress...",
          icon: IconAdjustments,
          action: () => {
            activeModalRecord = rec;
            showProgressModal = true;
          },
        },
        {
          id: "review",
          label: "Post a Review...",
          icon: IconMessage,
          action: () => {
            activeModalRecord = rec;
            showReviewModal = true;
          },
        },
        {
          id: "coll",
          label: "Add to Collection...",
          icon: IconFolder,
          action: () => {
            activeModalRecord = rec;
            showCollectionModal = true;
          },
        },
        {
          id: "rewatch",
          label: "Log Occurrence (Rewatch)",
          icon: IconRepeat,
          action: () => onUpdateStatus?.(rec.id, "completed"),
        },
        { id: "d1", label: "", divider: true, action: () => {} },
        {
          id: "copy_id",
          label: `Copy Fasti ID (${rec.id})`,
          action: () => navigator.clipboard.writeText(rec.id),
        },
      ],
    };
  }
</script>

<div class="discover-container">
  <header class="discover-header">
    <div>
      <div class="heading-row">
        <IconCompass size={28} class="discover-icon" />
        <h1 class="view-title">Discover & Explore</h1>
      </div>
      <p class="view-subtitle">
        Explore global trending media, top-rated classics, and live online
        metadata providers.
      </p>
    </div>

    <div class="header-actions">
      <button
        type="button"
        class="refresh-btn"
        aria-label="Refresh Recommendations"
        onclick={() => performOnlineSearch(searchQuery.trim() || "popular")}
      >
        <IconRefresh size={16} /> Refresh Feeds
      </button>
    </div>
  </header>

  <!-- Global Online + Library Search Bar -->
  <div class="search-section">
    <div class="search-bar-wrap">
      {#if isSearchingOnline}
        <IconLoader2 size={18} class="search-icon animate-spin" />
      {:else}
        <IconSearch size={18} class="search-icon" />
      {/if}
      <input
        type="search"
        placeholder="Search titles online across TMDB, Open Library, Kitsu, AniList, Steam, MusicBrainz..."
        value={searchQuery}
        oninput={handleSearchInput}
        class="online-search-input"
        aria-label="Search online media"
      />
      <select
        class="form-select form-select-sm provider-select-control"
        bind:value={selectedProvider}
        onchange={() => {
          if (searchQuery.trim().length > 1) {
            void performOnlineSearch(searchQuery.trim());
          }
        }}
        aria-label="Metadata Provider Selector"
      >
        <option value="auto">Auto / Multi-Provider</option>
        <option value="kitsu">Kitsu (Anime & Manga)</option>
        <option value="mal">MyAnimeList (MAL)</option>
        <option value="anilist">AniList (Anime/Manga)</option>
        <option value="tmdb">TheMovieDatabase (TMDB)</option>
        <option value="tvdb">TheTVDB v4 (TV & Series)</option>
        <option value="open-library">Open Library (Books)</option>
        <option value="google-books">Google Books</option>
        <option value="steam">Steam (Games)</option>
        <option value="rawg">RAWG (Video Games)</option>
        <option value="musicbrainz">MusicBrainz (Music)</option>
      </select>
    </div>

    {#if searchError}
      <div
        class="alert alert-warning d-flex align-items-center justify-content-between p-2 mt-2 rounded-2"
        role="alert"
      >
        <div class="d-flex align-items-center gap-2 small">
          <IconAlertCircle size={16} />
          <span>{searchError}</span>
        </div>
      </div>
    {/if}

    <!-- Category Pill Filter -->
    <div
      class="categories-bar"
      role="radiogroup"
      aria-label="Filter discover by media kind"
    >
      {#each categories as cat}
        <button
          type="button"
          class="cat-pill"
          class:active={selectedCategory === cat.id}
          onclick={() => {
            selectedCategory = cat.id;
            if (searchQuery.trim().length > 1) {
              void performOnlineSearch(searchQuery.trim());
            }
          }}
        >
          {cat.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- In-Library Matches (Local Cache First) -->
  {#if localMatchingRecords.length > 0 && searchQuery.trim().length > 1}
    <section class="feed-section" aria-label="Library Matches">
      <div class="section-header-row">
        <div class="title-with-icon">
          <IconCheck size={20} class="text-success" />
          <h2 class="section-title">
            In Your Library ({localMatchingRecords.length})
          </h2>
        </div>
        <span class="section-tagline">Locally cached and tracked items</span>
      </div>

      <div class="media-carousel mb-4">
        {#each localMatchingRecords as item (item.id)}
          <div class="discover-card" role="group" aria-label={item.title}>
            <div
              class="poster-box"
              role="button"
              tabindex="0"
              onclick={() => onSelectRecord(item.id)}
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === " ") onSelectRecord(item.id);
              }}
            >
              {#if item.posterUrl}
                <img
                  src={item.posterUrl}
                  alt={item.title}
                  class="poster-img"
                  loading="lazy"
                  referrerpolicy="no-referrer"
                  onerror={(e) => {
                    (e.currentTarget as HTMLElement).style.display = "none";
                  }}
                />
              {/if}
              <span class="top-badge kind font-monospace"
                >{item.mediaKind.toUpperCase()}</span
              >
              {#if item.userRating}
                <span class="top-badge rating font-monospace"
                  >★ {item.userRating}</span
                >
              {/if}
            </div>
            <div class="card-info">
              <button
                type="button"
                class="card-title text-truncate text-start p-0 bg-transparent border-0 w-100 fw-bold"
                onclick={() => onSelectRecord(item.id)}
              >
                {item.title}
              </button>
              {#if item.originalTitle && item.originalTitle !== item.title}
                <div
                  class="text-muted small fst-italic text-truncate"
                  style="font-size: 0.72rem;"
                >
                  {item.originalTitle}
                </div>
              {/if}
              <div class="card-sub-row">
                <span class="year-text">{item.releaseYear ?? "—"}</span>
                <span class="status-badge-mini text-success fw-bold"
                  >Tracked</span
                >
              </div>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Live Provider Search Results -->
  {#if onlineResults.length > 0}
    <section class="feed-section" aria-label="Online Search Results">
      <div class="section-header-row">
        <div class="title-with-icon">
          <IconSearch size={20} class="text-primary" />
          <h2 class="section-title">
            Live Provider Results ({onlineResults.length})
          </h2>
        </div>
        <span class="section-tagline"
          >Results fetched directly from online metadata providers</span
        >
      </div>

      <div class="online-results-grid">
        {#each onlineResults as cand (cand.provider + "_" + cand.provider_id)}
          <div class="online-card card">
            <div class="card-body p-3 d-flex gap-3">
              <div class="candidate-poster-wrap">
                {#if cand.image_url}
                  <img
                    src={cand.image_url}
                    alt=""
                    class="candidate-poster"
                    loading="lazy"
                    referrerpolicy="no-referrer"
                    onerror={(e) => {
                      (e.currentTarget as HTMLElement).style.display = "none";
                    }}
                  />
                {:else}
                  <div class="poster-fallback small">{cand.kind}</div>
                {/if}
              </div>
              <div class="candidate-info flex-grow-1">
                <div
                  class="d-flex align-items-start justify-content-between gap-2"
                >
                  <div>
                    <h3 class="candidate-title mb-0">{cand.title}</h3>
                    {#if cand.original_title && cand.original_title !== cand.title}
                      <div
                        class="text-muted small fst-italic mb-1"
                        style="font-size: 0.78rem;"
                      >
                        {cand.original_title}
                      </div>
                    {/if}
                  </div>
                  <span
                    class="badge bg-secondary-lt text-uppercase font-monospace flex-shrink-0"
                    >{cand.provider}</span
                  >
                </div>
                {#if cand.authors && cand.authors.length > 0}
                  <p class="candidate-author text-muted small mb-1">
                    {cand.authors.join(", ")}
                  </p>
                {/if}
                {#if cand.overview}
                  <p
                    class="candidate-overview small text-muted text-truncate-2 mb-2"
                  >
                    {cand.overview}
                  </p>
                {/if}
                <div class="candidate-actions d-flex align-items-center gap-2">
                  {#if addedCandidateIds.includes(cand.provider_id)}
                    <span
                      class="badge bg-success-lt d-flex align-items-center gap-1"
                    >
                      <IconCheck size={14} /> Added to Library
                    </span>
                  {:else}
                    <button
                      type="button"
                      class="btn btn-sm btn-primary d-flex align-items-center gap-1"
                      onclick={() => handleTrackCandidate(cand, "completed")}
                    >
                      <IconPlus size={14} /> Track Now
                    </button>
                    <button
                      type="button"
                      class="btn btn-sm btn-outline-secondary d-flex align-items-center gap-1"
                      onclick={() =>
                        handleTrackCandidate(cand, "plan_to_watch")}
                    >
                      <IconBookmark size={14} /> Plan to Watch
                    </button>
                  {/if}
                </div>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Row 1: Trending Right Now -->
  <section class="feed-section" aria-label="Trending Media">
    <div class="section-header-row">
      <div class="title-with-icon">
        <IconFlame size={20} class="trend-flame-icon" />
        <h2 class="section-title">Trending Right Now</h2>
      </div>
      <span class="section-tagline"
        >What media lovers are tracking and scrobbling this week</span
      >
    </div>

    <div class="media-carousel">
      {#each filteredTrending as item (item.id)}
        <div
          class="discover-card"
          role="group"
          aria-label="{item.title} card"
          oncontextmenu={(e) => handleOpenContextMenu(item, e)}
        >
          <button
            type="button"
            class="card-art-btn"
            onclick={() => onSelectRecord(item.id)}
          >
            <div class="poster-frame">
              {#if item.posterUrl}
                <img
                  src={item.posterUrl}
                  alt=""
                  class="poster-img"
                  loading="lazy"
                  referrerpolicy="no-referrer"
                />
              {:else}
                <div class="poster-fallback">{item.mediaKind}</div>
              {/if}

              <span class="kind-chip {item.mediaKind}">{item.mediaKind}</span>

              {#if item.communityRating}
                <div class="rating-chip">
                  <IconStarFilled size={12} class="star-icon" />
                  <span>{item.communityRating.score}</span>
                </div>
              {/if}
            </div>
          </button>

          <!-- Ryot-Style Fast Action Bar -->
          <div class="fast-action-toolbar-wrap">
            <FastActionBar
              record={item}
              onToggleWatched={handleToggleWatched}
              onToggleWatchlist={handleToggleWatchlist}
              onOpenCollection={handleOpenCollection}
              onOpenReview={handleOpenReview}
              onOpenContextMenu={handleOpenContextMenu}
            />
          </div>

          <div class="card-details">
            <button
              type="button"
              class="title-link"
              onclick={() => onSelectRecord(item.id)}
            >
              <h3 class="item-title">{item.title}</h3>
            </button>
            <div class="item-meta-row">
              <span class="item-year">{item.releaseYear ?? "—"}</span>
              <span class="item-format">{item.format ?? item.mediaKind}</span>
            </div>
          </div>
        </div>
      {/each}
    </div>
  </section>

  <!-- Row 2: All-Time Greats & Must-Reads -->
  <section class="feed-section" aria-label="All-Time Greats">
    <div class="section-header-row">
      <div class="title-with-icon">
        <IconAward size={20} class="award-icon" />
        <h2 class="section-title">All-Time Greats & Acclaimed Runs</h2>
      </div>
      <span class="section-tagline"
        >Essential classics with top-tier community acclaim</span
      >
    </div>

    <div class="media-carousel">
      {#each filteredTrending.slice().reverse() as item (item.id + "_great")}
        <div
          class="discover-card"
          role="group"
          aria-label="{item.title} card"
          oncontextmenu={(e) => handleOpenContextMenu(item, e)}
        >
          <button
            type="button"
            class="card-art-btn"
            onclick={() => onSelectRecord(item.id)}
          >
            <div class="poster-frame">
              {#if item.posterUrl}
                <img
                  src={item.posterUrl}
                  alt=""
                  class="poster-img"
                  loading="lazy"
                  referrerpolicy="no-referrer"
                />
              {:else}
                <div class="poster-fallback">{item.mediaKind}</div>
              {/if}
              <span class="kind-chip {item.mediaKind}">{item.mediaKind}</span>
              {#if item.communityRating}
                <div class="rating-chip">
                  <IconStarFilled size={12} class="star-icon" />
                  <span>{item.communityRating.score}</span>
                </div>
              {/if}
            </div>
          </button>

          <!-- Ryot-Style Fast Action Bar -->
          <div class="fast-action-toolbar-wrap">
            <FastActionBar
              record={item}
              onToggleWatched={handleToggleWatched}
              onToggleWatchlist={handleToggleWatchlist}
              onOpenCollection={handleOpenCollection}
              onOpenReview={handleOpenReview}
              onOpenContextMenu={handleOpenContextMenu}
            />
          </div>

          <div class="card-details">
            <button
              type="button"
              class="title-link"
              onclick={() => onSelectRecord(item.id)}
            >
              <h3 class="item-title">{item.title}</h3>
            </button>
            <div class="item-meta-row">
              <span class="item-year">{item.releaseYear ?? "—"}</span>
              <span class="item-format">{item.format ?? item.mediaKind}</span>
            </div>
          </div>
        </div>
      {/each}
    </div>
  </section>
</div>

<!-- Context Menu Portal -->
{#if contextMenuState}
  <ContextMenu
    x={contextMenuState.x}
    y={contextMenuState.y}
    items={contextMenuState.items}
    onClose={() => (contextMenuState = null)}
  />
{/if}

<!-- Modals -->
{#if showProgressModal && activeModalRecord}
  <ProgressModal
    record={activeModalRecord}
    onClose={() => {
      showProgressModal = false;
      activeModalRecord = null;
    }}
    onSaveProgress={(recordId, episodes, seconds, status) => {
      onUpdateProgress?.(recordId, episodes, seconds, status);
      showProgressModal = false;
      activeModalRecord = null;
    }}
  />
{/if}

{#if showReviewModal && activeModalRecord}
  <RatingReviewModal
    record={activeModalRecord}
    onClose={() => {
      showReviewModal = false;
      activeModalRecord = null;
    }}
    onSaveReview={(recordId, rating, notes) => {
      onSaveReview?.(recordId, rating, notes);
      showReviewModal = false;
      activeModalRecord = null;
    }}
  />
{/if}

{#if showCollectionModal && activeModalRecord}
  <CollectionModal
    record={activeModalRecord}
    onClose={() => {
      showCollectionModal = false;
      activeModalRecord = null;
    }}
    onSaveCollection={(recordId, collections) => {
      onSaveCollection?.(recordId, collections);
      showCollectionModal = false;
      activeModalRecord = null;
    }}
  />
{/if}

<style>
  .discover-container {
    max-width: 1360px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 36px;
  }

  .discover-header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
  }

  .heading-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  :global(.discover-icon) {
    color: var(--fasti-brand-mark);
  }

  .view-title {
    font-family: var(--fasti-font-display);
    font-size: 2.4rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .view-subtitle {
    margin: 4px 0 0;
    color: var(--fasti-text-muted);
    font-size: 0.95rem;
  }

  .refresh-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 6px;
    color: var(--fasti-text-primary);
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
  }

  .search-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .search-bar-wrap {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  :global(.search-bar-wrap .search-icon) {
    position: absolute;
    left: 16px;
    color: var(--fasti-text-muted);
  }

  .online-search-input {
    flex: 1;
    width: 100%;
    padding: 14px 16px 14px 44px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 8px;
    font-size: 0.95rem;
    color: var(--fasti-text-primary);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
  }

  .provider-select-control {
    width: auto;
    min-width: 180px;
    max-width: 230px;
    height: 48px;
    background-color: var(--fasti-surface-paper) !important;
    color: var(--fasti-text-primary) !important;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent) !important;
    border-radius: 8px;
    font-size: 0.84rem;
    font-weight: 600;
  }

  .search-badge {
    position: absolute;
    right: 14px;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    color: var(--fasti-state-verified);
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 12%,
      transparent
    );
    padding: 3px 8px;
    border-radius: 4px;
  }

  .categories-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .cat-pill {
    padding: 6px 14px;
    border-radius: 20px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    color: var(--fasti-text-muted);
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
  }

  .cat-pill:hover {
    color: var(--fasti-text-primary);
    border-color: var(--fasti-text-muted);
  }

  .cat-pill.active {
    background: var(--fasti-action-primary);
    border-color: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
  }

  .feed-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .section-header-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
    padding-bottom: 8px;
  }

  .title-with-icon {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  :global(.trend-flame-icon) {
    color: #e65100;
  }

  :global(.award-icon) {
    color: var(--fasti-brand-gold);
  }

  .section-title {
    font-family: var(--fasti-font-display);
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .section-tagline {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
  }

  .online-results-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 16px;
  }

  .online-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: 8px;
  }

  .candidate-poster-wrap {
    width: 64px;
    height: 96px;
    border-radius: 4px;
    overflow: hidden;
    flex-shrink: 0;
    background: var(--fasti-surface-archive);
  }

  .candidate-poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .candidate-title {
    font-size: 1rem;
    font-weight: 600;
    line-height: 1.25;
    color: var(--fasti-text-primary);
  }

  .text-truncate-2 {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .media-carousel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 20px;
  }

  .discover-card {
    display: flex;
    flex-direction: column;
    position: relative;
    background: var(--fasti-surface-paper);
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .card-art-btn {
    border: none;
    background: none;
    padding: 0;
    margin: 0;
    cursor: pointer;
    text-align: left;
    display: block;
    width: 100%;
  }

  .poster-frame {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    background: var(--fasti-surface-archive);
    overflow: hidden;
  }

  .poster-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .poster-fallback {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--fasti-font-mono);
    text-transform: uppercase;
    font-size: 0.8rem;
    color: var(--fasti-text-muted);
  }

  .poster-fallback.small {
    font-size: 0.65rem;
  }

  .kind-chip {
    position: absolute;
    top: 8px;
    left: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.65rem;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
  }

  .rating-chip {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    gap: 3px;
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.75);
    color: var(--fasti-brand-gold);
  }

  :global(.star-icon) {
    color: var(--fasti-brand-gold);
  }

  .fast-action-toolbar-wrap {
    padding: 4px 8px;
    background: var(--fasti-surface-paper);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 10%, transparent);
  }

  .card-details {
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .title-link {
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    text-align: left;
    cursor: pointer;
  }

  .item-title {
    font-size: 0.95rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title-link:hover .item-title {
    color: var(--fasti-action-primary);
  }

  .item-meta-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
  }
</style>
