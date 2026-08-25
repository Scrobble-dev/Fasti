<script lang="ts">
  import type { MediaRecord } from "./types.js";
  import {
    IconX,
    IconFolderPlus,
    IconCheck,
    IconFolder,
  } from "@tabler/icons-svelte";

  interface Props {
    record: MediaRecord;
    onClose: () => void;
    onSaveCollection: (recordId: string, collectionNames: string[]) => void;
  }

  let { record, onClose, onSaveCollection }: Props = $props();

  let collections = $state<string[]>([
    "Favorites",
    "Must Watch Classics",
    "Sci-Fi Hall of Fame",
    "Cyberpunk & Neo-Noir",
    "High Fantasy Runs",
  ]);

  let selected = $state<string[]>([]);

  $effect(() => {
    selected = record.collectionName ? [record.collectionName] : ["Favorites"];
  });
  let newCollectionInput = $state("");

  function handleToggle(name: string): void {
    if (selected.includes(name)) {
      selected = selected.filter((s) => s !== name);
    } else {
      selected = [...selected, name];
    }
  }

  function handleCreateCollection(e: Event): void {
    e.preventDefault();
    if (
      newCollectionInput.trim().length > 0 &&
      !collections.includes(newCollectionInput.trim())
    ) {
      collections = [...collections, newCollectionInput.trim()];
      selected = [...selected, newCollectionInput.trim()];
      newCollectionInput = "";
    }
  }

  function handleSave(): void {
    onSaveCollection(record.id, selected);
    onClose();
  }
</script>

<div
  class="modal-backdrop"
  role="dialog"
  aria-modal="true"
  aria-labelledby="coll-title"
>
  <div class="modal-card">
    <div class="modal-header">
      <h2 id="coll-title" class="modal-title">
        Add to Collection — {record.title}
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
      <p class="body-desc">
        Organize this record into curated personal lists or franchises.
      </p>

      <div class="collections-list">
        {#each collections as c}
          <label class="collection-item" class:checked={selected.includes(c)}>
            <input
              type="checkbox"
              checked={selected.includes(c)}
              onchange={() => handleToggle(c)}
            />
            <IconFolder size={18} class="folder-icon" />
            <span class="collection-name">{c}</span>
          </label>
        {/each}
      </div>

      <!-- Add New Collection Form -->
      <form onsubmit={handleCreateCollection} class="new-coll-form">
        <input
          type="text"
          placeholder="+ Create new collection..."
          bind:value={newCollectionInput}
          class="new-coll-input"
          aria-label="New collection name"
        />
        <button type="submit" class="add-coll-btn">Add</button>
      </form>
    </div>

    <div class="modal-footer">
      <button type="button" class="btn-cancel" onclick={onClose}>Cancel</button>
      <button type="button" class="btn-save" onclick={handleSave}>
        <IconCheck size={16} /> Save Collections ({selected.length})
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
  }

  .modal-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 8px;
    width: 100%;
    max-width: 440px;
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
  }

  .body-desc {
    font-size: 0.88rem;
    color: var(--fasti-text-muted);
    margin: 0;
  }

  .collections-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 220px;
    overflow-y: auto;
  }

  .collection-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .collection-item.checked {
    border-color: var(--fasti-action-primary);
    background: color-mix(in srgb, var(--fasti-action-primary) 8%, transparent);
  }

  :global(.folder-icon) {
    color: var(--fasti-brand-gold);
  }
  .collection-name {
    font-size: 0.9rem;
    font-weight: 500;
  }

  .new-coll-form {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }

  .new-coll-input {
    flex: 1;
    height: 36px;
    padding: 6px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.88rem;
    background: var(--fasti-surface-paper);
  }

  .add-coll-btn {
    padding: 0 14px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-weight: 600;
    font-size: 0.85rem;
    cursor: pointer;
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
