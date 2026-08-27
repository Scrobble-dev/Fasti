import type {
  EndpointConnectionStatus,
  IntegrationRuntimeStatus,
  IntegrationStatusHost,
  IntegrationStatusResponse,
  NetworkConfiguration,
  ProviderCredentialStatus,
  ProviderSearchCandidate,
  RecordSummary,
  SaveNetworkConfigurationRequest,
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
): NetworkConfiguration {
  return {
    connection: {
      service_url: {
        value: defaultApiUrl || "http://127.0.0.1:8420",
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
  if (typeof localStorage === "undefined")
    return defaultNetworkConfiguration(defaultApiUrl);
  try {
    const raw = localStorage.getItem(NETWORK_STORAGE_KEY);
    if (!raw) return defaultNetworkConfiguration(defaultApiUrl);
    const value = JSON.parse(raw) as Partial<NetworkConfiguration>;
    if (
      typeof value?.connection?.service_url?.value !== "string" ||
      !value.outbound_policy ||
      !Array.isArray(value.outbound_policy.allow_networks) ||
      !Array.isArray(value.outbound_policy.deny_networks)
    )
      return defaultNetworkConfiguration(defaultApiUrl);
    return value as NetworkConfiguration;
  } catch {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
}

function unavailable(message: string): Error {
  return new Error(message);
}
function normalizedEndpoint(value: string): string {
  return value.replace(/\/+$/, "");
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

export async function fetchIntegrationStatus(
  endpoint: string,
): Promise<IntegrationRuntimeStatus[]> {
  const normalized = normalizedEndpoint(endpoint);
  const response = await fetch(`${normalized}/api/v1/integrations`, {
    headers: { Accept: "application/json" },
    cache: "no-store",
    signal: AbortSignal.timeout(3_000),
  });
  if (!response.ok)
    throw unavailable(`Fasti integration status returned ${response.status}.`);
  const value = (await response.json()) as Partial<IntegrationStatusResponse>;
  if (
    !Array.isArray(value.integrations) ||
    !value.integrations.every(isIntegrationStatus)
  ) {
    throw unavailable(
      "Fasti integration status did not match the supported contract.",
    );
  }
  return value.integrations;
}

export function createWebHost(
  defaultApiUrl: string,
): WorkbenchHost & IntegrationStatusHost {
  return {
    async loadNetworkConfiguration(): Promise<NetworkConfiguration> {
      return loadNetworkConfiguration(defaultApiUrl);
    },
    async saveNetworkConfiguration(
      input: SaveNetworkConfigurationRequest,
    ): Promise<NetworkConfiguration> {
      const config: NetworkConfiguration = {
        connection: {
          service_url: {
            value: input.service_url,
            source: "saved",
            managed: false,
          },
          public_url: {
            value: input.public_url,
            source: "saved",
            managed: false,
          },
        },
        outbound_policy: input.outbound_policy,
      };
      if (typeof localStorage !== "undefined") {
        try {
          localStorage.setItem(NETWORK_STORAGE_KEY, JSON.stringify(config));
        } catch {}
      }
      return config;
    },
    async testEndpointConnection(
      endpoint: string,
    ): Promise<EndpointConnectionStatus> {
      const normalized = normalizedEndpoint(endpoint);
      const parsed = new URL(normalized);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
        throw unavailable("Only HTTP and HTTPS Fasti endpoints are supported.");
      }
      if (
        parsed.protocol === "http:" &&
        !["127.0.0.1", "localhost", "::1", "[::1]"].includes(parsed.hostname)
      ) {
        throw unavailable(
          "Cleartext HTTP is allowed only for loopback endpoints.",
        );
      }
      const response = await fetch(`${normalized}/api/v1/health`, {
        signal: AbortSignal.timeout(3_000),
      });
      if (!response.ok)
        throw unavailable(`The endpoint returned status ${response.status}.`);
      const data = (await response.json()) as {
        status?: string;
        version?: string;
      };
      return {
        endpoint: normalized,
        scheme: parsed.protocol === "https:" ? "https" : "http",
        status: data.status ?? "healthy",
        version: data.version ?? "unknown",
      };
    },
    async listIntegrations(): Promise<IntegrationRuntimeStatus[]> {
      const configured = loadNetworkConfiguration(defaultApiUrl);
      return fetchIntegrationStatus(
        configured.connection.service_url.value || defaultApiUrl,
      );
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
      throw unavailable(
        "Record listing is not active in the browser host. It requires the trusted native or server host.",
      );
    },
  };
}
