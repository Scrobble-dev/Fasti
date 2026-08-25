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
  let position = $state({ x: 0, y: 0 });

  function menuButtons(): HTMLButtonElement[] {
    return menuRef
      ? Array.from(
          menuRef.querySelectorAll<HTMLButtonElement>("[role=menuitem]"),
        )
      : [];
  }

  function handleKeyDown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      onClose();
      return;
    }

    const buttons = menuButtons();
    if (buttons.length === 0) return;
    const current = buttons.indexOf(
      document.activeElement as HTMLButtonElement,
    );
    let next = current;
    if (e.key === "ArrowDown") next = (current + 1) % buttons.length;
    else if (e.key === "ArrowUp")
      next = (current - 1 + buttons.length) % buttons.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = buttons.length - 1;
    else return;

    e.preventDefault();
    buttons[next]?.focus();
  }

  function clampToViewport(): void {
    if (!menuRef) return;
    const rect = menuRef.getBoundingClientRect();
    const inset = 8;
    position = {
      x: Math.max(inset, Math.min(x, window.innerWidth - rect.width - inset)),
      y: Math.max(inset, Math.min(y, window.innerHeight - rect.height - inset)),
    };
  }

  function handleResize(): void {
    clampToViewport();
  }

  function focusFirstItem(): void {
    const first = menuButtons()[0];
    if (first) {
      first.focus();
    }
  }

  function handleWindowClick(e: MouseEvent): void {
    if (menuRef && !menuRef.contains(e.target as Node)) {
      onClose();
    }
  }

  onMount(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    clampToViewport();
    focusFirstItem();
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("mousedown", handleWindowClick);
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("mousedown", handleWindowClick);
      window.removeEventListener("resize", handleResize);
      queueMicrotask(() => {
        if (
          document.activeElement === document.body &&
          previouslyFocused?.isConnected
        ) {
          previouslyFocused.focus();
        }
      });
    };
  });
</script>

<div
  bind:this={menuRef}
  class="dropdown-menu show shadow-lg border fasti-context-menu"
  style="position: fixed; top: {position.y}px; left: {position.x}px; z-index: 1050; display: block;"
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
