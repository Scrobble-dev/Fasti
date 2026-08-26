<script lang="ts">
  import { onMount } from "svelte";

  export interface ContextMenuItem {
    id: string;
    label: string;
    icon?: any;
    danger?: boolean;
    divider?: boolean;
    header?: string;
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

  const adjustedX = $derived.by(() => {
    if (typeof window !== "undefined") {
      const maxX = Math.max(10, window.innerWidth - 250);
      return Math.max(10, Math.min(x, maxX));
    }
    return x;
  });

  const adjustedY = $derived.by(() => {
    if (typeof window !== "undefined") {
      const maxY = Math.max(10, window.innerHeight - 420);
      return Math.max(10, Math.min(y, maxY));
    }
    return y;
  });

  function actionButtons(): HTMLButtonElement[] {
    if (!menuRef) return [];
    return Array.from(menuRef.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]'));
  }

  function focusItem(index: number): void {
    const buttons = actionButtons();
    if (buttons.length === 0) return;
    const target = (index + buttons.length) % buttons.length;
    buttons[target].focus();
  }

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      onClose();
      return;
    }

    const buttons = actionButtons();
    if (buttons.length === 0) return;
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement);

    if (e.key === "ArrowDown") {
      e.preventDefault();
      focusItem(current + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      focusItem(current - 1);
    } else if (e.key === "Home") {
      e.preventDefault();
      focusItem(0);
    } else if (e.key === "End") {
      e.preventDefault();
      focusItem(buttons.length - 1);
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
    focusItem(0);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("mousedown", handleWindowClick);
    };
  });
</script>

<div
  bind:this={menuRef}
  class="dropdown-menu show shadow-lg border fasti-context-menu"
  style="position: fixed; top: {adjustedY}px; left: {adjustedX}px; z-index: 1050; display: block;"
  role="menu"
  tabindex="-1"
>
  {#each items as item}
    {#if item.header}
      <h6
        class="dropdown-header text-uppercase text-muted font-monospace px-3 pt-2 pb-1"
        style="font-size: 0.65rem; letter-spacing: 0.06em;"
      >
        {item.header}
      </h6>
    {:else if item.divider}
      <div class="dropdown-divider" role="separator"></div>
    {:else}
      <button
        type="button"
        class="dropdown-item d-flex align-items-center gap-2 py-2 px-3"
        class:text-danger={item.danger}
        onclick={() => {
          item.action();
          onClose();
        }}
        role="menuitem"
      >
        {#if item.icon}
          {@const Icon = item.icon}
          <span class="dropdown-item-icon text-muted d-inline-flex align-items-center" aria-hidden="true">
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
    background: var(--fasti-surface-paper) !important;
    border-color: var(--fasti-border) !important;
    color: var(--fasti-text-primary) !important;
    animation: fastiMenuFade 90ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .dropdown-item {
    color: var(--fasti-text-primary) !important;
    min-height: 44px;
  }

  .dropdown-item:hover,
  .dropdown-item:focus-visible {
    background: var(--fasti-surface-archive) !important;
    color: var(--fasti-action-primary) !important;
  }

  .dropdown-item:focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: -3px;
  }

  .dropdown-divider {
    border-top-color: var(--fasti-border) !important;
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
