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
    collections: string[];
    onClose: () => void;
    onSaveCollection: (recordId: string, collectionNames: string[]) => void;
  }

  let { record, collections, onClose, onSaveCollection }: Props = $props();

  let selected = $state<string>();
  let dialog: HTMLDialogElement | undefined;
  let availableCollections = $state<string[]>([]);
  let syncedRecordId = $state("");

  $effect(() => {
    availableCollections = [...collections];
  });

  $effect(() => {
    if (record.id !== syncedRecordId) {
      syncedRecordId = record.id;
      selected = record.collectionName;
    }
  });

  $effect(() => {
    if (!dialog?.open) dialog?.showModal();
  });
  let newCollectionInput = $state("");

  function handleSelect(name: string): void {
    selected = name;
  }

  function handleCreateCollection(e: Event): void {
    e.preventDefault();
    if (
      newCollectionInput.trim().length > 0 &&
      !availableCollections.includes(newCollectionInput.trim())
    ) {
      availableCollections = [
        ...availableCollections,
        newCollectionInput.trim(),
      ];
      selected = newCollectionInput.trim();
      newCollectionInput = "";
    }
  }

  function handleSave(): void {
    onSaveCollection(record.id, selected ? [selected] : []);
    onClose();
  }
</script>

<dialog
  bind:this={dialog}
  class="modal-backdrop"
  aria-labelledby="coll-title"
  oncancel={onClose}
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
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
        Organize this record in one curated personal list or franchise.
      </p>

      <div class="collections-list">
        <label class="collection-item" class:checked={!selected}>
          <input
            type="radio"
            name="collection"
            checked={!selected}
            onchange={() => (selected = undefined)}
          />
          <IconFolder size={18} class="folder-icon" />
          <span class="collection-name">No collection</span>
        </label>
        {#each availableCollections as c}
          <label class="collection-item" class:checked={selected === c}>
            <input
              type="radio"
              name="collection"
              checked={selected === c}
              onchange={() => handleSelect(c)}
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
        <IconCheck size={16} /> Save Collection
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
    border-radius: calc(8px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-size: 0.88rem;
    background: var(--fasti-surface-paper);
  }

  .add-coll-btn {
    padding: 0 14px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-weight: 600;
    cursor: pointer;
  }

  .btn-save {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 18px;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    border: none;
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    font-weight: 600;
    cursor: pointer;
  }
</style>
