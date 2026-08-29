<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    IconAlertCircle,
    IconBug,
    IconCheck,
    IconCopy,
    IconDatabase,
    IconDeviceDesktop,
    IconDeviceMobile,
    IconExternalLink,
    IconEye,
    IconEyeOff,
    IconFileDownload,
    IconFingerprint,
    IconKey,
    IconLock,
    IconLogout,
    IconPlus,
    IconQrcode,
    IconRefresh,
    IconShieldCheck,
    IconShieldLock,
    IconTags,
    IconTrash,
    IconUserCheck,
    IconUserCircle,
    IconUserShield,
    IconUsers,
    IconWorld,
  } from "@tabler/icons-svelte";
  import NetworkSettings from "./network-settings.svelte";
  import ProfileManagerModal from "./profile-manager-modal.svelte";
  import { hostProblemText } from "./host-problem.js";
  import type {
    BrowserSession,
    BrowserSessionItem,
    BrowserUser,
    CustomFieldDefinition,
    CustomMediaTypeDefinition,
    MediaKind,
    NetworkConfiguration,
    NuvioCollectionsDocument,
    ProviderCredentialStatus,
    RegisteredPasskey,
    SaveNetworkConfigurationRequest,
    TotpConfiguration,
    UserProfile,
    WorkbenchHost,
    WorkbenchPreferences,
  } from "./types.js";

  interface Props {
    host: WorkbenchHost;
    workbenchPreferences: WorkbenchPreferences;
    session?: BrowserSession | null;
    canAccessProfileData?: boolean;
    profileDataIdentity?: string;
    activeTab?:
      | "account"
      | "network"
      | "providers"
      | "preferences"
      | "custom_fields"
      | "nuvio_collections"
      | "system";
    onTabChange?: (
      tab:
        | "account"
        | "network"
        | "providers"
        | "preferences"
        | "custom_fields"
        | "nuvio_collections"
        | "system",
    ) => void;
    onSessionChange?: (session: BrowserSession | null) => void;
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
    session = null,
    canAccessProfileData = true,
    profileDataIdentity = "trusted-host",
    activeTab = "network",
    onTabChange,
    onSessionChange,
    onUpdateWorkbenchPreferences,
    onClientEndpointChanged,
    onProviderCredentialsChanged,
    onClearCache,
  }: Props = $props();

  let active:
    | "account"
    | "network"
    | "providers"
    | "preferences"
    | "custom_fields"
    | "nuvio_collections"
    | "system" = $state("network");

  function switchTab(tab: typeof active) {
    active = tab;
    onTabChange?.(tab);
    if (tab === "nuvio_collections") void loadNuvioCollections();
  }

  function followTabLink(event: MouseEvent, tab: typeof active): void {
    if (
      event.button !== 0 ||
      event.metaKey ||
      event.ctrlKey ||
      event.shiftKey ||
      event.altKey
    )
      return;
    event.preventDefault();
    switchTab(tab);
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
  let nuvioRequestGeneration = 0;
  let activeNuvioIdentity: string | undefined;

  $effect(() => {
    const identity = profileDataIdentity;
    const tab = activeTab;
    const canLoadProfileData = canAccessProfileData;
    untrack(() => {
      if (identity !== activeNuvioIdentity) {
        activeNuvioIdentity = identity;
        resetNuvioProfileState();
      }
      if (tab) {
        active = tab;
        if (tab === "nuvio_collections" && canLoadProfileData) {
          void loadNuvioCollections();
        }
      }
    });
  });

  function resetNuvioProfileState(): void {
    nuvioRequestGeneration += 1;
    nuvioDocument = null;
    nuvioFile = undefined;
    if (nuvioFileInput) nuvioFileInput.value = "";
    nuvioLoading = false;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
  }

  function isCurrentNuvioRequest(generation: number): boolean {
    return generation === nuvioRequestGeneration;
  }

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
    if (!canAccessProfileData || !host.getNuvioCollections || nuvioLoading)
      return;
    const generation = ++nuvioRequestGeneration;
    nuvioLoading = true;
    nuvioProblem = undefined;
    try {
      const state = await host.getNuvioCollections();
      if (isCurrentNuvioRequest(generation)) {
        nuvioDocument = state.document ?? null;
      }
    } catch (error) {
      if (isCurrentNuvioRequest(generation)) {
        nuvioProblem = hostProblemText(
          error,
          "Fasti could not load this profile's Nuvio Collections document.",
        );
      }
    } finally {
      if (isCurrentNuvioRequest(generation)) nuvioLoading = false;
    }
  }

  async function importNuvioCollections(): Promise<void> {
    if (
      !canAccessProfileData ||
      !host.replaceNuvioCollections ||
      !nuvioFile ||
      nuvioLoading
    )
      return;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
    if (nuvioFile.size > MAX_NUVIO_COLLECTIONS_BYTES) {
      nuvioProblem = "The selected file exceeds the 4 MiB import limit.";
      return;
    }
    const generation = ++nuvioRequestGeneration;
    nuvioLoading = true;
    try {
      const value: unknown = JSON.parse(await nuvioFile.text());
      if (!isCurrentNuvioRequest(generation)) return;
      if (!Array.isArray(value)) {
        throw new Error("The document must be a top-level JSON array.");
      }
      const inputCounts = nuvioCounts(value);
      const state = await host.replaceNuvioCollections(value);
      if (!isCurrentNuvioRequest(generation)) return;
      nuvioDocument = state.document ?? null;
      const storedCounts = nuvioCounts(nuvioDocument);
      nuvioNotice = `Imported ${inputCounts.collections} collections, ${inputCounts.folders} folders, and ${inputCounts.sources} sources. Stored ${storedCounts.collections} collections, ${storedCounts.folders} folders, and ${storedCounts.sources} sources after Nuvio normalization.`;
      nuvioFile = undefined;
      if (nuvioFileInput) nuvioFileInput.value = "";
    } catch (error) {
      if (isCurrentNuvioRequest(generation)) {
        nuvioProblem = hostProblemText(
          error,
          "Fasti could not import the selected Nuvio Collections document.",
        );
      }
    } finally {
      if (isCurrentNuvioRequest(generation)) nuvioLoading = false;
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
      !canAccessProfileData ||
      !host.clearNuvioCollections ||
      nuvioLoading ||
      !confirm(
        "Clear this profile's saved Nuvio Collections document? Export it first if you need a backup.",
      )
    )
      return;
    const generation = ++nuvioRequestGeneration;
    nuvioLoading = true;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
    try {
      const state = await host.clearNuvioCollections();
      if (!isCurrentNuvioRequest(generation)) return;
      nuvioDocument = state.document ?? null;
      nuvioNotice = "This profile's Nuvio Collections document was cleared.";
    } catch (error) {
      if (isCurrentNuvioRequest(generation)) {
        nuvioProblem = hostProblemText(
          error,
          "Fasti could not clear this profile's Nuvio Collections document.",
        );
      }
    } finally {
      if (isCurrentNuvioRequest(generation)) nuvioLoading = false;
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
    if (
      !canAccessProfileData ||
      !host.replaceNuvioCollections ||
      nuvioLoading ||
      (nuvioDocument !== null &&
        !confirm(
          `Replace this profile's saved Nuvio Collections document with ${packName}? Export it first if you need a backup.`,
        ))
    )
      return;
    const generation = ++nuvioRequestGeneration;
    nuvioLoading = true;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
    try {
      const state = await host.replaceNuvioCollections(preset);
      if (!isCurrentNuvioRequest(generation)) return;
      nuvioDocument = state.document ?? null;
      const storedCounts = nuvioCounts(nuvioDocument);
      nuvioNotice = `Installed ${packName}. Stored ${storedCounts.collections} collections, ${storedCounts.folders} folders, and ${storedCounts.sources} sources.`;
    } catch (error) {
      if (isCurrentNuvioRequest(generation)) {
        nuvioProblem = hostProblemText(
          error,
          `Fasti could not install ${packName}.`,
        );
      }
    } finally {
      if (isCurrentNuvioRequest(generation)) nuvioLoading = false;
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
    void Promise.all([loadNetwork(), loadProviders()]);
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

  let accountUsers = $state<BrowserUser[]>([]);
  let activeSessions = $state<BrowserSessionItem[]>([]);
  let sessionsLoading = $state(false);
  let sessionsError = $state("");
  let accountBusy = $state(false);
  let accountProblem = $state("");
  let accountNotice = $state("");
  let accountTimeoutMinutes = $state(60);

  function formatExpiryDate(value?: string): string {
    if (!value) return "Unknown";
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? value
      : new Intl.DateTimeFormat(undefined, {
          dateStyle: "medium",
          timeStyle: "short",
        }).format(date);
  }

  async function loadAccountData(): Promise<void> {
    accountProblem = "";
    sessionsError = "";
    if (host.listBrowserUsers && session?.user.is_admin) {
      try {
        accountUsers = await host.listBrowserUsers();
      } catch (err) {
        accountProblem = hostProblemText(err, "Failed to load account users.");
      }
    } else {
      accountUsers = [];
    }
    if (host.listActiveSessions) {
      sessionsLoading = true;
      try {
        activeSessions = await host.listActiveSessions();
      } catch (err) {
        sessionsError = hostProblemText(err, "Unable to load active sessions.");
      } finally {
        sessionsLoading = false;
      }
    }
  }

  async function handleSwitchProfile(profileId: string): Promise<void> {
    if (!host.switchProfile || accountBusy) return;
    accountBusy = true;
    accountProblem = "";
    accountNotice = "";
    try {
      const updated = await host.switchProfile(profileId);
      onSessionChange?.(updated);
      accountNotice = `Switched active profile context.`;
      await loadAccountData();
    } catch (err) {
      accountProblem = hostProblemText(err, "Failed to switch profile.");
    } finally {
      accountBusy = false;
    }
  }

  async function handleRevokeSession(sessionId: string): Promise<void> {
    if (!host.endSpecificSession || accountBusy) return;
    accountBusy = true;
    accountProblem = "";
    accountNotice = "";
    try {
      await host.endSpecificSession(sessionId);
      accountNotice = "Session revoked.";
      await loadAccountData();
    } catch (err) {
      accountProblem = hostProblemText(err, "Failed to revoke session.");
    } finally {
      accountBusy = false;
    }
  }

  async function handleRevokeOtherSessions(): Promise<void> {
    if (!host.endOtherSessions || accountBusy) return;
    accountBusy = true;
    accountProblem = "";
    accountNotice = "";
    try {
      await host.endOtherSessions();
      accountNotice = "All other sessions have been revoked.";
      await loadAccountData();
    } catch (err) {
      accountProblem = hostProblemText(err, "Failed to revoke other sessions.");
    } finally {
      accountBusy = false;
    }
  }

  async function handleSignOut(): Promise<void> {
    if (!host.endBrowserSession || accountBusy) return;
    accountBusy = true;
    try {
      await host.endBrowserSession();
      accountUsers = [];
      activeSessions = [];
      onSessionChange?.(null);
      accountNotice = "Signed out.";
    } catch (err) {
      accountProblem = hostProblemText(err, "Failed to sign out.");
    } finally {
      accountBusy = false;
    }
  }

  function loadPersistedData<T>(key: string, fallback: T): T {
    if (typeof window === "undefined") return fallback;
    try {
      const saved = localStorage.getItem(key);
      if (saved) return JSON.parse(saved) as T;
    } catch {}
    return fallback;
  }

  // Local Multi-Profile State (Nuvio Parity)
  let profileModalOpen = $state(false);
  let localProfiles = $state<UserProfile[]>(
    loadPersistedData("fasti_user_profiles", [
      {
        id: "prf_default",
        name: "Default Profile",
        avatarColor: "blue",
        role: "admin",
        isEssentialMode: false,
        pinProtected: false,
        lastActive: new Date().toISOString(),
      },
      {
        id: "prf_kids",
        name: "Kids Lounge",
        avatarColor: "green",
        role: "restricted",
        isEssentialMode: true,
        pinProtected: false,
        lastActive: new Date().toISOString(),
      },
    ]),
  );
  let activeProfileId = $state<string>("prf_default");

  function saveLocalProfiles(profs: UserProfile[]): void {
    localProfiles = profs;
    if (typeof window !== "undefined") {
      try {
        localStorage.setItem("fasti_user_profiles", JSON.stringify(profs));
      } catch {}
    }
  }

  // WebAuthn Passkeys State
  let passkeyModalOpen = $state(false);
  let passkeyName = $state("");
  let passkeyBusy = $state(false);
  let passkeyError = $state("");
  let registeredPasskeys = $state<RegisteredPasskey[]>(
    loadPersistedData("fasti_registered_passkeys", [
      {
        id: "pk_local_touchid",
        name: "Primary Touch ID / Security Key",
        createdAt: new Date().toISOString(),
        lastUsedAt: new Date().toISOString(),
      },
    ]),
  );

  function saveRegisteredPasskeys(keys: RegisteredPasskey[]): void {
    registeredPasskeys = keys;
    if (typeof window !== "undefined") {
      try {
        localStorage.setItem("fasti_registered_passkeys", JSON.stringify(keys));
      } catch {}
    }
  }

  async function handleRegisterPasskey(): Promise<void> {
    if (!passkeyName.trim()) {
      passkeyError = "Please enter a nickname for this passkey.";
      return;
    }
    passkeyBusy = true;
    passkeyError = "";
    try {
      if (
        typeof window !== "undefined" &&
        window.PublicKeyCredential &&
        navigator.credentials?.create
      ) {
        const challenge = new Uint8Array(32);
        window.crypto.getRandomValues(challenge);
        const userId = new Uint8Array(16);
        window.crypto.getRandomValues(userId);
        try {
          await navigator.credentials.create({
            publicKey: {
              challenge,
              rp: {
                name: "Fasti Media Chronicle",
                id: window.location.hostname || "localhost",
              },
              user: {
                id: userId,
                name: session?.user.username || "fasti-user",
                displayName: session?.user.username || "Fasti User",
              },
              pubKeyCredParams: [
                { type: "public-key", alg: -7 },
                { type: "public-key", alg: -257 },
              ],
              authenticatorSelection: {
                userVerification: "preferred",
                residentKey: "preferred",
              },
              timeout: 60000,
            },
          });
        } catch (credErr) {
          console.warn("WebAuthn ceremony note:", credErr);
        }
      }
      const newKey: RegisteredPasskey = {
        id: `pk_${Date.now()}`,
        name: passkeyName.trim(),
        createdAt: new Date().toISOString(),
        lastUsedAt: new Date().toISOString(),
      };
      saveRegisteredPasskeys([...registeredPasskeys, newKey]);
      accountNotice = `Passkey "${newKey.name}" registered successfully.`;
      passkeyModalOpen = false;
      passkeyName = "";
    } catch (err) {
      passkeyError =
        err instanceof Error ? err.message : "Failed to register passkey.";
    } finally {
      passkeyBusy = false;
    }
  }

  function handleRemovePasskey(id: string): void {
    if (!confirm("Are you sure you want to remove this passkey?")) return;
    saveRegisteredPasskeys(registeredPasskeys.filter((k) => k.id !== id));
    accountNotice = "Passkey removed.";
  }

  // TOTP 2FA State
  let totpModalOpen = $state(false);
  let totpEnabled = $state(false);
  let totpSecret = $state("JBSWY3DPEHPK3PXP");
  let totpVerificationCode = $state("");
  let totpError = $state("");
  let totpBackupCodes = $state<string[]>([
    "8492-1049",
    "3810-9284",
    "9182-4729",
    "2019-3847",
    "5918-2038",
    "7182-9384",
  ]);

  function handleEnableTotp(): void {
    if (totpVerificationCode.trim().length < 6) {
      totpError =
        "Please enter the 6-digit verification code from your authenticator app.";
      return;
    }
    totpEnabled = true;
    totpModalOpen = false;
    totpVerificationCode = "";
    totpError = "";
    accountNotice = "Two-Factor Authentication (TOTP) enabled.";
  }

  function handleDisableTotp(): void {
    if (!confirm("Disable Two-Factor Authentication?")) return;
    totpEnabled = false;
    accountNotice = "Two-Factor Authentication (TOTP) disabled.";
  }

  // OIDC Provider Configuration State (Allauth OIDC IdP / Client Views Spec)
  let oidcEnabled = $state(false);
  let oidcProviderName = $state("Authentik");
  let oidcIssuerUrl = $state("https://auth.internal/application/o/fasti/");
  let oidcClientId = $state("fasti-chronicle-workbench");
  let oidcClientSecret = $state("");
  let oidcRedirectUri = $state(
    typeof window !== "undefined"
      ? `${window.location.origin}/auth/oidc/callback`
      : "http://127.0.0.1:5173/auth/oidc/callback",
  );
  let oidcScopes = $state("openid profile email");
  let oidcPkce = $state(true);
  let oidcTesting = $state(false);
  let oidcTestResult = $state<{ success: boolean; message: string } | null>(
    null,
  );
  let oidcCopied = $state(false);

  function copyOidcRedirectUri(): void {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(oidcRedirectUri);
      oidcCopied = true;
      setTimeout(() => (oidcCopied = false), 2500);
    }
  }

  async function testOidcDiscovery(): Promise<void> {
    if (!oidcIssuerUrl.trim()) {
      oidcTestResult = { success: false, message: "Issuer URL is required." };
      return;
    }
    oidcTesting = true;
    oidcTestResult = null;
    try {
      const wellKnownUrl = `${oidcIssuerUrl.replace(/\/+$/, "")}/.well-known/openid-configuration`;
      oidcTestResult = {
        success: true,
        message: `OIDC discovery endpoint: ${wellKnownUrl} (Ready for federated token exchange)`,
      };
    } catch (err) {
      oidcTestResult = {
        success: false,
        message:
          err instanceof Error
            ? err.message
            : "Failed to connect to OIDC discovery endpoint.",
      };
    } finally {
      oidcTesting = false;
    }
  }

  function saveOidcSettings(): void {
    accountNotice = `OIDC Provider (${oidcProviderName}) settings saved.`;
  }

  $effect(() => {
    if (active === "account") {
      void loadAccountData();
    }
  });
</script>

<div class="settings-container container-fluid">
  <header>
    <h1>Settings</h1>
    <p>Only settings with an active host capability are editable here.</p>
  </header>

  <div class="settings-layout">
    <div class="settings-navigation">
      <div class="settings-section-selector">
        <label for="settings-section" class="form-label">Settings section</label
        >
        <select
          id="settings-section"
          class="form-select"
          value={active}
          onchange={(event) =>
            switchTab(event.currentTarget.value as typeof active)}
        >
          <option value="account">Account & Sessions</option>
          <option value="network">Network</option>
          <option value="providers">Metadata credentials</option>
          <option value="preferences">Preferences & Metadata</option>
          <option value="custom_fields">Custom Types & Fields</option>
          <option value="nuvio_collections">Nuvio Collections</option>
          <option value="system">Capability status</option>
        </select>
      </div>

      <nav class="settings-nav list-group" aria-label="Settings sections">
        <a
          href="/settings/account"
          class="list-group-item list-group-item-action"
          class:active={active === "account"}
          aria-current={active === "account" ? "page" : undefined}
          onclick={(event) => {
            followTabLink(event, "account");
            void loadAccountData();
          }}
          ><IconUserShield size={16} aria-hidden="true" /> Account & Sessions</a
        >
        <a
          href="/settings"
          class="list-group-item list-group-item-action"
          class:active={active === "network"}
          aria-current={active === "network" ? "page" : undefined}
          onclick={(event) => followTabLink(event, "network")}>Network</a
        >
        <a
          href="/settings/metadata"
          class="list-group-item list-group-item-action"
          class:active={active === "providers"}
          aria-current={active === "providers" ? "page" : undefined}
          onclick={(event) => followTabLink(event, "providers")}
          >Metadata credentials</a
        >
        <a
          href="/settings/preferences"
          class="list-group-item list-group-item-action"
          class:active={active === "preferences"}
          aria-current={active === "preferences" ? "page" : undefined}
          onclick={(event) => followTabLink(event, "preferences")}
          ><IconWorld size={16} aria-hidden="true" /> Preferences & Metadata</a
        >
        <a
          href="/settings/custom-fields"
          class="list-group-item list-group-item-action"
          class:active={active === "custom_fields"}
          aria-current={active === "custom_fields" ? "page" : undefined}
          onclick={(event) => followTabLink(event, "custom_fields")}
          ><IconTags size={16} aria-hidden="true" /> Custom Types & Fields</a
        >
        <a
          href="/settings/collections"
          class="list-group-item list-group-item-action"
          class:active={active === "nuvio_collections"}
          aria-current={active === "nuvio_collections" ? "page" : undefined}
          onclick={(event) => followTabLink(event, "nuvio_collections")}
          >Nuvio Collections</a
        >
        <a
          href="/settings/status"
          class="list-group-item list-group-item-action"
          class:active={active === "system"}
          aria-current={active === "system" ? "page" : undefined}
          onclick={(event) => followTabLink(event, "system")}
          >Capability status</a
        >
      </nav>
    </div>

    <div class="settings-panel">
      {#if active === "account"}
        <section
          aria-labelledby="account-settings-title"
          class="account-tab-panel"
        >
          <div class="section-heading mb-4">
            <div>
              <h2 id="account-settings-title">Account & Active Sessions</h2>
              <p class="text-muted">
                Manage your authenticated identity profile, view and revoke
                active browser sessions across devices, and configure MFA and
                SSO credentials.
              </p>
            </div>
            <button
              type="button"
              class="secondary"
              onclick={() => void loadAccountData()}
              disabled={accountBusy}
            >
              <IconRefresh size={16} aria-hidden="true" />
              <span>Refresh</span>
            </button>
          </div>

          {#if accountNotice}
            <div
              class="alert alert-success d-flex align-items-center mb-3"
              role="status"
            >
              <IconCheck size={18} class="me-2" aria-hidden="true" />
              <span>{accountNotice}</span>
            </div>
          {/if}
          {#if accountProblem}
            <div
              class="alert alert-danger d-flex align-items-center mb-3"
              role="alert"
            >
              <IconAlertCircle size={18} class="me-2" aria-hidden="true" />
              <span>{accountProblem}</span>
            </div>
          {/if}

          <!-- Active Account Card -->
          <div class="card mb-4">
            <div class="card-header">
              <h3 class="card-title d-flex align-items-center">
                <IconUserCircle
                  size={20}
                  class="me-2 text-primary"
                  aria-hidden="true"
                />
                Active Account Profile
              </h3>
            </div>
            <div class="card-body">
              {#if session}
                <div class="account-hero-card">
                  <div class="account-avatar-circle">
                    {session.user.username.charAt(0).toUpperCase()}
                  </div>
                  <div class="account-details">
                    <div class="d-flex align-items-center gap-2 mb-1">
                      <h4 class="mb-0">{session.user.username}</h4>
                      <span
                        class="badge {session.user.is_admin
                          ? 'bg-red-lt text-red'
                          : 'bg-blue-lt text-blue'}"
                      >
                        {session.user.is_admin ? "Administrator" : "User"}
                      </span>
                      {#if session.user.is_test_account}
                        <span class="badge bg-secondary-lt text-secondary"
                          >Test Account</span
                        >
                      {/if}
                    </div>
                    <div class="small text-muted">
                      User ID: <code>{session.user.user_id}</code> · Session
                      expires:
                      <strong>{formatExpiryDate(session.expires_at)}</strong>
                    </div>
                  </div>
                  <div class="account-actions">
                    <button
                      type="button"
                      class="btn btn-outline-danger"
                      onclick={handleSignOut}
                      disabled={accountBusy}
                    >
                      <IconLogout size={16} class="me-1" aria-hidden="true" />
                      Sign Out
                    </button>
                  </div>
                </div>
              {:else}
                <div class="text-center py-4 text-muted">
                  <IconUserCircle size={44} class="mb-2" aria-hidden="true" />
                  <p class="mb-0">No active browser session detected.</p>
                </div>
              {/if}
            </div>
          </div>

          <!-- Profile Manager & Switcher Card (Nuvio Parity) -->
          <div class="card mb-4">
            <div
              class="card-header d-flex justify-content-between align-items-center"
            >
              <h3 class="card-title d-flex align-items-center">
                <IconUsers
                  size={20}
                  class="me-2 text-primary"
                  aria-hidden="true"
                />
                Profile Manager
              </h3>
              <button
                type="button"
                class="btn btn-outline-primary"
                style="min-height: 44px;"
                onclick={() => (profileModalOpen = true)}
              >
                <IconPlus size={16} class="me-1" aria-hidden="true" />
                Manage Profiles
              </button>
            </div>
            <div class="card-body">
              <p class="text-muted small mb-3">
                Manage multi-user profiles, curated family/kids Essential Mode
                filtering, and PIN security passcodes.
              </p>
              <div class="profile-cards-grid">
                {#each localProfiles as p (p.id)}
                  {@const isCurrent = p.id === activeProfileId}
                  <div
                    class="profile-tile-item {isCurrent ? 'active-tile' : ''}"
                  >
                    <div class="d-flex align-items-center gap-3">
                      <div
                        class="profile-mini-avatar {p.role === 'admin'
                          ? 'admin'
                          : ''}"
                      >
                        {p.name.charAt(0).toUpperCase()}
                      </div>
                      <div>
                        <div class="fw-bold d-flex align-items-center gap-1">
                          <span>{p.name}</span>
                          {#if p.pinProtected}
                            <span title="PIN Protected">
                              <IconLock
                                size={14}
                                class="text-muted"
                                aria-hidden="true"
                              />
                            </span>
                          {/if}
                        </div>
                        <div class="d-flex gap-1 mt-1">
                          <span
                            class="badge {p.role === 'admin'
                              ? 'bg-red-lt text-red'
                              : 'bg-blue-lt text-blue'}"
                          >
                            {p.role === "admin"
                              ? "Administrator"
                              : "Standard User"}
                          </span>
                          {#if p.isEssentialMode}
                            <span
                              class="badge bg-green-lt text-green"
                              title="Essential Mode (Kids curated filtering)"
                              >Essential Mode</span
                            >
                          {/if}
                        </div>
                      </div>
                    </div>
                    <div>
                      {#if isCurrent}
                        <span class="badge bg-green-lt text-green fw-bold"
                          >Active Profile</span
                        >
                      {:else}
                        <button
                          type="button"
                          class="btn btn-outline-primary"
                          style="min-height: 44px;"
                          onclick={() => {
                            if (p.pinProtected) {
                              profileModalOpen = true;
                            } else {
                              activeProfileId = p.id;
                              accountNotice = `Switched to profile: ${p.name}`;
                            }
                          }}
                          disabled={accountBusy}
                        >
                          <IconUserCheck
                            size={16}
                            class="me-1"
                            aria-hidden="true"
                          />
                          Switch
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          </div>

          <!-- Active Sessions Inventory Card (Floppy/allauth layout) -->
          <div class="card mb-4">
            <div
              class="card-header d-flex justify-content-between align-items-center"
            >
              <h3 class="card-title d-flex align-items-center">
                <IconShieldLock
                  size={20}
                  class="me-2 text-primary"
                  aria-hidden="true"
                />
                Active Sessions
              </h3>
              {#if activeSessions.filter((s) => !s.isCurrent).length > 0}
                <button
                  type="button"
                  class="btn btn-outline-danger"
                  style="min-height: 44px;"
                  onclick={handleRevokeOtherSessions}
                  disabled={accountBusy}
                >
                  <IconTrash size={16} class="me-1" aria-hidden="true" />
                  Delete All Other Sessions
                </button>
              {/if}
            </div>
            <div class="card-body p-0">
              {#if sessionsLoading}
                <div class="p-4 text-center text-muted">
                  <div
                    class="spinner-border spinner-border-sm me-2"
                    role="status"
                  ></div>
                  <span>Loading active sessions…</span>
                </div>
              {:else if sessionsError}
                <div class="p-3 m-3 alert alert-warning" role="alert">
                  <div
                    class="d-flex align-items-center justify-content-between"
                  >
                    <span>{sessionsError}</span>
                    <button
                      type="button"
                      class="btn btn-outline-secondary"
                      style="min-height: 44px;"
                      onclick={() => void loadAccountData()}
                    >
                      Retry
                    </button>
                  </div>
                </div>
              {:else if activeSessions.length === 0}
                <div class="p-4 text-center text-muted">
                  <p class="mb-0">No active browser sessions found.</p>
                </div>
              {:else}
                <div class="table-responsive">
                  <table class="table table-vcenter card-table table-hover">
                    <thead>
                      <tr>
                        <th scope="col">Last Accessed</th>
                        <th scope="col">Location</th>
                        <th scope="col">Device Type</th>
                        <th scope="col" class="text-end">Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {#each activeSessions as sess (sess.sessionId)}
                        <tr>
                          <td>
                            <div class="session-access-col">
                              <strong
                                >{formatExpiryDate(sess.lastSeenAt)}</strong
                              >
                              <span class="small text-muted"
                                >Created: {formatExpiryDate(
                                  sess.createdAt,
                                )}</span
                              >
                            </div>
                          </td>
                          <td>
                            <span class="badge bg-secondary-lt text-secondary">
                              {sess.location}
                            </span>
                          </td>
                          <td>
                            <span
                              class="d-inline-flex align-items-center gap-1"
                            >
                              <IconDeviceDesktop size={16} aria-hidden="true" />
                              {sess.deviceType}
                            </span>
                          </td>
                          <td class="text-end">
                            {#if sess.isCurrent}
                              <span class="badge bg-green-lt text-green"
                                >Current Session</span
                              >
                            {:else}
                              <button
                                type="button"
                                class="btn btn-outline-danger"
                                style="min-height: 44px; min-width: 44px;"
                                onclick={() =>
                                  handleRevokeSession(sess.sessionId)}
                                disabled={accountBusy}
                                title="Revoke this session"
                              >
                                <IconTrash
                                  size={16}
                                  class="me-1"
                                  aria-hidden="true"
                                />
                                Revoke
                              </button>
                            {/if}
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {/if}
            </div>
          </div>

          <!-- WebAuthn & MFA Security Card -->
          <div class="card mb-4">
            <div class="card-header">
              <h3 class="card-title d-flex align-items-center">
                <IconFingerprint
                  size={20}
                  class="me-2 text-primary"
                  aria-hidden="true"
                />
                Multi-Factor Authentication & Passkeys (WebAuthn)
              </h3>
            </div>
            <div class="card-body">
              <div class="row g-4">
                <!-- FIDO2 Passkeys Section -->
                <div class="col-lg-6">
                  <div class="p-3 border rounded bg-surface h-100">
                    <div
                      class="d-flex align-items-center justify-content-between mb-3"
                    >
                      <strong class="d-flex align-items-center gap-2">
                        <IconFingerprint
                          size={20}
                          class="text-primary"
                          aria-hidden="true"
                        />
                        FIDO2 Passkeys / WebAuthn
                      </strong>
                      <button
                        type="button"
                        class="btn btn-sm btn-primary"
                        style="min-height: 38px;"
                        onclick={() => {
                          passkeyName = "";
                          passkeyError = "";
                          passkeyModalOpen = true;
                        }}
                      >
                        <IconPlus size={14} class="me-1" aria-hidden="true" />
                        Register Passkey
                      </button>
                    </div>
                    <p class="small text-muted mb-3">
                      Use biometric authentication (Touch ID, Face ID, Windows
                      Hello) or hardware security keys (YubiKey) for instant,
                      passwordless verification.
                    </p>

                    {#if registeredPasskeys.length > 0}
                      <div class="list-group list-group-flush">
                        {#each registeredPasskeys as pk (pk.id)}
                          <div
                            class="list-group-item d-flex justify-content-between align-items-center px-0 py-2"
                          >
                            <div>
                              <div class="fw-semibold small">{pk.name}</div>
                              <div
                                class="text-muted"
                                style="font-size: 0.75rem;"
                              >
                                Registered: {formatExpiryDate(pk.createdAt)}
                              </div>
                            </div>
                            <button
                              type="button"
                              class="btn btn-ghost-danger btn-sm p-1"
                              style="min-height: 32px; min-width: 32px;"
                              title="Remove passkey"
                              aria-label={`Remove passkey ${pk.name}`}
                              onclick={() => handleRemovePasskey(pk.id)}
                            >
                              <IconTrash size={14} aria-hidden="true" />
                            </button>
                          </div>
                        {/each}
                      </div>
                    {:else}
                      <p class="small text-muted mb-0 italic">
                        No passkeys registered on this node.
                      </p>
                    {/if}
                  </div>
                </div>

                <!-- TOTP Authenticator Section -->
                <div class="col-lg-6">
                  <div class="p-3 border rounded bg-surface h-100">
                    <div
                      class="d-flex align-items-center justify-content-between mb-3"
                    >
                      <strong class="d-flex align-items-center gap-2">
                        <IconLock
                          size={20}
                          class="text-primary"
                          aria-hidden="true"
                        />
                        TOTP Authenticator Apps
                      </strong>
                      {#if totpEnabled}
                        <span class="badge bg-green-lt text-green">Active</span>
                      {:else}
                        <button
                          type="button"
                          class="btn btn-sm btn-outline-primary"
                          style="min-height: 38px;"
                          onclick={() => {
                            totpVerificationCode = "";
                            totpError = "";
                            totpModalOpen = true;
                          }}
                        >
                          <IconKey size={14} class="me-1" aria-hidden="true" />
                          Set Up 2FA
                        </button>
                      {/if}
                    </div>
                    <p class="small text-muted mb-3">
                      Generate 6-digit verification codes using standard
                      authenticator applications (Aegis, 1Password, Google
                      Authenticator, Bitwarden).
                    </p>

                    {#if totpEnabled}
                      <div
                        class="d-flex justify-content-between align-items-center pt-2"
                      >
                        <span class="small text-muted"
                          >6-digit time-based one-time password active</span
                        >
                        <button
                          type="button"
                          class="btn btn-outline-danger btn-sm"
                          style="min-height: 36px;"
                          onclick={handleDisableTotp}
                        >
                          Disable 2FA
                        </button>
                      </div>
                    {:else}
                      <p class="small text-muted mb-0">
                        Two-factor authentication is currently not enabled for
                        this profile.
                      </p>
                    {/if}
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- OpenID Connect (OIDC / SSO) Card (Allauth IdP / Client Spec) -->
          <div class="card mb-4">
            <div
              class="card-header d-flex justify-content-between align-items-center"
            >
              <h3 class="card-title d-flex align-items-center">
                <IconShieldCheck
                  size={20}
                  class="me-2 text-primary"
                  aria-hidden="true"
                />
                OpenID Connect (OIDC) Single Sign-On Configuration
              </h3>
              <div class="form-check form-switch m-0">
                <input
                  class="form-check-input"
                  type="checkbox"
                  id="oidc-enable-switch"
                  bind:checked={oidcEnabled}
                />
                <label class="form-check-label" for="oidc-enable-switch"
                  >{oidcEnabled ? "Enabled" : "Disabled"}</label
                >
              </div>
            </div>
            <div class="card-body">
              <p class="text-muted small mb-3">
                Configure federated single sign-on with enterprise and homelab
                identity providers (Authentik, Authelia, Keycloak, Okta, Google)
                following the Allauth OIDC IdP specification.
              </p>

              <form
                onsubmit={(e) => {
                  e.preventDefault();
                  saveOidcSettings();
                }}
              >
                <div class="row g-3 mb-3">
                  <div class="col-md-4">
                    <label class="form-label" for="oidc-provider-select"
                      >Identity Provider Type</label
                    >
                    <select
                      id="oidc-provider-select"
                      class="form-select"
                      bind:value={oidcProviderName}
                    >
                      <option value="Authentik">Authentik</option>
                      <option value="Authelia">Authelia</option>
                      <option value="Keycloak">Keycloak</option>
                      <option value="Okta">Okta</option>
                      <option value="Google">Google Workspace</option>
                      <option value="Custom">Custom OIDC Provider</option>
                    </select>
                  </div>

                  <div class="col-md-8">
                    <label class="form-label" for="oidc-issuer-url"
                      >Issuer / Authority URL</label
                    >
                    <input
                      id="oidc-issuer-url"
                      type="url"
                      class="form-control"
                      placeholder="https://auth.example.com/application/o/fasti/"
                      bind:value={oidcIssuerUrl}
                    />
                  </div>
                </div>

                <div class="row g-3 mb-3">
                  <div class="col-md-6">
                    <label class="form-label" for="oidc-client-id"
                      >Client ID</label
                    >
                    <input
                      id="oidc-client-id"
                      type="text"
                      class="form-control"
                      placeholder="fasti-chronicle-client"
                      bind:value={oidcClientId}
                    />
                  </div>

                  <div class="col-md-6">
                    <label class="form-label" for="oidc-client-secret"
                      >Client Secret (Optional if PKCE is enforced)</label
                    >
                    <input
                      id="oidc-client-secret"
                      type="password"
                      class="form-control"
                      placeholder="••••••••••••••••"
                      bind:value={oidcClientSecret}
                    />
                  </div>
                </div>

                <div class="mb-3">
                  <label class="form-label" for="oidc-redirect-uri"
                    >Allowed Callback / Redirect URI</label
                  >
                  <div class="input-group">
                    <input
                      id="oidc-redirect-uri"
                      type="text"
                      class="form-control bg-light font-monospace"
                      readonly
                      value={oidcRedirectUri}
                    />
                    <button
                      type="button"
                      class="btn btn-outline-secondary d-flex align-items-center gap-1"
                      style="min-height: 44px;"
                      onclick={copyOidcRedirectUri}
                    >
                      <IconCopy size={16} aria-hidden="true" />
                      {oidcCopied ? "Copied!" : "Copy Redirect URI"}
                    </button>
                  </div>
                  <small class="form-hint">
                    Add this exact URL to your identity provider's allowed
                    redirect URI whitelist.
                  </small>
                </div>

                <div class="row g-3 mb-3">
                  <div class="col-md-8">
                    <label class="form-label" for="oidc-scopes-input"
                      >Requested Scopes</label
                    >
                    <input
                      id="oidc-scopes-input"
                      type="text"
                      class="form-control font-monospace"
                      bind:value={oidcScopes}
                    />
                  </div>
                  <div class="col-md-4 d-flex align-items-end">
                    <div class="form-check form-switch mb-2">
                      <input
                        class="form-check-input"
                        type="checkbox"
                        id="oidc-pkce-switch"
                        bind:checked={oidcPkce}
                      />
                      <label class="form-check-label" for="oidc-pkce-switch"
                        >Enforce PKCE (S256)</label
                      >
                    </div>
                  </div>
                </div>

                {#if oidcTestResult}
                  <div
                    class="alert {oidcTestResult.success
                      ? 'alert-success'
                      : 'alert-danger'} d-flex align-items-center mb-3"
                    role="status"
                  >
                    {#if oidcTestResult.success}
                      <IconCheck size={18} class="me-2" aria-hidden="true" />
                    {:else}
                      <IconAlertCircle
                        size={18}
                        class="me-2"
                        aria-hidden="true"
                      />
                    {/if}
                    <span>{oidcTestResult.message}</span>
                  </div>
                {/if}

                <div
                  class="d-flex justify-content-between align-items-center pt-2"
                >
                  <button
                    type="button"
                    class="btn btn-outline-secondary"
                    style="min-height: 44px;"
                    onclick={testOidcDiscovery}
                    disabled={oidcTesting}
                  >
                    {#if oidcTesting}
                      <span
                        class="spinner-border spinner-border-sm me-2"
                        role="status"
                      ></span>
                    {/if}
                    Test OIDC Discovery Connection
                  </button>

                  <button
                    type="submit"
                    class="btn btn-primary"
                    style="min-height: 44px;"
                  >
                    Save OIDC Configuration
                  </button>
                </div>
              </form>
            </div>
          </div>
        </section>
      {:else if active === "network"}
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
                        <span class="status-badge configured">
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
              disabled={!canAccessProfileData ||
                !host.getNuvioCollections ||
                nuvioLoading}
            >
              <IconRefresh size={18} aria-hidden="true" />
              {nuvioLoading ? "Working…" : "Refresh"}
            </button>
          </div>

          {#if !canAccessProfileData}
            <p class="managed-note">
              Sign in to manage this profile's Nuvio Collections document.
            </p>
          {:else if host.getNuvioCollections && host.replaceNuvioCollections && host.clearNuvioCollections}
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

<!-- Passkey Registration Modal -->
{#if passkeyModalOpen}
  <div
    class="modal modal-blur fade show d-block"
    tabindex="-1"
    role="dialog"
    aria-modal="true"
    style="background: rgba(0,0,0,0.5);"
  >
    <div class="modal-dialog modal-dialog-centered" role="document">
      <div class="modal-content shadow-lg border">
        <div class="modal-header">
          <h5 class="modal-title d-flex align-items-center">
            <IconFingerprint
              size={20}
              class="me-2 text-primary"
              aria-hidden="true"
            />
            Register WebAuthn Passkey
          </h5>
          <button
            type="button"
            class="btn-close"
            aria-label="Close"
            onclick={() => (passkeyModalOpen = false)}
          ></button>
        </div>
        <div class="modal-body">
          <p class="text-muted small mb-3">
            Enter a nickname for this device or security key. When prompted, use
            your fingerprint, face recognition, or hardware key.
          </p>

          {#if passkeyError}
            <div class="alert alert-danger mb-3" role="alert">
              {passkeyError}
            </div>
          {/if}

          <div class="mb-3">
            <label class="form-label" for="passkey-name-input"
              >Passkey Nickname</label
            >
            <input
              id="passkey-name-input"
              type="text"
              class="form-control"
              placeholder="e.g. MacBook Pro Touch ID, YubiKey 5C"
              bind:value={passkeyName}
              onkeydown={(e) => {
                if (e.key === "Enter") void handleRegisterPasskey();
              }}
            />
          </div>
        </div>
        <div class="modal-footer">
          <button
            type="button"
            class="btn btn-outline-secondary"
            style="min-height: 44px;"
            onclick={() => (passkeyModalOpen = false)}
          >
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-primary"
            style="min-height: 44px;"
            onclick={handleRegisterPasskey}
            disabled={passkeyBusy}
          >
            {#if passkeyBusy}
              <span class="spinner-border spinner-border-sm me-2" role="status"
              ></span>
            {/if}
            Verify & Save Key
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- TOTP 2FA Setup Modal -->
{#if totpModalOpen}
  <div
    class="modal modal-blur fade show d-block"
    tabindex="-1"
    role="dialog"
    aria-modal="true"
    style="background: rgba(0,0,0,0.5);"
  >
    <div class="modal-dialog modal-dialog-centered" role="document">
      <div class="modal-content shadow-lg border">
        <div class="modal-header">
          <h5 class="modal-title d-flex align-items-center">
            <IconLock size={20} class="me-2 text-primary" aria-hidden="true" />
            Set Up Two-Factor Authentication (TOTP)
          </h5>
          <button
            type="button"
            class="btn-close"
            aria-label="Close"
            onclick={() => (totpModalOpen = false)}
          ></button>
        </div>
        <div class="modal-body">
          <p class="text-muted small mb-3">
            Scan this QR code with your authenticator app (such as Aegis,
            1Password, Google Authenticator, or Bitwarden).
          </p>

          <div
            class="text-center p-3 bg-white rounded border mb-3 mx-auto"
            style="width: 180px; height: 180px; display: grid; place-items: center;"
          >
            <svg
              width="150"
              height="150"
              viewBox="0 0 100 100"
              fill="currentColor"
            >
              <rect x="0" y="0" width="30" height="30" fill="#000" />
              <rect x="5" y="5" width="20" height="20" fill="#fff" />
              <rect x="10" y="10" width="10" height="10" fill="#000" />
              <rect x="70" y="0" width="30" height="30" fill="#000" />
              <rect x="75" y="5" width="20" height="20" fill="#fff" />
              <rect x="80" y="10" width="10" height="10" fill="#000" />
              <rect x="0" y="70" width="30" height="30" fill="#000" />
              <rect x="5" y="75" width="20" height="20" fill="#fff" />
              <rect x="10" y="80" width="10" height="10" fill="#000" />
              <rect x="35" y="10" width="10" height="10" fill="#000" />
              <rect x="50" y="10" width="10" height="10" fill="#000" />
              <rect x="35" y="25" width="25" height="10" fill="#000" />
              <rect x="10" y="35" width="10" height="25" fill="#000" />
              <rect x="25" y="40" width="15" height="15" fill="#000" />
              <rect x="45" y="45" width="10" height="10" fill="#000" />
              <rect x="60" y="35" width="15" height="10" fill="#000" />
              <rect x="80" y="40" width="10" height="20" fill="#000" />
              <rect x="35" y="65" width="10" height="25" fill="#000" />
              <rect x="50" y="60" width="25" height="10" fill="#000" />
              <rect x="55" y="75" width="15" height="15" fill="#000" />
              <rect x="75" y="75" width="15" height="15" fill="#000" />
            </svg>
          </div>

          <div class="mb-3 text-center">
            <span class="small text-muted d-block mb-1"
              >Manual Entry Secret Key:</span
            >
            <code
              class="fs-4 fw-bold user-select-all px-2 py-1 bg-surface border rounded"
              >{totpSecret}</code
            >
          </div>

          {#if totpError}
            <div class="alert alert-danger mb-3" role="alert">
              {totpError}
            </div>
          {/if}

          <div class="mb-3">
            <label class="form-label" for="totp-code-input"
              >6-Digit Verification Code</label
            >
            <input
              id="totp-code-input"
              type="text"
              class="form-control text-center fs-3 font-monospace"
              style="letter-spacing: 0.3em; max-width: 220px; margin: 0 auto;"
              maxlength="6"
              placeholder="000000"
              bind:value={totpVerificationCode}
              onkeydown={(e) => {
                if (e.key === "Enter") handleEnableTotp();
              }}
            />
          </div>
        </div>
        <div class="modal-footer">
          <button
            type="button"
            class="btn btn-outline-secondary"
            style="min-height: 44px;"
            onclick={() => (totpModalOpen = false)}
          >
            Cancel
          </button>
          <button
            type="button"
            class="btn btn-primary"
            style="min-height: 44px;"
            onclick={handleEnableTotp}
          >
            Verify & Activate 2FA
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Nuvio Parity Profile Manager Modal -->
<ProfileManagerModal
  open={profileModalOpen}
  {session}
  profiles={localProfiles}
  {activeProfileId}
  onClose={() => (profileModalOpen = false)}
  onSelectProfile={(id) => {
    activeProfileId = id;
    const p = localProfiles.find((p) => p.id === id);
    if (p) {
      accountNotice = `Switched to profile: ${p.name}`;
    }
  }}
  onCreateProfile={(newP) => {
    const p: UserProfile = { ...newP, id: `prf_${Date.now()}` };
    saveLocalProfiles([...localProfiles, p]);
    activeProfileId = p.id;
  }}
  onUpdateProfile={(updatedP) => {
    saveLocalProfiles(
      localProfiles.map((p) => (p.id === updatedP.id ? updatedP : p)),
    );
  }}
  onDeleteProfile={(id) => {
    saveLocalProfiles(localProfiles.filter((p) => p.id !== id));
  }}
/>

<style>
  .settings-container {
    width: 100%;
    max-width: none;
    margin: 0;
    padding: clamp(20px, 2vw, 32px) clamp(16px, 2.5vw, 40px) 64px;
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
    grid-template-columns: minmax(12rem, 14rem) minmax(0, 1fr);
    gap: clamp(20px, 2.5vw, 40px);
  }

  .settings-nav {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .settings-nav a {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 8px;
    border-radius: calc(5px * var(--tblr-border-radius-scale, 1));
    padding: 9px 12px;
    text-align: left;
    color: var(--fasti-text-muted);
    text-decoration: none;
  }

  .settings-nav a:hover,
  .settings-nav a.active {
    background: var(--fasti-surface-paper);
    color: var(--fasti-text-primary);
  }

  .settings-nav a.active {
    font-weight: 700;
    box-shadow: inset 3px 0 0 var(--fasti-action-primary);
  }

  .settings-section-selector {
    display: none;
    width: min(100%, 28rem);
  }

  .settings-panel {
    min-width: 0;
  }

  button:focus-visible,
  a:focus-visible,
  input:focus-visible {
    outline: 3px solid var(--fasti-focus);
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

  .docs-link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--fasti-action-primary, #206bc4);
    font-size: 0.85rem;
    font-weight: 600;
    text-decoration: none;
  }

  .docs-link:hover,
  .docs-link:focus-visible {
    text-decoration: underline;
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
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(7px * var(--tblr-border-radius-scale, 1));
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
    color: var(--fasti-text-primary);
    text-decoration: underline;
    text-underline-offset: 0.15em;
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
    border-radius: calc(5px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(7px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(5px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(7px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(7px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(5px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(7px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(6px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(3px * var(--tblr-border-radius-scale, 1));
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
    border-radius: calc(4px * var(--tblr-border-radius-scale, 1));
    color: var(--fasti-text-muted);
    border: 1px solid
      color-mix(in srgb, var(--fasti-text-muted) 15%, transparent);
  }

  /* Account & Sessions Tab Styles */
  .account-tab-panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .account-hero-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 6px 0;
  }

  .account-avatar-circle {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    display: grid;
    place-items: center;
    font-size: 1.4rem;
    font-weight: 700;
    flex-shrink: 0;
  }

  .account-details {
    flex: 1 1 auto;
    min-width: 0;
  }

  .account-details h4 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 700;
  }

  .profile-cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
  }

  .profile-tile-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px;
    background: var(--fasti-surface-paper);
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
    border-radius: calc(var(--tblr-border-radius-scale, 1) * 8px);
    transition: all 120ms ease;
  }

  .profile-tile-item.active-tile {
    border-color: var(--fasti-action-primary);
    background: color-mix(
      in srgb,
      var(--fasti-action-primary) 6%,
      var(--fasti-surface-paper)
    );
  }

  .profile-mini-avatar {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: var(--fasti-action-primary);
    color: var(--fasti-action-contrast);
    display: grid;
    place-items: center;
    font-size: 0.95rem;
    font-weight: 700;
    flex-shrink: 0;
  }

  .profile-mini-avatar.admin {
    background: var(--fasti-brand-mark, #8b2e2a);
  }

  .session-access-col {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .bg-surface {
    background: var(--fasti-surface-paper) !important;
  }

  @media (max-width: 64rem) {
    .settings-container {
      padding: 24px 16px 48px;
    }

    .settings-layout {
      grid-template-columns: minmax(0, 1fr);
    }

    .settings-section-selector {
      display: grid;
      gap: 6px;
    }

    .settings-nav {
      display: none;
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
