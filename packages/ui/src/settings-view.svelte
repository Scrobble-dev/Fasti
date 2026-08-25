<script lang="ts">
  import type {
    ConnectionTestStatus,
    CustomFieldDefinition,
    ScopedApiToken,
    OutboundAccessPolicy,
    ProviderCredentialStatus,
    OidcConfiguration,
    AppriseNotificationConfig,
    ThemeSettings,
    WorkbenchPreferences,
    NavItemConfig,
    ContextMenuItemConfig,
  } from "./types.js";
  import type { ConnectionEndpoint } from "@fasti/sdk";
  import ConnectionSettings from "./connection-settings.svelte";
  import { DEFAULT_WORKBENCH_PREFERENCES } from "./mock-data.js";
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
  } from "@tabler/icons-svelte";

  interface Props {
    connectionEndpoint: ConnectionEndpoint;
    customFields: CustomFieldDefinition[];
    tokens: ScopedApiToken[];
    providerCredentials: ProviderCredentialStatus[];
    providerPolicy: OutboundAccessPolicy;
    oidcConfig: OidcConfiguration;
    appriseConfig: AppriseNotificationConfig;
    themeSettings: ThemeSettings;
    workbenchPreferences?: WorkbenchPreferences;
    onUpdateTheme?: (theme: Partial<ThemeSettings>) => void;
    onUpdateWorkbenchPreferences?: (
      prefs: Partial<WorkbenchPreferences>,
    ) => void;
    onSaveConnection: (value: string) => Promise<ConnectionEndpoint>;
    onTestConnection: (value: string) => Promise<ConnectionTestStatus>;
    onSaveProviderKey: (provider: string, key: string | null) => Promise<void>;
    onUpdateProviderPolicy: (policy: OutboundAccessPolicy) => void;
    onSaveOidc?: (config: OidcConfiguration) => void;
    onSaveApprise?: (config: AppriseNotificationConfig) => void;
  }

  let {
    connectionEndpoint,
    customFields,
    tokens,
    providerCredentials,
    providerPolicy,
    oidcConfig,
    appriseConfig,
    themeSettings,
    workbenchPreferences = DEFAULT_WORKBENCH_PREFERENCES,
    onUpdateTheme,
    onUpdateWorkbenchPreferences,
    onSaveConnection,
    onTestConnection,
    onSaveProviderKey,
    onUpdateProviderPolicy,
    onSaveOidc,
    onSaveApprise,
  }: Props = $props();

  let activeSettingsSection:
    | "connection"
    | "appearance"
    | "navigation"
    | "providers"
    | "connectors"
    | "tokens"
    | "oidc"
    | "notifications"
    | "importers" = $state("connection");

  // Local state for token generator
  let newTokenName = $state("");
  let selectedScopes: string[] = $state(["chronicle:write", "metadata:read"]);
  let newAppriseUrl = $state("");

  // Local state for keys
  let editingKeyMap: Record<string, string> = $state({});
  let savingProvider = $state<string | null>(null);
  let providerNotice = $state<{
    kind: "success" | "error";
    message: string;
  } | null>(null);
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
    oidcDraft = { ...oidcConfig };
  });

  function providerError(error: unknown): string {
    if (error !== null && typeof error === "object") {
      const value = error as { title?: string; detail?: string };
      return [value.title, value.detail].filter(Boolean).join(" ");
    }
    return "Fasti could not update the provider credential.";
  }

  async function saveCredential(
    provider: string,
    key: string | null,
  ): Promise<void> {
    savingProvider = provider;
    providerNotice = null;
    try {
      await onSaveProviderKey(provider, key);
      editingKeyMap[provider] = "";
      providerNotice = {
        kind: "success",
        message: key
          ? "The key is stored in the system credential store."
          : "The saved key was removed.",
      };
    } catch (error) {
      providerNotice = { kind: "error", message: providerError(error) };
    } finally {
      savingProvider = null;
    }
  }

  type PolicyDimension = "provider" | "capability" | "host" | "network";

  function policyAllows(dimension: PolicyDimension): boolean {
    return {
      provider: providerPolicy.deny_providers.length === 0,
      capability: providerPolicy.deny_capabilities.length === 0,
      host: providerPolicy.deny_hosts.length === 0,
      network: providerPolicy.deny_networks.length === 0,
    }[dimension];
  }

  function setPolicy(dimension: PolicyDimension, allow: boolean): void {
    const value: OutboundAccessPolicy = {
      ...providerPolicy,
      allow_providers:
        dimension === "provider"
          ? allow
            ? ["google-books"]
            : []
          : providerPolicy.allow_providers,
      deny_providers:
        dimension === "provider"
          ? allow
            ? []
            : ["google-books"]
          : providerPolicy.deny_providers,
      allow_capabilities:
        dimension === "capability"
          ? allow
            ? ["metadata.search"]
            : []
          : providerPolicy.allow_capabilities,
      deny_capabilities:
        dimension === "capability"
          ? allow
            ? []
            : ["metadata.search"]
          : providerPolicy.deny_capabilities,
      allow_hosts:
        dimension === "host"
          ? allow
            ? ["www.googleapis.com"]
            : []
          : providerPolicy.allow_hosts,
      deny_hosts:
        dimension === "host"
          ? allow
            ? []
            : ["www.googleapis.com"]
          : providerPolicy.deny_hosts,
      allow_networks:
        dimension === "network"
          ? allow
            ? ["public"]
            : []
          : providerPolicy.allow_networks,
      deny_networks:
        dimension === "network"
          ? allow
            ? []
            : ["public"]
          : providerPolicy.deny_networks,
    };
    onUpdateProviderPolicy(value);
  }

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
    onUpdateWorkbenchPreferences?.(DEFAULT_WORKBENCH_PREFERENCES);
  }
</script>

<div class="settings-container">
  <header class="settings-header">
    <div>
      <h1 class="view-title">Settings & Studio</h1>
      <p class="view-subtitle">
        Configure metadata API keys, appearance themes, media server connectors,
        and security.
      </p>
    </div>
  </header>

  <div class="settings-layout">
    <label class="settings-nav-label" for="settings-section"
      >Settings section</label
    >
    <select
      id="settings-section"
      class="settings-nav-select"
      bind:value={activeSettingsSection}
    >
      <option value="connection">Connection</option>
      <option value="appearance">Appearance & Theme</option>
      <option value="navigation">Navigation & Menus</option>
      <option value="providers">Metadata Providers & Keys</option>
      <option value="connectors">Nuvio & Media Connectors</option>
      <option value="tokens">Personal Access Tokens (PAT)</option>
      <option value="oidc">Single Sign-On (OIDC)</option>
      <option value="notifications">Notifications & Apprise</option>
      <option value="importers">Lossless Importers & Backups</option>
    </select>

    <!-- Left Settings Navigation -->
    <nav class="settings-nav" aria-label="Settings subnavigation">
      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "connection"}
        aria-current={activeSettingsSection === "connection"
          ? "page"
          : undefined}
        onclick={() => (activeSettingsSection = "connection")}
      >
        <IconDevices size={18} /> Connection
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "appearance"}
        aria-current={activeSettingsSection === "appearance"
          ? "page"
          : undefined}
        onclick={() => (activeSettingsSection = "appearance")}
      >
        <IconPalette size={18} /> Appearance & Theme
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "navigation"}
        aria-current={activeSettingsSection === "navigation"
          ? "page"
          : undefined}
        onclick={() => (activeSettingsSection = "navigation")}
      >
        <IconLayoutSidebar size={18} /> Navigation & Menus
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "providers"}
        aria-current={activeSettingsSection === "providers"
          ? "page"
          : undefined}
        onclick={() => (activeSettingsSection = "providers")}
      >
        <IconKey size={18} /> Metadata Providers & Keys
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "connectors"}
        aria-current={activeSettingsSection === "connectors"
          ? "page"
          : undefined}
        onclick={() => (activeSettingsSection = "connectors")}
      >
        <IconDeviceTv size={18} /> Nuvio & Media Connectors
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "tokens"}
        aria-current={activeSettingsSection === "tokens" ? "page" : undefined}
        onclick={() => (activeSettingsSection = "tokens")}
      >
        <IconCode size={18} /> Personal Access Tokens (PAT)
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "oidc"}
        aria-current={activeSettingsSection === "oidc" ? "page" : undefined}
        onclick={() => (activeSettingsSection = "oidc")}
      >
        <IconUserCheck size={18} /> Single Sign-On (OIDC)
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "notifications"}
        aria-current={activeSettingsSection === "notifications"
          ? "page"
          : undefined}
        onclick={() => (activeSettingsSection = "notifications")}
      >
        <IconBell size={18} /> Notifications & Apprise
      </button>

      <button
        type="button"
        class="nav-tab-btn"
        class:active={activeSettingsSection === "importers"}
        aria-current={activeSettingsSection === "importers"
          ? "page"
          : undefined}
        onclick={() => (activeSettingsSection = "importers")}
      >
        <IconDatabaseImport size={18} /> Lossless Importers & Backups
      </button>
    </nav>

    <!-- Right Settings Content Panel -->
    <section class="settings-content-card" aria-label="Settings details">
      <!-- 1. Appearance & Theme Editor (Tabler Customizer) -->
      {#if activeSettingsSection === "connection"}
        <ConnectionSettings
          endpoint={connectionEndpoint}
          onSave={onSaveConnection}
          onTest={onTestConnection}
        />
      {:else if activeSettingsSection === "appearance"}
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

        <!-- 3. Metadata Providers & Keys -->
      {:else if activeSettingsSection === "providers"}
        <section class="section-pane">
          <h2 class="pane-title">Provider access</h2>
          <p class="pane-desc">
            Google Books search is available in Fasti Desktop. A key is
            optional. Fasti never returns saved keys to this interface.
          </p>

          <div class="providers-list">
            {#each providerCredentials as prov}
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
                    {prov.configured
                      ? `Key: ${prov.source}`
                      : "Public quota; no key"}
                  </span>
                </div>

                <div class="key-input-row">
                  <input
                    type="password"
                    placeholder="Enter a new Google Books API key"
                    value={editingKeyMap[prov.provider] ?? ""}
                    oninput={(e) =>
                      (editingKeyMap[prov.provider] = e.currentTarget.value)}
                    class="api-key-input"
                    aria-label="New API key for {prov.label}"
                    autocomplete="off"
                    spellcheck="false"
                    disabled={!prov.writable ||
                      savingProvider === prov.provider}
                  />
                  <button
                    type="button"
                    class="save-key-btn"
                    disabled={!prov.writable ||
                      savingProvider === prov.provider ||
                      !(editingKeyMap[prov.provider] ?? "").trim()}
                    onclick={() =>
                      void saveCredential(
                        prov.provider,
                        editingKeyMap[prov.provider] ?? "",
                      )}
                  >
                    {savingProvider === prov.provider ? "Saving…" : "Save key"}
                  </button>
                  {#if prov.configured && prov.source === "keyring"}
                    <button
                      type="button"
                      class="remove-key-btn"
                      disabled={savingProvider === prov.provider}
                      onclick={() => void saveCredential(prov.provider, null)}
                      >Remove key</button
                    >
                  {/if}
                </div>
                {#if !prov.writable}
                  <p class="provider-help">
                    {prov.source === "environment"
                      ? "GOOGLE_BOOKS_API_KEY manages this key. Change the environment secret, then restart Fasti."
                      : "Open Fasti Desktop to manage credentials and run provider searches."}
                  </p>
                {/if}
              </div>
            {/each}
          </div>

          {#if providerNotice}
            <p
              class="provider-notice"
              class:error={providerNotice.kind === "error"}
              role={providerNotice.kind === "error" ? "alert" : "status"}
            >
              {providerNotice.message}
            </p>
          {/if}

          <div class="policy-section">
            <h3 class="form-title">Effective outbound policy</h3>
            <p class="provider-help">
              Each allow is limited by the provider manifest. A deny always
              wins. Private, loopback, link-local, multicast, documentation, and
              unspecified networks remain outside this provider manifest.
            </p>
            <div class="policy-list">
              <label class="policy-row">
                <span><strong>Provider</strong><small>google-books</small></span
                >
                <select
                  aria-label="Google Books provider access"
                  value={policyAllows("provider") ? "allow" : "deny"}
                  onchange={(event) =>
                    setPolicy(
                      "provider",
                      event.currentTarget.value === "allow",
                    )}
                  ><option value="allow">Allow</option><option value="deny"
                    >Deny</option
                  ></select
                >
              </label>
              <label class="policy-row">
                <span
                  ><strong>Capability</strong><small>metadata.search</small
                  ></span
                >
                <select
                  aria-label="Metadata search capability access"
                  value={policyAllows("capability") ? "allow" : "deny"}
                  onchange={(event) =>
                    setPolicy(
                      "capability",
                      event.currentTarget.value === "allow",
                    )}
                  ><option value="allow">Allow</option><option value="deny"
                    >Deny</option
                  ></select
                >
              </label>
              <label class="policy-row">
                <span
                  ><strong>Host</strong><small>www.googleapis.com</small></span
                >
                <select
                  aria-label="Google Books host access"
                  value={policyAllows("host") ? "allow" : "deny"}
                  onchange={(event) =>
                    setPolicy("host", event.currentTarget.value === "allow")}
                  ><option value="allow">Allow</option><option value="deny"
                    >Deny</option
                  ></select
                >
              </label>
              <label class="policy-row">
                <span
                  ><strong>Network</strong><small>Public addresses only</small
                  ></span
                >
                <select
                  aria-label="Public network access"
                  value={policyAllows("network") ? "allow" : "deny"}
                  onchange={(event) =>
                    setPolicy("network", event.currentTarget.value === "allow")}
                  ><option value="allow">Allow</option><option value="deny"
                    >Deny</option
                  ></select
                >
              </label>
            </div>
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
            Generate cryptographically verified Bearer tokens for scripts and
            devices.
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
          <p class="pane-desc">
            Authenticate with Authentik, Authelia, Keycloak, or your identity
            provider.
          </p>

          <form
            onsubmit={(e) => {
              e.preventDefault();
              onSaveOidc?.(oidcDraft);
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
        </section>

        <!-- 6. Notifications & Apprise -->
      {:else if activeSettingsSection === "notifications"}
        <section class="section-pane">
          <h2 class="pane-title">Notifications & Apprise Webhooks</h2>
          <p class="pane-desc">
            Push notifications to Discord, Telegram, Pushover, Gotify, or Slack.
          </p>

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
              <button type="submit" class="btn-secondary">+ Add Service</button>
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
        </section>

        <!-- 7. Lossless Importers & Backups -->
      {:else if activeSettingsSection === "importers"}
        <section class="section-pane">
          <h2 class="pane-title">Lossless Importers & Migrations</h2>
          <p class="pane-desc">
            Migrate your entire scrobble and media history with zero data loss.
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
    </section>
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
    min-width: 0;
  }

  .settings-nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .settings-nav-label,
  .settings-nav-select {
    display: none;
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
    min-height: var(--fasti-touch-target-min, 44px);
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
    min-width: 0;
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
    font-size: 0.78rem;
    color: var(--fasti-action-primary);
    display: inline-flex;
    align-items: center;
    gap: 4px;
    text-decoration: none;
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
    height: 40px;
    padding: 8px 14px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 30%, transparent);
    border-radius: 4px;
    font-family: var(--fasti-font-mono);
    font-size: 0.9rem;
    background: var(--fasti-surface-paper);
  }

  .save-key-btn {
    min-height: 44px;
    padding: 8px 18px;
    background: var(--fasti-action-primary);
    color: white;
    font-weight: 600;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .save-key-btn:disabled,
  .remove-key-btn:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .remove-key-btn {
    min-height: 44px;
    padding: 8px 14px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-brand-mark) 45%, transparent);
    border-radius: 4px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-brand-mark);
    font-weight: 600;
  }

  .provider-help,
  .provider-notice {
    margin: 10px 0 0;
    color: var(--fasti-text-muted);
    font-size: 0.85rem;
  }

  .provider-notice {
    padding: 12px;
    border: 1px solid var(--fasti-state-verified);
    background: var(--fasti-surface-archive);
    color: var(--fasti-text-primary);
  }

  .provider-notice.error {
    border-color: var(--fasti-brand-mark);
  }

  .policy-section {
    margin-top: 24px;
    padding-top: 20px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 24%, transparent);
  }

  .policy-list {
    display: grid;
    gap: 1px;
    margin-top: 12px;
    background: color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .policy-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 56px;
    padding: 8px 12px;
    background: var(--fasti-surface-paper);
  }

  .policy-row span,
  .policy-row small {
    display: block;
  }

  .policy-row small {
    margin-top: 2px;
    color: var(--fasti-text-muted);
    font-family: var(--fasti-font-mono);
  }

  .policy-row select {
    min-width: 104px;
    min-height: 44px;
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

  @media (max-width: 1100px) {
    .settings-layout {
      grid-template-columns: minmax(0, 1fr);
      gap: 16px;
    }

    .settings-nav {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 760px) {
    .settings-container {
      padding: 20px 16px;
      gap: 20px;
    }

    .view-title {
      font-size: 2rem;
    }

    .settings-nav {
      display: none;
    }

    .settings-nav-label,
    .settings-nav-select {
      display: block;
    }

    .settings-nav-label {
      margin-bottom: -10px;
      color: var(--fasti-text-primary);
      font-weight: 700;
    }

    .settings-nav-select {
      width: 100%;
      min-height: var(--fasti-touch-target-min, 44px);
      padding: 8px 12px;
      border: 1px solid
        color-mix(in srgb, var(--fasti-text-muted) 40%, transparent);
      border-radius: 4px;
      background: var(--fasti-surface-paper);
      color: var(--fasti-text-primary);
    }

    .settings-content-card {
      padding: 18px 16px;
    }

    .provider-key-header,
    .key-input-row {
      align-items: stretch;
      flex-direction: column;
    }
  }
</style>
