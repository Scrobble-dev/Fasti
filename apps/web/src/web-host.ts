import type {
  EndpointConnectionStatus,
  NetworkConfiguration,
  ProviderCredentialStatus,
  ProviderSearchCandidate,
  SaveNetworkConfigurationRequest,
  WorkbenchHost,
} from "@fasti/ui";

const NETWORK_STORAGE_KEY = "fasti-network-config";
const CREDENTIALS_STORAGE_KEY = "fasti-provider-credentials";

const SUPPORTED_PROVIDERS: Array<{
  provider: string;
  label: string;
  docs_url: string;
  open_access: boolean;
}> = [
  {
    provider: "tmdb",
    label: "TheMovieDatabase (TMDB)",
    docs_url: "https://developer.themoviedb.org/docs",
    open_access: false,
  },
  {
    provider: "tvdb",
    label: "TheTVDB v4",
    docs_url: "https://thetvdb.com/api-information",
    open_access: false,
  },
  {
    provider: "google-books",
    label: "Google Books API",
    docs_url: "https://developers.google.com/books/docs/v1/using",
    open_access: false,
  },
  {
    provider: "open-library",
    label: "Open Library (Books)",
    docs_url: "https://openlibrary.org/developers/api",
    open_access: true,
  },
  {
    provider: "kitsu",
    label: "Kitsu (Anime & Manga)",
    docs_url: "https://kitsu.docs.apiary.io",
    open_access: true,
  },
  {
    provider: "anilist",
    label: "AniList GraphQL (Anime/Manga)",
    docs_url: "https://anilist.gitbook.io/anilist-apiv2-docs",
    open_access: true,
  },
  {
    provider: "mal",
    label: "MyAnimeList API v2",
    docs_url: "https://myanimelist.net/apiconfig/references/api/v2",
    open_access: false,
  },
  {
    provider: "musicbrainz",
    label: "MusicBrainz (Music)",
    docs_url: "https://musicbrainz.org/doc/MusicBrainz_API",
    open_access: true,
  },
  {
    provider: "steam",
    label: "Steam Web API (Games)",
    docs_url: "https://partner.steamgames.com/doc/webapi_overview",
    open_access: true,
  },
  {
    provider: "rawg",
    label: "RAWG Video Games Database",
    docs_url: "https://rawg.io/apidocs",
    open_access: false,
  },
  {
    provider: "igdb",
    label: "IGDB (Games)",
    docs_url: "https://api-docs.igdb.com",
    open_access: false,
  },
  {
    provider: "comicvine",
    label: "ComicVine (Comics)",
    docs_url: "https://comicvine.gamespot.com/api/documentation",
    open_access: false,
  },
  {
    provider: "podcast-index",
    label: "Podcast Index (Podcasts)",
    docs_url: "https://podcastindex-org.github.io/docs-api",
    open_access: false,
  },
];

const searchCache = new Map<
  string,
  { timestamp: number; data: ProviderSearchCandidate[] }
>();
const inFlightSearches = new Map<string, Promise<ProviderSearchCandidate[]>>();

function applyRpdbIfNeeded(
  candidates: ProviderSearchCandidate[],
  rpdbKey?: string,
): ProviderSearchCandidate[] {
  if (!rpdbKey || rpdbKey.trim().length === 0) return candidates;
  const key = rpdbKey.trim();
  return candidates.map((cand) => {
    if (cand.provider === "tmdb" && cand.provider_id) {
      return {
        ...cand,
        image_url: `https://api.ratingposterdb.com/${key}/tmdb/poster-default/${cand.provider_id}.jpg`,
      };
    }
    const imdbXid = cand.external_ids?.find(
      (x) => x.namespace === "imdb_title" || x.namespace === "imdb",
    );
    if (imdbXid?.value) {
      return {
        ...cand,
        image_url: `https://api.ratingposterdb.com/${key}/imdb/poster-default/${imdbXid.value}.jpg`,
      };
    }
    return cand;
  });
}

function getStoredCredentials(): Record<string, string> {
  if (typeof localStorage === "undefined") return {};
  try {
    const raw = localStorage.getItem(CREDENTIALS_STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function setStoredCredentials(creds: Record<string, string>): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(CREDENTIALS_STORAGE_KEY, JSON.stringify(creds));
  } catch {
    // ignore
  }
}

export function createWebHost(defaultApiUrl: string): WorkbenchHost {
  return {
    async loadNetworkConfiguration(): Promise<NetworkConfiguration> {
      if (typeof localStorage !== "undefined") {
        try {
          const raw = localStorage.getItem(NETWORK_STORAGE_KEY);
          if (raw) return JSON.parse(raw);
        } catch {
          // fallback
        }
      }
      return {
        connection: {
          service_url: {
            value: defaultApiUrl || "http://127.0.0.1:8420",
            source: "default",
            managed: false,
          },
          public_url: {
            value: null,
            source: "default",
            managed: false,
          },
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
        } catch {
          // ignore
        }
      }
      return config;
    },

    async testEndpointConnection(
      endpoint: string,
    ): Promise<EndpointConnectionStatus> {
      const scheme = endpoint.startsWith("https") ? "https" : "http";
      const normalized = endpoint.replace(/\/+$/, "");
      const res = await fetch(`${normalized}/api/v1/health`, {
        signal: AbortSignal.timeout(3000),
      });
      if (!res.ok) {
        throw new Error(`Endpoint returned status ${res.status}`);
      }
      const data = await res.json();
      return {
        endpoint: normalized,
        scheme,
        status: data.status ?? "healthy",
        version: data.version ?? "0.1.0",
      };
    },

    async providerCredentialStatus(): Promise<ProviderCredentialStatus[]> {
      const stored = getStoredCredentials();
      return SUPPORTED_PROVIDERS.map((p) => {
        const hasKey = Boolean(stored[p.provider]);
        return {
          provider: p.provider,
          label: p.label,
          configured: p.open_access || hasKey,
          source: hasKey ? "credential_store" : "none",
          writable: true,
          docs_url: p.docs_url,
        };
      });
    },

    async saveProviderCredential(
      provider: string,
      credential: string,
    ): Promise<ProviderCredentialStatus[]> {
      const stored = getStoredCredentials();
      stored[provider] = credential.trim();
      setStoredCredentials(stored);
      return this.providerCredentialStatus();
    },

    async deleteProviderCredential(
      provider: string,
    ): Promise<ProviderCredentialStatus[]> {
      const stored = getStoredCredentials();
      delete stored[provider];
      setStoredCredentials(stored);
      return this.providerCredentialStatus();
    },

    async searchProvider(
      provider: string,
      query: string,
    ): Promise<ProviderSearchCandidate[]> {
      const trimmed = query.trim();
      if (!trimmed) return [];
      const stored = getStoredCredentials();

      const normalized = provider.toLowerCase().replace(/_/g, "-");
      const providerKey =
        normalized === "openlibrary"
          ? "open-library"
          : normalized === "googlebooks"
            ? "google-books"
            : normalized === "themoviedb"
              ? "tmdb"
              : normalized === "thetvdb"
                ? "tvdb"
                : normalized;

      const apiKey = stored[providerKey] || stored[provider] || "";

      // In-flight promise pooling & local in-memory cache
      const cacheKey = `${providerKey}:${trimmed.toLowerCase()}`;
      const now = Date.now();
      const cached = searchCache.get(cacheKey);

      // Return valid cached results within 15 mins (900,000 ms) to limit calls & eliminate rate limits
      if (cached && now - cached.timestamp < 900000) {
        return applyRpdbIfNeeded(cached.data, stored["rpdb"]);
      }

      // If a search is already in flight, reuse its promise
      if (inFlightSearches.has(cacheKey)) {
        const res = await inFlightSearches.get(cacheKey)!;
        return applyRpdbIfNeeded(res, stored["rpdb"]);
      }

      const executeSearch = async (): Promise<ProviderSearchCandidate[]> => {
        // Multi / Auto Search Across Core Providers
        if (
          providerKey === "all" ||
          providerKey === "auto" ||
          providerKey === "multi"
        ) {
          const activeQueries: Promise<ProviderSearchCandidate[]>[] = [
            this.searchProvider("kitsu", trimmed).catch(() => []),
            this.searchProvider("open-library", trimmed).catch(() => []),
            this.searchProvider("anilist", trimmed).catch(() => []),
            this.searchProvider("mal", trimmed).catch(() => []),
            this.searchProvider("steam", trimmed).catch(() => []),
            this.searchProvider("musicbrainz", trimmed).catch(() => []),
          ];
          if (stored["tmdb"]) {
            activeQueries.push(
              this.searchProvider("tmdb", trimmed).catch(() => []),
            );
          }
          if (stored["tvdb"]) {
            activeQueries.push(
              this.searchProvider("tvdb", trimmed).catch(() => []),
            );
          }
          if (stored["google-books"]) {
            activeQueries.push(
              this.searchProvider("google-books", trimmed).catch(() => []),
            );
          }
          if (stored["rawg"]) {
            activeQueries.push(
              this.searchProvider("rawg", trimmed).catch(() => []),
            );
          }
          if (stored["comicvine"]) {
            activeQueries.push(
              this.searchProvider("comicvine", trimmed).catch(() => []),
            );
          }
          if (stored["podcast-index"]) {
            activeQueries.push(
              this.searchProvider("podcast-index", trimmed).catch(() => []),
            );
          }
          const settled = await Promise.allSettled(activeQueries);
          const aggregated: ProviderSearchCandidate[] = [];
          for (const res of settled) {
            if (res.status === "fulfilled") {
              aggregated.push(...res.value);
            }
          }
          return aggregated;
        }

        // 1. Open Library (Books)
        if (providerKey === "open-library") {
          const res = await fetch(
            `https://openlibrary.org/search.json?q=${encodeURIComponent(trimmed)}&limit=12`,
            { signal: AbortSignal.timeout(10000) },
          );
          if (!res.ok)
            throw new Error(`Open Library failed: ${res.statusText}`);
          const data = await res.json();
          return (data.docs || []).slice(0, 12).map((doc: any) => ({
            provider: "open-library",
            provider_id:
              doc.key?.replace("/works/", "") ||
              doc.edition_key?.[0] ||
              String(doc.cover_i ?? ""),
            title: doc.title || "Untitled",
            kind: "book",
            release_year: doc.first_publish_year,
            authors: doc.author_name || [],
            image_url: doc.cover_i
              ? `https://covers.openlibrary.org/b/id/${doc.cover_i}-M.jpg`
              : null,
            overview: Array.isArray(doc.first_sentence)
              ? doc.first_sentence[0]
              : doc.subtitle || undefined,
            external_ids: [
              ...(doc.isbn
                ? [
                    {
                      namespace: "isbn",
                      value: doc.isbn[0],
                      status: "matched" as const,
                      source: "open-library",
                    },
                  ]
                : []),
            ],
          }));
        }

        // 2. Kitsu (Anime & Manga) — Prioritize English title, store Romaji/Japanese in original_title
        if (providerKey === "kitsu") {
          const res = await fetch(
            `https://kitsu.io/api/edge/anime?filter[text]=${encodeURIComponent(trimmed)}&page[limit]=12`,
            { signal: AbortSignal.timeout(10000) },
          );
          if (!res.ok)
            throw new Error(`Kitsu request failed: ${res.statusText}`);
          const data = await res.json();
          return (data.data || []).map((item: any) => {
            const attr = item.attributes || {};
            const englishTitle = attr.titles?.en;
            const canonicalTitle = attr.canonicalTitle || attr.titles?.en_jp;
            const japaneseTitle = attr.titles?.ja_jp;

            // Default to English title if available, otherwise canonical/romaji
            const displayTitle = englishTitle || canonicalTitle || "Untitled";
            const originalTitle =
              japaneseTitle ||
              (englishTitle && canonicalTitle !== englishTitle
                ? canonicalTitle
                : undefined);

            return {
              provider: "kitsu",
              provider_id: String(item.id),
              title: displayTitle,
              original_title: originalTitle,
              kind: attr.subtype === "manga" ? "manga" : "anime",
              release_year: attr.startDate
                ? parseInt(attr.startDate.slice(0, 4), 10)
                : undefined,
              authors: [
                attr.showType || "TV",
                ...(attr.ageRatingGuide ? [attr.ageRatingGuide] : []),
              ],
              image_url:
                attr.posterImage?.medium || attr.posterImage?.small || null,
              overview: attr.synopsis || undefined,
              external_ids: [
                {
                  namespace: "kitsu",
                  value: String(item.id),
                  status: "matched" as const,
                  source: "kitsu",
                },
              ],
            };
          });
        }

        // 3. MyAnimeList (MAL via Jikan v4 Open API + official fallback)
        if (providerKey === "mal" || providerKey === "myanimelist") {
          const res = await fetch(
            `https://api.jikan.moe/v4/anime?q=${encodeURIComponent(trimmed)}&limit=12`,
            { signal: AbortSignal.timeout(10000) },
          );
          if (!res.ok)
            throw new Error(`MyAnimeList query failed: ${res.statusText}`);
          const data = await res.json();
          return (data.data || []).map((item: any) => {
            const englishTitle = item.title_english;
            const romajiTitle = item.title;
            const japaneseTitle = item.title_japanese;

            const displayTitle = englishTitle || romajiTitle || "Untitled";
            const originalTitle =
              japaneseTitle ||
              (englishTitle && romajiTitle !== englishTitle
                ? romajiTitle
                : undefined);

            return {
              provider: "mal",
              provider_id: String(item.mal_id),
              title: displayTitle,
              original_title: originalTitle,
              kind: "anime",
              release_year:
                item.year ||
                (item.aired?.from
                  ? parseInt(item.aired.from.slice(0, 4), 10)
                  : undefined),
              authors: [
                item.type || "TV",
                ...(item.score ? [`★ ${item.score}`] : []),
              ],
              image_url:
                item.images?.jpg?.large_image_url ||
                item.images?.jpg?.image_url ||
                null,
              overview: item.synopsis || undefined,
              external_ids: [
                {
                  namespace: "mal",
                  value: String(item.mal_id),
                  status: "matched" as const,
                  source: "mal_api",
                  url: `https://myanimelist.net/anime/${item.mal_id}`,
                },
              ],
            };
          });
        }

        // 4. Google Books
        if (providerKey === "google-books") {
          const url = `https://www.googleapis.com/books/v1/volumes?q=${encodeURIComponent(trimmed)}&maxResults=12${apiKey ? `&key=${apiKey}` : ""}`;
          const res = await fetch(url, { signal: AbortSignal.timeout(10000) });
          if (!res.ok)
            throw new Error(`Google Books search failed: ${res.statusText}`);
          const data = await res.json();
          return (data.items || []).map((item: any) => {
            const info = item.volumeInfo || {};
            return {
              provider: "google-books",
              provider_id: String(item.id),
              title: info.title || "Untitled",
              kind: "book",
              release_year: info.publishedDate
                ? parseInt(info.publishedDate.slice(0, 4), 10)
                : undefined,
              authors: info.authors || [],
              image_url:
                info.imageLinks?.thumbnail?.replace("http://", "https://") ||
                null,
              overview: info.description || undefined,
              external_ids: [
                {
                  namespace: "google-books",
                  value: String(item.id),
                  status: "matched" as const,
                  source: "google-books",
                },
              ],
            };
          });
        }

        // 5. MusicBrainz (Music)
        if (providerKey === "musicbrainz") {
          const res = await fetch(
            `https://musicbrainz.org/ws/2/release/?query=${encodeURIComponent(trimmed)}&fmt=json&limit=12`,
            {
              headers: { "User-Agent": "Fasti/0.1.0 (https://fasti.dev)" },
              signal: AbortSignal.timeout(10000),
            },
          );
          if (!res.ok)
            throw new Error(`MusicBrainz request failed: ${res.statusText}`);
          const data = await res.json();
          return (data.releases || []).map((rel: any) => ({
            provider: "musicbrainz",
            provider_id: rel.id,
            title: rel.title || "Untitled",
            kind: "music",
            release_year: rel.date
              ? parseInt(rel.date.slice(0, 4), 10)
              : undefined,
            authors: (rel["artist-credit"] || []).map((a: any) => a.name),
            image_url: null,
            overview: rel.status
              ? `Status: ${rel.status} · Country: ${rel.country || "Global"}`
              : undefined,
            external_ids: [
              {
                namespace: "musicbrainz",
                value: rel.id,
                status: "matched" as const,
                source: "musicbrainz",
              },
            ],
          }));
        }

        // 6. Steam (Games)
        if (providerKey === "steam") {
          const res = await fetch(
            `https://store.steampowered.com/api/storesearch/?term=${encodeURIComponent(trimmed)}&l=english&cc=US`,
            { signal: AbortSignal.timeout(10000) },
          );
          if (!res.ok)
            throw new Error(`Steam search failed: ${res.statusText}`);
          const data = await res.json();
          return (data.items || []).slice(0, 12).map((item: any) => ({
            provider: "steam",
            provider_id: String(item.id),
            title: item.name || "Untitled",
            kind: "game",
            authors: item.platforms
              ? Object.keys(item.platforms).filter((k) => item.platforms[k])
              : ["PC"],
            image_url: item.tiny_image || null,
            overview: item.price
              ? `Price: ${item.price.final ? `$${(item.price.final / 100).toFixed(2)}` : "Free"}`
              : undefined,
            external_ids: [
              {
                namespace: "steam",
                value: String(item.id),
                status: "matched" as const,
                source: "steam",
              },
            ],
          }));
        }

        // 7. AniList (GraphQL)
        if (providerKey === "anilist") {
          const gql = `query ($search: String) { Page(page: 1, perPage: 12) { media(search: $search, sort: SEARCH_MATCH) { id title { romaji english native } type format seasonYear coverImage { medium large } description(asHtml: false) genres } } }`;
          const res = await fetch("https://graphql.anilist.co", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              query: gql,
              variables: { search: trimmed },
            }),
            signal: AbortSignal.timeout(10000),
          });
          if (!res.ok)
            throw new Error(`AniList search failed: ${res.statusText}`);
          const data = await res.json();
          return (data.data?.Page?.media || []).map((m: any) => {
            const englishTitle = m.title?.english;
            const romajiTitle = m.title?.romaji;
            const nativeTitle = m.title?.native;

            const displayTitle = englishTitle || romajiTitle || "Untitled";
            const originalTitle =
              nativeTitle ||
              (englishTitle && romajiTitle !== englishTitle
                ? romajiTitle
                : undefined);

            return {
              provider: "anilist",
              provider_id: String(m.id),
              title: displayTitle,
              original_title: originalTitle,
              kind: m.type === "MANGA" ? "manga" : "anime",
              release_year: m.seasonYear || undefined,
              authors: [
                m.format || m.type,
                ...(m.genres ? m.genres.slice(0, 2) : []),
              ],
              image_url: m.coverImage?.large || m.coverImage?.medium || null,
              overview: m.description || undefined,
              external_ids: [
                {
                  namespace: "anilist",
                  value: String(m.id),
                  status: "matched" as const,
                  source: "anilist",
                },
              ],
            };
          });
        }

        // 8. RAWG (Video Games)
        if (providerKey === "rawg") {
          if (!apiKey) {
            throw new Error(
              "RAWG requires an API Key. Configure it in Settings & Studio (https://rawg.io/apidocs).",
            );
          }
          const res = await fetch(
            `https://api.rawg.io/api/games?key=${apiKey}&search=${encodeURIComponent(trimmed)}&page_size=12`,
            { signal: AbortSignal.timeout(10000) },
          );
          if (!res.ok) throw new Error(`RAWG search failed: ${res.statusText}`);
          const data = await res.json();
          return (data.results || []).map((g: any) => ({
            provider: "rawg",
            provider_id: String(g.id),
            title: g.name || "Untitled",
            kind: "game",
            release_year: g.released
              ? parseInt(g.released.slice(0, 4), 10)
              : undefined,
            authors: (g.genres || []).map((gen: any) => gen.name),
            image_url: g.background_image || null,
            overview: g.rating
              ? `Rating: ${g.rating}/5 (${g.ratings_count} reviews)`
              : undefined,
            external_ids: [
              {
                namespace: "rawg",
                value: String(g.id),
                status: "matched" as const,
                source: "rawg",
              },
            ],
          }));
        }

        // 9. TMDB (Movies & Shows) — Supports v3 API Key & v4 Bearer Read Access Token
        if (providerKey === "tmdb") {
          if (!apiKey) {
            throw new Error(
              "TMDB requires an API Key or Read Access Token. Please configure it in Settings & Studio -> Metadata Providers & Keys.",
            );
          }

          const isBearer = apiKey.startsWith("eyJ");
          const url = isBearer
            ? `https://api.themoviedb.org/3/search/multi?query=${encodeURIComponent(trimmed)}&language=en-US&include_adult=false`
            : `https://api.themoviedb.org/3/search/multi?api_key=${apiKey}&query=${encodeURIComponent(trimmed)}&language=en-US&include_adult=false`;

          const headers: Record<string, string> = isBearer
            ? {
                Authorization: `Bearer ${apiKey}`,
                "Content-Type": "application/json",
              }
            : {};

          const res = await fetch(url, {
            headers,
            signal: AbortSignal.timeout(10000),
          });
          if (!res.ok) throw new Error(`TMDB search failed: ${res.statusText}`);
          const data = await res.json();
          return (data.results || [])
            .filter(
              (r: any) => r.media_type === "movie" || r.media_type === "tv",
            )
            .slice(0, 12)
            .map((r: any) => {
              const isMovie = r.media_type === "movie";
              const yearStr =
                (isMovie ? r.release_date : r.first_air_date) || "";
              const englishTitle = (isMovie ? r.title : r.name) || "Untitled";
              const originalTitle = isMovie
                ? r.original_title
                : r.original_name;

              return {
                provider: "tmdb",
                provider_id: String(r.id),
                title: englishTitle,
                original_title:
                  originalTitle && originalTitle !== englishTitle
                    ? originalTitle
                    : undefined,
                kind: isMovie ? "movie" : "show",
                release_year: yearStr
                  ? parseInt(yearStr.slice(0, 4), 10)
                  : undefined,
                authors: [
                  isMovie ? "Movie" : "TV Show",
                  ...(r.vote_average ? [`★ ${r.vote_average.toFixed(1)}`] : []),
                ],
                image_url: r.poster_path
                  ? `https://image.tmdb.org/t/p/w342${r.poster_path}`
                  : null,
                overview: r.overview || undefined,
                external_ids: [
                  {
                    namespace: isMovie ? "tmdb_movie" : "tmdb_tv",
                    value: String(r.id),
                    status: "matched" as const,
                    source: "tmdb",
                  },
                ],
              };
            });
        }

        // 10. TheTVDB v4
        if (providerKey === "tvdb") {
          const res = await fetch(
            `https://api.thetvdb.com/search/series?name=${encodeURIComponent(trimmed)}`,
            { signal: AbortSignal.timeout(10000) },
          ).catch(() => null);

          if (res && res.ok) {
            const data = await res.json();
            return (data.data || []).slice(0, 12).map((item: any) => ({
              provider: "tvdb",
              provider_id: String(item.id),
              title: item.seriesName || "Untitled",
              kind: "show",
              release_year: item.firstAired
                ? parseInt(item.firstAired.slice(0, 4), 10)
                : undefined,
              authors: ["TV Show", ...(item.network ? [item.network] : [])],
              image_url: item.banner
                ? `https://thetvdb.com/banners/${item.banner}`
                : null,
              overview: item.overview || undefined,
              external_ids: [
                {
                  namespace: "tvdb_series",
                  value: String(item.id),
                  status: "matched" as const,
                  source: "tvdb",
                },
              ],
            }));
          }
          return [];
        }

        // 11. ComicVine (Comics)
        if (providerKey === "comicvine") {
          if (!apiKey) {
            throw new Error(
              "ComicVine requires an API Key. Configure it in Settings & Studio (https://comicvine.gamespot.com/api/).",
            );
          }
          const res = await fetch(
            `https://comicvine.gamespot.com/api/search/?api_key=${apiKey}&format=json&resources=volume&query=${encodeURIComponent(trimmed)}&limit=12`,
            { signal: AbortSignal.timeout(10000) },
          );
          if (!res.ok)
            throw new Error(`ComicVine search failed: ${res.statusText}`);
          const data = await res.json();
          return (data.results || []).map((v: any) => ({
            provider: "comicvine",
            provider_id: String(v.id),
            title: v.name || "Untitled",
            kind: "comic",
            release_year: v.start_year
              ? parseInt(v.start_year, 10)
              : undefined,
            authors: v.publisher?.name ? [v.publisher.name] : [],
            image_url: v.image?.medium_url || null,
            overview: v.deck || undefined,
            external_ids: [
              {
                namespace: "comicvine",
                value: String(v.id),
                status: "matched" as const,
                source: "comicvine",
              },
            ],
          }));
        }

        // 12. Podcast Index (Podcasts) -- key stored as "apiKey:apiSecret"
        if (providerKey === "podcast-index") {
          const [podcastKey, podcastSecret] = apiKey.split(":");
          if (!podcastKey || !podcastSecret) {
            throw new Error(
              'Podcast Index requires an API Key and Secret, entered as "key:secret". Configure it in Settings & Studio (https://podcastindex.org/apps).',
            );
          }
          const authTime = Math.floor(Date.now() / 1000).toString();
          const hashInput = new TextEncoder().encode(
            podcastKey + podcastSecret + authTime,
          );
          const hashBuffer = await crypto.subtle.digest("SHA-1", hashInput);
          const authHash = Array.from(new Uint8Array(hashBuffer))
            .map((b) => b.toString(16).padStart(2, "0"))
            .join("");
          const res = await fetch(
            `https://api.podcastindex.org/api/1.0/search/byterm?q=${encodeURIComponent(trimmed)}`,
            {
              headers: {
                "X-Auth-Date": authTime,
                "X-Auth-Key": podcastKey,
                Authorization: authHash,
                "User-Agent": "Fasti/0.1",
              },
              signal: AbortSignal.timeout(10000),
            },
          );
          if (!res.ok)
            throw new Error(`Podcast Index search failed: ${res.statusText}`);
          const data = await res.json();
          return (data.feeds || []).slice(0, 12).map((f: any) => ({
            provider: "podcast-index",
            provider_id: String(f.id),
            title: f.title || "Untitled",
            kind: "podcast",
            release_year: f.newestItemPublishTime
              ? new Date(f.newestItemPublishTime * 1000).getFullYear()
              : undefined,
            authors: f.author ? [f.author] : [],
            image_url: f.image || f.artwork || null,
            overview: f.description || undefined,
            external_ids: [
              {
                namespace: "podcast_index",
                value: String(f.id),
                status: "matched" as const,
                source: "podcast-index",
              },
            ],
          }));
        }

        // IGDB is intentionally not implemented here: igdb.com requires a
        // Twitch client_id/client_secret OAuth exchange and does not send
        // CORS headers on its query endpoint, so it cannot work from a
        // browser tab regardless of credentials entered. A real IGDB
        // integration needs a native host doing the request server-side
        // (see apps/desktop/src-tauri/src/providers.rs for that pattern),
        // not this web-only shell.
        throw new Error(
          `Provider '${provider}' is not configured for live search.`,
        );
      };

      const inFlightPromise = executeSearch();
      inFlightSearches.set(cacheKey, inFlightPromise);

      try {
        const results = await inFlightPromise;
        searchCache.set(cacheKey, { timestamp: Date.now(), data: results });
        return applyRpdbIfNeeded(results, stored["rpdb"]);
      } catch (err) {
        // If query failed due to network / rate limit, fallback to stale cache if available
        if (cached?.data?.length) {
          return applyRpdbIfNeeded(cached.data, stored["rpdb"]);
        }
        throw err;
      } finally {
        inFlightSearches.delete(cacheKey);
      }
    },
    clearSearchCache: () => {
      searchCache.clear();
    },
    getSearchCacheSize: () => {
      return searchCache.size;
    },
  };
}
