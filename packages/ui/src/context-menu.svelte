<script lang="ts">
  import { onMount } from "svelte";

  export interface ContextMenuItem {
    id: string;
    label: string;
    icon?: any;
    group?: string;
    description?: string;
    disabled?: boolean;
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
        ).filter((button) => !button.disabled)
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
  {#each items as item, index}
    {#if item.divider}
      <div class="dropdown-divider" role="separator"></div>
    {:else}
      {#if item.group && item.group !== items[index - 1]?.group}
        <div class="menu-group-label" role="presentation">{item.group}</div>
      {/if}
      <button
        type="button"
        class="dropdown-item d-flex align-items-center gap-2 py-2"
        class:text-danger={item.danger}
        disabled={item.disabled}
        onclick={() => {
          if (item.disabled) return;
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
        <span class="menu-item-copy">
          <span>{item.label}</span>
          {#if item.description}
            <small>{item.description}</small>
          {/if}
        </span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .fasti-context-menu {
    min-width: 260px;
    max-width: min(360px, calc(100vw - 16px));
    animation: fastiMenuFade 90ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .menu-group-label {
    padding: 8px 12px 4px;
    color: var(--fasti-text-muted);
    font-family: var(--fasti-font-mono);
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .menu-item-copy {
    display: flex;
    min-width: 0;
    flex-direction: column;
    align-items: flex-start;
    white-space: normal;
  }

  .menu-item-copy small {
    color: var(--fasti-text-muted);
    font-size: 0.72rem;
    line-height: 1.3;
  }

  .dropdown-item:disabled {
    cursor: not-allowed;
    opacity: 0.62;
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
