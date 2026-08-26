import type {
  EndpointConnectionStatus,
  NetworkConfiguration,
  ProviderCredentialStatus,
  ProviderSearchCandidate,
  SaveNetworkConfigurationRequest,
  WorkbenchHost,
} from "@fasti/ui";

const NETWORK_STORAGE_KEY = "fasti-network-config";
const SEARCH_TTL_MS = 15 * 60 * 1000;

const PROVIDERS: ReadonlyArray<{
  provider: string;
  label: string;
  docs_url: string;
  browser_search: boolean;
}> = [
  { provider: "open-library", label: "Open Library (Books)", docs_url: "https://openlibrary.org/developers/api", browser_search: true },
  { provider: "kitsu", label: "Kitsu (Anime & Manga)", docs_url: "https://kitsu.docs.apiary.io", browser_search: true },
  { provider: "anilist", label: "AniList GraphQL (Anime/Manga)", docs_url: "https://docs.anilist.co", browser_search: true },
  { provider: "musicbrainz", label: "MusicBrainz (Music)", docs_url: "https://musicbrainz.org/doc/MusicBrainz_API", browser_search: true },
  { provider: "tmdb", label: "TheMovieDatabase (TMDB)", docs_url: "https://developer.themoviedb.org/docs", browser_search: false },
  { provider: "tvdb", label: "TheTVDB v4", docs_url: "https://thetvdb.com/api-information", browser_search: false },
  { provider: "google-books", label: "Google Books API", docs_url: "https://developers.google.com/books/docs/v1/using", browser_search: false },
  { provider: "mal", label: "MyAnimeList API v2", docs_url: "https://myanimelist.net/apiconfig/references/api/v2", browser_search: false },
  { provider: "rawg", label: "RAWG Video Games Database", docs_url: "https://rawg.io/apidocs", browser_search: false },
  { provider: "igdb", label: "IGDB (Games)", docs_url: "https://api-docs.igdb.com", browser_search: false },
  { provider: "comicvine", label: "ComicVine (Comics)", docs_url: "https://comicvine.gamespot.com/api/documentation", browser_search: false },
  { provider: "podcast-index", label: "Podcast Index (Podcasts)", docs_url: "https://podcastindex-org.github.io/docs-api", browser_search: false },
];

const searchCache = new Map<string, { timestamp: number; data: ProviderSearchCandidate[] }>();
const inFlightSearches = new Map<string, Promise<ProviderSearchCandidate[]>>();

function normalizeProvider(provider: string): string {
  const normalized = provider.toLowerCase().replace(/_/g, "-");
  if (normalized === "openlibrary") return "open-library";
  if (normalized === "googlebooks") return "google-books";
  if (normalized === "themoviedb") return "tmdb";
  if (normalized === "thetvdb") return "tvdb";
  return normalized;
}

function defaultNetworkConfiguration(defaultApiUrl: string): NetworkConfiguration {
  return {
    connection: {
      service_url: { value: defaultApiUrl || "http://127.0.0.1:8420", source: "default", managed: false },
      public_url: { value: null, source: "default", managed: false },
    },
    outbound_policy: {
      allow_providers: ["*"], deny_providers: [], allow_capabilities: ["*"], deny_capabilities: [],
      allow_hosts: ["*"], deny_hosts: [], allow_networks: ["public", "loopback", "private"], deny_networks: [],
    },
  };
}

function loadNetworkConfiguration(defaultApiUrl: string): NetworkConfiguration {
  if (typeof localStorage === "undefined") return defaultNetworkConfiguration(defaultApiUrl);
  try {
    const raw = localStorage.getItem(NETWORK_STORAGE_KEY);
    if (!raw) return defaultNetworkConfiguration(defaultApiUrl);
    const value = JSON.parse(raw) as Partial<NetworkConfiguration>;
    if (
      typeof value?.connection?.service_url?.value !== "string" ||
      !value.outbound_policy ||
      !Array.isArray(value.outbound_policy.allow_networks) ||
      !Array.isArray(value.outbound_policy.deny_networks)
    ) return defaultNetworkConfiguration(defaultApiUrl);
    return value as NetworkConfiguration;
  } catch {
    return defaultNetworkConfiguration(defaultApiUrl);
  }
}

function unavailableCredentialMessage(provider: string): Error {
  return new Error(`${provider} credentials require a native or server host. The browser host never accepts or stores provider secrets.`);
}

async function searchOpenLibrary(query: string): Promise<ProviderSearchCandidate[]> {
  const response = await fetch(`https://openlibrary.org/search.json?q=${encodeURIComponent(query)}&limit=12`, { signal: AbortSignal.timeout(10_000) });
  if (!response.ok) throw new Error(`Open Library request failed (${response.status})`);
  const data = await response.json();
  return (Array.isArray(data.docs) ? data.docs : []).slice(0, 12).map((doc: any) => ({
    provider: "open-library",
    provider_id: String(doc.key?.replace("/works/", "") || doc.edition_key?.[0] || ""),
    title: doc.title || "Untitled",
    kind: "book",
    release_year: doc.first_publish_year,
    authors: Array.isArray(doc.author_name) ? doc.author_name : [],
    image_url: doc.cover_i ? `https://covers.openlibrary.org/b/id/${doc.cover_i}-M.jpg` : null,
    overview: doc.subtitle || undefined,
    external_ids: Array.isArray(doc.isbn) && doc.isbn[0]
      ? [{ namespace: "isbn", value: String(doc.isbn[0]), status: "matched" as const, source: "open-library" }]
      : [],
  }));
}

async function searchKitsu(query: string): Promise<ProviderSearchCandidate[]> {
  const response = await fetch(`https://kitsu.io/api/edge/anime?filter[text]=${encodeURIComponent(query)}&page[limit]=12`, { signal: AbortSignal.timeout(10_000) });
  if (!response.ok) throw new Error(`Kitsu request failed (${response.status})`);
  const data = await response.json();
  return (Array.isArray(data.data) ? data.data : []).map((item: any) => {
    const attr = item.attributes ?? {};
    const english = attr.titles?.en;
    const canonical = attr.canonicalTitle || attr.titles?.en_jp;
    return {
      provider: "kitsu",
      provider_id: String(item.id),
      title: english || canonical || "Untitled",
      original_title: attr.titles?.ja_jp || undefined,
      kind: attr.subtype === "manga" ? "manga" : "anime",
      release_year: attr.startDate ? Number.parseInt(attr.startDate.slice(0, 4), 10) : undefined,
      authors: [], image_url: attr.posterImage?.medium || attr.posterImage?.small || null,
      overview: attr.synopsis || undefined,
      external_ids: [{ namespace: "kitsu", value: String(item.id), status: "matched" as const, source: "kitsu" }],
    } satisfies ProviderSearchCandidate;
  });
}

async function searchAniList(query: string): Promise<ProviderSearchCandidate[]> {
  const response = await fetch("https://graphql.anilist.co", {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({
      query: `query ($search: String) { Page(perPage: 12) { media(search: $search, type: ANIME) { id idMal title { romaji english native } startDate { year } description coverImage { medium } } } }`,
      variables: { search: query },
    }),
    signal: AbortSignal.timeout(10_000),
  });
  if (!response.ok) throw new Error(`AniList request failed (${response.status})`);
  const data = await response.json();
  const media = data?.data?.Page?.media;
  return (Array.isArray(media) ? media : []).map((item: any) => ({
    provider: "anilist", provider_id: String(item.id),
    title: item.title?.english || item.title?.romaji || item.title?.native || "Untitled",
    original_title: item.title?.native || undefined, kind: "anime", release_year: item.startDate?.year,
    authors: [], image_url: item.coverImage?.medium || null, overview: item.description || undefined,
    external_ids: [
      { namespace: "anilist", value: String(item.id), status: "matched" as const, source: "anilist" },
      ...(item.idMal ? [{ namespace: "mal", value: String(item.idMal), status: "matched" as const, source: "anilist" }] : []),
    ],
  }));
}

async function searchMusicBrainz(query: string): Promise<ProviderSearchCandidate[]> {
  const response = await fetch(`https://musicbrainz.org/ws/2/release-group/?query=${encodeURIComponent(query)}&fmt=json&limit=12`, {
    headers: { "User-Agent": "Fasti/0.1 (https://scrobble.dev)" }, signal: AbortSignal.timeout(10_000),
  });
  if (!response.ok) throw new Error(`MusicBrainz request failed (${response.status})`);
  const data = await response.json();
  const groups = data?.["release-groups"];
  return (Array.isArray(groups) ? groups : []).map((item: any) => ({
    provider: "musicbrainz", provider_id: String(item.id), title: item.title || "Untitled", kind: "music",
    release_year: item["first-release-date"] ? Number.parseInt(String(item["first-release-date"]).slice(0, 4), 10) : undefined,
    authors: Array.isArray(item["artist-credit"]) ? item["artist-credit"].map((credit: any) => credit?.name).filter(Boolean) : [],
    image_url: null,
    external_ids: [{ namespace: "musicbrainz.release_group", value: String(item.id), status: "matched" as const, source: "musicbrainz" }],
  }));
}

async function executeOpenSearch(provider: string, query: string): Promise<ProviderSearchCandidate[]> {
  if (provider === "open-library") return searchOpenLibrary(query);
  if (provider === "kitsu") return searchKitsu(query);
  if (provider === "anilist") return searchAniList(query);
  if (provider === "musicbrainz") return searchMusicBrainz(query);
  throw unavailableCredentialMessage(provider);
}

export function createWebHost(defaultApiUrl: string): WorkbenchHost {
  return {
    async loadNetworkConfiguration(): Promise<NetworkConfiguration> { return loadNetworkConfiguration(defaultApiUrl); },
    async saveNetworkConfiguration(input: SaveNetworkConfigurationRequest): Promise<NetworkConfiguration> {
      const config: NetworkConfiguration = {
        connection: {
          service_url: { value: input.service_url, source: "saved", managed: false },
          public_url: { value: input.public_url, source: "saved", managed: false },
        },
        outbound_policy: input.outbound_policy,
      };
      if (typeof localStorage !== "undefined") localStorage.setItem(NETWORK_STORAGE_KEY, JSON.stringify(config));
      return config;
    },
    async testEndpointConnection(endpoint: string): Promise<EndpointConnectionStatus> {
      const normalized = endpoint.replace(/\/+$/, "");
      const parsed = new URL(normalized);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") throw new Error("Only HTTP and HTTPS Fasti endpoints are supported");
      if (parsed.protocol === "http:" && !["127.0.0.1", "localhost", "::1"].includes(parsed.hostname)) {
        throw new Error("Cleartext HTTP is allowed only for loopback endpoints");
      }
      const response = await fetch(`${normalized}/api/v1/health`, { signal: AbortSignal.timeout(3_000) });
      if (!response.ok) throw new Error(`Endpoint returned status ${response.status}`);
      const data = await response.json();
      return { endpoint: normalized, scheme: parsed.protocol === "https:" ? "https" : "http", status: data.status ?? "healthy", version: data.version ?? "unknown" };
    },
    async providerCredentialStatus(): Promise<ProviderCredentialStatus[]> {
      return PROVIDERS.map((provider) => ({
        provider: provider.provider, label: provider.label, configured: provider.browser_search,
        source: "none", writable: false, docs_url: provider.docs_url,
      }));
    },
    async saveProviderCredential(provider: string): Promise<ProviderCredentialStatus[]> { throw unavailableCredentialMessage(provider); },
    async deleteProviderCredential(provider: string): Promise<ProviderCredentialStatus[]> { throw unavailableCredentialMessage(provider); },
    async searchProvider(provider: string, query: string): Promise<ProviderSearchCandidate[]> {
      const normalizedProvider = normalizeProvider(provider);
      const trimmed = query.trim();
      if (!trimmed) return [];
      if (["all", "auto", "multi"].includes(normalizedProvider)) {
        const providers = PROVIDERS.filter((item) => item.browser_search).map((item) => item.provider);
        const results = await Promise.allSettled(providers.map((item) => this.searchProvider(item, trimmed)));
        return results.flatMap((result) => result.status === "fulfilled" ? result.value : []);
      }
      const descriptor = PROVIDERS.find((item) => item.provider === normalizedProvider);
      if (!descriptor?.browser_search) throw unavailableCredentialMessage(normalizedProvider);
      const cacheKey = `${normalizedProvider}:${trimmed.toLocaleLowerCase()}`;
      const cached = searchCache.get(cacheKey);
      if (cached && Date.now() - cached.timestamp < SEARCH_TTL_MS) return cached.data;
      const existing = inFlightSearches.get(cacheKey);
      if (existing) return existing;
      const pending = executeOpenSearch(normalizedProvider, trimmed)
        .then((data) => { searchCache.set(cacheKey, { timestamp: Date.now(), data }); return data; })
        .finally(() => inFlightSearches.delete(cacheKey));
      inFlightSearches.set(cacheKey, pending);
      return pending;
    },
    clearSearchCache(): void { searchCache.clear(); },
    getSearchCacheSize(): number { return searchCache.size; },
  };
}
