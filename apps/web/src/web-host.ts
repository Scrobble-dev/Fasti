import {
  connectionEndpoint,
  FastiClient,
  type CredentialProvider,
  type LocalSearchRequestDto,
  type LocalSearchResponseDto,
} from "@fasti/sdk";
import type {
  AttachIdentifierInput,
  AttachIdentifierResult,
  ConfigureMetadataProjectionRequest,
  CreateRecordResult,
  EndpointConnectionStatus,
  IntegrationRuntimeStatus,
  IntegrationStatusHost,
  IntegrationStatusResponse,
  ListRecordsQueryParameters,
  NetworkConfiguration,
  MetadataProjectionConfigurationResponse,
  MetadataProjectionResponse,
  NuvioCollectionsDocument,
  NuvioCollectionsState,
  ProviderCredentialStatus,
  ProviderSearchCandidate,
  RefreshMetadataClaimsRequest,
  RefreshMetadataClaimsResponse,
  RecordPage,
  RecordSummary,
  RegisterNamespaceInput,
  RegisterNamespaceResult,
  SaveNetworkConfigurationRequest,
  TrackingDispositionState,
  TrackingDispositionList,
  TrackingDispositionUpdate,
  WorkbenchHost,
} from "@fasti/ui";

const NETWORK_STORAGE_KEY = "fasti-network-config";

const INTEGRATION_STATES = new Set([
  "available",
  "setup_required",
  "active",
  "degraded",
  "disabled",
  "unsupported",
  "error",
]);

function defaultNetworkConfiguration(
  defaultApiUrl: string,
  source: "default" | "saved" = "default",
): NetworkConfiguration {
  const serviceUrl = connectionEndpoint(
    defaultApiUrl || "http://127.0.0.1:8420",
    "default",
  ).url;
  return {
    connection: {
      service_url: {
        value: serviceUrl,
        source,
        managed: false,
      },
      public_url: { value: null, source: "default", managed: true },
    },
    outbound_policy: {
      allow_providers: [],
      deny_providers: [],
      allow_capabilities: [],
      deny_capabilities: [],
      allow_hosts: [],
      deny_hosts: [],
      allow_networks: [],
      deny_networks: [],
    },
  };
}

function unavailable(message: string): Error {
  return new Error(message);
}

function checkedEndpoint(value: string) {
  try {
    return connectionEndpoint(value);
  } catch {
    throw unavailable(
      "The Fasti service URL must contain only a scheme, host, and optional port. Cleartext HTTP is allowed only for loopback endpoints.",
    );
  }
}

function loadNetworkConfiguration(defaultApiUrl: string): NetworkConfiguration {
  if (typeof localStorage === "undefined") {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
  try {
    const raw = localStorage.getItem(NETWORK_STORAGE_KEY);
    if (!raw) return defaultNetworkConfiguration(defaultApiUrl);
    const value = JSON.parse(raw) as Partial<NetworkConfiguration> & {
      service_url?: unknown;
    };
    const candidate =
      typeof value.service_url === "string"
        ? value.service_url
        : value.connection?.service_url?.value;
    if (typeof candidate !== "string") {
      return defaultNetworkConfiguration(defaultApiUrl);
    }
    const serviceUrl = checkedEndpoint(candidate).url;
    if (
      !value.connection ||
      !value.outbound_policy ||
      !Array.isArray(value.outbound_policy.allow_networks) ||
      !Array.isArray(value.outbound_policy.deny_networks)
    ) {
      return defaultNetworkConfiguration(serviceUrl, "saved");
    }
    const publicUrl = value.connection.public_url?.value;
    return {
      connection: {
        service_url: { value: serviceUrl, source: "saved", managed: false },
        public_url: {
          value:
            typeof publicUrl === "string"
              ? checkedEndpoint(publicUrl).url
              : null,
          source: "saved",
          managed: false,
        },
      },
      outbound_policy: value.outbound_policy,
    };
  } catch {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
}

function isIntegrationStatus(
  value: unknown,
): value is IntegrationRuntimeStatus {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<IntegrationRuntimeStatus>;
  return (
    typeof candidate.id === "string" &&
    typeof candidate.label === "string" &&
    typeof candidate.state === "string" &&
    INTEGRATION_STATES.has(candidate.state) &&
    typeof candidate.available === "boolean" &&
    typeof candidate.endpoint_ready === "boolean" &&
    typeof candidate.setup_action === "string" &&
    typeof candidate.detail === "string"
  );
}

function parseIntegrationStatusResponse(
  integrations: unknown,
): IntegrationRuntimeStatus[] {
  if (
    !Array.isArray(integrations) ||
    !integrations.every(isIntegrationStatus)
  ) {
    throw unavailable(
      "Fasti integration status did not match the supported contract.",
    );
  }
  return integrations as unknown as IntegrationRuntimeStatus[];
}

async function loadProviderRows(
  client: FastiClient,
): Promise<ProviderCredentialStatus[]> {
  const response = await client.listProviders();
  return response.providers.flatMap((provider) =>
    provider.capabilities.map((capability) => ({
      provider: provider.provider_id,
      capability_id: capability.capability_id,
      label: provider.display_name,
      purpose: capability.purpose,
      credential_requirement: capability.credential_requirement,
      credential_state: capability.credential_state,
      state: capability.state,
      source: capability.credential_source,
      writable: capability.writable,
      testable: capability.testable,
      docs_url: provider.documentation_url,
    })),
  );
}

export async function fetchIntegrationStatus(
  endpoint: string,
  signal?: AbortSignal,
): Promise<IntegrationRuntimeStatus[]> {
  const target = checkedEndpoint(endpoint);
  const probeClient = new FastiClient({
    baseUrl: target.url,
    timeoutMs: 3_000,
    retryPolicy: { maxAttempts: 1 },
  });
  const response = await probeClient.listIntegrations({ signal });
  return parseIntegrationStatusResponse(response.integrations);
}

export function createWebHost(
  defaultApiUrl: string,
  credential?: CredentialProvider,
): WorkbenchHost & IntegrationStatusHost {
  const createClient = (baseUrl: string): FastiClient =>
    new FastiClient({
      baseUrl,
      credential,
    });
  let network = loadNetworkConfiguration(defaultApiUrl);
  let client = createClient(network.connection.service_url.value);
  const accessClient = new FastiClient({
    baseUrl:
      typeof window === "undefined" ? defaultApiUrl : window.location.origin,
  });

  const metadataHost: Partial<WorkbenchHost> = credential
    ? {
        async readMetadataProjection(
          recordId: string,
          offline = false,
        ): Promise<MetadataProjectionResponse> {
          return client.readMetadataProjection(recordId, { offline });
        },

        async configureMetadataProjection(
          request: ConfigureMetadataProjectionRequest,
        ): Promise<MetadataProjectionConfigurationResponse> {
          return client.configureMetadataProjection(request);
        },

        async refreshMetadataClaims(
          request: RefreshMetadataClaimsRequest,
        ): Promise<RefreshMetadataClaimsResponse> {
          return client.refreshMetadataClaims(request);
        },
      }
    : {};

  return {
    networkConfigurationScope: "client",
    profileDataAuthority: credential ? "scoped" : "browser_session",
    startTrailBaseSignIn: (request) =>
      accessClient.startTrailBaseSignIn(request),
    readTrailBaseContinuation: (signal) =>
      accessClient.readTrailBaseContinuation({ signal }),
    completeTrailBaseContinuation: (request) =>
      accessClient.completeTrailBaseContinuation(request),
    cancelTrailBaseContinuation: () =>
      accessClient.cancelTrailBaseContinuation(),
    readAccessProjection: (signal) =>
      accessClient.readAccessProjection({ signal }),
    endBrowserSession: () => accessClient.endBrowserSession(),
    revokeBrowserSession: (browserSessionId) =>
      accessClient.revokeBrowserSession(browserSessionId),
    revokeOtherBrowserSessions: () => accessClient.revokeOtherBrowserSessions(),
    rotateBrowserSession: () => accessClient.rotateBrowserSession(),
    readAnimeGroupingPolicy: (query) =>
      (credential ? client : accessClient).readAnimeGroupingPolicy(query),
    previewAnimeGroupingPolicyChange: (request) =>
      (credential ? client : accessClient).previewAnimeGroupingPolicyChange(
        request,
      ),
    applyAnimeGroupingPolicyChange: (request) =>
      (credential ? client : accessClient).applyAnimeGroupingPolicyChange(
        request,
      ),
    async loadNetworkConfiguration(): Promise<NetworkConfiguration> {
      return network;
    },
    async saveNetworkConfiguration(
      input: SaveNetworkConfigurationRequest,
    ): Promise<NetworkConfiguration> {
      const endpoint = checkedEndpoint(input.service_url);
      const publicUrl = input.public_url
        ? checkedEndpoint(input.public_url).url
        : null;
      const nextClient = createClient(endpoint.url);
      const config: NetworkConfiguration = {
        connection: {
          service_url: {
            value: endpoint.url,
            source: "saved",
            managed: false,
          },
          public_url: {
            value: publicUrl,
            source: "saved",
            managed: false,
          },
        },
        outbound_policy: input.outbound_policy,
      };
      if (typeof localStorage !== "undefined") {
        try {
          localStorage.setItem(NETWORK_STORAGE_KEY, JSON.stringify(config));
        } catch {
          // Best-effort: a blocked or full localStorage should not fail a
          // save the caller already validated -- the returned config is
          // still correct, it just won't survive a reload.
        }
      }
      // Recreate the client with the saved service URL so subsequent SDK
      // calls use the new endpoint rather than the original defaultApiUrl.
      network = config;
      client = nextClient;
      return config;
    },
    async testEndpointConnection(
      endpoint: string,
      signal?: AbortSignal,
    ): Promise<EndpointConnectionStatus> {
      const target = checkedEndpoint(endpoint);
      const health = await new FastiClient({
        baseUrl: target.url,
        timeoutMs: 3_000,
        retryPolicy: { maxAttempts: 1 },
      }).health({ signal });
      return {
        endpoint: target.url,
        scheme: target.scheme,
        status: health.status,
        version: health.version,
      };
    },
    async listIntegrations(): Promise<IntegrationRuntimeStatus[]> {
      const response = await client.listIntegrations();
      return parseIntegrationStatusResponse(response.integrations);
    },
    async providerCredentialStatus(): Promise<ProviderCredentialStatus[]> {
      return loadProviderRows(client);
    },
    async saveProviderCredential(
      provider: string,
      capabilityId: string,
      credential: string,
    ): Promise<ProviderCredentialStatus[]> {
      await client.configureProviderCredential(provider, capabilityId, {
        secret: credential,
      });
      return loadProviderRows(client);
    },
    async deleteProviderCredential(
      provider: string,
      capabilityId: string,
    ): Promise<ProviderCredentialStatus[]> {
      await client.removeProviderCredential(provider, capabilityId);
      return loadProviderRows(client);
    },
    async testProviderCredential(
      provider: string,
      capabilityId: string,
    ): Promise<ProviderCredentialStatus[]> {
      await client.testProviderCredential(provider, capabilityId);
      return loadProviderRows(client);
    },
    async readProviderHealth(
      provider: string,
    ): Promise<ProviderCredentialStatus[]> {
      await client.readProviderHealth(provider);
      return loadProviderRows(client);
    },
    async searchProvider(
      provider: string,
      _query: string,
    ): Promise<ProviderSearchCandidate[]> {
      throw unavailable(
        `${provider} search is not active in the browser host. Provider requests must use the governed native or server host.`,
      );
    },
    async searchRecords(
      request: LocalSearchRequestDto,
    ): Promise<LocalSearchResponseDto> {
      const response = await (credential ? client : accessClient).searchRecords(
        request,
      );
      return {
        ...response,
        records: response.records.map((record) => ({
          ...record,
          poster: { ...record.poster, value: null },
        })),
      };
    },
    searchProviderPage: (provider, request) =>
      (credential ? client : accessClient).searchProviderPage(
        provider,
        request,
      ),
    readSearchCandidate: (provider, grain, candidateReceiptId, offline) =>
      (credential ? client : accessClient).readSearchCandidate(
        provider,
        grain,
        candidateReceiptId,
        { offline },
      ),
    saveSearchCandidate: (provider, grain, candidateReceiptId, request) =>
      (credential ? client : accessClient).saveSearchCandidate(
        provider,
        grain,
        candidateReceiptId,
        request,
      ),
    clearSearchCache(): void {},
    getSearchCacheSize(): number {
      return 0;
    },
    async listRecords(query?: ListRecordsQueryParameters): Promise<RecordPage> {
      const response = await client.listRecords({}, query);
      return {
        truncated: response.truncated,
        records: response.records.map((record) => ({
          ...record,
          poster: { ...record.poster, value: null },
        })) as RecordSummary[],
      };
    },

    async createRecord(grain: string): Promise<CreateRecordResult> {
      return client.createRecord({ grain });
    },

    async attachIdentifier(
      input: AttachIdentifierInput,
    ): Promise<AttachIdentifierResult> {
      return client.attachIdentifier(input);
    },

    async registerNamespace(
      input: RegisterNamespaceInput,
    ): Promise<RegisterNamespaceResult> {
      return client.registerNamespace(input);
    },

    async listTrackingDispositions(): Promise<TrackingDispositionList> {
      const response = await client.listTrackingDispositions();
      return {
        states: response.states as TrackingDispositionState[],
        truncated: response.truncated,
      };
    },

    async setTrackingDisposition(
      recordId: string,
      disposition: TrackingDispositionUpdate,
    ): Promise<TrackingDispositionState> {
      return client.setTrackingDisposition(recordId, {
        disposition,
      }) as Promise<TrackingDispositionState>;
    },

    async getNuvioCollections(): Promise<NuvioCollectionsState> {
      return client.getNuvioCollections();
    },

    async replaceNuvioCollections(
      document: NuvioCollectionsDocument,
    ): Promise<NuvioCollectionsState> {
      return client.replaceNuvioCollections(document);
    },

    async clearNuvioCollections(): Promise<NuvioCollectionsState> {
      return client.clearNuvioCollections();
    },

    ...metadataHost,
  };
}
