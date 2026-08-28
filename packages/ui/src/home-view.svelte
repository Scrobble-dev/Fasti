<script lang="ts">
  import type {
    MediaRecord,
    WatchStatus,
    ContextMenuItemConfig,
  } from "./types.js";
  import CollectionModal from "./collection-modal.svelte";
  import RatingReviewModal from "./rating-review-modal.svelte";
  import ProgressModal from "./progress-modal.svelte";
  import ContextMenu, { type ContextMenuItem } from "./context-menu.svelte";
  import PosterCard from "./poster-card.svelte";
  import {
    IconShieldCheck,
    IconChevronRight,
    IconAdjustments,
    IconEye,
    IconBookmark,
    IconFolderPlus,
    IconStar,
  } from "@tabler/icons-svelte";

  interface Props {
    records: MediaRecord[];
    availableCollections: string[];
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
    onViewAllSection?: (section: "in_progress" | "history" | "up_next") => void;
  }

  let {
    records,
    availableCollections,
    onSelectRecord,
    onUpdateStatus,
    onUpdateProgress,
    onSaveReview,
    onSaveCollection,
    contextMenuConfigs,
    onViewAllSection,
  }: Props = $props();

  let activeProgressRecord = $state<MediaRecord | null>(null);
  let activeRatingRecord = $state<MediaRecord | null>(null);
  let activeCollectionRecord = $state<MediaRecord | null>(null);
  let contextMenuVisible = $state(false);
  let contextMenuPos = $state({ x: 0, y: 0 });
  let contextMenuRecord = $state<MediaRecord | null>(null);

  const inProgressRecords = $derived(
    records
      .filter(
        (r) =>
          r.status === "watching" ||
          (r.progressEpisodes &&
            r.progressEpisodes > 0 &&
            r.status !== "completed"),
      )
      .slice(0, 6),
  );

  const recentlyRecorded = $derived(
    [...records]
      .filter((record) => Boolean(record.lastActivityAt))
      .sort((a, b) =>
        (b.lastActivityAt || "").localeCompare(a.lastActivityAt || ""),
      )
      .slice(0, 6),
  );

  const upNextRecords = $derived(
    records
      .filter(
        (r) =>
          r.status === "plan_to_watch" ||
          (r.status === "watching" &&
            (r.progressEpisodes ?? 0) < (r.totalEpisodes ?? 1)),
      )
      .slice(0, 6),
  );

  function handleToggleWatched(rec: MediaRecord) {
    if (!onUpdateStatus) return;
    const newStatus: WatchStatus =
      rec.status === "completed" ? "watching" : "completed";
    onUpdateStatus(rec.id, newStatus);
  }

  function handleToggleWatchlist(rec: MediaRecord) {
    if (!onUpdateStatus) return;
    const newStatus: WatchStatus =
      rec.status === "plan_to_watch" ? "watching" : "plan_to_watch";
    onUpdateStatus(rec.id, newStatus);
  }

  function handleOpenContextMenu(rec: MediaRecord, e: MouseEvent) {
    contextMenuRecord = rec;
    contextMenuPos = { x: e.clientX, y: e.clientY };
    contextMenuVisible = true;
  }

  const contextMenuItems = $derived.by<ContextMenuItem[]>(() => {
    if (!contextMenuRecord) return [];
    const r = contextMenuRecord;
    const allItems: Record<string, ContextMenuItem> = {
      view: {
        id: "view",
        label: "View details",
        icon: IconEye,
        action: () => onSelectRecord(r.id),
      },
      watched: {
        id: "watched",
        label: r.status === "completed" ? "Mark unplayed" : "Mark as seen",
        icon: IconEye,
        action: () => handleToggleWatched(r),
      },
      progress: {
        id: "progress",
        label: "Update progress & episodes",
        icon: IconAdjustments,
        action: () => (activeProgressRecord = r),
      },
      watchlist: {
        id: "watchlist",
        label:
          r.status === "plan_to_watch"
            ? "Remove from watchlist"
            : "Add to watchlist",
        icon: IconBookmark,
        action: () => handleToggleWatchlist(r),
      },
      collection: {
        id: "collection",
        label: "Add to collection...",
        icon: IconFolderPlus,
        action: () => (activeCollectionRecord = r),
      },
      review: {
        id: "review",
        label: "Rate & personal review...",
        icon: IconStar,
        action: () => (activeRatingRecord = r),
      },
      edit_tags: {
        id: "edit_tags",
        label: "Inspect tags & genres",
        icon: IconFolderPlus,
        action: () => onSelectRecord(r.id),
      },
      manage_ids: {
        id: "manage_ids",
        label: "Inspect external claims & IDs",
        icon: IconShieldCheck,
        action: () => onSelectRecord(r.id),
      },
      reconcile: {
        id: "reconcile",
        label: "Review identity evidence",
        icon: IconShieldCheck,
        action: () => onSelectRecord(r.id),
      },
    };

    if (!onUpdateStatus) {
      delete allItems.watched;
      delete allItems.watchlist;
    }
    if (!onUpdateProgress) delete allItems.progress;
    if (!onSaveCollection) delete allItems.collection;
    if (!onSaveReview) delete allItems.review;

    if (contextMenuConfigs && contextMenuConfigs.length > 0) {
      return [...contextMenuConfigs]
        .filter((cfg) => cfg.visible && allItems[cfg.id])
        .sort((a, b) => a.order - b.order)
        .map((cfg) => allItems[cfg.id]);
    }

    return Object.values(allItems);
  });
</script>

<div class="home-container">
  <header class="home-header">
    <h1>Overview</h1>
    <p>Review the records available from the active Fasti host.</p>
  </header>

  <!-- Section 1: In progress -->
  <section class="shelf-section">
    <div class="shelf-header">
      <div class="shelf-title-row">
        <h2 class="shelf-title">In progress</h2>
        <span class="count-pill">{inProgressRecords.length}</span>
      </div>
      <button
        type="button"
        class="view-all-btn"
        onclick={() => onViewAllSection?.("in_progress")}
        disabled={!onViewAllSection}
        title={onViewAllSection
          ? "View all records in progress"
          : "Section navigation is not available in this view"}
      >
        <span>View all</span>
        <IconChevronRight size={16} />
      </button>
    </div>

    <div class="cards-grid">
      {#each inProgressRecords as rec (rec.id)}
        <PosterCard
          record={rec}
          {onSelectRecord}
          onToggleWatched={onUpdateStatus ? handleToggleWatched : undefined}
          onToggleWatchlist={onUpdateStatus ? handleToggleWatchlist : undefined}
          onOpenCollection={onSaveCollection
            ? (r) => (activeCollectionRecord = r)
            : undefined}
          onOpenReview={onSaveReview
            ? (r) => (activeRatingRecord = r)
            : undefined}
          onOpenContextMenu={handleOpenContextMenu}
        />
      {/each}
    </div>
  </section>

  <!-- Section 2: Recently recorded -->
  <section class="shelf-section">
    <div class="shelf-header">
      <div class="shelf-title-row">
        <h2 class="shelf-title">Recently recorded</h2>
        <span class="count-pill">{recentlyRecorded.length}</span>
      </div>
      <button
        type="button"
        class="view-all-btn"
        onclick={() => onViewAllSection?.("history")}
        disabled={!onViewAllSection}
        title={onViewAllSection
          ? "View all recent records"
          : "Section navigation is not available in this view"}
      >
        <span>View all</span>
        <IconChevronRight size={16} />
      </button>
    </div>

    <div class="cards-grid">
      {#each recentlyRecorded as rec (rec.id)}
        <PosterCard
          record={rec}
          {onSelectRecord}
          onToggleWatched={onUpdateStatus ? handleToggleWatched : undefined}
          onToggleWatchlist={onUpdateStatus ? handleToggleWatchlist : undefined}
          onOpenCollection={onSaveCollection
            ? (r) => (activeCollectionRecord = r)
            : undefined}
          onOpenReview={onSaveReview
            ? (r) => (activeRatingRecord = r)
            : undefined}
          onOpenContextMenu={handleOpenContextMenu}
        />
      {/each}
    </div>
  </section>

  <!-- Section 3: Up next -->
  <section class="shelf-section">
    <div class="shelf-header">
      <div class="shelf-title-row">
        <h2 class="shelf-title">Up next</h2>
        <span class="count-pill">{upNextRecords.length}</span>
      </div>
      <button
        type="button"
        class="view-all-btn"
        onclick={() => onViewAllSection?.("up_next")}
        disabled={!onViewAllSection}
        title={onViewAllSection
          ? "View all records up next"
          : "Section navigation is not available in this view"}
      >
        <span>View all</span>
        <IconChevronRight size={16} />
      </button>
    </div>

    <div class="cards-grid">
      {#each upNextRecords as rec (rec.id)}
        <PosterCard
          record={rec}
          {onSelectRecord}
          onToggleWatched={onUpdateStatus ? handleToggleWatched : undefined}
          onToggleWatchlist={onUpdateStatus ? handleToggleWatchlist : undefined}
          onOpenCollection={onSaveCollection
            ? (r) => (activeCollectionRecord = r)
            : undefined}
          onOpenReview={onSaveReview
            ? (r) => (activeRatingRecord = r)
            : undefined}
          onOpenContextMenu={handleOpenContextMenu}
        />
      {/each}
    </div>
  </section>

  <!-- Modals & Context Menu -->
  {#if activeProgressRecord && onUpdateProgress}
    <ProgressModal
      record={activeProgressRecord}
      onClose={() => (activeProgressRecord = null)}
      onSaveProgress={(recId, ep, sec, st) => {
        onUpdateProgress(recId, ep, sec, st);
        activeProgressRecord = null;
      }}
    />
  {/if}

  {#if activeRatingRecord && onSaveReview}
    <RatingReviewModal
      record={activeRatingRecord}
      onClose={() => (activeRatingRecord = null)}
      onSaveReview={(recId, rating, notes) => {
        onSaveReview(recId, rating, notes);
        activeRatingRecord = null;
      }}
    />
  {/if}

  {#if activeCollectionRecord && onSaveCollection}
    <CollectionModal
      record={activeCollectionRecord}
      collections={availableCollections}
      onClose={() => (activeCollectionRecord = null)}
      onSaveCollection={(recId, collections) => {
        onSaveCollection(recId, collections);
        activeCollectionRecord = null;
      }}
    />
  {/if}

  {#if contextMenuVisible}
    <ContextMenu
      x={contextMenuPos.x}
      y={contextMenuPos.y}
      items={contextMenuItems}
      onClose={() => (contextMenuVisible = false)}
    />
  {/if}
</div>

<style>
  .home-container {
    padding: 24px 32px 64px;
    display: flex;
    flex-direction: column;
    gap: 36px;
    box-sizing: border-box;
  }

  .home-header h1,
  .home-header p {
    margin: 0;
  }

  .home-header h1 {
    font-family: var(--fasti-font-display);
  }

  .home-header p {
    margin-top: 4px;
    color: var(--fasti-text-muted);
  }

  .shelf-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .shelf-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .shelf-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .shelf-title {
    font-size: 1.25rem;
    font-weight: 700;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .count-pill {
    font-size: 0.8rem;
    font-weight: 600;
    padding: 2px 8px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-muted);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 12px;
  }

  .view-all-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    background: transparent;
    border: none;
    color: var(--fasti-text-muted);
    font-size: 0.86rem;
    font-weight: 600;
    cursor: pointer;
    min-height: 44px;
    padding: 4px 8px;
    border-radius: 4px;
    transition: all 120ms ease;
  }

  .view-all-btn:hover {
    color: var(--fasti-action-primary);
  }

  .view-all-btn:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 18px;
  }

  /* .poster-card and its descendants now live in poster-card.svelte -- see
   * that file for the card markup and styling used by all three shelves. */
</style>
