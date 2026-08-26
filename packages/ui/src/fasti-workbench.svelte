<script lang="ts">
  import { onMount } from "svelte";
  import type {
    ActiveNavSection,
    MediaRecord,
    WatchStatus,
    ChronicleOccurrence,
    ProviderCredentialStatus,
    OidcConfiguration,
    AppriseNotificationConfig,
    ThemeSettings,
    WorkbenchPreferences,
    WorkbenchHost,
    ReconciliationCase,
    ScopedApiToken,
  } from "./types.js";
  import { hostProblemText } from "./host-problem.js";
  import {
    SAMPLE_RECORDS,
    SAMPLE_CHRONICLE,
    SAMPLE_RECONCILIATION,
    SAMPLE_DISCOVER_TRENDING,
    SAMPLE_CUSTOM_FIELDS,
    SAMPLE_TOKENS,
    SAMPLE_OIDC_CONFIG,
    SAMPLE_APPRISE_CONFIG,
    DEFAULT_THEME_SETTINGS,
    DEFAULT_WORKBENCH_PREFERENCES,
  } from "./mock-data.js";
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
    IconLayoutGrid,
    IconList,
    IconFilter,
    IconMoon,
    IconSun,
    IconCircleCheck,
    IconLayoutSidebar,
    IconAlertCircle,
    IconLoader2,
    IconChevronRight,
  } from "@tabler/icons-svelte";

  interface Props {
    host?: WorkbenchHost;
  }

  let { host }: Props = $props();

  function computeInitialSection(): ActiveNavSection {
    if (typeof window === "undefined") return "home";
    const pathname = window.location.pathname;
    if (pathname.startsWith("/records/")) return "detail";
    if (pathname === "/discover") return "discover";
    if (pathname === "/media/shows") return "tv_shows";
    if (pathname === "/media/movies") return "movies";
    if (pathname === "/media/anime") return "anime";
    if (pathname === "/media/manga") return "manga";
    if (pathname === "/media/games") return "games";
    if (pathname === "/media/books") return "books";
    if (pathname === "/media/comics") return "comics";
    if (pathname === "/media/podcasts") return "podcasts";
    if (pathname === "/media/music") return "music";
    if (pathname === "/calendar") return "calendar";
    if (pathname === "/collection") return "collection";
    if (pathname === "/history" || pathname === "/chronicle")
      return "chronicle";
    if (pathname === "/review-inbox" || pathname === "/reconciliation")
      return "reconciliation";
    if (pathname === "/connections" || pathname === "/sources")
      return "connections";
    if (pathname === "/settings") return "settings";
    return "home";
  }

  function loadInitialState<T>(key: string, fallback: T): T {
    if (typeof window === "undefined") return fallback;
    try {
      const saved = localStorage.getItem(key);
      if (saved) return JSON.parse(saved);
    } catch {}
    return fallback;
  }

  let activeSection: ActiveNavSection = $state(computeInitialSection());
  let records = $state<MediaRecord[]>(
    loadInitialState("fasti-records", SAMPLE_RECORDS),
  );
  let chronicle = $state<ChronicleOccurrence[]>(
    loadInitialState("fasti-chronicle", SAMPLE_CHRONICLE),
  );
  let reconciliationCases = $state<ReconciliationCase[]>(
    loadInitialState("fasti-reconciliation", SAMPLE_RECONCILIATION),
  );
  let tokens = $state<ScopedApiToken[]>(
    loadInitialState("fasti-tokens", SAMPLE_TOKENS),
  );
  let providerKeys = $state<ProviderCredentialStatus[]>([]);
  let providerLoading = $state(false);
  let providerLoadProblem = $state<string | undefined>();
  let oidcConfig = $state<OidcConfiguration>(
    loadInitialState("fasti-oidc", SAMPLE_OIDC_CONFIG),
  );
  let appriseConfig = $state<AppriseNotificationConfig>(
    loadInitialState("fasti-apprise", SAMPLE_APPRISE_CONFIG),
  );
  let themeSettings = $state<ThemeSettings>(
    loadInitialState("fasti-theme-settings", DEFAULT_THEME_SETTINGS),
  );
  let workbenchPreferences = $state<WorkbenchPreferences>(
    loadInitialState(
      "fasti-workbench-preferences",
      DEFAULT_WORKBENCH_PREFERENCES,
    ),
  );

  $effect(() => {
    try {
      localStorage.setItem("fasti-records", JSON.stringify(records));
    } catch {}
  });
  $effect(() => {
    try {
      localStorage.setItem("fasti-chronicle", JSON.stringify(chronicle));
    } catch {}
  });
  $effect(() => {
    try {
      localStorage.setItem(
        "fasti-reconciliation",
        JSON.stringify(reconciliationCases),
      );
    } catch {}
  });
  $effect(() => {
    try {
      localStorage.setItem("fasti-tokens", JSON.stringify(tokens));
    } catch {}
  });
  $effect(() => {
    try {
      localStorage.setItem("fasti-oidc", JSON.stringify(oidcConfig));
    } catch {}
  });
  $effect(() => {
    try {
      localStorage.setItem("fasti-apprise", JSON.stringify(appriseConfig));
    } catch {}
  });
  $effect(() => {
    try {
      localStorage.setItem(
        "fasti-theme-settings",
        JSON.stringify(themeSettings),
      );
      if (typeof document !== "undefined") {
        const root = document.documentElement;
        root.dataset.bsTheme =
          themeSettings.mode === "light" ? "light" : "dark";
        root.style.colorScheme =
          themeSettings.mode === "light" ? "light" : "dark";

        if (themeSettings.mode === "night") {
          root.style.setProperty("--fasti-surface-archive", "#0b0b0c");
          root.style.setProperty("--fasti-surface-paper", "#141416");
          root.style.setProperty("--fasti-text-primary", "#ffffff");
          root.style.setProperty("--fasti-text-muted", "#71717a");
        } else if (themeSettings.mode === "dark") {
          root.style.setProperty("--fasti-surface-archive", "#182433");
          root.style.setProperty("--fasti-surface-paper", "#1f2d3d");
          root.style.setProperty("--fasti-text-primary", "#f8fafc");
          root.style.setProperty("--fasti-text-muted", "#94a3b8");
        } else {
          root.style.setProperty("--fasti-surface-archive", "#F2EFE6");
          root.style.setProperty("--fasti-surface-paper", "#FFFDF8");
          root.style.setProperty("--fasti-text-primary", "#181716");
          root.style.setProperty("--fasti-text-muted", "#625E56");
        }

        if (themeSettings.accentColor) {
          root.style.setProperty(
            "--fasti-action-primary",
            themeSettings.accentColor,
          );
          root.style.setProperty("--tblr-primary", themeSettings.accentColor);
        }

        if (themeSettings.fontFamily === "serif") {
          root.style.setProperty(
            "--fasti-font-display",
            "'Newsreader', Georgia, serif",
          );
        } else if (themeSettings.fontFamily === "monospace") {
          root.style.setProperty(
            "--fasti-font-display",
            "'IBM Plex Mono', monospace",
          );
        } else {
          root.style.setProperty(
            "--fasti-font-display",
            "'Atkinson Hyperlegible', sans-serif",
          );
        }
      }
    } catch {}
  });
  $effect(() => {
    try {
      localStorage.setItem(
        "fasti-workbench-preferences",
        JSON.stringify(workbenchPreferences),
      );
    } catch {}
  });
  let selectedRecordId = $state<string | null>(null);
  let themeDrawerOpen = $state(false);
  let searchQuery = $state("");
  let mediaScope = $state("all");
  let viewMode: "grid" | "list" = $state("grid");
  let nodeHealthy = $state<boolean | null>(null);

  const selectedRecord = $derived(
    records.find((r) => r.id === selectedRecordId),
  );
  const watchingRecords = $derived(
    records.filter((r) => r.status === "watching"),
  );
  const openReviewCount = $derived(
    reconciliationCases.filter((c) => c.status === "open").length,
  );

  const globalSearchResults = $derived.by(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return [];
    return records
      .filter(
        (r) =>
          r.title.toLowerCase().includes(q) ||
          r.originalTitle?.toLowerCase().includes(q) ||
          r.genres?.some((g) => g.toLowerCase().includes(q)) ||
          r.tags?.some((t) => t.toLowerCase().includes(q)) ||
          r.cast?.some((c) => c.name.toLowerCase().includes(q)) ||
          r.crew?.some((cr) => cr.name.toLowerCase().includes(q)),
      )
      .slice(0, 8);
  });

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
    document.body.className = `theme-${mode} layout-fluid`;

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
      const res = await fetch("/api/v1/health");
      nodeHealthy = res.ok;
    } catch {
      nodeHealthy = false;
    }
  }

  async function loadProviderKeys(): Promise<void> {
    if (!host?.providerCredentialStatus) return;
    providerLoading = true;
    providerLoadProblem = undefined;
    try {
      providerKeys = await host.providerCredentialStatus();
    } catch (error) {
      providerLoadProblem = hostProblemText(
        error,
        "The trusted host rejected the provider credential status request.",
      );
    } finally {
      providerLoading = false;
    }
  }

  onMount(() => {
    syncFromUrl();
    checkNodeHealth();
    void loadProviderKeys();
    window.addEventListener("popstate", syncFromUrl);
    return () => {
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

  async function handleSaveProviderKey(
    provider: string,
    key: string,
  ): Promise<void> {
    if (!host?.saveProviderCredential) {
      throw new Error(
        "This host cannot save provider credentials outside the trusted desktop shell.",
      );
    }
    providerKeys = await host.saveProviderCredential(provider, key);
  }

  async function handleDeleteProviderKey(provider: string): Promise<void> {
    if (!host?.deleteProviderCredential) {
      throw new Error(
        "This host cannot remove provider credentials outside the trusted desktop shell.",
      );
    }
    providerKeys = await host.deleteProviderCredential(provider);
  }

  function handleCreateToken(name: string, scopes: string[]): void {
    const newToken = {
      id: `tok_${Date.now()}`,
      name,
      tokenPrefix: `fst_pat_${Math.random().toString(36).substring(2, 8)}...`,
      scopes,
      createdAt: new Date().toISOString(),
    };
    tokens = [newToken, ...tokens];
  }

  function handleDeleteToken(id: string): void {
    tokens = tokens.filter((t) => t.id !== id);
  }

  function handleSaveOidc(config: OidcConfiguration): void {
    oidcConfig = { ...config };
  }

  function handleSaveApprise(config: AppriseNotificationConfig): void {
    appriseConfig = { ...config };
  }

  function handleAcceptCase(caseId: string): void {
    reconciliationCases = reconciliationCases.filter((c) => c.id !== caseId);
  }

  function handleRejectCase(caseId: string): void {
    reconciliationCases = reconciliationCases.filter((c) => c.id !== caseId);
  }

  function handleDeferCase(caseId: string): void {
    reconciliationCases = reconciliationCases.map((c) =>
      c.id === caseId ? { ...c, status: "deferred" } : c,
    );
  }

  function handleAddRecord(rec: MediaRecord, andOcc = false): void {
    const existing = records.find((r) => r.id === rec.id);
    if (!existing) {
      records = [rec, ...records];
    } else {
      records = records.map((r) => (r.id === rec.id ? rec : r));
    }
    if (andOcc) {
      const newOcc: ChronicleOccurrence = {
        id: `occ_${Date.now()}`,
        recordId: rec.id,
        title: rec.title,
        mediaKind: rec.mediaKind,
        posterUrl: rec.posterUrl,
        timestamp: new Date().toISOString(),
        progressPercentage: 100,
        durationMinutes: rec.runtimeMinutes ?? 30,
        deviceName: "Web Browser",
        clientName: "Fasti Web",
        isRewatch: false,
        userRating: rec.userRating,
      };
      chronicle = [newOcc, ...chronicle];
    }
  }

  function handleImportData(
    importedRecords: MediaRecord[],
    importedOccurrences: ChronicleOccurrence[],
  ): void {
    if (importedRecords.length > 0) {
      const existingIds = new Set(records.map((r) => r.id));
      const newRecords = importedRecords.filter((r) => !existingIds.has(r.id));
      records = [...records, ...newRecords];
    }
    if (importedOccurrences.length > 0) {
      const existingOccIds = new Set(chronicle.map((o) => o.id));
      const newOcc = importedOccurrences.filter(
        (o) => !existingOccIds.has(o.id),
      );
      chronicle = [...newOcc, ...chronicle];
    }
  }

  function handleExportChronicle(): void {
    const backup = {
      version: "1.0",
      exportedAt: new Date().toISOString(),
      records,
      chronicle,
      reconciliationCases,
      tokens,
      oidcConfig,
      appriseConfig,
      themeSettings,
      workbenchPreferences,
    };
    const blob = new Blob([JSON.stringify(backup, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `fasti-chronicle-backup-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  function handleExportCsv(): void {
    const headers =
      "id,recordId,title,mediaKind,timestamp,durationMinutes,deviceName,clientName,userRating\n";
    const rows = chronicle
      .map(
        (o) =>
          `"${o.id}","${o.recordId}","${o.title.replace(/"/g, '""')}","${o.mediaKind}","${o.timestamp}",${o.durationMinutes},"${o.deviceName}","${o.clientName}",${o.userRating ?? ""}`,
      )
      .join("\n");
    const blob = new Blob([headers + rows], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `fasti-scrobble-history-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }
  function formatSectionTitle(sec: ActiveNavSection): string {
    const titles: Record<string, string> = {
      home: "Chronicle Dashboard",
      discover: "Discover & Explore",
      tv_shows: "TV Shows",
      tv_seasons: "TV Seasons",
      movies: "Movies",
      anime: "Anime",
      manga: "Manga",
      games: "Games",
      books: "Books",
      comics: "Comics",
      board_games: "Board Games",
      music: "Music",
      podcasts: "Podcasts",
      calendar: "Calendar & Schedule",
      collection: "Collections",
      custom: "Custom Collections",
      chronicle: "Living Chronicle",
      history: "Living Chronicle",
      lists: "Custom Lists",
      statistics: "Statistics & Insights",
      tags: "Tags & Genres",
      reconciliation: "Reconciliation Inbox",
      connections: "Media Server Connections",
      sources: "Media Server Connections",
      settings: "Settings & Studio",
      detail: "Media Details",
    };
    return titles[sec] || "Fasti Chronicle";
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
    hidden={false}
    onToggleCollapse={() =>
      (workbenchPreferences = {
        ...workbenchPreferences,
        sidebarCollapsed: !workbenchPreferences.sidebarCollapsed,
      })}
    onToggleHide={() =>
      (workbenchPreferences = {
        ...workbenchPreferences,
        sidebarCollapsed: !workbenchPreferences.sidebarCollapsed,
      })}
    onSelectSection={handleSelectSection}
  />

  <div class="workbench-main-shell">
    <!-- Top Bar Header: Left (Sidebar toggle + Title), Center (Search), Right (Controls) -->
    <header
      class="top-nav-bar"
      role="toolbar"
      aria-label="Global workbench toolbar"
    >
      <!-- Left: Sidebar Toggle + Active Section Breadcrumb -->
      <div class="top-nav-left d-flex align-items-center gap-3">
        <button
          type="button"
          class="btn btn-icon btn-outline-secondary btn-sm sidebar-toggle-btn"
          onclick={() =>
            (workbenchPreferences = {
              ...workbenchPreferences,
              sidebarCollapsed: !workbenchPreferences.sidebarCollapsed,
            })}
          title={workbenchPreferences.sidebarCollapsed
            ? "Expand sidebar"
            : "Collapse sidebar"}
          aria-label={workbenchPreferences.sidebarCollapsed
            ? "Expand sidebar"
            : "Collapse sidebar"}
        >
          <IconLayoutSidebar size={18} />
        </button>

        <div class="section-title-wrap d-flex align-items-center gap-2">
          <span class="active-section-label font-display fw-bold text-truncate">
            {formatSectionTitle(activeSection)}
          </span>
        </div>
      </div>

      <!-- Center: Global Search Input -->
      <div
        class="top-nav-center flex-grow-1 px-3 d-flex justify-content-center position-relative"
      >
        <div
          class="search-field-wrapper position-relative"
          style="width: 100%; max-width: 480px;"
        >
          <IconSearch size={18} class="search-icon" />
          <input
            type="search"
            class="global-search-input form-control form-control-sm"
            placeholder="Search titles, creators, tags..."
            bind:value={searchQuery}
            aria-label="Search media collection"
          />

          {#if searchQuery.trim().length > 0}
            <div
              class="dropdown-menu show shadow-lg border p-2 position-absolute w-100 mt-1"
              style="top: 100%; left: 0; z-index: 1050; max-height: 380px; overflow-y: auto; background: var(--fasti-surface-paper); color: var(--fasti-text-primary); border-color: color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);"
            >
              <div
                class="px-2 py-1 small text-muted font-monospace text-uppercase"
                style="font-size: 0.68rem;"
              >
                Library Matches ({globalSearchResults.length})
              </div>

              {#if globalSearchResults.length === 0}
                <div class="px-3 py-2 text-muted small">
                  No direct matches in library for "{searchQuery}"
                </div>
              {:else}
                {#each globalSearchResults as res (res.id)}
                  <button
                    type="button"
                    class="dropdown-item d-flex align-items-center gap-2 py-2 px-2 rounded"
                    onclick={() => {
                      handleSelectRecord(res.id);
                      searchQuery = "";
                    }}
                  >
                    {#if res.posterUrl}
                      <img
                        src={res.posterUrl}
                        alt=""
                        referrerpolicy="no-referrer"
                        style="width: 32px; height: 48px; object-fit: cover; border-radius: 3px;"
                      />
                    {:else}
                      <div
                        class="bg-secondary-lt text-muted d-flex align-items-center justify-content-center"
                        style="width: 32px; height: 48px; border-radius: 3px; font-size: 0.65rem;"
                      >
                        {res.mediaKind}
                      </div>
                    {/if}
                    <div class="flex-grow-1 text-truncate text-start">
                      <div class="fw-bold text-truncate">{res.title}</div>
                      <div
                        class="small text-muted font-monospace"
                        style="font-size: 0.72rem;"
                      >
                        {res.mediaKind.toUpperCase()} • {res.releaseYear ??
                          "Unknown"} • {res.status.replace("_", " ")}
                      </div>
                    </div>
                    {#if res.userRating}
                      <span class="badge bg-warning text-dark font-monospace"
                        >★ {res.userRating}</span
                      >
                    {/if}
                  </button>
                {/each}
              {/if}

              <div class="dropdown-divider my-1"></div>
              <button
                type="button"
                class="dropdown-item d-flex align-items-center justify-content-between py-2 px-2 text-primary fw-bold small"
                onclick={() => {
                  activeSection = "discover";
                }}
              >
                <span>Search online in Discover for "{searchQuery}"</span>
                <IconChevronRight size={14} />
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Right: Scope, View mode, Theme drawer, Health status -->
      <div class="top-nav-right d-flex align-items-center gap-2">
        <!-- Media Scope Select -->
        <select
          class="scope-select form-select form-select-sm"
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

        <!-- Grid / List Toggle -->
        <div
          class="btn-group btn-group-sm"
          role="radiogroup"
          aria-label="View layout mode"
        >
          <button
            type="button"
            class="btn btn-outline-secondary btn-icon"
            class:active={viewMode === "grid"}
            onclick={() => (viewMode = "grid")}
            role="radio"
            aria-checked={viewMode === "grid"}
            title="Grid view"
            aria-label="Grid view"
          >
            <IconLayoutGrid size={16} />
          </button>
          <button
            type="button"
            class="btn btn-outline-secondary btn-icon"
            class:active={viewMode === "list"}
            onclick={() => (viewMode = "list")}
            role="radio"
            aria-checked={viewMode === "list"}
            title="List view"
            aria-label="List view"
          >
            <IconList size={16} />
          </button>
        </div>

        <!-- Theme Settings Drawer Trigger -->
        <button
          type="button"
          class="btn btn-icon btn-outline-secondary btn-sm"
          onclick={() => (themeDrawerOpen = true)}
          title="Open Theme Studio"
          aria-label="Open Theme Studio"
        >
          {#if themeSettings.mode === "light"}
            <IconSun size={17} />
          {:else}
            <IconMoon size={17} />
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
        <HomeView
          {records}
          contextMenuConfigs={workbenchPreferences.contextMenuItems}
          onSelectRecord={handleSelectRecord}
          onUpdateStatus={handleUpdateStatus}
          onUpdateProgress={handleUpdateProgress}
          onSaveReview={handleSaveReview}
          onSaveCollection={handleSaveCollection}
          onViewAllSection={(sec) => handleSelectSection(sec as any)}
        />
      {:else if activeSection === "discover"}
        <DiscoverView
          trendingRecords={SAMPLE_DISCOVER_TRENDING}
          libraryRecords={records}
          {host}
          onSelectRecord={handleSelectRecord}
          onUpdateStatus={handleUpdateStatus}
          onUpdateProgress={handleUpdateProgress}
          onSaveReview={handleSaveReview}
          onSaveCollection={handleSaveCollection}
          onAddRecord={handleAddRecord}
        />
      {:else if activeSection === "detail" && selectedRecord}
        <MediaDetailView
          record={selectedRecord}
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
        <ReconciliationView
          cases={reconciliationCases}
          onAcceptCase={handleAcceptCase}
          onRejectCase={handleRejectCase}
          onDeferCase={handleDeferCase}
        />
      {:else if activeSection === "connections" || activeSection === "sources"}
        <ConnectionsView />
      {:else if activeSection === "settings"}
        <SettingsView
          customFields={SAMPLE_CUSTOM_FIELDS}
          {tokens}
          {providerKeys}
          {providerLoading}
          {providerLoadProblem}
          {oidcConfig}
          {appriseConfig}
          {themeSettings}
          {workbenchPreferences}
          onUpdateTheme={handleUpdateTheme}
          onUpdateWorkbenchPreferences={(prefs) =>
            (workbenchPreferences = { ...workbenchPreferences, ...prefs })}
          onSaveProviderKey={handleSaveProviderKey}
          onDeleteProviderKey={handleDeleteProviderKey}
          onRetryProviderState={loadProviderKeys}
          onCreateToken={handleCreateToken}
          onDeleteToken={handleDeleteToken}
          onSaveOidc={handleSaveOidc}
          onSaveApprise={handleSaveApprise}
          onImportData={handleImportData}
          onExportChronicle={handleExportChronicle}
          onExportCsv={handleExportCsv}
        />
      {:else}
        <!-- Media Category Grid (Shows, Movies, Anime, Manga, Games, Books, etc.) -->
        <LibraryView
          records={filteredSectionRecords}
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
    flex: 1 1 0%;
    min-width: 0;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    position: relative;
  }

  .top-nav-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 20px;
    background: var(--fasti-surface-paper);
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    gap: 16px;
    z-index: 10;
    min-height: 56px;
    box-sizing: border-box;
  }

  .top-nav-left {
    flex-shrink: 0;
    min-width: 200px;
  }

  .active-section-label {
    font-size: 1.15rem;
    color: var(--fasti-text-primary);
  }

  .top-nav-center {
    flex: 1;
    max-width: 520px;
  }

  .search-field-wrapper {
    position: relative;
    width: 100%;
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
    padding: 7px 12px 7px 36px;
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

  .top-nav-right {
    flex-shrink: 0;
  }

  .scope-select {
    min-height: 36px;
    padding: 6px 12px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: var(--tblr-border-radius, 4px);
    color: var(--fasti-text-primary);
    font-size: 0.86rem;
    font-weight: 500;
  }

  .status-indicator {
    min-width: 36px;
    min-height: 36px;
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
    flex: 1 1 0%;
    min-width: 0;
    overflow-y: auto;
    overflow-x: hidden;
    background-color: var(--fasti-surface-archive);
    box-sizing: border-box;
  }
</style>
