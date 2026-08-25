<script lang="ts">
  import type { MediaRecord, WatchStatus } from "./types.js";
  import { IconX, IconCheck, IconPlayerPlay } from "@tabler/icons-svelte";

  interface Props {
    record: MediaRecord;
    onClose: () => void;
    onSaveProgress: (
      recordId: string,
      episodes: number,
      seconds: number,
      status: WatchStatus,
    ) => void;
  }

  let { record, onClose, onSaveProgress }: Props = $props();

  let episodeVal = $state(0);
  let totalEps = $derived(record.totalEpisodes ?? 1);
  let progressSec = $state(0);
  let totalSec = $derived(
    record.totalDurationSeconds ??
      (record.runtimeMinutes ? record.runtimeMinutes * 60 : 3600),
  );
  let statusVal = $state<WatchStatus>("watching");

  $effect(() => {
    episodeVal = record.progressEpisodes ?? 0;
    progressSec = record.progressSeconds ?? 0;
    statusVal = record.status;
  });

  const percentage = $derived(
    record.mediaKind === "movie" || !record.totalEpisodes
      ? Math.min(100, Math.round((progressSec / totalSec) * 100))
      : Math.min(100, Math.round((episodeVal / totalEps) * 100)),
  );

  function handleSave(): void {
    const finalStatus: WatchStatus =
      percentage >= 100
        ? "completed"
        : statusVal === "plan_to_watch"
          ? "watching"
          : statusVal;
    onSaveProgress(record.id, episodeVal, progressSec, finalStatus);
    onClose();
  }
</script>

<div
  class="modal-backdrop"
  role="dialog"
  aria-modal="true"
  aria-labelledby="prog-title"
>
  <div class="modal-card">
    <div class="modal-header">
      <h2 id="prog-title" class="modal-title">
        Update Progress — {record.title}
      </h2>
      <button
        type="button"
        class="close-btn"
        onclick={onClose}
        aria-label="Close dialog"
      >
        <IconX size={18} />
      </button>
    </div>

    <div class="modal-body">
      {#if record.mediaKind === "show" || record.mediaKind === "anime"}
        <div class="field-group">
          <label for="ep-input" class="field-label"
            >Episodes Watched ({episodeVal} of {totalEps}):</label
          >
          <div class="stepper-row">
            <button
              type="button"
              class="step-btn"
              onclick={() => (episodeVal = Math.max(0, episodeVal - 1))}
            >
              -
            </button>
            <input
              id="ep-input"
              type="number"
              min="0"
              max={totalEps}
              bind:value={episodeVal}
              class="num-input"
            />
            <button
              type="button"
              class="step-btn"
              onclick={() => (episodeVal = Math.min(totalEps, episodeVal + 1))}
            >
              +
            </button>
            <button
              type="button"
              class="max-btn"
              onclick={() => (episodeVal = totalEps)}
            >
              Set Max ({totalEps})
            </button>
          </div>
        </div>
      {:else}
        <div class="field-group">
          <label for="time-slider" class="field-label"
            >Minutes Elapsed ({Math.floor(progressSec / 60)} / {Math.floor(
              totalSec / 60,
            )} min):</label
          >
          <input
            id="time-slider"
            type="range"
            min="0"
            max={totalSec}
            step="60"
            bind:value={progressSec}
            class="range-slider"
          />
        </div>
      {/if}

      <!-- Progress Meter -->
      <div class="meter-wrap">
        <div class="meter-bar" style="width: {percentage}%"></div>
        <span class="meter-text">{percentage}% Completed</span>
      </div>

      <!-- Status selection -->
      <div class="field-group mt-3">
        <label for="status-picker" class="field-label">Watch Status:</label>
        <select id="status-picker" bind:value={statusVal} class="select-input">
          <option value="watching">In Progress / Watching</option>
          <option value="completed">Completed</option>
          <option value="plan_to_watch">Plan to Watch</option>
          <option value="on_hold">On Hold</option>
          <option value="dropped">Dropped</option>
        </select>
      </div>
    </div>

    <div class="modal-footer">
      <button type="button" class="btn-cancel" onclick={onClose}>Cancel</button>
      <button type="button" class="btn-save" onclick={handleSave}>
        <IconCheck size={16} /> Save Progress
      </button>
    </div>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9999;
    background: rgba(0, 0, 0, 0.5);
    display: grid;
    place-items: center;
    padding: 16px;
    animation: fadeIn 100ms ease-out;
  }

  .modal-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 8px;
    width: 100%;
    max-width: 480px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.2);
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .modal-title {
    font-family: var(--fasti-font-display);
    font-size: 1.25rem;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .close-btn {
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--fasti-text-muted);
  }

  .modal-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field-label {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
    text-transform: uppercase;
  }

  .stepper-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .step-btn {
    width: 38px;
    height: 38px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    background: var(--fasti-surface-archive);
    font-size: 1.2rem;
    font-weight: bold;
    cursor: pointer;
  }

  .num-input {
    width: 80px;
    height: 38px;
    text-align: center;
    font-family: var(--fasti-font-mono);
    font-size: 1.1rem;
    font-weight: 700;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    background: var(--fasti-surface-paper);
  }

  .max-btn {
    height: 38px;
    padding: 0 12px;
    border-radius: 4px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    background: var(--fasti-surface-archive);
    font-size: 0.85rem;
    font-weight: 600;
    cursor: pointer;
  }

  .range-slider {
    width: 100%;
    height: 8px;
    accent-color: var(--fasti-action-primary);
  }

  .meter-wrap {
    height: 24px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    overflow: hidden;
    position: relative;
    display: flex;
    align-items: center;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .meter-bar {
    height: 100%;
    background: var(--fasti-state-verified);
    transition: width 150ms ease;
  }

  .meter-text {
    position: absolute;
    width: 100%;
    text-align: center;
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    font-weight: 700;
    color: var(--fasti-text-primary);
  }

  .select-input {
    height: 38px;
    padding: 6px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.9rem;
    background: var(--fasti-surface-paper);
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    padding: 14px 20px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    background: var(--fasti-surface-archive);
  }

  .btn-cancel {
    padding: 8px 16px;
    background: transparent;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
  }

  .btn-save {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 18px;
    background: var(--fasti-action-primary);
    color: white;
    border: none;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
  }
</style>
