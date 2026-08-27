<script lang="ts">
  import type { MediaRecord } from "./types.js";

  interface Props {
    watchingRecords: MediaRecord[];
    onSelectRecord: (recordId: string) => void;
  }

  let { watchingRecords, onSelectRecord }: Props = $props();
</script>

<div class="calendar-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Up Next & Calendar</h1>
      <p class="view-subtitle">
        Review records that are currently in progress.
      </p>
    </div>
  </header>

  {#if watchingRecords.length === 0}
    <section class="calendar-empty" aria-labelledby="calendar-empty-title">
      <h2 id="calendar-empty-title">No active records</h2>
      <p>Calendar entries appear only when Fasti receives real record data.</p>
    </section>
  {:else}
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
              <span class="next-label">Active record</span>
              <h3 class="deck-title">{rec.title}</h3>
              <p class="deck-meta">
                {#if rec.progressEpisodes !== undefined}
                  Ep {rec.totalEpisodes === undefined
                    ? rec.progressEpisodes + 1
                    : Math.min(rec.progressEpisodes + 1, rec.totalEpisodes)} of
                  {rec.totalEpisodes ?? "?"}
                {:else}
                  No episode progress recorded
                {/if}
              </p>
              <button
                type="button"
                class="open-record-btn"
                onclick={() => onSelectRecord(rec.id)}
              >
                Open record
              </button>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}
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

  .calendar-empty {
    padding: 32px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border-radius: 6px;
    background: var(--fasti-surface-paper);
  }

  .calendar-empty h2 {
    margin: 0 0 8px;
    font-family: var(--fasti-font-display);
  }

  .calendar-empty p {
    margin: 0;
    color: var(--fasti-text-muted);
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

  .open-record-btn {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 6px 12px;
    border-radius: 4px;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    font-size: 0.82rem;
    font-weight: 600;
    border: none;
    cursor: pointer;
  }
</style>
