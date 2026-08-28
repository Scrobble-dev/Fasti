<script lang="ts">
  import type {
    CustomFieldDefinition,
    CustomMediaTypeDefinition,
    MediaKind,
    ScopedApiToken,
    ProviderCredentialStatus,
    NetworkConfiguration,
    SaveNetworkConfigurationRequest,
    EndpointConnectionStatus,
    OidcConfiguration,
    AppriseNotificationConfig,
    ThemeSettings,
    WorkbenchPreferences,
    NavItemConfig,
    ContextMenuItemConfig,
  } from "./types.js";
  import { createDefaultWorkbenchPreferences } from "./defaults.js";
  import NetworkSettings from "./network-settings.svelte";
  import TmdbAttribution from "./tmdb-attribution.svelte";
  import { hostProblemText } from "./host-problem.js";
  import {
    IconKey,
    IconPalette,
    IconLayoutSidebar,
    IconPin,
    IconArrowUp,
    IconArrowDown,
    IconEye,
    IconEyeOff,
    IconRotate2,
    IconDevices,
    IconUserCheck,
    IconBell,
    IconDatabaseImport,
    IconCode,
    IconPlus,
    IconTrash,
    IconDeviceTv,
    IconExternalLink,
    IconSettings,
    IconWorld,
    IconTags,
    IconDatabase,
    IconFileDownload,
    IconBug,
  } from "@tabler/icons-svelte";

  interface Props {
    customFields: CustomFieldDefinition[];
    tokens: ScopedApiToken[];
    providerKeys?: ProviderCredentialStatus[];
    networkConfiguration?: NetworkConfiguration;
    providerLoading?: boolean;
    networkLoading?: boolean;
    providerLoadProblem?: string;
    networkLoadProblem?: string;
    initialSection?: "providers" | "advanced";
    oidcConfig: OidcConfiguration;
    appriseConfig: AppriseNotificationConfig;
    themeSettings: ThemeSettings;
    workbenchPreferences?: WorkbenchPreferences;
    onUpdateTheme?: (theme: Partial<ThemeSettings>) => void;
    onUpdateWorkbenchPreferences?: (
      prefs: Partial<WorkbenchPreferences>,
    ) => void;
    onSaveProviderKey?: (provider: string, key: string) => Promise<void>;
    onDeleteProviderKey?: (provider: string) => Promise<void>;
    onSaveNetworkConfiguration?: (
      input: SaveNetworkConfigurationRequest,
    ) => Promise<NetworkConfiguration>;
    onTestEndpoint?: (endpoint: string) => Promise<EndpointConnectionStatus>;
    onRetryProviderState?: () => void;
    onRetryNetworkState?: () => void;
    onSaveOidc?: (config: OidcConfiguration) => void;
    onSaveApprise?: (config: AppriseNotificationConfig) => void;
    onClearCache?: (
      cache: "search" | "history" | "statistics" | "discover" | "all",
    ) => void;
  }

  let {
    customFields,
    tokens,
    providerKeys,
    networkConfiguration,
    providerLoading = false,
    networkLoading = false,
    providerLoadProblem,
    networkLoadProblem,
    initialSection,
    oidcConfig,
    appriseConfig,
    themeSettings,
    workbenchPreferences = createDefaultWorkbenchPreferences(),
    onUpdateTheme,
    onUpdateWorkbenchPreferences,
    onSaveProviderKey,
    onDeleteProviderKey,
    onSaveNetworkConfiguration,
    onTestEndpoint,
    onRetryProviderState,
    onRetryNetworkState,
    onSaveOidc,
    onSaveApprise,
    onClearCache,
  }: Props = $props();

  let activeSettingsSection:
    | "appearance"
    | "navigation"
    | "preferences"
    | "custom_fields"
    | "providers"
    | "connectors"
    | "tokens"
    | "oidc"
    | "notifications"
    | "importers"
    | "advanced" = $state("appearance");

  // Local state for the custom field / custom media type creators
  let newFieldName = $state("");
  let newFieldKey = $state("");
  let newFieldType = $state<CustomFieldDefinition["valueType"]>("string");
  let newFieldTarget = $state<MediaKind | "all">("all");
  let newFieldOptions = $state("");

  let newTypeName = $state("");
  let newTypeSingular = $state("");
  let newTypePlural = $state("");
  let newTypeIcon = $state("");
  let newTypeProgress =
    $state<CustomMediaTypeDefinition["progressTrackingType"]>("none");

  const MEDIA_KIND_OPTIONS: Array<MediaKind | "all"> = [
    "all",
    "movie",
    "show",
    "anime",
    "manga",
    "book",
    "comic",
    "game",
    "music",
    "podcast",
    "custom",
  ];

  function handleAddCustomField(e: Event): void {
    e.preventDefault();
    const name = newFieldName.trim();
    const key = newFieldKey.trim();
    if (!name || !key || !workbenchPreferences) return;
    const field: CustomFieldDefinition = {
      key,
      label: name,
      targetType: newFieldTarget,
      valueType: newFieldType,
      isFilterable: false,
      options:
        newFieldType === "select"
          ? newFieldOptions
              .split(",")
              .map((o) => o.trim())
              .filter((o) => o.length > 0)
          : undefined,
    };
    onUpdateWorkbenchPreferences?.({
      customFields: [...workbenchPreferences.customFields, field],
    });
    newFieldName = "";
    newFieldKey = "";
    newFieldOptions = "";
    newFieldType = "string";
    newFieldTarget = "all";
  }

  function handleDeleteCustomField(key: string): void {
    if (!workbenchPreferences) return;
    onUpdateWorkbenchPreferences?.({
      customFields: workbenchPreferences.customFields.filter(
        (f) => f.key !== key,
      ),
    });
  }

  function handleAddCustomMediaType(e: Event): void {
    e.preventDefault();
    const name = newTypeName.trim();
    const singular = newTypeSingular.trim();
    const plural = newTypePlural.trim();
    if (!name || !singular || !plural || !workbenchPreferences) return;
    const mediaType: CustomMediaTypeDefinition = {
      id: crypto.randomUUID(),
      name,
      singular,
      plural,
      icon: newTypeIcon.trim() || "🎬",
      progressTrackingType: newTypeProgress,
    };
    onUpdateWorkbenchPreferences?.({
      customMediaTypes: [...workbenchPreferences.customMediaTypes, mediaType],
    });
    newTypeName = "";
    newTypeSingular = "";
    newTypePlural = "";
    newTypeIcon = "";
    newTypeProgress = "none";
  }

  function handleDeleteCustomMediaType(id: string): void {
    if (!workbenchPreferences) return;
    onUpdateWorkbenchPreferences?.({
      customMediaTypes: workbenchPreferences.customMediaTypes.filter(
        (t) => t.id !== id,
      ),
    });
  }

  // Local state for token generator
  let newTokenName = $state("");
  let selectedScopes: string[] = $state(["chronicle:write", "metadata:read"]);
  let newAppriseUrl = $state("");

  // Local state for keys
  let editingKeyMap: Record<string, string> = $state({});
  let providerBusy = $state<string | undefined>();
  let providerNotice = $state("");
  let providerProblem = $state("");
  let oidcDraft = $state({
    enabled: false,
    issuerUrl: "",
    clientId: "",
    clientSecret: "",
    redirectUri: "",
    autoProvisionUsers: false,
  });
  let appriseDraft = $state({
    enabled: false,
    urls: [] as string[],
    notifyOnReviewRequired: false,
    notifyOnSyncError: false,
    notifyOnMilestone: false,
  });

  $effect(() => {
    if (initialSection) activeSettingsSection = initialSection;
  });

  $effect(() => {
    oidcDraft = { ...oidcConfig };
  });

  $effect(() => {
    appriseDraft = { ...appriseConfig, urls: [...appriseConfig.urls] };
  });

  function handleAddAppriseUrl(e: Event): void {
    e.preventDefault();
    if (newAppriseUrl.trim().length > 0) {
      const updated = {
        ...appriseDraft,
        urls: [...appriseDraft.urls, newAppriseUrl.trim()],
      };
      appriseDraft = updated;
      newAppriseUrl = "";
    }
  }

  function handleToggleNavVisible(id: string): void {
    if (!workbenchPreferences) return;
    const updated = workbenchPreferences.navItems.map((item) =>
      item.id === id ? { ...item, visible: !item.visible } : item,
    );
    onUpdateWorkbenchPreferences?.({ navItems: updated });
  }

  function handleToggleNavPin(id: string): void {
    if (!workbenchPreferences) return;
    const updated = workbenchPreferences.navItems.map((item) =>
      item.id === id ? { ...item, pinned: !item.pinned } : item,
    );
    onUpdateWorkbenchPreferences?.({ navItems: updated });
  }

  function handleMoveNav(id: string, direction: -1 | 1): void {
    if (!workbenchPreferences) return;
    const items = [...workbenchPreferences.navItems].sort(
      (a, b) => a.order - b.order,
    );
    const idx = items.findIndex((i) => i.id === id);
    if (idx < 0) return;
    const targetIdx = idx + direction;
    if (targetIdx < 0 || targetIdx >= items.length) return;
    const temp = items[idx];
    items[idx] = items[targetIdx];
    items[targetIdx] = temp;
    const reordered = items.map((it, i) => ({ ...it, order: i }));
    onUpdateWorkbenchPreferences?.({ navItems: reordered });
  }

  function handleToggleContextVisible(id: string): void {
    if (!workbenchPreferences) return;
    const updated = workbenchPreferences.contextMenuItems.map((item) =>
      item.id === id ? { ...item, visible: !item.visible } : item,
    );
    onUpdateWorkbenchPreferences?.({ contextMenuItems: updated });
  }

  function handleMoveContext(id: string, direction: -1 | 1): void {
    if (!workbenchPreferences) return;
    const items = [...workbenchPreferences.contextMenuItems].sort(
      (a, b) => a.order - b.order,
    );
    const idx = items.findIndex((i) => i.id === id);
    if (idx < 0) return;
    const targetIdx = idx + direction;
    if (targetIdx < 0 || targetIdx >= items.length) return;
    const temp = items[idx];
    items[idx] = items[targetIdx];
    items[targetIdx] = temp;
    const reordered = items.map((it, i) => ({ ...it, order: i }));
    onUpdateWorkbenchPreferences?.({ contextMenuItems: reordered });
  }

  function handleResetNavPreferences(): void {
    const defaults = createDefaultWorkbenchPreferences();
    onUpdateWorkbenchPreferences?.({
      sidebarCollapsed: defaults.sidebarCollapsed,
      sidebarHidden: defaults.sidebarHidden,
      navItems: defaults.navItems,
      contextMenuItems: defaults.contextMenuItems,
    });
  }

  async function saveProviderKey(provider: string): Promise<void> {
    const credential = editingKeyMap[provider]?.trim() ?? "";
    if (!credential || !onSaveProviderKey || providerBusy) return;
    editingKeyMap[provider] = "";
    providerBusy = provider;
    providerNotice = "";
    providerProblem = "";
    try {
      await onSaveProviderKey(provider, credential);
      providerNotice = "Credential saved securely for this Fasti node.";
    } catch (error) {
      providerProblem = hostProblemText(
        error,
        "The trusted desktop host rejected this credential request.",
      );
    } finally {
      providerBusy = undefined;
    }
  }

  async function deleteProviderKey(provider: string): Promise<void> {
    if (!onDeleteProviderKey || providerBusy) return;
    providerBusy = provider;
    providerNotice = "";
    providerProblem = "";
    try {
      await onDeleteProviderKey(provider);
      editingKeyMap[provider] = "";
      providerNotice = "Credential removed from the platform credential store.";
    } catch (error) {
      providerProblem = hostProblemText(
        error,
        "The trusted desktop host rejected this credential request.",
      );
    } finally {
      providerBusy = undefined;
    }
  }

  function confirmProviderDelete(provider: string): void {
    if (window.confirm("Remove this provider key from the credential store?")) {
      void deleteProviderKey(provider);
    }
  }

  /** Builds a diagnostics bundle from whatever's already loaded client-side
   * and triggers a browser download. Secrets (client secrets, token
   * secrets, Apprise webhook URLs which often embed tokens) are
   * deliberately excluded rather than redacted-in-place, so there's no risk
   * of a redaction bug leaking one. */
  function handleDownloadLogs(): void {
    const bundle = {
      generatedAt: new Date().toISOString(),
      theme: themeSettings,
      workbenchPreferences: workbenchPreferences
        ? {
            sidebarCollapsed: workbenchPreferences.sidebarCollapsed,
            sidebarHidden: workbenchPreferences.sidebarHidden,
            providerRegion: workbenchPreferences.providerRegion,
            metadataLanguage: workbenchPreferences.metadataLanguage,
            tvProvider: workbenchPreferences.tvProvider,
            animeProvider: workbenchPreferences.animeProvider,
          }
        : undefined,
      providerKeys: (providerKeys ?? []).map((p) => ({
        provider: p.provider,
        configured: p.configured,
        source: p.source,
      })),
      oidcEnabled: oidcConfig?.enabled ?? false,
      appriseEnabled: appriseConfig?.enabled ?? false,
      appriseUrlCount: appriseConfig?.urls.length ?? 0,
      tokenCount: tokens.length,
      customFieldCount: workbenchPreferences?.customFields.length ?? 0,
      customMediaTypeCount: workbenchPreferences?.customMediaTypes.length ?? 0,
    };
    const blob = new Blob([JSON.stringify(bundle, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `fasti-diagnostics-${Date.now()}.json`;
    link.click();
    URL.revokeObjectURL(url);
  }
</script>

<div class="settings-container">
  <header class="settings-header">
    <div>
      <h1 class="view-title">Settings & Studio</h1>
      <p class="view-subtitle">
        Configure available metadata providers, network access, appearance, and
        navigation. Unavailable sections say so.
      </p>
    </div>
  </header>

  <div class="settings-layout">
    <!-- Left Settings Navigation -->
    <nav class="settings-nav" aria-label="Settings subnavigation">
      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "appearance"}
        aria-pressed={activeSettingsSection === "appearance"}
        onclick={() => (activeSettingsSection = "appearance")}
      >
        <IconPalette size={18} /> Appearance & Theme
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "navigation"}
        aria-pressed={activeSettingsSection === "navigation"}
        onclick={() => (activeSettingsSection = "navigation")}
      >
        <IconLayoutSidebar size={18} /> Navigation & Menus
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "preferences"}
        aria-pressed={activeSettingsSection === "preferences"}
        onclick={() => (activeSettingsSection = "preferences")}
      >
        <IconWorld size={18} /> Preferences & Metadata
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "custom_fields"}
        aria-pressed={activeSettingsSection === "custom_fields"}
        onclick={() => (activeSettingsSection = "custom_fields")}
      >
        <IconTags size={18} /> Custom Types & Fields
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "providers"}
        aria-pressed={activeSettingsSection === "providers"}
        onclick={() => (activeSettingsSection = "providers")}
      >
        <IconKey size={18} /> Metadata Providers & Keys
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "advanced"}
        aria-pressed={activeSettingsSection === "advanced"}
        onclick={() => (activeSettingsSection = "advanced")}
      >
        <IconSettings size={18} /> Advanced Network Access
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "connectors"}
        aria-pressed={activeSettingsSection === "connectors"}
        onclick={() => (activeSettingsSection = "connectors")}
      >
        <IconDeviceTv size={18} /> Nuvio & Media Connectors
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "tokens"}
        aria-pressed={activeSettingsSection === "tokens"}
        onclick={() => (activeSettingsSection = "tokens")}
      >
        <IconCode size={18} /> Personal Access Tokens (PAT)
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "oidc"}
        aria-pressed={activeSettingsSection === "oidc"}
        onclick={() => (activeSettingsSection = "oidc")}
      >
        <IconUserCheck size={18} /> Single Sign-On (OIDC)
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "notifications"}
        aria-pressed={activeSettingsSection === "notifications"}
        onclick={() => (activeSettingsSection = "notifications")}
      >
        <IconBell size={18} /> Notifications & Apprise
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "importers"}
        aria-pressed={activeSettingsSection === "importers"}
        onclick={() => (activeSettingsSection = "importers")}
      >
        <IconDatabaseImport size={18} /> Lossless Importers & Backups
      </button>
    </nav>

    <!-- Right Settings Content Panel -->
    <div class="settings-content-card">
      <!-- 1. Appearance & Theme Editor (Tabler Customizer) -->
      {#if activeSettingsSection === "appearance"}
        <section class="section-pane">
          <h2 class="pane-title">Appearance & Theme Editor</h2>
          <p class="pane-desc">
            Tabler Theme Engine: configure live color modes, accent color
            schemes, font families, base grays, and corner radii.
          </p>

          <!-- Color Mode -->
          <div class="setting-group">
            <h3 class="group-title">Color Mode</h3>
            <div class="options-grid-3">
              <button
                type="button"
                class="theme-card-btn"
                class:selected={themeSettings.mode === "light"}
                onclick={() => onUpdateTheme?.({ mode: "light" })}
              >
                <div class="preview-swatch light-swatch"></div>
                <div class="swatch-label">
                  <strong>Light Mode</strong>
                  <span>Clean archival paper theme</span>
                </div>
              </button>

              <button
                type="button"
                class="theme-card-btn"
                class:selected={themeSettings.mode === "dark"}
                onclick={() => onUpdateTheme?.({ mode: "dark" })}
              >
                <div class="preview-swatch dark-swatch"></div>
                <div class="swatch-label">
                  <strong>Dark Mode</strong>
                  <span>Tabler charcoal slate #1e293b</span>
                </div>
              </button>

              <button
                type="button"
                class="theme-card-btn"
                class:selected={themeSettings.mode === "night"}
                onclick={() => onUpdateTheme?.({ mode: "night" })}
              >
                <div class="preview-swatch night-swatch"></div>
                <div class="swatch-label">
                  <strong>Night Mode (OLED)</strong>
                  <span>True black #000000 high-contrast</span>
                </div>
              </button>
            </div>
          </div>

          <!-- Color Scheme (Accent Palette) -->
          <div class="setting-group">
            <h3 class="group-title">Color Scheme (Primary Accent)</h3>
            <div class="accents-row">
              {#each [{ id: "#066fd1", name: "Tabler Blue", hex: "#066fd1" }, { id: "#4263eb", name: "Indigo", hex: "#4263eb" }, { id: "#ae3ec9", name: "Purple", hex: "#ae3ec9" }, { id: "#d6336c", name: "Pink", hex: "#d6336c" }, { id: "#d63939", name: "Red", hex: "#d63939" }, { id: "#f76707", name: "Orange", hex: "#f76707" }, { id: "#f59f00", name: "Yellow", hex: "#f59f00" }, { id: "#74b816", name: "Lime", hex: "#74b816" }, { id: "#2fb344", name: "Green", hex: "#2fb344" }, { id: "#0ca678", name: "Teal", hex: "#0ca678" }, { id: "#17a2b8", name: "Cyan", hex: "#17a2b8" }, { id: "#8B2E2A", name: "Fasti Oxblood", hex: "#8B2E2A" }, { id: "#D4AF37", name: "Horological Gold", hex: "#D4AF37" }] as acc}
                <button
                  type="button"
                  class="accent-btn"
                  class:selected={themeSettings.accentColor === acc.id ||
                    themeSettings.accentColor === acc.hex}
                  style="--accent-hex: {acc.hex}"
                  onclick={() => onUpdateTheme?.({ accentColor: acc.hex })}
                  title={acc.name}
                >
                  <span class="accent-circle"></span>
                  <span class="accent-name">{acc.name}</span>
                </button>
              {/each}
            </div>
          </div>

          <!-- Font Family -->
          <div class="setting-group">
            <h3 class="group-title">Font Family</h3>
            <div class="options-grid-3">
              {#each [{ id: "sans-serif", title: "Sans-serif", desc: "Atkinson Hyperlegible / Clean modern" }, { id: "serif", title: "Serif", desc: "Newsreader / Archival editorial" }, { id: "monospace", title: "Monospace", desc: "IBM Plex Mono / Precision terminal" }] as f}
                <button
                  type="button"
                  class="density-btn"
                  class:selected={(themeSettings.fontFamily ?? "sans-serif") ===
                    f.id}
                  onclick={() => onUpdateTheme?.({ fontFamily: f.id as any })}
                >
                  <strong>{f.title}</strong>
                  <span>{f.desc}</span>
                </button>
              {/each}
            </div>
          </div>

          <!-- Theme Base (Gray Shade) -->
          <div class="setting-group">
            <h3 class="group-title">Theme Base Shade</h3>
            <div class="accents-row">
              {#each [{ id: "slate", name: "Slate" }, { id: "gray", name: "Gray" }, { id: "zinc", name: "Zinc" }, { id: "neutral", name: "Neutral" }, { id: "stone", name: "Stone" }] as b}
                <button
                  type="button"
                  class="density-btn text-center"
                  class:selected={(themeSettings.themeBase ?? "slate") === b.id}
                  onclick={() => onUpdateTheme?.({ themeBase: b.id as any })}
                >
                  <strong>{b.name}</strong>
                </button>
              {/each}
            </div>
          </div>

          <!-- Corner Radius -->
          <div class="setting-group">
            <h3 class="group-title">Corner Radius Factor</h3>
            <div class="accents-row">
              {#each [{ id: 0, name: "0 (Square)" }, { id: 0.5, name: "0.5 (2px)" }, { id: 1, name: "1 (4px)" }, { id: 1.5, name: "1.5 (6px)" }, { id: 2, name: "2 (8px)" }] as rad}
                <button
                  type="button"
                  class="density-btn text-center"
                  class:selected={(themeSettings.cornerRadius ?? 1) === rad.id}
                  onclick={() => onUpdateTheme?.({ cornerRadius: rad.id })}
                >
                  <strong>{rad.name}</strong>
                </button>
              {/each}
            </div>
          </div>

          <!-- Layout Density -->
          <div class="setting-group">
            <h3 class="group-title">Layout Density</h3>
            <div class="options-grid-3">
              {#each [{ id: "compact", title: "Compact", desc: "Tighter rows, maximum information density" }, { id: "normal", title: "Normal", desc: "Balanced spacing, standard margins" }, { id: "spacious", title: "Spacious", desc: "Relaxed editorial breathing room" }] as d}
                <button
                  type="button"
                  class="density-btn"
                  class:selected={themeSettings.density === d.id}
                  onclick={() => onUpdateTheme?.({ density: d.id as any })}
                >
                  <strong>{d.title}</strong>
                  <span>{d.desc}</span>
                </button>
              {/each}
            </div>
          </div>
        </section>

        <!-- 2. Navigation & Menus Customizer -->
      {:else if activeSettingsSection === "navigation"}
        <section class="section-pane">
          <div class="d-flex align-items-center justify-content-between mb-3">
            <div>
              <h2 class="pane-title">Navigation & Menus Customizer</h2>
              <p class="pane-desc">
                Configure sidebar visibility, pinned shortcuts, reorder
                sections, and customize right-click context menu actions.
              </p>
            </div>
            <button
              type="button"
              class="btn btn-outline-secondary btn-sm d-flex align-items-center gap-1"
              onclick={handleResetNavPreferences}
            >
              <IconRotate2 size={16} /> Reset to Defaults
            </button>
          </div>

          <!-- Sidebar Display Mode -->
          <div class="setting-group mb-4">
            <h3 class="group-title">Sidebar Display Mode</h3>
            <div class="options-grid-2">
              <button
                type="button"
                class="density-btn"
                class:selected={!workbenchPreferences.sidebarCollapsed &&
                  !workbenchPreferences.sidebarHidden}
                onclick={() =>
                  onUpdateWorkbenchPreferences?.({
                    sidebarCollapsed: false,
                    sidebarHidden: false,
                  })}
              >
                <strong>Expanded (240 px)</strong>
                <span
                  >Full navigation rail with titles, counts, and category labels</span
                >
              </button>
              <button
                type="button"
                class="density-btn"
                class:selected={workbenchPreferences.sidebarCollapsed &&
                  !workbenchPreferences.sidebarHidden}
                onclick={() =>
                  onUpdateWorkbenchPreferences?.({
                    sidebarCollapsed: true,
                    sidebarHidden: false,
                  })}
              >
                <strong>Collapsed (64 px)</strong>
                <span>Compact icon-only vertical rail with tooltips</span>
              </button>
            </div>
          </div>

          <!-- Sidebar Navigation Items -->
          <div class="setting-group mb-4">
            <div class="d-flex align-items-center justify-content-between mb-2">
              <h3 class="group-title mb-0">Sidebar Navigation Items</h3>
              <span class="text-muted small"
                >Show, hide, pin, and reorder sidebar links</span
              >
            </div>

            <div class="card border">
              <div class="table-responsive">
                <table class="table table-vcenter card-table">
                  <thead>
                    <tr>
                      <th style="width: 80px;">Order</th>
                      <th style="width: 70px;">Pin</th>
                      <th>Navigation Item</th>
                      <th>Category</th>
                      <th class="text-end" style="width: 120px;">Visibility</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each [...(workbenchPreferences.navItems || [])].sort((a, b) => a.order - b.order) as item, idx (item.id)}
                      <tr>
                        <td>
                          <div class="btn-group btn-group-sm">
                            <button
                              type="button"
                              class="btn btn-sm btn-ghost-secondary p-1"
                              disabled={idx === 0}
                              onclick={() => handleMoveNav(item.id, -1)}
                              title="Move Up"
                              aria-label="Move Up"
                            >
                              <IconArrowUp size={14} />
                            </button>
                            <button
                              type="button"
                              class="btn btn-sm btn-ghost-secondary p-1"
                              disabled={idx ===
                                workbenchPreferences.navItems.length - 1}
                              onclick={() => handleMoveNav(item.id, 1)}
                              title="Move Down"
                              aria-label="Move Down"
                            >
                              <IconArrowDown size={14} />
                            </button>
                          </div>
                        </td>
                        <td>
                          <button
                            type="button"
                            class="btn btn-sm p-1 border-0"
                            class:text-primary={item.pinned}
                            class:text-muted={!item.pinned}
                            onclick={() => handleToggleNavPin(item.id)}
                            title={item.pinned
                              ? "Pinned to top shortcuts"
                              : "Click to pin"}
                            aria-label={item.pinned ? "Unpin item" : "Pin item"}
                          >
                            <IconPin size={16} />
                          </button>
                        </td>
                        <td>
                          <span
                            class="fw-semibold"
                            class:text-muted={!item.visible}>{item.label}</span
                          >
                        </td>
                        <td>
                          <span
                            class="badge bg-secondary-lt text-uppercase font-monospace"
                            >{item.category}</span
                          >
                        </td>
                        <td class="text-end">
                          <button
                            type="button"
                            class="btn btn-sm"
                            class:btn-outline-primary={item.visible}
                            class:btn-ghost-secondary={!item.visible}
                            onclick={() => handleToggleNavVisible(item.id)}
                          >
                            {#if item.visible}
                              <IconEye size={14} class="me-1" /> Visible
                            {:else}
                              <IconEyeOff size={14} class="me-1" /> Hidden
                            {/if}
                          </button>
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            </div>
          </div>

          <!-- Context Menu (Right Click) Actions -->
          <div class="setting-group">
            <div class="d-flex align-items-center justify-content-between mb-2">
              <h3 class="group-title mb-0">Right-Click Context Menu Actions</h3>
              <span class="text-muted small"
                >Show, hide, and reorder card actions</span
              >
            </div>

            <div class="card border">
              <div class="table-responsive">
                <table class="table table-vcenter card-table">
                  <thead>
                    <tr>
                      <th style="width: 80px;">Order</th>
                      <th>Action Name</th>
                      <th class="text-end" style="width: 120px;">Visibility</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each [...(workbenchPreferences.contextMenuItems || [])].sort((a, b) => a.order - b.order) as item, idx (item.id)}
                      <tr>
                        <td>
                          <div class="btn-group btn-group-sm">
                            <button
                              type="button"
                              class="btn btn-sm btn-ghost-secondary p-1"
                              disabled={idx === 0}
                              onclick={() => handleMoveContext(item.id, -1)}
                              title="Move Up"
                              aria-label="Move Up"
                            >
                              <IconArrowUp size={14} />
                            </button>
                            <button
                              type="button"
                              class="btn btn-sm btn-ghost-secondary p-1"
                              disabled={idx ===
                                workbenchPreferences.contextMenuItems.length -
                                  1}
                              onclick={() => handleMoveContext(item.id, 1)}
                              title="Move Down"
                              aria-label="Move Down"
                            >
                              <IconArrowDown size={14} />
                            </button>
                          </div>
                        </td>
                        <td>
                          <span
                            class="fw-semibold"
                            class:text-muted={!item.visible}>{item.label}</span
                          >
                        </td>
                        <td class="text-end">
                          <button
                            type="button"
                            class="btn btn-sm"
                            class:btn-outline-primary={item.visible}
                            class:btn-ghost-secondary={!item.visible}
                            onclick={() => handleToggleContextVisible(item.id)}
                          >
                            {#if item.visible}
                              <IconEye size={14} class="me-1" /> Visible
                            {:else}
                              <IconEyeOff size={14} class="me-1" /> Hidden
                            {/if}
                          </button>
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </section>

        <!-- 3. Preferences & Metadata -->
      {:else if activeSettingsSection === "preferences"}
        <section class="section-pane">
          <h2 class="pane-title">Preferences & Metadata</h2>
          <p class="pane-desc">
            Defaults used when searching providers, projecting metadata, and
            displaying progress across the library.
          </p>

          <div class="prefs-grid">
            <div class="form-field">
              <label for="pref-provider-region">Provider Region</label>
              <select
                id="pref-provider-region"
                class="form-input"
                value={workbenchPreferences.providerRegion}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    providerRegion: e.currentTarget.value,
                  })}
              >
                {#each [{ id: "US", name: "United States" }, { id: "GB", name: "United Kingdom" }, { id: "CA", name: "Canada" }, { id: "AU", name: "Australia" }, { id: "DE", name: "Germany" }, { id: "FR", name: "France" }, { id: "JP", name: "Japan" }, { id: "IE", name: "Ireland" }] as region}
                  <option value={region.id}>{region.name}</option>
                {/each}
              </select>
            </div>

            <div class="form-field">
              <label for="pref-metadata-language">Metadata Language</label>
              <select
                id="pref-metadata-language"
                class="form-input"
                value={workbenchPreferences.metadataLanguage}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    metadataLanguage: e.currentTarget.value,
                  })}
              >
                {#each [{ id: "en-US", name: "English (US)" }, { id: "en-GB", name: "English (UK)" }, { id: "ja-JP", name: "Japanese" }, { id: "de-DE", name: "German" }, { id: "fr-FR", name: "French" }, { id: "es-ES", name: "Spanish" }] as lang}
                  <option value={lang.id}>{lang.name}</option>
                {/each}
              </select>
            </div>

            <div class="form-field">
              <label for="pref-tv-provider">TV Provider</label>
              <select
                id="pref-tv-provider"
                class="form-input"
                value={workbenchPreferences.tvProvider}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    tvProvider: e.currentTarget.value as "tmdb" | "tvdb_v4",
                  })}
              >
                <option value="tmdb">TMDB</option>
                <option value="tvdb_v4">TheTVDB (v4)</option>
              </select>
            </div>

            <div class="form-field">
              <label for="pref-anime-provider">Anime Provider</label>
              <select
                id="pref-anime-provider"
                class="form-input"
                value={workbenchPreferences.animeProvider}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    animeProvider: e.currentTarget.value as
                      "mal" | "anilist" | "kitsu",
                  })}
              >
                <option value="mal">MyAnimeList</option>
                <option value="anilist">AniList</option>
                <option value="kitsu">Kitsu</option>
              </select>
            </div>

            <div class="form-field">
              <label for="pref-title-language">Title Language Preference</label>
              <select
                id="pref-title-language"
                class="form-input"
                value={workbenchPreferences.titleLanguage}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    titleLanguage: e.currentTarget.value as
                      "romaji" | "english" | "native",
                  })}
              >
                <option value="romaji">Romaji</option>
                <option value="english">English</option>
                <option value="native">Native</option>
              </select>
            </div>

            <div class="form-field">
              <label for="pref-hide-completed">Hide Completed</label>
              <select
                id="pref-hide-completed"
                class="form-input"
                value={workbenchPreferences.hideCompleted}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    hideCompleted: e.currentTarget.value as
                      "disabled" | "home_only" | "everywhere",
                  })}
              >
                <option value="disabled">Disabled</option>
                <option value="home_only">Home Only</option>
                <option value="everywhere">Everywhere</option>
              </select>
            </div>

            <div class="form-field">
              <label for="pref-game-logging">Game Logging</label>
              <select
                id="pref-game-logging"
                class="form-input"
                value={workbenchPreferences.gameLogging}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    gameLogging: e.currentTarget.value as
                      "repeats" | "sessions",
                  })}
              >
                <option value="sessions">Sessions</option>
                <option value="repeats">Repeats</option>
              </select>
            </div>

            <div class="form-field">
              <label for="pref-progress-format">Progress Format</label>
              <select
                id="pref-progress-format"
                class="form-input"
                value={workbenchPreferences.progressFormat}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    progressFormat: e.currentTarget.value as
                      "percentage" | "time_remaining" | "episodes",
                  })}
              >
                <option value="percentage">Percentage</option>
                <option value="time_remaining">Time Remaining</option>
                <option value="episodes">Episode Count</option>
              </select>
            </div>

            <div class="form-field">
              <label for="pref-session-duration">Session Duration</label>
              <select
                id="pref-session-duration"
                class="form-input"
                value={workbenchPreferences.sessionDuration}
                onchange={(e) =>
                  onUpdateWorkbenchPreferences?.({
                    sessionDuration: Number(e.currentTarget.value),
                  })}
              >
                <option value={15}>15 minutes</option>
                <option value={30}>30 minutes</option>
                <option value={60}>1 hour</option>
                <option value={240}>4 hours</option>
                <option value={480}>8 hours</option>
              </select>
            </div>
          </div>

          <label class="chk-label mt-3">
            <input
              type="checkbox"
              checked={workbenchPreferences.hideZeroRatings}
              onchange={(e) =>
                onUpdateWorkbenchPreferences?.({
                  hideZeroRatings: e.currentTarget.checked,
                })}
            />
            <span>Hide Zero Ratings</span>
          </label>
        </section>

        <!-- 4. Custom Types & Fields -->
      {:else if activeSettingsSection === "custom_fields"}
        <section class="section-pane">
          <h2 class="pane-title">Custom Types & Fields</h2>
          <p class="pane-desc">
            Register custom metadata fields and custom media types for this
            Fasti node. Both are stored in your local workbench preferences.
          </p>

          <div class="setting-group">
            <h3 class="group-title">Custom Metadata Fields</h3>
            <form onsubmit={handleAddCustomField} class="custom-field-form">
              <div class="prefs-grid">
                <div class="form-field">
                  <label for="cf-name">Name</label>
                  <input
                    id="cf-name"
                    type="text"
                    class="form-input"
                    bind:value={newFieldName}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cf-key">Key</label>
                  <input
                    id="cf-key"
                    type="text"
                    class="form-input mono"
                    placeholder="e.g. rewatch_count"
                    bind:value={newFieldKey}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cf-type">Type</label>
                  <select
                    id="cf-type"
                    class="form-input"
                    bind:value={newFieldType}
                  >
                    <option value="string">Text</option>
                    <option value="number">Number</option>
                    <option value="boolean">Boolean</option>
                    <option value="date">Date</option>
                    <option value="url">URL</option>
                    <option value="identifier">Identifier</option>
                    <option value="select">Select</option>
                  </select>
                </div>
                <div class="form-field">
                  <label for="cf-target">Target Media Kind</label>
                  <select
                    id="cf-target"
                    class="form-input"
                    bind:value={newFieldTarget}
                  >
                    {#each MEDIA_KIND_OPTIONS as kind}
                      <option value={kind}>{kind}</option>
                    {/each}
                  </select>
                </div>
                {#if newFieldType === "select"}
                  <div class="form-field">
                    <label for="cf-options">Options (comma-separated)</label>
                    <input
                      id="cf-options"
                      type="text"
                      class="form-input"
                      placeholder="e.g. Physical, Digital, Both"
                      bind:value={newFieldOptions}
                    />
                  </div>
                {/if}
              </div>
              <button type="submit" class="btn-secondary mt-2">
                <IconPlus size={16} /> Add Custom Field
              </button>
            </form>

            {#if workbenchPreferences.customFields.length > 0}
              <ul class="custom-entry-list">
                {#each workbenchPreferences.customFields as field (field.key)}
                  <li class="custom-entry-row">
                    <div>
                      <strong>{field.label}</strong>
                      <span class="entry-meta">
                        <code>{field.key}</code> · {field.valueType} · {field.targetType}
                      </span>
                    </div>
                    <button
                      type="button"
                      class="delete-entry-btn"
                      onclick={() => handleDeleteCustomField(field.key)}
                      aria-label="Delete custom field {field.label}"
                    >
                      <IconTrash size={14} />
                    </button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="wb-help">No custom metadata fields registered yet.</p>
            {/if}
          </div>

          <div class="setting-group">
            <h3 class="group-title">Custom Media Types</h3>
            <form onsubmit={handleAddCustomMediaType} class="custom-field-form">
              <div class="prefs-grid">
                <div class="form-field">
                  <label for="cmt-name">Name</label>
                  <input
                    id="cmt-name"
                    type="text"
                    class="form-input"
                    bind:value={newTypeName}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cmt-singular">Singular</label>
                  <input
                    id="cmt-singular"
                    type="text"
                    class="form-input"
                    placeholder="e.g. Board Game"
                    bind:value={newTypeSingular}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cmt-plural">Plural</label>
                  <input
                    id="cmt-plural"
                    type="text"
                    class="form-input"
                    placeholder="e.g. Board Games"
                    bind:value={newTypePlural}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cmt-icon">Icon</label>
                  <input
                    id="cmt-icon"
                    type="text"
                    class="form-input"
                    placeholder="🎲"
                    bind:value={newTypeIcon}
                  />
                </div>
                <div class="form-field">
                  <label for="cmt-progress">Progress Tracking</label>
                  <select
                    id="cmt-progress"
                    class="form-input"
                    bind:value={newTypeProgress}
                  >
                    <option value="none">None</option>
                    <option value="episodes">Episodes</option>
                    <option value="percentage">Percentage</option>
                    <option value="pages">Pages</option>
                    <option value="sessions">Sessions</option>
                  </select>
                </div>
              </div>
              <button type="submit" class="btn-secondary mt-2">
                <IconPlus size={16} /> Add Custom Media Type
              </button>
            </form>

            {#if workbenchPreferences.customMediaTypes.length > 0}
              <ul class="custom-entry-list">
                {#each workbenchPreferences.customMediaTypes as mediaType (mediaType.id)}
                  <li class="custom-entry-row">
                    <div>
                      <span class="entry-icon">{mediaType.icon}</span>
                      <strong>{mediaType.name}</strong>
                      <span class="entry-meta">
                        {mediaType.singular} / {mediaType.plural} · {mediaType.progressTrackingType}
                      </span>
                    </div>
                    <button
                      type="button"
                      class="delete-entry-btn"
                      onclick={() => handleDeleteCustomMediaType(mediaType.id)}
                      aria-label="Delete custom media type {mediaType.name}"
                    >
                      <IconTrash size={14} />
                    </button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="wb-help">No custom media types registered yet.</p>
            {/if}
          </div>
        </section>

        <!-- 5. Metadata Providers & Keys -->
      {:else if activeSettingsSection === "providers"}
        <section class="section-pane">
          <h2 id="provider-settings-title" class="pane-title" tabindex="-1">
            Metadata Providers & API Credentials
          </h2>
          <p class="pane-desc">
            Add a provider credential to enable real metadata search. Fasti
            never returns a stored secret to this interface.
          </p>

          <div class="providers-list">
            {#if providerLoading}
              <p class="unavailable-note" role="status">
                Loading provider credential status…
              </p>
            {:else if providerKeys === undefined && providerLoadProblem}
              <p class="provider-problem" role="alert">
                {providerLoadProblem}
              </p>
              {#if onRetryProviderState}
                <button
                  id="provider-retry"
                  type="button"
                  onclick={onRetryProviderState}
                >
                  Retry host connection
                </button>
              {/if}
            {/if}
            {#each providerKeys ?? [] as prov}
              <div class="provider-key-card">
                <div class="provider-key-header">
                  <div>
                    <h3 class="provider-title">{prov.label}</h3>
                    <a
                      href={prov.docs_url}
                      target="_blank"
                      rel="noopener"
                      class="docs-link"
                    >
                      API Documentation <IconExternalLink size={12} />
                    </a>
                  </div>
                  <span
                    class="prov-status-chip"
                    class:configured={prov.configured}
                  >
                    {prov.configured ? "Configured" : "Not configured"}
                  </span>
                </div>

                <p class="wb-help">
                  {#if prov.source === "environment"}
                    Source: environment. This value is read-only and applies to
                    this app process.
                  {:else if prov.source === "credential_store"}
                    Source: platform credential store. This value is shared by
                    all profiles on this Fasti node.
                  {:else}
                    No credential is saved for this Fasti node.
                  {/if}
                  {prov.provider === "tmdb"
                    ? "TMDB enables movie and TV metadata search and refresh. Use an API Read Access Token."
                    : "Google Books enables book metadata search and refresh."}
                </p>
                <form
                  class="key-input-row"
                  onsubmit={(event) => {
                    event.preventDefault();
                    void saveProviderKey(prov.provider);
                  }}
                >
                  <input
                    type="password"
                    placeholder={prov.writable
                      ? prov.provider === "tmdb"
                        ? "Enter an API Read Access Token"
                        : "Enter a new API key"
                      : "Managed by the environment"}
                    value={editingKeyMap[prov.provider] ?? ""}
                    oninput={(e) =>
                      (editingKeyMap[prov.provider] = e.currentTarget.value)}
                    class="api-key-input"
                    aria-label="Provider credential for {prov.label}"
                    autocomplete="off"
                    spellcheck="false"
                    disabled={!prov.writable || providerBusy === prov.provider}
                  />
                  <button
                    type="submit"
                    class="save-key-btn"
                    disabled={!prov.writable ||
                      !editingKeyMap[prov.provider]?.trim() ||
                      !!providerBusy}
                  >
                    {providerBusy === prov.provider
                      ? "Saving…"
                      : "Save credential"}
                  </button>
                  {#if prov.writable && prov.configured}
                    <button
                      type="button"
                      class="remove-key-btn"
                      disabled={!!providerBusy}
                      onclick={() => confirmProviderDelete(prov.provider)}
                    >
                      Remove credential
                    </button>
                  {/if}
                </form>
              </div>
            {/each}
          </div>
          {#if providerNotice}
            <p class="provider-notice" role="status">{providerNotice}</p>
          {/if}
          {#if providerProblem}
            <p class="provider-problem" role="alert">{providerProblem}</p>
          {/if}
          <TmdbAttribution />
        </section>
      {:else if activeSettingsSection === "advanced"}
        {#if onSaveNetworkConfiguration && onTestEndpoint}
          <NetworkSettings
            configuration={networkConfiguration}
            loading={networkLoading}
            loadProblem={networkLoadProblem}
            onSave={onSaveNetworkConfiguration}
            onTest={onTestEndpoint}
            onRetry={onRetryNetworkState}
          />
        {:else}
          <section class="section-pane">
            <h2 class="pane-title">Advanced network access</h2>
            <p class="pane-desc" role="alert">
              The trusted desktop host does not expose network settings.
            </p>
          </section>
        {/if}

        <section class="section-pane">
          <h2 class="pane-title">Cache Management</h2>
          <p class="pane-desc">
            Clear cached data per category, or everything at once.
          </p>
          <div class="cache-cards-grid">
            {#each [{ id: "search", label: "Search Cache" }, { id: "history", label: "History Cache" }, { id: "statistics", label: "Statistics Cache" }, { id: "discover", label: "Discover Cache" }] as cache}
              <div class="cache-card">
                <div class="cache-card-header">
                  <IconDatabase size={18} />
                  <strong>{cache.label}</strong>
                </div>
                <button
                  type="button"
                  class="btn-secondary"
                  disabled={!onClearCache}
                  title={onClearCache
                    ? undefined
                    : "Cache clearing is not available in this build"}
                  onclick={() =>
                    onClearCache?.(
                      cache.id as
                        "search" | "history" | "statistics" | "discover",
                    )}
                >
                  Clear
                </button>
              </div>
            {/each}
          </div>
          <button
            type="button"
            class="btn-save mt-3"
            disabled={!onClearCache}
            title={onClearCache
              ? undefined
              : "Cache clearing is not available in this build"}
            onclick={() => onClearCache?.("all")}
          >
            Clear All Caches
          </button>
        </section>

        <section class="section-pane">
          <h2 class="pane-title">Diagnostics & Support</h2>
          <div class="diagnostics-actions">
            <button
              type="button"
              class="btn-secondary"
              onclick={handleDownloadLogs}
            >
              <IconFileDownload size={16} /> Download Sanitized Logs
            </button>
            <a
              href="https://github.com/Scrobble-dev/Fasti/issues/new"
              target="_blank"
              rel="noopener noreferrer"
              class="btn-secondary"
            >
              <IconBug size={16} /> File a Bug Report
            </a>
          </div>
        </section>

        <!-- 3. Nuvio & Media Server Connectors -->
      {:else if activeSettingsSection === "connectors"}
        <section class="section-pane">
          <h2 class="pane-title">NuvioTV & Media Server Connectors</h2>
          <p class="pane-desc">
            Connector setup is not available in this build. Fasti does not
            publish ingest or webhook endpoints until the matching capability is
            implemented.
          </p>
        </section>

        <!-- 4. Scoped Personal Access Tokens (PAT) -->
      {:else if activeSettingsSection === "tokens"}
        <section class="section-pane">
          <h2 class="pane-title">Personal Access Tokens (PAT)</h2>
          <p class="pane-desc">
            Token creation and revocation are not available in this build. The
            disabled controls show the planned scope model.
          </p>

          <!-- New Token Form -->
          <form
            onsubmit={(event) => event.preventDefault()}
            class="token-form-card"
          >
            <h3 class="form-title">Create New Access Token</h3>
            <div class="form-row">
              <input
                type="text"
                placeholder="Token description (e.g. Living Room Apple TV)..."
                bind:value={newTokenName}
                class="token-name-input"
                aria-label="Token description"
                disabled
                required
              />
              <button type="submit" class="create-token-btn" disabled>
                <IconPlus size={16} /> Generate Token
              </button>
            </div>

            <div class="scopes-checkboxes">
              {#each [{ id: "chronicle:write", label: "chronicle:write (Log watch events)" }, { id: "chronicle:read", label: "chronicle:read (Read activity history)" }, { id: "metadata:read", label: "metadata:read (Search titles)" }, { id: "identity:resolve", label: "identity:resolve (Review Inbox)" }] as sc}
                <label class="scope-chk-label">
                  <input
                    type="checkbox"
                    checked={selectedScopes.includes(sc.id)}
                    disabled
                    onchange={(e) => {
                      if (e.currentTarget.checked)
                        selectedScopes = [...selectedScopes, sc.id];
                      else
                        selectedScopes = selectedScopes.filter(
                          (s) => s !== sc.id,
                        );
                    }}
                  />
                  <span>{sc.label}</span>
                </label>
              {/each}
            </div>
          </form>
          <p class="wb-help" role="status">
            Token issuance is unavailable until an authorized application
            command can return the one-time secret.
          </p>

          <!-- Active Tokens Table -->
          <table class="tokens-table">
            <thead>
              <tr>
                <th scope="col">Name</th>
                <th scope="col">Prefix</th>
                <th scope="col">Scopes</th>
                <th scope="col">Created</th>
                <th scope="col">Actions</th>
              </tr>
            </thead>
            <tbody>
              {#each tokens as tok (tok.id)}
                <tr>
                  <td><strong>{tok.name}</strong></td>
                  <td><code>{tok.tokenPrefix}</code></td>
                  <td>
                    {#each tok.scopes as s}
                      <span class="scope-pill">{s}</span>
                    {/each}
                  </td>
                  <td>{new Date(tok.createdAt).toLocaleDateString()}</td>
                  <td>
                    <button
                      type="button"
                      class="delete-tok-btn"
                      disabled
                      title="Token revocation is not available in this build"
                    >
                      <IconTrash size={14} />
                    </button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>

        <!-- 5. Single Sign-On (OIDC) -->
      {:else if activeSettingsSection === "oidc"}
        <section class="section-pane">
          <h2 class="pane-title">Single Sign-On & OpenID Connect (OIDC)</h2>
          {#if !onSaveOidc}
            <p class="pane-desc">
              OIDC configuration is not available until a host command can
              validate and store it.
            </p>
          {/if}

          {#if onSaveOidc}
            <form
              onsubmit={(e) => {
                e.preventDefault();
                onSaveOidc(oidcDraft);
              }}
              class="oidc-form"
            >
              <div class="form-toggle-row">
                <label class="toggle-label">
                  <input type="checkbox" bind:checked={oidcDraft.enabled} />
                  <strong>Enable Single Sign-On (OIDC)</strong>
                </label>
              </div>

              <div class="form-field">
                <label for="oidc-issuer">OIDC Issuer Discovery URL</label>
                <input
                  id="oidc-issuer"
                  type="url"
                  bind:value={oidcDraft.issuerUrl}
                  class="form-input"
                />
              </div>

              <div class="form-row-2">
                <div class="form-field">
                  <label for="oidc-client-id">Client ID</label>
                  <input
                    id="oidc-client-id"
                    type="text"
                    bind:value={oidcDraft.clientId}
                    class="form-input"
                  />
                </div>
                <div class="form-field">
                  <label for="oidc-client-secret">Client Secret</label>
                  <input
                    id="oidc-client-secret"
                    type="password"
                    bind:value={oidcDraft.clientSecret}
                    class="form-input"
                  />
                </div>
              </div>

              <div class="form-field">
                <label for="oidc-redirect-uri"
                  >Authorized Redirect Callback URI</label
                >
                <input
                  id="oidc-redirect-uri"
                  type="text"
                  readonly
                  value={oidcDraft.redirectUri}
                  class="form-input mono"
                />
              </div>

              <button type="submit" class="btn-save"
                >Save OIDC Configuration</button
              >
            </form>
          {/if}
        </section>

        <!-- 6. Notifications & Apprise -->
      {:else if activeSettingsSection === "notifications"}
        <section class="section-pane">
          <h2 class="pane-title">Notifications & Apprise Webhooks</h2>
          {#if !onSaveApprise}
            <p class="pane-desc">
              Notification configuration is not available until a host command
              can validate and store it.
            </p>
          {/if}

          {#if onSaveApprise}
            <div class="apprise-box">
              <div class="notify-triggers">
                <label class="chk-label">
                  <input
                    type="checkbox"
                    bind:checked={appriseDraft.notifyOnReviewRequired}
                  />
                  <span>Notify when Review Inbox requires attention</span>
                </label>
                <label class="chk-label">
                  <input
                    type="checkbox"
                    bind:checked={appriseDraft.notifyOnSyncError}
                  />
                  <span>Notify on sync and ingest connection errors</span>
                </label>
                <label class="chk-label">
                  <input
                    type="checkbox"
                    bind:checked={appriseDraft.notifyOnMilestone}
                  />
                  <span
                    >Notify when milestone achievements or backups complete</span
                  >
                </label>
              </div>

              <h3 class="sub-heading">Configured Apprise URLs</h3>
              <ul class="apprise-urls-list">
                {#each appriseDraft.urls as u}
                  <li class="apprise-url-item">
                    <code>{u}</code>
                  </li>
                {/each}
              </ul>

              <form onsubmit={handleAddAppriseUrl} class="add-url-form">
                <input
                  type="text"
                  placeholder="discord://webhook_id/webhook_token or telegram://bot_token/chat_id..."
                  bind:value={newAppriseUrl}
                  class="form-input"
                  aria-label="New Apprise URL"
                />
                <button type="submit" class="btn-secondary"
                  >+ Add Service</button
                >
              </form>

              <button
                type="button"
                class="btn-save"
                onclick={() => onSaveApprise?.(appriseDraft)}
              >
                Save notification settings
              </button>

              <div class="test-notify-row">
                <button
                  type="button"
                  class="btn-test"
                  disabled
                  title="Test notifications are not available in this build"
                >
                  Send Test Notification
                </button>
              </div>
            </div>
          {/if}
        </section>

        <!-- 7. Lossless Importers & Backups -->
      {:else if activeSettingsSection === "importers"}
        <section class="section-pane">
          <h2 class="pane-title">Lossless Importers & Migrations</h2>
          <p class="pane-desc">
            Importers are not available in this build. Each disabled option
            shows its planned source formats.
          </p>

          <div class="importers-grid">
            {#each [{ name: "Floppy / Yamtrack", desc: "Import full history, custom fields, and tags from Floppy SQLite/JSON.", ext: ".json, .db" }, { name: "SIMKL Archive", desc: "Import TV, Anime, and Movie tracking records from SIMKL CSV/JSON.", ext: ".csv, .json" }, { name: "Trakt.tv Export", desc: "Import Trakt scrobbles, ratings, and watchlists.", ext: ".csv" }, { name: "MyAnimeList / AniList", desc: "Import anime and manga lists via XML / JSON format.", ext: ".xml, .json" }] as imp}
              <div class="importer-card">
                <h3 class="imp-title">{imp.name}</h3>
                <p class="imp-desc">{imp.desc}</p>
                <div class="imp-action-row">
                  <label class="file-upload-btn">
                    <input
                      type="file"
                      accept={imp.ext}
                      class="hidden-file-input"
                      disabled
                    />
                    <span>Importer unavailable</span>
                  </label>
                </div>
              </div>
            {/each}
          </div>
        </section>
      {/if}
    </div>
  </div>
</div>

<style>
  .settings-container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .settings-header {
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
    padding-bottom: 16px;
  }

  .view-title {
    font-family: var(--fasti-font-display);
    font-size: 2.4rem;
    font-weight: 600;
    margin: 0;
    color: var(--fasti-text-primary);
  }

  .view-subtitle {
    margin: 4px 0 0;
    color: var(--fasti-text-muted);
    font-size: 0.95rem;
  }

  .settings-layout {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 28px;
  }

  .settings-nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .nav-tab-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    background: transparent;
    border: none;
    border-radius: 6px;
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
    cursor: pointer;
    text-align: left;
    min-height: 44px;
  }

  .nav-tab-btn:hover {
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .nav-tab-btn.active {
    background: var(--fasti-surface-paper);
    color: var(--fasti-action-primary);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  }

  .settings-content-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 25%, transparent);
    border-radius: 8px;
    padding: 28px;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.03);
  }

  .pane-title {
    font-family: var(--fasti-font-display);
    font-size: 1.6rem;
    font-weight: 600;
    margin: 0 0 4px;
    color: var(--fasti-text-primary);
  }

  .pane-desc {
    font-size: 0.92rem;
    color: var(--fasti-text-muted);
    margin: 0 0 24px;
  }

  .setting-group {
    margin-bottom: 24px;
  }

  .group-title {
    font-family: var(--fasti-font-mono);
    font-size: 0.8rem;
    text-transform: uppercase;
    color: var(--fasti-text-muted);
    margin: 0 0 12px;
  }

  .options-grid-3 {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 12px;
  }

  .theme-card-btn,
  .density-btn {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 14px;
    background: var(--fasti-surface-archive);
    border: 2px solid transparent;
    border-radius: 6px;
    cursor: pointer;
    text-align: left;
  }

  .theme-card-btn.selected,
  .density-btn.selected {
    border-color: var(--fasti-action-primary);
    background: var(--fasti-surface-paper);
  }

  .preview-swatch {
    width: 100%;
    height: 36px;
    border-radius: 4px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
  }

  .light-swatch {
    background: #fffdf8;
  }
  .dark-swatch {
    background: #1e1e24;
  }
  .night-swatch {
    background: #000000;
  }

  .swatch-label strong {
    display: block;
    font-size: 0.9rem;
  }
  .swatch-label span {
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
  }

  .accents-row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .accent-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-radius: 20px;
    background: var(--fasti-surface-archive);
    border: 2px solid transparent;
    cursor: pointer;
    font-size: 0.85rem;
    font-weight: 600;
  }

  .accent-btn.selected {
    border-color: var(--accent-hex);
  }

  .accent-circle {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent-hex);
  }

  .prefs-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 16px;
  }

  .cache-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    margin-bottom: 16px;
  }
  .cache-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
  }
  .cache-card-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.9rem;
  }

  .diagnostics-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  a.btn-secondary {
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .custom-field-form {
    padding: 16px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
    margin-bottom: 12px;
  }
  .custom-entry-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .custom-entry-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    font-size: 0.88rem;
  }
  .entry-meta {
    display: block;
    font-size: 0.76rem;
    color: var(--fasti-text-muted);
    margin-top: 2px;
  }
  .entry-icon {
    margin-right: 6px;
  }
  .delete-entry-btn {
    background: transparent;
    border: none;
    color: var(--fasti-text-muted);
    cursor: pointer;
    padding: 4px;
    flex-shrink: 0;
  }
  .delete-entry-btn:hover {
    color: #e11d48;
  }

  .providers-list {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .provider-key-card {
    padding: 18px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
  }

  .provider-key-header {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 12px;
  }

  .provider-title {
    font-size: 1.05rem;
    font-weight: 600;
    margin: 0;
  }
  .docs-link {
    min-height: 44px;
    font-size: 0.78rem;
    color: var(--fasti-text-primary);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    text-decoration: underline;
  }
  .prov-status-chip {
    font-family: var(--fasti-font-mono);
    font-size: 0.75rem;
    padding: 2px 8px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.1);
  }
  .prov-status-chip.configured {
    background: color-mix(
      in srgb,
      var(--fasti-state-verified) 20%,
      transparent
    );
    color: var(--fasti-state-verified);
    font-weight: 700;
  }

  .key-input-row {
    display: flex;
    gap: 10px;
  }

  .api-key-input {
    flex: 1;
    min-height: 44px;
    padding: 8px 14px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-family: var(--fasti-font-mono);
    font-size: 0.9rem;
    background: var(--fasti-surface-paper);
  }

  .save-key-btn,
  .remove-key-btn {
    min-height: 44px;
    padding: 8px 18px;
    font-weight: 600;
    border-radius: 4px;
    cursor: pointer;
  }

  .save-key-btn {
    background: var(--fasti-action-primary);
    color: white;
    border: none;
  }

  .remove-key-btn {
    background: transparent;
    color: var(--fasti-text-primary);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 35%, transparent);
  }

  .provider-notice,
  .provider-problem {
    margin: 14px 0 0;
    font-size: 0.86rem;
  }

  .provider-notice {
    color: var(--fasti-state-verified);
  }

  .provider-problem {
    color: var(--fasti-state-error, #b42318);
  }

  :is(
    .nav-tab-btn,
    .api-key-input,
    .save-key-btn,
    .remove-key-btn
  ):focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }

  .wb-help {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
    margin: 6px 0 0;
  }

  .token-form-card {
    padding: 18px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
    margin-bottom: 20px;
  }
  .form-row {
    display: flex;
    gap: 10px;
    margin-bottom: 12px;
  }
  .token-name-input {
    flex: 1;
    height: 40px;
    padding: 8px 14px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    background: var(--fasti-surface-paper);
  }
  .create-token-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 18px;
    background: var(--fasti-action-primary);
    color: white;
    border: none;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
  }
  .scopes-checkboxes {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .scope-chk-label {
    font-size: 0.82rem;
    font-family: var(--fasti-font-mono);
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .tokens-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
    text-align: left;
  }
  .tokens-table th,
  .tokens-table td {
    padding: 10px 12px;
    border-bottom: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }
  .scope-pill {
    font-size: 0.72rem;
    font-family: var(--fasti-font-mono);
    padding: 2px 6px;
    background: var(--fasti-surface-archive);
    border-radius: 3px;
    margin-right: 4px;
  }
  .delete-tok-btn {
    background: transparent;
    border: none;
    color: #e11d48;
    cursor: pointer;
    padding: 4px;
  }

  .importers-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 16px;
  }
  .importer-card {
    padding: 18px;
    background: var(--fasti-surface-archive);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }
  .imp-title {
    font-size: 1.05rem;
    margin: 0 0 6px;
  }
  .imp-desc {
    font-size: 0.82rem;
    color: var(--fasti-text-muted);
    margin: 0 0 14px;
  }
  .file-upload-btn {
    display: block;
    text-align: center;
    padding: 8px 14px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-weight: 600;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .hidden-file-input {
    display: none;
  }

  .oidc-form,
  .apprise-box {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .form-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .form-field label {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--fasti-text-muted);
  }
  .form-input {
    height: 38px;
    padding: 8px 12px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-size: 0.9rem;
    background: var(--fasti-surface-paper);
  }
  .btn-save {
    padding: 10px 20px;
    background: var(--fasti-action-primary);
    color: white;
    border: none;
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
    align-self: flex-start;
  }
  .btn-test {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    background: var(--fasti-surface-archive);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-weight: 600;
    cursor: pointer;
  }
  .test-notify-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  @media (max-width: 47.99rem) {
    .settings-container {
      padding: 24px 16px;
    }

    .settings-layout {
      grid-template-columns: minmax(0, 1fr);
      gap: 16px;
    }

    .settings-nav {
      display: grid;
      grid-template-columns: minmax(0, 1fr);
    }

    .nav-tab-btn {
      width: 100%;
    }

    .settings-content-card {
      min-width: 0;
      padding: 20px;
    }

    .options-grid-3,
    .importers-grid {
      grid-template-columns: minmax(0, 1fr);
    }

    .key-input-row {
      flex-direction: column;
    }
  }
</style>
