<script lang="ts">
  import type {
    MediaRecord,
    WatchStatus,
    ContextMenuItemConfig,
  } from "./types.js";
  import FastActionBar from "./fast-action-bar.svelte";
  import CollectionModal from "./collection-modal.svelte";
  import RatingReviewModal from "./rating-review-modal.svelte";
  import ProgressModal from "./progress-modal.svelte";
  import ContextMenu, { type ContextMenuItem } from "./context-menu.svelte";
  import {
    IconDeviceTv,
    IconMovie,
    IconBook,
    IconDeviceGamepad2,
    IconMicrophone,
    IconChevronRight,
    IconEye,
    IconBookmark,
    IconFolderPlus,
    IconStar,
    IconPlayerPlay,
    IconAdjustments,
  } from "@tabler/icons-svelte";

  interface Props {
    records: MediaRecord[];
    onSelectRecord: (recordId: string) => void;
    onUpdateStatus: (recordId: string, status: WatchStatus) => void;
    onUpdateProgress: (
      recordId: string,
      episodes: number,
      seconds: number,
      status: WatchStatus,
    ) => void;
    onSaveReview: (recordId: string, rating: number, notes: string) => void;
    onSaveCollection: (recordId: string, collections: string[]) => void;
    contextMenuConfigs?: ContextMenuItemConfig[];
    onViewAllSection?: (section: string) => void;
  }

  let {
    records,
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
      .sort((a, b) =>
        (b.lastActivityAt || "").localeCompare(a.lastActivityAt || ""),
      )
      .slice(0, 6),
  );

  const upNextRecords = $derived(
    records
      .filter(
        (r) =>
          r.status === "watching" &&
          (r.progressEpisodes ?? 0) < (r.totalEpisodes ?? 1),
      )
      .slice(0, 6),
  );

  const watchlistRecords = $derived(
    records.filter((r) => r.status === "plan_to_watch").slice(0, 6),
  );

  function getKindIcon(kind: string) {
    switch (kind) {
      case "movie":
        return IconMovie;
      case "show":
        return IconDeviceTv;
      case "book":
      case "manga":
      case "comic":
        return IconBook;
      case "game":
        return IconDeviceGamepad2;
      case "podcast":
        return IconMicrophone;
      default:
        return IconDeviceTv;
    }
  }

  function calculateProgress(rec: MediaRecord): number {
    if (rec.status === "completed") return 100;
    if (rec.totalEpisodes && rec.progressEpisodes) {
      return Math.round((rec.progressEpisodes / rec.totalEpisodes) * 100);
    }
    if (rec.totalDurationSeconds && rec.progressSeconds) {
      return Math.round((rec.progressSeconds / rec.totalDurationSeconds) * 100);
    }
    return 0;
  }

  function handleToggleWatched(rec: MediaRecord) {
    const newStatus: WatchStatus =
      rec.status === "completed" ? "watching" : "completed";
    onUpdateStatus(rec.id, newStatus);
  }

  function handleToggleWatchlist(rec: MediaRecord) {
    const newStatus: WatchStatus =
      rec.status === "plan_to_watch" ? "dropped" : "plan_to_watch";
    onUpdateStatus(rec.id, newStatus);
  }

  function handleOpenContextMenu(rec: MediaRecord, e: MouseEvent) {
    e.preventDefault();
    contextMenuRecord = rec;
    contextMenuPos = { x: e.clientX, y: e.clientY };
    contextMenuVisible = true;
  }

  const contextMenuItems = $derived.by<ContextMenuItem[]>(() => {
    if (!contextMenuRecord) return [];
    const r = contextMenuRecord;
    return [
      {
        id: "h_act",
        label: "",
        header: "Playback & Actions",
        action: () => {},
      },
      {
        id: "view",
        label: "View Media Details",
        icon: IconPlayerPlay,
        action: () => onSelectRecord(r.id),
      },
      {
        id: "watched",
        label:
          r.status === "completed"
            ? "Mark as In Progress"
            : "Mark as Completed",
        icon: IconEye,
        action: () => handleToggleWatched(r),
      },
      {
        id: "progress",
        label: "Update Progress & Episodes",
        icon: IconAdjustments,
        action: () => (activeProgressRecord = r),
      },
      { id: "d1", label: "", divider: true, action: () => {} },
      { id: "h_lib", label: "", header: "Library & Lists", action: () => {} },
      {
        id: "watchlist",
        label:
          r.status === "plan_to_watch"
            ? "Remove from Watchlist"
            : "Add to Watchlist",
        icon: IconBookmark,
        action: () => handleToggleWatchlist(r),
      },
      {
        id: "collection",
        label: "Add to Collection...",
        icon: IconFolderPlus,
        action: () => (activeCollectionRecord = r),
      },
      {
        id: "review",
        label: "Rate & Personal Review...",
        icon: IconStar,
        action: () => (activeRatingRecord = r),
      },
      { id: "d2", label: "", divider: true, action: () => {} },
      {
        id: "h_id",
        label: "",
        header: "Identity & Metadata",
        action: () => {},
      },
      {
        id: "inspect_claims",
        label: "Inspect External Claim IDs",
        icon: IconPlayerPlay,
        action: () => onSelectRecord(r.id),
      },
      {
        id: "copy_id",
        label: `Copy ID (${r.id})`,
        action: () => navigator.clipboard.writeText(r.id),
      },
    ];
  });
</script>

<div class="home-container">
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
      >
        <span>View all</span>
        <IconChevronRight size={16} />
      </button>
    </div>

    <div class="cards-grid">
      {#each inProgressRecords as rec (rec.id)}
        {@const KindIcon = getKindIcon(rec.mediaKind)}
        {@const pct = calculateProgress(rec)}
        <div
          class="poster-card"
          tabindex="0"
          role="button"
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") onSelectRecord(rec.id);
          }}
          onclick={() => onSelectRecord(rec.id)}
          oncontextmenu={(e) => handleOpenContextMenu(rec, e)}
        >
          <div class="artwork-wrapper">
            {#if rec.posterUrl}
              <img
                src={rec.posterUrl}
                alt={rec.title}
                class="poster-img"
                loading="lazy"
                referrerpolicy="no-referrer"
              />
            {:else}
              <div class="typographic-fallback">
                <span class="fallback-title">{rec.title}</span>
                <span class="fallback-kind">{rec.mediaKind}</span>
              </div>
            {/if}

            <div class="top-badge top-left-badge" title={rec.mediaKind}>
              <KindIcon size={14} />
            </div>

            {#if pct > 0}
              <div class="progress-bar-track">
                <div class="progress-bar-fill" style="width: {pct}%"></div>
              </div>
            {/if}
          </div>

          <div class="card-info">
            <h3 class="media-title" title={rec.title}>{rec.title}</h3>
            <div class="meta-row">
              <span class="meta-kind">{rec.mediaKind.toUpperCase()}</span>
              {#if rec.releaseYear}
                <span class="meta-dot">•</span>
                <span>{rec.releaseYear}</span>
              {/if}
            </div>
            {#if pct > 0}
              <div class="pct-text">{pct}%</div>
            {/if}
          </div>

          <!-- Permanent 4-Action Footer Bar -->
          <div
            class="card-footer"
            role="presentation"
            onclick={(e) => e.stopPropagation()}
          >
            <FastActionBar
              record={rec}
              onToggleWatched={handleToggleWatched}
              onToggleWatchlist={handleToggleWatchlist}
              onOpenCollection={(r) => (activeCollectionRecord = r)}
              onOpenReview={(r) => (activeRatingRecord = r)}
              onOpenContextMenu={handleOpenContextMenu}
            />
          </div>
        </div>
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
      >
        <span>View all</span>
        <IconChevronRight size={16} />
      </button>
    </div>

    <div class="cards-grid">
      {#each recentlyRecorded as rec (rec.id)}
        {@const KindIcon = getKindIcon(rec.mediaKind)}
        {@const pct = calculateProgress(rec)}
        <div
          class="poster-card"
          tabindex="0"
          role="button"
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") onSelectRecord(rec.id);
          }}
          onclick={() => onSelectRecord(rec.id)}
          oncontextmenu={(e) => handleOpenContextMenu(rec, e)}
        >
          <div class="artwork-wrapper">
            {#if rec.posterUrl}
              <img
                src={rec.posterUrl}
                alt={rec.title}
                class="poster-img"
                loading="lazy"
                referrerpolicy="no-referrer"
              />
            {:else}
              <div class="typographic-fallback">
                <span class="fallback-title">{rec.title}</span>
                <span class="fallback-kind">{rec.mediaKind}</span>
              </div>
            {/if}

            <div class="top-badge top-left-badge" title={rec.mediaKind}>
              <KindIcon size={14} />
            </div>

            {#if pct > 0}
              <div class="progress-bar-track">
                <div class="progress-bar-fill" style="width: {pct}%"></div>
              </div>
            {/if}
          </div>

          <div class="card-info">
            <h3 class="media-title" title={rec.title}>{rec.title}</h3>
            <div class="meta-row">
              <span class="meta-kind">{rec.mediaKind.toUpperCase()}</span>
              {#if rec.releaseYear}
                <span class="meta-dot">•</span>
                <span>{rec.releaseYear}</span>
              {/if}
            </div>
            {#if pct > 0}
              <div class="pct-text">{pct}%</div>
            {/if}
          </div>

          <div
            class="card-footer"
            role="presentation"
            onclick={(e) => e.stopPropagation()}
          >
            <FastActionBar
              record={rec}
              onToggleWatched={handleToggleWatched}
              onToggleWatchlist={handleToggleWatchlist}
              onOpenCollection={(r) => (activeCollectionRecord = r)}
              onOpenReview={(r) => (activeRatingRecord = r)}
              onOpenContextMenu={handleOpenContextMenu}
            />
          </div>
        </div>
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
      >
        <span>View all</span>
        <IconChevronRight size={16} />
      </button>
    </div>

    <div class="cards-grid">
      {#each upNextRecords as rec (rec.id)}
        {@const KindIcon = getKindIcon(rec.mediaKind)}
        {@const pct = calculateProgress(rec)}
        <div
          class="poster-card"
          tabindex="0"
          role="button"
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") onSelectRecord(rec.id);
          }}
          onclick={() => onSelectRecord(rec.id)}
          oncontextmenu={(e) => handleOpenContextMenu(rec, e)}
        >
          <div class="artwork-wrapper">
            {#if rec.posterUrl}
              <img
                src={rec.posterUrl}
                alt={rec.title}
                class="poster-img"
                loading="lazy"
                referrerpolicy="no-referrer"
              />
            {:else}
              <div class="typographic-fallback">
                <span class="fallback-title">{rec.title}</span>
                <span class="fallback-kind">{rec.mediaKind}</span>
              </div>
            {/if}

            <div class="top-badge top-left-badge" title={rec.mediaKind}>
              <KindIcon size={14} />
            </div>

            {#if pct > 0}
              <div class="progress-bar-track">
                <div class="progress-bar-fill" style="width: {pct}%"></div>
              </div>
            {/if}
          </div>

          <div class="card-info">
            <h3 class="media-title" title={rec.title}>{rec.title}</h3>
            <div class="meta-row">
              <span class="meta-kind">{rec.mediaKind.toUpperCase()}</span>
              {#if rec.releaseYear}
                <span class="meta-dot">•</span>
                <span>{rec.releaseYear}</span>
              {/if}
            </div>
            {#if pct > 0}
              <div class="pct-text">{pct}%</div>
            {/if}
          </div>

          <div
            class="card-footer"
            role="presentation"
            onclick={(e) => e.stopPropagation()}
          >
            <FastActionBar
              record={rec}
              onToggleWatched={handleToggleWatched}
              onToggleWatchlist={handleToggleWatchlist}
              onOpenCollection={(r) => (activeCollectionRecord = r)}
              onOpenReview={(r) => (activeRatingRecord = r)}
              onOpenContextMenu={handleOpenContextMenu}
            />
          </div>
        </div>
      {/each}
    </div>
  </section>

  <!-- Section 4: Watchlist / Plan to Watch -->
  {#if watchlistRecords.length > 0}
    <section class="shelf-section">
      <div class="shelf-header">
        <div class="shelf-title-row">
          <h2 class="shelf-title">Watchlist & Backlog</h2>
          <span class="count-pill">{watchlistRecords.length}</span>
        </div>
        <button
          type="button"
          class="view-all-btn"
          onclick={() => onViewAllSection?.("library")}
        >
          <span>View full library</span>
          <IconChevronRight size={16} />
        </button>
      </div>

      <div class="cards-grid">
        {#each watchlistRecords as rec (rec.id)}
          {@const KindIcon = getKindIcon(rec.mediaKind)}
          <div
            class="poster-card"
            tabindex="0"
            role="button"
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") onSelectRecord(rec.id);
            }}
            onclick={() => onSelectRecord(rec.id)}
            oncontextmenu={(e) => handleOpenContextMenu(rec, e)}
          >
            <div class="artwork-wrapper">
              {#if rec.posterUrl}
                <img
                  src={rec.posterUrl}
                  alt={rec.title}
                  class="poster-img"
                  loading="lazy"
                  referrerpolicy="no-referrer"
                />
              {:else}
                <div class="typographic-fallback">
                  <span class="fallback-title">{rec.title}</span>
                  <span class="fallback-kind">{rec.mediaKind}</span>
                </div>
              {/if}

              <div class="top-badge top-left-badge" title={rec.mediaKind}>
                <KindIcon size={14} />
              </div>
            </div>

            <div class="card-info">
              <h3 class="media-title" title={rec.title}>{rec.title}</h3>
              <div class="meta-row">
                <span class="meta-kind">{rec.mediaKind.toUpperCase()}</span>
                {#if rec.releaseYear}
                  <span class="meta-dot">•</span>
                  <span>{rec.releaseYear}</span>
                {/if}
              </div>
            </div>

            <div
              class="card-footer"
              role="presentation"
              onclick={(e) => e.stopPropagation()}
            >
              <FastActionBar
                record={rec}
                onToggleWatched={handleToggleWatched}
                onToggleWatchlist={handleToggleWatchlist}
                onOpenCollection={(r) => (activeCollectionRecord = r)}
                onOpenReview={(r) => (activeRatingRecord = r)}
                onOpenContextMenu={handleOpenContextMenu}
              />
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Modals & Context Menu -->
  {#if activeProgressRecord}
    <ProgressModal
      record={activeProgressRecord}
      onClose={() => (activeProgressRecord = null)}
      onSaveProgress={(recId, ep, sec, st) => {
        onUpdateProgress(recId, ep, sec, st);
        activeProgressRecord = null;
      }}
    />
  {/if}

  {#if activeRatingRecord}
    <RatingReviewModal
      record={activeRatingRecord}
      onClose={() => (activeRatingRecord = null)}
      onSaveReview={(recId, rating, notes) => {
        onSaveReview(recId, rating, notes);
        activeRatingRecord = null;
      }}
    />
  {/if}

  {#if activeCollectionRecord}
    <CollectionModal
      record={activeCollectionRecord}
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
    padding: 16px 20px 36px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    box-sizing: border-box;
  }

  .shelf-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .shelf-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .shelf-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .shelf-title {
    font-size: 1.15rem;
    font-weight: 700;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .count-pill {
    font-size: 0.76rem;
    font-weight: 600;
    padding: 1px 7px;
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
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    padding: 3px 6px;
    border-radius: 4px;
    transition: all 120ms ease;
  }

  .view-all-btn:hover {
    color: var(--fasti-action-primary);
  }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 12px;
  }

  .poster-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: var(--tblr-border-radius, 6px);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    cursor: pointer;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }

  .poster-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.08);
  }

  .artwork-wrapper {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    background: var(--fasti-surface-night, #0f172a);
    overflow: hidden;
  }

  .poster-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .typographic-fallback {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 12px;
    text-align: center;
    background: var(--fasti-surface-archive);
  }

  .fallback-title {
    font-family: var(--fasti-font-display);
    font-weight: 600;
    font-size: 1rem;
    color: var(--fasti-text-primary);
  }

  .fallback-kind {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    margin-top: 4px;
  }

  .top-badge {
    position: absolute;
    top: 8px;
    width: 26px;
    height: 26px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.65);
    color: #ffffff;
    backdrop-filter: blur(4px);
    z-index: 2;
  }

  .top-left-badge {
    left: 8px;
  }

  .progress-bar-track {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: rgba(0, 0, 0, 0.4);
    z-index: 2;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--fasti-action-primary);
  }

  .card-info {
    padding: 6px 10px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
  }

  .media-title {
    font-size: 0.88rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta-row {
    font-size: 0.72rem;
    color: var(--fasti-text-muted);
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .meta-kind {
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .pct-text {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
    color: var(--fasti-action-primary);
    margin-top: 1px;
  }

  .card-footer {
    padding: 4px 6px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
    background: var(--fasti-surface-archive);
  }
</style>
