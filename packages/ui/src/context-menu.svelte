<script lang="ts">
  import { onMount } from "svelte";
  import {
    IconEye,
    IconBookmark,
    IconFolderPlus,
    IconStar,
    IconPlayerPlay,
    IconRefresh,
    IconGitPullRequest,
    IconCopy,
    IconTrash,
    IconCheck,
  } from "@tabler/icons-svelte";

  export interface ContextMenuItem {
    id: string;
    label: string;
    icon?: any;
    danger?: boolean;
    divider?: boolean;
    action: () => void;
  }

  interface Props {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onClose: () => void;
  }

  let { x, y, items, onClose }: Props = $props();

  let menuRef: HTMLDivElement | null = $state(null);

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      onClose();
    }
  }

  function handleWindowClick(e: MouseEvent): void {
    if (menuRef && !menuRef.contains(e.target as Node)) {
      onClose();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("mousedown", handleWindowClick);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("mousedown", handleWindowClick);
    };
  });
</script>

<div
  bind:this={menuRef}
  class="context-menu-popover"
  style="top: {y}px; left: {x}px;"
  role="menu"
  tabindex="-1"
>
  {#each items as item}
    {#if item.divider}
      <div class="menu-divider" role="separator"></div>
    {:else}
      <button
        type="button"
        class="menu-item-btn"
        class:danger={item.danger}
        onclick={() => {
          item.action();
          onClose();
        }}
        role="menuitem"
      >
        {#if item.icon}
          <item.icon size={16} class="item-icon" />
        {/if}
        <span class="item-label">{item.label}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .context-menu-popover {
    position: fixed;
    z-index: 10000;
    min-width: 200px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.16);
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: fadeIn 80ms ease-out;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .menu-divider {
    height: 1px;
    background: color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    margin: 4px 0;
  }

  .menu-item-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 12px;
    background: transparent;
    border: none;
    border-radius: 4px;
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--fasti-text-primary);
    cursor: pointer;
    text-align: left;
    transition: background 80ms ease;
  }

  .menu-item-btn:hover {
    background: var(--fasti-surface-archive);
    color: var(--fasti-action-primary);
  }

  .menu-item-btn.danger {
    color: #e11d48;
  }

  .menu-item-btn.danger:hover {
    background: rgba(225, 29, 72, 0.1);
  }

  :global(.item-icon) {
    color: var(--fasti-text-muted);
  }

  .menu-item-btn:hover :global(.item-icon) {
    color: currentColor;
  }

  .item-label {
    flex: 1;
  }
</style>
