import { FastiClient } from "@fasti/sdk";
import type {
  AttachIdentifierInput,
  AttachIdentifierResult,
  CreateRecordResult,
  EndpointConnectionStatus,
  NetworkConfiguration,
  ProviderCredentialStatus,
  ProviderSearchCandidate,
  RecordSummary,
  RegisterNamespaceInput,
  RegisterNamespaceResult,
  SaveNetworkConfigurationRequest,
  WorkbenchHost,
} from "@fasti/ui";

const NETWORK_STORAGE_KEY = "fasti-network-config";
const RETIRED_CREDENTIAL_STORAGE_KEY = "fasti-bearer-credential";

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
  source: "default" | "saved" = "default",
): NetworkConfiguration {
  return {
    connection: {
      service_url: {
        value: defaultApiUrl || "http://127.0.0.1:8420",
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

function validatedEndpoint(endpoint: string): {
  normalized: string;
  parsed: URL;
} {
  let parsed: URL;
  try {
    parsed = new URL(endpoint.trim());
  } catch {
    throw unavailable("Enter a valid Fasti service URL.");
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw unavailable("Only HTTP and HTTPS Fasti endpoints are supported.");
  }
  if (
    parsed.protocol === "http:" &&
    !["127.0.0.1", "localhost", "::1", "[::1]"].includes(parsed.hostname)
  ) {
    throw unavailable("Cleartext HTTP is allowed only for loopback endpoints.");
  }
  if (
    parsed.username ||
    parsed.password ||
    parsed.search ||
    parsed.hash ||
    (parsed.pathname !== "" && parsed.pathname !== "/")
  ) {
    throw unavailable(
      "The Fasti service URL must contain only a scheme, host, and optional port.",
    );
  }
  return { normalized: parsed.origin, parsed };
}

function loadNetworkConfiguration(defaultApiUrl: string): NetworkConfiguration {
  if (typeof localStorage === "undefined") {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
  try {
    const raw = localStorage.getItem(NETWORK_STORAGE_KEY);
    if (!raw) return defaultNetworkConfiguration(defaultApiUrl);
    const value = JSON.parse(raw) as {
      service_url?: unknown;
      connection?: { service_url?: { value?: unknown } };
    };
    const candidate =
      typeof value.service_url === "string"
        ? value.service_url
        : value.connection?.service_url?.value;
    if (typeof candidate !== "string")
      return defaultNetworkConfiguration(defaultApiUrl);
    return defaultNetworkConfiguration(
      validatedEndpoint(candidate).normalized,
      "saved",
    );
  } catch {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
}

export function createWebHost(defaultApiUrl: string): WorkbenchHost {
  if (typeof localStorage !== "undefined") {
    try {
      localStorage.removeItem(RETIRED_CREDENTIAL_STORAGE_KEY);
    } catch {
      // The browser host still fails closed when storage is unavailable.
    }
  }
  let sessionCredential: string | undefined;
  const requireCredential = (): string => {
    if (!sessionCredential) {
      throw unavailable(
        "Records need an active local bearer credential. Select Connect records and paste a credential with identity_read scope.",
      );
    }
    return sessionCredential;
  };
  let network = loadNetworkConfiguration(defaultApiUrl);
  let client = new FastiClient({
    baseUrl: network.connection.service_url.value,
    credential: requireCredential,
  });

  return {
    networkConfigurationScope: "client",

    setSessionCredential(credential: string): void {
      const normalized = credential.trim();
      if (!/^[0-9a-f]{64}$/i.test(normalized)) {
        throw unavailable(
          "The bearer credential must contain exactly 64 hexadecimal characters.",
        );
      }
      sessionCredential = normalized;
    },

    clearSessionCredential(): void {
      sessionCredential = undefined;
    },

    async loadNetworkConfiguration(): Promise<NetworkConfiguration> {
      return network;
    },

    async saveNetworkConfiguration(
      input: SaveNetworkConfigurationRequest,
    ): Promise<NetworkConfiguration> {
      const serviceUrl = validatedEndpoint(input.service_url).normalized;
      const config = defaultNetworkConfiguration(serviceUrl, "saved");
      if (typeof localStorage === "undefined") {
        throw unavailable("Browser storage is unavailable.");
      }
      try {
        localStorage.setItem(
          NETWORK_STORAGE_KEY,
          JSON.stringify({ service_url: serviceUrl }),
        );
      } catch {
        throw unavailable(
          "The browser could not save the Fasti service URL. Check site storage permissions and try again.",
        );
      }
      if (network.connection.service_url.value !== serviceUrl) {
        sessionCredential = undefined;
      }
      network = config;
      client = new FastiClient({
        baseUrl: serviceUrl,
        credential: requireCredential,
      });
      return config;
    },

    async testEndpointConnection(
      endpoint: string,
    ): Promise<EndpointConnectionStatus> {
      const { normalized, parsed } = validatedEndpoint(endpoint);
      const health = await new FastiClient({
        baseUrl: normalized,
        timeoutMs: 3_000,
        retryPolicy: { maxAttempts: 1 },
      }).health();
      return {
        endpoint: normalized,
        scheme: parsed.protocol === "https:" ? "https" : "http",
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
      requireCredential();
      const response = await client.listRecords();
      return response.records as RecordSummary[];
    },

    async createRecord(grain: string): Promise<CreateRecordResult> {
      requireCredential();
      return client.createRecord({ grain });
    },

    async attachIdentifier(
      input: AttachIdentifierInput,
    ): Promise<AttachIdentifierResult> {
      requireCredential();
      return client.attachIdentifier(input);
    },

    async registerNamespace(
      input: RegisterNamespaceInput,
    ): Promise<RegisterNamespaceResult> {
      requireCredential();
      return client.registerNamespace(input);
    },
  };
}
