<script lang="ts">
  import type { MediaRecord, MediaKind, WatchStatus } from "./types.js";
  import {
    IconSearch,
    IconLayoutGrid,
    IconList,
    IconStarFilled,
    IconCheck,
    IconPlayerPlay,
    IconBookmark,
  } from "@tabler/icons-svelte";

  interface Props {
    records: MediaRecord[];
    onSelectRecord: (recordId: string) => void;
  }

  let { records, onSelectRecord }: Props = $props();

  let selectedKind: MediaKind | "all" = $state("all");
  let selectedStatus: WatchStatus | "all" = $state("all");
  let searchQuery = $state("");
  let viewMode: "grid" | "list" = $state("grid");

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

    <div class="pill-group" role="radiogroup" aria-label="Filter by media kind">
      {#each kinds as kind}
        <button
          type="button"
          class="pill-btn"
          class:active={selectedKind === kind.id}
          onclick={() => (selectedKind = kind.id)}
        >
          {kind.label}
        </button>
      {/each}
    </div>

    <div
      class="pill-group"
      role="radiogroup"
      aria-label="Filter by watch status"
    >
      {#each statuses as status}
        <button
          type="button"
          class="pill-btn status-pill"
          class:active={selectedStatus === status.id}
          onclick={() => (selectedStatus = status.id)}
        >
          {status.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Records Grid / List -->
  {#if filteredRecords.length === 0}
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
        <button
          type="button"
          class="card-btn"
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

            {#if rec.progressEpisodes && rec.totalEpisodes}
              <div class="episode-pill">
                {rec.progressEpisodes}/{rec.totalEpisodes} eps
              </div>
            {/if}
          </div>

          <div class="card-info">
            <h3 class="card-title">{rec.title}</h3>
            <div class="card-sub-row">
              <span class="card-year">{rec.releaseYear ?? "—"}</span>
              <span class="card-source">{rec.displaySource}</span>
            </div>
          </div>
        </button>
      {/each}
    </div>
  {:else}
    <div class="table-wrap">
      <table class="media-table">
        <thead>
          <tr>
            <th scope="col">Title</th>
            <th scope="col">Type</th>
            <th scope="col">Year</th>
            <th scope="col">Status</th>
            <th scope="col">Progress</th>
            <th scope="col">Rating</th>
            <th scope="col">Source</th>
          </tr>
        </thead>
        <tbody>
          {#each filteredRecords as rec (rec.id)}
            <tr class="table-row" onclick={() => onSelectRecord(rec.id)}>
              <td class="td-title"><strong>{rec.title}</strong></td>
              <td
                ><span class="kind-tag {rec.mediaKind}">{rec.mediaKind}</span
                ></td
              >
              <td class="mono-cell">{rec.releaseYear ?? "—"}</td>
              <td
                ><span class="status-badge {rec.status}"
                  >{rec.status.replace("_", " ")}</span
                ></td
              >
              <td class="mono-cell">
                {#if rec.progressEpisodes && rec.totalEpisodes}
                  {rec.progressEpisodes} / {rec.totalEpisodes}
                {:else}
                  —
                {/if}
              </td>
              <td class="mono-cell rating-cell">
                {#if rec.userRating}
                  <IconStarFilled size={12} class="star-icon" />
                  {rec.userRating}/10
                {:else}
                  —
                {/if}
              </td>
              <td class="mono-cell text-muted">{rec.displaySource}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .library-container {
    max-width: 1100px;
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
    margin: 0 0 4px;
    color: var(--fasti-text-primary);
  }

  .view-subtitle {
    margin: 0;
    color: var(--fasti-text-muted);
    font-size: 0.95rem;
  }

  .view-controls {
    display: flex;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    overflow: hidden;
  }

  .mode-btn {
    background: var(--fasti-surface-paper);
    border: none;
    padding: 8px 12px;
    color: var(--fasti-text-muted);
    cursor: pointer;
    display: grid;
    place-items: center;
  }

  .mode-btn.active {
    background: var(--fasti-action-primary);
    color: white;
  }

  .toolbar {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  :global(.search-icon) {
    position: absolute;
    left: 14px;
    color: var(--fasti-text-muted);
  }

  .search-input {
    width: 100%;
    height: 44px;
    padding: 10px 14px 10px 42px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.95rem;
    color: var(--fasti-text-primary);
  }

  .search-input:focus {
    outline: 2px solid var(--fasti-action-primary);
  }

  .pill-group {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .pill-btn {
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 20px;
    background: var(--fasti-surface-paper);
    padding: 6px 14px;
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--fasti-text-muted);
    cursor: pointer;
    transition: all 120ms ease;
  }

  .pill-btn:hover {
    border-color: var(--fasti-text-primary);
    color: var(--fasti-text-primary);
  }

  .pill-btn.active {
    background: var(--fasti-brand-mark);
    border-color: var(--fasti-brand-mark);
    color: white;
    font-weight: 600;
  }

  .media-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 20px;
  }

  .card-btn {
    background: transparent;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    border-radius: 6px;
    overflow: hidden;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }

  .card-btn:hover {
    transform: translateY(-3px);
  }

  .card-btn:focus-visible {
    outline: 3px solid var(--fasti-brand-gold);
    outline-offset: 3px;
  }

  .poster-box {
    position: relative;
    width: 100%;
    aspect-ratio: 2 / 3;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
    overflow: hidden;
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.06);
  }

  .poster-image {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .fallback-poster {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    text-transform: uppercase;
    font-family: var(--fasti-font-mono);
    color: var(--fasti-text-muted);
  }

  .kind-badge {
    position: absolute;
    top: 8px;
    left: 8px;
    padding: 3px 6px;
    border-radius: 3px;
    font-family: var(--fasti-font-mono);
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
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
    padding: 3px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.75);
    color: var(--fasti-brand-gold);
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
  }

  .episode-pill {
    position: absolute;
    bottom: 8px;
    left: 8px;
    padding: 3px 8px;
    border-radius: 12px;
    background: var(--fasti-action-primary);
    color: white;
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    font-weight: 700;
  }

  .card-info {
    padding: 8px 2px;
  }

  .card-title {
    font-family: var(--fasti-font-display);
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0 0 2px;
    color: var(--fasti-text-primary);
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card-sub-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.78rem;
    font-family: var(--fasti-font-mono);
    color: var(--fasti-text-muted);
  }

  .table-wrap {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    overflow-x: auto;
  }

  .media-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.9rem;
    text-align: left;
  }

  .media-table th,
  .media-table td {
    padding: 12px 16px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .media-table th {
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    background: var(--fasti-surface-archive);
  }

  .table-row {
    cursor: pointer;
    transition: background 100ms ease;
  }

  .table-row:hover {
    background: color-mix(
      in srgb,
      var(--fasti-surface-archive) 60%,
      transparent
    );
  }

  .mono-cell {
    font-family: var(--fasti-font-mono);
  }

  .status-badge {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .status-badge.watching {
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 15%,
      transparent
    );
    color: var(--fasti-action-primary);
  }
  .status-badge.completed {
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 15%,
      transparent
    );
    color: var(--fasti-state-verified);
  }

  .empty-results {
    padding: 48px;
    text-align: center;
    background: var(--fasti-surface-paper);
    border-radius: 6px;
    border: 1px dashed
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    color: var(--fasti-text-muted);
  }

  .reset-btn {
    margin-top: 12px;
    background: var(--fasti-action-primary);
    color: white;
    border: none;
    border-radius: 4px;
    padding: 8px 16px;
    cursor: pointer;
    font-weight: 600;
  }
</style>
