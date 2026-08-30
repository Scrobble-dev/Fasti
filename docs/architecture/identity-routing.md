# Purpose-specific identity routing

Fasti does not have a primary external media identifier. A stable `RecordId`
owns the local identity. Provider identifiers remain evidence used for one
named operation.

## Resolution intents

The routing contract uses these intents:

- `metadata_search`
- `metadata_lookup`
- `metadata_enrichment`
- `rating_lookup`
- `catalog_lookup`
- `display_projection`
- `nuvio_export`
- `nuvio_import_attachment`
- `tracker_read`
- `tracker_write`
- `segment_translation`
- `deduplication_review`

An accepted read alias does not authorize a provider write. Tracker writes
require the target provider's own identifier. Missing and equally ranked routes
fail closed. The route plan retains all known identifiers for inspection.

## TMDB route order

TMDB metadata lookup and enrichment use this order:

1. A compatible `tmdb.movie` or `tmdb.tv` identifier.
2. A compatible `imdb.title` alias through TMDB Find.
3. A compatible TVDB-series or Wikidata alias through TMDB Find. TMDB does
   not advertise TheTVDB as a movie lookup source.
4. A reviewed and accepted crosswalk assertion to TMDB.

Title and year similarity is not an identity route. It can produce a review
candidate, but it cannot attach an identifier or change a Record.

Known provider identifiers must also match the provider's bounded wire format.
Malformed evidence stays visible for review but cannot become an outbound
route.

The checked-in golden fixtures include the NuvioTV issue where `mal:49894`
also carries IMDb identifier `tt28254942`. TMDB uses the IMDb alias. The MAL
identifier and Fasti Record remain unchanged.

## Anime grouping and export preference

The profile preference has four values:

| Value | Behavior |
| --- | --- |
| `group_by_tv_work` | Prefer an IMDb, TMDB, or TVDB work-style coordinate. |
| `keep_mal_releases_separate` | Prefer MAL release identifiers. |
| `keep_kitsu_releases_separate` | Prefer Kitsu release identifiers. |
| `automatic` | Use the connection compatibility profile. Nuvio uses its pinned standard order. |

For Nuvio, the standard order is IMDb, TMDB, TVDB, MAL, AniDB, AniList,
Kitsu, then SIMKL. The MAL preference tries MAL, Kitsu, and AniDB before the
standard order. The Kitsu preference tries Kitsu, MAL, and AniDB before the
standard order.

A reviewed crosswalk can satisfy the selected coordinate preference. The
selected route retains the immutable assertion ID for inspection. Direct
evidence for the same coordinate wins; an accepted preferred crosswalk can win
over a less-preferred direct fallback. Candidate or disputed mappings cannot
enter the route planner as accepted evidence.

The shared encoder keeps IMDb values bare and prefixes the other coordinate
spaces: `tmdb:`, `tvdb:`, `mal:`, `anidb:`, `anilist:`, `kitsu:`, or `simkl:`.
Adapters consume this encoder instead of maintaining their own prefix tables.

The preference can change outward catalog identifiers, grouping, deep links,
and safe provider routes. It cannot change:

- `RecordId`;
- original observations;
- Chronicle occurrences;
- prior interpretations; or
- accepted external identity evidence.

## Preview and rollback

A change is previewed against the authenticated profile before it is applied.
The preview reports:

- total and affected Records;
- the previous and proposed route for each returned Record;
- unresolved or ambiguous routes;
- possible season regrouping;
- the current policy revision; and
- a keyset cursor for the next bounded impact page.

Apply uses an operation ID, a semantic digest, and the expected policy
revision. The durable implementation stores an immutable receipt. Rollback is
a new compare-and-set operation that refers to the receipt being reversed. It
does not delete the original receipt or rewrite media history.

## Source evidence

- [Nuvio Desktop pinned projection rules](https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/simkl/SimklProjections.kt)
- [Nuvio Desktop pinned anime routing tests](https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonTest/kotlin/com/nuvio/app/features/simkl/SimklAnimeWatchedResolutionTest.kt)
- [NuvioTV issue 2742](https://github.com/NuvioMedia/NuvioTV/issues/2742)
- [TMDB Find by ID](https://developer.themoviedb.org/reference/find-by-id)
- [Electric Town anime crosswalk doctrine](https://github.com/Electric-Town/anime-crosswalk-mappings/tree/dee4c1f4808d656b7ca71da584a8af95a2653277)
