<script lang="ts">
  import type { ReviewItem } from "./types.js";
  import {
    IconCheck,
    IconClock,
    IconShieldCheck,
    IconArrowRight,
  } from "@tabler/icons-svelte";

  const GRAINS = [
    "work",
    "series",
    "release",
    "edition",
    "season",
    "segment",
    "episode",
    "film",
    "recording",
    "album_release",
    "track",
    "chapter",
    "podcast_feed",
    "podcast_episode",
    "game_release",
    "custom",
  ] as const;

  interface Props {
    items: ReviewItem[];
    onResolveExisting?: (reviewItemId: string, recordId: string) => void;
    onResolveNew?: (reviewItemId: string, grain: string) => void;
  }

  let { items, onResolveExisting, onResolveNew }: Props = $props();
  const openItems = $derived(items.filter((item) => item.status === "open"));
  let newGrainByItem = $state<Record<string, string>>({});

  function grainFor(itemId: string): string {
    return newGrainByItem[itemId] ?? GRAINS[0];
  }
</script>

<div class="reconciliation-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Review Inbox</h1>
      <p class="view-subtitle">
        Resolve observations whose identity is still ambiguous.
      </p>
    </div>
    <div class="safe-banner">
      <IconShieldCheck size={20} class="verified-icon" />
      <span
        ><strong>Zero Silent Merges:</strong> Unresolved records remain safe and usable.</span
      >
    </div>
  </header>

  {#if openItems.length === 0}
    <div class="empty-inbox">
      <IconShieldCheck size={48} class="empty-icon" />
      <h2>All caught up!</h2>
      <p>No open reviews right now.</p>
    </div>
  {:else}
    <div class="cases-list">
      {#each openItems as item (item.review_item_id)}
        <article class="case-card">
          <div class="case-header">
            <span class="case-badge">Needs review</span>
            <h2 class="case-subject">
              Review <code>{item.review_item_id}</code>
            </h2>
          </div>
          <div class="case-body">
            <dl class="fact-list">
              <div>
                <dt>Observation</dt>
                <dd><code>{item.observation_id}</code></dd>
              </div>
              <div>
                <dt>Current interpretation</dt>
                <dd><code>{item.current_interpretation_id}</code></dd>
              </div>
            </dl>
            {#if item.candidate_record_ids.length > 0}
              <div class="candidates">
                <h3 class="section-label">Candidate records</h3>
                <ul class="candidate-list">
                  {#each item.candidate_record_ids as recordId (recordId)}
                    <li class="candidate-row">
                      <code>{recordId}</code>
                      <button
                        type="button"
                        class="action-btn accept"
                        disabled={!onResolveExisting}
                        onclick={() =>
                          onResolveExisting?.(item.review_item_id, recordId)}
                        title={!onResolveExisting
                          ? "Resolving is unavailable until a resolve command is implemented"
                          : undefined}
                      >
                        <IconCheck size={16} stroke={2.5} /> Accept as this record
                        <IconArrowRight size={14} />
                      </button>
                    </li>
                  {/each}
                </ul>
              </div>
            {:else}
              <p class="no-candidates">
                No candidate records were found for this observation.
              </p>
            {/if}
          </div>
          <div class="case-actions">
            <div class="new-record-form">
              <label for="grain-{item.review_item_id}"
                >Not one of these — create a new record with grain</label
              >
              <select
                id="grain-{item.review_item_id}"
                value={grainFor(item.review_item_id)}
                onchange={(e) =>
                  (newGrainByItem = {
                    ...newGrainByItem,
                    [item.review_item_id]: e.currentTarget.value,
                  })}
              >
                {#each GRAINS as grain}<option value={grain}>{grain}</option
                  >{/each}
              </select>
              <button
                type="button"
                class="action-btn not-same"
                disabled={!onResolveNew}
                onclick={() =>
                  onResolveNew?.(
                    item.review_item_id,
                    grainFor(item.review_item_id),
                  )}
                title={!onResolveNew
                  ? "Resolving is unavailable until a resolve command is implemented"
                  : undefined}>Create new record</button
              >
            </div>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</div>

<style>
  .reconciliation-container {
    max-width: 960px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }
  .view-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    flex-wrap: wrap;
    gap: 12px;
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
  .safe-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 12%,
      transparent
    );
    border-radius: 4px;
    font-size: 0.85rem;
    color: var(--fasti-text-primary);
  }
  :global(.verified-icon) {
    color: var(--fasti-state-verified);
  }
  .empty-inbox {
    padding: 64px 24px;
    text-align: center;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 6px;
  }
  :global(.empty-icon) {
    color: var(--fasti-state-verified);
    margin-bottom: 16px;
  }
  .empty-inbox h2 {
    font-family: var(--fasti-font-display);
    font-size: 1.8rem;
    margin: 0 0 8px;
  }
  .empty-inbox p {
    color: var(--fasti-text-muted);
    margin: 0;
  }
  .cases-list {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }
  .case-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.04);
  }
  .case-header {
    padding: 16px 20px;
    background: var(--fasti-surface-archive);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .case-badge {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    padding: 3px 8px;
    border-radius: 3px;
    background: color-mix(
      in srgb,
      var(--fasti-state-attention) 20%,
      transparent
    );
    color: var(--fasti-state-attention);
  }
  .case-subject {
    font-family: var(--fasti-font-display);
    font-size: 1.15rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
    overflow-wrap: anywhere;
  }
  .case-body {
    padding: 20px;
  }
  .fact-list {
    display: grid;
    gap: 8px 16px;
    margin: 0 0 16px;
    font-size: 0.85rem;
  }
  .fact-list dt {
    font-weight: 700;
    color: var(--fasti-text-muted);
  }
  .fact-list dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .section-label {
    font-size: 0.8rem;
    font-family: var(--fasti-font-mono);
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    margin: 0 0 8px;
  }
  .candidate-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .candidate-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    overflow-wrap: anywhere;
  }
  .no-candidates {
    color: var(--fasti-text-muted);
    margin: 0;
  }
  .case-actions {
    padding: 16px 20px;
    background: var(--fasti-surface-archive);
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }
  .new-record-form {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
  }
  .new-record-form select {
    min-height: 44px;
    padding: 6px 10px;
    border-radius: 4px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }
  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 44px;
    padding: 9px 18px;
    border-radius: 4px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: all 120ms ease;
  }
  .action-btn:focus-visible,
  .new-record-form select:focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }
  .action-btn:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }
  .action-btn.accept {
    background: var(--fasti-action-primary);
    color: white;
  }
  .action-btn.not-same {
    background: transparent;
    border: 1px solid var(--fasti-brand-mark);
    color: var(--fasti-brand-mark);
  }
  @media (prefers-reduced-motion: reduce) {
    .action-btn {
      transition: none;
    }
  }
</style>
