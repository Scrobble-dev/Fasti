<script lang="ts">
  import type { ChronicleOccurrence } from "./types.js";
  import IconRepeat from "@tabler/icons-svelte/icons/repeat";
  import IconStarFilled from "@tabler/icons-svelte/icons/star-filled";
  import IconDeviceTv from "@tabler/icons-svelte/icons/device-tv";
  import IconCalendarTime from "@tabler/icons-svelte/icons/calendar-time";
  import IconSparkles from "@tabler/icons-svelte/icons/sparkles";

  interface Props {
    occurrences: ChronicleOccurrence[];
    onSelectRecord: (recordId: string) => void;
  }

  let { occurrences, onSelectRecord }: Props = $props();

  // Compute summary stats
  const totalMinutes = $derived(
    occurrences.reduce((acc, occ) => acc + occ.durationMinutes, 0),
  );
  const totalHours = $derived((totalMinutes / 60).toFixed(1));
  const totalEvents = $derived(occurrences.length);
</script>

<div class="chronicle-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Chronicle</h1>
      <p class="view-subtitle">
        The immutable, chronological record of your media life.
      </p>
    </div>

    <div class="summary-stats">
      <div class="stat-card">
        <span class="stat-value">{totalHours}h</span>
        <span class="stat-label">Logged Time</span>
      </div>
      <div class="stat-card">
        <span class="stat-value">{totalEvents}</span>
        <span class="stat-label">Events</span>
      </div>
    </div>
  </header>

  <!-- Timeline Feed -->
  <section class="timeline-section" aria-label="Recent Media Occurrences">
    <h2 class="section-heading">Recent Timeline</h2>

    {#if occurrences.length === 0}
      <div class="timeline-empty">
        <h3>No Chronicle entries</h3>
        <p>Entries appear after Fasti receives real media activity.</p>
      </div>
    {:else}
      <div class="timeline-feed">
        {#each occurrences as occ (occ.id)}
          <article class="timeline-card">
            <div class="card-art">
              {#if occ.posterUrl}
                <img
                  src={occ.posterUrl}
                  alt=""
                  class="poster-img"
                  loading="lazy"
                />
              {:else}
                <div class="poster-fallback">{occ.mediaKind}</div>
              {/if}
            </div>

            <div class="card-main">
              <div class="card-header-row">
                <span class="kind-tag {occ.mediaKind}">{occ.mediaKind}</span>
                {#if occ.isRewatch}
                  <span class="rewatch-tag" title="Rewatch">
                    <IconRepeat size={12} stroke={2.5} /> Rewatch
                  </span>
                {/if}
                <time class="event-time" datetime={occ.timestamp}>
                  {new Date(occ.timestamp).toLocaleDateString("en-IE", {
                    month: "short",
                    day: "numeric",
                    hour: "2-digit",
                    minute: "2-digit",
                  })}
                </time>
              </div>

              <button
                type="button"
                class="title-link-button"
                onclick={() => onSelectRecord(occ.recordId)}
              >
                <h3 class="event-title">{occ.title}</h3>
              </button>

              {#if occ.episodeTitle}
                <p class="episode-title">
                  S{occ.seasonNumber} E{occ.episodeNumber} · {occ.episodeTitle}
                </p>
              {/if}

              <div class="card-footer-row">
                <div class="client-info">
                  <IconDeviceTv size={14} stroke={1.75} />
                  <span>{occ.deviceName}</span>
                  <span class="bullet">·</span>
                  <span class="client-badge">{occ.clientName}</span>
                </div>

                {#if occ.userRating}
                  <div class="rating-badge">
                    <IconStarFilled size={13} class="star-icon" />
                    <span>{occ.userRating}/10</span>
                  </div>
                {/if}
              </div>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .chronicle-container {
    max-width: 900px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .view-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 18px;
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

  .summary-stats {
    display: flex;
    gap: 16px;
  }

  .stat-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    padding: 8px 14px;
    text-align: right;
  }

  .stat-value {
    display: block;
    font-family: var(--fasti-font-mono);
    font-size: 1.25rem;
    font-weight: 700;
    color: var(--fasti-brand-mark);
  }

  .stat-label {
    font-size: 0.72rem;
    color: var(--fasti-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .section-heading {
    font-family: var(--fasti-font-display);
    font-size: 1.4rem;
    margin: 0 0 16px;
    color: var(--fasti-text-primary);
  }

  .timeline-feed {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .timeline-card {
    display: flex;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    overflow: hidden;
    transition:
      transform 120ms ease,
      box-shadow 120ms ease;
  }

  .timeline-card:hover {
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.05);
  }

  .card-art {
    width: 90px;
    min-height: 120px;
    background: var(--fasti-surface-archive);
    flex-shrink: 0;
  }

  .poster-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .card-main {
    flex: 1;
    padding: 14px 18px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }

  .card-header-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .kind-tag {
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
    background: color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    color: var(--fasti-text-muted);
  }

  .kind-tag.anime {
    background: color-mix(in srgb, #9333ea 15%, transparent);
    color: #7e22ce;
  }
  .kind-tag.movie {
    background: color-mix(in srgb, #2563eb 15%, transparent);
    color: #1d4ed8;
  }
  .kind-tag.show {
    background: color-mix(in srgb, #059669 15%, transparent);
    color: #047857;
  }
  .kind-tag.game {
    background: color-mix(in srgb, #d97706 15%, transparent);
    color: #b45309;
  }
  .kind-tag.book {
    background: color-mix(in srgb, #b91c1c 15%, transparent);
    color: #991b1b;
  }

  .rewatch-tag {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    padding: 2px 6px;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 15%,
      transparent
    );
    color: var(--fasti-action-primary);
  }

  .event-time {
    margin-left: auto;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
  }

  .title-link-button {
    background: transparent;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    margin: 0;
  }

  .title-link-button:hover .event-title {
    color: var(--fasti-action-primary);
  }

  .event-title {
    font-family: var(--fasti-font-display);
    font-size: 1.25rem;
    font-weight: 600;
    margin: 0 0 2px;
    color: var(--fasti-text-primary);
    transition: color 100ms ease;
  }

  .episode-title {
    margin: 0 0 8px;
    font-size: 0.88rem;
    color: var(--fasti-text-muted);
  }

  .card-footer-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 8px;
    font-size: 0.8rem;
    color: var(--fasti-text-muted);
  }

  .client-info {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .client-badge {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    background: color-mix(
      in srgb,
      var(--fasti-surface-archive) 90%,
      transparent
    );
    padding: 2px 6px;
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
  }

  .rating-badge {
    display: flex;
    align-items: center;
    gap: 4px;
    font-family: var(--fasti-font-mono);
    font-weight: 700;
    color: var(--fasti-brand-gold);
  }

  .star-icon {
    color: var(--fasti-brand-gold);
  }

  .timeline-empty {
    padding: 24px;
    border: 1px solid var(--fasti-border-subtle);
    background: var(--fasti-surface-paper);
  }

  .timeline-empty :is(h3, p) {
    margin: 0;
  }

  .timeline-empty p {
    margin-top: 8px;
    color: var(--fasti-text-muted);
  }
</style>
