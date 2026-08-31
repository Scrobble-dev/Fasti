<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
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
    IconUser,
    IconWorld,
  } from "@tabler/icons-svelte";
  import AccountSecurityView from "./account-security-view.svelte";
  import NetworkSettings from "./network-settings.svelte";
  import { hostProblemText } from "./host-problem.js";
  import { newOperationId } from "./operation-id.js";
  import type {
    AccessProjectionResponse,
    AnimeGroupingPolicyImpactResponse,
    AnimeGroupingPreferenceDto,
    CustomFieldDefinition,
    CustomMediaTypeDefinition,
    ConfigureMetadataProjectionRequest,
    EnrichmentPolicyDto,
    MediaKind,
    MetadataFieldGroupDto,
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
    metadataPolicyRecordId?: string;
    canAccessProfileData?: boolean;
    profileDataIdentity?: string;
    accessProjection?: AccessProjectionResponse;
    readAccessProjection: () => Promise<AccessProjectionResponse>;
    accessNotice?: string;
    callbackMarker?: "continue" | "failed";
    onAccessNoticeConsumed?: () => void;
    onAccessCallbackConsumed?: () => void;
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
    onUpdateWorkbenchPreferences?: (
      patch: Partial<WorkbenchPreferences>,
    ) => void;
    onClientEndpointChanged?: () => void;
    onProviderCredentialsChanged?: () => void;
    onMetadataPolicyChanged?: () => void;
    onAccessProjection?: (projection?: AccessProjectionResponse) => void;
    onStartFirstRun?: () => void;
    onOpenConnections?: () => void;
    onClearCache?: (
      cache: "search" | "history" | "statistics" | "discover" | "all",
    ) => void;
  }

  let {
    host,
    workbenchPreferences,
    metadataPolicyRecordId,
    canAccessProfileData = true,
    profileDataIdentity = "trusted-host",
    accessProjection,
    readAccessProjection,
    accessNotice,
    callbackMarker,
    onAccessNoticeConsumed,
    onAccessCallbackConsumed,
    activeTab = "network",
    onTabChange,
    onUpdateWorkbenchPreferences,
    onClientEndpointChanged,
    onProviderCredentialsChanged,
    onMetadataPolicyChanged,
    onAccessProjection,
    onStartFirstRun,
    onOpenConnections,
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
  let healthProvider = $state<string>();
  let testResults = $state<
    Record<string, { ok: boolean; message: string } | undefined>
  >({});
  let healthResults = $state<
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

  function providerCredentialLabel(provider: string): string {
    if (provider === "tmdb") return "TMDB API Read Access Token";
    if (provider === "google-books") return "Google Books API key";
    return "Provider credential";
  }

  function providerRowKey(provider: ProviderCredentialStatus): string {
    return `${provider.provider}:${provider.capability_id}`;
  }

  function hasStoredCredential(provider: ProviderCredentialStatus): boolean {
    return ["stored_unverified", "valid", "invalid", "expired"].includes(
      provider.credential_state,
    );
  }

  function credentialStateLabel(provider: ProviderCredentialStatus): string {
    return provider.credential_state.replaceAll("_", " ");
  }

  async function testProviderCredential(provider: ProviderCredentialStatus) {
    if (!canAccessProfileData || credentialOperationBusy) return;
    const identity = profileDataIdentity;
    const key = providerRowKey(provider);
    testingProvider = key;
    testResults = { ...testResults, [key]: undefined };
    try {
      const nextProviders = await host.testProviderCredential(
        provider.provider,
        provider.capability_id,
      );
      if (identity !== profileDataIdentity) return;
      providers = nextProviders;
      testResults = {
        ...testResults,
        [key]: {
          ok: true,
          message: "Credential test passed.",
        },
      };
    } catch (error: unknown) {
      if (identity !== profileDataIdentity) return;
      testResults = {
        ...testResults,
        [key]: {
          ok: false,
          message: hostProblemText(
            error,
            "Credential test failed. Verify the credential and network policy.",
          ),
        },
      };
    } finally {
      if (identity === profileDataIdentity) testingProvider = undefined;
    }
  }

  async function readProviderHealth(provider: ProviderCredentialStatus) {
    if (!canAccessProfileData || credentialOperationBusy) return;
    const identity = profileDataIdentity;
    healthProvider = provider.provider;
    healthResults = { ...healthResults, [provider.provider]: undefined };
    try {
      const nextProviders = await host.readProviderHealth(provider.provider);
      if (identity !== profileDataIdentity) return;
      providers = nextProviders;
      healthResults = {
        ...healthResults,
        [provider.provider]: {
          ok: true,
          message: "Provider health check passed.",
        },
      };
    } catch (error: unknown) {
      if (identity !== profileDataIdentity) return;
      try {
        const nextProviders = await host.providerCredentialStatus();
        if (identity !== profileDataIdentity) return;
        providers = nextProviders;
      } catch {
        // Preserve the health failure. The regular Refresh action remains available.
      }
      healthResults = {
        ...healthResults,
        [provider.provider]: {
          ok: false,
          message: hostProblemText(
            error,
            "Provider health check failed. Verify the credential and network policy.",
          ),
        },
      };
    } finally {
      if (identity === profileDataIdentity) healthProvider = undefined;
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
  const credentialOperationBusy = $derived(
    !canAccessProfileData ||
      providerLoading ||
      Boolean(busyProvider) ||
      Boolean(testingProvider) ||
      Boolean(healthProvider),
  );
  let nuvioDocument = $state<NuvioCollectionsDocument | null>(null);
  let nuvioFile = $state<File>();
  let nuvioFileInput = $state<HTMLInputElement>();
  let nuvioLoading = $state(false);
  let nuvioProblem = $state<string>();
  let nuvioNotice = $state<string>();
  let nuvioRequestGeneration = 0;
  let activeNuvioIdentity: string | undefined;
  let metadataPolicy = $state<EnrichmentPolicyDto>();
  let metadataPolicyDraft = $state<ConfigureMetadataProjectionRequest>();
  let metadataPolicyLoading = $state(false);
  let metadataPolicySaving = $state(false);
  let metadataPolicyProblem = $state<string>();
  let metadataPolicyNotice = $state<string>();
  let loadedMetadataPolicyRecordId = "";
  let metadataPolicyGeneration = 0;
  let animePolicy =
    $state<
      Awaited<ReturnType<NonNullable<WorkbenchHost["readAnimeGroupingPolicy"]>>>
    >();
  let animePolicyDraft = $state<AnimeGroupingPreferenceDto>("automatic");
  let animePolicyPreview = $state<AnimeGroupingPolicyImpactResponse>();
  let animePolicyLoading = $state(false);
  let animePolicySaving = $state(false);
  let animePolicyProblem = $state<string>();
  let animePolicyNotice = $state<string>();
  let animePolicyOperationId = "";
  let animePolicyGeneration = 0;

  const animeGroupingPreferences = [
    ["automatic", "Automatic"],
    ["group_by_tv_work", "Group releases by TV work"],
    ["keep_mal_releases_separate", "Keep MyAnimeList releases separate"],
    ["keep_kitsu_releases_separate", "Keep Kitsu releases separate"],
  ] as const satisfies ReadonlyArray<
    readonly [AnimeGroupingPreferenceDto, string]
  >;

  const metadataFieldGroups = [
    "artwork",
    "basic_info",
    "details",
    "release_dates",
    "credits",
    "production_companies",
    "networks",
    "episodes",
    "season_artwork",
    "recommendations",
    "collections",
    "trailers",
    "watch_providers",
  ] as const satisfies readonly MetadataFieldGroupDto[];

  function metadataFieldGroupLabel(group: MetadataFieldGroupDto): string {
    return group.replaceAll("_", " ");
  }

  function policyDraftFrom(
    policy: EnrichmentPolicyDto,
  ): ConfigureMetadataProjectionRequest {
    return {
      preferred_provider_id: policy.preferred_provider_id,
      preferred_locale: policy.preferred_locale,
      original_locale: policy.original_locale,
      allow_english_fallback: policy.allow_english_fallback,
      last_known_good: policy.last_known_good,
      region: policy.region,
      enabled_field_groups: [...policy.enabled_field_groups],
      overrides: [],
    };
  }

  $effect(() => {
    const identity = profileDataIdentity;
    const tab = activeTab;
    const canLoadProfileData = canAccessProfileData;
    const policyRecordId = metadataPolicyRecordId;
    untrack(() => {
      if (identity !== activeNuvioIdentity) {
        activeNuvioIdentity = identity;
        resetNuvioProfileState();
        resetMetadataPolicyState();
        resetAnimePolicyState();
        resetProviderState();
      }
      if (tab) {
        active = tab;
        if (tab === "nuvio_collections" && canLoadProfileData) {
          void loadNuvioCollections();
        }
        if (tab === "providers" && canLoadProfileData) {
          void loadProviders();
        }
        if (
          tab === "preferences" &&
          policyRecordId &&
          policyRecordId !== loadedMetadataPolicyRecordId
        ) {
          void loadMetadataPolicy(policyRecordId);
        }
        if (tab === "preferences" && canLoadProfileData && !animePolicy) {
          void loadAnimePolicy();
        }
      }
    });
  });

  async function loadMetadataPolicy(
    recordId: string,
    restoreRetryFocus = false,
  ): Promise<void> {
    const generation = ++metadataPolicyGeneration;
    loadedMetadataPolicyRecordId = recordId;
    metadataPolicyProblem = undefined;
    metadataPolicyNotice = undefined;
    if (!host.readMetadataProjection) {
      metadataPolicy = undefined;
      metadataPolicyDraft = undefined;
      metadataPolicyProblem =
        "This host does not expose the governed profile metadata policy.";
      return;
    }
    metadataPolicyLoading = true;
    try {
      const projection = await host.readMetadataProjection(recordId, false);
      if (generation === metadataPolicyGeneration) {
        metadataPolicy = projection.policy;
        metadataPolicyDraft = policyDraftFrom(projection.policy);
      }
    } catch (error) {
      if (generation === metadataPolicyGeneration) {
        metadataPolicy = undefined;
        metadataPolicyDraft = undefined;
        metadataPolicyProblem = hostProblemText(
          error,
          "Fasti could not load this profile's metadata policy.",
        );
      }
    } finally {
      if (generation === metadataPolicyGeneration) {
        metadataPolicyLoading = false;
        if (restoreRetryFocus) {
          await tick();
          document.getElementById("retry-metadata-policy")?.focus();
        }
      }
    }
  }

  function toggleMetadataFieldGroup(
    group: MetadataFieldGroupDto,
    enabled: boolean,
  ): void {
    if (!metadataPolicyDraft) return;
    const groups = new Set(metadataPolicyDraft.enabled_field_groups);
    if (enabled) groups.add(group);
    else groups.delete(group);
    const orderedGroups = [
      ...metadataFieldGroups,
      ...metadataPolicyDraft.enabled_field_groups.filter(
        (item) => !metadataFieldGroups.includes(item),
      ),
    ];
    metadataPolicyDraft = {
      ...metadataPolicyDraft,
      enabled_field_groups: orderedGroups.filter((item) => groups.has(item)),
    };
  }

  async function saveMetadataPolicy(): Promise<void> {
    if (
      !canAccessProfileData ||
      !host.configureMetadataProjection ||
      !metadataPolicyDraft ||
      metadataPolicySaving
    )
      return;
    const generation = ++metadataPolicyGeneration;
    const identity = profileDataIdentity;
    metadataPolicySaving = true;
    metadataPolicyProblem = undefined;
    metadataPolicyNotice = undefined;
    try {
      const response =
        await host.configureMetadataProjection(metadataPolicyDraft);
      if (
        generation !== metadataPolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      metadataPolicy = response.policy;
      metadataPolicyDraft = policyDraftFrom(response.policy);
      onMetadataPolicyChanged?.();
      metadataPolicyNotice = `Saved the profile metadata policy. Fasti invalidated ${response.invalidated_cache_entries.toLocaleString()} affected cache entries.`;
    } catch (error) {
      if (
        generation !== metadataPolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      metadataPolicyProblem = hostProblemText(
        error,
        "Fasti could not save this profile's metadata policy.",
      );
    } finally {
      if (
        generation === metadataPolicyGeneration &&
        identity === profileDataIdentity
      ) {
        metadataPolicySaving = false;
      }
    }
  }

  function resetNuvioProfileState(): void {
    nuvioRequestGeneration += 1;
    nuvioDocument = null;
    nuvioFile = undefined;
    if (nuvioFileInput) nuvioFileInput.value = "";
    nuvioLoading = false;
    nuvioProblem = undefined;
    nuvioNotice = undefined;
  }

  function resetMetadataPolicyState(): void {
    metadataPolicyGeneration += 1;
    loadedMetadataPolicyRecordId = "";
    metadataPolicy = undefined;
    metadataPolicyDraft = undefined;
    metadataPolicyLoading = false;
    metadataPolicySaving = false;
    metadataPolicyProblem = undefined;
    metadataPolicyNotice = undefined;
  }

  function resetAnimePolicyState(): void {
    animePolicyGeneration += 1;
    animePolicy = undefined;
    animePolicyDraft = "automatic";
    animePolicyPreview = undefined;
    animePolicyLoading = false;
    animePolicySaving = false;
    animePolicyProblem = undefined;
    animePolicyNotice = undefined;
    animePolicyOperationId = "";
  }

  async function loadAnimePolicy(restoreRetryFocus = false): Promise<void> {
    const generation = ++animePolicyGeneration;
    const identity = profileDataIdentity;
    animePolicyProblem = undefined;
    animePolicyNotice = undefined;
    if (!host.readAnimeGroupingPolicy) {
      animePolicyProblem =
        "This host does not expose the governed anime grouping policy.";
      return;
    }
    animePolicyLoading = true;
    try {
      const response = await host.readAnimeGroupingPolicy({
        scope: "profile",
        client_id: null,
      });
      if (
        generation !== animePolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      animePolicy = response;
      animePolicyDraft = response.policy.preference;
      animePolicyPreview = undefined;
      animePolicyOperationId = "";
    } catch (error) {
      if (
        generation !== animePolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      animePolicyProblem = hostProblemText(
        error,
        "Fasti could not load this profile's anime grouping policy.",
      );
    } finally {
      if (
        generation === animePolicyGeneration &&
        identity === profileDataIdentity
      ) {
        animePolicyLoading = false;
        if (restoreRetryFocus) {
          await tick();
          document.getElementById("retry-anime-grouping-policy")?.focus();
        }
      }
    }
  }

  async function previewAnimePolicy(): Promise<void> {
    if (!host.previewAnimeGroupingPolicyChange || animePolicyLoading) return;
    const generation = ++animePolicyGeneration;
    const identity = profileDataIdentity;
    animePolicyLoading = true;
    animePolicyProblem = undefined;
    animePolicyNotice = undefined;
    animePolicyOperationId = "";
    try {
      const preview = await host.previewAnimeGroupingPolicyChange({
        scope: { kind: "profile", client_id: null },
        change: { kind: "set", preference: animePolicyDraft },
        after_record_id: null,
        limit: 100,
      });
      if (
        generation !== animePolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      animePolicyPreview = preview;
    } catch (error) {
      if (
        generation !== animePolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      animePolicyPreview = undefined;
      animePolicyProblem = hostProblemText(
        error,
        "Fasti could not preview this anime grouping change.",
      );
    } finally {
      if (
        generation === animePolicyGeneration &&
        identity === profileDataIdentity
      )
        animePolicyLoading = false;
    }
  }

  async function applyAnimePolicy(): Promise<void> {
    if (
      !host.applyAnimeGroupingPolicyChange ||
      !animePolicy ||
      !animePolicyPreview ||
      animePolicySaving
    )
      return;
    const generation = ++animePolicyGeneration;
    const identity = profileDataIdentity;
    animePolicySaving = true;
    animePolicyProblem = undefined;
    animePolicyNotice = undefined;
    animePolicyOperationId ||= newOperationId();
    try {
      const applied = await host.applyAnimeGroupingPolicyChange({
        operation_id: animePolicyOperationId,
        scope: { kind: "profile", client_id: null },
        expected_revision: animePolicy.policy.revision,
        change: { kind: "set", preference: animePolicyDraft },
      });
      if (
        generation !== animePolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      animePolicy = { policy: applied.policy };
      animePolicyDraft = applied.policy.preference;
      animePolicyPreview = undefined;
      animePolicyOperationId = "";
      animePolicyNotice = `Saved the profile policy. ${applied.affected_records.toLocaleString()} Records changed route.`;
    } catch (error) {
      if (
        generation !== animePolicyGeneration ||
        identity !== profileDataIdentity
      )
        return;
      animePolicyProblem = hostProblemText(
        error,
        "Fasti could not save this anime grouping policy.",
      );
    } finally {
      if (
        generation === animePolicyGeneration &&
        identity === profileDataIdentity
      )
        animePolicySaving = false;
    }
  }

  function resetProviderState(): void {
    providers = [];
    providerLoading = false;
    providerProblem = canAccessProfileData
      ? undefined
      : "Sign in before reviewing or changing provider credentials.";
    providerNotice = undefined;
    editing = {};
    showPassword = {};
    busyProvider = undefined;
    testingProvider = undefined;
    healthProvider = undefined;
    testResults = {};
    healthResults = {};
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
    if (!canAccessProfileData) {
      resetProviderState();
      return;
    }
    if (credentialOperationBusy) return;
    const identity = profileDataIdentity;
    providerLoading = true;
    providerProblem = undefined;
    testResults = {};
    try {
      const loaded = await host.providerCredentialStatus();
      if (identity !== profileDataIdentity) return;
      const writable = new Set(
        loaded.filter((provider) => provider.writable).map(providerRowKey),
      );
      editing = Object.fromEntries(
        Object.entries(editing).filter(([provider]) => writable.has(provider)),
      );
      showPassword = Object.fromEntries(
        Object.entries(showPassword).filter(([provider]) =>
          writable.has(provider),
        ),
      );
      providers = loaded;
    } catch (error) {
      if (identity !== profileDataIdentity) return;
      providerProblem = hostProblemText(
        error,
        "Fasti could not load provider status.",
      );
    } finally {
      if (identity === profileDataIdentity) providerLoading = false;
    }
  }

  async function saveProvider(
    provider: ProviderCredentialStatus,
  ): Promise<void> {
    const key = providerRowKey(provider);
    const credential = editing[key]?.trim();
    if (!canAccessProfileData || !credential || credentialOperationBusy) return;
    const identity = profileDataIdentity;
    showPassword = { ...showPassword, [key]: false };
    busyProvider = key;
    providerProblem = undefined;
    providerNotice = undefined;
    try {
      const nextProviders = await host.saveProviderCredential(
        provider.provider,
        provider.capability_id,
        credential,
      );
      if (identity !== profileDataIdentity) return;
      providers = nextProviders;
      testResults = { ...testResults, [key]: undefined };
      providerNotice = "Credential saved in the platform credential store.";
      editing = { ...editing, [key]: "" };
      onProviderCredentialsChanged?.();
    } catch (error) {
      if (identity !== profileDataIdentity) return;
      providerProblem = hostProblemText(
        error,
        "Fasti rejected the provider credential.",
      );
      busyProvider = undefined;
      await tick();
      document.getElementById(`provider-${key}`)?.focus();
    } finally {
      if (identity === profileDataIdentity) {
        showPassword = { ...showPassword, [key]: false };
        busyProvider = undefined;
      }
    }
  }

  async function deleteProvider(
    provider: ProviderCredentialStatus,
  ): Promise<void> {
    if (!canAccessProfileData || credentialOperationBusy) return;
    const identity = profileDataIdentity;
    const key = providerRowKey(provider);
    const label = provider.label;
    if (
      !globalThis.confirm(
        `Remove the ${label} credential? You will need to enter it again to restore provider access.`,
      )
    ) {
      return;
    }
    busyProvider = key;
    providerProblem = undefined;
    providerNotice = undefined;
    let removed = false;
    try {
      const nextProviders = await host.deleteProviderCredential(
        provider.provider,
        provider.capability_id,
      );
      if (identity !== profileDataIdentity) return;
      providers = nextProviders;
      editing = { ...editing, [key]: "" };
      showPassword = { ...showPassword, [key]: false };
      testResults = { ...testResults, [key]: undefined };
      providerNotice = "Credential removed from the platform credential store.";
      onProviderCredentialsChanged?.();
      removed = true;
    } catch (error) {
      if (identity !== profileDataIdentity) return;
      providerProblem = hostProblemText(
        error,
        "Fasti could not remove the provider credential.",
      );
    } finally {
      if (identity === profileDataIdentity) busyProvider = undefined;
    }
    if (removed) {
      await tick();
      document.getElementById(`provider-${key}`)?.focus();
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
        capability: p.capability_id,
        credentialState: p.credential_state,
        capabilityState: p.state,
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
          <option value="account">Account and security</option>
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
          onclick={(event) => followTabLink(event, "account")}
          ><IconUser size={16} aria-hidden="true" /> Account and security</a
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
        <AccountSecurityView
          {host}
          mode="task_map"
          projection={accessProjection}
          {readAccessProjection}
          initialNotice={accessNotice}
          onInitialNoticeConsumed={onAccessNoticeConsumed}
          onProjection={onAccessProjection}
          {callbackMarker}
          onCallbackConsumed={onAccessCallbackConsumed}
          {onStartFirstRun}
          {onOpenConnections}
        />
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
              disabled={credentialOperationBusy}
            >
              <IconRefresh size={18} aria-hidden="true" />
              {providerLoading ? "Loading…" : "Refresh"}
            </button>
          </div>

          <div class="table-responsive provider-list">
            <table class="table table-vcenter">
              <caption class="visually-hidden">
                Metadata provider capability and credential status
              </caption>
              <thead>
                <tr>
                  <th scope="col">Provider</th>
                  <th scope="col">Capability</th>
                  <th scope="col">Status</th>
                  <th scope="col">Credential</th>
                  <th scope="col">Actions</th>
                </tr>
              </thead>
              <tbody>
                {#each providers as provider (providerRowKey(provider))}
                  {@const key = providerRowKey(provider)}
                  <tr>
                    <th scope="row">
                      <span class="provider-name">{provider.label}</span>
                      <span class="badge bg-secondary-lt"
                        >{providerCategory(provider.provider).label}</span
                      >
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
                    </th>
                    <td>
                      <code>{provider.capability_id}</code>
                      <span class="capability-purpose">{provider.purpose}</span>
                    </td>
                    <td>
                      <span
                        class="badge"
                        class:bg-success-lt={provider.state === "available"}
                        class:bg-secondary-lt={provider.state !== "available"}
                        >{provider.state}</span
                      >
                    </td>
                    <td>
                      <span
                        class="badge"
                        class:bg-success-lt={hasStoredCredential(provider)}
                        class:bg-secondary-lt={!hasStoredCredential(provider)}
                        >{credentialStateLabel(provider)}</span
                      >
                      <span class="credential-source">
                        {provider.source.replaceAll("_", " ")}
                      </span>
                    </td>
                    <td class="provider-actions">
                      {#if provider.writable}
                        <form
                          class="credential-form"
                          onsubmit={(event) => {
                            event.preventDefault();
                            void saveProvider(provider);
                          }}
                        >
                          <label
                            class="visually-hidden"
                            for={`provider-${key}`}
                          >
                            {providerCredentialLabel(provider.provider)} for
                            {provider.capability_id}
                          </label>
                          <div class="credential-input-row">
                            <div class="secret-field-wrap">
                              <input
                                id={`provider-${key}`}
                                class="form-control"
                                type={showPassword[key] ? "text" : "password"}
                                autocomplete="new-password"
                                maxlength="4096"
                                placeholder={providerCredentialLabel(
                                  provider.provider,
                                )}
                                value={editing[key] ?? ""}
                                oninput={(event) =>
                                  (editing = {
                                    ...editing,
                                    [key]: event.currentTarget.value,
                                  })}
                                disabled={credentialOperationBusy}
                              />
                              <button
                                type="button"
                                class="toggle-reveal-btn"
                                title={showPassword[key]
                                  ? "Hide secret"
                                  : "Show secret"}
                                aria-label={showPassword[key]
                                  ? "Hide secret"
                                  : "Show secret"}
                                onclick={() =>
                                  (showPassword = {
                                    ...showPassword,
                                    [key]: !showPassword[key],
                                  })}
                              >
                                {#if showPassword[key]}
                                  <IconEyeOff size={16} aria-hidden="true" />
                                {:else}
                                  <IconEye size={16} aria-hidden="true" />
                                {/if}
                              </button>
                            </div>
                            <button
                              type="submit"
                              class="btn btn-primary primary"
                              disabled={!editing[key]?.trim() ||
                                credentialOperationBusy}
                            >
                              <IconKey size={18} aria-hidden="true" /> Save
                            </button>
                            {#if hasStoredCredential(provider)}
                              <button
                                type="button"
                                class="btn btn-outline-danger danger"
                                onclick={() => void deleteProvider(provider)}
                                disabled={credentialOperationBusy}
                                >Remove</button
                              >
                            {/if}
                          </div>
                        </form>
                      {:else}
                        <p class="managed-note">
                          {provider.source === "environment"
                            ? "Managed by the process environment."
                            : provider.state === "unavailable"
                              ? "This capability is unavailable in this runtime."
                              : "This host cannot write this credential."}
                        </p>
                      {/if}

                      {#if provider.testable && hasStoredCredential(provider)}
                        <button
                          type="button"
                          class="btn btn-outline-secondary secondary test-conn-btn"
                          onclick={() => void testProviderCredential(provider)}
                          disabled={credentialOperationBusy}
                        >
                          {testingProvider === key
                            ? "Testing…"
                            : "Test credential"}
                        </button>
                      {/if}
                      {#if provider.capability_id === "metadata.search" && provider.testable && hasStoredCredential(provider)}
                        <button
                          type="button"
                          class="btn btn-outline-secondary secondary test-conn-btn"
                          onclick={() => void readProviderHealth(provider)}
                          disabled={credentialOperationBusy}
                        >
                          {healthProvider === provider.provider
                            ? "Checking…"
                            : "Check provider health"}
                        </button>
                      {/if}
                      {#if testResults[key]}
                        <div
                          class="test-result-alert"
                          class:success={testResults[key]?.ok}
                          class:failure={!testResults[key]?.ok}
                          role={testResults[key]?.ok ? "status" : "alert"}
                        >
                          {#if testResults[key]?.ok}
                            <IconCheck size={16} aria-hidden="true" />
                          {:else}
                            <IconAlertCircle size={16} aria-hidden="true" />
                          {/if}
                          <span>{testResults[key]?.message}</span>
                        </div>
                      {/if}
                      {#if provider.capability_id === "metadata.search" && healthResults[provider.provider]}
                        <div
                          class="test-result-alert"
                          class:success={healthResults[provider.provider]?.ok}
                          class:failure={!healthResults[provider.provider]?.ok}
                          role={healthResults[provider.provider]?.ok
                            ? "status"
                            : "alert"}
                        >
                          {#if healthResults[provider.provider]?.ok}
                            <IconCheck size={16} aria-hidden="true" />
                          {:else}
                            <IconAlertCircle size={16} aria-hidden="true" />
                          {/if}
                          <span
                            >{healthResults[provider.provider]?.message}</span
                          >
                        </div>
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
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
          <section
            class="card metadata-policy-card"
            data-testid="anime-grouping-policy"
            aria-labelledby="anime-grouping-policy-title"
          >
            <div class="card-header">
              <div>
                <h3 id="anime-grouping-policy-title" class="card-title">
                  Anime grouping
                </h3>
                <p class="card-subtitle text-secondary">
                  Set the profile default used for identity routes and Nuvio
                  exports. Application clients can keep a separate override
                  through the same governed API.
                </p>
              </div>
            </div>
            <div class="card-body">
              {#if animePolicyLoading && !animePolicyPreview}
                <p role="status">Loading the anime grouping policy…</p>
              {:else if animePolicyProblem && !animePolicy}
                <div class="alert alert-danger" role="alert">
                  <p>{animePolicyProblem}</p>
                  <button
                    id="retry-anime-grouping-policy"
                    type="button"
                    class="btn btn-outline-danger"
                    onclick={() => void loadAnimePolicy(true)}
                  >
                    Retry policy read
                  </button>
                </div>
              {:else if animePolicy}
                <div class="metadata-policy-grid">
                  <div>
                    <label class="form-label" for="anime-grouping-preference"
                      >Profile default</label
                    >
                    <select
                      id="anime-grouping-preference"
                      class="form-select"
                      value={animePolicyDraft}
                      disabled={animePolicySaving}
                      onchange={(event) => {
                        animePolicyDraft = event.currentTarget
                          .value as AnimeGroupingPreferenceDto;
                        animePolicyPreview = undefined;
                        animePolicyOperationId = "";
                        animePolicyNotice = undefined;
                      }}
                    >
                      {#each animeGroupingPreferences as [value, label]}
                        <option {value}>{label}</option>
                      {/each}
                    </select>
                  </div>
                  <div class="metadata-policy-actions">
                    <button
                      type="button"
                      class="btn btn-outline-primary"
                      data-testid="preview-anime-grouping-policy"
                      disabled={!host.previewAnimeGroupingPolicyChange ||
                        animePolicyLoading ||
                        animePolicySaving}
                      onclick={() => void previewAnimePolicy()}
                    >
                      {animePolicyLoading ? "Reviewing…" : "Review impact"}
                    </button>
                    <span class="text-secondary"
                      >Revision {animePolicy.policy.revision.toLocaleString()}</span
                    >
                  </div>
                </div>

                {#if animePolicyPreview}
                  <div class="alert alert-info mt-3" role="status">
                    <strong
                      >{animePolicyPreview.affected_records.toLocaleString()}
                      affected Records.</strong
                    >
                    {animePolicyPreview.unresolved_routes.toLocaleString()}
                    unresolved routes and
                    {animePolicyPreview.possible_season_regroupings.toLocaleString()}
                    possible season regroupings.
                  </div>
                  {#if animePolicyPreview.records.length > 0}
                    <div class="table-responsive">
                      <table class="table table-vcenter">
                        <caption>
                          First {animePolicyPreview.records.length.toLocaleString()}
                          affected Records from a stable, paged preview
                        </caption>
                        <thead>
                          <tr>
                            <th scope="col">Record</th>
                            <th scope="col">Proposed route</th>
                            <th scope="col">Status</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each animePolicyPreview.records as record}
                            <tr>
                              <td><code>{record.record_id}</code></td>
                              <td>
                                {record.proposed_route
                                  ? `${record.proposed_route.identifier.namespace}:${record.proposed_route.identifier.value}`
                                  : "No route"}
                              </td>
                              <td
                                >{record.proposed_status.replaceAll(
                                  "_",
                                  " ",
                                )}</td
                              >
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    </div>
                  {/if}
                  <button
                    type="button"
                    class="btn btn-primary"
                    data-testid="apply-anime-grouping-policy"
                    disabled={!host.applyAnimeGroupingPolicyChange ||
                      animePolicySaving}
                    onclick={() => void applyAnimePolicy()}
                  >
                    {animePolicySaving ? "Saving…" : "Apply profile policy"}
                  </button>
                {/if}

                {#if animePolicyNotice}
                  <p class="notice" role="status">{animePolicyNotice}</p>
                {/if}
                {#if animePolicyProblem}
                  <p class="problem" role="alert">{animePolicyProblem}</p>
                {/if}
              {/if}
            </div>
          </section>
          <section
            class="card metadata-policy-card"
            data-testid="metadata-projection-policy"
            aria-labelledby="metadata-policy-title"
          >
            <div class="card-header">
              <div>
                <h3 id="metadata-policy-title" class="card-title">
                  Profile metadata projection
                </h3>
                <p class="card-subtitle text-secondary">
                  Fasti owns and validates this profile policy. This browser
                  does not store a second policy.
                </p>
              </div>
            </div>
            <div class="card-body">
              {#if !metadataPolicyRecordId}
                <div class="alert alert-warning" role="status">
                  <strong>Current policy unavailable.</strong>
                  Add a Record before editing this policy. The projection read requires
                  a real Record context.
                </div>
              {:else if metadataPolicyLoading}
                <p role="status">Loading the profile metadata policy…</p>
              {:else if metadataPolicyProblem && !metadataPolicyDraft}
                <div class="alert alert-danger" role="alert">
                  <p>{metadataPolicyProblem}</p>
                  <button
                    id="retry-metadata-policy"
                    type="button"
                    class="btn btn-outline-danger"
                    onclick={() =>
                      void loadMetadataPolicy(metadataPolicyRecordId, true)}
                  >
                    Retry policy read
                  </button>
                </div>
              {:else if metadataPolicy && metadataPolicyDraft}
                <form
                  onsubmit={(event) => {
                    event.preventDefault();
                    void saveMetadataPolicy();
                  }}
                >
                  <div class="metadata-policy-grid">
                    <div>
                      <label class="form-label" for="metadata-provider-policy"
                        >Preferred provider</label
                      >
                      <select
                        id="metadata-provider-policy"
                        class="form-select"
                        value={metadataPolicyDraft.preferred_provider_id ?? ""}
                        onchange={(event) =>
                          (metadataPolicyDraft = {
                            ...metadataPolicyDraft!,
                            preferred_provider_id:
                              event.currentTarget.value || null,
                          })}
                      >
                        <option value="">No preferred provider</option>
                        {#each Array.from(new Set( [...(metadataPolicy.preferred_provider_id ? [metadataPolicy.preferred_provider_id] : []), ...providers.map((provider) => provider.provider)] )) as provider}
                          <option value={provider}>{provider}</option>
                        {/each}
                      </select>
                    </div>

                    <div>
                      <label class="form-label" for="metadata-preferred-locale"
                        >Preferred locale</label
                      >
                      <input
                        id="metadata-preferred-locale"
                        class="form-control"
                        value={metadataPolicyDraft.preferred_locale ?? ""}
                        maxlength="16"
                        placeholder="en-IE"
                        onchange={(event) =>
                          (metadataPolicyDraft = {
                            ...metadataPolicyDraft!,
                            preferred_locale:
                              event.currentTarget.value.trim() || null,
                          })}
                      />
                    </div>

                    <div>
                      <label class="form-label" for="metadata-original-locale"
                        >Original locale</label
                      >
                      <input
                        id="metadata-original-locale"
                        class="form-control"
                        value={metadataPolicyDraft.original_locale ?? ""}
                        maxlength="16"
                        placeholder="ja-JP"
                        onchange={(event) =>
                          (metadataPolicyDraft = {
                            ...metadataPolicyDraft!,
                            original_locale:
                              event.currentTarget.value.trim() || null,
                          })}
                      />
                    </div>

                    <div>
                      <label class="form-label" for="metadata-region-policy"
                        >Region</label
                      >
                      <input
                        id="metadata-region-policy"
                        class="form-control"
                        value={metadataPolicyDraft.region ?? ""}
                        maxlength="8"
                        placeholder="IE"
                        onchange={(event) =>
                          (metadataPolicyDraft = {
                            ...metadataPolicyDraft!,
                            region: event.currentTarget.value.trim() || null,
                          })}
                      />
                    </div>

                    <div>
                      <label class="form-label" for="metadata-lkg-policy"
                        >Last known good claims</label
                      >
                      <select
                        id="metadata-lkg-policy"
                        class="form-select"
                        value={metadataPolicyDraft.last_known_good}
                        onchange={(event) =>
                          (metadataPolicyDraft = {
                            ...metadataPolicyDraft!,
                            last_known_good: event.currentTarget.value as
                              "allow" | "deny",
                          })}
                      >
                        <option value="allow">Allow when policy permits</option>
                        <option value="deny">Deny</option>
                      </select>
                    </div>

                    <label class="form-check metadata-policy-check">
                      <input
                        class="form-check-input"
                        type="checkbox"
                        checked={metadataPolicyDraft.allow_english_fallback}
                        onchange={(event) =>
                          (metadataPolicyDraft = {
                            ...metadataPolicyDraft!,
                            allow_english_fallback: event.currentTarget.checked,
                          })}
                      />
                      <span class="form-check-label"
                        >Allow English fallback</span
                      >
                    </label>
                  </div>

                  <fieldset class="metadata-field-groups">
                    <legend class="form-label">Enabled field groups</legend>
                    <div class="metadata-field-group-grid">
                      {#each metadataFieldGroups as group}
                        <label class="form-check metadata-field-group-check">
                          <input
                            class="form-check-input"
                            type="checkbox"
                            checked={metadataPolicyDraft.enabled_field_groups.includes(
                              group,
                            )}
                            onchange={(event) =>
                              toggleMetadataFieldGroup(
                                group,
                                event.currentTarget.checked,
                              )}
                          />
                          <span class="form-check-label"
                            >{metadataFieldGroupLabel(group)}</span
                          >
                        </label>
                      {/each}
                    </div>
                  </fieldset>

                  <div class="metadata-policy-actions">
                    <button
                      type="submit"
                      class="btn btn-primary"
                      data-testid="configure-metadata-projection"
                      disabled={!host.configureMetadataProjection ||
                        metadataPolicySaving}
                      title={host.configureMetadataProjection
                        ? undefined
                        : "Metadata policy configuration is unavailable on this host"}
                    >
                      {metadataPolicySaving ? "Saving…" : "Save profile policy"}
                    </button>
                    <span class="text-secondary"
                      >Profile <code>{metadataPolicy.profile_id}</code></span
                    >
                  </div>
                </form>
              {:else}
                <p class="managed-note">
                  The current profile metadata policy is unavailable.
                </p>
              {/if}

              {#if metadataPolicyNotice}
                <p class="notice" role="status">{metadataPolicyNotice}</p>
              {/if}
              {#if metadataPolicyProblem && metadataPolicyDraft}
                <p class="problem" role="alert">{metadataPolicyProblem}</p>
              {/if}
            </div>
          </section>

          <h3 class="legacy-preferences-title">Legacy display preferences</h3>
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
              Profile data access is unavailable in this host. Open Fasti in a
              host that provides profile-data access.
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

<style>
  .settings-container {
    width: 100%;
    max-width: none;
    margin: 0;
    padding: clamp(20px, 2vw, 32px) clamp(16px, 2.5vw, 40px) 64px;
    overflow-wrap: anywhere;
  }

  .metadata-policy-card {
    margin-block: 20px 28px;
  }

  .metadata-policy-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr));
    gap: 18px;
  }

  .metadata-policy-check {
    align-self: end;
    min-height: 44px;
    padding-block: 10px;
  }

  .metadata-field-group-check {
    min-height: 44px;
    align-items: center;
    padding-block: 8px;
  }

  .metadata-policy-card .form-check-input {
    flex: none;
    width: 1rem;
    min-width: 1rem;
    min-height: 1rem;
    padding: 0;
  }

  .metadata-field-groups {
    margin-block: 24px;
    padding: 0;
    border: 0;
  }

  .metadata-field-group-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 190px), 1fr));
    gap: 12px 20px;
  }

  .metadata-policy-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px;
  }

  .metadata-policy-actions .text-secondary {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .legacy-preferences-title {
    margin-block: 32px 8px;
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
  section > p {
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
  .credential-form > div {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
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
    margin-top: 20px;
    border: 1px solid
      var(--fasti-border, color-mix(in srgb, currentColor 18%, transparent));
  }

  .provider-list table {
    margin-bottom: 0;
  }

  .provider-name,
  .capability-purpose,
  .credential-source {
    display: block;
  }

  .provider-name {
    margin-bottom: 4px;
  }

  .capability-purpose,
  .credential-source {
    margin-top: 4px;
    color: var(--fasti-text-muted);
    font-size: 0.82rem;
  }

  .docs-link {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--fasti-text-primary);
    text-decoration: underline;
    text-underline-offset: 0.15em;
  }

  .provider-actions {
    min-width: min(32rem, 45vw);
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
