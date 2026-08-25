<script lang="ts">
  import type { ActiveNavSection, NavItemConfig } from "./types.js";
  import { DEFAULT_NAV_ITEMS } from "./defaults.js";
  import {
    IconHome,
    IconCompass,
    IconDeviceTv,
    IconStack2,
    IconMovie,
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
    IconEyeOff,
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
    openReviewCount = 0,
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
    class="navbar navbar-vertical navbar-expand-lg fasti-sidebar-vertical"
    class:navbar-vertical-collapsed={collapsed}
    aria-label="Main Navigation"
  >
    <div
      class="container-fluid flex-column align-items-stretch px-2 py-3 h-100"
    >
      <!-- Brand & Collapsible Controls Header -->
      <div
        class="brand-header-row d-flex align-items-center justify-content-between mb-3 px-2"
      >
        {#if !collapsed}
          <button
            type="button"
            class="btn p-0 border-0 bg-transparent text-reset d-flex align-items-center gap-2 brand-button"
            onclick={() => onSelectSection("home")}
            aria-label="Fasti Home"
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
            <span class="navbar-brand-title fw-bold fs-3 tracking-tight"
              >Fasti</span
            >
          </button>
        {/if}

        <div class="d-flex align-items-center gap-1">
          {#if !collapsed}
            <button
              type="button"
              class="btn btn-icon btn-sm btn-ghost-secondary"
              onclick={onToggleHide}
              title="Hide sidebar"
              aria-label="Hide sidebar"
            >
              <IconEyeOff size={16} />
            </button>
          {/if}
          <button
            type="button"
            class="btn btn-icon btn-sm btn-ghost-secondary"
            onclick={onToggleCollapse}
            title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {#if collapsed}
              <IconChevronRight size={16} />
            {:else}
              <IconChevronLeft size={16} />
            {/if}
          </button>
        </div>
      </div>

      <!-- Navigation Items List (Tabler .navbar-nav) -->
      <div class="navbar-collapse show flex-grow-1 overflow-y-auto">
        <ul class="navbar-nav pt-lg-1 d-flex flex-column gap-1">
          <!-- 1. Pinned Section (if any) -->
          {#if pinnedItems.length > 0}
            {#if !collapsed}
              <li class="nav-section-title">
                <span
                  class="text-uppercase text-muted fw-bold d-flex align-items-center gap-1 fs-6"
                >
                  <IconPin size={12} /> Pinned
                </span>
              </li>
            {/if}
            {#each pinnedItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center py-2 px-2 rounded"
                  class:active={activeSection === item.id ||
                    (item.id === "home" &&
                      (activeSection === "chronicle" ||
                        activeSection === "library"))}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-label={collapsed
                    ? item.id === "reconciliation" && openReviewCount > 0
                      ? `${item.label}, ${openReviewCount} open reviews`
                      : item.label
                    : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center me-2"
                  >
                    <Icon size={18} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate"
                      >{item.label}</span
                    >
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
                  class="nav-link w-100 text-start d-flex align-items-center py-2 px-2 rounded"
                  class:active={activeSection === item.id ||
                    (item.id === "home" &&
                      (activeSection === "chronicle" ||
                        activeSection === "library"))}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-label={collapsed ? item.label : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center me-2"
                  >
                    <Icon size={18} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate"
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
              <li class="nav-section-title mt-2">
                <span
                  class="text-uppercase text-muted fw-bold font-monospace fs-6"
                  >Media</span
                >
              </li>
            {/if}
            {#each mediaItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center py-2 px-2 rounded"
                  class:active={activeSection === item.id}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-label={collapsed ? item.label : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center me-2"
                  >
                    <Icon size={18} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate"
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
              <li class="nav-section-title mt-2">
                <span
                  class="text-uppercase text-muted fw-bold font-monospace fs-6"
                  >Library</span
                >
              </li>
            {/if}
            {#each libraryItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center py-2 px-2 rounded"
                  class:active={activeSection === item.id}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-label={collapsed ? item.label : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center me-2"
                  >
                    <Icon size={18} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate"
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
              <li class="nav-section-title mt-2">
                <span
                  class="text-uppercase text-muted fw-bold font-monospace fs-6"
                  >Utilities</span
                >
              </li>
            {/if}
            {#each utilityItems as item (item.id)}
              {@const Icon = getItemIcon(item.id)}
              <li class="nav-item">
                <button
                  type="button"
                  class="nav-link w-100 text-start d-flex align-items-center py-2 px-2 rounded"
                  class:active={activeSection === item.id ||
                    (item.id === "sources" && activeSection === "connections")}
                  onclick={() => onSelectSection(item.id)}
                  title={collapsed ? item.label : undefined}
                  aria-label={collapsed
                    ? item.id === "reconciliation" && openReviewCount > 0
                      ? `${item.label}, ${openReviewCount} open reviews`
                      : item.label
                    : undefined}
                >
                  <span
                    class="nav-link-icon d-inline-flex align-items-center justify-content-center me-2"
                  >
                    <Icon size={18} />
                  </span>
                  {#if !collapsed}
                    <span class="nav-link-title flex-grow-1 text-truncate"
                      >{item.label}</span
                    >
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
                </button>
              </li>
            {/each}
          {/if}
        </ul>
      </div>
    </div>
  </aside>
{/if}

<style>
  .fasti-sidebar-vertical {
    position: static !important;
    width: 240px;
    background: var(--fasti-surface-paper);
    border-right: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    transition: width 150ms cubic-bezier(0.16, 1, 0.3, 1);
    flex-shrink: 0;
  }

  .fasti-sidebar-vertical.navbar-vertical-collapsed {
    width: 64px;
  }

  .brand-button:hover .navbar-brand-title {
    color: var(--fasti-action-primary);
  }

  .brand-button,
  .brand-header-row :global(.btn-icon),
  .nav-link {
    min-height: 44px !important;
  }

  .brand-header-row :global(.btn-icon) {
    min-width: 44px !important;
  }

  .nav-section-title {
    padding: 6px 8px 2px;
    font-size: 0.7rem;
    letter-spacing: 0.06em;
  }

  .nav-link {
    position: relative;
    color: var(--fasti-text-muted);
    font-size: 0.86rem;
    font-weight: 500;
    transition:
      background-color 100ms ease,
      color 100ms ease;
    border: none;
    background: transparent;
    cursor: pointer;
  }

  .nav-link:hover {
    background-color: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .nav-link.active {
    background-color: color-mix(
      in srgb,
      var(--fasti-action-primary) 14%,
      transparent
    ) !important;
    color: var(--fasti-action-primary) !important;
    font-weight: 600;
  }

  .navbar-vertical-collapsed .nav-link {
    justify-content: center;
    padding: 10px 0 !important;
  }

  .navbar-vertical-collapsed .nav-link-icon {
    margin-right: 0 !important;
  }

  @media (max-width: 47.99rem) {
    .fasti-sidebar-vertical {
      position: fixed !important;
      inset: 0 auto 0 0;
      z-index: 30;
      box-shadow: 4px 0 16px rgba(0, 0, 0, 0.12);
    }
  }
</style>
