<script lang="ts">
  import { flushSync, onMount, tick } from "svelte";
  import {
    IconChevronRight,
    IconActivityHeartbeat,
    IconDatabase,
    IconLayoutSidebar,
    IconLayoutSidebarLeftExpand,
    IconPalette,
    IconPlugConnected,
    IconSettings,
    IconShieldCheck,
    IconLogout,
    IconUserCircle,
  } from "@tabler/icons-svelte";
  import AuthModal from "./auth-modal.svelte";
  import GlobalSearch from "./global-search.svelte";
  import HomeView from "./home-view.svelte";
  import ConnectionsView from "./connections-view.svelte";
  import RuntimeSettingsView from "./runtime-settings-view.svelte";
  import DiscoverView from "./discover-view.svelte";
  import ReconciliationView from "./reconciliation-view.svelte";
  import LibraryView from "./library-view.svelte";
  import CalendarView from "./calendar-view.svelte";
  import MediaDetailView from "./media-detail-view.svelte";
  import NavSidebar from "./nav-sidebar.svelte";
  import TablerThemeDrawer from "./tabler-theme-drawer.svelte";
  import {
    createDefaultWorkbenchPreferences,
    DEFAULT_THEME_SETTINGS,
  } from "./defaults.js";
  import { hostProblemText } from "./host-problem.js";
  import { projectRecordSummary } from "./record-projection.js";
  import type {
    ActiveNavSection,
    BrowserSession,
    CreateRecordResult,
    MediaRecord,
    ProviderCredentialStatus,
    ProviderSearchCandidate,
    ProviderSelection,
    ResolveReviewInput,
    ReviewItem,
    ThemeSettings,
    TrackingDispositionUpdate,
    WorkbenchHost,
    WorkbenchPreferences,
  } from "./types.js";

  interface Props {
    host: WorkbenchHost;
    onOpenStatus?: () => void;
  }

  type Section =
    | "home"
    | "connections"
    | "settings"
    | "discover"
    | "reconciliation"
    | "library"
    | "calendar"
    | "detail";

  let { host, onOpenStatus }: Props = $props();
  let credentialTarget = $state("");

  const credentialAdministration = $derived(
    Boolean(
      host.listApiClients && host.createApiClient && host.revokeApiClient,
    ),
  );

  function loadPersisted<T>(key: string, fallback: T): T {
    if (typeof window === "undefined") return fallback;
    try {
      const saved = localStorage.getItem(key);
      if (saved) return JSON.parse(saved) as T;
    } catch {}
    return fallback;
  }

  function defaultThemeSettings(): ThemeSettings {
    const activeMode =
      typeof document === "undefined"
        ? undefined
        : document.documentElement.dataset.fastiTheme;
    return {
      ...DEFAULT_THEME_SETTINGS,
      mode:
        activeMode === "dark" || activeMode === "night" ? activeMode : "light",
    };
  }

  function accessibleAccent(value: string): {
    color: string;
    contrast: "#000000" | "#ffffff";
    rgb: string;
  } {
    const match = /^#([0-9a-f]{6})$/i.exec(value.trim());
    const color = match ? `#${match[1]}` : DEFAULT_THEME_SETTINGS.accentColor;
    const channels = [1, 3, 5].map((index) => {
      const channel = Number.parseInt(color.slice(index, index + 2), 16) / 255;
      return channel <= 0.04045
        ? channel / 12.92
        : ((channel + 0.055) / 1.055) ** 2.4;
    });
    const luminance =
      channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
    return {
      color,
      contrast: luminance > 0.179 ? "#000000" : "#ffffff",
      rgb: [1, 3, 5]
        .map((index) => Number.parseInt(color.slice(index, index + 2), 16))
        .join(", "),
    };
  }

  type SettingsTab =
    | "network"
    | "providers"
    | "preferences"
    | "custom_fields"
    | "nuvio_collections"
    | "system";

  let settingsTab = $state<SettingsTab>("network");

  function settingsTabFromPath(path: string): SettingsTab {
    if (path === "/settings/metadata" || path === "/settings/providers")
      return "providers";
    if (path === "/settings/preferences") return "preferences";
    if (
      path === "/settings/custom-fields" ||
      path === "/settings/custom_fields"
    )
      return "custom_fields";
    if (
      path === "/settings/collections" ||
      path === "/settings/nuvio_collections"
    )
      return "nuvio_collections";
    if (path === "/settings/status" || path === "/settings/system")
      return "system";
    return "network";
  }

  function pathForSettingsTab(tab: SettingsTab): string {
    switch (tab) {
      case "providers":
        return "/settings/metadata";
      case "preferences":
        return "/settings/preferences";
      case "custom_fields":
        return "/settings/custom-fields";
      case "nuvio_collections":
        return "/settings/collections";
      case "system":
        return "/settings/status";
      default:
        return "/settings";
    }
  }

  function pathForSection(section: Section): string {
    switch (section) {
      case "connections":
        return "/connections";
      case "settings":
        return pathForSettingsTab(settingsTab);
      case "discover":
        return "/discover";
      case "reconciliation":
        return "/reconciliation";
      case "library":
        return "/library";
      case "calendar":
        return "/calendar";
      case "detail":
        return selectedRecordId ? `/records/${selectedRecordId}` : "/records";
      default:
        return "/";
    }
  }

  function sectionFromPath(): Section {
    if (typeof window === "undefined") return "home";
    const path = window.location.pathname;
    if (path === "/connections") return "connections";
    if (path.startsWith("/settings")) {
      settingsTab = settingsTabFromPath(path);
      return "settings";
    }
    if (path === "/discover") return "discover";
    if (path === "/reconciliation" || path === "/reviews")
      return "reconciliation";
    if (path === "/library") return "library";
    if (path === "/calendar") return "calendar";
    if (path.startsWith("/records")) {
      const id = path.slice("/records/".length);
      if (id) selectedRecordId = id;
      return "detail";
    }
    return "home";
  }

  let activeSection = $state<Section>("home");
  let selectedRecordId = $state<string | null>(null);
  let selectedRecordTab = $state<"overview" | "sources">("overview");

  // A prior session's localStorage predates nav items or context-menu items
  // added since (e.g. "settings", "connections") -- without this, those
  // items silently never appear for that browser profile because the
  // persisted array replaces the defaults wholesale instead of extending
  // them. Reconciled by id: keep the stored entry where one exists, append
  // any default entry that's missing.
  function mergeWithNewDefaults(
    stored: WorkbenchPreferences,
  ): WorkbenchPreferences {
    const defaults = createDefaultWorkbenchPreferences();
    const mergeById = <T extends { id: string }>(
      storedItems: T[],
      defaultItems: T[],
    ): T[] => {
      const supportedIds = new Set(defaultItems.map((item) => item.id));
      const seenIds = new Set<string>();
      const supportedStored = Array.isArray(storedItems)
        ? storedItems.filter((item) => {
            if (!supportedIds.has(item?.id) || seenIds.has(item.id))
              return false;
            seenIds.add(item.id);
            return true;
          })
        : [];
      return [
        ...supportedStored,
        ...defaultItems.filter(
          (item) => !supportedStored.some((stored) => stored.id === item.id),
        ),
      ];
    };
    // `defaults` spreads first so a preference field added since a browser's
    // stored copy was last written (e.g. `customFields`) is backfilled
    // instead of coming back `undefined` and crashing the first read.
    return {
      ...defaults,
      ...(stored && typeof stored === "object" ? stored : {}),
      sidebarCollapsed:
        typeof stored?.sidebarCollapsed === "boolean"
          ? stored.sidebarCollapsed
          : defaults.sidebarCollapsed,
      sidebarHidden:
        typeof stored?.sidebarHidden === "boolean"
          ? stored.sidebarHidden
          : defaults.sidebarHidden,
      navItems: mergeById(stored?.navItems, defaults.navItems),
      contextMenuItems: mergeById(
        stored?.contextMenuItems,
        defaults.contextMenuItems,
      ),
    };
  }

  let workbenchPreferences = $state<WorkbenchPreferences>(
    mergeWithNewDefaults(
      loadPersisted(
        "fasti-workbench-preferences",
        createDefaultWorkbenchPreferences(),
      ),
    ),
  );
  let themeSettings = $state<ThemeSettings>(
    normalizeThemeSettings(
      loadPersisted("fasti-theme-settings", defaultThemeSettings()),
    ),
  );
  let themeDrawerOpen = $state(false);
  let authModalOpen = $state(false);
  let mobileNavigationOpen = $state(false);
  let navigationTrigger = $state<HTMLButtonElement | undefined>();
  let showNavigationTrigger = $state<HTMLButtonElement | undefined>();
  let browserSession = $state<BrowserSession | null>(null);
  let browserSessionChecked = $state(false);

  $effect(() => {
    try {
      localStorage.setItem(
        "fasti-workbench-preferences",
        JSON.stringify(workbenchPreferences),
      );
    } catch {}
  });

  $effect(() => {
    try {
      localStorage.setItem(
        "fasti-theme-settings",
        JSON.stringify(themeSettings),
      );
      localStorage.setItem(
        "fasti-theme",
        themeSettings.mode === "light" ? "light" : "dark",
      );
    } catch {}
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    root.dataset.bsTheme = themeSettings.mode === "light" ? "light" : "dark";
    root.dataset.fastiTheme = themeSettings.mode;
    root.dataset.bsThemeBase = themeSettings.themeBase ?? "slate";
    root.dataset.bsThemeFont = themeSettings.fontFamily ?? "sans-serif";
    root.dataset.bsThemeRadius = String(themeSettings.cornerRadius ?? 1);
    root.style.colorScheme = themeSettings.mode === "light" ? "light" : "dark";
    if (themeSettings.accentColor) {
      const accent = accessibleAccent(themeSettings.accentColor);
      const tablerPrimary: Record<string, string> = {
        "#066fd1": "blue",
        "#d63939": "red",
        "#2fb344": "green",
        "#f76707": "orange",
        "#ae3ec9": "purple",
        "#0ca678": "teal",
        "#17a2b8": "cyan",
        "#8b2e2a": "red",
        "#d4af37": "yellow",
      };
      root.dataset.bsThemePrimary =
        tablerPrimary[accent.color.toLocaleLowerCase()] ?? "blue";
      root.style.setProperty("--fasti-action-primary", accent.color);
      root.style.setProperty("--fasti-action-contrast", accent.contrast);
      root.style.setProperty("--tblr-primary", accent.color);
      root.style.setProperty("--tblr-primary-rgb", accent.rgb);
      root.style.setProperty("--tblr-primary-fg", accent.contrast);
      root.style.setProperty(
        "--tblr-primary-darken",
        `color-mix(in srgb, ${accent.color} 88%, #000000)`,
      );
    }
    if (themeSettings.fontFamily === "serif") {
      root.style.setProperty(
        "--fasti-font-display",
        "'Newsreader', Georgia, serif",
      );
      root.style.setProperty(
        "--fasti-font-body",
        "'Newsreader', Georgia, serif",
      );
    } else if (themeSettings.fontFamily === "monospace") {
      root.style.setProperty(
        "--fasti-font-display",
        "'IBM Plex Mono', monospace",
      );
      root.style.setProperty("--fasti-font-body", "'IBM Plex Mono', monospace");
    } else {
      root.style.removeProperty("--fasti-font-display");
      root.style.removeProperty("--fasti-font-body");
    }
  });

  function updateTheme(updates: Partial<ThemeSettings>): void {
    themeSettings = normalizeThemeSettings({ ...themeSettings, ...updates });
  }

  function normalizeThemeSettings(
    value: Partial<ThemeSettings> | null,
  ): ThemeSettings {
    const candidate = value && typeof value === "object" ? value : {};
    const modes = new Set<ThemeSettings["mode"]>(["light", "dark", "night"]);
    const fonts = new Set<NonNullable<ThemeSettings["fontFamily"]>>([
      "sans-serif",
      "serif",
      "monospace",
    ]);
    const bases = new Set<NonNullable<ThemeSettings["themeBase"]>>([
      "slate",
      "gray",
      "zinc",
      "neutral",
      "stone",
    ]);
    const radii = new Set([0, 0.5, 1, 1.5, 2]);
    const accent = accessibleAccent(
      typeof candidate.accentColor === "string"
        ? candidate.accentColor
        : DEFAULT_THEME_SETTINGS.accentColor,
    ).color;
    return {
      ...DEFAULT_THEME_SETTINGS,
      mode: modes.has(candidate.mode as ThemeSettings["mode"])
        ? (candidate.mode as ThemeSettings["mode"])
        : DEFAULT_THEME_SETTINGS.mode,
      accentColor: accent,
      fontFamily: fonts.has(
        candidate.fontFamily as NonNullable<ThemeSettings["fontFamily"]>,
      )
        ? candidate.fontFamily
        : DEFAULT_THEME_SETTINGS.fontFamily,
      themeBase: bases.has(
        candidate.themeBase as NonNullable<ThemeSettings["themeBase"]>,
      )
        ? candidate.themeBase
        : DEFAULT_THEME_SETTINGS.themeBase,
      cornerRadius: radii.has(candidate.cornerRadius ?? Number.NaN)
        ? candidate.cornerRadius
        : DEFAULT_THEME_SETTINGS.cornerRadius,
      density: ["compact", "normal", "spacious"].includes(
        candidate.density ?? "",
      )
        ? (candidate.density as ThemeSettings["density"])
        : DEFAULT_THEME_SETTINGS.density,
      fontSize: ["sm", "md", "lg"].includes(candidate.fontSize ?? "")
        ? candidate.fontSize
        : DEFAULT_THEME_SETTINGS.fontSize,
    };
  }

  function select(section: Section): void {
    mobileNavigationOpen = false;
    activeSection = section;
    if (typeof window === "undefined") return;
    const path = pathForSection(section);
    if (window.location.pathname !== path) {
      window.history.pushState({}, "", path);
    }
    window.requestAnimationFrame(() =>
      document.getElementById("main-content")?.focus(),
    );
  }

  function openProviderSettings(): void {
    settingsTab = "providers";
    select("settings");
  }

  function handleSelectSection(section: string): void {
    select(section as Section);
  }

  function hrefForSection(section: ActiveNavSection): string {
    return pathForSection(section as Section);
  }

  async function hideNavigation(): Promise<void> {
    workbenchPreferences = {
      ...workbenchPreferences,
      sidebarHidden: true,
    };
    await tick();
    showNavigationTrigger?.focus();
  }

  function focusNavigationEntry(): void {
    const navigation = document.querySelector<HTMLElement>(
      "#fasti-main-navigation",
    );
    (
      navigation?.querySelector<HTMLElement>('[aria-current="page"]') ??
      navigation?.querySelector<HTMLElement>(".nav-link[href]") ??
      navigation?.querySelector<HTMLElement>("button:not([disabled])")
    )?.focus();
  }

  async function showNavigation(): Promise<void> {
    if (isNarrowViewport) {
      flushSync(() => (mobileNavigationOpen = true));
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          if (!mobileNavigationOpen) return;
          focusNavigationEntry();
        });
      });
      return;
    }
    workbenchPreferences = {
      ...workbenchPreferences,
      sidebarHidden: false,
    };
    await tick();
    focusNavigationEntry();
  }

  async function closeMobileNavigation(): Promise<void> {
    mobileNavigationOpen = false;
    await tick();
    navigationTrigger?.focus();
  }

  function openRecord(
    recordId: string,
    tab: "overview" | "sources" = "overview",
  ): void {
    selectedRecordId = recordId;
    selectedRecordTab = tab;
    select("detail");
  }

  // --- Discover: Google Books search, lazily loaded on first visit ---
  let discoverProviders = $state<ProviderCredentialStatus[] | undefined>(
    undefined,
  );
  let discoverLoading = $state(false);
  let discoverHostProblem = $state<string | undefined>(undefined);
  let discoverSelectedProviderId = $state("");
  let discoverSelectionExplicit = $state(false);
  let discoverLoadId = 0;
  let discoverSectionActive = false;

  async function loadDiscover(): Promise<void> {
    const loadId = ++discoverLoadId;
    discoverLoading = true;
    discoverHostProblem = undefined;
    try {
      const providers = await host.providerCredentialStatus();
      if (loadId === discoverLoadId) discoverProviders = providers;
    } catch (error) {
      if (loadId === discoverLoadId) {
        discoverProviders = undefined;
        discoverHostProblem = hostProblemText(
          error,
          "Could not load provider status from the host.",
        );
      }
    } finally {
      if (loadId === discoverLoadId) discoverLoading = false;
    }
  }

  function invalidateDiscoverProviders(): void {
    discoverLoadId += 1;
    discoverLoading = false;
    discoverProviders = undefined;
  }

  // --- Reconciliation: review inbox, lazily loaded on first visit ---
  let reviews = $state<ReviewItem[]>([]);
  let reviewsLoading = $state(false);
  let reviewsProblem = $state<string | undefined>(undefined);
  let reviewsLoaded = false;
  let resolvingReviewId = $state<string | undefined>(undefined);

  async function loadReviews(): Promise<void> {
    if (!host.listReviews) {
      reviewsProblem = "This host does not support review listing yet.";
      return;
    }
    reviewsLoading = true;
    reviewsProblem = undefined;
    try {
      reviews = await host.listReviews();
    } catch (error) {
      reviewsProblem = hostProblemText(
        error,
        "Could not load the review inbox from the host.",
      );
    } finally {
      reviewsLoading = false;
    }
  }

  async function resolveReview(input: ResolveReviewInput): Promise<void> {
    if (!host.resolveReview || resolvingReviewId) return;
    resolvingReviewId = input.review_item_id;
    try {
      await host.resolveReview(input);
      await loadReviews();
    } catch (error) {
      reviewsProblem = hostProblemText(error, "Could not resolve that review.");
    } finally {
      if (resolvingReviewId === input.review_item_id) {
        resolvingReviewId = undefined;
      }
    }
  }

  const openReviewCount = $derived(
    reviews.filter((item) => item.status === "open").length,
  );

  // --- Records: Library / Calendar / Media Detail, lazily loaded on first visit ---
  let mediaRecords = $state<MediaRecord[]>([]);
  let recordsLoading = $state(false);
  let recordsProblem = $state<string | undefined>(undefined);
  let recordActionProblem = $state<string | undefined>(undefined);
  let recordActionNotice = $state<string | undefined>(undefined);
  let recordsLoaded = false;

  async function loadRecords(restoreRetryFocus = false): Promise<boolean> {
    if (!host.listRecords) {
      recordsProblem = "This host does not support record listing yet.";
      return false;
    }
    const showLoading = mediaRecords.length === 0;
    if (showLoading) recordsLoading = true;
    recordsProblem = undefined;
    try {
      const statesPromise = host.listTrackingDispositions
        ? host.listTrackingDispositions().catch((error) => {
            const detail = hostProblemText(error, "Fasti request failed.");
            recordActionNotice = `Could not load profile tracking state. Records still use their activity fallback. ${detail}`;
            return { states: [], truncated: false };
          })
        : Promise.resolve({ states: [], truncated: false });
      const [summaries, statePage] = await Promise.all([
        host.listRecords(),
        statesPromise,
      ]);
      if (statePage.truncated) {
        recordActionProblem =
          "Only the first 500 profile tracking states are shown. Additional states remain stored.";
      }
      const dispositions = new Map(
        statePage.states.map((state) => [state.record_id, state.disposition]),
      );
      mediaRecords = summaries.map((summary) =>
        projectRecordSummary(summary, dispositions.get(summary.record_id)),
      );
    } catch (error) {
      recordsProblem = hostProblemText(
        error,
        "Could not load records from the host.",
      );
      return false;
    } finally {
      if (showLoading) recordsLoading = false;
      if (restoreRetryFocus) {
        await tick();
        document.getElementById("retry-records")?.focus();
      }
    }
    return true;
  }

  async function setTrackingDisposition(
    recordId: string,
    disposition: TrackingDispositionUpdate,
  ): Promise<void> {
    if (!host.setTrackingDisposition) {
      recordActionProblem =
        "Profile tracking state is not available on this host.";
      return;
    }
    recordActionProblem = undefined;
    recordActionNotice = undefined;
    try {
      const state = await host.setTrackingDisposition(recordId, disposition);
      mediaRecords = mediaRecords.map((record) =>
        record.id === recordId
          ? {
              ...record,
              status:
                state.disposition ??
                (record.lastActivityAt ? "watching" : "plan_to_watch"),
              trackingDisposition: state.disposition,
            }
          : record,
      );
      recordActionNotice =
        disposition === "unset"
          ? "Tracking state now follows recorded activity."
          : `Tracking state set to ${disposition.replaceAll("_", " ")}.`;
    } catch (error) {
      recordActionProblem = hostProblemText(
        error,
        "Could not update the profile tracking state.",
      );
    }
  }

  async function createRecordFromDiscover(
    candidate: ProviderSearchCandidate,
  ): Promise<CreateRecordResult> {
    if (!host.trackProviderCandidate) {
      throw new Error(
        "Creating a Record from provider metadata is not available on this host.",
      );
    }
    const result = await host.trackProviderCandidate({
      provider: candidate.provider,
      provider_id: candidate.provider_id,
      kind: candidate.kind,
    });
    recordsLoaded = false;
    await loadRecords();
    return result;
  }

  function resetClientEndpoint(): void {
    browserSession = null;
    browserSessionChecked = false;
    mediaRecords = [];
    recordsLoaded = false;
    recordsProblem = undefined;
    recordActionNotice = undefined;
    recordActionProblem = undefined;
    void refreshBrowserSession();
  }

  function retryRecords(): void {
    recordsLoaded = true;
    void loadRecords(true);
  }

  async function applyProviderMetadata(
    recordId: string,
    selection: ProviderSelection,
  ): Promise<void> {
    if (!host.applyProviderMetadata) {
      throw new Error(
        "Metadata refresh is only available in the trusted desktop host.",
      );
    }
    recordActionProblem = undefined;
    recordActionNotice = undefined;
    try {
      await host.applyProviderMetadata(recordId, selection);
      await loadRecords();
      recordActionNotice = `Metadata refreshed from ${selection.provider}.`;
    } catch (error) {
      recordActionProblem = hostProblemText(
        error,
        "Could not refresh metadata for this record.",
      );
      throw error;
    }
  }

  const watchingRecords = $derived(
    mediaRecords.filter((record) => record.status === "watching"),
  );
  const selectedRecord = $derived(
    mediaRecords.find((record) => record.id === selectedRecordId),
  );
  const showsRecordFeedback = $derived(
    activeSection === "home" ||
      activeSection === "library" ||
      activeSection === "calendar" ||
      activeSection === "detail",
  );

  $effect(() => {
    const needsDiscoverProviders =
      activeSection === "discover" || activeSection === "detail";
    if (needsDiscoverProviders && !discoverSectionActive) {
      discoverSectionActive = true;
      void loadDiscover();
    } else if (!needsDiscoverProviders) {
      discoverSectionActive = false;
    }
    if (activeSection === "reconciliation" && !reviewsLoaded) {
      reviewsLoaded = true;
      void loadReviews();
    }
    if (
      (!host.currentBrowserSession ||
        (browserSessionChecked && browserSession !== null)) &&
      !recordsLoaded
    ) {
      recordsLoaded = true;
      void loadRecords();
    }
  });

  // Tabler's `lg` boundary converts the vertical navbar into an offcanvas.
  // The persisted collapsed/hidden preferences remain desktop choices; a
  // narrow viewport always starts with a full-width canvas and closed nav.
  let isNarrowViewport = $state(
    typeof window !== "undefined" &&
      window.matchMedia("(max-width: 61.99rem)").matches,
  );

  $effect(() => {
    if (typeof document === "undefined") return;
    const locked = isNarrowViewport && mobileNavigationOpen;
    document.body.classList.toggle("fasti-navigation-open", locked);
    return () => document.body.classList.remove("fasti-navigation-open");
  });

  async function refreshBrowserSession(): Promise<void> {
    if (!host.currentBrowserSession) {
      browserSessionChecked = true;
      return;
    }
    browserSessionChecked = false;
    try {
      browserSession = await host.currentBrowserSession();
    } catch {
      browserSession = null;
      recordsProblem = "Sign in to load records from this Fasti service.";
    } finally {
      browserSessionChecked = true;
    }
  }

  onMount(() => {
    activeSection = sectionFromPath();
    const sync = () => (activeSection = sectionFromPath());
    window.addEventListener("popstate", sync);
    const media = window.matchMedia("(max-width: 61.99rem)");
    const syncViewport = () => {
      isNarrowViewport = media.matches;
      if (!media.matches) mobileNavigationOpen = false;
    };
    const closeNavigationOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !isNarrowViewport || !mobileNavigationOpen)
        return;
      event.preventDefault();
      void closeMobileNavigation();
    };
    syncViewport();
    media.addEventListener("change", syncViewport);
    document.addEventListener("keydown", closeNavigationOnEscape);
    void refreshBrowserSession();
    return () => {
      window.removeEventListener("popstate", sync);
      media.removeEventListener("change", syncViewport);
      document.removeEventListener("keydown", closeNavigationOnEscape);
    };
  });

  function formatSectionTitle(section: Section): string {
    const titles: Record<Section, string> = {
      home: "Overview",
      connections: "Connections",
      settings: "Settings",
      discover: "Discover",
      reconciliation: "Review Inbox",
      library: "Library",
      calendar: "Calendar",
      detail: "Media Detail",
    };
    return titles[section];
  }
</script>

{#snippet recordStatus()}
  {#if recordsLoading}
    <p class="record-load-status alert alert-info" role="status">
      Loading records…
    </p>
  {:else if recordsProblem}
    <section class="record-access-alert alert alert-warning" role="alert">
      <div>
        <strong class="record-access-title">Records are unavailable</strong>
        <p>{recordsProblem}</p>
      </div>
      {#if host.currentBrowserSession && !browserSession}
        <button
          type="button"
          class="btn btn-primary"
          onclick={() => (authModalOpen = true)}>Sign in</button
        >
      {:else}
        <button
          id="retry-records"
          type="button"
          class="btn btn-primary"
          onclick={retryRecords}>Retry records</button
        >
      {/if}
    </section>
  {/if}
{/snippet}

<div class="page workbench-shell">
  <NavSidebar
    {activeSection}
    navItems={workbenchPreferences.navItems}
    {openReviewCount}
    collapsed={workbenchPreferences.sidebarCollapsed}
    hidden={workbenchPreferences.sidebarHidden}
    narrowViewport={isNarrowViewport}
    mobileOpen={mobileNavigationOpen}
    {hrefForSection}
    onToggleCollapse={() =>
      (workbenchPreferences = {
        ...workbenchPreferences,
        sidebarCollapsed: !workbenchPreferences.sidebarCollapsed,
      })}
    onToggleHide={() => void hideNavigation()}
    onCloseMobile={() => void closeMobileNavigation()}
    onSelectSection={handleSelectSection}
  />

  <div
    class="page-wrapper workbench-main-shell"
    class:sidebar-collapsed={workbenchPreferences.sidebarCollapsed}
    class:sidebar-hidden={workbenchPreferences.sidebarHidden}
    inert={isNarrowViewport && mobileNavigationOpen}
  >
    <header
      class="navbar navbar-expand-md top-bar"
      aria-label="Workbench toolbar"
    >
      <div class="top-bar-left">
        {#if isNarrowViewport}
          <button
            bind:this={navigationTrigger}
            type="button"
            class="btn btn-icon btn-ghost-secondary icon-btn"
            aria-controls="fasti-main-navigation"
            aria-expanded={mobileNavigationOpen}
            onclick={() => void showNavigation()}
            title="Open navigation"
            aria-label="Open navigation"
          >
            <IconLayoutSidebar size={18} />
          </button>
        {:else if workbenchPreferences.sidebarHidden}
          <button
            bind:this={showNavigationTrigger}
            type="button"
            class="btn btn-icon btn-ghost-secondary icon-btn"
            onclick={() => void showNavigation()}
            title="Show navigation"
            aria-label="Show navigation"
          >
            <IconLayoutSidebarLeftExpand size={18} />
          </button>
        {/if}
        <span class="section-title">{formatSectionTitle(activeSection)}</span>
      </div>

      <GlobalSearch
        records={mediaRecords}
        navItems={workbenchPreferences.navItems}
        onSelectRecord={openRecord}
        onSelectSection={handleSelectSection}
      />

      <div class="top-bar-actions">
        <a
          class="icon-btn"
          href="/status"
          title="Service status"
          aria-label="Service status"
          onclick={(event) => {
            if (!onOpenStatus) return;
            event.preventDefault();
            onOpenStatus();
          }}
        >
          <IconActivityHeartbeat size={18} />
        </a>

        <button
          type="button"
          class="icon-btn"
          onclick={() => (themeDrawerOpen = true)}
          title="Theme settings"
          aria-label="Theme settings"
        >
          <IconPalette size={18} />
        </button>

        <button
          type="button"
          class="icon-btn"
          onclick={() => (authModalOpen = true)}
          title={browserSession
            ? `Account: ${browserSession.user.username}`
            : "Account access"}
          aria-label={browserSession
            ? `Manage account ${browserSession.user.username}`
            : "Open account access"}
        >
          <IconUserCircle size={18} />
        </button>
      </div>
    </header>

    <main id="main-content" class="page-body main-content" tabindex="-1">
      {#if showsRecordFeedback && recordActionNotice}
        <p class="record-action-feedback" role="status">
          {recordActionNotice}
        </p>
      {/if}
      {#if showsRecordFeedback && recordActionProblem}
        <p class="record-action-feedback problem" role="alert">
          {recordActionProblem}
        </p>
      {/if}
      {#if activeSection === "connections"}
        <ConnectionsView {host} />
      {:else if activeSection === "settings"}
        <RuntimeSettingsView
          {host}
          {workbenchPreferences}
          canAccessProfileData={!host.currentBrowserSession ||
            (browserSessionChecked && browserSession !== null)}
          profileDataIdentity={host.currentBrowserSession
            ? (browserSession?.user.user_id ?? "signed-out")
            : "trusted-host"}
          activeTab={settingsTab}
          onTabChange={(tab: SettingsTab) => {
            settingsTab = tab;
            if (typeof window !== "undefined") {
              const newPath = pathForSettingsTab(tab);
              if (window.location.pathname !== newPath) {
                window.history.pushState(null, "", newPath);
              }
            }
          }}
          onClientEndpointChanged={resetClientEndpoint}
          onProviderCredentialsChanged={invalidateDiscoverProviders}
          onUpdateWorkbenchPreferences={(patch) =>
            (workbenchPreferences = { ...workbenchPreferences, ...patch })}
        />
      {:else if activeSection === "discover"}
        <DiscoverView
          providerCredentials={discoverProviders}
          loading={discoverLoading}
          hostProblem={discoverHostProblem}
          bind:selectedProviderId={discoverSelectedProviderId}
          bind:selectionExplicit={discoverSelectionExplicit}
          onSearch={(provider, query) => host.searchProvider(provider, query)}
          onOpenSettings={openProviderSettings}
          onRetry={() => loadDiscover()}
          onCandidateAction={host.trackProviderCandidate
            ? createRecordFromDiscover
            : undefined}
        />
      {:else if activeSection === "reconciliation"}
        <ReconciliationView
          items={reviewsProblem ? [] : reviews}
          loading={reviewsLoading}
          unavailableReason={reviewsProblem}
          {resolvingReviewId}
          onResolveExisting={host.resolveReview
            ? (reviewItemId, recordId) =>
                resolveReview({
                  review_item_id: reviewItemId,
                  target: { kind: "existing", value: recordId },
                  identifiers: [],
                })
            : undefined}
          onResolveNew={host.resolveReview
            ? (reviewItemId, grain) =>
                resolveReview({
                  review_item_id: reviewItemId,
                  target: { kind: "new", value: grain },
                  identifiers: [],
                })
            : undefined}
        />
      {:else if activeSection === "library"}
        {@render recordStatus()}
        <LibraryView
          records={recordsProblem ? [] : mediaRecords}
          availableCollections={[]}
          onSelectRecord={openRecord}
          contextMenuConfigs={workbenchPreferences.contextMenuItems}
          onSetTrackingDisposition={(recordId, disposition) =>
            void setTrackingDisposition(recordId, disposition)}
          onOpenReconciliation={() => select("reconciliation")}
        />
      {:else if activeSection === "calendar"}
        {@render recordStatus()}
        <CalendarView
          watchingRecords={recordsProblem ? [] : watchingRecords}
          stateUnavailable={!recordsProblem &&
            mediaRecords.some((record) => record.status === "unknown")}
          onSelectRecord={openRecord}
        />
      {:else if activeSection === "detail"}
        {@render recordStatus()}
        {#if selectedRecord && !recordsProblem}
          <MediaDetailView
            record={selectedRecord}
            availableCollections={[]}
            initialTab={selectedRecordTab}
            contextMenuConfigs={workbenchPreferences.contextMenuItems}
            providerCredentials={discoverProviders}
            providerLoading={discoverLoading}
            providerHostProblem={discoverHostProblem}
            onBack={() => select("library")}
            onSearchMetadata={(provider, query) =>
              host.searchProvider(provider, query)}
            onApplyMetadata={host.applyProviderMetadata
              ? applyProviderMetadata
              : undefined}
            onOpenProviderSettings={openProviderSettings}
            onRetryProviders={() => loadDiscover()}
            onSetTrackingDisposition={(recordId, disposition) =>
              void setTrackingDisposition(recordId, disposition)}
            onOpenReconciliation={() => select("reconciliation")}
          />
        {:else}
          <div class="state-message">
            <h1>Media Detail</h1>
            <p>No record is selected.</p>
            <button
              type="button"
              class="link-btn"
              onclick={() => select("library")}>Choose one from Library</button
            >
          </div>
        {/if}
      {:else if activeSection === "home"}
        {@render recordStatus()}
        <HomeView
          records={recordsProblem ? [] : mediaRecords}
          availableCollections={[]}
          onSelectRecord={openRecord}
          contextMenuConfigs={workbenchPreferences.contextMenuItems}
          onSetTrackingDisposition={(recordId, disposition) =>
            void setTrackingDisposition(recordId, disposition)}
          onOpenReconciliation={() => select("reconciliation")}
        />
      {:else}
        <div class="overview">
          <header class="overview-header">
            <p class="eyebrow">Local workbench</p>
            <h1>Current Fasti capability</h1>
            <p>
              This surface reports only behavior that the active host and local
              API can perform. It does not load sample media or substitute
              browser storage for the Chronicle.
            </p>
          </header>

          <section class="truth-grid" aria-label="Current capability status">
            <article>
              <IconShieldCheck size={28} aria-hidden="true" />
              <div>
                <h2>Durable occurrence ingress</h2>
                <p>
                  <strong>Active on the local API.</strong> Scoped bearer
                  clients can submit complete consumption occurrences to
                  <code>POST /api/v1/observations</code>. Fasti stores evidence,
                  applies idempotency, and returns a durable receipt.
                </p>
              </div>
            </article>

            <article>
              <IconDatabase size={28} aria-hidden="true" />
              <div>
                <h2>Media library presentation</h2>
                <p>
                  <strong>Record listing is active.</strong> Chronicle listing, metadata
                  editing, collections, ratings, imports, and progress still need
                  their own application and public contracts before this workbench
                  can present them as working product state.
                </p>
                <button
                  type="button"
                  class="inline-action"
                  onclick={() => select("library")}
                >
                  Open Library <IconChevronRight size={17} aria-hidden="true" />
                </button>
              </div>
            </article>

            <article>
              <IconPlugConnected size={28} aria-hidden="true" />
              <div>
                <h2>Nuvio pathway</h2>
                <p>
                  <strong
                    >Fasti-side occurrence ingress is ready for an authenticated
                    observer.</strong
                  >
                  Current upstream Nuvio exposes Trakt and SIMKL tracking providers,
                  not Fasti. Native Nuvio pairing, progress synchronization, and two-way
                  state are therefore not claimed here.
                </p>
                <button
                  type="button"
                  class="inline-action"
                  onclick={() => select("connections")}
                >
                  Open Connections <IconChevronRight
                    size={17}
                    aria-hidden="true"
                  />
                </button>
              </div>
            </article>

            <article>
              <IconSettings size={28} aria-hidden="true" />
              <div>
                <h2>External client credentials</h2>
                <p>
                  {credentialAdministration
                    ? "The trusted packaged host can create, list, and revoke independently scoped API client credentials. Plaintext is returned once and is not stored by the workbench."
                    : "This host does not expose credential administration. Browser distributions fail closed and do not create or persist API bearer secrets."}
                </p>
                <button
                  type="button"
                  class="inline-action"
                  onclick={() => select("connections")}
                >
                  Manage API clients <IconChevronRight
                    size={17}
                    aria-hidden="true"
                  />
                </button>
              </div>
            </article>
          </section>

          <section class="next-step" aria-labelledby="next-step-title">
            <h2 id="next-step-title">Next implementation gate</h2>
            <p>
              Activate Chronicle query/mutation contracts and bind metadata
              editing, collections, ratings, and progress to the media UI. Until
              that gate passes, Fasti keeps the richer prototype out of the
              runtime path instead of presenting fake success.
            </p>
          </section>
        </div>
      {/if}
    </main>
  </div>
</div>

<TablerThemeDrawer
  open={themeDrawerOpen}
  {themeSettings}
  onClose={() => (themeDrawerOpen = false)}
  onUpdateTheme={updateTheme}
/>

<AuthModal
  show={authModalOpen}
  {host}
  session={browserSession}
  onClose={() => (authModalOpen = false)}
  onSessionChange={(session) => {
    browserSession = session;
    if (!session) {
      mediaRecords = [];
      recordsLoaded = false;
      recordsProblem = undefined;
      recordActionNotice = undefined;
      recordActionProblem = undefined;
    }
  }}
/>

<style>
  .workbench-shell {
    --fasti-collapsed-navigation-width: 4rem;
    min-height: 100dvh;
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .workbench-main-shell {
    min-width: 0;
  }

  @media (min-width: 62rem) {
    :global(.fasti-sidebar-vertical.navbar-vertical-collapsed)
      + .workbench-main-shell.sidebar-collapsed {
      margin-left: var(--fasti-collapsed-navigation-width);
    }

    :global(.fasti-sidebar-vertical.desktop-hidden)
      + .workbench-main-shell.sidebar-hidden {
      margin-left: 0;
    }
  }

  :global(body.fasti-navigation-open) {
    overflow: hidden;
  }

  .top-bar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 16px;
    background: var(--fasti-surface-paper);
    border-bottom: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
  }

  .top-bar-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 0 1 220px;
  }

  .top-bar-actions {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    gap: 4px;
  }

  .section-title {
    font-family: var(--fasti-font-display);
    font-weight: 700;
    font-size: 1.05rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 44px;
    min-height: 44px;
    border: 0;
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
    background: transparent;
    color: var(--fasti-text-muted);
    cursor: pointer;
    text-decoration: none;
  }

  .icon-btn:hover {
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .icon-btn:focus-visible,
  .main-content:focus-visible,
  .link-btn:focus-visible {
    outline: 3px solid var(--fasti-focus);
    outline-offset: 2px;
  }

  .main-content {
    min-width: 0;
    margin-bottom: 0;
  }

  .record-action-feedback {
    margin: 0;
    padding: 9px 16px;
    border-bottom: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 10%,
      var(--fasti-surface-paper)
    );
    color: var(--fasti-text-primary);
    font-size: 0.86rem;
  }

  .record-action-feedback.problem {
    background: color-mix(
      in srgb,
      var(--fasti-state-error, #b42318) 10%,
      var(--fasti-surface-paper)
    );
    color: var(--fasti-state-error, #b42318);
  }

  .state-message {
    max-width: 640px;
    margin: 48px auto;
    padding: 0 24px;
    text-align: center;
    color: var(--fasti-text-muted);
  }

  .record-load-status,
  .record-access-alert {
    max-width: 1080px;
    margin: 24px auto 0;
  }

  .record-access-alert {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }

  .record-access-alert p {
    margin: 4px 0 0;
  }

  .record-access-title {
    color: var(--fasti-text-primary);
  }

  .record-access-alert .btn {
    min-width: max-content;
    min-height: 44px;
  }

  .link-btn {
    min-height: var(--fasti-touch-target-min);
    border: 1px solid var(--fasti-action-primary);
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    font-weight: 700;
    cursor: pointer;
    padding: 10px 16px;
  }

  .overview {
    max-width: 1080px;
    margin: 0 auto;
    padding: 48px 32px 72px;
  }

  .overview-header {
    max-width: 72ch;
    margin-bottom: 32px;
  }

  .eyebrow {
    margin: 0 0 6px;
    color: var(--fasti-brand-mark);
    font-family: var(--fasti-font-mono);
    font-size: 0.78rem;
    font-weight: 750;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  h1,
  h2,
  p {
    margin-top: 0;
  }

  h1,
  h2 {
    font-family: var(--fasti-font-display);
  }

  h1 {
    margin-bottom: 8px;
    font-size: clamp(2rem, 5vw, 3rem);
  }

  .overview-header > p:last-child,
  article p,
  .next-step p {
    color: var(--fasti-text-muted);
    line-height: 1.6;
  }

  .truth-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 18px;
  }

  article {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 20px;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: calc(8px * var(--tblr-border-radius-scale, 1));
    background: var(--fasti-surface-paper);
  }

  article > :global(svg) {
    flex: 0 0 auto;
    color: var(--fasti-action-primary);
  }

  article h2 {
    margin-bottom: 6px;
    font-size: 1.2rem;
  }

  article p {
    margin-bottom: 0;
  }

  article code {
    overflow-wrap: anywhere;
  }

  .inline-action {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 12px;
    border: 0;
    background: transparent;
    color: var(--fasti-action-primary);
    padding: 6px 0;
    font-weight: 700;
    cursor: pointer;
  }

  .next-step {
    margin-top: 24px;
    padding: 20px;
    border: 1px dashed
      var(--fasti-border, color-mix(in srgb, currentColor 25%, transparent));
    border-radius: calc(8px * var(--tblr-border-radius-scale, 1));
  }

  .next-step h2 {
    margin-bottom: 5px;
    font-size: 1.2rem;
  }

  .next-step p {
    margin-bottom: 0;
  }

  @media (max-width: 56rem) {
    .top-bar {
      flex-wrap: wrap;
    }

    .top-bar-left {
      flex: 1 1 auto;
    }

    .top-bar :global(.global-search) {
      order: 3;
      flex: 1 0 100%;
      width: 100%;
    }

    .overview {
      padding: 32px 20px 56px;
    }

    .truth-grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    * {
      scroll-behavior: auto !important;
    }
  }
</style>
