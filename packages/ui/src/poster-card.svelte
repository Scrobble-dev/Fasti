<script lang="ts">
  import type { MediaRecord } from "./types.js";
  import FastActionBar from "./fast-action-bar.svelte";
  import {
    IconDeviceTv,
    IconMovie,
    IconBook,
    IconDeviceGamepad2,
    IconMicrophone,
  } from "@tabler/icons-svelte";

  interface Props {
    record: MediaRecord;
    onSelectRecord: (recordId: string) => void;
    onToggleWatched?: (rec: MediaRecord) => void;
    onToggleWatchlist?: (rec: MediaRecord) => void;
    onOpenCollection?: (rec: MediaRecord) => void;
    onOpenReview?: (rec: MediaRecord) => void;
    onOpenContextMenu: (rec: MediaRecord, e: MouseEvent) => void;
  }

  let {
    record,
    onSelectRecord,
    onToggleWatched,
    onToggleWatchlist,
    onOpenCollection,
    onOpenReview,
    onOpenContextMenu,
  }: Props = $props();

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

  const KindIcon = $derived(getKindIcon(record.mediaKind));
  const pct = $derived(calculateProgress(record));
</script>

<div class="poster-card" role="group" aria-label="{record.title} card">
  <button
    type="button"
    class="artwork-wrapper"
    onclick={() => onSelectRecord(record.id)}
  >
    {#if record.posterUrl}
      <img
        src={record.posterUrl}
        alt={record.title}
        class="poster-img"
        loading="lazy"
      />
    {:else}
      <div class="typographic-fallback">
        <span class="fallback-title">{record.title}</span>
        <span class="fallback-kind">{record.mediaKind}</span>
      </div>
    {/if}

    <div class="top-badge top-left-badge" title={record.mediaKind}>
      <KindIcon size={14} />
    </div>

    {#if pct > 0}
      <div class="top-badge top-right-badge" title="{pct}% completed">
        <span class="badge-pct">{pct}%</span>
      </div>
      <div class="progress-bar-track">
        <div class="progress-bar-fill" style="width: {pct}%"></div>
      </div>
    {/if}
  </button>

  <div class="card-info">
    <h3 class="media-title" title={record.title}>{record.title}</h3>
    <div class="meta-row">
      <span class="meta-kind">{record.mediaKind.toUpperCase()}</span>
      {#if record.releaseYear}
        <span class="meta-dot">•</span>
        <span>{record.releaseYear}</span>
      {/if}
    </div>
    {#if record.statusText}
      <div class="next-ep-text" title={record.statusText}>
        {record.statusText}
      </div>
    {:else if record.totalEpisodes && record.progressEpisodes}
      <div class="next-ep-text">
        Ep {record.progressEpisodes} of {record.totalEpisodes}
      </div>
    {/if}
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
      {record}
      {onToggleWatched}
      {onToggleWatchlist}
      {onOpenCollection}
      {onOpenReview}
      onOpenContextMenu={(r, e) => onOpenContextMenu(r, e)}
    />
  </div>
</div>

<style>
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
    padding: 0;
    border: 0;
    color: inherit;
    cursor: pointer;
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
    color: var(--fasti-overlay-contrast);
    backdrop-filter: blur(4px);
    z-index: 2;
  }

  .top-left-badge {
    left: 8px;
  }

  .top-right-badge {
    right: 8px;
    width: auto;
    min-width: 34px;
    padding: 0 6px;
  }

  .badge-pct {
    font-family: var(--fasti-font-mono);
    font-size: 0.68rem;
    font-weight: 700;
    color: var(--fasti-overlay-contrast);
    letter-spacing: -0.02em;
  }

  .next-ep-text {
    font-size: 0.72rem;
    color: var(--fasti-text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: 1px;
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
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }

  .media-title {
    font-size: 0.9rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta-row {
    font-size: 0.75rem;
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
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--fasti-action-primary);
    margin-top: 2px;
  }

  .card-footer {
    padding: 8px 10px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
    background: var(--fasti-surface-archive);
  }
</style>
