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
Its durable owner must load a bounded complete identifier and assertion set,
detect overflow with one extra row, and fail closed instead of truncating
evidence into an apparently complete plan.

## TMDB route order

TMDB metadata lookup and enrichment use this order:

1. A compatible `tmdb.movie` or `tmdb.tv` identifier.
2. A compatible `imdb.title` alias through TMDB Find.
3. A compatible TVDB-series or Wikidata alias through TMDB Find. TMDB does
   not advertise TheTVDB as a movie lookup source.
4. A reviewed and accepted crosswalk assertion to TMDB.

Title and year similarity is not an identity route. It can produce a review
candidate, but it cannot attach an identifier or change a Record.

## Crosswalk assertions

A crosswalk assertion is directional and immutable. It retains its source
identifier reference, target coordinate, relation, bounded coverage and episode
links, evidence class, acquisition route, source version, derivation root, and
initial lifecycle state. Corroboration requires at least two distinct known
non-heuristic derivation roots; title similarity does not count, and two mirrors
of one upstream source count once. Later acceptance, dispute, rejection, and
revocation are append-only lifecycle events.

The assertion derives its workspace, Record, source identifier reference, and
source coordinate from one `ExternalIdentifier` aggregate. Evidence cannot
postdate the assertion's server-owned creation time.

Only the effective `accepted` state can enter routing. An `exact` assertion can
support compatible read and export routes. A `subset_of` assertion can support a
Nuvio TV-work grouping projection when it carries explicit coverage; it is not a
general alias. `superset_of`, `overlaps`, `alternate_cut_of`, `related`, and
`not_same_as` remain review evidence and never become outbound routes.
Immutable `candidate` and `disputed` evidence classes cannot be promoted by a
lifecycle event; reviewed evidence is recorded as a new supported assertion.
Inferred mappings start as candidates and require an append-only review event
before they can become accepted routes.
An `asserted` mapping remains audit evidence until its named authority and
namespace scope have been verified against the pinned authority registry.
The planner also requires the assertion owner to match the requested `RecordId`;
evidence from another Record cannot enter that plan or become visible through it.

Known provider identifiers must also match the provider's bounded wire format.
Malformed evidence stays visible for review but cannot become an outbound
route.

The checked-in golden fixtures include the NuvioTV issue where `mal:49894`
also carries IMDb identifier `tt28254942`. TMDB uses the IMDb alias. The MAL
identifier and Fasti Record remain unchanged.

## Anime grouping and export preference

Each profile has a default preference. An authorized client, including a Nuvio
connection, can have an explicit override. Both scopes use the same four values:

A client without an override reads the profile default and its revision. The
response says whether the value is inherited or client-owned, so compare-and-set
apply detects a profile change between preview and apply. A client override can
be cleared explicitly to resume profile inheritance.

| Value | Behavior |
| --- | --- |
| `group_by_tv_work` | Prefer an IMDb, TMDB, or TVDB work-style coordinate. |
| `keep_mal_releases_separate` | Prefer MAL release identifiers. |
| `keep_kitsu_releases_separate` | Prefer Kitsu release identifiers. |
| `automatic` | Use the connection compatibility profile. Nuvio uses its pinned standard order. |

For Nuvio, the standard order is IMDb, TMDB, TVDB, MAL, AniDB, AniList,
Kitsu, then SIMKL. The MAL preference tries MAL, Kitsu, and AniDB before the
standard order. The Kitsu preference tries Kitsu, MAL, and AniDB before the
standard order. TMDB and TVDB movie coordinates route only at `Film` grain,
and their TV-series coordinates route only at `Series` grain. They are not
relabelled as anime `Release` coordinates for export.

A reviewed crosswalk can satisfy the selected coordinate preference. The
selected route retains every immutable supporting assertion ID for inspection.
Corroborating assertions for one coordinate do not create false ambiguity. Direct
evidence for the same coordinate wins; an accepted preferred crosswalk can win
over a less-preferred direct fallback. Candidate or disputed mappings cannot
enter the route planner as accepted evidence.

The shared encoder keeps IMDb values bare and prefixes the other coordinate
spaces: `tmdb:`, `tvdb:`, `mal:`, `anidb:`, `anilist:`, `kitsu:`, or `simkl:`.
Adapters consume this encoder instead of maintaining their own prefix tables.
The independent Scrob implementation at the pinned programme revision also
uses a validated bare IMDb identifier for outbound Nuvio library items and
skips an item when it cannot resolve a safe identifier. Fasti uses that as
behavioral corroboration, not as a code source or a reason to discard other
verified Nuvio-compatible identifiers.

The preference can change outward catalog identifiers, grouping, deep links,
and safe provider routes. It cannot change:

- `RecordId`;
- original observations;
- Chronicle occurrences;
- prior interpretations; or
- accepted external identity evidence.

## Preview and rollback

A policy read result is built from its exact authenticated query. It cannot
return another profile or profile-versus-client scope.

A change is previewed against the authenticated profile before it is applied.
The preview result is built from that exact query. Its profile and target
connection scope must match the query, and its proposed preference and
ownership source must match the requested `Set` or `InheritProfile` change;
rollback may restore either otherwise-valid state.
The preview reports:

- total and affected Records;
- the previous and proposed route for each returned Record;
- unresolved or ambiguous routes;
- possible season regrouping;
- the current policy revision; and
- a keyset cursor for the next bounded impact page.

Each page must advance beyond the requested cursor and must not exceed the
exact requested page limit.

Apply uses an operation ID, a semantic digest, and the expected policy
revision. Its result is built from that exact command and must retain the same
authenticated profile, target connection scope, operation ID, and change. The
durable implementation stores an immutable receipt. Rollback is a new
compare-and-set operation that refers to the receipt being reversed. It does
not delete the original receipt or rewrite media history.

## Source evidence

- [Nuvio Desktop pinned projection rules](https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/simkl/SimklProjections.kt)
- [Nuvio Desktop pinned anime routing tests](https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonTest/kotlin/com/nuvio/app/features/simkl/SimklAnimeWatchedResolutionTest.kt)
- [NuvioTV issue 2742](https://github.com/NuvioMedia/NuvioTV/issues/2742)
- [TMDB Find by ID](https://developer.themoviedb.org/reference/find-by-id)
- [Electric Town anime crosswalk doctrine](https://github.com/Electric-Town/anime-crosswalk-mappings/tree/dee4c1f4808d656b7ca71da584a8af95a2653277)
- [Scrob pinned Nuvio identifier benchmark](https://github.com/ellite/scrob/blob/1c4d775b70f489ca0531376b2c3de6a8c3de2a2b/backend/routers/sync.py#L554-L643)
