<script lang="ts">
  import type { ActiveNavSection, NavItemConfig } from "./types.js";
  import { DEFAULT_NAV_ITEMS } from "./defaults.js";
  import {
    IconAdjustments,
    IconBook,
    IconBook2,
    IconBookmarks,
    IconBox,
    IconCalendar,
    IconChartBar,
    IconChevronLeft,
    IconChevronRight,
    IconClock,
    IconCompass,
    IconDatabase,
    IconDeviceGamepad2,
    IconDeviceTv,
    IconDice,
    IconEyeOff,
    IconFileText,
    IconHeadphones,
    IconHome,
    IconLibrary,
    IconListDetails,
    IconMicrophone,
    IconMovie,
    IconPin,
    IconPlugConnected,
    IconSettings,
    IconShieldCheck,
    IconStack2,
    IconTag,
    IconX,
  } from "@tabler/icons-svelte";

  interface Props {
    activeSection: ActiveNavSection;
    navItems?: NavItemConfig[];
    openReviewCount?: number;
    collapsed?: boolean;
    hidden?: boolean;
    narrowViewport?: boolean;
    mobileOpen?: boolean;
    onToggleCollapse?: () => void;
    onToggleHide?: () => void;
    onCloseMobile?: () => void;
    hrefForSection: (section: ActiveNavSection) => string;
    onSelectSection: (section: ActiveNavSection) => void;
  }

  let {
    activeSection,
    navItems = DEFAULT_NAV_ITEMS,
    openReviewCount = 0,
    collapsed = false,
    hidden = false,
    narrowViewport = false,
    mobileOpen = false,
    onToggleCollapse,
    onToggleHide,
    onCloseMobile,
    hrefForSection,
    onSelectSection,
  }: Props = $props();

  let sidebar: HTMLElement | undefined;

  const ICON_MAP: Record<string, any> = {
    home: IconHome,
    discover: IconCompass,
    tv_shows: IconDeviceTv,
    tv_seasons: IconStack2,
    movies: IconMovie,
    anime: IconDeviceTv,
    manga: IconBook2,
    games: IconDeviceGamepad2,
    books: IconBook,
    comics: IconBookmarks,
    board_games: IconDice,
    music: IconHeadphones,
    podcasts: IconMicrophone,
    calendar: IconCalendar,
    collection: IconBox,
    custom: IconAdjustments,
    history: IconClock,
    lists: IconListDetails,
    statistics: IconChartBar,
    tags: IconTag,
    reconciliation: IconShieldCheck,
    sources: IconDatabase,
    connections: IconPlugConnected,
    library: IconLibrary,
    detail: IconFileText,
    settings: IconSettings,
  };

  const visibleItems = $derived(
    [...navItems]
      .filter((item) => item.visible)
      .sort((a, b) => a.order - b.order),
  );
  const desktopCollapsed = $derived(collapsed && !narrowViewport);
  const navGroups = $derived.by(() => [
    {
      id: "pinned",
      label: "Pinned",
      items: visibleItems.filter((item) => item.pinned),
    },
    {
      id: "primary",
      label: "Main",
      items: visibleItems.filter(
        (item) => !item.pinned && item.category === "primary",
      ),
    },
    {
      id: "media",
      label: "Media",
      items: visibleItems.filter(
        (item) => !item.pinned && item.category === "media",
      ),
    },
    {
      id: "library",
      label: "Library",
      items: visibleItems.filter(
        (item) => !item.pinned && item.category === "library",
      ),
    },
    {
      id: "utilities",
      label: "Utilities",
      items: visibleItems.filter(
        (item) => !item.pinned && item.category === "utilities",
      ),
    },
  ]);

  function isCurrent(item: NavItemConfig): boolean {
    return activeSection === item.id;
  }

  function select(event: MouseEvent, item: NavItemConfig): void {
    if (
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    )
      return;
    event.preventDefault();
    onSelectSection(item.id);
  }

  function focusableControls(): HTMLElement[] {
    if (!sidebar) return [];
    return Array.from(
      sidebar.querySelectorAll<HTMLElement>(
        'a[href], button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    ).filter((element) => element.offsetParent !== null);
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (!narrowViewport || !mobileOpen) return;
    if (event.key === "Escape") {
      event.preventDefault();
      onCloseMobile?.();
      return;
    }
    if (event.key !== "Tab") return;
    const controls = focusableControls();
    const first = controls[0];
    const last = controls.at(-1);
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }
</script>

<div
  id="fasti-main-navigation"
  bind:this={sidebar}
  class="navbar navbar-vertical navbar-expand-lg offcanvas-lg offcanvas-start fasti-sidebar-vertical"
  class:show={mobileOpen}
  class:navbar-vertical-collapsed={desktopCollapsed}
  class:desktop-hidden={hidden}
  aria-label="Main navigation"
  aria-modal={narrowViewport && mobileOpen ? "true" : undefined}
  aria-hidden={narrowViewport && !mobileOpen ? "true" : undefined}
  role={narrowViewport ? "dialog" : "navigation"}
  inert={narrowViewport && !mobileOpen}
  onkeydown={handleKeydown}
>
  <div class="offcanvas-header">
    <span class="navbar-brand m-0">Fasti</span>
    <button
      type="button"
      class="btn btn-icon btn-ghost-secondary"
      onclick={onCloseMobile}
      aria-label="Close navigation"
    >
      <IconX size={20} />
    </button>
  </div>

  <div class="offcanvas-body">
    <div class="container-fluid flex-column align-items-stretch h-100">
      <div
        class="brand-header-row d-flex align-items-center justify-content-between px-2"
      >
        {#if !desktopCollapsed}
          <a
            class="navbar-brand brand-button"
            href="/"
            onclick={(event) => {
              event.preventDefault();
              onSelectSection("home");
            }}
            aria-label="Fasti home"
          >
            <svg
              class="brand-mark text-danger"
              viewBox="0 0 32 32"
              width="24"
              height="24"
              aria-hidden="true"
            >
              <rect
                x="4"
                y="4"
                width="4"
                height="24"
                fill="currentColor"
                opacity="0.4"
              />
              <rect
                x="12"
                y="4"
                width="4"
                height="24"
                fill="currentColor"
                opacity="0.7"
              />
              <rect x="20" y="4" width="4" height="24" fill="currentColor" />
              <rect x="28" y="4" width="4" height="24" fill="currentColor" />
            </svg>
            <span class="navbar-brand-title">Fasti</span>
          </a>
        {/if}

        <div class="desktop-sidebar-actions d-flex align-items-center gap-1">
          {#if !desktopCollapsed}
            <button
              type="button"
              class="btn btn-icon btn-sm btn-ghost-secondary"
              onclick={onToggleHide}
              title="Hide navigation"
              aria-label="Hide navigation"
            >
              <IconEyeOff size={16} />
            </button>
          {/if}
          <button
            type="button"
            class="btn btn-icon btn-sm btn-ghost-secondary sidebar-collapse-toggle"
            onclick={onToggleCollapse}
            title={desktopCollapsed
              ? "Expand navigation"
              : "Collapse navigation"}
            aria-label={desktopCollapsed
              ? "Expand navigation"
              : "Collapse navigation"}
          >
            {#if desktopCollapsed}
              <IconChevronRight size={16} />
            {:else}
              <IconChevronLeft size={16} />
            {/if}
          </button>
        </div>
      </div>

      <div class="navbar-collapse show flex-grow-1 overflow-y-auto">
        <ul class="navbar-nav pt-lg-1">
          {#each navGroups as group (group.id)}
            {#if group.items.length > 0}
              {#if !desktopCollapsed && group.id !== "primary"}
                <li class="nav-section-title">
                  <span class="section-label">
                    {#if group.id === "pinned"}
                      <IconPin size={12} aria-hidden="true" />
                    {/if}
                    {group.label}
                  </span>
                </li>
              {/if}
              {#each group.items as item (item.id)}
                {@const Icon = ICON_MAP[item.id] || IconDeviceTv}
                <li class="nav-item" class:active={isCurrent(item)}>
                  <a
                    class="nav-link"
                    class:active={isCurrent(item)}
                    href={hrefForSection(item.id)}
                    aria-current={isCurrent(item) ? "page" : undefined}
                    onclick={(event) => select(event, item)}
                    title={desktopCollapsed ? item.label : undefined}
                    aria-label={desktopCollapsed
                      ? item.id === "reconciliation" && openReviewCount > 0
                        ? `${item.label}, ${openReviewCount} open reviews`
                        : item.label
                      : undefined}
                  >
                    <span class="nav-link-icon">
                      <Icon size={18} aria-hidden="true" />
                    </span>
                    {#if !desktopCollapsed}
                      <span class="nav-link-title">{item.label}</span>
                      {#if item.id === "reconciliation" && openReviewCount > 0}
                        <span class="badge bg-warning text-dark ms-auto"
                          >{openReviewCount}</span
                        >
                      {/if}
                    {:else if item.id === "reconciliation" && openReviewCount > 0}
                      <span
                        class="badge bg-warning badge-dot position-absolute top-1 end-1"
                      ></span>
                    {/if}
                  </a>
                </li>
              {/each}
            {/if}
          {/each}
        </ul>
      </div>
    </div>
  </div>
</div>

{#if narrowViewport && mobileOpen}
  <button
    type="button"
    class="offcanvas-backdrop fade show navigation-backdrop"
    tabindex="-1"
    aria-label="Close navigation"
    onclick={onCloseMobile}
  ></button>
{/if}

<style>
  .fasti-sidebar-vertical {
    --tblr-offcanvas-width: min(20rem, 100vw);
    --tblr-offcanvas-transition: transform 200ms ease-in-out;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .offcanvas-body,
  .offcanvas-body > .container-fluid {
    min-height: 100%;
  }

  .offcanvas-body > .container-fluid {
    display: flex;
    padding: 0.75rem 0.5rem;
  }

  .brand-header-row {
    min-height: 3.25rem;
    margin-bottom: 0.75rem;
  }

  .brand-button {
    min-height: 44px;
    color: var(--fasti-text-primary);
    text-decoration: none;
  }

  .desktop-sidebar-actions :global(.btn-icon),
  .offcanvas-header :global(.btn-icon),
  .nav-link {
    min-width: 44px;
    min-height: 44px;
  }

  .nav-section-title {
    padding: 0.75rem 0.75rem 0.25rem;
  }

  .section-label {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    color: var(--fasti-text-muted);
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .nav-link {
    position: relative;
    gap: 0.5rem;
    color: var(--fasti-text-muted);
    font-size: 0.86rem;
    font-weight: 500;
  }

  .fasti-sidebar-vertical .navbar-nav .nav-link {
    min-height: 44px;
  }

  .nav-link:hover,
  .nav-link.active {
    color: var(--fasti-text-primary);
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 12%,
      transparent
    );
  }

  .nav-link.active {
    font-weight: 700;
  }

  .nav-link-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 1.5rem;
    margin: 0;
  }

  .nav-link-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .navigation-backdrop {
    position: fixed;
    inset: 0;
    z-index: calc(var(--tblr-offcanvas-zindex) - 1);
    width: 100%;
    height: 100%;
    padding: 0;
    border: 0;
  }

  @media (min-width: 62rem) {
    .fasti-sidebar-vertical.desktop-hidden {
      display: none;
    }

    .fasti-sidebar-vertical.navbar-vertical-collapsed {
      width: var(--fasti-collapsed-navigation-width);
    }

    .navbar-vertical-collapsed .brand-header-row {
      justify-content: center !important;
      padding-inline: 0 !important;
    }

    .navbar-vertical-collapsed .nav-link {
      justify-content: center;
      width: 44px;
      margin-inline: auto;
      padding-inline: 0;
    }
  }

  @media (max-width: 61.99rem) {
    .fasti-sidebar-vertical {
      flex-wrap: nowrap;
    }

    .offcanvas-header,
    .offcanvas-body {
      width: 100%;
    }

    .fasti-sidebar-vertical.show > .offcanvas-header,
    .fasti-sidebar-vertical.show > .offcanvas-body {
      visibility: visible;
    }

    .offcanvas-body {
      flex: 1 1 auto;
      min-height: 0;
      overflow-y: auto;
    }

    .offcanvas-body > .container-fluid,
    .navbar-collapse,
    .navbar-nav {
      width: 100%;
    }

    .brand-header-row {
      display: none !important;
    }
  }
</style>
