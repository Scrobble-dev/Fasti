<script lang="ts">
  import type { MediaRecord } from "./types.js";
  import IconEye from "@tabler/icons-svelte/icons/eye";
  import IconEyeCheck from "@tabler/icons-svelte/icons/eye-check";
  import IconBookmark from "@tabler/icons-svelte/icons/bookmark";
  import IconBookmarkFilled from "@tabler/icons-svelte/icons/bookmark-filled";
  import IconFolder from "@tabler/icons-svelte/icons/folder";
  import IconMessage from "@tabler/icons-svelte/icons/message";
  import IconDotsVertical from "@tabler/icons-svelte/icons/dots-vertical";

  interface Props {
    record: MediaRecord;
    onToggleWatched?: (record: MediaRecord) => void;
    onToggleWatchlist?: (record: MediaRecord) => void;
    onOpenCollection?: (record: MediaRecord) => void;
    onOpenReview?: (record: MediaRecord) => void;
    onOpenContextMenu: (record: MediaRecord, event: MouseEvent) => void;
  }

  let {
    record,
    onToggleWatched,
    onToggleWatchlist,
    onOpenCollection,
    onOpenReview,
    onOpenContextMenu,
  }: Props = $props();

  const isWatched = $derived(record.status === "completed");
  const isWatchlist = $derived(record.status === "plan_to_watch");
</script>

<div
  class="btn-group fast-action-bar"
  role="toolbar"
  aria-label="Quick actions for {record.title}"
>
  <button
    type="button"
    class="btn btn-icon fast-btn"
    class:active={isWatched}
    disabled={!onToggleWatched}
    onclick={(e) => {
      e.stopPropagation();
      onToggleWatched?.(record);
    }}
    title={onToggleWatched
      ? isWatched
        ? "Marked as completed / seen"
        : "Mark as seen"
      : "Completion needs Chronicle progress history"}
    aria-label="Toggle watched"
    aria-pressed={isWatched}
  >
    {#if isWatched}
      <IconEyeCheck size={16} stroke={2.5} class="icon-watched" />
    {:else}
      <IconEye size={16} stroke={2} />
    {/if}
  </button>

  <!-- 2. Watchlist / Bookmark Fast Button -->
  <button
    type="button"
    class="btn btn-icon fast-btn"
    class:active={isWatchlist}
    disabled={!onToggleWatchlist}
    onclick={(e) => {
      e.stopPropagation();
      onToggleWatchlist?.(record);
    }}
    title={onToggleWatchlist
      ? isWatchlist
        ? "In your watchlist"
        : "Add to watchlist"
      : "Watchlist membership is not active on this host"}
    aria-label="Toggle watchlist"
    aria-pressed={isWatchlist}
  >
    {#if isWatchlist}
      <IconBookmarkFilled size={16} class="icon-bookmark-active" />
    {:else}
      <IconBookmark size={16} stroke={2} />
    {/if}
  </button>

  <!-- 3. Add to Collection Fast Button -->
  <button
    type="button"
    class="fast-btn"
    disabled={!onOpenCollection}
    onclick={(e) => {
      e.stopPropagation();
      onOpenCollection?.(record);
    }}
    title={onOpenCollection
      ? record.collectionName
        ? `In collection: ${record.collectionName}`
        : "Add to collection / lists"
      : "Collections are not active on this host"}
    aria-label="Add to collection"
  >
    <IconFolder size={16} stroke={2} />
  </button>

  <!-- 4. Review / Rating Fast Button -->
  <button
    type="button"
    class="fast-btn"
    disabled={!onOpenReview}
    onclick={(e) => {
      e.stopPropagation();
      onOpenReview?.(record);
    }}
    title={onOpenReview
      ? record.userRating
        ? `Rated ${record.userRating}/10`
        : "Add rating or review"
      : "Personal ratings and reviews are not active on this host"}
    aria-label="Add rating or review"
  >
    <IconMessage size={16} stroke={2} />
  </button>

  <!-- 5. Context Menu Trigger -->
  <button
    type="button"
    class="btn btn-icon fast-btn menu-dots"
    onclick={(e) => {
      e.stopPropagation();
      onOpenContextMenu(record, e);
    }}
    title="More actions"
    aria-label="More actions context menu"
  >
    <IconDotsVertical size={16} stroke={2} />
  </button>
</div>

<style>
  .fast-action-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-around;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    padding: 2px;
    gap: 2px;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.04);
  }

  .fast-btn {
    flex: 1 1 44px;
    min-width: 44px;
    min-height: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
    color: var(--fasti-text-muted);
    cursor: pointer;
    transition: all 100ms ease;
    padding: 2px;
  }

  .fast-btn:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .fast-btn:hover {
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .fast-btn:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .fast-btn:disabled:hover {
    background: transparent;
    color: var(--fasti-text-muted);
  }

  .fast-btn:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .fast-btn.active {
    color: var(--fasti-action-primary);
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 14%,
      transparent
    );
  }

  :global(.icon-watched) {
    color: var(--fasti-state-verified);
  }

  :global(.icon-bookmark-active) {
    color: var(--fasti-brand-mark);
  }

  @media (prefers-reduced-motion: reduce) {
    .fast-btn {
      transition: none;
    }
  }
</style>
