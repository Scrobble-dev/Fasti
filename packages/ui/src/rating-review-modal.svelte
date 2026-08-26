<script lang="ts">
  import type { MediaRecord } from "./types.js";
  import {
    IconX,
    IconStarFilled,
    IconStar,
    IconCheck,
  } from "@tabler/icons-svelte";

  interface Props {
    record: MediaRecord;
    onClose: () => void;
    onSaveReview: (recordId: string, rating: number, notes: string) => void;
  }

  let { record, onClose, onSaveReview }: Props = $props();

  let ratingVal = $state(8);
  let dialog: HTMLDialogElement | undefined;
  let reviewText = $state("");
  let hoverRating = $state<number | null>(null);
  let syncedRecordId = $state("");

  $effect(() => {
    if (record.id !== syncedRecordId) {
      syncedRecordId = record.id;
      ratingVal = record.userRating ?? 8;
      reviewText = record.userNotes ?? "";
    }
  });

  $effect(() => {
    if (!dialog?.open) dialog?.showModal();
  });

  function handleSave(): void {
    onSaveReview(record.id, ratingVal, reviewText);
    onClose();
  }
</script>

<dialog
  bind:this={dialog}
  class="modal-backdrop"
  aria-labelledby="review-title"
  oncancel={onClose}
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <div class="modal-card">
    <div class="modal-header">
      <h2 id="review-title" class="modal-title">
        Rate & Review — {record.title}
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
      <!-- 10-Star Rating Picker -->
      <div class="rating-picker-section">
        <span class="section-label"
          >Your Rating: <strong>{hoverRating ?? ratingVal} / 10</strong></span
        >
        <div
          class="stars-row"
          role="radiogroup"
          aria-label="Rate from 1 to 10 stars"
        >
          {#each [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as star}
            <button
              type="button"
              role="radio"
              aria-checked={star === ratingVal}
              class="star-btn"
              class:filled={star <= (hoverRating ?? ratingVal)}
              onmouseenter={() => (hoverRating = star)}
              onmouseleave={() => (hoverRating = null)}
              onclick={() => (ratingVal = star)}
              aria-label="{star} stars"
            >
              ★
            </button>
          {/each}
        </div>
      </div>

      <!-- Review Text Field -->
      <div class="field-group">
        <label for="review-notes" class="section-label"
          >Personal Notes & Critical Review:</label
        >
        <textarea
          id="review-notes"
          bind:value={reviewText}
          rows="5"
          placeholder="Write your reflections, memorable quotes, or spoiler-free critique..."
          class="review-textarea"></textarea>
      </div>
    </div>

    <div class="modal-footer">
      <button type="button" class="btn-cancel" onclick={onClose}>Cancel</button>
      <button type="button" class="btn-save" onclick={handleSave}>
        <IconCheck size={16} /> Post Review
      </button>
    </div>
  </div>
</dialog>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 9999;
    width: 100%;
    max-width: none;
    height: 100%;
    max-height: none;
    margin: 0;
    border: 0;
    background: transparent;
    display: grid;
    place-items: center;
    padding: 16px;
  }

  .modal-backdrop::backdrop {
    background: rgba(0, 0, 0, 0.5);
  }

  .modal-backdrop:not([open]) {
    display: none;
  }

  .modal-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 8px;
    width: 100%;
    max-width: 520px;
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
    gap: 18px;
  }

  .rating-picker-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: center;
    background: var(--fasti-surface-archive);
    padding: 14px;
    border-radius: 6px;
  }

  .section-label {
    font-size: 0.85rem;
    font-family: var(--fasti-font-mono);
    color: var(--fasti-text-muted);
  }

  .stars-row {
    display: flex;
    gap: 4px;
  }

  .star-btn {
    background: transparent;
    border: none;
    font-size: 1.8rem;
    color: color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    cursor: pointer;
    padding: 0;
    transition:
      transform 80ms ease,
      color 80ms ease;
  }

  .star-btn:hover {
    transform: scale(1.2);
  }

  .star-btn.filled {
    color: var(--fasti-brand-gold);
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .review-textarea {
    width: 100%;
    padding: 10px 14px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-family: var(--fasti-font-body);
    font-size: 0.92rem;
    background: var(--fasti-surface-paper);
    box-sizing: border-box;
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
