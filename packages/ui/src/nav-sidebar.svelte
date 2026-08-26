<script lang="ts">
  import type { ActiveNavSection, NavItemConfig } from "./types.js";
  import { DEFAULT_NAV_ITEMS } from "./mock-data.js";
  import {
    IconHome,
    IconCompass,
    IconDeviceTv,
    IconStack2,
    IconMovie,
    IconPlayerPlay,
    IconBook2,
    IconDeviceGamepad2,
    IconBook,
    IconBookmarks,
    IconDice,
    IconHeadphones,
    IconMicrophone,
    IconCalendar,
    IconBox,
    IconAdjustments,
    IconClock,
    IconListDetails,
    IconChartBar,
    IconTag,
    IconShieldCheck,
    IconDatabase,
    IconSettings,
    IconChevronLeft,
    IconChevronRight,
    IconPin,
  } from "@tabler/icons-svelte";

  interface Props {
    activeSection: ActiveNavSection;
    navItems?: NavItemConfig[];
    openReviewCount?: number;
    collapsed?: boolean;
    hidden?: boolean;
    onToggleCollapse?: () => void;
    onToggleHide?: () => void;
    onSelectSection: (section: ActiveNavSection) => void;
  }

  let {
    activeSection,
    navItems = DEFAULT_NAV_ITEMS,
    openReviewCount = 3,
    collapsed = false,
    hidden = false,
    onToggleCollapse,
    onToggleHide,
    onSelectSection,
  }: Props = $props();

  const ICON_MAP: Record<string, any> = {
    home: IconHome,
    discover: IconCompass,
    tv_shows: IconDeviceTv,
    tv_seasons: IconStack2,
    movies: IconMovie,
    anime: IconPlayerPlay,
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
    settings: IconSettings,
  };

  const visibleItems = $derived(
    [...navItems].filter((i) => i.visible).sort((a, b) => a.order - b.order),
  );

  const pinnedItems = $derived(visibleItems.filter((i) => i.pinned));
  const primaryItems = $derived(
    visibleItems.filter((i) => !i.pinned && i.category === "primary"),
  );
  const mediaItems = $derived(
    visibleItems.filter((i) => !i.pinned && i.category === "media"),
  );
  const libraryItems = $derived(
    visibleItems.filter((i) => !i.pinned && i.category === "library"),
  );
  const utilityItems = $derived(
    visibleItems.filter((i) => !i.pinned && i.category === "utilities"),
  );

  function getItemIcon(id: string) {
    return ICON_MAP[id] || IconDeviceTv;
  }
</script>

{#if !hidden}
  <aside
    class="navbar navbar-vertical fasti-sidebar-vertical"
    class:navbar-vertical-collapsed={collapsed}
    aria-label="Main Navigation"
  >
    <div class="sidebar-inner-container d-flex flex-column h-100 p-1">
      <!-- Brand & Collapsible Controls Header -->
      <div
        class="brand-header-row d-flex align-items-center mb-1 px-1 py-1"
        class:justify-content-center={collapsed}
        class:justify-content-between={!collapsed}
      >
        {#if collapsed}
          <button
            type="button"
            class="btn btn-icon btn-ghost-secondary w-100 p-1 d-flex flex-column align-items-center justify-content-center brand-expand-trigger"
            onclick={onToggleCollapse}
            title="Expand sidebar"
            aria-label="Expand sidebar"
          >
            <svg
              class="brand-mark"
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
                rx="1"
                fill="#8B2E2A"
                opacity="0.45"
              />
              <rect
                x="12"
                y="4"
                width="4"
                height="24"
                rx="1"
                fill="#8B2E2A"
                opacity="0.75"
              />
              <rect x="20" y="4" width="4" height="24" rx="1" fill="#8B2E2A" />
              <circle cx="22" cy="16" r="3" fill="#D4AF37" />
            </svg>
          </button>
        {:else}
          <button
            type="button"
            class="btn p-0 border-0 bg-transparent text-reset d-flex align-items-center gap-2 brand-button"
            onclick={() => onSelectSection("home")}
            aria-label="Fasti Living Chronicle Home"
          >
            <svg
              class="brand-mark"
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
                rx="1"
                fill="#8B2E2A"
                opacity="0.45"
              />
              <rect
                x="12"
                y="4"
                width="4"
                height="24"
                rx="1"
                fill="#8B2E2A"
                opacity="0.75"
              />
              <rect x="20" y="4" width="4" height="24" rx="1" fill="#8B2E2A" />
              <circle cx="22" cy="16" r="3" fill="#D4AF37" />
            </svg>
            <div class="brand-text-block d-flex flex-column text-start">
              <span class="navbar-brand-title font-display fw-bold fs-4 lh-1"
                >Fasti</span
              >
              <span
                class="brand-subline text-uppercase text-muted font-monospace"
                style="font-size: 0.60rem; letter-spacing: 0.08em;"
                >Living Chronicle</span
              >
            </div>
          </button>

          <button
            type="button"
            class="btn btn-icon btn-sm btn-ghost-secondary collapse-toggle-btn"
            onclick={onToggleCollapse}
            title="Collapse sidebar"
            aria-label="Collapse sidebar"
          >
            <IconChevronLeft size={16} />
          </button>
        {/if}
      </div>

      <!-- Navigation Items List (Tabler .navbar-nav) -->
      <nav
        class="navbar-collapse show flex-grow-1 overflow-y-auto px-1 py-0"
        aria-label="Sections"
      >
        <ul class="navbar-nav d-flex flex-column gap-1 list-unstyled m-0 p-0">
          <!-- 1. Pinned Section (if any) -->
          {#if pinnedItems.length > 0}
            {#if !collapsed}
              <li class="nav-section-title px-2 pt-1 pb-0">
                <span
                  class="text-uppercase text-muted fw-bold d-flex align-items-center gap-1 font-monospace"
                  style="font-size: 0.65rem; letter-spacing: 0.06em;"
                >
                  <IconPin size={11} /> Pinned
                </span>
              </li>
            {/if}
            {#each pinnedItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center rounded-2"
                  class:active={activeSection === item.id ||
                    (item.id === "home" &&
                      (activeSection === "chronicle" ||
                        activeSection === "library"))}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-current={activeSection === item.id ? "page" : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center"
                  >
                    <Icon size={18} stroke={1.75} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate ms-2"
                      >{item.label}</span
                    >
                    {#if item.id === "reconciliation" && openReviewCount > 0}
                      <span
                        class="badge bg-warning text-dark ms-auto font-monospace px-1 py-0"
                        style="font-size: 0.72rem;">{openReviewCount}</span
                      >
                    {/if}
                  {:else if item.id === "reconciliation" && openReviewCount > 0}
                    <span
                      class="badge bg-warning badge-dot position-absolute top-1 end-1"
                    ></span>
                  {/if}
                </button>
              </li>
            {/each}
          {/if}

          <!-- 2. Primary Items (Home / Discover) -->
          {#if primaryItems.length > 0}
            {#each primaryItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center rounded-2"
                  class:active={activeSection === item.id ||
                    (item.id === "home" &&
                      (activeSection === "chronicle" ||
                        activeSection === "library"))}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-current={activeSection === item.id ? "page" : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center"
                  >
                    <Icon size={18} stroke={1.75} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate ms-2"
                      >{item.label}</span
                    >
                  {/if}
                </button>
              </li>
            {/each}
          {/if}

          <!-- 3. MEDIA Section -->
          {#if mediaItems.length > 0}
            {#if !collapsed}
              <li class="nav-section-title px-2 pt-2 pb-0">
                <span
                  class="text-uppercase text-muted fw-bold font-monospace"
                  style="font-size: 0.65rem; letter-spacing: 0.06em;"
                  >Media</span
                >
              </li>
            {/if}
            {#each mediaItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center rounded-2"
                  class:active={activeSection === item.id}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-current={activeSection === item.id ? "page" : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center"
                  >
                    <Icon size={18} stroke={1.75} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate ms-2"
                      >{item.label}</span
                    >
                  {/if}
                </button>
              </li>
            {/each}
          {/if}

          <!-- 4. LIBRARY Section -->
          {#if libraryItems.length > 0}
            {#if !collapsed}
              <li class="nav-section-title px-2 pt-2 pb-0">
                <span
                  class="text-uppercase text-muted fw-bold font-monospace"
                  style="font-size: 0.65rem; letter-spacing: 0.06em;"
                  >Library</span
                >
              </li>
            {/if}
            {#each libraryItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center rounded-2"
                  class:active={activeSection === item.id}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-current={activeSection === item.id ? "page" : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center"
                  >
                    <Icon size={18} stroke={1.75} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate ms-2"
                      >{item.label}</span
                    >
                  {/if}
                </button>
              </li>
            {/each}
          {/if}

          <!-- 5. UTILITIES Section -->
          {#if utilityItems.length > 0}
            {#if !collapsed}
              <li class="nav-section-title px-2 pt-2 pb-0">
                <span
                  class="text-uppercase text-muted fw-bold font-monospace"
                  style="font-size: 0.65rem; letter-spacing: 0.06em;"
                  >Utilities</span
                >
              </li>
            {/if}
            {#each utilityItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center rounded-2"
                  class:active={activeSection === item.id ||
                    (item.id === "sources" && activeSection === "connections")}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-current={activeSection === item.id ? "page" : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center"
                  >
                    <Icon size={18} stroke={1.75} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate ms-2"
                      >{item.label}</span
                    >
                    {#if item.id === "reconciliation" && openReviewCount > 0}
                      <span
                        class="badge bg-warning text-dark ms-auto font-monospace px-1 py-0"
                        style="font-size: 0.72rem;">{openReviewCount}</span
                      >
                    {/if}
                  {:else if item.id === "reconciliation" && openReviewCount > 0}
                    <span
                      class="badge bg-warning badge-dot position-absolute top-1 end-1"
                    ></span>
                  {/if}
                </button>
              </li>
            {/each}
          {/if}
        </ul>
      </nav>

      <!-- Sidebar Footer (Always pinned Settings & Studio in line with navigation) -->
      <footer
        class="sidebar-footer pt-1 pb-1 border-top border-opacity-10 px-1 d-flex flex-column gap-1"
      >
        <button
          type="button"
          class="nav-link w-100 text-start d-flex align-items-center rounded-2"
          class:active={activeSection === "settings"}
          onclick={() => onSelectSection("settings")}
          title={collapsed ? "Settings & Studio" : undefined}
          aria-label="Settings & Studio"
          aria-current={activeSection === "settings" ? "page" : undefined}
        >
          <span
            class="nav-link-icon d-inline-flex align-items-center justify-content-center"
          >
            <IconSettings size={18} stroke={1.75} />
          </span>
          {#if !collapsed}
            <span class="nav-link-title flex-grow-1 text-truncate ms-2"
              >Settings & Studio</span
            >
          {/if}
        </button>

        {#if collapsed}
          <button
            type="button"
            class="btn btn-icon btn-sm btn-ghost-secondary w-100 py-1"
            onclick={onToggleCollapse}
            title="Expand sidebar"
            aria-label="Expand sidebar"
          >
            <IconChevronRight size={16} />
          </button>
        {/if}
      </footer>
    </div>
  </aside>
{/if}

<style>
  .fasti-sidebar-vertical {
    position: relative !important;
    display: flex !important;
    flex-direction: column !important;
    height: 100vh !important;
    width: 220px !important;
    min-width: 220px !important;
    max-width: 220px !important;
    flex: 0 0 220px !important;
    background: var(--fasti-surface-paper);
    border-right: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    transition:
      width 150ms cubic-bezier(0.16, 1, 0.3, 1),
      min-width 150ms cubic-bezier(0.16, 1, 0.3, 1),
      max-width 150ms cubic-bezier(0.16, 1, 0.3, 1),
      flex-basis 150ms cubic-bezier(0.16, 1, 0.3, 1);
    flex-shrink: 0 !important;
    user-select: none;
    z-index: 20;
    overflow: hidden;
  }

  .fasti-sidebar-vertical.navbar-vertical-collapsed {
    width: 56px !important;
    min-width: 56px !important;
    max-width: 56px !important;
    flex: 0 0 56px !important;
    flex-shrink: 0 !important;
  }

  .font-display {
    font-family: var(--fasti-font-display, "Newsreader", Georgia, serif);
  }

  .brand-button {
    outline: none;
    transition: opacity 120ms ease;
  }

  .brand-button:hover,
  .brand-expand-trigger:hover {
    opacity: 0.85;
  }

  .brand-button:focus-visible,
  .brand-expand-trigger:focus-visible {
    outline: 3px solid var(--fasti-action-primary, #1e4fa3);
    outline-offset: 2px;
  }

  .nav-link {
    color: var(--fasti-text-muted);
    font-size: 0.84rem;
    font-weight: 500;
    min-height: 36px;
    padding: 6px 10px;
    border: none;
    background: transparent;
    cursor: pointer;
    position: relative;
    transition:
      background-color 100ms ease,
      color 100ms ease;
  }

  .nav-link:hover {
    background-color: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .nav-link:focus-visible {
    outline: 3px solid var(--fasti-action-primary, #1e4fa3) !important;
    outline-offset: 2px;
  }

  .nav-link.active {
    background-color: color-mix(
      in srgb,
      var(--fasti-action-primary, #1e4fa3) 12%,
      transparent
    ) !important;
    color: var(--fasti-action-primary, #1e4fa3) !important;
    font-weight: 600;
  }

  .nav-link.active::before {
    content: "";
    position: absolute;
    left: 0;
    top: 5px;
    bottom: 5px;
    width: 3px;
    background-color: var(--fasti-brand-mark, #8b2e2a);
    border-radius: 2px;
  }

  .navbar-vertical-collapsed .nav-link {
    justify-content: center;
    padding: 8px 0 !important;
  }

  .navbar-vertical-collapsed .nav-link-icon {
    margin: 0 !important;
  }

  .navbar-vertical-collapsed .nav-link.active::before {
    left: 2px;
  }
</style>
