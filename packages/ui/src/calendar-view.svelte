<script lang="ts">
  import type { MediaRecord } from "./types.js";
  import {
    IconPlayerPlay,
    IconCalendar,
    IconCheck,
  } from "@tabler/icons-svelte";

  interface Props {
    watchingRecords: MediaRecord[];
    onSelectRecord: (recordId: string) => void;
  }

  let { watchingRecords, onSelectRecord }: Props = $props();

  const scheduleDays = [
    {
      day: "Today",
      date: "Aug 25",
      items: [{ title: "Frieren Mini Anime", episode: "Ep 11", time: "18:00" }],
    },
    {
      day: "Tomorrow",
      date: "Aug 26",
      items: [
        {
          title: "Severance Season 2",
          episode: "Trailer / Preview",
          time: "15:00",
        },
      ],
    },
    {
      day: "Friday",
      date: "Aug 29",
      items: [
        {
          title: "The Lord of the Rings: The Rings of Power",
          episode: "S2 E1",
          time: "08:00",
        },
      ],
    },
  ];
</script>

<div class="calendar-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Up Next & Calendar</h1>
      <p class="view-subtitle">
        Resume your active series and track upcoming broadcast releases.
      </p>
    </div>
  </header>

  <!-- Up Next Deck -->
  <section class="up-next-section" aria-label="Up Next Deck">
    <h2 class="section-title">Up Next Deck</h2>
    <div class="deck-grid">
      {#each watchingRecords as rec (rec.id)}
        <div class="deck-card">
          <div class="card-thumb">
            {#if rec.posterUrl}
              <img src={rec.posterUrl} alt="" class="thumb-img" />
            {/if}
          </div>
          <div class="deck-content">
            <span class="next-label">Next to Watch</span>
            <h3 class="deck-title">{rec.title}</h3>
            <p class="deck-meta">
              {#if rec.progressEpisodes}
                Ep {rec.progressEpisodes + 1} of {rec.totalEpisodes ?? "?"}
              {:else}
                Resume playback
              {/if}
            </p>
            <button
              type="button"
              class="play-next-btn"
              onclick={() => onSelectRecord(rec.id)}
            >
              <IconPlayerPlay size={16} stroke={2.5} /> Continue
            </button>
          </div>
        </div>
      {/each}
    </div>
  </section>

  <!-- Release Schedule -->
  <section class="schedule-section" aria-label="Upcoming Releases">
    <h2 class="section-title">Release Schedule</h2>
    <div class="schedule-timeline">
      {#each scheduleDays as day}
        <div class="day-group">
          <div class="day-header">
            <span class="day-name">{day.day}</span>
            <span class="day-date">{day.date}</span>
          </div>
          <div class="day-items">
            {#each day.items as item}
              <div class="schedule-item-card">
                <span class="item-time">{item.time}</span>
                <div class="item-details">
                  <span class="item-title">{item.title}</span>
                  <span class="item-ep">{item.episode}</span>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </section>
</div>

<style>
  .calendar-container {
    max-width: 960px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .view-header {
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

  .section-title {
    font-family: var(--fasti-font-display);
    font-size: 1.4rem;
    margin: 0 0 16px;
    color: var(--fasti-text-primary);
  }

  .deck-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }

  .deck-card {
    display: flex;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
    overflow: hidden;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.03);
  }

  .card-thumb {
    width: 85px;
    background: var(--fasti-surface-archive);
    flex-shrink: 0;
  }

  .thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .deck-content {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    flex: 1;
  }

  .next-label {
    font-family: var(--fasti-font-mono);
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--fasti-action-primary);
  }

  .deck-title {
    font-family: var(--fasti-font-display);
    font-size: 1.1rem;
    font-weight: 600;
    margin: 2px 0;
    line-height: 1.2;
  }

  .deck-meta {
    font-size: 0.8rem;
    color: var(--fasti-text-muted);
    margin: 0 0 10px;
  }

  .play-next-btn {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 6px 12px;
    border-radius: 4px;
    background: var(--fasti-action-primary);
    color: white;
    font-size: 0.82rem;
    font-weight: 600;
    border: none;
    cursor: pointer;
  }

  .schedule-timeline {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .day-group {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 16px;
  }

  .day-header {
    display: flex;
    flex-direction: column;
  }

  .day-name {
    font-family: var(--fasti-font-display);
    font-size: 1.2rem;
    font-weight: 600;
  }

  .day-date {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
    color: var(--fasti-text-muted);
  }

  .day-items {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .schedule-item-card {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 16px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: 4px;
  }

  .item-time {
    font-family: var(--fasti-font-mono);
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--fasti-text-muted);
  }

  .item-title {
    font-weight: 600;
    font-size: 0.92rem;
  }

  .item-ep {
    margin-left: 8px;
    font-size: 0.82rem;
    color: var(--fasti-text-muted);
  }
</style>
