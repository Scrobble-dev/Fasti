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
const CREDENTIALS_STORAGE_KEY = "fasti-provider-credentials";

function loadSavedCredentials(): Record<string, string> {
  if (typeof localStorage === "undefined") return {};
  try {
    const raw = localStorage.getItem(CREDENTIALS_STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveStoredCredentials(creds: Record<string, string>): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(CREDENTIALS_STORAGE_KEY, JSON.stringify(creds));
  } catch {
    // Best-effort storage
  }
}

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
  let network = loadNetworkConfiguration(defaultApiUrl);
  let client = createClient(network.connection.service_url.value);
  let savedCredentials = loadSavedCredentials();

  return {
    networkConfigurationScope: "client",
    developmentTestAccountHint: import.meta.env.DEV
      ? "Fresh development data root: testadmin / testadmin"
      : undefined,
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

    async providerCredentialStatus(): Promise<ProviderCredentialStatus[]> {
      return PROVIDERS.map((provider) => {
        const hasCred = Boolean(savedCredentials[provider.provider]);
        return {
          provider: provider.provider,
          label: provider.label,
          configured: hasCred,
          source: hasCred ? "credential_store" : "none",
          writable: true,
          docs_url: provider.docs_url,
        };
      });
    },

    async saveProviderCredential(
      provider: string,
      credential: string,
    ): Promise<ProviderCredentialStatus[]> {
      if (!credential || !credential.trim()) {
        throw new Error("Credential cannot be empty.");
      }
      savedCredentials = {
        ...savedCredentials,
        [provider]: credential.trim(),
      };
      saveStoredCredentials(savedCredentials);
      return this.providerCredentialStatus();
    },

    async deleteProviderCredential(
      provider: string,
    ): Promise<ProviderCredentialStatus[]> {
      const next = { ...savedCredentials };
      delete next[provider];
      savedCredentials = next;
      saveStoredCredentials(savedCredentials);
      return this.providerCredentialStatus();
    },

    async searchProvider(
      provider: string,
      query: string,
    ): Promise<ProviderSearchCandidate[]> {
      const cred = savedCredentials[provider];
      if (provider === "tmdb") {
        if (!cred) {
          throw new Error("TMDB Read Access Token is required to search TMDB.");
        }
        const isV3Key = cred.length === 32 && /^[0-9a-fA-F]+$/.test(cred);
        const url = isV3Key
          ? `https://api.themoviedb.org/3/search/multi?api_key=${encodeURIComponent(cred)}&query=${encodeURIComponent(query)}&include_adult=false&language=en-US&page=1`
          : `https://api.themoviedb.org/3/search/multi?query=${encodeURIComponent(query)}&include_adult=false&language=en-US&page=1`;
        const headers: Record<string, string> = isV3Key
          ? { Accept: "application/json" }
          : { Accept: "application/json", Authorization: `Bearer ${cred}` };
        const response = await fetch(url, { headers });
        if (!response.ok) {
          throw new Error(
            `TMDB returned HTTP ${response.status}: ${response.statusText}`,
          );
        }
        const data = await response.json();
        const results = Array.isArray(data.results) ? data.results : [];
        return results
          .filter(
            (item: any) =>
              item.media_type === "movie" || item.media_type === "tv",
          )
          .map((item: any) => {
            const isMovie = item.media_type === "movie";
            const title = isMovie
              ? item.title || item.original_title
              : item.name || item.original_name;
            const releaseDate = isMovie
              ? item.release_date
              : item.first_air_date;
            const year = releaseDate ? releaseDate.slice(0, 4) : undefined;
            const posterPath = item.poster_path
              ? `https://image.tmdb.org/t/p/w500${item.poster_path}`
              : null;
            return {
              provider: "tmdb",
              provider_id: String(item.id),
              title: title || "Untitled",
              kind: isMovie ? "movie" : "show",
              release_year: year ? parseInt(year, 10) : undefined,
              authors: [],
              image_url: posterPath,
            } satisfies ProviderSearchCandidate;
          });
      }

      if (provider === "google-books") {
        const url = cred
          ? `https://www.googleapis.com/books/v1/volumes?q=${encodeURIComponent(query)}&key=${encodeURIComponent(cred)}`
          : `https://www.googleapis.com/books/v1/volumes?q=${encodeURIComponent(query)}`;
        const response = await fetch(url);
        if (!response.ok) {
          throw new Error(`Google Books API returned HTTP ${response.status}`);
        }
        const data = await response.json();
        const items = Array.isArray(data.items) ? data.items : [];
        return items.map((item: any) => {
          const info = item.volumeInfo || {};
          const imageLinks = info.imageLinks || {};
          const img = imageLinks.thumbnail || imageLinks.smallThumbnail || null;
          const year = info.publishedDate
            ? info.publishedDate.slice(0, 4)
            : undefined;
          return {
            provider: "google-books",
            provider_id: String(item.id),
            title: info.title || "Untitled Book",
            kind: "book",
            release_year: year ? parseInt(year, 10) : undefined,
            authors: Array.isArray(info.authors) ? info.authors : [],
            image_url: img ? img.replace(/^http:/, "https:") : null,
          } satisfies ProviderSearchCandidate;
        });
      }

      if (provider === "open-library") {
        const response = await fetch(
          `https://openlibrary.org/search.json?q=${encodeURIComponent(query)}&limit=20`,
        );
        if (!response.ok) throw new Error("Open Library search failed");
        const data = await response.json();
        const docs = Array.isArray(data.docs) ? data.docs : [];
        return docs.map(
          (doc: any) =>
            ({
              provider: "open-library",
              provider_id:
                doc.key?.replace("/works/", "") ||
                String(doc.cover_i || Math.random()),
              title: doc.title || "Untitled",
              kind: "book",
              release_year: doc.first_publish_year,
              authors: Array.isArray(doc.author_name) ? doc.author_name : [],
              image_url: doc.cover_i
                ? `https://covers.openlibrary.org/b/id/${doc.cover_i}-M.jpg`
                : null,
            }) satisfies ProviderSearchCandidate,
        );
      }

      if (provider === "kitsu") {
        const response = await fetch(
          `https://kitsu.io/api/edge/anime?filter[text]=${encodeURIComponent(query)}`,
        );
        if (!response.ok) throw new Error("Kitsu search failed");
        const data = await response.json();
        const items = Array.isArray(data.data) ? data.data : [];
        return items.map((item: any) => {
          const attr = item.attributes || {};
          const titles = attr.titles || {};
          return {
            provider: "kitsu",
            provider_id: String(item.id),
            title: titles.en || titles.en_jp || attr.canonicalTitle || "Anime",
            kind: "anime",
            release_year: attr.startDate
              ? parseInt(attr.startDate.slice(0, 4), 10)
              : undefined,
            authors: [],
            image_url:
              attr.posterImage?.medium || attr.posterImage?.small || null,
          } satisfies ProviderSearchCandidate;
        });
      }

      throw new Error(
        `Provider ${provider} does not support live browser search yet.`,
      );
    },

    clearSearchCache(): void {},
    getSearchCacheSize(): number {
      return 0;
    },

    async listRecords(): Promise<RecordSummary[]> {
      const response = await client.listRecords();
      return response.records.map((record) => ({
        ...record,
        poster: { ...record.poster, value: null },
      })) as RecordSummary[];
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
