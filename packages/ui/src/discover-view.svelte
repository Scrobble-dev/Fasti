<script lang="ts">
  import type {
    ContextMenuItemConfig,
    MediaRecord,
    MediaKind,
    WatchStatus,
  } from "./types.js";
  import {
    IconCompass,
    IconSearch,
    IconStarFilled,
    IconPlus,
    IconCheck,
    IconFlame,
    IconAward,
    IconCalendarEvent,
    IconRefresh,
    IconAdjustments,
    IconMessage,
    IconFolder,
    IconRepeat,
    IconPlayerPlay,
  } from "@tabler/icons-svelte";
  import FastActionBar from "./fast-action-bar.svelte";
  import ProgressModal from "./progress-modal.svelte";
  import RatingReviewModal from "./rating-review-modal.svelte";
  import CollectionModal from "./collection-modal.svelte";
  import ContextMenu, { type ContextMenuItem } from "./context-menu.svelte";

  interface Props {
    trendingRecords: MediaRecord[];
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
    contextMenuConfigs?: ContextMenuItemConfig[];
  }

  let {
    trendingRecords,
    onSelectRecord,
    onUpdateStatus,
    onUpdateProgress,
    onSaveReview,
    onSaveCollection,
    contextMenuConfigs,
  }: Props = $props();

  let selectedCategory: MediaKind | "all" = $state("all");
  let searchQuery = $state("");

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
    const allItems: Record<string, ContextMenuItem> = {
      view: {
        id: "view",
        label: "View Details...",
        icon: IconPlayerPlay,
        action: () => onSelectRecord(rec.id),
      },
      progress: {
        id: "progress",
        label: "Update Progress...",
        icon: IconAdjustments,
        action: () => {
          activeModalRecord = rec;
          showProgressModal = true;
        },
      },
      review: {
        id: "review",
        label: "Post a Review...",
        icon: IconMessage,
        action: () => {
          activeModalRecord = rec;
          showReviewModal = true;
        },
      },
      collection: {
        id: "collection",
        label: "Add to Collection...",
        icon: IconFolder,
        action: () => {
          activeModalRecord = rec;
          showCollectionModal = true;
        },
      },
      watched: {
        id: "watched",
        label: rec.status === "completed" ? "Mark as unplayed" : "Mark as seen",
        icon: IconRepeat,
        action: () => handleToggleWatched(rec),
      },
      manage_ids: {
        id: "manage_ids",
        label: `Copy Fasti ID (${rec.id})`,
        action: () => navigator.clipboard.writeText(rec.id),
      },
    };
    const items = contextMenuConfigs?.length
      ? [...contextMenuConfigs]
          .filter((config) => config.visible && allItems[config.id])
          .sort((a, b) => a.order - b.order)
          .map((config) => allItems[config.id])
      : Object.values(allItems);
    contextMenuState = {
      x: e.clientX,
      y: e.clientY,
      items,
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
        Explore global trending media, top-rated classics, and upcoming
        broadcasts.
      </p>
    </div>

    <div class="header-actions">
      <button
        type="button"
        class="refresh-btn"
        aria-label="Refresh Recommendations"
        disabled
        title="Feed refresh is not available in this build"
      >
        <IconRefresh size={16} /> Refresh Feeds
      </button>
    </div>
  </header>

  <!-- Global Online + Library Search Bar -->
  <div class="search-section">
    <div class="search-bar-wrap">
      <IconSearch size={18} class="search-icon" />
      <input
        type="search"
        placeholder="Search titles online across TMDB, TVDB, AniList, MAL, OpenLibrary, RAWG..."
        bind:value={searchQuery}
        class="online-search-input"
        aria-label="Search online media"
      />
      {#if searchQuery.trim().length > 0}
        <span class="search-badge">Online Index Active</span>
      {/if}
    </div>

    <!-- Category Pill Filter -->
    <div
      class="categories-bar"
      role="radiogroup"
      aria-label="Filter discover by media kind"
    >
      {#each categories as cat}
        <button
          type="button"
          role="radio"
          aria-checked={selectedCategory === cat.id}
          class="cat-pill"
          class:active={selectedCategory === cat.id}
          onclick={() => (selectedCategory = cat.id)}
        >
          {cat.label}
        </button>
      {/each}
    </div>
  </div>

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

<!-- Modal Dialogs -->
{#if showProgressModal && activeModalRecord}
  <ProgressModal
    record={activeModalRecord}
    onClose={() => {
      showProgressModal = false;
      activeModalRecord = null;
    }}
    onSaveProgress={(recId, eps, sec, st) =>
      onUpdateProgress?.(recId, eps, sec, st)}
  />
{/if}

{#if showReviewModal && activeModalRecord}
  <RatingReviewModal
    record={activeModalRecord}
    onClose={() => {
      showReviewModal = false;
      activeModalRecord = null;
    }}
    onSaveReview={(recId, r, n) => onSaveReview?.(recId, r, n)}
  />
{/if}

{#if showCollectionModal && activeModalRecord}
  <CollectionModal
    record={activeModalRecord}
    onClose={() => {
      showCollectionModal = false;
      activeModalRecord = null;
    }}
    onSaveCollection={(recId, colls) => onSaveCollection?.(recId, colls)}
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
  .discover-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .discover-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
  }

  .heading-row {
    display: flex;
    align-items: center;
    gap: 10px;
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
    border-radius: 4px;
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
  }

  .search-section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .search-bar-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }
  :global(.search-icon) {
    position: absolute;
    left: 16px;
    color: var(--fasti-text-muted);
  }
  .online-search-input {
    width: 100%;
    height: 48px;
    padding: 12px 16px 12px 46px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-brand-mark) 40%, transparent);
    border-radius: 6px;
    font-size: 1rem;
    color: var(--fasti-text-primary);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.03);
  }
  .online-search-input:focus {
    outline: 2px solid var(--fasti-action-primary);
  }
  .search-badge {
    position: absolute;
    right: 14px;
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 4px;
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 15%,
      transparent
    );
    color: var(--fasti-state-verified);
  }

  .categories-bar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .cat-pill {
    padding: 6px 14px;
    border-radius: 20px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    background: var(--fasti-surface-paper);
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--fasti-text-muted);
    cursor: pointer;
    transition: all 120ms ease;
  }
  .cat-pill:hover {
    color: var(--fasti-text-primary);
    border-color: var(--fasti-text-primary);
  }
  .cat-pill.active {
    background: var(--fasti-brand-mark);
    border-color: var(--fasti-brand-mark);
    color: white;
    font-weight: 600;
  }

  .feed-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .section-header-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title-with-icon {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  :global(.trend-flame-icon) {
    color: #e11d48;
  }
  :global(.award-icon) {
    color: var(--fasti-brand-gold);
  }
  .section-title {
    font-family: var(--fasti-font-display);
    font-size: 1.45rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }
  .section-tagline {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
  }

  .media-carousel {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 18px;
  }
  .discover-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card-art-btn {
    background: transparent;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    display: block;
  }
  .poster-frame {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
    background: var(--fasti-surface-archive);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
    transition: transform 120ms ease;
  }
  .poster-frame:hover {
    transform: translateY(-4px);
  }
  .poster-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .poster-fallback {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    font-family: var(--fasti-font-mono);
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }
  .kind-chip {
    position: absolute;
    top: 8px;
    left: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.75);
    color: white;
  }
  .rating-chip {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    align-items: center;
    gap: 3px;
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.8);
    color: var(--fasti-brand-gold);
  }

  .fast-action-toolbar-wrap {
    margin-top: 2px;
  }

  .title-link {
    background: transparent;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
  }
  .item-title {
    font-family: var(--fasti-font-display);
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
    line-height: 1.2;
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
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
    margin-top: 2px;
  }
</style>
