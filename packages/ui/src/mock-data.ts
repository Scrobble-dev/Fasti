import type {
  MediaRecord,
  ChronicleOccurrence,
  ReconciliationCase,
  CustomFieldDefinition,
  ScopedApiToken,
} from "./types.js";

export const SAMPLE_RECORDS: MediaRecord[] = [
  {
    id: "rec_01K89Z01FrierenAnime",
    title: "Frieren: Beyond Journey's End",
    originalTitle: "Sousou no Frieren",
    mediaKind: "anime",
    releaseYear: 2023,
    overview:
      "The adventure is over but life goes on for an elf mage just beginning to learn what living is all about.",
    posterUrl:
      "https://images.unsplash.com/photo-1578632767115-351597cf2477?w=400&q=80",
    backdropUrl:
      "https://images.unsplash.com/photo-1534447677768-be436bb09401?w=1200&q=80",
    status: "watching",
    userRating: 10,
    progressEpisodes: 24,
    totalEpisodes: 28,
    displaySource: "tmdb_tv",
    tags: ["Fantasy", "Slice of Life", "Elves", "Adventure"],
    lastActivityAt: "2026-08-24T20:15:00Z",
    externalIds: [
      {
        namespace: "tmdb_tv",
        value: "209867",
        status: "matched",
        source: "tmdb_api",
      },
      {
        namespace: "kitsu_anime",
        value: "46001",
        status: "matched",
        source: "simkl_import",
      },
      {
        namespace: "mal_anime",
        value: "52991",
        status: "matched",
        source: "anibridge_pack",
      },
      {
        namespace: "imdb_title",
        value: "tt22238804",
        status: "matched",
        source: "simkl_import",
      },
    ],
    userNotes:
      "Exemplary pacing, sound design, and emotional restraint. Season 1 finale was perfection.",
    seasons: [
      {
        seasonNumber: 1,
        title: "Season 1",
        episodes: [
          {
            id: "ep_1",
            number: 1,
            seasonNumber: 1,
            title: "The Journey's End",
            durationSeconds: 1440,
            watched: true,
            watchedAt: "2026-08-10T19:00:00Z",
          },
          {
            id: "ep_2",
            number: 2,
            seasonNumber: 1,
            title: "It Didn't Have to Be Magic...",
            durationSeconds: 1440,
            watched: true,
            watchedAt: "2026-08-11T20:00:00Z",
          },
          {
            id: "ep_24",
            number: 24,
            seasonNumber: 1,
            title: "Perfect Replication",
            durationSeconds: 1440,
            watched: true,
            watchedAt: "2026-08-24T20:15:00Z",
          },
          {
            id: "ep_25",
            number: 25,
            seasonNumber: 1,
            title: "A Fatal Vulnerability",
            durationSeconds: 1440,
            watched: false,
          },
          {
            id: "ep_26",
            number: 26,
            seasonNumber: 1,
            title: "The Height of Magic",
            durationSeconds: 1440,
            watched: false,
          },
        ],
      },
    ],
  },
  {
    id: "rec_01K89Z02SeveranceShow",
    title: "Severance",
    mediaKind: "show",
    releaseYear: 2022,
    overview:
      "Mark leads a team of office workers whose memories have been surgically divided between their work and personal lives.",
    posterUrl:
      "https://images.unsplash.com/photo-1486406146926-c627a92ad1ab?w=400&q=80",
    backdropUrl:
      "https://images.unsplash.com/photo-1497366216548-37526070297c?w=1200&q=80",
    status: "watching",
    userRating: 9,
    progressEpisodes: 9,
    totalEpisodes: 10,
    displaySource: "tvdb",
    tags: ["Sci-Fi", "Psychological Thriller", "Mystery"],
    lastActivityAt: "2026-08-22T21:40:00Z",
    externalIds: [
      {
        namespace: "tmdb_tv",
        value: "95396",
        status: "matched",
        source: "tmdb_api",
      },
      {
        namespace: "tvdb_series",
        value: "371980",
        status: "matched",
        source: "tvdb_api",
      },
      {
        namespace: "imdb_title",
        value: "tt11280740",
        status: "matched",
        source: "plex_webhook",
      },
    ],
    userNotes:
      "Lumon Industries architecture and cinematography is mesmerizing.",
  },
  {
    id: "rec_01K89Z03DunePartTwo",
    title: "Dune: Part Two",
    mediaKind: "movie",
    releaseYear: 2024,
    overview:
      "Paul Atreides unites with Chani and the Fremen while seeking revenge against the conspirators who destroyed his family.",
    posterUrl:
      "https://images.unsplash.com/photo-1509198397868-475647b2a1e5?w=400&q=80",
    backdropUrl:
      "https://images.unsplash.com/photo-1506744038136-46273834b3fb?w=1200&q=80",
    status: "completed",
    userRating: 10,
    progressSeconds: 9960,
    totalDurationSeconds: 9960,
    displaySource: "tmdb_movie",
    tags: ["Sci-Fi", "Epic", "Adventure"],
    lastActivityAt: "2026-08-18T22:30:00Z",
    externalIds: [
      {
        namespace: "tmdb_movie",
        value: "693134",
        status: "matched",
        source: "tmdb_api",
      },
      {
        namespace: "imdb_title",
        value: "tt15239678",
        status: "matched",
        source: "jellyfin_webhook",
      },
    ],
  },
  {
    id: "rec_01K89Z04NeuromancerBook",
    title: "Neuromancer",
    mediaKind: "book",
    releaseYear: 1984,
    overview:
      "Case had been the sharpest data-thief in the business, until he crossed the wrong people and they crippled his nervous system.",
    posterUrl:
      "https://images.unsplash.com/photo-1544716278-ca5e3f4abd8c?w=400&q=80",
    status: "completed",
    userRating: 9,
    displaySource: "google_books",
    tags: ["Cyberpunk", "Classic Sci-Fi", "AI"],
    lastActivityAt: "2026-08-15T18:00:00Z",
    externalIds: [
      {
        namespace: "isbn_13",
        value: "9780441569595",
        status: "matched",
        source: "open_library",
      },
      {
        namespace: "google_books",
        value: "vH_lAgAAQBAJ",
        status: "matched",
        source: "google_books_api",
      },
    ],
  },
  {
    id: "rec_01K89Z05EldenRingGame",
    title: "Elden Ring: Shadow of the Erdtree",
    mediaKind: "game",
    releaseYear: 2024,
    overview:
      "Guided by Empyrean Miquella, players are summoned to the Land of Shadow, a place obscured by the Erdtree where goddess Marika first set foot.",
    posterUrl:
      "https://images.unsplash.com/photo-1542751371-adc38448a05e?w=400&q=80",
    status: "watching",
    userRating: 10,
    progressSeconds: 144000,
    totalDurationSeconds: 180000,
    displaySource: "steam_app",
    tags: ["Action RPG", "Soulslike", "Open World", "Dark Fantasy"],
    lastActivityAt: "2026-08-25T01:00:00Z",
    externalIds: [
      {
        namespace: "steam_app",
        value: "2778580",
        status: "matched",
        source: "steam_api",
      },
      {
        namespace: "gog_product",
        value: "1928374",
        status: "matched",
        source: "user_override",
      },
    ],
  },
];

export const SAMPLE_CHRONICLE: ChronicleOccurrence[] = [
  {
    id: "occ_01K89Z11",
    recordId: "rec_01K89Z05EldenRingGame",
    title: "Elden Ring: Shadow of the Erdtree",
    mediaKind: "game",
    posterUrl:
      "https://images.unsplash.com/photo-1542751371-adc38448a05e?w=400&q=80",
    timestamp: "2026-08-25T01:00:00Z",
    progressPercentage: 80,
    durationMinutes: 120,
    deviceName: "Gaming Desktop (Linux/Wayland)",
    clientName: "Desktop MPRIS / Steam Hook",
    isRewatch: false,
    userRating: 10,
  },
  {
    id: "occ_01K89Z12",
    recordId: "rec_01K89Z01FrierenAnime",
    title: "Frieren: Beyond Journey's End",
    episodeTitle: "Perfect Replication",
    seasonNumber: 1,
    episodeNumber: 24,
    mediaKind: "anime",
    posterUrl:
      "https://images.unsplash.com/photo-1578632767115-351597cf2477?w=400&q=80",
    timestamp: "2026-08-24T20:15:00Z",
    progressPercentage: 100,
    durationMinutes: 24,
    deviceName: "Living Room NuvioTV",
    clientName: "Nuvio Ingest B7",
    isRewatch: false,
    userRating: 10,
  },
  {
    id: "occ_01K89Z13",
    recordId: "rec_01K89Z02SeveranceShow",
    title: "Severance",
    episodeTitle: "The We We Are",
    seasonNumber: 1,
    episodeNumber: 9,
    mediaKind: "show",
    posterUrl:
      "https://images.unsplash.com/photo-1486406146926-c627a92ad1ab?w=400&q=80",
    timestamp: "2026-08-22T21:40:00Z",
    progressPercentage: 100,
    durationMinutes: 48,
    deviceName: "Bedroom Kodi",
    clientName: "Jellyfin Webhook",
    isRewatch: false,
    userRating: 9,
  },
  {
    id: "occ_01K89Z14",
    recordId: "rec_01K89Z03DunePartTwo",
    title: "Dune: Part Two",
    mediaKind: "movie",
    posterUrl:
      "https://images.unsplash.com/photo-1509198397868-475647b2a1e5?w=400&q=80",
    timestamp: "2026-08-18T22:30:00Z",
    progressPercentage: 100,
    durationMinutes: 166,
    deviceName: "Living Room Apple TV",
    clientName: "Plex Webhook",
    isRewatch: true,
    userRating: 10,
  },
];

export const SAMPLE_RECONCILIATION: ReconciliationCase[] = [
  {
    id: "rec_case_01",
    recordId: "rec_01K89Z01FrierenAnime",
    title: "Frieren Mini Anime (Sousou no Frieren: ●● no Mahou)",
    mediaKind: "anime",
    suppliedIds: [
      {
        namespace: "kitsu_anime",
        value: "48102",
        status: "matched",
        source: "simkl_import",
      },
      {
        namespace: "imdb_title",
        value: "tt29273618",
        status: "matched",
        source: "simkl_import",
      },
    ],
    candidateId: "cand_01",
    candidateTitle: "Sousou no Frieren: Marumaru no Mahou",
    candidateNamespace: "mal_anime",
    candidateExternalId: "56885",
    candidatePosterUrl:
      "https://images.unsplash.com/photo-1578632767115-351597cf2477?w=400&q=80",
    matchingReasons: [
      "Exact Kitsu-linked crosswalk match",
      "Identical air schedule timeframe (Oct 2023 - Mar 2024)",
      "Shared native Japanese title",
      "Identical production studio (Madhouse)",
    ],
    conflictingFactors: [
      "MAL classifies this as a separate 11-episode Special release",
      "TVDB groups these mini-episodes into Season 0 (Specials)",
    ],
    status: "open",
  },
];

export const SAMPLE_CUSTOM_FIELDS: CustomFieldDefinition[] = [
  {
    key: "games.gog_product_id",
    label: "GOG Product ID",
    targetType: "game",
    valueType: "identifier",
    registeredNamespace: "gog_product",
    isFilterable: true,
  },
  {
    key: "books.hardcover_id",
    label: "Hardcover Book ID",
    targetType: "book",
    valueType: "identifier",
    registeredNamespace: "hardcover_book",
    isFilterable: true,
  },
  {
    key: "custom.personal_archive_shelf",
    label: "Physical Archive Shelf",
    targetType: "all",
    valueType: "string",
    isFilterable: true,
  },
];

export const SAMPLE_TOKENS: ScopedApiToken[] = [
  {
    id: "tok_01",
    name: "Living Room NuvioTV Ingest",
    tokenPrefix: "fst_pat_9a2f...",
    scopes: ["chronicle:write", "progress:read_write"],
    createdAt: "2026-08-20T10:00:00Z",
    lastUsedAt: "2026-08-25T01:00:00Z",
  },
  {
    id: "tok_02",
    name: "Home Assistant Local Bridge",
    tokenPrefix: "fst_pat_b17c...",
    scopes: ["chronicle:read", "metadata:read"],
    createdAt: "2026-08-15T14:30:00Z",
    lastUsedAt: "2026-08-24T22:00:00Z",
  },
];
