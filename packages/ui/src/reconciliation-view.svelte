<script lang="ts">
  import type { ReconciliationCase } from "./types.js";
  import {
    IconGitPullRequest,
    IconCheck,
    IconX,
    IconClock,
    IconAlertTriangle,
    IconShieldCheck,
    IconArrowRight,
  } from "@tabler/icons-svelte";

  interface Props {
    cases: ReconciliationCase[];
    onAcceptCase?: (caseId: string) => void;
    onRejectCase?: (caseId: string) => void;
    onDeferCase?: (caseId: string) => void;
  }

  let { cases, onAcceptCase, onRejectCase, onDeferCase }: Props = $props();

  const actionableCases = $derived(
    cases.filter((c) => c.status !== "deferred"),
  );
</script>

<div class="reconciliation-container">
  <header class="view-header">
    <div>
      <h1 class="view-title">Review Inbox</h1>
      <p class="view-subtitle">
        Review ambiguous matches, topology variances, and candidate crosswalks.
      </p>
    </div>

    <div class="safe-banner">
      <IconShieldCheck size={20} class="verified-icon" />
      <span
        ><strong>Zero Silent Merges:</strong> Unresolved records remain safe and usable.</span
      >
    </div>
  </header>

  {#if actionableCases.length === 0}
    <div class="empty-inbox">
      <IconShieldCheck size={48} class="empty-icon" />
      <h2>All caught up!</h2>
      <p>
        No records currently require manual identity reconciliation or topology
        review.
      </p>
    </div>
  {:else}
    <div class="cases-list">
      {#each actionableCases as item (item.id)}
        <article class="case-card">
          <div class="case-header">
            <span class="case-badge">Topology Conflict</span>
            <h2 class="case-subject">{item.title}</h2>
          </div>

          <div class="comparison-grid">
            <!-- Left: Fasti Local Ingest -->
            <div class="comparison-pane local">
              <h3 class="pane-title">Fasti Local Ingest</h3>
              <p class="pane-desc">
                Supplied from original import observation:
              </p>
              <div class="id-chips">
                {#each item.suppliedIds as xid}
                  <span class="id-chip">
                    <span class="ns">{xid.namespace}:</span>
                    <span class="val">{xid.value}</span>
                  </span>
                {/each}
              </div>
            </div>

            <div class="vs-divider" aria-hidden="true">
              <IconArrowRight size={20} />
            </div>

            <!-- Right: Candidate Match -->
            <div class="comparison-pane candidate">
              <h3 class="pane-title">Candidate Match</h3>
              <div class="candidate-header">
                {#if item.candidatePosterUrl}
                  <img
                    src={item.candidatePosterUrl}
                    alt=""
                    class="candidate-thumb"
                  />
                {/if}
                <div>
                  <h4 class="candidate-name">{item.candidateTitle}</h4>
                  <span class="candidate-id-badge"
                    >{item.candidateNamespace}: {item.candidateExternalId}</span
                  >
                </div>
              </div>

              <div class="reasons-list">
                <h4 class="reasons-label">Matching Evidence:</h4>
                <ul>
                  {#each item.matchingReasons as reason}
                    <li class="reason-item match">
                      <IconCheck size={14} class="reason-icon check" />
                      {reason}
                    </li>
                  {/each}
                </ul>
              </div>

              {#if item.conflictingFactors.length > 0}
                <div class="conflicts-list">
                  <h4 class="reasons-label">Variance to Note:</h4>
                  <ul>
                    {#each item.conflictingFactors as conflict}
                      <li class="reason-item conflict">
                        <IconAlertTriangle size={14} class="reason-icon warn" />
                        {conflict}
                      </li>
                    {/each}
                  </ul>
                </div>
              {/if}
            </div>
          </div>

          <div class="case-actions">
            <button
              type="button"
              class="action-btn accept"
              onclick={() => onAcceptCase?.(item.id)}
            >
              <IconCheck size={16} stroke={2.5} /> Accept Identifier Only
            </button>

            <button
              type="button"
              class="action-btn not-same"
              onclick={() => onRejectCase?.(item.id)}
            >
              <IconX size={16} stroke={2.5} /> Not the Same (not_same_as)
            </button>

            <button
              type="button"
              class="action-btn defer"
              onclick={() => onDeferCase?.(item.id)}
            >
              <IconClock size={16} /> Resolve Later
            </button>
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
    font-size: 1.3rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .comparison-grid {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    gap: 20px;
    padding: 20px;
    align-items: center;
  }

  .comparison-pane {
    background: var(--fasti-surface-archive);
    border-radius: 6px;
    padding: 16px;
    height: 100%;
    box-sizing: border-box;
  }

  .pane-title {
    font-family: var(--fasti-font-display);
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 6px;
    color: var(--fasti-text-primary);
  }

  .pane-desc {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
    margin: 0 0 12px;
  }

  .id-chips {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .id-chip {
    display: inline-flex;
    gap: 6px;
    padding: 6px 10px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 4px;
    font-family: var(--fasti-font-mono);
    font-size: 0.82rem;
  }

  .id-chip .ns {
    color: var(--fasti-text-muted);
  }
  .id-chip .val {
    font-weight: 700;
    color: var(--fasti-text-primary);
  }

  .candidate-header {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-bottom: 12px;
  }

  .candidate-thumb {
    width: 48px;
    height: 68px;
    object-fit: cover;
    border-radius: 4px;
  }

  .candidate-name {
    margin: 0 0 4px;
    font-size: 1rem;
    font-weight: 600;
  }

  .candidate-id-badge {
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    color: var(--fasti-action-primary);
  }

  .reasons-list,
  .conflicts-list {
    margin-top: 12px;
  }

  .reasons-label {
    font-size: 0.8rem;
    font-family: var(--fasti-font-mono);
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    margin: 0 0 6px;
  }

  .reason-item {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: 0.85rem;
    margin-bottom: 4px;
    line-height: 1.3;
  }

  .reason-item.match {
    color: var(--fasti-state-verified);
  }
  .reason-item.conflict {
    color: var(--fasti-state-attention);
  }

  :global(.reason-icon.check) {
    color: var(--fasti-state-verified);
    flex-shrink: 0;
    margin-top: 2px;
  }
  :global(.reason-icon.warn) {
    color: var(--fasti-state-attention);
    flex-shrink: 0;
    margin-top: 2px;
  }

  .case-actions {
    padding: 16px 20px;
    background: var(--fasti-surface-archive);
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 9px 18px;
    border-radius: 4px;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: all 120ms ease;
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

  .action-btn.defer {
    background: transparent;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 40%, transparent);
    color: var(--fasti-text-muted);
    margin-left: auto;
  }
</style>
