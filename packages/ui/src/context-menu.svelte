<script lang="ts">
  import { onMount } from "svelte";

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
  class="dropdown-menu show shadow-lg border fasti-context-menu"
  style="position: fixed; top: {y}px; left: {x}px; z-index: 1050; display: block;"
  role="menu"
  tabindex="-1"
>
  {#each items as item}
    {#if item.divider}
      <div class="dropdown-divider" role="separator"></div>
    {:else}
      <button
        type="button"
        class="dropdown-item d-flex align-items-center gap-2 py-2"
        class:text-danger={item.danger}
        onclick={() => {
          item.action();
          onClose();
        }}
        role="menuitem"
      >
        {#if item.icon}
          {@const Icon = item.icon}
          <span
            class="dropdown-item-icon text-muted d-inline-flex align-items-center"
          >
            <Icon size={16} />
          </span>
        {/if}
        <span>{item.label}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .fasti-context-menu {
    min-width: 210px;
    animation: fastiMenuFade 90ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes fastiMenuFade {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .fasti-context-menu {
      animation: none !important;
    }
  }
</style>
