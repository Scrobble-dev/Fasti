<script lang="ts">
  import { onMount } from "svelte";
  import type {
    ActiveNavSection,
    MediaKind,
    MediaRecord,
    WatchStatus,
    ChronicleOccurrence,
    ReconciliationCase,
    ProviderApiKeyConfig,
    OidcConfiguration,
    AppriseNotificationConfig,
    ThemeSettings,
    WorkbenchPreferences,
  } from "./types.js";
  import {
    DEFAULT_THEME_SETTINGS,
    DEFAULT_WORKBENCH_PREFERENCES,
  } from "./defaults.js";
  import NavSidebar from "./nav-sidebar.svelte";
  import HomeView from "./home-view.svelte";
  import ChronicleView from "./chronicle-view.svelte";
  import DiscoverView from "./discover-view.svelte";
  import LibraryView from "./library-view.svelte";
  import MediaDetailView from "./media-detail-view.svelte";
  import ReconciliationView from "./reconciliation-view.svelte";
  import CalendarView from "./calendar-view.svelte";
  import ConnectionsView from "./connections-view.svelte";
  import SettingsView from "./settings-view.svelte";
  import TablerThemeDrawer from "./tabler-theme-drawer.svelte";
  import {
    IconSearch,
    IconMoon,
    IconSun,
    IconCircleCheck,
    IconLayoutSidebar,
    IconAlertCircle,
    IconLoader2,
  } from "@tabler/icons-svelte";

  let activeSection: ActiveNavSection = $state("home");
  let records = $state<MediaRecord[]>([]);
  let chronicle = $state<ChronicleOccurrence[]>([]);
  let reconciliationCases = $state<ReconciliationCase[]>([]);
  let tokens = $state([]);
  let providerKeys = $state<ProviderApiKeyConfig[]>([]);
  let oidcConfig = $state<OidcConfiguration>({
    enabled: false,
    issuerUrl: "",
    clientId: "",
    clientSecret: "",
    redirectUri: "",
    autoProvisionUsers: false,
  });
  let appriseConfig = $state<AppriseNotificationConfig>({
    enabled: false,
    urls: [],
    notifyOnReviewRequired: false,
    notifyOnSyncError: false,
    notifyOnMilestone: false,
  });
  let themeSettings = $state<ThemeSettings>(DEFAULT_THEME_SETTINGS);
  let workbenchPreferences = $state<WorkbenchPreferences>(
    DEFAULT_WORKBENCH_PREFERENCES,
  );
  let selectedRecordId = $state<string | null>(null);
  let themeDrawerOpen = $state(false);
  let searchQuery = $state("");
  let mediaScope = $state("all");
  let nodeHealthy = $state<boolean | null>(null);

  const selectedRecord = $derived(
    records.find((r) => r.id === selectedRecordId),
  );
  const watchingRecords = $derived(
    records.filter((r) => r.status === "watching"),
  );
  const availableCollections = $derived(
    [
      ...new Set(records.flatMap((record) => record.collectionName ?? [])),
    ].sort(),
  );
  const openReviewCount = $derived(
    reconciliationCases.filter((c) => c.status === "open").length,
  );

  // Filtered records based on active section
  const filteredSectionRecords = $derived.by(() => {
    let list = records;
    if (activeSection === "tv_shows" || activeSection === "tv_seasons") {
      list = records.filter((r) => r.mediaKind === "show");
    } else if (activeSection === "movies") {
      list = records.filter((r) => r.mediaKind === "movie");
    } else if (activeSection === "anime") {
      list = records.filter((r) => r.mediaKind === "anime");
    } else if (activeSection === "manga") {
      list = records.filter((r) => r.mediaKind === "manga");
    } else if (activeSection === "games" || activeSection === "board_games") {
      list = records.filter((r) => r.mediaKind === "game");
    } else if (activeSection === "books") {
      list = records.filter((r) => r.mediaKind === "book");
    } else if (activeSection === "comics") {
      list = records.filter((r) => r.mediaKind === "comic");
    } else if (activeSection === "podcasts") {
      list = records.filter((r) => r.mediaKind === "podcast");
    } else if (activeSection === "music") {
      list = records.filter((r) => r.mediaKind === "music");
    } else if (activeSection === "collection") {
      list = records.filter((r) => !!r.collectionName);
    }

    if (mediaScope !== "all") {
      const scopeKinds: Record<string, MediaKind> = {
        shows: "show",
        movies: "movie",
        anime: "anime",
        games: "game",
        books: "book",
      };
      const kind = scopeKinds[mediaScope];
      list = kind ? list.filter((record) => record.mediaKind === kind) : [];
    }

    if (searchQuery.trim().length > 0) {
      const q = searchQuery.toLowerCase();
      list = list.filter(
        (r) =>
          r.title.toLowerCase().includes(q) ||
          r.overview?.toLowerCase().includes(q) ||
          r.tags?.some((t) => t.toLowerCase().includes(q)),
      );
    }
    return list;
  });

  // --- 1. HTML5 History API URL Routing ---
  function sectionToPath(
    section: ActiveNavSection,
    recordId?: string | null,
  ): string {
    switch (section) {
      case "home":
        return "/";
      case "discover":
        return "/discover";
      case "tv_shows":
        return "/media/shows";
      case "tv_seasons":
        return "/media/seasons";
      case "movies":
        return "/media/movies";
      case "anime":
        return "/media/anime";
      case "manga":
        return "/media/manga";
      case "games":
        return "/media/games";
      case "books":
        return "/media/books";
      case "comics":
        return "/media/comics";
      case "board_games":
        return "/media/board-games";
      case "music":
        return "/media/music";
      case "podcasts":
        return "/media/podcasts";
      case "calendar":
        return "/calendar";
      case "collection":
        return "/collection";
      case "custom":
        return "/custom";
      case "history":
      case "chronicle":
        return "/history";
      case "lists":
        return "/lists";
      case "statistics":
        return "/statistics";
      case "tags":
        return "/tags";
      case "reconciliation":
        return "/review-inbox";
      case "sources":
      case "connections":
        return "/connections";
      case "settings":
        return "/settings";
      case "detail":
        return recordId ? `/records/${recordId}` : "/";
      default:
        return "/";
    }
  }

  function syncFromUrl(): void {
    if (typeof window === "undefined") return;
    const pathname = window.location.pathname;

    if (pathname.startsWith("/records/")) {
      const recId = pathname.replace("/records/", "").trim();
      const rec = records.find((r) => r.id === recId);
      if (rec) {
        selectedRecordId = recId;
        activeSection = "detail";
        return;
      }
    }

    if (pathname === "/discover") {
      activeSection = "discover";
      selectedRecordId = null;
    } else if (pathname === "/media/shows") {
      activeSection = "tv_shows";
      selectedRecordId = null;
    } else if (pathname === "/media/seasons") {
      activeSection = "tv_seasons";
      selectedRecordId = null;
    } else if (pathname === "/media/movies") {
      activeSection = "movies";
      selectedRecordId = null;
    } else if (pathname === "/media/anime") {
      activeSection = "anime";
      selectedRecordId = null;
    } else if (pathname === "/media/manga") {
      activeSection = "manga";
      selectedRecordId = null;
    } else if (pathname === "/media/games") {
      activeSection = "games";
      selectedRecordId = null;
    } else if (pathname === "/media/board-games") {
      activeSection = "board_games";
      selectedRecordId = null;
    } else if (pathname === "/media/books") {
      activeSection = "books";
      selectedRecordId = null;
    } else if (pathname === "/media/comics") {
      activeSection = "comics";
      selectedRecordId = null;
    } else if (pathname === "/media/podcasts") {
      activeSection = "podcasts";
      selectedRecordId = null;
    } else if (pathname === "/media/music") {
      activeSection = "music";
      selectedRecordId = null;
    } else if (pathname === "/calendar") {
      activeSection = "calendar";
      selectedRecordId = null;
    } else if (pathname === "/collection") {
      activeSection = "collection";
      selectedRecordId = null;
    } else if (pathname === "/custom") {
      activeSection = "custom";
      selectedRecordId = null;
    } else if (pathname === "/history" || pathname === "/chronicle") {
      activeSection = "history";
      selectedRecordId = null;
    } else if (pathname === "/lists") {
      activeSection = "lists";
      selectedRecordId = null;
    } else if (pathname === "/statistics") {
      activeSection = "statistics";
      selectedRecordId = null;
    } else if (pathname === "/tags") {
      activeSection = "tags";
      selectedRecordId = null;
    } else if (pathname === "/review-inbox" || pathname === "/reconciliation") {
      activeSection = "reconciliation";
      selectedRecordId = null;
    } else if (pathname === "/connections" || pathname === "/sources") {
      activeSection = "connections";
      selectedRecordId = null;
    } else if (pathname === "/settings") {
      activeSection = "settings";
      selectedRecordId = null;
    } else {
      activeSection = "home";
      selectedRecordId = null;
      if (pathname !== "/") window.history.replaceState({}, "", "/");
    }
  }

  function handleSelectSection(section: ActiveNavSection): void {
    activeSection = section;
    if (section !== "detail") {
      selectedRecordId = null;
    }
    if (typeof window !== "undefined") {
      const path = sectionToPath(section);
      window.history.pushState({}, "", path);
    }
  }

  function handleSelectRecord(recordId: string): void {
    selectedRecordId = recordId;
    activeSection = "detail";
    if (typeof window !== "undefined") {
      window.history.pushState({}, "", `/records/${recordId}`);
    }
  }

  function handleBackToLibrary(): void {
    activeSection = "home";
    selectedRecordId = null;
    if (typeof window !== "undefined") {
      window.history.pushState({}, "", "/");
    }
  }

  // --- 2. Tabler Live Theme Engine ---
  $effect(() => {
    if (typeof document === "undefined") return;
    const mode = themeSettings.mode;
    const isDark = mode === "dark" || mode === "night";

    document.documentElement.setAttribute(
      "data-bs-theme",
      isDark ? "dark" : "light",
    );
    document.documentElement.setAttribute("data-fasti-mode", mode);
    document.body.classList.add("layout-fluid");
    document.body.classList.remove("theme-light", "theme-dark", "theme-night");
    document.body.classList.add(`theme-${mode}`);

    const accent = themeSettings.accentColor || "#066fd1";
    document.documentElement.style.setProperty("--tblr-primary", accent);
    document.documentElement.style.setProperty(
      "--fasti-action-primary",
      accent,
    );

    if (themeSettings.cornerRadius !== undefined) {
      document.documentElement.style.setProperty(
        "--tblr-border-radius",
        `${themeSettings.cornerRadius * 4}px`,
      );
    }

    if (themeSettings.fontFamily) {
      const font =
        themeSettings.fontFamily === "serif"
          ? '"Newsreader", Georgia, serif'
          : themeSettings.fontFamily === "monospace"
            ? '"IBM Plex Mono", monospace'
            : '"Atkinson Hyperlegible", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif';
      document.documentElement.style.setProperty("--fasti-font-body", font);
    }
  });

  async function checkNodeHealth(): Promise<void> {
    if (typeof fetch === "undefined") return;
    try {
      const res = await fetch("/api/v1/health", {
        signal: AbortSignal.timeout(3000),
      });
      nodeHealthy = res.ok;
    } catch {
      nodeHealthy = false;
    }
  }

  onMount(() => {
    syncFromUrl();
    void checkNodeHealth();
    const healthInterval = window.setInterval(() => {
      void checkNodeHealth();
    }, 30_000);
    window.addEventListener("popstate", syncFromUrl);
    return () => {
      window.clearInterval(healthInterval);
      window.removeEventListener("popstate", syncFromUrl);
    };
  });

  function handleUpdateStatus(recordId: string, newStatus: WatchStatus): void {
    records = records.map((r) =>
      r.id === recordId ? { ...r, status: newStatus } : r,
    );

    if (newStatus === "completed") {
      const rec = records.find((r) => r.id === recordId);
      if (rec) {
        const newOcc: ChronicleOccurrence = {
          id: `occ_${Date.now()}`,
          recordId: rec.id,
          title: rec.title,
          mediaKind: rec.mediaKind,
          posterUrl: rec.posterUrl,
          timestamp: new Date().toISOString(),
          progressPercentage: 100,
          durationMinutes: rec.runtimeMinutes ?? 45,
          deviceName: "Fasti Workbench Web",
          clientName: "Manual Quick Action",
          isRewatch: false,
          userRating: rec.userRating,
        };
        chronicle = [newOcc, ...chronicle];
      }
    }
  }

  function handleUpdateRating(recordId: string, newRating: number): void {
    records = records.map((r) =>
      r.id === recordId ? { ...r, userRating: newRating } : r,
    );
  }

  function handleUpdateProgress(
    recordId: string,
    episodes: number,
    seconds: number,
    status: WatchStatus,
  ): void {
    records = records.map((r) =>
      r.id === recordId
        ? {
            ...r,
            progressEpisodes: episodes,
            progressSeconds: seconds,
            status,
          }
        : r,
    );
  }

  function handleSaveReview(
    recordId: string,
    rating: number,
    notes: string,
  ): void {
    records = records.map((r) =>
      r.id === recordId
        ? {
            ...r,
            userRating: rating,
            userNotes: notes,
          }
        : r,
    );
  }

  function handleSaveCollection(
    recordId: string,
    collectionNames: string[],
  ): void {
    records = records.map((r) =>
      r.id === recordId
        ? {
            ...r,
            collectionName:
              collectionNames.length > 0 ? collectionNames[0] : undefined,
          }
        : r,
    );
  }

  function handleToggleEpisode(recordId: string, episodeId: string): void {
    records = records.map((r) => {
      if (r.id !== recordId || !r.seasons) return r;
      const updatedSeasons = r.seasons.map((s) => ({
        ...s,
        episodes: s.episodes.map((ep) =>
          ep.id === episodeId
            ? {
                ...ep,
                watched: !ep.watched,
                watchedAt: !ep.watched ? new Date().toISOString() : undefined,
              }
            : ep,
        ),
      }));
      const watchedCount = updatedSeasons.reduce(
        (acc, s) => acc + s.episodes.filter((e) => e.watched).length,
        0,
      );
      return { ...r, seasons: updatedSeasons, progressEpisodes: watchedCount };
    });
  }

  function handleUpdateNotes(recordId: string, notes: string): void {
    records = records.map((r) =>
      r.id === recordId ? { ...r, userNotes: notes } : r,
    );
  }

  function handleAddTag(recordId: string, tag: string): void {
    records = records.map((r) =>
      r.id === recordId && !r.tags.includes(tag)
        ? { ...r, tags: [...r.tags, tag] }
        : r,
    );
  }

  function handleRemoveTag(recordId: string, tag: string): void {
    records = records.map((r) =>
      r.id === recordId ? { ...r, tags: r.tags.filter((t) => t !== tag) } : r,
    );
  }

  function handleUpdateTheme(updates: Partial<ThemeSettings>): void {
    themeSettings = { ...themeSettings, ...updates };
  }
</script>

<div
  class="fasti-workbench-layout theme-{themeSettings.mode} density-{themeSettings.density}"
>
  <NavSidebar
    {activeSection}
    navItems={workbenchPreferences.navItems}
    {openReviewCount}
    collapsed={workbenchPreferences.sidebarCollapsed}
    hidden={workbenchPreferences.sidebarHidden}
    onToggleCollapse={() =>
      (workbenchPreferences = {
        ...workbenchPreferences,
        sidebarCollapsed: !workbenchPreferences.sidebarCollapsed,
      })}
    onToggleHide={() =>
      (workbenchPreferences = {
        ...workbenchPreferences,
        sidebarHidden: !workbenchPreferences.sidebarHidden,
      })}
    onSelectSection={handleSelectSection}
  />

  <div class="workbench-main-shell">
    <!-- Top Bar Header (Search, Scope, View Mode, Filters, Theme Drawer Toggle) -->
    <header
      class="top-nav-bar"
      role="toolbar"
      aria-label="Global workbench toolbar"
    >
      <div
        class="d-flex align-items-center gap-2 flex-grow-1"
        style="max-width: 480px;"
      >
        {#if workbenchPreferences.sidebarHidden}
          <button
            type="button"
            class="btn btn-icon btn-outline-secondary btn-sm"
            onclick={() =>
              (workbenchPreferences = {
                ...workbenchPreferences,
                sidebarHidden: false,
              })}
            title="Show sidebar menu"
            aria-label="Show sidebar menu"
          >
            <IconLayoutSidebar size={18} />
          </button>
        {/if}

        <div class="search-field-wrapper">
          <IconSearch size={18} class="search-icon" />
          <input
            type="search"
            class="global-search-input"
            placeholder="Search your media..."
            bind:value={searchQuery}
            aria-label="Search media collection"
          />
        </div>
      </div>

      <div class="top-actions-right">
        <!-- Media Scope Select -->
        <select
          class="scope-select"
          bind:value={mediaScope}
          aria-label="Filter media scope"
        >
          <option value="all">All media</option>
          <option value="shows">TV Shows</option>
          <option value="movies">Movies</option>
          <option value="anime">Anime</option>
          <option value="games">Games</option>
          <option value="books">Books</option>
        </select>

        <!-- Theme Settings Drawer Trigger -->
        <button
          type="button"
          class="tool-btn icon-only"
          onclick={() => (themeDrawerOpen = true)}
          title="Open theme customizer"
          aria-label="Open theme customizer"
        >
          {#if themeSettings.mode === "light"}
            <IconSun size={18} />
          {:else}
            <IconMoon size={18} />
          {/if}
        </button>

        <!-- Node Health Status Indicator (WCAG 1.4.1 & EN 301 549 Multi-sensory representation) -->
        <div
          class="status-indicator"
          role="status"
          aria-live="polite"
          title={nodeHealthy === true
            ? "Local node active and verified"
            : nodeHealthy === false
              ? "Local node unreachable / uninitialized"
              : "Connecting to local node..."}
        >
          {#if nodeHealthy === true}
            <span
              class="d-inline-flex align-items-center gap-1 text-success"
              title="Verified"
            >
              <IconCircleCheck size={18} stroke={2.5} />
              <span class="visually-hidden">Local node verified</span>
            </span>
          {:else if nodeHealthy === false}
            <span
              class="d-inline-flex align-items-center gap-1 text-danger"
              title="Unreachable"
            >
              <IconAlertCircle size={18} stroke={2} />
              <span class="visually-hidden">Local node unreachable</span>
            </span>
          {:else}
            <span
              class="d-inline-flex align-items-center gap-1 text-warning"
              title="Connecting..."
            >
              <IconLoader2 size={18} stroke={2} class="spin" />
              <span class="visually-hidden">Connecting to local node</span>
            </span>
          {/if}
        </div>
      </div>
    </header>

    <!-- Main Viewport Canvas -->
    <main class="viewport-canvas" id="main-content">
      {#if activeSection === "home"}
        {#if records.length === 0}
          <section class="empty-workbench" aria-labelledby="empty-title">
            <h1 id="empty-title">No media records</h1>
            <p>
              This review build has no active media ingest capability. It does
              not load sample records.
            </p>
          </section>
        {:else}
          <HomeView
            {records}
            {availableCollections}
            contextMenuConfigs={workbenchPreferences.contextMenuItems}
            onSelectRecord={handleSelectRecord}
            onUpdateStatus={handleUpdateStatus}
            onUpdateProgress={handleUpdateProgress}
            onSaveReview={handleSaveReview}
            onSaveCollection={handleSaveCollection}
            onViewAllSection={(sec) => handleSelectSection(sec as any)}
          />
        {/if}
      {:else if activeSection === "discover"}
        <DiscoverView
          trendingRecords={[]}
          {availableCollections}
          contextMenuConfigs={workbenchPreferences.contextMenuItems}
          onSelectRecord={handleSelectRecord}
          onUpdateStatus={handleUpdateStatus}
          onUpdateProgress={handleUpdateProgress}
          onSaveReview={handleSaveReview}
          onSaveCollection={handleSaveCollection}
        />
      {:else if activeSection === "detail" && selectedRecord}
        <MediaDetailView
          record={selectedRecord}
          {availableCollections}
          occurrences={chronicle}
          onBack={handleBackToLibrary}
          onUpdateStatus={handleUpdateStatus}
          onUpdateRating={handleUpdateRating}
          onToggleEpisode={handleToggleEpisode}
          onUpdateProgress={handleUpdateProgress}
          onSaveReview={handleSaveReview}
          onSaveCollection={handleSaveCollection}
          onUpdateNotes={handleUpdateNotes}
          onAddTag={handleAddTag}
          onRemoveTag={handleRemoveTag}
        />
      {:else if activeSection === "history" || activeSection === "chronicle"}
        <ChronicleView
          occurrences={chronicle}
          onSelectRecord={handleSelectRecord}
        />
      {:else if activeSection === "calendar"}
        <CalendarView {watchingRecords} onSelectRecord={handleSelectRecord} />
      {:else if activeSection === "reconciliation"}
        <ReconciliationView cases={reconciliationCases} />
      {:else if activeSection === "connections" || activeSection === "sources"}
        <ConnectionsView />
      {:else if activeSection === "settings"}
        <SettingsView
          customFields={[]}
          {tokens}
          {providerKeys}
          {oidcConfig}
          {appriseConfig}
          {themeSettings}
          {workbenchPreferences}
          onUpdateTheme={handleUpdateTheme}
          onUpdateWorkbenchPreferences={(prefs) =>
            (workbenchPreferences = { ...workbenchPreferences, ...prefs })}
        />
      {:else}
        <!-- Media Category Grid (Shows, Movies, Anime, Manga, Games, Books, etc.) -->
        <LibraryView
          records={filteredSectionRecords}
          {availableCollections}
          contextMenuConfigs={workbenchPreferences.contextMenuItems}
          onSelectRecord={handleSelectRecord}
          onUpdateStatus={handleUpdateStatus}
          onUpdateRating={handleUpdateRating}
          onUpdateProgress={handleUpdateProgress}
          onSaveReview={handleSaveReview}
          onSaveCollection={handleSaveCollection}
        />
      {/if}
    </main>
  </div>

  <!-- Off-Canvas Tabler Theme Drawer -->
  <TablerThemeDrawer
    open={themeDrawerOpen}
    {themeSettings}
    onClose={() => (themeDrawerOpen = false)}
    onUpdateTheme={handleUpdateTheme}
  />
</div>

<style>
  .fasti-workbench-layout {
    display: flex !important;
    flex-direction: row !important;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    background-color: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .workbench-main-shell {
    flex: 1;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .density-compact {
    font-size: 14px;
  }

  .density-normal {
    font-size: 16px;
  }

  .density-spacious {
    font-size: 18px;
  }

  .top-nav-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 24px;
    background: var(--fasti-surface-paper);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    gap: 16px;
    z-index: 10;
  }

  .search-field-wrapper {
    position: relative;
    flex: 1;
    max-width: 440px;
    display: flex;
    align-items: center;
  }

  :global(.search-icon) {
    position: absolute;
    left: 12px;
    color: var(--fasti-text-muted);
    pointer-events: none;
  }

  .global-search-input {
    width: 100%;
    padding: 8px 12px 8px 38px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: var(--tblr-border-radius, 4px);
    color: var(--fasti-text-primary);
    font-size: 0.88rem;
    outline: none;
    transition: border-color 120ms ease;
  }

  .global-search-input:focus {
    border-color: var(--fasti-action-primary);
    outline: 2px solid var(--fasti-action-primary);
    outline-offset: 1px;
  }

  .top-actions-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .scope-select {
    min-height: 38px;
    padding: 8px 12px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: var(--tblr-border-radius, 4px);
    color: var(--fasti-text-primary);
    font-size: 0.86rem;
    font-weight: 500;
  }

  .tool-btn {
    min-height: 38px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: var(--tblr-border-radius, 4px);
    color: var(--fasti-text-primary);
    font-size: 0.84rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .tool-btn:hover {
    background: color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  .tool-btn.icon-only {
    min-width: 38px;
    min-height: 38px;
    padding: 7px;
    display: grid;
    place-items: center;
  }

  .status-indicator {
    min-width: 38px;
    min-height: 38px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  :global(.spin) {
    animation: spin 1.2s linear infinite;
  }

  .viewport-canvas {
    flex: 1;
    overflow-y: auto;
    background-color: var(--fasti-surface-archive);
    box-sizing: border-box;
  }

  .empty-workbench {
    max-width: 42rem;
    margin: 4rem auto;
    padding: 2rem;
    border: 1px solid var(--fasti-border-subtle);
    border-radius: var(--fasti-radius-lg);
    background: var(--fasti-surface-paper);
  }

  .empty-workbench h1 {
    margin-top: 0;
  }
</style>
