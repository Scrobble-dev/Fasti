<script lang="ts">
  import { onMount } from "svelte";
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
    MediaRecord,
    ProviderCredentialStatus,
    ProviderSearchCandidate,
    ResolveReviewInput,
    ReviewItem,
    ThemeSettings,
    WorkbenchHost,
    WorkbenchPreferences,
  } from "./types.js";

  interface Props {
    host: WorkbenchHost;
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

  let { host }: Props = $props();

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

  function accessibleAccent(value: string): {
    color: string;
    contrast: "#000000" | "#ffffff";
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
    };
  }

  function pathForSection(section: Section): string {
    switch (section) {
      case "connections":
        return "/connections";
      case "settings":
        return "/settings";
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
    if (path === "/settings") return "settings";
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
      const supportedStored = Array.isArray(storedItems)
        ? storedItems.filter((item) => supportedIds.has(item?.id))
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
    loadPersisted("fasti-theme-settings", DEFAULT_THEME_SETTINGS),
  );
  let themeDrawerOpen = $state(false);
  let authModalOpen = $state(false);
  let sessionCredentialActive = $state(false);

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
    } catch {}
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    // Both "dark" and "night" use the same dark token set; see
    // packages/tokens/src/index.ts's `[data-bs-theme="dark"]` block.
    root.dataset.bsTheme = themeSettings.mode === "light" ? "light" : "dark";
    root.style.colorScheme = themeSettings.mode === "light" ? "light" : "dark";
    if (themeSettings.accentColor) {
      const accent = accessibleAccent(themeSettings.accentColor);
      root.style.setProperty("--fasti-action-primary", accent.color);
      root.style.setProperty("--fasti-action-contrast", accent.contrast);
      root.style.setProperty("--tblr-primary", accent.color);
      root.style.setProperty("--tblr-primary-fg", accent.contrast);
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
      root.style.removeProperty("--fasti-font-display");
    }
  });

  function updateTheme(updates: Partial<ThemeSettings>): void {
    themeSettings = { ...themeSettings, ...updates };
  }

  function select(section: Section): void {
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

  function handleSelectSection(section: string): void {
    select(section as Section);
  }

  function openRecord(recordId: string): void {
    selectedRecordId = recordId;
    select("detail");
  }

  // --- Discover: Google Books search, lazily loaded on first visit ---
  let discoverProviders = $state<ProviderCredentialStatus[] | undefined>(
    undefined,
  );
  let discoverLoading = $state(false);
  let discoverHostProblem = $state<string | undefined>(undefined);
  let discoverLoaded = false;

  async function loadDiscover(): Promise<void> {
    discoverLoading = true;
    discoverHostProblem = undefined;
    try {
      discoverProviders = await host.providerCredentialStatus();
    } catch (error) {
      discoverProviders = undefined;
      discoverHostProblem = hostProblemText(
        error,
        "Could not load provider status from the host.",
      );
    } finally {
      discoverLoading = false;
    }
  }

  function invalidateDiscoverProviders(): void {
    discoverLoaded = false;
    discoverProviders = undefined;
  }

  // --- Reconciliation: review inbox, lazily loaded on first visit ---
  let reviews = $state<ReviewItem[]>([]);
  let reviewsLoading = $state(false);
  let reviewsProblem = $state<string | undefined>(undefined);
  let reviewsLoaded = false;

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
    if (!host.resolveReview) return;
    try {
      await host.resolveReview(input);
      await loadReviews();
    } catch (error) {
      reviewsProblem = hostProblemText(error, "Could not resolve that review.");
    }
  }

  const openReviewCount = $derived(
    reviews.filter((item) => item.status === "open").length,
  );

  // --- Records: Library / Calendar / Media Detail, lazily loaded on first visit ---
  let mediaRecords = $state<MediaRecord[]>([]);
  let recordsLoading = $state(false);
  let recordsProblem = $state<string | undefined>(undefined);
  let recordsLoaded = false;

  async function loadRecords(): Promise<boolean> {
    if (!host.listRecords) {
      recordsProblem = "This host does not support record listing yet.";
      return false;
    }
    recordsLoading = true;
    recordsProblem = undefined;
    try {
      mediaRecords = (await host.listRecords()).map(projectRecordSummary);
      return true;
    } catch (error) {
      recordsProblem = hostProblemText(
        error,
        "Could not load records from the host.",
      );
      return false;
    } finally {
      recordsLoading = false;
    }
  }

  async function connectSessionCredential(credential: string): Promise<void> {
    if (!host.setSessionCredential) {
      throw new Error("This host does not accept browser session credentials.");
    }
    host.setSessionCredential(credential);
    recordsLoaded = true;
    if (!(await loadRecords())) {
      const problem =
        recordsProblem ?? "The host rejected this browser credential.";
      host.clearSessionCredential?.();
      sessionCredentialActive = false;
      mediaRecords = [];
      throw new Error(problem);
    }
    sessionCredentialActive = true;
  }

  function clearSessionCredential(): void {
    host.clearSessionCredential?.();
    sessionCredentialActive = false;
    mediaRecords = [];
    recordsLoaded = false;
    recordsProblem =
      "Records need an active local bearer credential. Select Connect records and paste a credential with identity_read scope.";
  }

  /** Inverse of record-projection.ts's `mediaKindForGrain` -- picks one
   * representative grain per display kind so a provider search result can
   * become a record. */
  function grainForMediaKind(kind: string): string {
    switch (kind) {
      case "movie":
        return "film";
      case "show":
      case "anime":
        return "series";
      case "music":
        return "recording";
      case "book":
      case "manga":
      case "comic":
        return "chapter";
      case "podcast":
        return "podcast_feed";
      case "game":
        return "game_release";
      default:
        return "custom";
    }
  }

  async function trackRecordFromDiscover(
    candidate: ProviderSearchCandidate,
  ): Promise<void> {
    if (
      !host.createRecord ||
      !host.attachIdentifier ||
      !host.registerNamespace
    ) {
      throw new Error(
        "Adding titles to your library is not available on this host.",
      );
    }
    const grain = grainForMediaKind(candidate.kind);
    let namespace = candidate.provider;
    if (candidate.provider === "tmdb") {
      if (candidate.kind === "movie") namespace = "tmdb.movie";
      else if (candidate.kind === "show") namespace = "tmdb.tv";
      else throw new Error("TMDB returned an unsupported media kind.");
    }
    await host.registerNamespace({
      namespace,
      label: namespace,
      grains: [grain],
      id_pattern: ".+",
      normalization: "identity",
      licence_posture: "identifiers_only",
    });
    const created = await host.createRecord(grain);
    await host.attachIdentifier({
      record_id: created.record_id,
      namespace,
      grain,
      value: candidate.provider_id,
    });
    recordsLoaded = false;
    await loadRecords();
  }

  const watchingRecords = $derived(
    mediaRecords.filter((record) => record.status === "watching"),
  );
  const selectedRecord = $derived(
    mediaRecords.find((record) => record.id === selectedRecordId),
  );

  $effect(() => {
    if (activeSection === "discover" && !discoverLoaded) {
      discoverLoaded = true;
      void loadDiscover();
    }
    if (activeSection === "reconciliation" && !reviewsLoaded) {
      reviewsLoaded = true;
      void loadReviews();
    }
    if (
      (activeSection === "home" ||
        activeSection === "library" ||
        activeSection === "calendar" ||
        activeSection === "detail") &&
      !recordsLoaded
    ) {
      recordsLoaded = true;
      void loadRecords();
    }
  });

  // nav-sidebar.svelte turns itself into a fixed-position rail below this
  // breakpoint (see its own `@media (max-width: 47.99rem)` block). At a
  // phone width, an always-expanded 240px rail leaves too little room for
  // real content and forces horizontal scroll, which then slides content
  // out from under the fixed rail. Force the icon-only (64px) width there
  // regardless of the persisted preference, matching a standard mobile
  // nav-rail pattern.
  let isNarrowViewport = $state(
    typeof window !== "undefined" &&
      window.matchMedia("(max-width: 61.99rem)").matches,
  );

  onMount(() => {
    activeSection = sectionFromPath();
    const sync = () => (activeSection = sectionFromPath());
    window.addEventListener("popstate", sync);
    const media = window.matchMedia("(max-width: 61.99rem)");
    const syncViewport = () => (isNarrowViewport = media.matches);
    syncViewport();
    media.addEventListener("change", syncViewport);
    return () => {
      window.removeEventListener("popstate", sync);
      media.removeEventListener("change", syncViewport);
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
      {#if host.setSessionCredential}
        <button
          type="button"
          class="btn btn-primary"
          onclick={() => (authModalOpen = true)}>Connect records</button
        >
      {/if}
    </section>
  {/if}
{/snippet}

<div class="workbench-shell">
  <NavSidebar
    {activeSection}
    navItems={workbenchPreferences.navItems}
    {openReviewCount}
    collapsed={workbenchPreferences.sidebarCollapsed || isNarrowViewport}
    hidden={workbenchPreferences.sidebarHidden}
    onToggleCollapse={() =>
      (workbenchPreferences = {
        ...workbenchPreferences,
        sidebarCollapsed: !workbenchPreferences.sidebarCollapsed,
      })}
    onToggleHide={() =>
      (workbenchPreferences = {
        ...workbenchPreferences,
        sidebarHidden: true,
      })}
    onSelectSection={handleSelectSection}
  />

  <div
    class="workbench-main-shell"
    class:sidebar-hidden={workbenchPreferences.sidebarHidden}
  >
    <header class="top-bar" aria-label="Workbench toolbar">
      <div class="top-bar-left">
        {#if workbenchPreferences.sidebarHidden}
          <button
            type="button"
            class="icon-btn"
            onclick={() =>
              (workbenchPreferences = {
                ...workbenchPreferences,
                sidebarHidden: false,
              })}
            title="Show sidebar"
            aria-label="Show sidebar"
          >
            <IconLayoutSidebarLeftExpand size={18} />
          </button>
        {:else}
          <button
            type="button"
            class="icon-btn"
            onclick={() =>
              (workbenchPreferences = isNarrowViewport
                ? { ...workbenchPreferences, sidebarHidden: true }
                : {
                    ...workbenchPreferences,
                    sidebarCollapsed: !workbenchPreferences.sidebarCollapsed,
                  })}
            title={isNarrowViewport
              ? "Hide sidebar"
              : workbenchPreferences.sidebarCollapsed
                ? "Expand sidebar"
                : "Collapse sidebar"}
            aria-label={isNarrowViewport
              ? "Hide sidebar"
              : workbenchPreferences.sidebarCollapsed
                ? "Expand sidebar"
                : "Collapse sidebar"}
          >
            <IconLayoutSidebar size={18} />
          </button>
        {/if}
        <span class="section-title">{formatSectionTitle(activeSection)}</span>
      </div>

      <div class="top-bar-actions">
        <a
          class="icon-btn"
          href="/status"
          title="Service status"
          aria-label="Service status"
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

        {#if host.setSessionCredential}
          <button
            type="button"
            class="icon-btn"
            onclick={sessionCredentialActive
              ? clearSessionCredential
              : () => (authModalOpen = true)}
            title={sessionCredentialActive
              ? "Clear browser credential"
              : "Connect local credential"}
            aria-label={sessionCredentialActive
              ? "Clear browser credential"
              : "Connect local credential"}
          >
            {#if sessionCredentialActive}
              <IconLogout size={18} />
            {:else}
              <IconUserCircle size={18} />
            {/if}
          </button>
        {/if}
      </div>
    </header>

    <main id="main-content" class="main-content" tabindex="-1">
      {#if activeSection === "connections"}
        <ConnectionsView {host} />
      {:else if activeSection === "settings"}
        <RuntimeSettingsView
          {host}
          {workbenchPreferences}
          onClientEndpointChanged={clearSessionCredential}
          onProviderCredentialsChanged={invalidateDiscoverProviders}
          onUpdateWorkbenchPreferences={(patch) =>
            (workbenchPreferences = { ...workbenchPreferences, ...patch })}
        />
      {:else if activeSection === "discover"}
        <DiscoverView
          providerCredentials={discoverProviders}
          loading={discoverLoading}
          hostProblem={discoverHostProblem}
          onSearch={(provider, query) => host.searchProvider(provider, query)}
          onOpenSettings={() => select("settings")}
          onRetry={() => loadDiscover()}
          onTrackRecord={host.createRecord &&
          host.attachIdentifier &&
          host.registerNamespace
            ? trackRecordFromDiscover
            : undefined}
        />
      {:else if activeSection === "reconciliation"}
        <ReconciliationView
          items={reviewsProblem ? [] : reviews}
          loading={reviewsLoading}
          unavailableReason={reviewsProblem}
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
            onBack={() => select("library")}
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

{#if host.setSessionCredential}
  <AuthModal
    show={authModalOpen}
    onClose={() => (authModalOpen = false)}
    onSubmit={connectSessionCredential}
  />
{/if}

<style>
  .workbench-shell {
    min-height: 100dvh;
    display: flex;
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .workbench-main-shell {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  /* Below Tabler's `lg` boundary, nav-sidebar.svelte switches itself to
   * `position: fixed`, removing it from flex flow so it stops claiming
   * layout space. Reserve that space here (matching its collapsed width,
   * which `isNarrowViewport` forces at this breakpoint) so the fixed rail
   * doesn't overlap and intercept clicks on the main content. */
  @media (max-width: 61.99rem) {
    .workbench-main-shell {
      margin-left: 64px;
    }

    .workbench-main-shell.sidebar-hidden {
      margin-left: 0;
    }
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
  }

  .top-bar-actions {
    display: flex;
    align-items: center;
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
    border-radius: 6px;
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
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }

  .main-content {
    min-width: 0;
    flex: 1;
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
    border: 1px solid var(--fasti-action-primary);
    border-radius: 4px;
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
    border-radius: 8px;
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
    border-radius: 8px;
  }

  .next-step h2 {
    margin-bottom: 5px;
    font-size: 1.2rem;
  }

  .next-step p {
    margin-bottom: 0;
  }

  @media (max-width: 56rem) {
    .record-load-status,
    .record-access-alert {
      margin-inline: 20px;
    }

    .record-access-alert {
      align-items: stretch;
      flex-direction: column;
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
