<script lang="ts">
  import { onMount } from "svelte";
  import type {
    ActiveNavSection,
    MediaRecord,
    WatchStatus,
    ChronicleOccurrence,
    ProviderApiKeyConfig,
    OidcConfiguration,
    AppriseNotificationConfig,
    ThemeSettings,
  } from "./types.js";
  import {
    SAMPLE_RECORDS,
    SAMPLE_CHRONICLE,
    SAMPLE_RECONCILIATION,
    SAMPLE_DISCOVER_TRENDING,
    SAMPLE_CUSTOM_FIELDS,
    SAMPLE_TOKENS,
    SAMPLE_PROVIDER_KEYS,
    SAMPLE_OIDC_CONFIG,
    SAMPLE_APPRISE_CONFIG,
    DEFAULT_THEME_SETTINGS,
  } from "./mock-data.js";
  import NavSidebar from "./nav-sidebar.svelte";
  import ChronicleView from "./chronicle-view.svelte";
  import DiscoverView from "./discover-view.svelte";
  import LibraryView from "./library-view.svelte";
  import MediaDetailView from "./media-detail-view.svelte";
  import ReconciliationView from "./reconciliation-view.svelte";
  import CalendarView from "./calendar-view.svelte";
  import ConnectionsView from "./connections-view.svelte";
  import SettingsView from "./settings-view.svelte";

  let activeSection: ActiveNavSection = $state("chronicle");
  let records = $state<MediaRecord[]>(SAMPLE_RECORDS);
  let chronicle = $state<ChronicleOccurrence[]>(SAMPLE_CHRONICLE);
  let reconciliationCases = $state(SAMPLE_RECONCILIATION);
  let tokens = $state(SAMPLE_TOKENS);
  let providerKeys = $state(SAMPLE_PROVIDER_KEYS);
  let oidcConfig = $state(SAMPLE_OIDC_CONFIG);
  let appriseConfig = $state(SAMPLE_APPRISE_CONFIG);
  let themeSettings = $state<ThemeSettings>(DEFAULT_THEME_SETTINGS);
  let selectedRecordId = $state<string | null>(null);

  const selectedRecord = $derived(
    records.find((r) => r.id === selectedRecordId),
  );
  const watchingRecords = $derived(
    records.filter((r) => r.status === "watching"),
  );
  const openReviewCount = $derived(
    reconciliationCases.filter((c) => c.status === "open").length,
  );

  // --- 1. HTML5 History API URL Routing ---
  function sectionToPath(
    section: ActiveNavSection,
    recordId?: string | null,
  ): string {
    switch (section) {
      case "chronicle":
        return "/chronicle";
      case "discover":
        return "/discover";
      case "library":
        return "/library";
      case "detail":
        return recordId ? `/records/${recordId}` : "/library";
      case "up_next":
        return "/up-next";
      case "calendar":
        return "/calendar";
      case "reconciliation":
        return "/review-inbox";
      case "connections":
        return "/connections";
      case "settings":
        return "/settings";
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
    } else if (pathname === "/library") {
      activeSection = "library";
      selectedRecordId = null;
    } else if (pathname === "/up-next") {
      activeSection = "up_next";
      selectedRecordId = null;
    } else if (pathname === "/calendar") {
      activeSection = "calendar";
      selectedRecordId = null;
    } else if (pathname === "/review-inbox" || pathname === "/reconciliation") {
      activeSection = "reconciliation";
      selectedRecordId = null;
    } else if (pathname === "/connections") {
      activeSection = "connections";
      selectedRecordId = null;
    } else if (pathname === "/settings") {
      activeSection = "settings";
      selectedRecordId = null;
    } else {
      activeSection = "chronicle";
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
    activeSection = "library";
    selectedRecordId = null;
    if (typeof window !== "undefined") {
      window.history.pushState({}, "", "/library");
    }
  }

  // --- 2. Real Tabler Theme Engine Synchronization ---
  $effect(() => {
    if (typeof document === "undefined") return;
    const mode = themeSettings.mode;
    const isDark = mode === "dark" || mode === "night";

    // Tabler & Bootstrap Theme Attributes
    document.documentElement.setAttribute(
      "data-bs-theme",
      isDark ? "dark" : "light",
    );
    document.documentElement.setAttribute("data-fasti-mode", mode);
    document.body.className = `theme-${mode} layout-fluid`;

    // Dynamic Tabler Accent Color and Variables
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

  onMount(() => {
    syncFromUrl();
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

  function handleSaveProviderKey(provider: string, key: string): void {
    providerKeys = providerKeys.map((p) =>
      p.provider === provider
        ? { ...p, apiKey: key, isConfigured: key.trim().length > 0 }
        : p,
    );
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
</script>

<div
  class="fasti-workbench-layout theme-{themeSettings.mode} density-{themeSettings.density}"
>
  <NavSidebar
    {activeSection}
    {openReviewCount}
    onSelectSection={handleSelectSection}
  />

  <main class="viewport-canvas" id="main-content">
    {#if activeSection === "chronicle"}
      <ChronicleView
        occurrences={chronicle}
        onSelectRecord={handleSelectRecord}
      />
    {:else if activeSection === "discover"}
      <DiscoverView
        trendingRecords={SAMPLE_DISCOVER_TRENDING}
        onSelectRecord={handleSelectRecord}
        onUpdateStatus={handleUpdateStatus}
        onUpdateProgress={handleUpdateProgress}
        onSaveReview={handleSaveReview}
        onSaveCollection={handleSaveCollection}
      />
    {:else if activeSection === "library"}
      <LibraryView
        {records}
        onSelectRecord={handleSelectRecord}
        onUpdateStatus={handleUpdateStatus}
        onUpdateRating={handleUpdateRating}
        onUpdateProgress={handleUpdateProgress}
        onSaveReview={handleSaveReview}
        onSaveCollection={handleSaveCollection}
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
    {:else if activeSection === "up_next"}
      <CalendarView {watchingRecords} onSelectRecord={handleSelectRecord} />
    {:else if activeSection === "calendar"}
      <CalendarView {watchingRecords} onSelectRecord={handleSelectRecord} />
    {:else if activeSection === "reconciliation"}
      <ReconciliationView
        cases={reconciliationCases}
        onAcceptCase={handleAcceptCase}
        onRejectCase={handleRejectCase}
        onDeferCase={handleDeferCase}
      />
    {:else if activeSection === "connections"}
      <ConnectionsView />
    {:else if activeSection === "settings"}
      <SettingsView
        customFields={SAMPLE_CUSTOM_FIELDS}
        {tokens}
        {providerKeys}
        {oidcConfig}
        {appriseConfig}
        {themeSettings}
        onUpdateTheme={handleUpdateTheme}
        onSaveProviderKey={handleSaveProviderKey}
        onCreateToken={handleCreateToken}
        onDeleteToken={handleDeleteToken}
        onSaveOidc={handleSaveOidc}
        onSaveApprise={handleSaveApprise}
      />
    {/if}
  </main>
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

  .viewport-canvas {
    flex: 1;
    height: 100vh;
    overflow-y: auto;
    background-color: var(--fasti-surface-archive);
    box-sizing: border-box;
  }
</style>
