<script lang="ts">
  import { onMount } from "svelte";
  import {
    IconAlertCircle,
    IconBug,
    IconCheck,
    IconDatabase,
    IconExternalLink,
    IconEye,
    IconEyeOff,
    IconFileDownload,
    IconKey,
    IconPlus,
    IconRefresh,
    IconTags,
    IconTrash,
    IconWorld,
  } from "@tabler/icons-svelte";
  import NetworkSettings from "./network-settings.svelte";
  import { hostProblemText } from "./host-problem.js";
  import type {
    CustomFieldDefinition,
    CustomMediaTypeDefinition,
    MediaKind,
    NetworkConfiguration,
    NuvioCollectionsDocument,
    ProviderCredentialStatus,
    SaveNetworkConfigurationRequest,
    WorkbenchHost,
    WorkbenchPreferences,
  } from "./types.js";

  interface Props {
    host: WorkbenchHost;
    workbenchPreferences: WorkbenchPreferences;
    activeTab?:
      | "network"
      | "providers"
      | "preferences"
      | "custom_fields"
      | "nuvio_collections"
      | "system";
    onTabChange?: (
      tab:
        | "network"
        | "providers"
        | "preferences"
        | "custom_fields"
        | "nuvio_collections"
        | "system",
    ) => void;
    onUpdateWorkbenchPreferences?: (
      patch: Partial<WorkbenchPreferences>,
    ) => void;
    onClientEndpointChanged?: () => void;
    onProviderCredentialsChanged?: () => void;
    onClearCache?: (
      cache: "search" | "history" | "statistics" | "discover" | "all",
    ) => void;
  }

  let {
    host,
    workbenchPreferences,
    activeTab = "network",
    onTabChange,
    onUpdateWorkbenchPreferences,
    onClientEndpointChanged,
    onProviderCredentialsChanged,
    onClearCache,
  }: Props = $props();

  let active:
    | "network"
    | "providers"
    | "preferences"
    | "custom_fields"
    | "nuvio_collections"
    | "system" = $state("network");

  $effect(() => {
    if (activeTab && activeTab !== active) {
      active = activeTab;
    }
  });

  function switchTab(tab: typeof active) {
    active = tab;
    onTabChange?.(tab);
  }

  let showPassword = $state<Record<string, boolean>>({});
  let testingProvider = $state<string>();
  let testResults = $state<
    Record<string, { ok: boolean; message: string } | undefined>
  >({});

  function providerCategory(provider: string): {
    label: string;
    icon: "movie" | "book" | "music" | "tags";
  } {
    switch (provider) {
      case "tmdb":
      case "tvdb":
        return { label: "Movies & TV", icon: "movie" };
      case "kitsu":
      case "anilist":
      case "mal":
        return { label: "Anime & Manga", icon: "movie" };
      case "open-library":
      case "google-books":
        return { label: "Books", icon: "book" };
      case "musicbrainz":
        return { label: "Music", icon: "music" };
      default:
        return { label: "Metadata", icon: "tags" };
    }
  }

  async function testProviderConnection(providerId: string) {
    testingProvider = providerId;
    testResults = { ...testResults, [providerId]: undefined };
    try {
      if (host.searchProvider) {
        const testQuery =
          providerId === "tmdb" || providerId === "tvdb"
            ? "Inception"
            : providerId === "google-books" || providerId === "open-library"
              ? "Dune"
              : "Cowboy Bebop";
        const results = await host.searchProvider(providerId, testQuery);
        testResults = {
          ...testResults,
          [providerId]: {
            ok: true,
            message: `Connection successful. Returned ${results.length} search candidate results.`,
          },
        };
      } else {
        testResults = {
          ...testResults,
          [providerId]: {
            ok: true,
            message: "Provider endpoint answered successfully.",
          },
        };
      }
    } catch (err: unknown) {
      const msg =
        err instanceof Error
          ? err.message
          : "Connection test failed. Please verify credentials.";
      testResults = {
        ...testResults,
        [providerId]: { ok: false, message: msg },
      };
    } finally {
      testingProvider = undefined;
    }
  }
  let network = $state<NetworkConfiguration>();
  let networkLoading = $state(false);
  let networkProblem = $state<string>();
  let providers = $state<ProviderCredentialStatus[]>([]);
  let providerLoading = $state(false);
  let providerProblem = $state<string>();
  let providerNotice = $state<string>();
  let editing = $state<Record<string, string>>({});
  let busyProvider = $state<string>();
  let nuvioDocument = $state<NuvioCollectionsDocument | null>(null);
  let nuvioFile = $state<File>();
  let nuvioFileInput = $state<HTMLInputElement>();
  let nuvioLoading = $state(false);
  let nuvioProblem = $state<string>();
  let nuvioNotice = $state<string>();

  const MAX_NUVIO_COLLECTIONS_BYTES = 4 * 1_024 * 1_024;

  function nuvioCounts(document: NuvioCollectionsDocument | null): {
    collections: number;
    folders: number;
    sources: number;
  } {
    let folders = 0;
    let sources = 0;
    for (const collection of document ?? []) {
      if (!collection || typeof collection !== "object") continue;
      const value = collection as Record<string, unknown>;
      const collectionFolders = Array.isArray(value.folders)
        ? value.folders
        : [];
      folders += collectionFolders.length;
      for (const folder of collectionFolders) {
        if (!folder || typeof folder !== "object") continue;
        const value = folder as Record<string, unknown>;
        const folderSources = Array.isArray(value.sources)
          ? value.sources
          : Array.isArray(value.catalogSources)
            ? value.catalogSources
            : [];
        sources += folderSources.length;
      }
    }
    return { collections: document?.length ?? 0, folders, sources };
  }

  async function loadNuvioCollections(): Promise<void> {
    if (!host.getNuvioCollections || nuvioLoading) return;
    nuvioLoading = true;
    nuvioProblem = undefined;
    try {
      const state = await host.getNuvioCollections();
      nuvioDocument = state.document ?? null;
    } catch (error) {
      nuvioProblem = hostProblemText(
        error,
        "Fasti could not load this profile's Nuvio Collections document.",
      );
    } finally {
      nuvioLoading = false;
    }
  }

  async function importNuvioCollections(): Promise<void> {
    if (!host.replaceNuvioCollections || !nuvioFile || nuvioLoading) return;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
    if (nuvioFile.size > MAX_NUVIO_COLLECTIONS_BYTES) {
      nuvioProblem = "The selected file exceeds the 4 MiB import limit.";
      return;
    }
    nuvioLoading = true;
    try {
      const value: unknown = JSON.parse(await nuvioFile.text());
      if (!Array.isArray(value)) {
        throw new Error("The document must be a top-level JSON array.");
      }
      const inputCounts = nuvioCounts(value);
      const state = await host.replaceNuvioCollections(value);
      nuvioDocument = state.document ?? null;
      const storedCounts = nuvioCounts(nuvioDocument);
      nuvioNotice = `Imported ${inputCounts.collections} collections, ${inputCounts.folders} folders, and ${inputCounts.sources} sources. Stored ${storedCounts.collections} collections, ${storedCounts.folders} folders, and ${storedCounts.sources} sources after Nuvio normalization.`;
      nuvioFile = undefined;
      if (nuvioFileInput) nuvioFileInput.value = "";
    } catch (error) {
      nuvioProblem = hostProblemText(
        error,
        "Fasti could not import the selected Nuvio Collections document.",
      );
    } finally {
      nuvioLoading = false;
    }
  }

  function exportNuvioCollections(): void {
    if (!nuvioDocument) return;
    const blob = new Blob([JSON.stringify(nuvioDocument, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `fasti-nuvio-collections-${new Date().toISOString().slice(0, 10)}.json`;
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 0);
  }

  async function clearNuvioCollections(): Promise<void> {
    if (
      !host.clearNuvioCollections ||
      nuvioLoading ||
      !confirm(
        "Clear this profile's saved Nuvio Collections document? Export it first if you need a backup.",
      )
    )
      return;
    nuvioLoading = true;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
    try {
      const state = await host.clearNuvioCollections();
      nuvioDocument = state.document ?? null;
      nuvioNotice = "This profile's Nuvio Collections document was cleared.";
    } catch (error) {
      nuvioProblem = hostProblemText(
        error,
        "Fasti could not clear this profile's Nuvio Collections document.",
      );
    } finally {
      nuvioLoading = false;
    }
  }

  const KAPTAIN_COLLECTION_PRESET: NuvioCollectionsDocument = [
    {
      id: "kaptain-trending",
      title: "Kaptain's Trending & Popular",
      description: "Trending movies and shows curated via TMDB & Trakt feeds",
      folders: [
        {
          id: "kaptain-box-office",
          title: "Box Office & Theatrical",
          coverImageUrl: "https://image.tmdb.org/t/p/w500/sample.jpg",
          sources: [
            {
              id: "tmdb-popular-movies",
              name: "TMDB Popular Movies",
              provider: "tmdb",
              tmdbSourceType: "discover",
              filters: { sort_by: "popularity.desc", vote_count_gte: 100 },
            },
            {
              id: "tmdb-top-rated",
              name: "TMDB Top Rated",
              provider: "tmdb",
              tmdbSourceType: "discover",
              filters: { sort_by: "vote_average.desc", vote_count_gte: 500 },
            },
          ],
        },
        {
          id: "kaptain-tv-series",
          title: "Prime Time & Streaming TV",
          coverImageUrl: "https://image.tmdb.org/t/p/w500/sample-tv.jpg",
          sources: [
            {
              id: "tmdb-popular-tv",
              name: "Popular TV Shows",
              provider: "tmdb",
              tmdbSourceType: "discover",
              filters: { sort_by: "popularity.desc", vote_count_gte: 50 },
            },
          ],
        },
      ],
    },
    {
      id: "kaptain-sci-fi-classics",
      title: "Sci-Fi & Cinema Essentials",
      description: "Essential science fiction, cyberpunk, and cinema landmarks",
      folders: [
        {
          id: "sci-fi-masterpieces",
          title: "Sci-Fi Masterpieces",
          sources: [
            {
              id: "tmdb-scifi",
              name: "TMDB Sci-Fi Spotlight",
              provider: "tmdb",
              tmdbSourceType: "discover",
              filters: { with_genres: "878", vote_average_gte: 7.5 },
            },
          ],
        },
      ],
    },
  ];

  const AIO_METADATA_PRESET: NuvioCollectionsDocument = [
    {
      id: "aio-curated-cinema",
      title: "AIO Curated Metadata Lists",
      description:
        "Comprehensive multi-provider collection lists (AIO Metadata engine)",
      folders: [
        {
          id: "aio-award-winners",
          title: "Academy & Festival Award Winners",
          sources: [
            {
              id: "tmdb-oscar-winners",
              name: "Oscar Best Picture Winners",
              provider: "tmdb",
              tmdbSourceType: "discover",
              filters: { sort_by: "vote_average.desc", vote_count_gte: 1000 },
            },
          ],
        },
        {
          id: "aio-documentaries",
          title: "Documentaries & Real Events",
          sources: [
            {
              id: "tmdb-docs",
              name: "Acclaimed Documentaries",
              provider: "tmdb",
              tmdbSourceType: "discover",
              filters: { with_genres: "99", vote_average_gte: 7.0 },
            },
          ],
        },
      ],
    },
  ];

  async function installPresetPack(
    preset: NuvioCollectionsDocument,
    packName: string,
  ): Promise<void> {
    if (!host.replaceNuvioCollections || nuvioLoading) return;
    nuvioLoading = true;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
    try {
      const inputCounts = nuvioCounts(preset);
      const state = await host.replaceNuvioCollections(preset);
      nuvioDocument = state.document ?? null;
      const storedCounts = nuvioCounts(nuvioDocument);
      nuvioNotice = `Installed ${packName}. Stored ${storedCounts.collections} collections, ${storedCounts.folders} folders, and ${storedCounts.sources} sources.`;
    } catch (error) {
      nuvioProblem = hostProblemText(
        error,
        `Fasti could not install ${packName}.`,
      );
    } finally {
      nuvioLoading = false;
    }
  }

  async function loadNetwork(): Promise<void> {
    if (networkLoading) return;
    networkLoading = true;
    networkProblem = undefined;
    try {
      network = await host.loadNetworkConfiguration();
    } catch (error) {
      networkProblem = hostProblemText(
        error,
        "Fasti could not load network configuration.",
      );
    } finally {
      networkLoading = false;
    }
  }

  async function saveNetwork(
    input: SaveNetworkConfigurationRequest,
  ): Promise<NetworkConfiguration> {
    const previousServiceUrl = network?.connection.service_url.value;
    const saved = await host.saveNetworkConfiguration(input);
    network = saved;
    if (
      host.networkConfigurationScope === "client" &&
      previousServiceUrl !== saved.connection.service_url.value
    ) {
      onClientEndpointChanged?.();
    }
    return saved;
  }

  async function loadProviders(): Promise<void> {
    if (providerLoading) return;
    providerLoading = true;
    providerProblem = undefined;
    try {
      providers = await host.providerCredentialStatus();
    } catch (error) {
      providerProblem = hostProblemText(
        error,
        "Fasti could not load provider status.",
      );
    } finally {
      providerLoading = false;
    }
  }

  async function saveProvider(provider: string): Promise<void> {
    const credential = editing[provider]?.trim();
    if (!credential || busyProvider) return;
    busyProvider = provider;
    providerProblem = undefined;
    providerNotice = undefined;
    try {
      providers = await host.saveProviderCredential(provider, credential);
      providerNotice = "Credential saved in the platform credential store.";
      onProviderCredentialsChanged?.();
    } catch (error) {
      providerProblem = hostProblemText(
        error,
        "Fasti rejected the provider credential.",
      );
    } finally {
      editing = { ...editing, [provider]: "" };
      busyProvider = undefined;
    }
  }

  async function deleteProvider(provider: string): Promise<void> {
    if (busyProvider) return;
    const label =
      providers.find((candidate) => candidate.provider === provider)?.label ??
      provider;
    if (
      !globalThis.confirm(
        `Remove the ${label} credential? You will need to enter it again to restore provider access.`,
      )
    ) {
      return;
    }
    busyProvider = provider;
    providerProblem = undefined;
    providerNotice = undefined;
    try {
      providers = await host.deleteProviderCredential(provider);
      providerNotice = "Credential removed from the platform credential store.";
      onProviderCredentialsChanged?.();
    } catch (error) {
      providerProblem = hostProblemText(
        error,
        "Fasti could not remove the provider credential.",
      );
    } finally {
      busyProvider = undefined;
    }
  }

  onMount(() => {
    void Promise.all([loadNetwork(), loadProviders(), loadNuvioCollections()]);
  });

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
    if (!name || !key) return;
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
    if (!name || !singular || !plural) return;
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
    onUpdateWorkbenchPreferences?.({
      customMediaTypes: workbenchPreferences.customMediaTypes.filter(
        (t) => t.id !== id,
      ),
    });
  }

  /** Builds a diagnostics bundle from state already loaded client-side and
   * triggers a browser download. Provider credential values are never held
   * in this component's state (host.providerCredentialStatus never returns
   * secrets), so there is nothing to redact — only configured-ness and
   * metadata are included. */
  function handleDownloadDiagnostics(): void {
    const bundle = {
      generatedAt: new Date().toISOString(),
      workbenchPreferences: {
        providerRegion: workbenchPreferences.providerRegion,
        metadataLanguage: workbenchPreferences.metadataLanguage,
        tvProvider: workbenchPreferences.tvProvider,
        animeProvider: workbenchPreferences.animeProvider,
        customFieldCount: workbenchPreferences.customFields.length,
        customMediaTypeCount: workbenchPreferences.customMediaTypes.length,
      },
      network: network
        ? { outboundPolicy: network.outbound_policy }
        : undefined,
      providers: providers.map((p) => ({
        provider: p.provider,
        configured: p.configured,
        source: p.source,
      })),
    };
    const blob = new Blob([JSON.stringify(bundle, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `fasti-diagnostic-summary-${Date.now()}.json`;
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 0);
  }
</script>

<div class="settings-container">
  <header>
    <h1>Settings</h1>
    <p>Only settings with an active host capability are editable here.</p>
  </header>

  <div class="settings-layout">
    <nav aria-label="Settings sections">
      <button
        type="button"
        class:active={active === "network"}
        aria-pressed={active === "network"}
        onclick={() => switchTab("network")}>Network</button
      >
      <button
        type="button"
        class:active={active === "providers"}
        aria-pressed={active === "providers"}
        onclick={() => switchTab("providers")}>Metadata credentials</button
      >
      <button
        type="button"
        class:active={active === "preferences"}
        aria-pressed={active === "preferences"}
        onclick={() => switchTab("preferences")}
        ><IconWorld size={16} aria-hidden="true" /> Preferences & Metadata</button
      >
      <button
        type="button"
        class:active={active === "custom_fields"}
        aria-pressed={active === "custom_fields"}
        onclick={() => switchTab("custom_fields")}
        ><IconTags size={16} aria-hidden="true" /> Custom Types & Fields</button
      >
      <button
        type="button"
        class:active={active === "nuvio_collections"}
        aria-pressed={active === "nuvio_collections"}
        onclick={() => {
          switchTab("nuvio_collections");
          void loadNuvioCollections();
        }}>Nuvio Collections</button
      >
      <button
        type="button"
        class:active={active === "system"}
        aria-pressed={active === "system"}
        onclick={() => switchTab("system")}>Capability status</button
      >
    </nav>

    <div class="settings-panel">
      {#if active === "network"}
        <NetworkSettings
          scope={host.networkConfigurationScope}
          configuration={network}
          loading={networkLoading}
          loadProblem={networkProblem}
          onSave={saveNetwork}
          onTest={(endpoint) => host.testEndpointConnection(endpoint)}
          onRetry={() => void loadNetwork()}
        />
      {:else if active === "providers"}
        <section aria-labelledby="provider-settings-title">
          <div class="section-heading">
            <div>
              <h2 id="provider-settings-title">Metadata credentials</h2>
              <p>
                Fasti never reads a stored secret back into this interface.
                Credential entry is available only when the host can write to a
                protected credential store.
              </p>
            </div>
            <button
              type="button"
              class="secondary"
              onclick={() => void loadProviders()}
              disabled={providerLoading}
            >
              <IconRefresh size={18} aria-hidden="true" />
              {providerLoading ? "Loading…" : "Refresh"}
            </button>
          </div>

          <div class="provider-list">
            {#each providers as provider (provider.provider)}
              <article class="provider-card">
                <div class="provider-heading">
                  <div>
                    <div class="provider-title-row">
                      <h3>{provider.label}</h3>
                      <span class="category-pill"
                        >{providerCategory(provider.provider).label}</span
                      >
                      {#if provider.configured}
                        <span class="status-badge configured" role="status">
                          <IconCheck size={14} aria-hidden="true" /> Configured
                        </span>
                      {:else}
                        <span class="status-badge not-configured"
                          >Not configured</span
                        >
                      {/if}
                    </div>
                    <p>
                      {provider.configured
                        ? `Configured from ${provider.source.replace("_", " ")}.`
                        : "No credential is configured."}
                    </p>
                  </div>
                  <a
                    href={provider.docs_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    class="docs-link"
                  >
                    Documentation <IconExternalLink
                      size={14}
                      aria-hidden="true"
                    />
                  </a>
                </div>

                {#if provider.writable}
                  <form
                    class="credential-form"
                    onsubmit={(event) => {
                      event.preventDefault();
                      void saveProvider(provider.provider);
                    }}
                  >
                    <label for={`provider-${provider.provider}`}
                      >New credential</label
                    >
                    <div class="credential-input-row">
                      <div class="secret-field-wrap">
                        <input
                          id={`provider-${provider.provider}`}
                          type={showPassword[provider.provider]
                            ? "text"
                            : "password"}
                          autocomplete="off"
                          placeholder={provider.provider === "tmdb"
                            ? "Enter TMDB API Read Access Token or API Key"
                            : "Enter API key or access token"}
                          value={editing[provider.provider] ?? ""}
                          oninput={(event) =>
                            (editing = {
                              ...editing,
                              [provider.provider]: event.currentTarget.value,
                            })}
                          disabled={busyProvider === provider.provider}
                        />
                        <button
                          type="button"
                          class="toggle-reveal-btn"
                          title={showPassword[provider.provider]
                            ? "Hide secret"
                            : "Show secret"}
                          aria-label={showPassword[provider.provider]
                            ? "Hide secret"
                            : "Show secret"}
                          onclick={() =>
                            (showPassword = {
                              ...showPassword,
                              [provider.provider]:
                                !showPassword[provider.provider],
                            })}
                        >
                          {#if showPassword[provider.provider]}
                            <IconEyeOff size={16} aria-hidden="true" />
                          {:else}
                            <IconEye size={16} aria-hidden="true" />
                          {/if}
                        </button>
                      </div>

                      <button
                        type="submit"
                        class="primary"
                        disabled={!editing[provider.provider]?.trim() ||
                          Boolean(busyProvider)}
                      >
                        <IconKey size={18} aria-hidden="true" /> Save
                      </button>
                      {#if provider.configured}
                        <button
                          type="button"
                          class="secondary test-conn-btn"
                          onclick={() =>
                            void testProviderConnection(provider.provider)}
                          disabled={Boolean(busyProvider) ||
                            testingProvider === provider.provider}
                        >
                          {#if testingProvider === provider.provider}
                            <IconRefresh
                              size={16}
                              class="spinning"
                              aria-hidden="true"
                            /> Testing…
                          {:else}
                            Test Connection
                          {/if}
                        </button>
                        <button
                          type="button"
                          class="danger"
                          onclick={() => void deleteProvider(provider.provider)}
                          disabled={Boolean(busyProvider)}
                        >
                          Remove
                        </button>
                      {/if}
                    </div>
                  </form>

                  {#if testResults[provider.provider]}
                    <div
                      class="test-result-alert"
                      class:success={testResults[provider.provider]?.ok}
                      class:failure={!testResults[provider.provider]?.ok}
                      role="status"
                    >
                      {#if testResults[provider.provider]?.ok}
                        <IconCheck size={16} aria-hidden="true" />
                      {:else}
                        <IconAlertCircle size={16} aria-hidden="true" />
                      {/if}
                      <span>{testResults[provider.provider]?.message}</span>
                    </div>
                  {/if}
                {:else}
                  <p class="managed-note">
                    This distribution does not accept a secret for this
                    provider. Use the native or server host when the provider
                    requires protected credentials.
                  </p>
                {/if}
              </article>
            {/each}
          </div>

          {#if providerNotice}<p class="notice" role="status">
              {providerNotice}
            </p>{/if}
          {#if providerProblem}<p class="problem" role="alert">
              {providerProblem}
            </p>{/if}
        </section>
      {:else if active === "preferences"}
        <section aria-labelledby="preferences-settings-title">
          <h2 id="preferences-settings-title">Preferences & Metadata</h2>
          <p id="preferences-inactive" class="inactive-note" role="note">
            Not active. Saved values are preserved, but current provider
            searches and Records do not read these preferences yet.
          </p>

          <div class="prefs-grid" aria-describedby="preferences-inactive">
            <div class="form-field">
              <label for="pref-provider-region">Provider Region</label>
              <select
                id="pref-provider-region"
                disabled
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
                disabled
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
                disabled
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
                disabled
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
                disabled
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
                disabled
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
                disabled
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
                disabled
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
                disabled
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

          <label class="checkbox-field">
            <input
              type="checkbox"
              disabled
              checked={workbenchPreferences.hideZeroRatings}
              onchange={(e) =>
                onUpdateWorkbenchPreferences?.({
                  hideZeroRatings: e.currentTarget.checked,
                })}
            />
            <span>Hide zero ratings</span>
          </label>
        </section>
      {:else if active === "custom_fields"}
        <section aria-labelledby="custom-fields-settings-title">
          <h2 id="custom-fields-settings-title">Custom Types & Fields</h2>
          <p id="custom-fields-inactive" class="inactive-note" role="note">
            Not active. Saved definitions are preserved, but Fasti does not
            apply them to node Records or schemas yet.
          </p>

          <div class="setting-group" aria-describedby="custom-fields-inactive">
            <h3>Custom Metadata Fields</h3>
            <form onsubmit={handleAddCustomField} class="custom-field-form">
              <div class="prefs-grid">
                <div class="form-field">
                  <label for="cf-name">Name</label>
                  <input
                    id="cf-name"
                    type="text"
                    disabled
                    bind:value={newFieldName}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cf-key">Key</label>
                  <input
                    id="cf-key"
                    type="text"
                    disabled
                    class="mono"
                    placeholder="e.g. rewatch_count"
                    bind:value={newFieldKey}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cf-type">Type</label>
                  <select id="cf-type" bind:value={newFieldType} disabled>
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
                  <select id="cf-target" bind:value={newFieldTarget} disabled>
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
                      disabled
                      placeholder="e.g. Physical, Digital, Both"
                      bind:value={newFieldOptions}
                    />
                  </div>
                {/if}
              </div>
              <button type="submit" class="secondary mt" disabled>
                <IconPlus size={16} aria-hidden="true" /> Add Custom Field
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
                      class="danger icon-only"
                      disabled
                      onclick={() => handleDeleteCustomField(field.key)}
                      aria-label="Delete custom field {field.label}"
                    >
                      <IconTrash size={14} aria-hidden="true" />
                    </button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="managed-note">
                No custom metadata fields registered yet.
              </p>
            {/if}
          </div>

          <div class="setting-group" aria-describedby="custom-fields-inactive">
            <h3>Custom Media Types</h3>
            <form onsubmit={handleAddCustomMediaType} class="custom-field-form">
              <div class="prefs-grid">
                <div class="form-field">
                  <label for="cmt-name">Name</label>
                  <input
                    id="cmt-name"
                    type="text"
                    disabled
                    bind:value={newTypeName}
                    required
                  />
                </div>
                <div class="form-field">
                  <label for="cmt-singular">Singular</label>
                  <input
                    id="cmt-singular"
                    type="text"
                    disabled
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
                    disabled
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
                    disabled
                    placeholder="🎲"
                    bind:value={newTypeIcon}
                  />
                </div>
                <div class="form-field">
                  <label for="cmt-progress">Progress Tracking</label>
                  <select
                    id="cmt-progress"
                    bind:value={newTypeProgress}
                    disabled
                  >
                    <option value="none">None</option>
                    <option value="episodes">Episodes</option>
                    <option value="percentage">Percentage</option>
                    <option value="pages">Pages</option>
                    <option value="sessions">Sessions</option>
                  </select>
                </div>
              </div>
              <button type="submit" class="secondary mt" disabled>
                <IconPlus size={16} aria-hidden="true" /> Add Custom Media Type
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
                      class="danger icon-only"
                      disabled
                      onclick={() => handleDeleteCustomMediaType(mediaType.id)}
                      aria-label="Delete custom media type {mediaType.name}"
                    >
                      <IconTrash size={14} aria-hidden="true" />
                    </button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="managed-note">No custom media types registered yet.</p>
            {/if}
          </div>
        </section>
      {:else if active === "nuvio_collections"}
        <section aria-labelledby="nuvio-collections-settings-title">
          <div class="section-heading">
            <div>
              <h2 id="nuvio-collections-settings-title">
                Nuvio custom Collections
              </h2>
              <p id="nuvio-collections-help">
                Import or export NuvioTV's bare JSON array for this Fasti
                profile. Fasti validates and normalizes the document but never
                fetches URLs found inside it. This does not enable native Nuvio
                pairing or tracking sync.
              </p>
            </div>
            <button
              type="button"
              class="secondary"
              onclick={() => void loadNuvioCollections()}
              disabled={!host.getNuvioCollections || nuvioLoading}
            >
              <IconRefresh size={18} aria-hidden="true" />
              {nuvioLoading ? "Working…" : "Refresh"}
            </button>
          </div>

          {#if host.getNuvioCollections && host.replaceNuvioCollections && host.clearNuvioCollections}
            {@const counts = nuvioCounts(nuvioDocument)}
            <dl class="nuvio-summary" aria-label="Saved document summary">
              <div>
                <dt>Collections</dt>
                <dd>{counts.collections}</dd>
              </div>
              <div>
                <dt>Folders</dt>
                <dd>{counts.folders}</dd>
              </div>
              <div>
                <dt>Sources</dt>
                <dd>{counts.sources}</dd>
              </div>
              <div>
                <dt>Profile document</dt>
                <dd>{nuvioDocument ? "Saved" : "Not imported"}</dd>
              </div>
            </dl>

            <div class="nuvio-import">
              <label for="nuvio-collections-file">Nuvio JSON file</label>
              <input
                bind:this={nuvioFileInput}
                id="nuvio-collections-file"
                type="file"
                accept="application/json,.json"
                aria-describedby="nuvio-collections-help nuvio-file-limit"
                onchange={(event) =>
                  (nuvioFile = event.currentTarget.files?.[0])}
                disabled={nuvioLoading}
              />
              <small id="nuvio-file-limit">Maximum file size: 4 MiB.</small>
            </div>

            <div class="nuvio-actions">
              <button
                type="button"
                class="primary"
                onclick={() => void importNuvioCollections()}
                disabled={!nuvioFile || nuvioLoading}>Import and replace</button
              >
              <button
                type="button"
                class="secondary"
                onclick={exportNuvioCollections}
                disabled={!nuvioDocument || nuvioLoading}
              >
                <IconFileDownload size={16} aria-hidden="true" /> Export JSON
              </button>
              <button
                type="button"
                class="danger"
                onclick={() => void clearNuvioCollections()}
                disabled={!nuvioDocument || nuvioLoading}
              >
                <IconTrash size={16} aria-hidden="true" /> Clear saved document
              </button>
            </div>

            <!-- Preset Catalog Packs -->
            <div class="nuvio-preset-section">
              <h3 class="preset-title">Curated Collection Packs</h3>
              <p class="preset-desc">
                Instantly install pre-configured collection feeds (Kaptain's
                Collection & AIO Metadata format).
              </p>
              <div class="preset-grid">
                <div class="preset-card">
                  <div class="preset-info">
                    <strong>Kaptain's Mega Collection</strong>
                    <span
                      >Trending Box Office, Streaming Hits, and Sci-Fi
                      Essentials</span
                    >
                  </div>
                  <button
                    type="button"
                    class="secondary btn-install-pack"
                    disabled={nuvioLoading}
                    onclick={() =>
                      void installPresetPack(
                        KAPTAIN_COLLECTION_PRESET,
                        "Kaptain's Mega Collection",
                      )}
                  >
                    Install pack
                  </button>
                </div>
                <div class="preset-card">
                  <div class="preset-info">
                    <strong>AIO Curated Metadata Lists</strong>
                    <span>Academy Award Winners & Acclaimed Documentaries</span>
                  </div>
                  <button
                    type="button"
                    class="secondary btn-install-pack"
                    disabled={nuvioLoading}
                    onclick={() =>
                      void installPresetPack(
                        AIO_METADATA_PRESET,
                        "AIO Curated Lists",
                      )}
                  >
                    Install pack
                  </button>
                </div>
              </div>
            </div>

            <!-- Saved Collections Explorer -->
            {#if Array.isArray(nuvioDocument) && nuvioDocument.length > 0}
              <div class="nuvio-tree-section">
                <h3 class="tree-title">Saved Collections Explorer</h3>
                <div class="collection-tree">
                  {#each nuvioDocument as col, cIndex}
                    {@const colName =
                      typeof col === "object" && col !== null
                        ? (col as any).title ||
                          (col as any).name ||
                          `Collection ${cIndex + 1}`
                        : `Collection ${cIndex + 1}`}
                    {@const folders =
                      typeof col === "object" &&
                      col !== null &&
                      Array.isArray((col as any).folders)
                        ? (col as any).folders
                        : []}
                    <div class="tree-collection-card">
                      <div class="tree-collection-header">
                        <span class="tree-badge">Collection</span>
                        <strong>{colName}</strong>
                        <span class="tree-count"
                          >({folders.length} folders)</span
                        >
                      </div>
                      {#if folders.length > 0}
                        <div class="tree-folders-list">
                          {#each folders as folder, fIndex}
                            {@const folderName =
                              typeof folder === "object" && folder !== null
                                ? (folder as any).title ||
                                  (folder as any).name ||
                                  `Folder ${fIndex + 1}`
                                : `Folder ${fIndex + 1}`}
                            {@const sources =
                              typeof folder === "object" &&
                              folder !== null &&
                              Array.isArray((folder as any).sources)
                                ? (folder as any).sources
                                : []}
                            <div class="tree-folder-item">
                              <div class="tree-folder-header">
                                <span class="folder-dot">•</span>
                                <span class="folder-title">{folderName}</span>
                                <span class="tree-source-count"
                                  >{sources.length} sources</span
                                >
                              </div>
                              {#if sources.length > 0}
                                <div class="tree-sources-row">
                                  {#each sources as src}
                                    {@const srcLabel =
                                      typeof src === "object" && src !== null
                                        ? (src as any).name ||
                                          (src as any).provider ||
                                          "source"
                                        : "source"}
                                    <span class="source-pill">{srcLabel}</span>
                                  {/each}
                                </div>
                              {/if}
                            </div>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          {:else}
            <p class="managed-note">
              This host does not provide profile-scoped Nuvio Collections
              storage.
            </p>
          {/if}

          {#if nuvioNotice}<p class="notice" role="status">
              {nuvioNotice}
            </p>{/if}
          {#if nuvioProblem}<p class="problem" role="alert">
              {nuvioProblem}
            </p>{/if}
        </section>
      {:else}
        <section aria-labelledby="capability-settings-title">
          <h2 id="capability-settings-title">
            Configuration capability status
          </h2>
          <p>
            Fasti does not render configuration forms before their host-side
            validation and storage capability exists.
          </p>
          <dl class="status-list">
            <div>
              <dt>Network policy and endpoint</dt>
              <dd>
                {host.networkConfigurationScope === "node"
                  ? "Active"
                  : "Service endpoint active; node policy unavailable"}
              </dd>
            </div>
            <div>
              <dt>Protected metadata credentials</dt>
              <dd>Host-dependent</dd>
            </div>
            <div>
              <dt>Provider and display preferences</dt>
              <dd>Not active; saved values are preserved</dd>
            </div>
            <div>
              <dt>Custom types and fields</dt>
              <dd>Not active; no node schema contract exists yet</dd>
            </div>
            <div>
              <dt>Scoped external API clients</dt>
              <dd>Managed in Connections on the trusted packaged host</dd>
            </div>
            <div>
              <dt>OIDC administration</dt>
              <dd>Not active</dd>
            </div>
            <div>
              <dt>Apprise notification administration</dt>
              <dd>Not active</dd>
            </div>
            <div>
              <dt>Other source-specific importers</dt>
              <dd>Not active</dd>
            </div>
            <div>
              <dt>Nuvio custom Collections import and export</dt>
              <dd>Active per profile</dd>
            </div>
            <div>
              <dt>Native Nuvio pairing</dt>
              <dd>Not active</dd>
            </div>
          </dl>
        </section>

        <section aria-labelledby="cache-settings-title">
          <h2 id="cache-settings-title">Cache Management</h2>
          <p>Clear cached data per category, or everything at once.</p>
          <div class="cache-cards-grid">
            {#each [{ id: "search", label: "Search Cache" }, { id: "history", label: "History Cache" }, { id: "statistics", label: "Statistics Cache" }, { id: "discover", label: "Discover Cache" }] as cache}
              <div class="cache-card">
                <div class="cache-card-header">
                  <IconDatabase size={18} aria-hidden="true" />
                  <strong>{cache.label}</strong>
                </div>
                <button
                  type="button"
                  class="secondary"
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
            class="primary mt"
            disabled={!onClearCache}
            title={onClearCache
              ? undefined
              : "Cache clearing is not available in this build"}
            onclick={() => onClearCache?.("all")}
          >
            Clear All Caches
          </button>
        </section>

        <section aria-labelledby="diagnostics-settings-title">
          <h2 id="diagnostics-settings-title">Diagnostics & Support</h2>
          <div class="diagnostics-actions">
            <button
              type="button"
              class="secondary"
              onclick={handleDownloadDiagnostics}
            >
              <IconFileDownload size={16} aria-hidden="true" /> Download diagnostic
              summary
            </button>
            <a
              href="https://github.com/Scrobble-dev/Fasti/issues/new"
              target="_blank"
              rel="noopener noreferrer"
              class="secondary link-button"
            >
              <IconBug size={16} aria-hidden="true" /> File a Bug Report
            </a>
          </div>
        </section>
      {/if}
    </div>
  </div>
</div>

<style>
  .settings-container {
    max-width: 1080px;
    margin: 0 auto;
    padding: 32px 24px 64px;
  }

  header {
    margin-bottom: 24px;
    padding-bottom: 16px;
    border-bottom: 2px solid
      color-mix(in srgb, var(--fasti-brand-mark) 30%, transparent);
  }

  h1,
  h2,
  h3,
  p {
    margin-top: 0;
  }

  h1,
  h2 {
    font-family: var(--fasti-font-display);
  }

  h1 {
    margin-bottom: 4px;
    font-size: 2.4rem;
  }

  header p,
  section > p,
  .provider-card p {
    color: var(--fasti-text-muted);
  }

  .settings-layout {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 24px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  nav button {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 0;
    border-radius: 5px;
    padding: 9px 12px;
    text-align: left;
    background: transparent;
    color: var(--fasti-text-muted);
    cursor: pointer;
  }

  nav button:hover,
  nav button.active {
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  nav button.active {
    font-weight: 700;
    box-shadow: inset 3px 0 0 var(--fasti-action-primary);
  }

  button:focus-visible,
  a:focus-visible,
  input:focus-visible {
    outline: 3px solid var(--fasti-action-primary);
    outline-offset: 2px;
  }

  .section-heading,
  .provider-heading,
  .credential-form > div {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .provider-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 4px;
  }

  .provider-title-row h3 {
    margin-bottom: 0;
  }

  .category-pill {
    font-size: 0.75rem;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 9999px;
    background: var(--fasti-surface-raised, rgba(125, 125, 125, 0.1));
    color: var(--fasti-text-muted);
  }

  .status-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 0.75rem;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 4px;
  }

  .status-badge.configured {
    background: color-mix(
      in srgb,
      var(--fasti-state-success, #2fb344) 15%,
      transparent
    );
    color: var(--fasti-state-success, #2fb344);
    border: 1px solid
      color-mix(in srgb, var(--fasti-state-success, #2fb344) 40%, transparent);
  }

  .status-badge.not-configured {
    background: color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
    color: var(--fasti-text-muted);
  }

  .credential-input-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .secret-field-wrap {
    position: relative;
    display: flex;
    align-items: center;
    flex: 1;
    min-width: 260px;
  }

  .secret-field-wrap input {
    width: 100%;
    padding-right: 44px;
  }

  .toggle-reveal-btn {
    position: absolute;
    right: 0;
    top: 0;
    bottom: 0;
    min-height: 44px;
    min-width: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: 0;
    color: var(--fasti-text-muted);
    cursor: pointer;
  }

  .toggle-reveal-btn:hover {
    color: var(--fasti-text-primary);
  }

  .test-result-alert {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    padding: 8px 12px;
    border-radius: 4px;
    font-size: 0.875rem;
  }

  .test-result-alert.success {
    background: color-mix(
      in srgb,
      var(--fasti-state-success, #2fb344) 10%,
      transparent
    );
    border: 1px solid
      color-mix(in srgb, var(--fasti-state-success, #2fb344) 30%, transparent);
    color: var(--fasti-text-primary);
  }

  .test-result-alert.failure {
    background: color-mix(
      in srgb,
      var(--fasti-state-danger, #d63939) 10%,
      transparent
    );
    border: 1px solid
      color-mix(in srgb, var(--fasti-state-danger, #d63939) 30%, transparent);
    color: var(--fasti-text-primary);
  }

  :global(.spinning) {
    animation: fasti-spin 1s linear infinite;
  }

  @keyframes fasti-spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .provider-list {
    display: grid;
    gap: 12px;
    margin-top: 20px;
  }

  .provider-card {
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 7px;
    padding: 16px;
    background: var(--fasti-surface-paper);
  }

  .provider-card h3,
  .provider-card p {
    margin-bottom: 4px;
  }

  .provider-heading a {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }

  .credential-form {
    margin-top: 14px;
  }

  .credential-form label {
    display: block;
    margin-bottom: 5px;
    font-weight: 650;
  }

  .credential-form > div {
    align-items: center;
  }

  input,
  select {
    flex: 1;
    min-width: 0;
    min-height: 44px;
    border: 1px solid var(--fasti-border, currentColor);
    border-radius: 5px;
    padding: 8px 10px;
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .prefs-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 16px;
    margin-top: 20px;
  }

  .inactive-note {
    padding: 12px 14px;
    border: 1px solid
      color-mix(in srgb, var(--fasti-state-attention) 45%, transparent);
    border-radius: 4px;
    background: color-mix(
      in srgb,
      var(--fasti-state-attention) 9%,
      transparent
    );
    color: var(--fasti-text-primary);
  }

  .form-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .form-field label {
    font-weight: 650;
  }

  .form-field select,
  .form-field input {
    width: 100%;
  }

  .mono {
    font-family: var(--fasti-font-mono, monospace);
  }

  .checkbox-field {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }

  .checkbox-field input {
    flex: none;
    min-height: 0;
    width: auto;
  }

  .mt {
    margin-top: 16px;
  }

  .setting-group {
    margin-top: 20px;
  }

  .setting-group h3 {
    margin-bottom: 10px;
  }

  .custom-field-form {
    padding: 16px;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 7px;
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
    background: var(--fasti-surface-paper);
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 5px;
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

  .icon-only {
    padding: 6px;
    flex-shrink: 0;
  }

  .cache-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    margin: 20px 0 16px;
  }

  .nuvio-summary {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 1px;
    margin: 20px 0;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 7px;
    overflow: hidden;
  }

  .nuvio-summary div {
    padding: 12px 14px;
    background: var(--fasti-surface-paper);
  }

  .nuvio-summary dt {
    color: var(--fasti-text-muted);
    font-size: 0.78rem;
  }

  .nuvio-summary dd {
    margin: 3px 0 0;
    font-weight: 700;
  }

  .nuvio-import {
    display: grid;
    gap: 6px;
  }

  .nuvio-import label {
    font-weight: 650;
  }

  .nuvio-import small {
    color: var(--fasti-text-muted);
  }

  .nuvio-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-top: 16px;
  }

  .cache-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 7px;
    background: var(--fasti-surface-paper);
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
    margin-top: 16px;
  }

  .link-button {
    text-decoration: none;
  }

  .primary,
  .secondary,
  .danger {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border-radius: 5px;
    padding: 8px 13px;
    font-weight: 650;
    cursor: pointer;
  }

  .primary {
    border: 1px solid var(--fasti-action-primary);
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
  }

  .secondary {
    border: 1px solid var(--fasti-border, currentColor);
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .danger {
    border: 1px solid var(--fasti-state-error, #b42318);
    background: transparent;
    color: var(--fasti-state-error, #b42318);
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.62;
  }

  .managed-note {
    margin: 12px 0 0;
  }

  .notice {
    color: var(--fasti-state-verified, #207a42);
    font-weight: 600;
  }

  .problem {
    color: var(--fasti-state-error, #b42318);
    font-weight: 600;
  }

  .status-list {
    display: grid;
    gap: 1px;
    margin: 20px 0 0;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: 7px;
    overflow: hidden;
  }

  .status-list div {
    display: grid;
    grid-template-columns: minmax(180px, 1fr) minmax(220px, 1fr);
    gap: 16px;
    padding: 12px 14px;
    background: var(--fasti-surface-paper);
  }

  .status-list dt {
    font-weight: 650;
  }

  .status-list dd {
    margin: 0;
    color: var(--fasti-text-muted);
  }

  /* Nuvio Presets & Tree Explorer */
  .nuvio-preset-section {
    margin-top: 28px;
    padding-top: 24px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .preset-title {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 6px;
    color: var(--fasti-text-primary);
  }

  .preset-desc {
    font-size: 0.85rem;
    color: var(--fasti-text-muted);
    margin: 0 0 16px;
  }

  .preset-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
  }

  .preset-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 22%, transparent);
    border-radius: 6px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 12px;
  }

  .preset-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .preset-info strong {
    font-size: 0.92rem;
    color: var(--fasti-text-primary);
  }

  .preset-info span {
    font-size: 0.78rem;
    color: var(--fasti-text-muted);
    line-height: 1.3;
  }

  .btn-install-pack {
    align-self: flex-start;
    font-size: 0.82rem;
    min-height: 44px;
    min-width: 44px;
  }

  .nuvio-tree-section {
    margin-top: 28px;
    padding-top: 24px;
    border-top: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
  }

  .tree-title {
    font-size: 1rem;
    font-weight: 600;
    margin: 0 0 16px;
    color: var(--fasti-text-primary);
  }

  .collection-tree {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .tree-collection-card {
    background: var(--fasti-surface-paper);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 22%, transparent);
    border-radius: 6px;
    padding: 14px 16px;
  }

  .tree-collection-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.92rem;
    color: var(--fasti-text-primary);
  }

  .tree-badge {
    font-family: var(--fasti-font-mono);
    font-size: 0.7rem;
    text-transform: uppercase;
    padding: 2px 6px;
    background: var(--fasti-surface-archive);
    border-radius: 3px;
    color: var(--fasti-text-muted);
  }

  .tree-count {
    font-size: 0.8rem;
    color: var(--fasti-text-muted);
  }

  .tree-folders-list {
    margin-top: 10px;
    padding-left: 12px;
    border-left: 2px solid
      color-mix(in srgb, var(--fasti-text-muted) 20%, transparent);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tree-folder-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tree-folder-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.85rem;
  }

  .folder-dot {
    color: var(--fasti-action-primary);
    font-weight: bold;
  }

  .folder-title {
    font-weight: 600;
    color: var(--fasti-text-primary);
  }

  .tree-source-count {
    font-size: 0.75rem;
    color: var(--fasti-text-muted);
  }

  .tree-sources-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding-left: 14px;
  }

  .source-pill {
    font-family: var(--fasti-font-mono);
    font-size: 0.72rem;
    padding: 2px 8px;
    background: var(--fasti-surface-archive);
    border-radius: 4px;
    color: var(--fasti-text-muted);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  @media (max-width: 64rem) {
    .settings-container {
      padding: 24px 16px 48px;
    }

    .settings-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    nav {
      flex-direction: row;
      overflow-x: auto;
      padding-block-end: 4px;
      scroll-snap-type: inline proximity;
    }

    nav button {
      flex: 0 0 auto;
      white-space: nowrap;
      scroll-snap-align: start;
    }

    nav button.active {
      box-shadow: inset 0 -3px 0 var(--fasti-action-primary);
    }

    .section-heading,
    .provider-heading,
    .credential-form > div,
    .status-list div {
      grid-template-columns: 1fr;
      flex-direction: column;
    }

    .credential-form > div,
    input,
    .primary,
    .secondary,
    .danger {
      width: 100%;
    }
  }
</style>
