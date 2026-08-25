<script lang="ts">
  import type {
    MediaRecord,
    MediaKind,
    WatchStatus,
    ContextMenuItemConfig,
  } from "./types.js";
  import {
    IconSearch,
    IconLayoutGrid,
    IconList,
    IconStarFilled,
    IconCheck,
    IconPlayerPlay,
    IconBookmark,
    IconEye,
    IconEyeCheck,
    IconFolder,
    IconMessage,
    IconDotsVertical,
    IconAdjustments,
    IconRepeat,
  } from "@tabler/icons-svelte";
  import FastActionBar from "./fast-action-bar.svelte";
  import ProgressModal from "./progress-modal.svelte";
  import RatingReviewModal from "./rating-review-modal.svelte";
  import CollectionModal from "./collection-modal.svelte";
  import ContextMenu, { type ContextMenuItem } from "./context-menu.svelte";

  interface Props {
    records: MediaRecord[];
    availableCollections: string[];
    onSelectRecord: (recordId: string) => void;
    onUpdateStatus?: (recordId: string, status: WatchStatus) => void;
    onUpdateRating?: (recordId: string, rating: number) => void;
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
    records,
    availableCollections,
    onSelectRecord,
    onUpdateStatus,
    onUpdateRating,
    onUpdateProgress,
    onSaveReview,
    onSaveCollection,
    contextMenuConfigs,
  }: Props = $props();

  let selectedKind: MediaKind | "all" = $state("all");
  let selectedStatus: WatchStatus | "all" = $state("all");
  let searchQuery = $state("");
  let viewMode: "grid" | "list" = $state("grid");

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

  const filteredRecords = $derived(
    records.filter((rec) => {
      const matchKind =
        selectedKind === "all" || rec.mediaKind === selectedKind;
      const matchStatus =
        selectedStatus === "all" || rec.status === selectedStatus;
      const matchSearch =
        searchQuery.trim() === "" ||
        rec.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
        (rec.tags &&
          rec.tags.some((t) =>
            t.toLowerCase().includes(searchQuery.toLowerCase()),
          ));
      return matchKind && matchStatus && matchSearch;
    }),
  );

  const kinds: Array<{ id: MediaKind | "all"; label: string }> = [
    { id: "all", label: "All Items" },
    { id: "movie", label: "Movies" },
    { id: "show", label: "TV Shows" },
    { id: "anime", label: "Anime" },
    { id: "book", label: "Books" },
    { id: "game", label: "Games" },
  ];

  const statuses: Array<{ id: WatchStatus | "all"; label: string }> = [
    { id: "all", label: "All States" },
    { id: "watching", label: "Watching / In Progress" },
    { id: "completed", label: "Completed" },
    { id: "plan_to_watch", label: "Plan to Watch" },
  ];

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
        label: "Log Occurrence (Rewatch)",
        icon: IconRepeat,
        action: () => onUpdateStatus?.(rec.id, "completed"),
      },
      watchlist: {
        id: "watchlist",
        label:
          rec.status === "plan_to_watch"
            ? "Remove from Watchlist"
            : "Add to Watchlist",
        icon: IconBookmark,
        action: () => handleToggleWatchlist(rec),
      },
      manage_ids: {
        id: "manage_ids",
        label: `Copy Fasti ID (${rec.id})`,
        action: () => navigator.clipboard.writeText(rec.id),
      },
    };

    let items: ContextMenuItem[] = [];
    if (contextMenuConfigs && contextMenuConfigs.length > 0) {
      items = [...contextMenuConfigs]
        .filter((cfg) => cfg.visible && allItems[cfg.id])
        .sort((a, b) => a.order - b.order)
        .map((cfg) => allItems[cfg.id]);
    } else {
      items = Object.values(allItems);
    }

    contextMenuState = {
      x: e.clientX,
      y: e.clientY,
      items,
    };
  }
</script>

<div class="library-container">
  <header class="library-header">
    <div>
      <h1 class="view-title">Library</h1>
      <p class="view-subtitle">
        Your unified media collection across all providers and formats.
      </p>
    </div>

    <!-- View Mode Switcher -->
    <div class="view-controls">
      <button
        type="button"
        class="mode-btn"
        class:active={viewMode === "grid"}
        onclick={() => (viewMode = "grid")}
        aria-label="Grid view"
      >
        <IconLayoutGrid size={18} stroke={1.75} />
      </button>
      <button
        type="button"
        class="mode-btn"
        class:active={viewMode === "list"}
        onclick={() => (viewMode = "list")}
        aria-label="List view"
      >
        <IconList size={18} stroke={1.75} />
      </button>
    </div>
  </header>

  {#if records.length > 0}
    <!-- Filter & Search Toolbar -->
    <div class="toolbar">
      <div class="search-wrap">
        <IconSearch size={16} stroke={2} class="search-icon" />
        <input
          type="search"
          placeholder="Filter by title, tag, or creator..."
          bind:value={searchQuery}
          class="search-input"
          aria-label="Filter library records"
        />
      </div>

      <div
        class="filter-pills"
        role="radiogroup"
        aria-label="Media kind filter"
      >
        {#each kinds as k}
          <button
            type="button"
            class="filter-pill"
            class:active={selectedKind === k.id}
            onclick={() => (selectedKind = k.id)}
          >
            {k.label}
          </button>
        {/each}
      </div>

      <div class="filter-pills" role="radiogroup" aria-label="Status filter">
        {#each statuses as s}
          <button
            type="button"
            class="filter-pill status"
            class:active={selectedStatus === s.id}
            onclick={() => (selectedStatus = s.id)}
          >
            {s.label}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Records Grid / List -->
  {#if records.length === 0}
    <div class="empty-results">
      <p>No media records are available in this view.</p>
    </div>
  {:else if filteredRecords.length === 0}
    <div class="empty-results">
      <p>No media records match the selected filters.</p>
      <button
        type="button"
        class="reset-btn"
        onclick={() => {
          selectedKind = "all";
          selectedStatus = "all";
          searchQuery = "";
        }}
      >
        Reset Filters
      </button>
    </div>
  {:else if viewMode === "grid"}
    <div class="media-grid">
      {#each filteredRecords as rec (rec.id)}
        <div
          class="card-wrapper"
          role="group"
          aria-label="{rec.title} card"
          oncontextmenu={(e) => handleOpenContextMenu(rec, e)}
        >
          <button
            type="button"
            class="card-art-btn"
            onclick={() => onSelectRecord(rec.id)}
          >
            <div class="poster-box">
              {#if rec.posterUrl}
                <img
                  src={rec.posterUrl}
                  alt=""
                  class="poster-image"
                  loading="lazy"
                />
              {:else}
                <div class="fallback-poster">{rec.mediaKind}</div>
              {/if}

              <span class="kind-badge {rec.mediaKind}">{rec.mediaKind}</span>

              {#if rec.userRating}
                <div class="card-rating">
                  <IconStarFilled size={12} class="star-icon" />
                  <span>{rec.userRating}</span>
                </div>
              {/if}

              {#if rec.progressEpisodes !== undefined && rec.totalEpisodes !== undefined}
                <div class="episode-pill">
                  {rec.progressEpisodes}/{rec.totalEpisodes} eps
                </div>
              {/if}
            </div>
          </button>

          <!-- Ryot-Style Fast Buttons Toolbar -->
          <div class="fast-action-toolbar-wrap">
            <FastActionBar
              record={rec}
              onToggleWatched={handleToggleWatched}
              onToggleWatchlist={handleToggleWatchlist}
              onOpenCollection={handleOpenCollection}
              onOpenReview={handleOpenReview}
              onOpenContextMenu={handleOpenContextMenu}
            />
          </div>

          <div class="card-info">
            <button
              type="button"
              class="title-link"
              onclick={() => onSelectRecord(rec.id)}
            >
              <h3 class="card-title">{rec.title}</h3>
            </button>
            <div class="card-sub-row">
              <span class="card-year">{rec.releaseYear ?? "—"}</span>
              <span class="status-indicator {rec.status}"
                >{rec.status.replace("_", " ")}</span
              >
            </div>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <!-- List Table View -->
    <div class="table-frame">
      <table class="records-table">
        <thead>
          <tr>
            <th scope="col">Title</th>
            <th scope="col">Kind</th>
            <th scope="col">Status</th>
            <th scope="col">Rating</th>
            <th scope="col">Progress</th>
            <th scope="col">Quick Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredRecords as rec (rec.id)}
            <tr
              class="table-row"
              onclick={() => onSelectRecord(rec.id)}
              oncontextmenu={(e) => handleOpenContextMenu(rec, e)}
            >
              <td>
                <div class="row-title-box">
                  <strong>{rec.title}</strong>
                  {#if rec.releaseYear}
                    <span class="row-year">({rec.releaseYear})</span>
                  {/if}
                </div>
              </td>
              <td
                ><span class="kind-tag {rec.mediaKind}">{rec.mediaKind}</span
                ></td
              >
              <td
                ><span class="status-pill {rec.status}"
                  >{rec.status.replace("_", " ")}</span
                ></td
              >
              <td>
                {#if rec.userRating}
                  <span class="table-star">★ {rec.userRating}</span>
                {:else}
                  <span class="unrated">—</span>
                {/if}
              </td>
              <td>
                {#if rec.progressEpisodes !== undefined && rec.totalEpisodes !== undefined}
                  <span class="mono-prog"
                    >{rec.progressEpisodes}/{rec.totalEpisodes} eps</span
                  >
                {:else}
                  <span class="mono-prog">—</span>
                {/if}
              </td>
              <td onclick={(e) => e.stopPropagation()}>
                <div class="table-fast-btns">
                  <button
                    type="button"
                    class="table-btn"
                    class:active={rec.status === "completed"}
                    onclick={() => handleToggleWatched(rec)}
                    title="Toggle Seen"
                  >
                    {#if rec.status === "completed"}
                      <IconEyeCheck size={16} />
                    {:else}
                      <IconEye size={16} />
                    {/if}
                  </button>
                  <button
                    type="button"
                    class="table-btn"
                    class:active={rec.status === "plan_to_watch"}
                    onclick={() => handleToggleWatchlist(rec)}
                    title="Toggle Watchlist"
                  >
                    <IconBookmark size={16} />
                  </button>
                  <button
                    type="button"
                    class="table-btn"
                    onclick={() => handleOpenCollection(rec)}
                    title="Collection"
                  >
                    <IconFolder size={16} />
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
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
    collections={availableCollections}
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
  .library-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .library-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
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

  .view-controls {
    display: flex;
    gap: 4px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    padding: 2px;
  }

  .mode-btn {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border: none;
    background: transparent;
    border-radius: 4px;
    color: var(--fasti-text-muted);
    cursor: pointer;
  }

  .mode-btn.active {
    background: var(--fasti-brand-mark);
    color: white;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }

  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 260px;
    flex: 1;
  }

  :global(.search-icon) {
    position: absolute;
    left: 12px;
    color: var(--fasti-text-muted);
  }

  .search-input {
    width: 100%;
    height: 38px;
    padding: 8px 14px 8px 36px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 6px;
    font-size: 0.9rem;
    color: var(--fasti-text-primary);
  }

  .filter-pills {
    display: flex;
    gap: 6px;
  }

  .filter-pill {
    padding: 6px 12px;
    border-radius: 20px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    background: var(--fasti-surface-paper);
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--fasti-text-muted);
    cursor: pointer;
  }

  .filter-pill.active {
    background: var(--fasti-brand-mark);
    border-color: var(--fasti-brand-mark);
    color: white;
    font-weight: 600;
  }

  .empty-results {
    text-align: center;
    padding: 48px 24px;
    background: var(--fasti-surface-paper);
    border-radius: 8px;
    border: 1px dashed
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
  }

  .reset-btn {
    margin-top: 12px;
    padding: 8px 16px;
    background: var(--fasti-brand-mark);
    color: white;
    border: none;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
  }

  .media-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 20px;
  }

  .card-wrapper {
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

  .poster-box {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
    background: var(--fasti-surface-archive);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
    transition: transform 120ms ease;
  }

  .poster-box:hover {
    transform: translateY(-3px);
  }

  .poster-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .fallback-poster {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    font-family: var(--fasti-font-mono);
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }

  .kind-badge {
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

  .card-rating {
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

  .episode-pill {
    position: absolute;
    bottom: 8px;
    left: 8px;
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.8);
    color: white;
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

  .card-title {
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

  .title-link:hover .card-title {
    color: var(--fasti-action-primary);
  }

  .card-sub-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
    margin-top: 2px;
  }

  .status-indicator.watching {
    color: var(--fasti-action-primary);
    font-weight: 700;
  }
  .status-indicator.completed {
    color: var(--fasti-state-verified);
    font-weight: 700;
  }

  /* Table View */
  .table-frame {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    overflow: hidden;
  }

  .records-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
    text-align: left;
  }

  .records-table th,
  .records-table td {
    padding: 12px 16px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .table-row {
    cursor: pointer;
    transition: background 80ms ease;
  }

  .table-row:hover {
    background: var(--fasti-surface-archive);
  }

  .kind-tag {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 3px;
    background: var(--fasti-surface-archive);
  }

  .status-pill {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .status-pill.watching {
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 15%,
      transparent
    );
    color: var(--fasti-action-primary);
  }
  .status-pill.completed {
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 15%,
      transparent
    );
    color: var(--fasti-state-verified);
  }

  .table-fast-btns {
    display: flex;
    gap: 4px;
  }

  .table-btn {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    cursor: pointer;
    color: var(--fasti-text-muted);
  }

  .table-btn.active {
    color: var(--fasti-action-primary);
    border-color: var(--fasti-action-primary);
  }
</style>
