import { connectionEndpoint, FastiClient } from "@fasti/sdk";
import type {
  AttachIdentifierInput,
  AttachIdentifierResult,
  BrowserSession,
  BrowserUser,
  BrowserUserUpdate,
  CreateRecordResult,
  EndpointConnectionStatus,
  NetworkConfiguration,
  NuvioCollectionsDocument,
  NuvioCollectionsState,
  ProviderCredentialStatus,
  ProviderSearchCandidate,
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

const PROVIDERS: ReadonlyArray<{
  provider: string;
  label: string;
  docs_url: string;
}> = [
  {
    provider: "open-library",
    label: "Open Library (Books)",
    docs_url: "https://openlibrary.org/developers/api",
  },
  {
    provider: "kitsu",
    label: "Kitsu (Anime & Manga)",
    docs_url: "https://kitsu.docs.apiary.io",
  },
  {
    provider: "anilist",
    label: "AniList GraphQL (Anime/Manga)",
    docs_url: "https://docs.anilist.co",
  },
  {
    provider: "musicbrainz",
    label: "MusicBrainz (Music)",
    docs_url: "https://musicbrainz.org/doc/MusicBrainz_API",
  },
  {
    provider: "tmdb",
    label: "TheMovieDatabase (TMDB)",
    docs_url: "https://developer.themoviedb.org/docs",
  },
  {
    provider: "tvdb",
    label: "TheTVDB v4",
    docs_url: "https://thetvdb.com/api-information",
  },
  {
    provider: "google-books",
    label: "Google Books API",
    docs_url: "https://developers.google.com/books/docs/v1/using",
  },
  {
    provider: "mal",
    label: "MyAnimeList API v2",
    docs_url: "https://myanimelist.net/apiconfig/references/api/v2",
  },
  {
    provider: "rawg",
    label: "RAWG Video Games Database",
    docs_url: "https://rawg.io/apidocs",
  },
  {
    provider: "igdb",
    label: "IGDB (Games)",
    docs_url: "https://api-docs.igdb.com",
  },
  {
    provider: "comicvine",
    label: "ComicVine (Comics)",
    docs_url: "https://comicvine.gamespot.com/api/documentation",
  },
  {
    provider: "podcast-index",
    label: "Podcast Index (Podcasts)",
    docs_url: "https://podcastindex-org.github.io/docs-api",
  },
];

function defaultNetworkConfiguration(
  defaultApiUrl: string,
): NetworkConfiguration {
  const serviceUrl = connectionEndpoint(
    defaultApiUrl || "http://127.0.0.1:8420",
    "default",
  ).url;
  return {
    connection: {
      service_url: {
        value: serviceUrl,
        source: "default",
        managed: false,
      },
      public_url: { value: null, source: "default", managed: false },
    },
    outbound_policy: {
      allow_providers: ["*"],
      deny_providers: [],
      allow_capabilities: ["*"],
      deny_capabilities: [],
      allow_hosts: ["*"],
      deny_hosts: [],
      allow_networks: ["public", "loopback", "private"],
      deny_networks: [],
    },
  };
}

function loadNetworkConfiguration(defaultApiUrl: string): NetworkConfiguration {
  if (typeof localStorage === "undefined") {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
  try {
    const raw = localStorage.getItem(NETWORK_STORAGE_KEY);
    if (!raw) return defaultNetworkConfiguration(defaultApiUrl);
    const value = JSON.parse(raw) as Partial<NetworkConfiguration>;
    if (
      typeof value?.connection?.service_url?.value !== "string" ||
      !value.outbound_policy ||
      !Array.isArray(value.outbound_policy.allow_networks) ||
      !Array.isArray(value.outbound_policy.deny_networks)
    ) {
      return defaultNetworkConfiguration(defaultApiUrl);
    }
    connectionEndpoint(value.connection.service_url.value);
    const publicUrl = value.connection.public_url?.value;
    if (typeof publicUrl === "string") connectionEndpoint(publicUrl);
    return value as NetworkConfiguration;
  } catch {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
}

function unavailable(message: string): Error {
  return new Error(message);
}

function csrfToken(): string {
  if (typeof document === "undefined") return "";
  const values = document.cookie
    .split(";")
    .map((part) => part.trim().split("="))
    .filter(([name]) => name === "fasti_csrf");
  return values.length === 1 ? (values[0][1] ?? "") : "";
}

export function createWebHost(defaultApiUrl: string): WorkbenchHost {
  const createClient = (baseUrl: string): FastiClient =>
    new FastiClient({
      baseUrl,
      useBrowserSession: true,
      csrfToken,
    });
  let client = createClient(
    connectionEndpoint(defaultApiUrl || "http://127.0.0.1:8420", "default").url,
  );

  return {
    developmentTestAccountHint: import.meta.env.DEV
      ? "Fresh development data root: testadmin / testadmin"
      : undefined,
    async loadNetworkConfiguration(): Promise<NetworkConfiguration> {
      return loadNetworkConfiguration(defaultApiUrl);
    },

    async saveNetworkConfiguration(
      input: SaveNetworkConfigurationRequest,
    ): Promise<NetworkConfiguration> {
      const endpoint = connectionEndpoint(input.service_url);
      const publicUrl = input.public_url
        ? connectionEndpoint(input.public_url).url
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
      client = nextClient;
      return config;
    },

    async testEndpointConnection(
      endpoint: string,
    ): Promise<EndpointConnectionStatus> {
      const target = connectionEndpoint(endpoint);
      const health = await new FastiClient({
        baseUrl: target.url,
        timeoutMs: 3_000,
        retryPolicy: { maxAttempts: 1 },
      }).health();
      return {
        endpoint: target.url,
        scheme: target.scheme,
        status: health.status,
        version: health.version,
      };
    },

    async providerCredentialStatus(): Promise<ProviderCredentialStatus[]> {
      return PROVIDERS.map((provider) => ({
        provider: provider.provider,
        label: provider.label,
        configured: false,
        source: "none",
        writable: false,
        docs_url: provider.docs_url,
      }));
    },

    async saveProviderCredential(
      provider: string,
    ): Promise<ProviderCredentialStatus[]> {
      throw unavailable(
        `${provider} credentials require the trusted native or server host. The browser host never accepts or stores provider secrets.`,
      );
    },

    async deleteProviderCredential(
      provider: string,
    ): Promise<ProviderCredentialStatus[]> {
      throw unavailable(
        `${provider} credentials are not managed by the browser host.`,
      );
    },

    async searchProvider(
      provider: string,
      _query: string,
    ): Promise<ProviderSearchCandidate[]> {
      throw unavailable(
        `${provider} search is not active in the browser host. Provider requests must use the governed native or server host.`,
      );
    },

    clearSearchCache(): void {},
    getSearchCacheSize(): number {
      return 0;
    },

    async listRecords(): Promise<RecordSummary[]> {
      const response = await client.listRecords();
      return response.records as RecordSummary[];
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

    async createBrowserSession(
      username: string,
      password: string,
      sessionTimeoutMinutes: number,
    ): Promise<BrowserSession> {
      return client.createBrowserSession({
        username,
        password,
        session_timeout_minutes: sessionTimeoutMinutes,
      });
    },

    async currentBrowserSession(): Promise<BrowserSession> {
      return client.readBrowserSession();
    },

    async endBrowserSession(): Promise<void> {
      await client.endBrowserSession();
    },

    async listBrowserUsers(): Promise<BrowserUser[]> {
      const response = await client.listBrowserUsers();
      return [...response.users];
    },

    async updateBrowserUser(
      userId: string,
      input: BrowserUserUpdate,
    ): Promise<BrowserUser> {
      return client.updateBrowserUser(userId, input);
    },

    async deleteBrowserUser(
      userId: string,
      currentPassword: string,
    ): Promise<void> {
      await client.deleteBrowserUser(userId, {
        current_password: currentPassword,
      });
    },
  };
}
