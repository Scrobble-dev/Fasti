<script lang="ts">
  import type { MediaRecord, WatchStatus } from "./types.js";
  import {
    IconArrowLeft,
    IconStarFilled,
    IconPlayerPlay,
    IconCheck,
    IconBookmark,
    IconExternalLink,
    IconSparkles,
    IconInfoCircle,
    IconNote,
    IconListNumbers,
    IconShieldCheck,
  } from "@tabler/icons-svelte";

  interface Props {
    record: MediaRecord;
    onBack: () => void;
    onUpdateStatus?: (recordId: string, status: WatchStatus) => void;
    onUpdateRating?: (recordId: string, rating: number) => void;
    onToggleEpisode?: (recordId: string, episodeId: string) => void;
  }

  let {
    record,
    onBack,
    onUpdateStatus,
    onUpdateRating,
    onToggleEpisode,
  }: Props = $props();

  let activeTab: "overview" | "sources" | "episodes" | "notes" =
    $state("overview");
  let selectedSeasonIndex = $state(0);
</script>

<div class="detail-container">
  <button type="button" class="back-btn" onclick={onBack}>
    <IconArrowLeft size={16} stroke={2} />
    <span>Back to Library</span>
  </button>

  <!-- Hero Backdrop & Header -->
  <header
    class="detail-hero"
    style={record.backdropUrl
      ? `background-image: linear-gradient(to bottom, rgba(17,17,15,0.7), var(--fasti-surface-archive)), url(${record.backdropUrl});`
      : ""}
  >
    <div class="hero-content">
      <div class="poster-container">
        {#if record.posterUrl}
          <img
            src={record.posterUrl}
            alt="{record.title} poster"
            class="detail-poster"
          />
        {:else}
          <div class="detail-poster-fallback">{record.mediaKind}</div>
        {/if}
      </div>

      <div class="hero-meta">
        <div class="tag-row">
          <span class="kind-chip {record.mediaKind}">{record.mediaKind}</span>
          <span class="status-chip {record.status}"
            >{record.status.replace("_", " ")}</span
          >
          <span class="year-chip">{record.releaseYear ?? "—"}</span>
        </div>

        <h1 class="record-title">{record.title}</h1>
        {#if record.originalTitle}
          <h2 class="original-title">{record.originalTitle}</h2>
        {/if}

        <div class="user-action-bar">
          <div
            class="rating-picker"
            role="radiogroup"
            aria-label="Rate out of 10"
          >
            {#each [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as star}
              <button
                type="button"
                class="star-btn"
                class:active={(record.userRating ?? 0) >= star}
                onclick={() => onUpdateRating?.(record.id, star)}
                aria-label="{star} stars"
              >
                <IconStarFilled size={18} />
              </button>
            {/each}
            <span class="rating-value"
              >{record.userRating ? `${record.userRating}/10` : "Unrated"}</span
            >
          </div>

          <button type="button" class="action-btn play-with-btn">
            <IconPlayerPlay size={16} stroke={2} /> Play With...
          </button>
        </div>
      </div>
    </div>
  </header>

  <!-- Sub Navigation Tabs -->
  <nav class="detail-tabs" aria-label="Media details navigation">
    <button
      type="button"
      class="tab-btn"
      class:active={activeTab === "overview"}
      onclick={() => (activeTab = "overview")}
    >
      <IconInfoCircle size={16} /> Overview
    </button>
    <button
      type="button"
      class="tab-btn"
      class:active={activeTab === "sources"}
      onclick={() => (activeTab = "sources")}
    >
      <IconShieldCheck size={16} /> Sources & Identity ({record.externalIds
        .length})
    </button>
    {#if record.seasons && record.seasons.length > 0}
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === "episodes"}
        onclick={() => (activeTab = "episodes")}
      >
        <IconListNumbers size={16} /> Episodes
      </button>
    {/if}
    <button
      type="button"
      class="tab-btn"
      class:active={activeTab === "notes"}
      onclick={() => (activeTab = "notes")}
    >
      <IconNote size={16} /> Notes & Tags
    </button>
  </nav>

  <!-- Tab Contents -->
  <main class="tab-body">
    {#if activeTab === "overview"}
      <section class="overview-section">
        <h3 class="section-title">Synopsis</h3>
        <p class="overview-text">
          {record.overview ?? "No overview available for this record."}
        </p>

        <h3 class="section-title">Tags & Genres</h3>
        <div class="genre-tags">
          {#each record.tags as tag}
            <span class="genre-tag">{tag}</span>
          {/each}
        </div>

        <h3 class="section-title">Fasti Chronicle ID</h3>
        <code class="code-id">{record.id}</code>
      </section>
    {:else if activeTab === "sources"}
      <section class="sources-section">
        <div class="banner-note">
          <IconShieldCheck size={20} class="verified-icon" />
          <p>
            <strong>Provider-Neutral Identity:</strong> Fasti maintains one stable
            record. Switching display providers changes metadata projection without
            rewriting your Chronicle history.
          </p>
        </div>

        <h3 class="section-title">External Identifier Assertions</h3>
        <table class="id-table">
          <thead>
            <tr>
              <th scope="col">Namespace</th>
              <th scope="col">Identifier</th>
              <th scope="col">Acquisition Route</th>
              <th scope="col">Status</th>
            </tr>
          </thead>
          <tbody>
            {#each record.externalIds as xid}
              <tr>
                <td class="mono">{xid.namespace}</td>
                <td class="mono"><strong>{xid.value}</strong></td>
                <td>{xid.source}</td>
                <td
                  ><span class="id-status-badge {xid.status}"
                    >{xid.status.replace("_", " ")}</span
                  ></td
                >
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {:else if activeTab === "episodes" && record.seasons}
      <section class="episodes-section">
        <div class="season-tabs">
          {#each record.seasons as season, idx}
            <button
              type="button"
              class="season-btn"
              class:active={selectedSeasonIndex === idx}
              onclick={() => (selectedSeasonIndex = idx)}
            >
              {season.title}
            </button>
          {/each}
        </div>

        <div class="episodes-list">
          {#each record.seasons[selectedSeasonIndex].episodes as ep (ep.id)}
            <div class="episode-row" class:watched={ep.watched}>
              <button
                type="button"
                class="check-btn"
                class:checked={ep.watched}
                onclick={() => onToggleEpisode?.(record.id, ep.id)}
                aria-label="Toggle watched state for Episode {ep.number}"
              >
                {#if ep.watched}
                  <IconCheck size={16} stroke={3} />
                {/if}
              </button>
              <div class="ep-number">#{ep.number}</div>
              <div class="ep-info">
                <span class="ep-title">{ep.title}</span>
                {#if ep.durationSeconds}
                  <span class="ep-meta"
                    >{Math.round(ep.durationSeconds / 60)} min</span
                  >
                {/if}
              </div>
              {#if ep.watchedAt}
                <span class="ep-watched-date">
                  {new Date(ep.watchedAt).toLocaleDateString("en-IE", {
                    month: "short",
                    day: "numeric",
                  })}
                </span>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {:else if activeTab === "notes"}
      <section class="notes-section">
        <h3 class="section-title">Personal Reflections & Notes</h3>
        <div class="notes-box">
          <p>{record.userNotes ?? "No notes recorded yet."}</p>
        </div>
      </section>
    {/if}
  </main>
</div>

<style>
  .detail-container {
    max-width: 1000px;
    margin: 0 auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .back-btn {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: var(--fasti-action-primary);
    font-weight: 600;
    font-size: 0.9rem;
    cursor: pointer;
    padding: 6px 0;
  }

  .detail-hero {
    position: relative;
    background-size: cover;
    background-position: center;
    border-radius: 8px;
    overflow: hidden;
    padding: 32px 24px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .hero-content {
    display: flex;
    gap: 28px;
    align-items: flex-start;
  }

  .poster-container {
    width: 180px;
    aspect-ratio: 2 / 3;
    border-radius: 6px;
    overflow: hidden;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
    background: var(--fasti-surface-archive);
    flex-shrink: 0;
  }

  .detail-poster {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .hero-meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tag-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .kind-chip,
  .status-chip,
  .year-chip {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    padding: 3px 8px;
    border-radius: 3px;
  }

  .kind-chip {
    background: var(--fasti-brand-mark);
    color: white;
  }
  .status-chip {
    background: var(--fasti-action-primary);
    color: white;
  }
  .year-chip {
    background: color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    color: var(--fasti-text-primary);
  }

  .record-title {
    font-family: var(--fasti-font-display);
    font-size: 2.5rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
    line-height: 1.1;
  }

  .original-title {
    font-family: var(--fasti-font-display);
    font-size: 1.25rem;
    font-style: italic;
    color: var(--fasti-text-muted);
    margin: 0 0 12px;
  }

  .user-action-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 20px;
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .rating-picker {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .star-btn {
    background: transparent;
    border: none;
    padding: 2px;
    color: color-mix(in srgb, var(--fasti-text-muted) 40%, transparent);
    cursor: pointer;
  }

  .star-btn.active {
    color: var(--fasti-brand-gold);
  }

  .rating-value {
    font-family: var(--fasti-font-mono);
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--fasti-brand-gold);
    margin-left: 8px;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 4px;
    font-weight: 600;
    font-size: 0.9rem;
    cursor: pointer;
  }

  .play-with-btn {
    background: var(--fasti-action-primary);
    color: white;
    border: none;
  }

  .detail-tabs {
    display: flex;
    gap: 4px;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    font-size: 0.92rem;
    font-weight: 500;
    color: var(--fasti-text-muted);
    cursor: pointer;
    margin-bottom: -2px;
  }

  .tab-btn.active {
    color: var(--fasti-action-primary);
    border-bottom-color: var(--fasti-action-primary);
    font-weight: 600;
  }

  .tab-body {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    padding: 24px;
  }

  .section-title {
    font-family: var(--fasti-font-display);
    font-size: 1.3rem;
    margin: 16px 0 8px;
    color: var(--fasti-text-primary);
  }

  .section-title:first-child {
    margin-top: 0;
  }

  .overview-text {
    line-height: 1.7;
    max-width: 75ch;
    color: var(--fasti-text-primary);
  }

  .genre-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 16px;
  }

  .genre-tag {
    padding: 4px 10px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: 4px;
    font-size: 0.85rem;
  }

  .code-id {
    display: inline-block;
    padding: 4px 8px;
    background: var(--fasti-surface-archive);
    font-family: var(--fasti-font-mono);
    font-size: 0.82rem;
    border-radius: 3px;
  }

  .banner-note {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 10%,
      transparent
    );
    border-left: 4px solid var(--fasti-state-verified);
    border-radius: 4px;
    margin-bottom: 20px;
    font-size: 0.88rem;
  }

  :global(.verified-icon) {
    color: var(--fasti-state-verified);
    flex-shrink: 0;
  }

  .id-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
    text-align: left;
  }

  .id-table th,
  .id-table td {
    padding: 10px 14px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .id-table th {
    background: var(--fasti-surface-archive);
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
  }

  .mono {
    font-family: var(--fasti-font-mono);
  }

  .id-status-badge {
    padding: 2px 6px;
    border-radius: 3px;
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    text-transform: uppercase;
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 15%,
      transparent
    );
    color: var(--fasti-state-verified);
  }

  .season-tabs {
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
  }

  .season-btn {
    padding: 6px 14px;
    border-radius: 4px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    background: var(--fasti-surface-archive);
    cursor: pointer;
    font-weight: 500;
  }

  .season-btn.active {
    background: var(--fasti-brand-mark);
    color: white;
    font-weight: 600;
  }

  .episodes-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .episode-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: 4px;
    background: var(--fasti-surface-archive);
    border: 1px solid transparent;
  }

  .episode-row.watched {
    opacity: 0.75;
  }

  .check-btn {
    width: 24px;
    height: 24px;
    border-radius: 4px;
    border: 2px solid var(--fasti-text-muted);
    background: transparent;
    display: grid;
    place-items: center;
    cursor: pointer;
    padding: 0;
  }

  .check-btn.checked {
    background: var(--fasti-state-verified);
    border-color: var(--fasti-state-verified);
    color: white;
  }

  .ep-number {
    font-family: var(--fasti-font-mono);
    font-weight: 700;
    color: var(--fasti-text-muted);
  }

  .ep-info {
    flex: 1;
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  .ep-title {
    font-weight: 600;
  }

  .ep-meta {
    font-size: 0.8rem;
    color: var(--fasti-text-muted);
  }

  .ep-watched-date {
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
  }

  .notes-box {
    padding: 16px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    font-style: italic;
    color: var(--fasti-text-primary);
  }
</style>
