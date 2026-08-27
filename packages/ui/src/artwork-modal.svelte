<script lang="ts">
  import type { MediaRecord } from "./types.js";
  import { IconX, IconCheck, IconPhoto } from "@tabler/icons-svelte";

  export interface ArtworkCandidate {
    readonly id: string;
    readonly namespace: string;
    readonly url: string;
  }

  interface Props {
    record: MediaRecord;
    candidates: ArtworkCandidate[];
    onClose: () => void;
    onSave?: (
      recordId: string,
      posterUrl: string,
      backdropUrl?: string,
    ) => void;
  }

  let { record, candidates, onClose, onSave }: Props = $props();

  let dialog: HTMLDialogElement | undefined;
  let editingPosterUrl = $state("");
  let editingBackdropUrl = $state("");
  let saveProblem = $state("");
  let syncedRecordId = $state("");

  $effect(() => {
    if (record.id !== syncedRecordId) {
      syncedRecordId = record.id;
      editingPosterUrl = record.posterUrl ?? "";
      editingBackdropUrl = record.backdropUrl ?? "";
    }
  });

  $effect(() => {
    if (!dialog?.open) dialog?.showModal();
  });

  function handleSelectCandidate(url: string): void {
    editingPosterUrl = url;
  }

  function handleSave(): void {
    if (!onSave) {
      saveProblem = "This host can't save artwork changes yet.";
      return;
    }
    onSave(
      record.id,
      editingPosterUrl.trim(),
      editingBackdropUrl.trim() || undefined,
    );
    onClose();
  }
</script>

<dialog
  bind:this={dialog}
  class="modal-backdrop"
  aria-labelledby="artwork-title"
  oncancel={onClose}
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <div class="modal-card">
    <div class="modal-header">
      <h2 id="artwork-title" class="modal-title">
        Edit Artwork — {record.title}
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
      <h3 class="section-heading">Candidate Posters</h3>
      {#if candidates.length > 0}
        <div class="candidate-grid">
          {#each candidates as candidate (candidate.id)}
            <button
              type="button"
              class="candidate-thumb"
              class:selected={editingPosterUrl === candidate.url}
              onclick={() => handleSelectCandidate(candidate.url)}
              title="From {candidate.namespace}"
            >
              <img
                src={candidate.url}
                alt="Poster from {candidate.namespace}"
              />
              {#if editingPosterUrl === candidate.url}
                <span class="selected-badge"><IconCheck size={14} /></span>
              {/if}
            </button>
          {/each}
        </div>
      {:else}
        <p class="empty-candidates-hint">
          <IconPhoto size={16} /> No candidate posters from linked providers yet.
          Enter a URL manually below.
        </p>
      {/if}

      <div class="form-field">
        <label for="artwork-poster-url">Custom Poster URL</label>
        <input
          id="artwork-poster-url"
          type="url"
          class="form-input"
          placeholder="https://..."
          bind:value={editingPosterUrl}
        />
      </div>

      <div class="form-field">
        <label for="artwork-backdrop-url">Custom Backdrop URL</label>
        <input
          id="artwork-backdrop-url"
          type="url"
          class="form-input"
          placeholder="https://..."
          bind:value={editingBackdropUrl}
        />
      </div>

      {#if editingPosterUrl}
        <div class="preview-row">
          <span class="preview-label">Preview</span>
          <img
            src={editingPosterUrl}
            alt="Selected poster preview"
            class="preview-img"
          />
        </div>
      {/if}

      {#if saveProblem}
        <p class="inline-problem" role="alert">{saveProblem}</p>
      {/if}
    </div>

    <div class="modal-footer">
      <button type="button" class="btn-cancel" onclick={onClose}>Cancel</button>
      <button type="button" class="btn-save" onclick={handleSave}>
        <IconCheck size={16} /> Save Artwork
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
    max-width: 560px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
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
    font-size: 1.2rem;
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
    gap: 14px;
    overflow-y: auto;
  }

  .section-heading {
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .candidate-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(90px, 1fr));
    gap: 10px;
  }
  .candidate-thumb {
    position: relative;
    aspect-ratio: 2 / 3;
    border-radius: 4px;
    overflow: hidden;
    border: 2px solid transparent;
    padding: 0;
    cursor: pointer;
    background: var(--fasti-surface-archive);
  }
  .candidate-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .candidate-thumb.selected {
    border-color: var(--fasti-action-primary);
  }
  .selected-badge {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--fasti-action-primary);
    color: white;
    display: grid;
    place-items: center;
  }

  .empty-candidates-hint {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .form-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .form-field label {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
  }
  .form-input {
    height: 38px;
    padding: 8px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.9rem;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .preview-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .preview-label {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
  }
  .preview-img {
    width: 60px;
    height: 90px;
    object-fit: cover;
    border-radius: 4px;
    background: var(--fasti-surface-archive);
  }

  .inline-problem {
    margin: 0;
    font-size: 0.86rem;
    color: var(--fasti-state-error, #b42318);
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
