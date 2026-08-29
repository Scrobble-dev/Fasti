# Fasti — Fresh Blind Master Plan v2

**Product:** Fasti  
**Role:** Self-hosted, local-first media Chronicle, state authority, identity resolver, and interoperability service  
**Date:** 21 August 2026  
**Status:** Product and architecture baseline for repository foundation  
**Relationship:** Scrobble.dev is the neutral vocabulary, knowledge, schema, and conformance vehicle. Fasti is one implementation.  
**Supersession:** This document replaces the identity, metadata, provider, reconciliation, migration, and related roadmap sections of the earlier Fasti blueprint where they conflict. It re-tests the rest of the plan rather than assuming it is correct.

> **Historical source notice (29 August 2026):** This seed predates the
> TrailBase authentication programme. Its django-allauth issuer, local-auth,
> browser-account, and compatibility proposals are superseded. TrailBase is the
> selected separate human-account service. Fasti owns its subject links,
> browser sessions, authorization, profiles, grants, scopes, and application
> state. Keep the superseded text below as decision provenance; do not execute
> it as the current authentication plan.

---

## 0. Decision in one minute

> **Identity is not a provider field. It is a versioned body of evidence.**

Fasti must not have a mutable external “main identifier” [S2–S6]. Fasti assigns every media entity a stable local identifier. TMDB, TVDB, IMDb, MAL, Kitsu, AniList, AniDB, SIMKL, MusicBrainz, ISBN, Podcast GUID, Steam, GOG, and every future identifier are external coordinates attached through typed assertions. They are never the database identity of the user’s record.

The operating model is:

```text
Observed fact
    ↓
Stable Fasti entity
    ↓
External identifier assertions
    ↓
Typed topology and mapping assertions
    ↓
Purpose-specific resolution plan
    ↓
Metadata views, player handoffs, imports, exports, and sync routes
```

The consequences are deliberate:

1. A SIMKL import that supplies IMDb and Kitsu but no MAL is usable immediately.
2. A later MAL or AniList match enriches the same Fasti entity. It does not replace it.
3. Changing the preferred metadata provider changes a view. It does not migrate history.
4. A mapping-bundle update cannot silently move recorded activity.
5. An alias useful for metadata lookup is not automatically safe for tracker writeback.
6. Unresolved media can still be recorded, searched, corrected, exported, and healed later.
7. No provider outage, deleted identifier, empty response, or cache miss deletes the Chronicle.

This is the architectural centre of Fasti. Database, API, add-on, import, sync, context-action, and user-interface decisions follow from it.

---

## 1. Executive narrative

### 1.1 Situation

People consume media through many systems. Each system records a partial view. A player may know a local file and an IMDb ID. A tracker may know a Kitsu ID. A metadata source may know TMDB and TVDB. An anime service may divide a series into cours while a television database groups it into seasons. A book service may describe a work while another describes one edition. A music service may identify a recording while another identifies one track on one release.

### 1.2 Complication

Most trackers choose one provider identifier as the item key. This works until data crosses a provider boundary. Then a valid record can become “missing” because the current provider does not expose the expected ID. Provider changes can duplicate a show, split history, map the wrong episode, or leave usable aliases ignored.

The existing Fasti plan acknowledged mappings, but it did not define an identity record, a resolution policy, provider-switch safety, field-level metadata provenance, or a complete repair journey. That was not sufficient.

### 1.3 Question

How can Fasti keep one durable media record while providers disagree, disappear, renumber, split, merge, or supply only partial identifiers?

### 1.4 Answer

Build Fasti around five separate contracts:

1. **Stable local entities** owned by Fasti.
2. **Typed, directional assertions** for external identity and media topology.
3. **Purpose-specific resolution** instead of one universal preferred ID.
4. **Field-level metadata projections** instead of provider-owned records.
5. **Explicit reconciliation** with preview, evidence, recovery, and no silent history movement.

### 1.5 Core message

> **Fasti keeps the record stable while its understanding improves.**

That is the product advantage. Fasti is not merely another service that stores watched flags. It makes a person’s record understandable, repairable, portable, and useful across players and providers.

---

## 2. Fresh-review method and evidence posture

This plan treats every previous Fasti conclusion as a hypothesis. The stack, sequence, terminology, and scope were reviewed again against:

- current Floppy models, issues, migrations, and provider behavior;
- the Electric-Town anime-crosswalk-mappings schema, glossary, conformance corpus, governance, and resolution principles;
- the AnimeAPI discussion about 1:1 mapping failure, specials, cours, provider disappearance, and stale IDs;
- Nuvio cases where MAL/Kitsu primary IDs and IMDb aliases produce different outcomes for enrichment and synchronization;
- AniList and AniChart’s current shared API, relation model, rate limits, and terms;
- AniBridge’s current mapping artifacts as a possible implementation input;
- OpenRefine’s reconciliation protocol as a candidate-matching reference;
- domain models from MusicBrainz, Open Library, Podcasting 2.0, and IGDB;
- the existing Nuvio synchronization work, mDNS, MQTT, WebTransport, offline, multi-user, API, and extension plans;
- the supplied gstack engineering and developer-experience review methods;
- Narrative Builder, Assumption Audit, Initiative Prioritizer, Risk and Mitigation, and War Gaming;
- Kathy Sierra’s user-success, just-in-time learning, cognitive-leak, deliberate-practice, and meaningful-payoff principles.

### 2.1 Evidence labels

This document uses these labels:

- **LOCKED:** required for repository foundation.
- **PROVISIONAL:** direction is chosen but needs a spike or current source review.
- **OBSERVED:** supported by current source or documentation.
- **REJECTED:** explicitly excluded.
- **GATE:** must be proved before the related release claim.

### 2.2 Important correction

The design lessons come primarily from **Electric-Town/anime-crosswalk-mappings** [S1–S4]. AniBridge may be a useful data and implementation adapter, but it is not the source of Fasti’s identity doctrine.

### 2.3 Research limits

- The panels in this report are synthetic expert-role audits. No real named experts participated.
- **SUPERSEDED:** This seed originally treated Fasti as a competing tracker and
  gated all direct AniList use on written permission. Fasti is not a competing
  tracker. Public metadata use follows current official API terms and limits;
  any future authenticated tracker operation is a separate capability [S13].
- AniList identifiers are present in current Nuvio-related work, but this research did not prove that current Nuvio releases now use the AniList API directly for their general metadata pipeline.
- Electric-Town anime-crosswalk-mappings currently supplies a schema, policy, fixtures, and conformance work, not a published production dataset [S1–S4].

---

## 3. Product constitution

### 3.1 Category

Fasti is a:

> **Self-hosted media Chronicle, state authority, identity resolver, and interoperability service.**

### 3.2 Boundary

> **Fasti records. Players play.**

Fasti does not decode, stream, or compete with Kodi, Nuvio, Stremio, Jellyfin, VLC, music players, reading applications, podcast players, or game launchers.

### 3.3 Narrative

> **Fasti is a book of time in which recorded events become stories.**

The literary idea guides the product. It does not replace plain product language.

### 3.4 Product invariants

1. Local recording never depends on a remote service.
2. The original observation is preserved separately from later interpretation.
3. A retry is not a rewatch.
4. Progress is not history.
5. Watched or completed state is not history.
6. Saved state is not consumed state.
7. Provider metadata is not user-owned truth.
8. External identifiers are not Fasti primary keys.
9. Absence, timeout, 404, and empty response are not deletion.
10. A relation such as sequel, adaptation, franchise, or collection is not identity.
11. A mapping that cannot round-trip must declare loss.
12. A display-source change cannot move Chronicle data.
13. A mapping update cannot silently reattribute historical activity.
14. Every destructive or hard-to-reverse identity action has a preview and receipt.
15. Every provider and extension is replaceable.
16. Fasti remains useful with no mapping bundle installed.
17. The user can export the record, its provenance, and unresolved state.
18. The same application services power UI, API, CLI, MCP, Tauri, imports, and integrations.

---

## 4. Kathy Sierra operating doctrine [S24–S28]

### 4.1 The user’s larger context

Fasti is a subset of a larger user capability:

> **Keeping a trustworthy memory of media life across changing services and devices.**

Users do not want to become experts in provider IDs. They want to become able to:

- move their record without losing meaning;
- understand why an item matched;
- identify when a match is uncertain;
- repair a record without damaging history;
- add a new source without waiting for Fasti maintainers;
- recover from provider or device failure;
- trust that a later enrichment will improve, not replace, the record.

Fasti is successful when users feel more capable outside the product, not when they spend more time inside its settings.

### 4.2 User capability path

| Stage | User capability | Practice | Feedback | Meaningful payoff |
|---|---|---|---|---|
| 1 | Keep an unknown record safely | Import or record one item with partial IDs | “Saved locally; identification can continue later” | Nothing is lost because metadata is incomplete |
| 2 | See what Fasti knows | Open Sources & Identity | Clear source, match, and unresolved labels | The user can judge trust without reading logs |
| 3 | Repair one ambiguity | Compare two candidates and choose | Preview shows fields and history that will remain unchanged | The right item is fixed without collateral damage |
| 4 | Add a source | Install a mapping bundle or provider | Sandbox result and capability preview | More records heal without bespoke development |
| 5 | Govern a collection | Review bundle changes and local overlays | Impact report, receipt, and rollback | The record stays stable as external data changes |
| 6 | Help the ecosystem | Export a reusable correction or conformance case | Validation and provenance checks | Other users avoid the same failure |

### 4.3 Just-in-time knowledge [S28]

Do not teach “identity assertion,” “cardinality,” or “lossy projection” during first-run setup.

Use plain product copy first:

- **Matched**
- **Needs review**
- **Local only**
- **Source changed**
- **No known equivalent**
- **This match covers only part of the season**

Show technical terms only when the user opens details or builds an integration.

### 4.4 Reduce cognitive leaks [S27]

Fasti must remember what the user should not have to remember:

- which IDs arrived in the original import;
- which provider supplied each field;
- why two records were not merged;
- which mapping bundle and policy produced a result;
- what changed since the previous mapping version;
- what remains queued while offline;
- which provider needs credentials;
- which action is safe to retry;
- which records need review.

### 4.5 Exposure to good patterns

Before asking users to resolve difficult cases, show small worked examples:

- exact match;
- one provider record covering part of a season;
- two source episodes forming one target episode;
- same title but different work;
- retired provider ID with redirect;
- no counterpart exists.

This builds recognition of correct identity patterns before exposing a full graph editor.

### 4.6 Product metrics

Measure meaningful capability, not engagement:

- percentage of imported activity retained;
- false automatic merge rate;
- percentage of unresolved records still usable locally;
- time to resolve the first ambiguous item;
- percentage of provider preference changes with zero broken history;
- clean backup/restore equivalence;
- percentage of users who complete a repair without support;
- time to first successful provider adapter;
- percentage of errors that include a safe-state statement and one next action;
- number of conformance cases that prevent a known class of data damage.

Do not optimize for notification volume, streaks, settings visits, or daily active use.

---

## 5. Full PRD

### 5.1 Problem

Media records arrive with incomplete, inconsistent, stale, provider-specific, or differently grained identifiers. Existing trackers often couple identity, metadata, URLs, and history to one provider. This makes imports brittle and provider changes dangerous.

### 5.2 Primary job

> When media data arrives from any player, tracker, file, API, or add-on, preserve the user’s activity immediately, identify the media as accurately as available evidence permits, and improve that identification later without breaking the record.

### 5.3 Goals

- Stable provider-neutral local identity.
- Transparent enrichment and reconciliation.
- Exact and loss-aware mapping across media structures.
- Safe provider switching.
- Field-level metadata provenance and user choice.
- Simple provider and mapping-bundle onboarding.
- Custom media types and custom fields without ungoverned schema sprawl.
- Offline operation and later healing.
- Multi-user and multi-device attribution.
- Full API, CLI, UI, MCP, import, export, and event parity.
- Portable settings, contracts, knowledge, and conformance fixtures.

### 5.4 Non-goals for 1.0

- A universal public identity database hosted by Fasti.
- A graph database dependency.
- Automatic title-only matching.
- Automatic entity merges from heuristic scores.
- Full mirroring of provider catalogues.
- Authenticated AniList tracker operations without a separately sourced capability contract.
- Executable mapping scripts in user-defined API Connections.
- A global ontology that forces every media domain into one hierarchy.
- AI deciding canonical identity without deterministic evidence and user review.
- Rewriting historical occurrences whenever mappings change.

### 5.5 Primary users

- Household media tracker owner.
- User migrating from Floppy, Yamtrack, SIMKL, Trakt, MAL, AniList, Cinephage, or another tracker.
- Nuvio, Stremio, Kodi, Jellyfin, or other player user.
- Anime user with complex season and episode mappings.
- Book, podcast, music, or game user whose provider grain differs.
- Integration developer.
- Metadata-provider author.
- Add-on and mapping-bundle maintainer.
- User who creates a custom record type or custom field.
- User who needs reduced cognitive load, keyboard access, or resumable workflows.

### 5.6 Success criteria for 1.0

1. A real Floppy library imports with no activity silently discarded.
2. A SIMKL item with IMDb and Kitsu but no MAL remains usable and can heal later.
3. Provider display preference can change without changing Fasti entity IDs or Chronicle links.
4. Anime conformance cases support ranges, offsets, dual numbering, discontinuities, expansion, merge, alternate cuts, negative assertions, and known absence.
5. A user can explain why a record matched from the UI without opening logs.
6. A provider outage leaves local records and last-known-good metadata intact.
7. A new simple read-only provider can be connected through a declarative contract without core code.
8. A full provider adapter can pass a published conformance kit.
9. An unresolved item can be recorded, searched, exported, and reconciled later.
10. Clean backup and restore preserve entities, assertions, decisions, user overrides, unresolved cases, and Chronicle references.

---

## 6. Scope challenge and priority correction

The requested product can easily become six platforms before it proves one user outcome. The blind engineering review rejects that sequence.

### 6.1 What moves earlier

Identity and metadata separation move to **Milestone 0 and Milestone 1**. Imports, sync, provider switching, and player interoperability are unsafe without them.

### 6.2 What remains broad in the model but narrow in delivery

The foundation must include fixtures for movies, television, anime, books, music, podcasts, games, and custom types. The first polished adapters should still be sequenced:

1. Movies and television.
2. Anime.
3. Books, music, and podcasts.
4. Games.
5. Custom types and providers.

This avoids a video-only schema while preventing a six-domain integration programme from blocking first use.

### 6.3 What is deferred

- executable WASI providers;
- a public mapping marketplace;
- automatic public correction publication;
- Cloudflare identity relay;
- semantic vector matching as an authority source;
- peer-to-peer identity consensus;
- a graph database;
- fully automatic entity merge and split.

---

## 7. Bounded contexts and dependency rules

Fasti uses one deployable application at first. Bounded contexts are code and ownership boundaries, not mandatory microservices.

```text
Chronicle
├── Occurrences
├── Progress
├── Consumed state
├── Saved state
├── Ratings
├── Notes
└── Lists

Catalogue Identity
├── Fasti entities
├── Identifier namespaces
├── Identity assertions
├── Topology and mappings
├── Resolution policy
├── Reconciliation
└── Lifecycle and redirects

Metadata
├── Provider registry
├── Provider observations
├── Field claims
├── Resolved views
├── User overrides
└── Cache and retention

Record Types
├── Built-in domain profiles
├── Custom type definitions
├── Custom field definitions
├── Validation
└── Index projections

Interoperability
├── Import/export
├── Compatibility profiles
├── Connections
├── Add-ons
├── Player handoff
└── Provider adapters

Accounts and Access
├── Workspaces
├── Memberships
├── Profiles
├── Devices
├── Clients
└── Grants/scopes

Operations
├── Notifications
├── Jobs and outbox
├── Network policy
├── Diagnostics
├── Backup/recovery
└── Knowledge
```

### 7.1 Dependency rule

```text
Domain primitives
    ↓
Application services
    ↓
Ports/contracts
    ↓
Adapters and transports
    ↓
UI, CLI, MCP, Tauri, importers, providers
```

The following dependencies are forbidden:

- Chronicle depending on TMDB, TVDB, MAL, AniList, or another provider.
- Identity depending on a user-interface framework.
- Metadata providers writing directly to Chronicle tables.
- Importers writing directly to SQLite.
- Context actions implementing separate domain behavior.
- MCP tools bypassing application authorization.
- A mapping bundle changing user state during installation.
- Custom fields creating unreviewed SQL or executing scripts.

### 7.2 Repository shape

```text
crates/
  domain-common/
  chronicle/
  catalogue-identity/
  identity-policy/
  reconciliation/
  metadata-core/
  record-types/
  custom-fields/
  provider-sdk/
  mapping-bundles/
  import-export/
  compatibility/
  notifications/
  accounts/
  sync/
  network-policy/
  application/
  contracts/
  storage-sqlite/
  transport-http/
  transport-webtransport/
  transport-websocket/
  mcp/

services/
  node/
  auth/                 # SUPERSEDED historical django-allauth sidecar proposal
  notify/               # Apprise adapter, uv

apps/
  web/                  # Svelte 5, TypeScript, Vite, Tabler
  desktop/              # Tauri 2
  mobile/               # Tauri 2 platform shells

adapters/
  metadata/
  mapping/
  players/
  imports/
  exports/
  connections/

contracts/
  openapi/
  asyncapi/
  json-schema/
  json-ld/
  okf/
  conformance/
  examples/

knowledge/
  concepts/
  capabilities/
  troubleshooting/
  providers/
  migrations/
  decisions/
```

No `utils` or `misc` package may become the owner of domain behavior.

---

## 8. Identity constitution

### 8.1 Stable Fasti entity identifiers

Every catalogue entity receives an opaque, permanent Fasti identifier:

```text
fst_ent_01J...
fst_occ_01J...
fst_ida_01J...
fst_map_01J...
fst_res_01J...
fst_rec_01J...
```

The prefix identifies the resource type for API and support clarity. The identifier does not encode a provider, title, type, season, or mutable classification.

Canonical Fasti URLs use the Fasti ID:

```text
/records/fst_ent_01J...
```

Provider aliases may resolve to a Fasti record:

```text
/lookup/tmdb.movie/550
```

That lookup can return one exact result, several scoped results, a redirect, or an unresolved response. It is not the canonical record URL.

### 8.2 No primary external identifier

The phrase **primary identifier** is removed from the technical model.

The product may expose these distinct preferences:

- preferred display metadata source;
- preferred identifier to show;
- preferred export namespace;
- preferred sync account/provider;
- preferred player handoff;
- fallback resolution order;
- locale and region policy.

None changes the Fasti entity ID.

### 8.3 Universal primitives, domain-specific profiles

Fasti does not force every domain into one rigid parent chain. It uses common primitives and domain profiles.

Common entity roles:

- `work` — an intellectual or creative work;
- `release` — a published, broadcast, issued, or provider-defined unit;
- `segment` — an episode, track, chapter, podcast episode, level, or other consumable part;
- `variant` — a cut, recording, edition, platform release, language version, remaster, or other meaningful variation;
- `grouping` — a series, franchise, season, collection, release group, feed, or presentation grouping.

A `record_type_definition` states which roles and relations are valid for one media domain. Roles support interoperability. Domain-specific kinds preserve meaning.

Examples:

| Domain | Useful entity kinds |
|---|---|
| Movie/TV | work, series grouping, season grouping, release, episode segment, alternate cut |
| Anime | series grouping, broadcast release/cour, OVA/ONA/film/special release, episode segment, numbering space |
| Books | work, edition/release, chapter segment, audiobook edition and segment |
| Music | work, recording variant, release group, release, track segment |
| Podcast | podcast/show, feed variant, season grouping, episode segment, enclosure variant |
| Games | game work, edition, platform release, DLC/expansion, episode/season where applicable |
| Custom | administrator-defined kinds mapped to shared roles where useful |

### 8.4 Relations are assertions, not hierarchy assumptions

Relations include:

- `exact`;
- `subset_of`;
- `superset_of`;
- `overlaps`;
- `part_of`;
- `edition_of`;
- `release_of`;
- `alternate_cut_of`;
- `recording_of`;
- `adaptation_of`;
- `sequel_to`;
- `prequel_to`;
- `spin_off_of`;
- `related`;
- `not_same_as`.

Only relations explicitly declared equivalent can be used for identity consolidation. `related`, sequel, franchise, collection, and adaptation must never trigger a merge.

### 8.5 Provider grain is explicit

Each namespace declares the grain of its identifiers. Examples:

```text
tmdb.movie
tmdb.tv
tvdb.series
tvdb.season
tvdb.episode
mal.anime
anilist.anime
kitsu.anime
musicbrainz.recording
musicbrainz.release
openlibrary.work
openlibrary.edition
podcast.guid
podcast.episode-guid
steam.app
gog.product
igdb.game
```

A namespace key is not created dynamically from an untrusted payload. A source-specific unknown ID remains source-scoped until a namespace definition is installed and approved.

---

## 9. Identity record and persistence model

### 9.1 Main records

| Record | Purpose |
|---|---|
| `catalog_entity` | Stable Fasti media entity |
| `record_type_definition` | Media-domain and custom-type contract |
| `entity_kind_definition` | Domain kind and shared conceptual role |
| `entity_relation_assertion` | Typed relationship between Fasti entities |
| `identifier_namespace` | Definition of an external identifier space |
| `external_identifier` | Normalized provider identifier and lifecycle state |
| `identity_assertion` | Claim connecting a Fasti entity to an external identifier |
| `mapping_coverage` | Range, season, absolute, territory, or numbering-space scope |
| `segment_mapping_assertion` | Directional segment correspondence |
| `explicit_segment_link` | Exact, expands, merges, or absent correspondence when offsets are insufficient |
| `evidence` | Method, source, time, authority, reviewer, and route |
| `derivation_root` | Original lineage used to avoid false corroboration |
| `source_snapshot` | Version/hash of the observed provider or bundle response |
| `resolution_policy` | Versioned rules for one operation class |
| `resolution_decision` | Reproducible accepted interpretation |
| `reconciliation_case` | Unresolved, disputed, stale, merge, or split work item |
| `mapping_bundle` | Installed immutable mapping artifact and manifest |
| `mapping_overlay` | Instance or user-owned corrections applied above a bundle |
| `known_absence` | Evidence that no counterpart exists in one namespace/scope |
| `assertion_revocation` | Withdrawal of a known-bad assertion |
| `entity_tombstone` | Retired Fasti entity with redirect or terminal reason |
| `observed_reference` | Immutable source information captured at ingestion |
| `occurrence_interpretation` | Versioned link from an occurrence to resolved entity/segment meaning |

### 9.2 Identifier namespace contract

```yaml
id: tmdb.movie
version: 1
owner: themoviedb
entity_grains: [work, release]
value:
  type: integer-string
  pattern: '^[1-9][0-9]*$'
  case_sensitive: false
normalization:
  trim: true
  strip_prefixes: ['tmdb:', 'tmdb_movie:']
uniqueness_scope: global
lookup:
  deep_link: 'https://www.themoviedb.org/movie/{id}'
validation:
  capability: metadata.lookup
lifecycle:
  supports_redirect: unknown
  identifiers_reused: false
acquisition_routes:
  - tmdb_api
  - provider_declared_external_id
  - user_verified
storage_policy:
  identifiers: allowed
  descriptive_payload: provider_terms
```

Custom namespaces require the same minimum information:

- stable namespace key;
- owner or local scope;
- identifier format and normalization;
- entity grain;
- uniqueness scope;
- collision rules;
- deep link where available;
- allowed acquisition routes;
- validation method;
- licensing and storage posture.

### 9.3 External identifier lifecycle

```text
unknown → active → retired → redirected
                 ↘ disputed
                 ↘ invalid
```

External identifiers are never silently removed. A retired ID can retain:

- last-known provider state;
- redirect target;
- validation history;
- assertions that used it;
- affected records;
- related source snapshots.

### 9.4 Identity assertion

```yaml
assertion_id: fst_ida_01J...
subject_entity_id: fst_ent_01J...
external_identifier:
  namespace: kitsu.anime
  value: '12345'
relation: exact
scope:
  mode: release
status: accepted
evidence_class: upstream_declared
id_source: simkl.import
source_snapshot: simkl-import-sha256:...
derivation_root: simkl
observed_at: '2026-08-21T12:00:00Z'
policy_version: identity-admission/1
```

### 9.5 Evidence classes

Evidence classes are categories, not a global numeric ranking:

- `authority_asserted`;
- `provider_declared`;
- `human_verified`;
- `corroborated`;
- `deterministic`;
- `inferred`;
- `imported`;
- `candidate`;
- `disputed`.

Candidate ranking may use a numeric score internally. The score must not be stored or shown as if it were objective truth.

### 9.6 Derivation roots

Two mapping projects can agree because both copied the same upstream dataset. Fasti records the derivation root where known. Corroboration requires independent roots. Unknown lineage fails closed for irreversible actions.

### 9.7 Negative and absence evidence

Store both:

- `not_same_as` — these entities or identifiers must not merge;
- `known_absent` — no valid counterpart is known in this namespace and scope.

These records stop Fasti from repeating the same bad search or merge after every refresh.

---

## 10. Original observation and later interpretation

Fasti stores what happened before it stores what it thinks the media means.

```text
Observed reference
  source client: living-room-kodi
  raw title: Example Show
  raw IDs: imdb=tt..., kitsu=...
  season: 2
  episode: 4
  source timestamp: ...
  payload hash: ...

Occurrence
  started_at: ...
  progress: ...
  device/profile: ...

Interpretation v1
  provisional entity: fst_ent_A
  unresolved reason: no exact segment mapping

Interpretation v2
  entity: fst_ent_B
  segment: fst_ent_C
  resolution decision: fst_res_...
  reason: user accepted exact provider mapping
```

### 10.1 Rules

- `observed_reference` is immutable except for redaction governed by privacy policy.
- An occurrence is never deleted because interpretation fails.
- A new interpretation supersedes an old interpretation; it does not rewrite the observation.
- The UI normally shows the current interpretation and can expose the original source on demand.
- Exports can include both.
- User corrections create a receipt.
- Mapping-bundle updates create proposals, not silent new interpretations.

### 10.2 Why this matters

This model lets Fasti say:

> “This activity was recorded from Kodi at 21:42. Fasti first linked it to an unresolved local episode. You later matched it to Season 2 Episode 4. The original observation remains available.”

That is materially safer than updating one `media_id` field in place.

---

## 11. Mapping and topology model

Electric-Town’s crosswalk work establishes the minimum expressive power required for difficult episodic identity. Fasti generalizes those primitives across domains without turning the crosswalk into a metadata database.

### 11.1 Required mapping capabilities

1. Range-scoped coverage.
2. Non-zero offset.
3. Season and absolute numbering together.
4. One source release crossing target season boundaries.
5. Discontinuous target coverage.
6. One source segment expanding to several target segments.
7. Several source segments merging into one target segment.
8. Separate numbering spaces for regular episodes, specials, trailers, credits, parodies, and other material.
9. Negative assertions.
10. Alternate cuts.
11. Membership in more than one series/grouping.
12. Known absence.
13. Ordering variance.
14. Territory or edition scope where the source relationship is region-specific.
15. Versioned revocation and supersession.

### 11.2 Directionality

A mapping from A to B does not imply B to A.

A target may cover a larger or smaller unit. Reversing the edge can lose information or produce multiple candidates. Reverse mappings need their own accepted assertion or a resolution plan that explicitly states loss.

### 11.3 Coverage example

```yaml
mapping_id: fst_map_01J...
subject:
  entity_id: fst_ent_anime_release
relation: subset_of
target:
  namespace: tvdb.season
  id: '267440:3'
coverage:
  source:
    numbering_space: regular
    start: 1
    end: 10
  target:
    mode: season
    season: 3
    start: 13
    end: 22
  transform:
    offset: 12
```

### 11.4 Non-affine segment links

```yaml
links:
  - from: [3]
    to: [4, 5]
    kind: expands
  - from: [6, 7]
    to: [6]
    kind: merges
  - from: [9]
    to: []
    kind: absent
    reason: no equivalent segment exists in target ordering
```

### 11.5 Series and franchise

A series is a useful grouping, not a universal parent of truth. A release may belong to zero, one, or several series. A franchise is for presentation and relations. It must not be used to merge records.

### 11.6 Permanent identifiers

Fasti internal identifiers are permanent. A merged or retired Fasti entity becomes a tombstone with a redirect. Redirect chains must terminate and cannot cycle.

### 11.7 Irreversibility gate

- Adding a reversible, non-conflicting alias can be automatic.
- Fetching metadata through a provisional route can be automatic when clearly labelled.
- Merging entities, moving history, splitting entities, or sending tracker writes needs stronger evidence, preview, authorization, and a receipt.

The risk of the action controls the gate. A generic confidence threshold does not.

---

## 12. Purpose-specific resolution

A single “best ID” is not a safe abstraction. Fasti resolves identity for an explicit intent.

### 12.1 Resolution intents

- `record_ingest`;
- `deduplicate_candidate`;
- `display_metadata`;
- `metadata_lookup`;
- `catalogue_lookup`;
- `segment_coordinates`;
- `progress_read`;
- `progress_write`;
- `history_read`;
- `history_write`;
- `rating_read`;
- `rating_write`;
- `play_with`;
- `import`;
- `export`;
- `notification_link`;
- `public_link`.

### 12.2 Resolution plan

```yaml
resolution_id: fst_res_01J...
intent: history_write
entity_id: fst_ent_01J...
status: exact
selected:
  provider: simkl
  namespace: kitsu.anime
  identifier: '12345'
  segment_coordinate:
    mode: flat
    episode: 8
alternatives:
  - namespace: imdb.title
    identifier: tt1234567
    permitted_for: [metadata_lookup, play_with]
    rejected_for: history_write
    reason: shared franchise alias does not identify the provider-native anime release
lossiness: none
policy_version: resolver/history-write/1
bundle_versions:
  - anibridge-mappings:v4:sha256:...
evidence:
  - fst_evd_...
```

### 12.3 Resolver output states

- `exact`;
- `exact_scoped`;
- `lossy`;
- `ambiguous`;
- `provisional`;
- `known_absent`;
- `unavailable`;
- `retired_target`;
- `conflict`;
- `unsupported`.

### 12.4 Read aggregation versus write routing

An alias can be safe for read aggregation and unsafe for writes.

Example:

- An IMDb alias may be useful to retrieve TMDB artwork.
- The same IMDb alias may point to a shared franchise record while a tracker expects one MAL or Kitsu release.
- A tracker write must therefore use the exact provider-native record and episode coordinates.

Fasti’s provider interface must declare which identifiers each operation accepts. The resolver never assumes that an alias accepted for `metadata_lookup` is accepted for `history_write`.

### 12.5 Resolution policies

Policies are versioned by intent. They declare:

- acceptable assertion relations;
- acceptable evidence classes and authority scopes;
- bundle and overlay order;
- whether lossy output is permitted;
- whether user confirmation is required;
- fallback namespaces;
- locale/region behavior;
- provider capability requirements;
- expected round-trip behavior;
- cache key inputs;
- errors and user next actions.

---

## 13. The SIMKL partial-identity journey

### 13.1 Input

A SIMKL import supplies:

```text
IMDb: tt1234567
Kitsu: 12345
MAL: absent
```

The user’s configured anime metadata view prefers MAL. A provider-keyed tracker would fail or create a dead record.

### 13.2 Fasti flow

1. Fasti creates or locates a stable `fst_ent_...` entity.
2. It stores both identifiers and their acquisition route, `simkl.import`.
3. It validates syntax immediately and schedules existence checks when online.
4. The record is usable without MAL.
5. Metadata resolution can use Kitsu directly or use IMDb to find a permitted TMDB projection.
6. An installed mapping bundle may propose MAL, AniList, AniDB, TVDB, or TMDB bindings.
7. If one exact, non-conflicting MAL assertion is found, Fasti adds it to the same entity.
8. If several MAL releases cover different parts, Fasti creates scoped assertions and a review case rather than choosing one global ID.
9. Chronicle activity remains attached to the stable Fasti entity and its versioned interpretation.
10. A later provider or mapping update may improve the record without changing its URL or history identity.

### 13.3 User-facing result

```text
Saved locally

Fasti received:
• Kitsu 12345
• IMDb tt1234567

MAL is not required to keep this record.
Fasti can look for more identifiers now or later.

[Find more sources] [View record]
```

### 13.4 If no MAL equivalent exists

Record `known_absent` with scope and evidence. The UI says:

> No known MAL record matches this release. Fasti will keep using the available sources. It will check again only when your mapping data changes.

### 13.5 Canonical acceptance test

The import, view, Chronicle, export, and later enrichment must all succeed with no MAL ID. This case is a P0 conformance fixture.

---

## 14. Changing provider preference without breaking records

### 14.1 Remove “switch main identifier”

The UI must not imply that choosing TVDB instead of TMDB changes the record’s identity.

Use separate settings:

- **Display source** — which provider normally supplies titles, summaries, and artwork.
- **Episode order** — which coordinates are shown for a profile/player.
- **Export identifier** — which namespace is preferred when the target supports several.
- **Sync account** — which external service receives user-state writes.
- **Fallback policy** — what Fasti may try if the preferred source is missing.

### 14.2 Change preview

Before applying a preference change, show:

| Impact | Example |
|---|---|
| Fasti records | 0 IDs changed |
| Chronicle | 0 occurrences moved |
| Metadata view | 1,263 titles may change |
| Exact provider matches | 1,188 |
| Missing target IDs | 52 |
| Different season topology | 19 |
| Lossy projections | 4 |
| User overrides | 27 remain unchanged |
| Cache | 1,263 resolved views will rebuild in the background |

### 14.3 Apply behavior

- Existing Fasti IDs remain unchanged.
- Existing metadata claims remain available until retention policy removes them.
- Resolved views rebuild in bounded background batches.
- Missing targets use declared fallback or display last-known-good metadata.
- Records with topology differences create review cases.
- History is never moved automatically.
- Deep links and player actions can change only when the new route is valid for that action.

### 14.4 Duplicate records

Changing a preference does not automatically merge duplicates. Fasti can propose a merge when exact assertions establish that the entities are the same. The user sees affected activity, fields, relations, and rollback behavior before approval.

---

## 15. Enrichment and healing pipeline

### 15.1 Pipeline

```text
Capture
  ↓
Normalize
  ↓
Validate
  ↓
Expand evidence graph
  ↓
Generate candidates
  ↓
Fetch metadata claims
  ↓
Resolve by intent
  ↓
Auto-apply reversible outcomes
  ↓
Queue ambiguous or irreversible outcomes
  ↓
Record decision, receipt, and policy version
```

### 15.2 Capture

Preserve every supplied identifier, source route, title, coordinates, timestamps, and raw-field names allowed by privacy and source terms. Do not discard an alias because the current preferred provider does not use it.

### 15.3 Normalize

Normalization is namespace-specific:

- trim and case rules;
- prefixes;
- leading zeros;
- URL extraction;
- ISBN check digits;
- UUID format;
- locale or platform scope;
- provider-specific entity hint.

Raw values remain available for audit.

### 15.4 Validate

Validation states:

- syntax valid;
- existence confirmed;
- existence unconfirmed;
- retired;
- redirected;
- inaccessible;
- provider unavailable;
- validation prohibited by terms/policy.

A temporary 404 or outage does not delete an assertion. Repeated terminal evidence can mark it retired or invalid through a recorded decision.

### 15.5 Expand

The resolver can consult configured layers:

1. accepted user/workspace decisions;
2. instance local overlays;
3. installed immutable mapping bundles;
4. direct provider cross-references;
5. provider adapters and exact external-ID lookup;
6. heuristic candidate generation.

Each layer retains provenance. The graph is not flattened into one source-free set of IDs.

### 15.6 Candidate generation

Candidate generation may use:

- exact identifiers;
- provider-declared aliases;
- dates;
- duration/runtime;
- creators/studios/authors/artists;
- edition or platform;
- episode counts and coordinates;
- titles and aliases;
- relation graph;
- content fingerprints where lawful and available.

Title similarity can rank candidates. It cannot authorize an exact match or merge by itself.

### 15.7 Auto-application policy

Automatic:

- add a non-conflicting exact alias from an allowed route;
- update last-validated status;
- fetch and cache permitted metadata;
- create a reversible candidate relation;
- record known absence from an authoritative source;
- stage a mapping-bundle diff.

Requires review or stronger evidence:

- merge entities;
- split an entity;
- move or reinterpret historical activity;
- replace one episode topology with another;
- accept a lossy projection for sync write;
- resolve a dispute;
- publish a correction outside the instance.

### 15.8 Retry control

Known absence, negative assertions, terminal validation results, provider backoff, and bundle versions prevent endless lookup loops.

### 15.9 Offline behavior

When offline:

- save observations and local state;
- create provisional Fasti entities;
- queue validation and enrichment;
- use last-known-good metadata;
- show exact pending work;
- permit manual local identifiers and fields;
- resume from the last committed checkpoint.

---

## 16. Reconciliation as a first-class product capability

### 16.1 Reconciliation case types

- missing identifier;
- ambiguous candidate;
- conflicting assertions;
- one-to-many topology;
- lossy projection;
- retired identifier;
- duplicate Fasti entities;
- suspected wrong merge;
- provider structure changed;
- mapping-bundle changed;
- custom namespace collision;
- provider field conflict;
- unresolvable source record;
- source terms prevent persistence.

### 16.2 Case states

```text
open
waiting_for_network
waiting_for_credentials
waiting_for_provider
waiting_for_user
accepted
rejected
known_absent
deferred
superseded
resolved
```

### 16.3 Reconciliation workbench

The workbench presents:

- the safe-state statement first;
- original observation;
- current Fasti interpretation;
- candidate records;
- identifiers and sources;
- field comparison;
- topology comparison;
- activity affected;
- exact versus lossy behavior;
- evidence and source lineage;
- recommended action;
- alternatives;
- preview and rollback implications.

### 16.4 OpenRefine compatibility

Fasti should support a reconciliation surface inspired by the Reconciliation API for candidate queries, manifests, type/property suggestions, and batch results. Fasti extends that pattern with:

- typed relation and topology claims;
- source and target grain;
- directional range coverage;
- evidence and derivation roots;
- operation intent;
- lossiness;
- merge/split preview;
- Chronicle impact.

### 16.5 Batch resolution

Batch actions are allowed only when all selected cases share the same rule and consequence. The preview states exact counts and any exceptions.

Example:

> Apply this exact Kitsu → MAL mapping to 38 records. 35 require no history change. 3 have different episode structures and will remain in review.

### 16.6 Undo

Reversible decisions can be undone by creating a superseding decision. Fasti does not erase the prior decision or evidence.

---

## 17. Metadata model: provider claims, not provider-owned records

### 17.1 Separation

```text
Stable Fasti entity
    ├── Identity assertions
    ├── Topology assertions
    ├── Provider metadata claims
    ├── User field overrides
    └── Resolved display view
```

A provider can change metadata claims. It cannot change the entity ID or user activity.

### 17.2 Metadata projection

```yaml
projection_id: fst_mdp_01J...
entity_id: fst_ent_01J...
provider_installation: tmdb-default
source_identifier:
  namespace: tmdb.movie
  value: '550'
locale: en-IE
region: IE
schema_version: tmdb-projection/2
fetched_at: '2026-08-21T12:00:00Z'
expires_at: '2026-08-28T12:00:00Z'
last_success_at: '2026-08-21T12:00:00Z'
source_snapshot_hash: sha256:...
storage_policy: persistent_anchored
status: fresh
```

### 17.3 Field claim

Each normalized field retains provenance:

```yaml
field_key: core.title
value: Example Film
language: en
source_projection: fst_mdp_...
source_path: '$.title'
observed_at: '2026-08-21T12:00:00Z'
provider_authority_scope: tmdb.movie.metadata
status: active
```

### 17.4 Resolved metadata view

A field-resolution policy selects the displayed value:

1. explicit user override;
2. workspace override;
3. preferred provider claim valid for locale/region;
4. fallback provider claim;
5. last-known-good claim;
6. original observed value;
7. empty state.

Different fields can come from different providers. The UI can show a simple source summary and an advanced field-by-field view.

### 17.5 User override

A user override is first-class, exportable, and never overwritten by refresh. It includes:

- value;
- locale where relevant;
- scope: user, profile, or workspace;
- reason/optional note;
- created time and actor;
- field schema version;
- supersession history.

### 17.6 Provider field conflict

Conflicts do not require one global winner. Policies may choose one value for display while retaining all claims.

### 17.7 Provider terms and retention

A provider manifest declares whether Fasti may:

- store identifiers;
- store normalized facts;
- cache full payloads;
- cache artwork;
- retain data after account disconnect;
- export provider data;
- use the data for public/shared mapping artifacts.

Transient application use and a public CC0 mapping dataset are different legal/provenance paths.

---

## 18. Cache and storage plan

### 18.1 Data classes

| Class | Examples | Authority | Default retention |
|---|---|---|---|
| Canonical user state | occurrences, progress, ratings, notes | Fasti/user | until explicit deletion |
| Identity evidence | identifiers, assertions, decisions | Fasti evidence store | durable, versioned |
| Anchored metadata | fields for records with user state | provider projection | retained under source policy |
| Discovery metadata | browse/search-only results | disposable cache | bounded TTL/LRU |
| Raw source snapshot | provider JSON/XML | evidence/cache | policy-specific, often short or hash-only |
| Artwork | poster/backdrop/logo | disposable content cache | bounded bytes/TTL |
| Derived view/index | resolved metadata, FTS, custom-field index | disposable | rebuildable |

### 18.2 Anchored record

A record is anchored when it has at least one:

- Chronicle occurrence;
- progress;
- consumed/saved state;
- rating;
- note;
- tag;
- list membership;
- user override;
- pin;
- explicit identity decision.

### 18.3 Retention presets

- **Minimal:** save metadata only for anchored records.
- **Balanced:** anchored persists; discovery expires after 7 days.
- **Extended:** discovery expires after 30 days.
- **Manual:** per provider/type budgets.
- **Until removed:** only where provider terms permit.

### 18.4 Cache key

A metadata/identity cache key includes:

```text
provider installation
operation
namespace and normalized ID
entity/segment hint
locale and region
provider schema version
resolution policy version
mapping bundle/overlay versions
content-policy hash
auth-scope hash
```

Secrets and signed URLs never appear in keys or logs.

### 18.5 Per-record actions

- refresh if stale;
- force refresh one provider;
- clear one disposable projection;
- restore last known good;
- compare providers;
- inspect provenance;
- dump normalized permitted data;
- inspect raw snapshot where policy permits;
- find more identifiers;
- reconcile identity;
- stage provider-source change.

### 18.6 Failure behavior

- Provider outage: show last-known-good and clear error.
- 404: validate terminality; do not remove the record.
- Empty response: record result; do not delete.
- Corrupt cache: discard and rebuild derived cache only.
- Mapping bundle unavailable: use existing accepted identity assertions.
- No network: queue refresh and keep local state.

---

## 19. Custom fields and custom media types

### 19.1 Requirement

A custom media type is incomplete without custom fields. Built-in types also need administrator- or add-on-defined fields.

### 19.2 Field-definition model

```yaml
field_key: games.gog.product_id
version: 1
label: GOG Product ID
description: The product identifier assigned by GOG.
value_type: external_identifier
cardinality: one
record_types: [game.release]
validation:
  namespace: gog.product
required: false
searchable: true
filterable: true
sortable: false
indexed: exact
privacy: normal
source_ownership: external_provider
override_policy: user_can_override
merge_policy: preserve_all_then_resolve
ui:
  component: identifier
  group: Sources
export:
  key: gog_product_id
```

### 19.3 Supported field types

- string;
- localized string;
- rich text with safe subset;
- integer;
- decimal;
- boolean;
- date;
- date-time;
- duration;
- URL;
- enum;
- multi-enum;
- external identifier;
- entity reference;
- bounded list;
- bounded object validated by JSON Schema.

### 19.4 Storage model

Do not use an unrestricted entity-attribute-value truth table.

Use:

1. versioned `field_definition` records;
2. one validated extension document per entity/type/namespace;
3. explicit `external_identifier` records for identity fields;
4. a derived `field_index_projection` only for fields declared searchable, filterable, or sortable;
5. promotion of widely used stable fields into built-in normalized columns only through a migration/RFC.

### 19.5 Identity-capable custom fields

A custom field can participate in identity only when it references an approved identifier namespace. A plain text field cannot be declared globally unique without:

- normalization rules;
- entity grain;
- uniqueness scope;
- collision behavior;
- lifecycle policy;
- validation method;
- provenance route.

### 19.6 Games example

A game record can carry:

- Fasti stable entity ID;
- IGDB game ID;
- Steam App ID;
- GOG Product ID;
- Epic catalogue item ID;
- platform release relations;
- user preference for GOG metadata;
- Play With routes for Steam, GOG, Epic, or a local launcher.

Choosing GOG for metadata or playback does not change the Fasti entity ID.

### 19.7 Books example

Separate:

- book work;
- edition;
- audiobook edition;
- chapter/segment.

Identifiers can include ISBN-10, ISBN-13, Open Library Work, Open Library Edition, Google Books Volume, Hardcover, ASIN, or a local custom namespace. ISBN normally identifies an edition/product, not the abstract work.

### 19.8 Podcasts example

Separate:

- podcast/show;
- feed variant;
- episode;
- enclosure/media variant.

Use podcast GUID, feed URL history/redirects, item GUID, enclosure URL/hash, and directory IDs as separate evidence. A changed feed URL must not create a new show when a stable podcast GUID is available.

### 19.9 Music example

Preserve MusicBrainz distinctions:

- work;
- recording;
- release group;
- release;
- track.

A recording can appear on many release tracks. A release and release group are not interchangeable.

### 19.10 Custom-type definition

```yaml
record_type_key: tabletop.session
version: 1
label: Tabletop session
consumption_verb: play
entity_kinds:
  - key: campaign
    role: grouping
  - key: session
    role: segment
progress:
  units: [minutes, explicit]
completion:
  policy: explicit
fields_schema: schemas/tabletop-session-v1.json
allowed_relations: [part_of, related, not_same_as]
capabilities:
  history: true
  progress: true
  ratings: true
  notes: true
  lists: true
```

---

## 20. Provider and mapping-source architecture

### 20.1 Provider classes

Do not force every source into one “metadata provider” interface.

| Class | Purpose | Examples |
|---|---|---|
| Metadata provider | Search and return field claims | TMDB, TVDB, Google Books, MusicBrainz |
| Identity source | Return identifier and relation assertions | provider external-ID endpoints, Wikidata where permitted |
| Mapping bundle | Supply versioned topology/mapping assertions | Electric-Town-compatible artifacts, AniBridge bundle |
| Catalogue source | Return candidate rows/collections | AIOmetadata, Stremio catalogues, Kaptain/Xperience exports |
| Tracker account | Read/write user state | SIMKL, MAL, and any future separately sourced authenticated AniList capability |
| Player connector | Observe activity and hand off playback | Nuvio, Kodi, Jellyfin, VLC, local apps |
| Importer/exporter | Translate a file/API dialect | Floppy, Yamtrack, Cinephage, generic compatibility profiles |
| Connection | User-defined declarative external endpoint/topic mapping | HTTP, webhook, MQTT |

One add-on can contribute more than one class, but each capability is separately declared and granted.

### 20.2 Provider manifest

```yaml
manifest_version: 1
id: org.example.google-books
version: 1.0.0
kind: metadata_provider
display_name: Google Books
supported_record_types: [book.work, book.edition]
accepts_namespaces: [isbn.10, isbn.13, googlebooks.volume]
emits_namespaces: [googlebooks.volume, isbn.10, isbn.13]
capabilities:
  - search
  - lookup_by_id
  - lookup_by_external_id
  - artwork
  - public_rating_read
auth:
  type: api_key
  secret_fields: [api_key]
network:
  allowed_hosts: [www.googleapis.com]
  tls_required: true
pagination:
  mode: offset
rate_limit:
  headers: []
  retry_after: true
locales:
  language_filter: true
fields:
  emits:
    - core.title
    - book.authors
    - book.publisher
    - core.release_date
    - book.page_count
    - core.description
    - core.artwork.cover
    - public.rating
storage_policy:
  identifiers: persistent
  normalized_fields: permitted_under_terms
  raw_payload: short_cache
attribution:
  required: true
knowledge: knowledge/providers/google-books.md
```

### 20.3 Adapter output

Providers return claims, not mutable core entities:

```text
ProviderResult
├── IdentifierClaim[]
├── RelationClaim[]
├── SegmentTopologyClaim[]
├── MetadataFieldClaim[]
├── AvailabilityObservation[]
├── SourceSnapshot
├── Attribution
└── Diagnostics
```

Application services validate and persist them according to capability, source policy, and resolution policy.

### 20.4 Provider onboarding Definition of Done

A new provider is not complete until it has:

- manifest and schema;
- supported record/entity-grain matrix;
- namespace definitions;
- operation capability mapping;
- authentication and secret model;
- Safe HTTP/Safe Broker policy;
- pagination behavior;
- rate-limit and retry behavior;
- field mapping and provenance;
- storage/licence/attribution review;
- empty, partial, ambiguous, 404, 429, timeout, and malformed-response fixtures;
- offline/last-known-good behavior;
- OpenAPI/AsyncAPI changes where applicable;
- Capabilities Ledger entries;
- local knowledge article;
- provider health UI;
- migration/deprecation policy;
- accessibility review;
- conformance tests.

### 20.5 Developer golden paths

1. **Simple read-only API:** import OpenAPI into Connection Studio and map one operation.
2. **Declarative metadata source:** create a provider manifest and JSON field mappings.
3. **Native adapter:** implement the Rust provider SDK traits for complex pagination, GraphQL, or topology.
4. **Mapping bundle:** publish immutable artifact, manifest, hashes, schema version, provenance, revocations, and diff.

Target developer outcomes:

- first test response in under 5 minutes;
- first local field claim in under 15 minutes for a simple API;
- all errors show problem, cause, fix, actual safe values, and knowledge link;
- `fasti provider test` runs the same fixtures used by CI;
- no database, UI, or Chronicle knowledge is required to write a provider.

### 20.6 Connection Studio boundary

Connection Studio supports bounded transforms and schemas. It cannot execute JavaScript, Python, shell, or downloaded code.

A declarative connection can map:

- endpoint and method;
- headers/query/body;
- auth secret reference;
- JSON Pointer/JSONPath fields;
- enum and date transforms;
- pagination;
- idempotency;
- external identifier namespaces;
- metadata field claims;
- events and commands.

Complex logic moves to a sandboxed plugin after the plugin model exists.

---

## 21. Anime strategy

### 21.1 Design authority

Electric-Town/anime-crosswalk-mappings is the first design and conformance input because it treats mappings as typed, directional, range-scoped assertions with provenance, lifecycle, negative evidence, and explicit cardinality [S1–S4].

Fasti should use its conformance shapes even when the runtime data comes from another bundle.

### 21.2 AniBridge role

AniBridge is a strong candidate implementation input because it currently publishes versioned daily mapping artifacts and represents directional ranges, discontinuities, ratios, provider scopes, and manual edits [S17].

It is not mandatory. Fasti must:

- vendor or download a versioned immutable artifact;
- verify hash/schema/version;
- retain provenance and source lineage where available;
- stage changes before application;
- support local overlays;
- operate when no bundle is installed;
- avoid treating inferred transitive closure as irreversible truth;
- run the Electric-Town conformance corpus against the adapter;
- validate target existence where practical;
- retain revocations and stale-target state.

### 21.3 AniList and AniChart

AniChart and AniList use the same AniList API. AniChart is not a separate identifier namespace. Use `anilist.anime` and `anilist.manga` [S13–S16].

AniList can be valuable for:

- provider-native media identifiers;
- relation assertions;
- schedule observations;
- format, season, source, and public metadata;
- user list import/write only if authorized.

However:

- titles are not unique and title search is candidate generation only;
- IDs need the media type/grain;
- relation edges are provider assertions, not universal truth;
- nested schedules require correct independent pagination;
- unknown episode totals must remain unknown;
- authenticated tracker operations require their own current terms and source review;
- API rate and availability limits require bounded cache and fallback.

**Decision (corrected 29 August 2026):** Fasti is not a competing tracker with
AniList. Public metadata use does not need a separate permission gate beyond
the current documented API terms and limits. Design the metadata adapter and
conformance fixtures against current official evidence. Treat any future
authenticated write or tracker operation as a separate capability with its own
terms, authorization, and source review. AniChart is not evidence for that
separate capability.

### 21.4 MAL, Kitsu, AniDB, TMDB, and TVDB

Each remains a separate namespace and provider view.

- MAL/Kitsu/AniList often identify a release, cour, OVA, film, or special.
- TVDB/TMDB often group around series, seasons, and episodes.
- AniDB separates numbering spaces and can group material differently.
- SIMKL may aggregate useful aliases but can contain stale external identifiers.
- No source becomes the universal parent.

### 21.5 Anime topology fixtures

The P0 corpus includes:

- split cour inside one TVDB season;
- one MAL entry spanning several target seasons;
- specials separated in MAL but grouped by AniDB;
- season zero versus provider-native special sequence;
- recap inserted in one provider’s order;
- one source episode corresponding to two target episodes;
- two source parts corresponding to one target episode;
- alternate cut;
- same title/different work;
- provider deletion/renumbering;
- schedule with unknown final count;
- regional availability without identity change;
- mapping-bundle update that would move an existing occurrence.

### 21.6 Local overlays

Users can add instance-local corrections without editing the vendored bundle:

```yaml
overlay_version: 1
base_bundle: anibridge:v4:sha256:...
assertions:
  - action: add
    subject: ...
    target: ...
    relation: subset_of
    evidence:
      method: human_verified
      note: Checked provider episode pages on 2026-08-21
revocations:
  - assertion_id: upstream-assertion-id
    reason: provider target was deleted
```

An overlay is exportable and reviewable. Publishing it to an external project is a separate explicit action with licensing checks.

---

## 22. Domain identity profiles

### 22.1 Movies and television

- TMDB, TVDB, and IMDb are identifiers and metadata views, not record keys.
- Series, season, episode, film, and alternate cut are distinct grains.
- Provider episode order is an explicit coordinate system.
- A Plex/Jellyfin/Kodi local library ID is source-scoped unless external IDs are present.
- Switching display/order preference requires topology impact preview.

### 22.2 Books

- Work and edition are distinct.
- ISBN identifies an edition/product, not always the abstract work.
- Open Library Work and Edition IDs remain separate.
- Google Books Volume, Hardcover, ASIN, and local catalogue IDs are external coordinates.
- Reading progress must declare pages, percentage, location, chapter, or explicit completion.
- Metadata-provider preference can differ by language and edition.

### 22.3 Music

- Work, recording, release group, release, and track are not interchangeable.
- A recording can appear on several release tracks.
- ISRC identifies a recording; a track position belongs to a release.
- AcoustID/fingerprint evidence is a candidate or deterministic route according to exact algorithm and quality.
- Listen events attach to the best known recording/track interpretation while preserving the original player payload.

### 22.4 Podcasts

- Podcast GUID is preferred when present and stable.
- Feed URL has redirect history and is not the only show identity.
- Item GUID identifies an episode within the feed ecosystem and must be preserved exactly.
- Enclosure URL/hash identifies one media delivery and may change independently.
- Directory IDs are aliases.
- Episode numbers are presentation/order data, not universal identity.

### 22.5 Games

- Game work, platform release, edition, bundle, DLC, expansion, remake, remaster, and port are explicit kinds/relations.
- IGDB external-game records can bridge Steam, GOG, Epic, and other store IDs, but the acquisition route remains visible.
- Store preference does not merge a remake with the original or a bundle with a base game.
- Play With uses the configured platform/store route.

### 22.6 Custom types

Custom types inherit the identity core but define their own:

- entity kinds and roles;
- allowed relations;
- progress units;
- completion rules;
- custom fields;
- namespace definitions;
- mapping capability requirements;
- context actions;
- display components.

---

## 23. User interface architecture

### 23.1 Normal record view

The normal record page shows:

- title and current resolved metadata;
- source summary;
- match state;
- user state and Chronicle;
- Play With;
- actions;
- a small source chip such as `Matched`, `Needs review`, `Local only`, or `Source changed`.

It does not lead with a graph.

### 23.2 Sources & Identity tab

```text
Fasti record
  fst_ent_01J...

Sources
  ✓ Kitsu 12345              Provided by SIMKL import
  ✓ IMDb tt1234567           Provided by SIMKL import
  ? MAL                      No exact match yet
  ✓ TMDB 98765               Found through IMDb, metadata use only

Structure
  Kitsu release 1–12
  ↳ TVDB Season 2 Episodes 1–12

Current uses
  Display metadata: TMDB
  Anime tracker write: Kitsu
  Play With: IMDb deep link

[Find more sources] [Compare] [Repair record] [Add ID]
```

### 23.3 Advanced evidence view

Progressively disclose:

- assertion relation and scope;
- evidence class;
- acquisition route;
- derivation root;
- source snapshot and version;
- resolution policy;
- conflicts and alternatives;
- affected occurrences;
- mapping-bundle version;
- revocations and redirects.

### 23.4 Reconciliation workbench layout

Use a stable three-panel layout on large screens:

1. current record and safe state;
2. candidates/comparison;
3. impact and action.

On mobile, preserve this order as resumable steps. Back navigation never loses selections.

### 23.5 Context actions

Core identity/metadata actions:

- Find identifiers.
- Enrich metadata.
- Compare providers.
- Repair record.
- Add external ID.
- Validate identifiers.
- Change display source.
- Change episode order.
- Refresh one provider.
- Clear one disposable cache.
- Mark as not the same.
- Record no known equivalent.
- Preview merge.
- Preview split.
- Export evidence.
- Undo reversible decision.

Right-click is one entry point. The same actions exist in a visible action menu, keyboard command palette, API, and touch action sheet.

### 23.6 Accessibility

- Match state never relies on color.
- Graphs have equivalent lists and tables.
- Every relation has text meaning.
- Focus returns to the triggering record after dialogs.
- Screen readers announce safe state, changed count, and destructive scope.
- Complex comparisons support linear reading order.
- Reduced motion disables graph animation.
- Tables preserve headers and row context.
- Keyboard users can complete every resolution.

---

## 24. Empty states and recovery copy

Every empty state states what is safe, what is missing, and the next action.

| State | Required copy | Primary action | Secondary action |
|---|---|---|---|
| No identifier | “Fasti saved this record, but it does not have an external ID yet. Your activity is safe.” | Find a match | Add an ID |
| One unvalidated ID | “This ID has not been checked yet. Fasti can use the local record while validation is pending.” | Check now | Keep local |
| Offline | “Fasti cannot check sources now. The record is saved locally and will retry.” | View queued work | Add local details |
| Provider not configured | “This source needs a URL or credential before Fasti can use it.” | Configure source | Use another source |
| No mapping bundle | “No mapping bundle is installed. Fasti can still keep and use this record.” | Install bundle | Continue without it |
| No match | “Fasti did not find an equivalent record. It did not guess.” | Search another source | Record no known equivalent |
| Ambiguous | “Fasti found 3 possible matches. It did not choose one.” | Compare matches | Review later |
| Lossy mapping | “This mapping covers only part of the target season. Your history will not move automatically.” | Review coverage | Keep current interpretation |
| Retired ID | “The provider no longer returns this ID. Fasti kept the old reference.” | Find redirect | Keep local reference |
| Known absent | “No counterpart is known in this source. Fasti will not repeat this lookup until the mapping data changes.” | View evidence | Check again |
| Provider outage | “The source is unavailable. Fasti is showing the last good data.” | Retry | View source health |
| Terms prevent storage | “Fasti can query this source, but it cannot store the full response. Allowed identifiers and facts were retained.” | View policy | Choose another source |
| Custom field missing | “This record has no value for GOG Product ID.” | Add value | Hide this field |
| Namespace collision | “This value matches more than one record in this namespace. Fasti did not merge them.” | Review collision | Change namespace rules |
| Stale bundle | “A newer mapping bundle is available. Review its impact before applying it.” | Preview update | Keep current version |
| Wrong merge suspected | “These records may not describe the same media. No activity has been deleted.” | Preview split | Mark for later |
| Empty URL/setup | “No endpoint is configured. Add a URL, import a manifest, or leave this connection disabled.” | Add endpoint | Import definition |

Do not use a blank card, hidden disabled button, or generic “No data” message for any of these states.

---

## 25. User journeys

### Journey 1 — Clean exact import

1. User selects a Floppy or SIMKL export.
2. Fasti inspects without writing.
3. Exact identifiers match existing entities.
4. Preview states created, reused, unresolved, and unchanged counts.
5. User applies.
6. Receipt links to the import report and restore checkpoint.

**Payoff:** The user proves the record can move.

### Journey 2 — SIMKL with IMDb and Kitsu, no MAL

1. Fasti preserves both supplied IDs.
2. It creates a usable Fasti entity.
3. Display metadata resolves through an allowed route.
4. MAL remains optional and pending.
5. Later mapping adds MAL without moving history.

**Payoff:** Partial data is not dead data.

### Journey 3 — Ambiguous anime mapping

1. Import supplies one MAL release.
2. Fasti finds several TVDB ranges.
3. It records the MAL binding and queues topology review.
4. Chronicle remains usable.
5. User compares episode coverage and accepts one scoped mapping.

**Payoff:** The user fixes the difficult part without rebuilding the record.

### Journey 4 — Add a metadata provider

1. User opens Add-ons → Metadata Providers.
2. User imports a manifest or OpenAPI document.
3. Fasti shows hosts, credentials, fields, IDs, storage policy, and capabilities.
4. Sandbox lookup produces normalized claims.
5. User enables the provider for selected media types/fields.
6. Existing entities remain unchanged; resolved views refresh.

**Payoff:** New coverage without waiting for a core release.

### Journey 5 — Change display source

1. User chooses TVDB instead of TMDB for TV presentation.
2. Fasti shows the impact report.
3. User applies.
4. Titles/artwork/order views rebuild.
5. Fasti IDs and Chronicle links remain unchanged.
6. Topology differences remain in review.

**Payoff:** Personal preference without data migration fear.

### Journey 6 — Provider ID disappears

1. Validation marks an ID retired or unavailable.
2. Fasti keeps the original assertion and last-known-good view.
3. Resolver looks for a redirect or exact alias.
4. User sees affected actions.
5. History remains intact.

**Payoff:** External deletion does not erase personal memory.

### Journey 7 — Merge duplicates

1. Fasti proposes two entities as exact duplicates.
2. Preview shows all identifiers, relations, fields, lists, and activity.
3. Conflicts are explicit.
4. User chooses survivor or rejects.
5. Accepted merge tombstones the retired entity with redirect.
6. No occurrence is deleted.

**Payoff:** The library becomes cleaner without hidden loss.

### Journey 8 — Split a wrong merge

1. User selects “These are different.”
2. Fasti shows activity and fields currently grouped.
3. User assigns selected interpretations to new entities.
4. Fasti creates new entities and a split decision.
5. Original observations remain unchanged.

**Payoff:** A bad assumption is repairable.

### Journey 9 — Create a custom identity field

1. User creates a GOG Product ID field for game releases.
2. Fasti requires a namespace definition.
3. User defines format, normalization, uniqueness, and deep link.
4. Test values reveal collisions.
5. Accepted values create identifier assertions, not plain strings.

**Payoff:** Customization remains trustworthy.

### Journey 10 — Record offline, enrich later

1. Player sends a local title and file reference while offline.
2. Fasti stores occurrence and provisional entity.
3. UI shows “Saved locally; identity pending.”
4. On reconnect, enrichment runs.
5. Candidate is accepted or queued.

**Payoff:** The network is not a condition of memory.

### Journey 11 — Mapping-bundle update

1. New bundle is downloaded and verified.
2. Fasti computes added, removed, changed, revoked, and impacted assertions.
3. Preview separates metadata-only changes from history-impacting changes.
4. Reversible changes can be accepted in batch.
5. History reattribution remains explicit.
6. Rollback restores the prior active bundle and overlays.

**Payoff:** Better mappings do not mean surprise rewrites.

### Journey 12 — Exact tracker write

1. User finishes an anime episode.
2. Fasti receives a Kitsu primary ID and IMDb alias.
3. Resolver selects the provider-native Kitsu/MAL/AniList record required by the target tracker.
4. IMDb remains available for metadata lookup.
5. If no exact write route exists, Fasti queues the outbound write and explains why rather than writing to a shared franchise record.

**Payoff:** Cross-provider convenience does not corrupt remote state.

---

## 26. API-first contract

Every identity, metadata, field, reconciliation, and provider action is an application capability before it is a UI action.

### 26.1 Core resource paths

```text
GET    /v1/records/{record_id}
GET    /v1/records/{record_id}/identity
GET    /v1/records/{record_id}/metadata
GET    /v1/records/{record_id}/relations
GET    /v1/records/{record_id}/interpretations

GET    /v1/identity/namespaces
POST   /v1/identity/namespaces
GET    /v1/identity/assertions
POST   /v1/identity/assertions
POST   /v1/identity/assertions/{id}:accept
POST   /v1/identity/assertions/{id}:revoke

POST   /v1/identity/resolutions:plan
POST   /v1/identity/enrichments
POST   /v1/identity/reconciliations:preview
POST   /v1/identity/reconciliations
POST   /v1/identity/merges:preview
POST   /v1/identity/merges
POST   /v1/identity/splits:preview
POST   /v1/identity/splits

GET    /v1/reconciliation/cases
GET    /v1/reconciliation/cases/{id}
POST   /v1/reconciliation/cases/{id}:defer
POST   /v1/reconciliation/cases/{id}:resolve

GET    /v1/metadata/providers
POST   /v1/metadata/providers
POST   /v1/metadata/providers/{id}:test
POST   /v1/metadata/refreshes
GET    /v1/metadata/fields
POST   /v1/metadata/fields

GET    /v1/mapping-bundles
POST   /v1/mapping-bundles:stage
POST   /v1/mapping-bundles/{id}:activate
POST   /v1/mapping-bundles/{id}:rollback

GET    /v1/record-types
POST   /v1/record-types
GET    /v1/field-definitions
POST   /v1/field-definitions
```

Custom methods use one consistent `:action` convention and appear in OpenAPI with stable operation IDs.

### 26.2 Resolution request

```json
{
  "intent": "metadata_lookup",
  "record_type": "anime.release",
  "observed": {
    "identifiers": [
      {"namespace": "kitsu.anime", "value": "12345", "id_source": "simkl.import"},
      {"namespace": "imdb.title", "value": "tt1234567", "id_source": "simkl.import"}
    ],
    "title": "Example",
    "episode": 8
  },
  "target": {
    "provider": "tmdb"
  },
  "policy_version": "resolver/metadata-lookup/1",
  "dry_run": true
}
```

### 26.3 Resolution response

```json
{
  "resolution_id": "fst_res_01J...",
  "status": "exact_scoped",
  "record_id": "fst_ent_01J...",
  "selected": [
    {
      "namespace": "imdb.title",
      "value": "tt1234567",
      "accepted_for": ["metadata_lookup"],
      "relation": "exact"
    }
  ],
  "alternatives": [],
  "lossiness": "none",
  "safe_state": "The local record and Chronicle are unchanged.",
  "explanation": "TMDB accepts the IMDb alias for metadata lookup. The Kitsu ID remains the anime release identity.",
  "evidence": ["fst_evd_01J..."],
  "next_actions": [],
  "policy_version": "resolver/metadata-lookup/1"
}
```

### 26.4 Error contract

```json
{
  "error": {
    "type": "identity_error",
    "code": "identity_ambiguous",
    "message": "Fasti found 3 possible records and did not choose one.",
    "cause": "The supplied title and year are shared by several provider records, and no exact external ID was supplied.",
    "safe_state": "The activity is saved locally against a provisional record.",
    "actual": {
      "title": "Example",
      "year": 2024,
      "candidate_count": 3
    },
    "next_actions": [
      {"action": "review_candidates", "href": "/v1/reconciliation/cases/fst_rec_..."},
      {"action": "add_identifier", "href": "/v1/identity/assertions"}
    ],
    "doc_url": "https://scrobble.dev/knowledge/identity/ambiguous-match"
  }
}
```

### 26.5 Stable error codes

- `identity_ambiguous`;
- `identity_conflict`;
- `identity_known_absent`;
- `identity_namespace_unknown`;
- `identity_value_invalid`;
- `identity_provider_unavailable`;
- `identity_target_retired`;
- `identity_mapping_lossy`;
- `identity_round_trip_failed`;
- `identity_route_not_permitted`;
- `identity_merge_requires_review`;
- `identity_concurrent_decision`;
- `metadata_source_unconfigured`;
- `metadata_storage_not_permitted`;
- `metadata_schema_changed`;
- `field_namespace_required`;
- `field_index_budget_exceeded`;
- `mapping_bundle_signature_invalid`.

### 26.6 Pagination and batch

- keyset pagination for changing reconciliation/record sets;
- bounded batch resolution and enrichment;
- idempotency keys on every mutation;
- `Prefer: respond-async` for large jobs;
- operation resource with progress and resumable checkpoint;
- no endpoint that hydrates an entire library by default.

### 26.7 AsyncAPI events

```text
identity.assertion.observed
identity.assertion.accepted
identity.assertion.revoked
identity.resolution.planned
identity.resolution.applied
identity.reconciliation.opened
identity.entity.merged
identity.entity.split
identity.identifier.retired
identity.mapping_bundle.staged
identity.mapping_bundle.activated
metadata.projection.refreshed
metadata.projection.failed
metadata.resolved_view.changed
record_type.changed
field_definition.changed
```

Events carry Fasti IDs, assertion/decision IDs, workspace/profile context, sequence, schema version, and safe redacted summaries. They do not expose secrets or full signed URLs.

### 26.8 Capability discovery

`/.well-known/fasti` and the Capabilities Ledger declare:

- identity schema versions;
- installed namespaces;
- installed mapping bundles;
- supported resolution intents;
- provider capabilities;
- custom-field support;
- import/export dialects;
- event channels;
- transport bindings;
- limits;
- authentication methods;
- deprecations.

### 26.9 Developer tools

```text
fasti identity inspect <record-id>
fasti identity resolve --intent metadata_lookup --id kitsu.anime:12345 --dry-run
fasti identity add-id <record-id> mal.anime:12345
fasti identity reconcile list
fasti mapping stage bundle.json
fasti provider init
fasti provider test ./provider
fasti fields validate ./record-type.yaml
fasti import inspect export.zip
```

Human-readable output is the terminal default. JSON/YAML output is stable for pipes.

---

## 27. Contracts, knowledge, and source of truth

### 27.1 Canonical definitions

The source-of-truth chain is:

```text
Rust semantic types + domain vocabulary
        ↓
JSON Schema / OpenAPI / AsyncAPI generation
        ↓
Capabilities Ledger
        ↓
JSON-LD/OKF knowledge projections
        ↓
Markdown reference and troubleshooting
        ↓
UI labels, permission text, CLI help, SDK docs
```

Do not hand-maintain seven competing definitions.

### 27.2 Utoipa

Use Utoipa for the Rust/Axum OpenAPI surface where its generated output remains clear and stable. Keep semantic DTOs in the contracts crate. Do not annotate persistence models directly.

### 27.3 AsyncAPI

Generate AsyncAPI from shared event/channel descriptors. Utoipa does not own event contracts.

### 27.4 JSON-LD

Use JSON-LD for:

- portable identity and provenance exports;
- public catalogue/record descriptions;
- Scrobble.dev vocabulary links;
- machine-readable relations;
- provider and authority references;
- knowledge graph ingestion.

Do not require JSON-LD framing in every internal API request.

### 27.5 OKF and KCS

Each identity concept and error has one stable knowledge unit containing:

- user problem;
- plain-language meaning;
- canonical term;
- safe-state statement;
- symptoms;
- causes;
- diagnosis steps;
- fix/recovery;
- examples;
- API/error codes;
- related capabilities;
- source/evidence;
- version and deprecation state;
- conformance fixtures.

OKF packages these units for human and AI retrieval. KCS keeps them attached to real support cases and product changes.

### 27.6 Docs-as-product gate

A feature does not ship until:

- glossary term exists;
- OpenAPI/AsyncAPI/schema is regenerated;
- Capabilities Ledger is updated;
- conformance fixture exists;
- user journey and empty state exist;
- troubleshooting article exists;
- migration/deprecation effect is documented;
- examples pass in CI;
- links resolve.

---

## 28. Capabilities Ledger additions

Identity-related capabilities include:

| Capability ID | Plain-language outcome | Risk |
|---|---|---|
| `identity.record.observe` | Keep supplied IDs and source details without guessing | write |
| `identity.record.read` | See what Fasti knows about a record | read |
| `identity.resolve.plan` | Preview how Fasti would use available identifiers | read-sensitive |
| `identity.resolve.apply` | Apply an accepted resolution | write |
| `identity.enrich` | Look for more identifiers and metadata | external-read |
| `identity.assertion.add` | Add an external identifier claim | write |
| `identity.assertion.accept` | Accept a candidate claim | write-high |
| `identity.assertion.revoke` | Mark a claim as wrong | write-high |
| `identity.known_absent.record` | Record that no counterpart exists | write |
| `identity.negative_assertion.record` | Record that two records are not the same | write |
| `identity.merge.preview` | Show the effect of merging records | read-sensitive |
| `identity.merge.apply` | Merge records and create redirects | destructive |
| `identity.split.preview` | Show the effect of separating records | read-sensitive |
| `identity.split.apply` | Create new records and reassign interpretations | destructive |
| `mapping.bundle.stage` | Download and compare a mapping bundle | external-read |
| `mapping.bundle.activate` | Make a reviewed bundle active | write-high |
| `mapping.overlay.manage` | Add local mapping corrections | write-high |
| `metadata.provider.install` | Add a metadata provider | network-write |
| `metadata.projection.refresh` | Refresh one provider view | external-read |
| `metadata.source.change.preview` | Preview a display-source change | read-sensitive |
| `metadata.source.change.apply` | Apply a display-source policy | write |
| `field.definition.manage` | Add or change a custom field | admin-write |
| `namespace.definition.manage` | Register a custom identifier namespace | admin-high |
| `reconciliation.case.resolve` | Resolve one identity problem | write-high |

Every capability must state data touched, network behavior, offline behavior, scopes, UI surfaces, API operations, events, knowledge article, examples, maturity, and tests.

---

## 29. Settings information architecture

```text
SETTINGS
├── Experience
│   ├── Appearance and theme
│   ├── Sidebar and pins
│   ├── Context actions
│   ├── Accessibility
│   └── Language and region
│
├── Records
│   ├── Media / Record Types
│   ├── Custom Types
│   ├── Custom Fields
│   ├── Notes, tags and filters
│   ├── Calendar, Upcoming and Up Next
│   └── Chronicle and statistics
│
├── Identity & Sources
│   ├── Source preferences
│   ├── Identifier namespaces
│   ├── Mapping bundles
│   ├── Local overlays
│   ├── Resolution policy
│   ├── Reconciliation queue
│   ├── Provider health
│   └── Identity diagnostics
│
├── Metadata & Cache
│   ├── Providers
│   ├── Field source policy
│   ├── Retention presets
│   ├── Storage budgets
│   ├── Artwork cache
│   └── Refresh and failure policy
│
├── Connections
│   ├── Players and Play With
│   ├── Local discovery and pairing
│   ├── API Connections
│   ├── MQTT and Home Assistant
│   ├── Add-ons
│   ├── Notifications
│   ├── RSS/Atom
│   └── External accounts
│
└── System
    ├── Users, profiles and workspaces
    ├── Devices, clients and grants
    ├── Sync and pending work
    ├── Authentication/OIDC
    ├── API, CLI, MCP and AI
    ├── Import/export
    ├── Backup/recovery
    ├── Network/security
    ├── Diagnostics/audit
    ├── Capabilities Ledger
    └── Local knowledge
```

### 29.1 Identity setting principles

- Default to one safe policy, not a mandatory wizard of provider choices.
- Show effective source beside each preference.
- Use “Display source,” not “Primary ID.”
- Show impact before changes.
- Keep advanced namespace and topology controls collapsed.
- Save every long-running review as a resumable draft.
- Search settings by provider, field, capability, error code, and plain-language task.

### 29.2 Portable YAML

Safe preferences can export to YAML:

```yaml
identity:
  policy: safe-default
  auto_accept:
    reversible_exact_aliases: true
    heuristic_matches: false
    entity_merges: false
    history_reattribution: false
  mapping_bundles:
    - id: anibridge
      version: pinned
  media:
    anime:
      display_sources: [tmdb, kitsu]
      export_identifiers: [mal.anime, anilist.anime, kitsu.anime]
    tv:
      display_sources: [tvdb, tmdb]
      episode_order: tvdb.aired
metadata:
  retention: balanced
  discovery_ttl: 7d
```

YAML stores policy and references. Assertions, user state, secrets, receipts, and active reconciliation work remain in SQLite or the encrypted secret store.

---

## 30. Import, export, and migration

### 30.1 Canonical archive additions

```text
manifest.json
record-types.ndjson
field-definitions.ndjson
entities.ndjson
entity-relations.ndjson
external-identifiers.ndjson
identity-assertions.ndjson
mapping-coverages.ndjson
segment-links.ndjson
evidence.ndjson
resolution-decisions.ndjson
reconciliation-cases.ndjson
metadata-claims.ndjson           # policy-permitted only
user-overrides.ndjson
observed-references.ndjson
occurrences.ndjson
occurrence-interpretations.ndjson
progress.ndjson
saved.ndjson
watched.ndjson
ratings.ndjson
lists.ndjson
mapping-bundle-references.json
checksums.json
```

### 30.2 Import classification

Every source record is classified as:

- exact existing binding;
- exact new entity;
- constructable from exact component evidence;
- provisional but usable;
- ambiguous;
- conflicting;
- known absent;
- invalid;
- unsupported;
- source-policy restricted.

No category is silently dropped.

### 30.3 Floppy migration

Current Floppy records are provider-keyed. Migration should:

1. read from a consistent copied database/export;
2. create stable Fasti entities;
3. convert `source + media_id + type + coordinates` into external identifiers and observed references;
4. convert `provider_external_ids` into assertions with the actual acquisition route where known;
5. convert manual metadata into user overrides;
6. preserve history, rewatches, ratings, notes, dates, lists, tags, and collection state;
7. identify duplicates as reconciliation cases rather than merge by title;
8. retain provider migration pins and incompatibility evidence;
9. report orphaned and ambiguous rows;
10. apply no destructive cleanup before a read-only audit.

### 30.4 Yamtrack migration

Use CSV where sufficient and version-aware database import for higher fidelity. Preserve source provider and unresolved records. Do not assume Yamtrack and Floppy schemas are semantically identical because they share ancestry.

### 30.5 SIMKL migration

Preserve all supplied external IDs, session/rewatch semantics, dates, list state, and null conventions. Treat SIMKL aliases as evidence with `simkl` as derivation root, not as automatically authoritative for every target.

### 30.6 Web Scrobbler compatibility

Implement a generic ingress profile that maps Web-Scrobbler-shaped observations into canonical Fasti commands. Do not create a one-off domain integration.

### 30.7 Export fidelity

The export report states:

- which provider data could not be included due to terms;
- which mapping bundle versions are referenced;
- which records are unresolved;
- which assertions are disputed or revoked;
- which custom namespaces/types are required;
- which secrets were omitted;
- how to verify checksums.

### 30.8 Restore

A clean restore must preserve semantic equivalence, not only row counts. Remote credentials and writes return disconnected until the user reconnects them.

---

## 31. Multi-device and offline identity decisions

### 31.1 Sync resources

Synchronize:

- stable entities;
- accepted local assertions;
- user/workspace overrides;
- resolution decisions;
- reconciliation status;
- tombstones and redirects;
- custom type/field definitions;
- mapping bundle references and active policy versions;
- Chronicle interpretation versions.

Large public mapping artifacts are downloaded by hash/version rather than copied through every delta.

### 31.2 Concurrent behavior

| Concurrent action | Result |
|---|---|
| Two devices add different non-conflicting IDs | Keep both assertions |
| Two devices propose conflicting matches | Open one conflict case with both proposals |
| Two devices edit display preference | Ordered workspace setting; preserve change history |
| Merge and split race | Serialize at workspace authority; stale base version fails with preview refresh |
| Bundle update and local overlay edit | Overlay remains separate; rebase preview required |
| Offline occurrence and later mapping | Save occurrence immediately; enrich when connected |

### 31.3 Authority

Every device can create local observations and candidate assertions. Irreversible workspace-level identity decisions use the workspace ordering authority and require the correct grant.

### 31.4 Receipts

Every accepted mutation stores:

- operation ID;
- client/device ID;
- actor/profile/workspace;
- payload digest;
- base version;
- policy version;
- accepted sequence;
- result and affected resource IDs;
- safe rollback/supersession path.

### 31.5 Transport

WebTransport remains the preferred live transport. HTTP snapshot/delta and mutation routes remain the correctness baseline. MQTT does not carry canonical identity synchronization by default.

---

## 32. Security, privacy, and source governance

### 32.1 Main threats

- malicious provider/add-on asserting false identity;
- SSRF through configurable endpoints;
- secret leakage in provider URLs and snapshots;
- title-based mass merge;
- mapping-bundle supply-chain compromise;
- custom namespace collisions;
- cross-user record access;
- unauthorized merge/split;
- poisoned derivation roots;
- provider terms violation;
- AI/MCP performing destructive reconciliation;
- replayed identity mutation;
- cache poisoning;
- enormous graph/JSON input;
- malicious artwork;
- mapping update moving history.

### 32.2 Controls

- signed/hash-verified mapping bundles;
- Safe HTTP host and address policy;
- encrypted secret references;
- provider and add-on capability grants;
- bounded payloads, depth, ranges, and segment fan-out;
- schema validation;
- provenance and derivation-root retention;
- no title-only exact match;
- previews and base versions for merge/split;
- idempotency receipts;
- workspace/profile authorization on every object;
- low-risk auto-apply only;
- audit history;
- local overlay separation;
- source terms and retention policies in provider manifests;
- no arbitrary SQL, JavaScript, Python, or shell in declarative connections;
- MCP defaults to read-only identity tools;
- destructive MCP actions require exact scope and confirmation;
- fuzz/property testing for parsers, normalization, redirects, and mappings.

### 32.3 Licence separation

Fasti may display or cache provider data under one set of terms while a public CC0 mapping artifact requires a much stricter acquisition route. Do not publish a provider identifier assertion to Electric-Town or Scrobble.dev merely because Fasti received it.

### 32.4 AniList boundary — corrected

Fasti is not a competing tracker. Public AniList metadata use does not require
a bespoke permission gate beyond the current documented API terms and limits.
A user token does not by itself authorize a future write or tracker capability;
that separate operation requires its own contract, source review, scopes, and
tests.

### 32.5 Privacy

Provider and identity views can reveal sensitive media activity. Workspace/profile visibility applies to:

- external account bindings;
- source observations;
- reconciliation cases;
- provider fields;
- Chronicle links;
- exported evidence.

Public knowledge/conformance artifacts contain no personal identifiers or user history.

---

## 33. Performance and local-first operation

### 33.1 Database decision

Use SQLite through `rusqlite` as the canonical local database.

Do not introduce a graph database. Identity graphs are sparse, typed, versioned, and usually traversed from known identifiers. Relational tables and explicit indexes are sufficient and easier to back up, inspect, migrate, and operate on low-end self-hosted systems.

### 33.2 Write model

Use one controlled writer for:

- Fasti entities;
- assertions/evidence;
- Chronicle mutation;
- receipts;
- ordered changes;
- outbox;
- notification candidates;
- resolution decisions.

Use bounded read connections and short transactions.

### 33.3 Required indexes

```text
external_identifier(namespace_id, normalized_value, scope_hash)
identity_assertion(external_identifier_id, status, relation)
identity_assertion(subject_entity_id, status)
entity_relation_assertion(subject_entity_id, relation, status)
entity_relation_assertion(target_entity_id, relation, status)
segment_mapping_assertion(subject_entity_id, target_namespace_id, status)
metadata_field_claim(entity_id, field_key, locale, provider_installation_id, status)
reconciliation_case(workspace_id, status, priority, created_sequence)
observed_reference(source_client_id, source_record_id)
entity_tombstone(redirect_target_id)
field_index_projection(field_definition_id, normalized_value)
```

### 33.4 Derived search

Use SQLite FTS for candidate search. Search results are candidates, not accepted matches.

### 33.5 Bounded enrichment

- hydrate visible/anchored records first;
- batch by provider and operation;
- per-host concurrency limits;
- backoff and jitter;
- resumable cursor/checkpoint;
- circuit breaker;
- no one-task-per-library-item fan-out;
- no full graph traversal for normal record display;
- cache resolution plans by policy and bundle version;
- cancel stale background work after provider preference changes.

### 33.6 Initial budgets

These are engineering gates, not published claims until measured:

| Operation | Target |
|---|---:|
| Exact namespace lookup, warm | p95 under 10 ms locally |
| Record identity summary, warm | p95 under 40 ms |
| Record detail with resolved metadata, warm | p95 under 100 ms |
| Reconciliation candidate query, first page | p95 under 250 ms local data only |
| Apply one exact assertion + receipt | p95 under 50 ms |
| Import preview | streaming; first result under 2 seconds |
| UI list | bounded page; no whole-library payload |
| Idle Node Lite memory | target under 96 MB, subject to benchmark |

### 33.7 Scale fixtures

Test at least:

- 10,000 entities / 100,000 occurrences;
- 100,000 entities / 1,000,000 occurrences;
- 1,000,000 identifier assertions;
- 100,000 open reconciliation cases;
- one mapping bundle with millions of directional edges;
- ten concurrent local clients with one writer;
- six months offline then snapshot recovery.

---

## 34. Stack revalidation

The identity review does not justify replacing the selected core stack.

| Area | Decision | State |
|---|---|---|
| Core | Rust | LOCKED |
| Application/API | Axum | LOCKED |
| Async/network runtime | Tokio | LOCKED |
| Canonical local DB | SQLite | LOCKED |
| Rust DB access | rusqlite | LOCKED |
| UI | Svelte 5 + TypeScript + Vite | LOCKED |
| Design system | Tabler Core + Tabler Icons + custom media components | LOCKED |
| Desktop/mobile shell | Tauri 2 | LOCKED, mobile background behavior needs spike |
| Human identity platform | TrailBase as a separate private service | SELECTED; supersedes the historical django-allauth proposal |
| Notifications | Rust domain + private Apprise adapter | LOCKED architecture, adapter spike needed |
| Live transport | WebTransport preferred; HTTP/WebSocket/SSE fallbacks | PROVISIONAL implementation |
| Discovery | DNS-SD over mDNS | LOCKED capability, implementation library provisional |
| Automation | MQTT 5 optional Connection | LOCKED boundary, post-core implementation |
| OpenAPI | Utoipa over contract DTOs | LOCKED with output-quality gate |
| Event docs | AsyncAPI generated separately | LOCKED |
| Knowledge | Markdown + JSON-LD + OKF + KCS | LOCKED |
| Mapping data | Versioned pluggable bundles | LOCKED boundary |
| AniBridge | Optional bundle adapter | PROVISIONAL |
| AniList | Public metadata adapter under current documented terms and limits | SELECTED boundary; authenticated tracker operations remain separate |
| PGlite/Turso/Fjall | No canonical role in 1.0 | REJECTED/DEFERRED |

### 34.1 Why not a graph database

- adds an operational dependency;
- weakens one-file local backup;
- duplicates SQLite state;
- does not solve provenance, policy, or conflict semantics;
- most queries begin from an exact namespace/ID or Fasti entity;
- mapping bundles can be indexed and traversed in bounded form.

### 34.2 Historical Django/allauth decision — superseded

This section is retained as provenance. Django-allauth is not the current
issuer and there is no authentication compatibility layer. TrailBase is the
selected separate human-account service. It must not own Fasti entities,
assertions, resolution policies, Chronicle data, browser sessions,
authorization, profiles, grants, or scopes.

### 34.3 Why not one universal provider abstraction

Metadata, identity, mapping bundles, catalogues, trackers, and players have different failure and authority models. A shared capability manifest and claim envelope provide reuse without erasing those boundaries.

---

## 35. Revised roadmap and initiative prioritization

Prioritization uses:

1. user outcome;
2. foundational leverage;
3. irreversible-risk retirement;
4. dependency order;
5. feasibility;
6. evidence;
7. meaningful payoff.

### Milestone 0 — Identity constitution and conformance

**User capability:** Contributors can state what a record is, what an identifier means, and what must never move silently.

**Epics**

- Fasti identity glossary.
- Stable internal ID contract.
- Namespace and evidence schema.
- Mapping/topology capability corpus.
- Record-type profiles for movie/TV, anime, books, music, podcasts, games, and custom.
- Observation versus interpretation fixture.
- Resolution-intent matrix.
- Metadata field/provenance schema.
- Threat model.
- API/error/capability skeleton.
- Provider and bundle manifest schemas.

**Exit gates**

- SIMKL IMDb + Kitsu/no MAL fixture passes.
- Title-only exact match is impossible in policy fixtures.
- All Electric-Town structural capabilities are expressible.
- A read alias can be accepted while write routing is rejected.
- Work/release/segment distinctions are represented across all target domains.
- No provider ID appears as a Fasti primary key.
- Docs, schemas, examples, and UAT are linked in CI.

### Milestone 1 — Stable local Chronicle and identity alpha

**User capability:** My activity is safe even when Fasti does not yet know exactly what the media is.

**Epics**

- SQLite schema and controlled writer.
- Stable entities, observed references, external IDs, assertions, evidence.
- Chronicle occurrence and interpretation versioning.
- Exact movie/TV provider bindings.
- Provisional records.
- Basic metadata field claims and user overrides.
- Identity summary UI and empty states.
- Local API/CLI.
- Multi-user workspaces/profiles.
- Backup/export/restore.

**Exit gates**

- Unknown/offline record survives restart and clean restore.
- Provider outage does not block local recording.
- User can add and inspect an external ID.
- Original observation remains after correction.
- API/UI/CLI produce the same application result.
- No external service is required.

### Milestone 2 — Enrichment, reconciliation, and migration alpha

**User capability:** I can move an existing history and repair the exceptions without losing it.

**Epics**

- Resolution planner.
- Reconciliation cases/workbench.
- Metadata provider registry and two native providers.
- Provider preference impact preview.
- Floppy import.
- Yamtrack import.
- SIMKL compatibility import.
- Exact/ambiguous/retired/known-absent handling.
- Merge preview and tombstone redirect.
- Source health and retry diagnostics.

**Exit gates**

- Real Floppy import retains all Chronicle/user state.
- Partial SIMKL IDs heal without record replacement.
- Display-source switch changes no Fasti IDs/history links.
- Repeat import is idempotent.
- Ambiguous rows remain usable and reviewable.
- Merge requires explicit preview and receipt.

### Milestone 3 — Anime topology and player interoperability alpha

**User capability:** My anime and episodic history remains correct across players that use different IDs and numbering.

**Epics**

- Electric-Town conformance runner.
- AniBridge bundle adapter and staged updates.
- Local mapping overlays.
- MAL/Kitsu/TMDB/TVDB adapters as permitted.
- AniList public-metadata adapter fixtures and current terms review.
- Nuvio exact write/read routing.
- Kodi/Stremio/Nuvio observation profiles.
- Play With.
- Context actions.
- Calendar, Upcoming, and Up Next.

**Exit gates**

- ranges, offsets, discontinuities, expansion, merge, and numbering spaces pass;
- provider-native write route differs safely from metadata alias route;
- bundle update never silently moves history;
- two external players produce exact state;
- Fasti works with no mapping bundle installed;
- AniList metadata stays disabled until its adapter, limits, attribution, and conformance evidence pass.

### Milestone 4 — Custom records, fields, and provider SDK beta

**User capability:** I can add a media field, source, or simple provider without waiting for the Fasti core team.

**Epics**

- custom record-type registry;
- custom field definitions and validated documents;
- identity-capable custom namespaces;
- derived field indexes;
- provider SDK and CLI scaffolding;
- OpenAPI import into Connection Studio;
- Google Books-style reference provider;
- game, podcast, music, and book domain adapters;
- add-on manifests and permissions;
- context-action contributions.

**Exit gates**

- new simple provider produces a field claim in under 15 target minutes;
- custom identity field cannot bypass namespace rules;
- field schema migration round-trips;
- custom type survives export/restore;
- one book, music, podcast, and game fixture proves non-video semantics;
- add-on cannot mutate Chronicle without declared capability.

### Milestone 5 — Multi-device identity and sync beta

**User capability:** My devices can improve the same record while disconnected without overwriting one another.

**Epics**

- device identities and grants;
- local outbox;
- identity/assertion delta resources;
- conflict cases;
- mapping bundle version negotiation;
- WebTransport live channels;
- HTTP snapshot/delta fallback;
- mDNS pairing;
- Tauri desktop/mobile;
- notification rules and Apprise delivery.

**Exit gates**

- concurrent non-conflicting assertions coexist;
- merge/split race is rejected safely;
- six-month offline recovery preserves tombstones;
- mobile suspension does not lose local outbox;
- WebTransport failure falls back without correctness loss;
- pairing is explicit and revocable.

### Milestone 6 — Open ecosystem and 1.0 hardening

**User capability:** I can trust, extend, migrate, diagnose, and leave Fasti.

**Epics**

- Connection Studio hardening;
- MQTT/Home Assistant;
- MCP/OpenAI/Claude packages;
- public provider/add-on SDK;
- import/export ecosystem;
- signed releases and SBOM;
- performance and low-resource qualification;
- security audit;
- KCS/OKF knowledge packs;
- governance and release policy;
- conformance badges through Scrobble.dev.

**Exit gates**

- restore equivalence passes;
- threat-model gates pass;
- provider and mapping update rollback passes;
- public contracts have version/deprecation policy;
- documentation examples execute in CI;
- performance budgets pass on reference hardware;
- user tests prove import, repair, provider change, and recovery journeys.

### 35.1 Kill/defer list

- Kill “primary external ID.”
- Kill provider IDs in canonical record URLs.
- Kill title-only automatic merge.
- Kill metadata provider writes to user history.
- Kill hidden mapping-bundle auto-updates.
- Kill one global numeric identity confidence.
- Kill full-catalogue background hydration.
- Kill arbitrary scripts in declarative Connections.
- Defer graph database.
- Defer automatic public overlay publication.
- Defer executable WASI plugins.
- Defer Cloudflare deployment until local product and identity contracts pass.
- Defer AniList metadata support until the adapter and current-terms evidence pass; keep authenticated tracker operations separate.

---

## 36. Epics and implementation stories

### Epic I-1 — Stable entity identity

- As Fasti, I assign one stable opaque ID independent of providers.
- As a user, I can open the same record after changing metadata sources.
- As an importer, I can resolve by any known external ID.
- As support, I can inspect redirects and lifecycle.

**Acceptance:** external provider changes do not change canonical Fasti URLs or Chronicle references.

### Epic I-2 — Namespace registry

- As an adapter author, I can declare identifier grain, format, normalization, and deep link.
- As an administrator, I can add a local namespace safely.
- As Fasti, I reject a collision or undeclared namespace with an actionable error.

### Epic I-3 — Evidence and provenance

- As a user, I can see where an ID came from.
- As Fasti, I retain derivation root and source snapshot.
- As a mapping maintainer, I can revoke one assertion without deleting the record.

### Epic I-4 — Purpose-specific resolver

- As a metadata adapter, I can request a lookup identity.
- As a tracker adapter, I can request an exact write identity.
- As a player, I can request a Play With route.
- As Fasti, I explain why the routes differ.

### Epic I-5 — Reconciliation

- As a user, I can review ambiguous candidates.
- As a user, I can mark two records as not the same.
- As a user, I can defer a case without blocking the Chronicle.
- As support, I can export a case report.

### Epic I-6 — Metadata projections

- As a user, I can select a display source without identity migration.
- As a user, I can inspect field provenance.
- As Fasti, I keep last-known-good metadata during an outage.
- As a provider author, I emit claims rather than domain rows.

### Epic I-7 — Custom fields

- As a user, I can add a field to one record type.
- As a user, I can make it filterable within a storage budget.
- As a user, I can register an identity namespace for an ID field.
- As Fasti, I validate extension documents and rebuild derived indexes.

### Epic I-8 — Mapping bundles

- As an administrator, I can stage and inspect a bundle.
- As a user, I can see which records and occurrences would be affected.
- As Fasti, I verify schema and hash.
- As an administrator, I can roll back.

### Epic I-9 — Safe migration

- As a Floppy user, I see exact created/reused/unresolved counts.
- As a user, I can stop and resume.
- As Fasti, I never delete an ambiguous source row.
- As a user, I can restore the pre-import checkpoint.

### Epic I-10 — Knowledge and developer experience

- As a developer, I can run a provider example without reading architecture docs.
- As a user, every identity error states what is safe.
- As support, one error code links to one current knowledge unit.
- As an add-on author, I can run public conformance locally.

---

## 37. User and developer personas

Personas are used to test decisions. They are not market segments that force separate products.

### 37.1 Household Chronicle owner

**Context:** Runs one Fasti instance for a household. Uses several televisions, phones, computers, and profiles.

**Job:** Keep each person’s media record correct without becoming the family database administrator.

**Current pain:** Duplicate titles, wrong profiles, opaque sync status, provider keys, and fear that changing a setting will damage history.

**Fasti outcome:** The household can record locally, pair devices, see who owns each occurrence, repair a bad match, restore a backup, and understand what remains unresolved.

**Success evidence**

- setup completes without a terminal;
- one device records while the internet is unavailable;
- a second device later receives the result once;
- one profile cannot see another profile’s private notes without permission;
- provider changes do not move history.

### 37.2 Tracker migrant

**Context:** Has years of Floppy, Yamtrack, SIMKL, Trakt, MAL, AniList, ListenBrainz, or other history.

**Job:** Move the record without losing timestamps, repeat events, notes, ratings, lists, or uncertain items.

**Current pain:** Imports often flatten multiple plays, discard unmapped rows, or require one provider’s ID.

**Fasti outcome:** The user previews the import, sees exact/reused/unresolved counts, resumes after interruption, and can repair uncertain records later.

### 37.3 Anime specialist

**Context:** Uses MAL, Kitsu, AniList, SIMKL, TMDB, TVDB, media servers, and seasonal charts.

**Job:** Keep cours, specials, films, split seasons, absolute order, regional release details, and repeat watches correct across incompatible catalogues.

**Current pain:** One-to-one crosswalks, season-zero assumptions, stale provider IDs, and silent remaps.

**Fasti outcome:** Directional range mappings, explicit segment links, negative assertions, evidence, bundle versions, and a reviewable overlay preserve the topology.

### 37.4 Player-first user

**Context:** Watches in Nuvio, Kodi, Stremio, Jellyfin, VLC, or another player.

**Job:** Continue using the preferred player while Fasti keeps one dependable record.

**Current pain:** Each player has a different ID, progress rule, and callback shape.

**Fasti outcome:** `Play With` chooses a compatible route. Player observations enter through compatibility profiles. The Chronicle remains independent of player availability.

### 37.5 Accessibility and AuDHD user

**Context:** Needs stable context, limited simultaneous choices, keyboard access, clear state, and resumable tasks.

**Job:** Complete setup, review uncertain matches, and recover from failure without remembering hidden state.

**Current pain:** Large settings walls, transient toast messages, unexplained scores, and non-resumable wizards.

**Fasti outcome:** One recommended path, persistent status, progressive disclosure, safe defaults, saved review queues, reduced motion, and direct language.

### 37.6 Custom record-type owner

**Context:** Tracks concerts, courses, tabletop campaigns, visual novels, fan works, or a domain not shipped by Fasti.

**Job:** Define useful fields, identity namespaces, progress units, actions, and metadata without forking the application.

**Current pain:** Generic notes are too weak; unrestricted schema systems become unsearchable.

**Fasti outcome:** Versioned record-type and field definitions provide typed fields, bounded indexes, namespace-backed identifiers, and generated forms and APIs.

### 37.7 Integration developer

**Context:** Connects a small or obscure API to Fasti.

**Job:** Produce the first valid read or write quickly, without learning internal storage.

**Current pain:** Hidden business rules, undocumented IDs, ambiguous errors, and integrations that require repository changes.

**Fasti outcome:** Capability discovery, generated contracts, a Connection Studio, a CLI, test fixtures, sandbox mode, and errors that state problem, cause, repair, and documentation.

### 37.8 Metadata-provider author

**Context:** Supplies metadata for one domain, region, language, or collection.

**Job:** Return searchable candidates and field claims without owning Fasti entities or user history.

**Current pain:** Provider implementations must touch models, routes, templates, settings, search, and caches separately.

**Fasti outcome:** One provider manifest, one adapter contract, one conformance suite, and field-level claim output.

### 37.9 Mapping maintainer

**Context:** Maintains an identity or topology dataset.

**Job:** Publish mappings, corrections, revocations, known absences, and provenance safely.

**Current pain:** Consumers lose source lineage, flatten topology, or update silently.

**Fasti outcome:** Immutable bundle manifests, staged impact reports, assertion identifiers, overlays, revocations, and public conformance fixtures.

### 37.10 Support and operations maintainer

**Context:** Diagnoses failed imports, stale providers, mismatches, and synchronization problems.

**Job:** Determine what happened without collecting private library contents unnecessarily.

**Current pain:** Logs expose provider URLs but not the decision path; users cannot explain what they saw.

**Fasti outcome:** Correlation IDs, redacted decision traces, KCS articles, deterministic case exports, and clear recovery commands.

---

## 38. UAT and conformance plan

### 38.1 Test layers

| Layer | Purpose | Required evidence |
|---|---|---|
| Domain unit tests | Prove state and identity invariants | deterministic tests for every command and relation |
| Property tests | Explore large transition spaces | no duplicate receipt, no redirect cycle, no unsafe merge |
| Schema tests | Validate contracts and extension documents | positive and negative fixtures |
| Provider contract tests | Prove adapter normalization | recorded fixtures, pagination, errors, unknown fields |
| Mapping conformance | Prove topology expressiveness | Electric-Town capability corpus plus Fasti cases |
| Migration replay | Prove old data survival | real versioned exports and copied databases |
| Offline/system tests | Prove local-first behavior | process kill, network loss, reconnect, cursor expiry |
| Multi-user tests | Prove isolation and attribution | workspace/profile/device/grant matrix |
| Accessibility tests | Prove operability | automated checks plus keyboard and screen-reader scripts |
| Performance tests | Prove bounded work | reference libraries and low-resource budgets |
| Security tests | Prove network and extension boundaries | SSRF, secret, auth, replay, bundle, and parser cases |
| Documentation tests | Prove examples still work | executable snippets and link validation |

### 38.2 Load-bearing identity scenarios

The full machine-readable matrix is a separate artifact. These are the release-blocking groups.

#### Stable entity

1. Import a movie with only IMDb. Add TMDB later. The Fasti entity ID remains unchanged.
2. Change display metadata from TMDB to TVDB. Chronicle links remain unchanged.
3. Retire an external ID. The external identity becomes a tombstone or inactive assertion; the Fasti entity remains valid.
4. Export and restore. Fasti IDs, assertions, evidence, and redirects remain equivalent.

#### Partial evidence and healing

5. Import SIMKL anime with IMDb and Kitsu but no MAL. Record remains usable.
6. Resolve IMDb to TMDB for artwork while Kitsu remains the safe anime tracker route.
7. Add MAL through a mapping bundle. Preview before acceptance.
8. Provider outage occurs during enrichment. Existing claims remain available and the job remains resumable.
9. A source returns an ID that later produces 404. Mark stale/invalid; do not delete the entity or occurrence.

#### Topology

10. One anime release maps to part of a TVDB season with a non-zero offset.
11. One release spans two target seasons.
12. One source episode expands to two target episodes.
13. Two source episodes merge into one target episode.
14. A special and regular episode both use number 1 in different numbering spaces.
15. Two identically titled works have a `not_same_as` assertion and never merge.
16. An alternate cut remains a separate entity linked by relation.
17. One release belongs to two series groupings without becoming two releases.

#### Historical interpretation

18. A new bundle changes a mapping candidate. Existing occurrence interpretation does not move automatically.
19. User accepts reinterpretation for one occurrence only.
20. User applies a reviewed reinterpretation to a bounded set and sees an exact diff.
21. User rolls back the interpretation decision without deleting the original observation.

#### Provider switching

22. TV metadata preference changes from TMDB to TVDB. Fasti runs an impact preview.
23. Exact match exists. New metadata claims become active with no history migration.
24. Episode topology differs. Record remains pinned for unsafe purposes and enters reconciliation.
25. User cancels the change. No state or cache preference changes.

#### Custom fields and namespaces

26. User adds `gog_product_id` as an identifier field for games.
27. The namespace declares grain, format, normalization, URL template, and uniqueness scope.
28. A Steam ID and GOG ID attach to one game entity without either becoming canonical.
29. A custom field schema changes. Existing values migrate or remain readable under the prior version.
30. A field selected for indexing exceeds the storage budget. Fasti explains and blocks the unsafe index.

#### Empty and degraded states

31. New instance has no providers. Manual record creation and local Chronicle still work.
32. Record has no artwork, URL, or resolvable provider. UI shows a usable local record, not a broken card.
33. Search provider is disabled. Existing records remain accessible.
34. Mapping bundle is absent. Fasti explains that exact cross-provider actions may be unavailable but local records remain safe.
35. Reconciliation queue is empty. UI explains what will appear there and offers no fake activity.

### 38.3 Required migration classifications

Every imported row ends in one of these explicit states:

```text
created
reused_exact
reused_with_new_assertion
candidate_match
needs_topology_review
known_absent
unsupported_but_preserved
invalid_source_record
failed_retryable
failed_terminal
```

No row is silently discarded.

### 38.4 Release conformance gates

A release cannot claim identity interoperability until:

- every Electric-Town structural capability has at least one passing Fasti fixture;
- exact and lossy routes are distinguished;
- target-existence checks are covered;
- source lineage is retained;
- mapping update impact is previewed;
- negative assertions prevent repeated bad matches;
- historical occurrences do not move without an explicit decision;
- import, export, backup, and restore preserve unresolved evidence.

---
## 39. Blind audit A — fifty expert-role lenses

**Method:** Fifty synthetic expert roles reviewed the stated product problem independently. Each first-pass lens received the requirements and evidence pack, but not the findings of the other roles. These are analytical roles, not claims of participation by real people.

| # | Expert lens | First-pass finding | Main condition or challenge |
|---:|---|---|---|
| 1 | Product strategist | Fasti has a defensible category when it owns the record rather than playback. | Do not let identity infrastructure hide the first user payoff. |
| 2 | Kathy Sierra user-success coach | Repairability can make users better at owning their media history. | Teach only at the moment of a real decision; do not make users study graph theory. |
| 3 | Domain-model architect | Identity, metadata, Chronicle, sync, and catalogue are separate domains. | Keep one shared vocabulary and ban provider terms from the domain core. |
| 4 | Rust systems engineer | Rust fits the local authority and contract-heavy core. | Keep domain crates independent of Tokio, Axum, SQLite, and provider SDKs. |
| 5 | SQLite engineer | SQLite is sufficient with one controlled writer and bounded reads. | Prove crash atomicity for mutation, receipt, sequence, and outbox in one transaction. |
| 6 | Distributed-systems engineer | Multi-writer clients still need one accepted ordering authority per workspace. | Define offline-local-only mode and failover before promising peer-to-peer sync. |
| 7 | Local-first engineer | Original observations must be written before remote resolution. | Never block a local occurrence on metadata or identity certainty. |
| 8 | Knowledge-graph modeller | Typed edges and provenance fit the problem, but a graph database is unnecessary. | Use relational tables with explicit traversal limits and explain plans. |
| 9 | Data-migration engineer | Preservation and classification matter more than automatic match rate. | Keep source payload, row identity, checkpoint, and rollback evidence. |
| 10 | Metadata architect | Field-level claims solve provider switching better than one active provider row. | Define merge policy per field and locale, not one global provider ranking. |
| 11 | TMDB adapter engineer | TMDB is useful for presentation and common video identity. | Do not treat TMDB seasons as universal episode truth. |
| 12 | TVDB adapter engineer | TVDB order choices can be operationally important for media servers. | Store order/coordinate assertions separately from work identity. |
| 13 | IMDb identity engineer | IMDb is a valuable bridge identifier. | It is not a complete metadata or episode-topology contract. |
| 14 | Anime mapping engineer | One-to-one mapping is structurally wrong for common high-traffic anime. | Range, offset, discontinuity, cardinality, numbering space, and negative assertions are mandatory. |
| 15 | MAL integration engineer | MAL may be a provider coordinate, not the Fasti identity. | Avoid mandatory MAL resolution and preserve inaccessible/deleted IDs. |
| 16 | AniList API and policy reviewer | AniList relations and schedules are useful evidence. | Public metadata follows current terms and limits; authenticated tracker operations are separate; no architectural dependency. |
| 17 | Kitsu integration engineer | Kitsu IDs often arrive in add-on and SIMKL workflows. | Retain them even when the active display provider cannot use them. |
| 18 | SIMKL integration engineer | SIMKL supplies useful cross-provider aliases and tracker state. | Preserve route provenance and do not assume its mappings are current or complete. |
| 19 | Nuvio/Stremio engineer | Read enrichment can use broad aliases while exact writes need provider-native IDs. | Resolver intent must be explicit in every call. |
| 20 | Kodi integration engineer | Kodi can provide local unique IDs and playback observations. | Local database IDs stay source-scoped; only declared external IDs enter global namespaces. |
| 21 | MusicBrainz modeller | Music proves work, recording, release, and track are distinct grains. | Do not force video hierarchy terms onto music. |
| 22 | Book metadata engineer | Work and edition must be distinct. | ISBN identifies an edition, not necessarily the abstract work. |
| 23 | Podcast standards engineer | Feed URL is not stable identity; podcast and episode GUIDs matter. | Preserve feed moves and never title-match episodes automatically. |
| 24 | Game metadata engineer | Storefront IDs represent editions/products across Steam, GOG, Epic, and consoles. | Model game work, release/edition, platform, and ownership separately. |
| 25 | Custom-schema engineer | Custom fields are required for long-tail identity and metadata. | Use versioned typed definitions, not unrestricted EAV or arbitrary SQL. |
| 26 | API designer | All UI and integration functions should map to application capabilities. | Do not expose CRUD that bypasses identity decisions or domain rules. |
| 27 | Developer-experience reviewer | The provider SDK can be excellent if the first working adapter is under 30 minutes. | Ship fixtures, generated types, validation, local sandbox, and exact errors. |
| 28 | CLI designer | CLI is the best advanced and recovery surface. | Commands must support dry-run, JSON output, idempotency, and resumable job IDs. |
| 29 | KCS/documentation engineer | Identity failures will be a major support category. | Error codes, decision traces, glossary, and recovery articles must share one source. |
| 30 | OKF/JSON-LD knowledge engineer | Fasti can expose a high-value machine-readable knowledge surface. | Do not confuse linked-data output with the internal transactional model. |
| 31 | MCP/AI engineer | AI can assist candidate ranking and explain provenance. | AI must never author accepted identity truth or destructive merges without review. |
| 32 | Application-security engineer | Provider URLs, manifests, bundles, imports, and plugins create several trust boundaries. | Centralize network policy, secret storage, parser limits, signatures, and scopes. |
| 33 | SSRF specialist | Generic API Connections and metadata providers can reach protected networks. | Deny private/link-local/metadata destinations by default and revalidate redirects and DNS. |
| 34 | Privacy engineer | Media history and unresolved source evidence are sensitive household data. | Minimize logs, separate profiles, encrypt secrets, and make sharing explicit. |
| 35 | Identity/auth engineer | Account identity and media identity are unrelated bounded contexts. | Historical django-allauth detail is superseded. Keep TrailBase issuer and subject details outside the media identity model. |
| 36 | Mobile platform engineer | Offline queues and background limits will dominate mobile reliability. | Prove suspension, key storage, and delayed retry before promising invisible sync. |
| 37 | Tauri engineer | Shared web UI can reach desktop and mobile quickly. | Keep native plugins bounded and do not put domain logic in the webview. |
| 38 | Accessibility engineer | Reconciliation can become inaccessible if represented only as a graph or confidence color. | Provide list/table views, textual evidence, keyboard paths, and persistent focus. |
| 39 | ADHD/AuDHD interaction designer | The plan risks too many expert settings. | Provide one recommended path, saved checkpoints, and progressive disclosure. |
| 40 | Notification architect | Identity changes and failed deliveries need durable notification semantics. | In-app inbox is canonical; Apprise is only delivery. |
| 41 | SRE/operability engineer | Providers and bundles will fail in partial, stale, and repeated ways. | Health, lag, queue depth, circuit state, and last-known-good status need low-cardinality metrics. |
| 42 | Performance engineer | Resolution graphs can create unbounded traversal and provider fan-out. | Bound hops, candidates, calls, time, memory, and visible-item hydration. |
| 43 | QA/property-testing engineer | Identity correctness needs generative tests, not only fixtures. | Generate redirect cycles, duplicate assertions, topology overlaps, retry races, and order conflicts. |
| 44 | Release engineer | Schema, policy, providers, bundles, and application releases will version independently. | Publish compatibility matrices, signed artifacts, SBOM, and rollback paths. |
| 45 | Open-source governance reviewer | Scrobble.dev can convene without forcing Fasti. | Keep neutral schemas and conformance separate from Fasti-only implementation choices. |
| 46 | Data-rights/licensing reviewer | Provider responses usable in an app may not be reusable in a public mapping dataset. | Record acquisition route and license posture; separate transient claims from publishable assertions. |
| 47 | Homelab/self-hosting user | One process and one database remain the strongest default. | Sidecars must be optional or clearly managed, backed up, and diagnosable. |
| 48 | Multi-user household tester | Profiles, account users, devices, and provider accounts must not collapse. | Every mutation needs explicit workspace, profile, actor, and client attribution. |
| 49 | Support engineer | Users need to know what remains safe during unresolved states. | Every failure message must state saved data, blocked action, and exact next step. |
| 50 | Maintainer-capacity reviewer | The full plan is too large for one early release. | Make identity foundation narrow, delay marketplace, cloud, broad provider set, and executable plugins. |

### 39.1 Fifty-lens convergence

The independent roles strongly agree on seven points:

1. Fasti must own a stable local entity ID.
2. Original observations must survive unresolved identity.
3. Provider switching must alter projections, not silently migrate history [S6, S10].
4. Identity needs typed assertions, evidence, provenance, topology, and lifecycle.
5. The resolver must be purpose-specific.
6. The user interface must hide complexity until it is needed while preserving auditability.
7. The first release must prove one dependable migration-to-Chronicle path before broad platform work.

### 39.2 Fifty-lens disagreements

| Disagreement | Position A | Position B | Final resolution |
|---|---|---|---|
| Entity hierarchy | One universal work/release/segment model | Per-domain models only | Use common roles plus domain profiles; do not force all domains into one tree. |
| Automatic healing | Auto-accept high-score candidates | Require review for every new assertion | Auto-accept only reversible, policy-approved assertions; never auto-merge or move history. |
| External mapping data | Depend on best current service | Vendor immutable snapshots | Prefer verified local snapshots and overlays; live services may provide candidates. |
| Graph database | Natural fit for identity graph | Operational overhead is unjustified | SQLite relational model first. |
| AniList | Strong default anime source | Terms, limits, and availability require governed use | Design the public-metadata adapter and fixtures; ship only after current-terms and conformance evidence. |
| User transparency | Show all evidence | Avoid overwhelming ordinary users | Summary first, expandable evidence, full diagnostics and export. |
| Custom fields | Maximum freedom | Strong schema governance | Typed versioned fields with bounded indexes and namespace registration. |

---

## 40. Blind audit B — ADHD and AuDHD panel

**Method:** A separate synthetic panel reviewed only the user tasks, information architecture, setup, recovery, settings, and developer workflow. It did not receive the fifty-role findings.

### 40.1 Panel verdict

**Gated approval.** The identity architecture is safer than a hidden provider key, but the reconciliation product can become cognitively hostile unless these rules are release gates.

### 40.2 Required interaction principles

1. **One recommended path.** Advanced identity controls must not appear during ordinary recording.
2. **Persistent context.** Review pages retain filters, selected record, scroll position, evidence expansion, and return destination.
3. **Resume every long task.** Imports, enrichment, mapping updates, provider switches, and reconciliation have stable job IDs and saved checkpoints.
4. **State before explanation.** First line says what happened and whether the user’s record is safe.
5. **No score without meaning.** Never show “87% match” without factors, evidence, and the consequence of accepting it.
6. **No color-only semantics.** Exact, candidate, disputed, stale, and blocked have text and icons.
7. **Avoid forced review.** Unresolved items remain useful and can be handled later.
8. **Batch with safeguards.** Users can approve repeated safe patterns after reviewing examples and an impact summary.
9. **Stable vocabulary.** Use the same words in UI, API, docs, logs, and support.
10. **No transient-only feedback.** Toasts may summarize; persistent job and inbox entries hold the result.
11. **Keyboard and touch equality.** Right-click actions also appear in an accessible action menu.
12. **Reduced motion and density.** No animated graph layout; compact and comfortable density are user choices.

### 40.3 Reconciliation screen pattern

```text
17 records need review
Nothing has been changed.

[Review recommended matches]  [Later]

Current record
Frieren Mini Anime
Known IDs: Kitsu 12345 · IMDb tt...

Recommended match
MyAnimeList 56885

Why Fasti suggests this
✓ Same Kitsu-linked release
✓ Overlapping air dates
✓ Same native title
! Episode grouping differs

What accepting this does
• Adds the MAL identifier
• Keeps the current Fasti record and history
• Does not change episode mappings

[Accept identifier only]  [Compare topology]  [Not the same]  [Skip]
```

### 40.4 Empty-state rules

An empty state must answer four questions:

1. What is this page for?
2. Why is it empty now?
3. Is anything wrong or at risk?
4. What is the smallest useful next action?

Example:

> **No identity reviews are waiting.** Fasti will add a review here when a provider change, import, or mapping update cannot be applied safely. Your current records do not need attention.

Not:

> No data found.

### 40.5 Developer workflow findings

- Provider templates need one command to scaffold, one local fixture, and one conformance command.
- Validation errors should point to the manifest path and expected schema.
- A failed test must print an exact replay command.
- Local documentation must be searchable without internet access.
- Generated files must state their source and regeneration command.
- Upgrade errors must preserve the last valid configuration and provide a rollback command.

---

## 41. Blind audit C — frontier panel

**Method:** A separate synthetic panel reviewed current and emerging technical paths without seeing the product or accessibility panel conclusions.

### 41.1 Adopt now

| Path | Decision | Reason |
|---|---|---|
| Stable local IDs | Adopt | Durable, provider-independent, and cheap to preserve. |
| Typed assertion graph in SQLite | Adopt | Expressive enough without graph-database operations. |
| State plus ordered change log | Adopt | Supports local reads, audit, sync, and recovery without full event-sourcing complexity. |
| Immutable mapping bundles and overlays | Adopt | Makes updates reproducible and reversible. |
| Field-level metadata claims | Adopt | Enables source switching and provenance. |
| Local outbox and receipts | Adopt | Required for disconnection tolerance. |
| OpenAPI/AsyncAPI/JSON Schema | Adopt | Needed for API-first clients and add-ons. |
| Machine-readable capability and knowledge outputs | Adopt | Improves human and agent discovery without giving AI authority. |

### 41.2 Prototype after the foundation

| Path | Decision | Required proof |
|---|---|---|
| WebTransport | Preferred live transport after sync semantics | HTTPS/proxy/mobile migration benchmark and fallback interoperability. |
| mDNS/DNS-SD | Native alpha | Cross-platform lifecycle, pairing security, VLAN/VPN behavior, and manual fallback. |
| MQTT 5 | Optional alpha/beta Connection | Duplicate delivery, retained cleanup, TLS/ACL, queue bounds, and Home Assistant UX. |
| WASI components | Later | Stable capability permissions, signing, resource limits, and two real plugin cases. |
| PGlite/SQLite-Wasm | Research | Browser-local value without a second domain implementation. |
| Turso/libSQL | Research | Clear benefit over application-owned sync and acceptable self-hosting posture. |
| Local AI ranking | Later assistive feature | Deterministic evidence display, privacy budget, and no accepted-truth authority. |
| Cloudflare profile | Later | Local product, auth abstraction, storage mapping, and cost/restore proof. |

### 41.3 Reject as core strategy

- CRDTs for all media state;
- provider databases as canonical Fasti storage;
- graph database in the first architecture;
- WebTransport as the only path;
- MQTT as synchronization correctness;
- AI-generated canonical identity;
- automatic transitive closure across all mappings;
- automatic history movement after bundle updates;
- arbitrary code in declarative API Connections;
- browser storage as the only authoritative household record.

### 41.4 Frontier verdict

The future-facing choice is not one novel transport or database. It is a **stable semantic core with replaceable transports, storage adapters, mapping bundles, providers, and clients**. Novel components are allowed where they improve measured user capability and remain removable.

---

## 42. Blind audit D — open-source panel

**Method:** A separate synthetic panel reviewed governance, ecosystem behavior, licensing, contribution, security, and maintainer load without seeing the other panel outputs.

### 42.1 Panel verdict

**Conditional approval.** Fasti can enter the ecosystem constructively if it publishes neutral contracts and gives accurate credit. It will look hostile if it markets itself as the corrected version of Floppy, Yamtrack, Nuvio, AniBridge, or any mapping project.

### 42.2 Required project boundaries

| Surface | Owner and posture |
|---|---|
| Fasti application | Opinionated reference implementation and product |
| Scrobble.dev | Neutral knowledge, vocabulary, schemas, examples, and conformance |
| Electric-Town anime-crosswalk-mappings | Independent CC0 design, policy, and future data project for anime mappings |
| AniBridge | External mapping implementation/data source adapter where compatible |
| Floppy/Yamtrack | Existing projects that may receive bounded fixes and migration support |
| Nuvio/Kodi/Stremio/etc. | Players and clients Fasti supports; not products Fasti replaces |

### 42.3 Contribution model

- Small bugs and docs can use focused pull requests.
- Identity semantics, schema, sync, security, extensions, and migrations require an RFC.
- Every accepted behavioral change includes fixtures and migration/deprecation impact.
- Provider-specific work stays in adapters unless the domain evidence proves a shared primitive.
- Existing contributor authorship and issue ownership remain visible.
- Do not edit another contributor’s issue or pull-request body to replace their narrative.
- Avoid automated comment volume and generic generated reviews.
- Upstream contributions should solve one demonstrated problem and retain upstream conventions.

### 42.4 Licensing and data posture

- Application code license must be selected before code copying.
- Public neutral schemas and conformance fixtures should use a permissive, clearly stated license where possible.
- Provider metadata terms are reviewed per adapter.
- A provider ID learned during normal application use is not automatically admissible to a CC0 public mapping dataset.
- Acquisition route and derivation root must survive.
- User overlays are private by default and are not submitted upstream without explicit review and consent.
- Mapping bundles carry license, source, version, manifest hash, and revocation information.

### 42.5 Sustainability controls

- one canonical issue tracker and release channel;
- CODEOWNERS by bounded context;
- public compatibility policy;
- supported-version matrix;
- security reporting route;
- reproducible release process;
- signed checksums and SBOM;
- provider/add-on maturity labels;
- deprecation windows;
- automated docs and conformance checks;
- no marketplace before reporting, revocation, moderation, and signature policy exist.

---
## 43. Cross-panel convergence and final decisions

### 43.1 Consensus

All four panels converge on these decisions:

1. **The local Chronicle is the first product.** Identity serves it; identity is not a separate end-user product.
2. **Fasti IDs are stable and provider-neutral.** External IDs are evidence-bearing coordinates.
3. **Unresolved does not mean unusable.** Users can keep recording, export, and return later.
4. **Original observation and later interpretation remain separate.** This is the only safe basis for healing history.
5. **Provider preference is purpose-specific.** Display, search, schedule, topology, playback, and tracker writeback can select different routes.
6. **Cross-provider mappings are typed and directional.** One-to-one alias tables are insufficient [S2–S5].
7. **Reversible enrichment may be automated within policy.** Irreversible merge or history movement requires explicit review.
8. **Complexity is disclosed progressively.** The user sees the safe state and next action first; evidence remains available.
9. **Every external system is optional to local recording.** Provider failure cannot delete or block the Chronicle.
10. **Scrobble.dev stays neutral.** Fasti implements and tests standards but does not own their right to exist.

### 43.2 Conflicts resolved

#### Conflict: “No primary ID” versus provider APIs that require one ID

**Resolution:** Fasti has no canonical external ID. A **resolution plan** selects one route for one operation. The chosen route is logged with policy, evidence, and expiry. It is not written back as the permanent identity of the entity.

#### Conflict: automatic healing versus user control

**Resolution:** Use an irreversibility gate.

- Adding a new external assertion is normally reversible and may be automated when policy and evidence permit.
- Activating a metadata claim is reversible and may be automated.
- Merging entities, changing segment topology, moving Chronicle interpretations, or deleting assertions is not automatically accepted.

#### Conflict: one universal media model versus domain-specific semantics

**Resolution:** Use shared identity roles and shared relation primitives, then declare domain profiles.

```text
shared:
  entity
  namespace
  external identity
  assertion
  evidence
  relation
  source snapshot
  lifecycle

domain profile:
  supported grains
  progress unit
  topology rules
  provider fields
  completion policy
  actions
```

#### Conflict: live provider data versus local-first operation

**Resolution:** Providers supply candidates and metadata projections. Fasti persists the user record, last-known-good projections, evidence needed for decisions, and bounded source snapshots. It does not mirror whole provider databases.

#### Conflict: custom identity fields versus namespace integrity

**Resolution:** A custom field can be marked `identity` only through a registered namespace definition. Ordinary fields cannot participate in automatic entity resolution.

#### Conflict: transparency versus cognitive load

**Resolution:** Use three levels:

1. **Outcome:** what happened and whether data is safe.
2. **Reason:** the small set of factors that drove the decision.
3. **Evidence:** full source, assertion, policy, and trace details on demand.

### 43.3 Rejected alternatives

| Alternative | Rejection reason |
|---|---|
| External provider ID as database key | Makes provider switching and partial imports destructive. |
| One `external_ids` JSON object as complete identity model | Cannot preserve assertion provenance, topology, conflict, lifecycle, or source route. |
| Universal numeric confidence | Hides evidence type, policy, source dependence, and irreversible risk. |
| Title/year automatic merge | Fails on remakes, editions, alternate cuts, cours, translations, and reused titles. |
| Transitive alias closure | Can turn one bad edge into large silent corruption. |
| Replace all source IDs when preference changes | Breaks sync routes and historical references. |
| Use AniBridge as the Fasti schema | Useful implementation source, but Fasti needs wider domains, local decisions, and its own lifecycle. |
| Use Electric-Town crosswalk as the universal media database | It is an anime crosswalk design/data project, not Fasti’s general metadata or user-state store. |
| Store all provider fields in canonical columns | Causes schema growth and source ambiguity. |
| Store every field only in arbitrary JSON | Weak validation, filtering, indexing, and upgrade behavior. |
| Graph database first | Adds operational burden without a proven relational limit. |
| AI decides identity | Non-deterministic, hard to audit, and unsafe for destructive operations. |

### 43.4 Unresolved decisions and exact spikes

| Decision | Spike | Pass condition |
|---|---|---|
| Internal common-grain vocabulary | Model 50 fixtures across video, anime, books, music, podcasts, games, and custom types | No domain loses required distinction; names remain understandable. |
| SQLite graph query performance | Load 1M external assertions and 5M mapping edges; run bounded resolution workloads | p95 within budget with bounded memory and no unbounded recursive query. |
| Automatic assertion admission | Replay real imports with policy variants | False automatic attachment below agreed threshold; all accepted decisions explainable. |
| Metadata field merge policies | Compare TMDB/TVDB/MAL/Kitsu/Google Books/MusicBrainz fixtures | Each field has deterministic locale/source/user-override behavior. |
| AniList metadata integration | Review current official terms and test rate/availability | Compliant public-metadata use, bounded adapter, no runtime dependency. |
| AniBridge adapter | Validate current release artifact against Fasti and Electric-Town cases | Directionality, ranges, ratios, provenance, versioning, and stale-target checks survive. |
| Custom field indexing | Benchmark generated/indexed SQLite columns versus side index table | Safe migrations, bounded storage, and useful query performance. |
| Reconciliation UX | Test with migration, provider switch, and topology conflict tasks | Users identify safe action, preserve context, and complete without external help. |
| OpenRefine-compatible reconciliation surface | Prototype optional protocol adapter | Useful batch workflow without forcing its entity assumptions into Fasti. |

---

## 44. Assumption audit

### 44.1 Strategy under test

Fasti can become the self-hosted authority for a person’s media record by making identity transparent, repairable, and provider-neutral while keeping ordinary use simple.

### 44.2 Assumption register

| Assumption | Category | Importance | Evidence strength | Risk if false | Test and decision trigger |
|---|---|---:|---|---|---|
| Users care more about preserved history than perfect immediate metadata | Customer | Critical | Medium | Product over-invests in preservation users do not notice | Migration interviews and UAT. If users prefer destructive convenience, keep preservation but simplify review defaults. |
| A stable local entity plus assertions can cover initial domains | Product/model | Critical | Medium-high | Core schema needs costly rewrite | Cross-domain fixture corpus. Fail if any initial domain requires provider identity as canonical. |
| SQLite can handle the identity graph and Chronicle on target hardware | Technical | Critical | Medium | Storage rearchitecture | Reference benchmark. Fail if bounded queries or migrations miss published budgets. |
| One accepted ordering authority per workspace is acceptable | Distributed systems | High | Medium | Offline multi-writer cannot converge as designed | Two- and five-device chaos test. Revisit if users require serverless peer merge. |
| Reversible identity enrichment can be automated safely | Product/security | High | Medium-low | False attachment corrupts later decisions | Replay audited imports. Disable auto-admission if false-positive threshold is exceeded. |
| Users can understand identity review with progressive disclosure | UX | High | Low | Reconciliation becomes support burden | Moderated UAT. Redesign if completion or comprehension targets fail. |
| Provider aliases usually supply a path to useful enrichment | Data | High | Medium | Many records remain permanently unresolved | Sample real SIMKL/Floppy/Yamtrack imports. Expand source adapters or improve manual flows if coverage is poor. |
| Provider terms permit intended adapters | Legal/operational | Critical | Low-medium by provider | Features cannot ship | Per-provider written review. No adapter ships without accepted posture. |
| AniBridge can be consumed as an optional mapping source | Data/technical | Medium | Medium | Anime coverage delayed | Artifact/schema/version spike; no effect on core if it fails. |
| Electric-Town conformance cases generalize into Fasti anime requirements | Model | High | High for structure | Missed topology case | Add independent community and real-import fixtures; update corpus when a new shape breaks. |
| Custom fields can remain governed without harming extensibility | Product/model | High | Medium | Either rigidity or schema chaos | Build game/book/custom prototypes. Revisit field system if users require arbitrary computation. |
| Field-level metadata claims are operationally affordable | Technical | High | Medium | Storage and query complexity | Benchmark claims, projections, and cache policies on large libraries. |
| A provider SDK can make new metadata sources cheap to add | DX | High | Low until implemented | Maintainer remains bottleneck | Time-to-first-provider usability test. Target under 30 minutes for fixture-backed read adapter. |
| Scrobble.dev can host neutral contracts without appearing Fasti-controlled | Ecosystem | High | Medium | Adoption and trust suffer | Governance review and external contributor feedback before claiming a standard. |
| Users accept unresolved state instead of forced guesses | Customer | High | Medium | Queue grows and trust falls | UAT copy and behavior tests. Provide batch-safe actions if deferral rate is high. |
| Local snapshots give enough offline metadata without mirroring providers | Technical/product | Medium | Medium | Poor offline experience | Offline reference-library UAT after configured retention. |

### 44.3 Load-bearing assumptions

1. **Stable local identity can remain independent of provider identity.** If this fails, the central strategy fails.
2. **Original observations can remain useful before exact resolution.** If this fails, local-first recording is blocked by metadata.
3. **The resolver can make intent-specific decisions deterministically.** If this fails, API and integration behavior becomes unpredictable.
4. **Users can review ambiguous cases without specialist knowledge.** If this fails, Fasti becomes an expert-only tool.
5. **SQLite meets the graph and Chronicle workload.** If this fails, deployment simplicity changes.
6. **Provider and mapping licenses permit the planned data paths.** If this fails, adapters or public artifacts must change.

### 44.4 Recommendation

Proceed with a gated foundation. Do not proceed directly to broad provider implementation. Prove the common identity model, observation separation, SQLite workload, import classification, and reconciliation experience first.

---

## 45. Strategic risk register

| Risk | Likelihood | Impact | Prevention and mitigation | Trigger | Contingency | Owner/phase |
|---|---|---|---|---|---|---|
| False entity merge | Medium | Critical | No title merge; irreversibility gate; negative assertions; preview; rollback | Any verified false automatic merge | Disable related policy, quarantine affected decisions, restore split from decision log | Identity, M0-M2 |
| Mapping update silently changes history | Medium | Critical | Pin interpretation decision/bundle; impact preview; no auto-move | Occurrence target changes after bundle install | Roll back bundle and interpretation decisions; publish incident report | Identity/Chronicle, M1-M3 |
| Provider disappears or deletes IDs | High | High | Stable Fasti IDs; source snapshots; tombstones; last-known-good | 404/410 spike or project shutdown | Mark source stale, retain record, route to alternates, preserve export | Metadata, all phases |
| AniList or another provider changes metadata terms or limits | Medium | High | Current-terms review; adapter isolation; no canonical dependency | Terms, rate, or availability change | Disable affected capability, retain source IDs, use licensed alternatives or manual import | Provider governance |
| Data-license contamination enters public bundle | Medium | Critical | Acquisition-route registry; separate app data from publishable assertions; reviews | Unknown/forbidden source in bundle build | Block release, revoke artifact, rotate manifest, notify consumers | Data governance |
| Malicious or compromised mapping bundle | Low-medium | Critical | Signatures, hashes, schema validation, limits, staged diff, source trust | Signature failure, abnormal impact, revocation | Reject/rollback bundle; quarantine source; security advisory | Supply chain, M3+ |
| Reconciliation queue overwhelms users | Medium | High | Safe unresolved state; grouping; batch rules; priority by blocked outcome | Queue age/size exceeds target; high abandonment | Reduce auto-generated cases, improve source coverage, offer scoped batch decisions | Product/UX |
| Identity model becomes universal-model bureaucracy | Medium | High | Common roles plus domain profiles; fixture-driven schema | New domain needs many exceptions or unclear terminology | Split bounded profile, retain shared assertion layer only | Architecture |
| Custom fields damage query and migration performance | Medium | Medium-high | Versioned definitions, bounded index budget, no unrestricted EAV | Storage/index budget exceeded | Disable index, rebuild projections, require plugin for computation | Custom records |
| SQLite writer contention | Medium | High | One writer actor, short transactions, batching, benchmarks | Queue latency or busy errors exceed SLO | Reduce job concurrency, shard derived caches, evaluate alternate storage adapter | Storage |
| Sync conflict misattributes profile or device | Low-medium | Critical | Explicit workspace/profile/actor/client in envelope; auth-derived origin | Cross-profile mutation or audit mismatch | Revoke device, quarantine operations, restore from change log | Sync/auth |
| Original source payload leaks private data | Medium | High | Minimize snapshots, redact secrets, per-profile authorization, retention | Sensitive field appears in diagnostics/export | Purge derived copy, rotate secrets, security response | Privacy |
| Provider adapter requires whole-app changes | Medium | High | SDK/port contract; generated registration; no provider terms in core | New adapter touches more than approved surfaces | Stop integration, improve port before adding provider | DX/architecture |
| Metadata fan-out exhausts low-end instance | High without controls | High | Visible-item hydration, budgets, backoff, queue limits, cache policy | Memory/CPU/network budget breach | Pause provider, show stale data, reschedule bounded jobs | Performance |
| User cannot understand evidence | Medium | High | Outcome-first UI, factors, examples, glossary, UAT | Low task completion or incorrect approvals | Simplify decisions, add guided compare, reduce automation | UX |
| AI agent performs unsafe merge | Low if scoped | Critical | No direct SQL; candidate-only AI; confirmation and scopes | Agent requests destructive identity action | Deny, audit, revoke client, require human workflow | MCP/AI |
| Maintainer scope collapse | High | High | Milestone gates, kill list, bounded contexts, no marketplace early | Roadmap growth or unfinished core milestones | Freeze new integrations; complete local Chronicle and migration path | Project governance |
| Scrobble.dev loses neutrality | Medium | High | Separate governance, neutral language, open fixtures, no Fasti dependency | External projects view spec as product funnel | Independent review, namespace Fasti extensions, revise governance | Ecosystem |
| Backup restores data but not identity decision state | Medium | Critical | Backup includes assertions, evidence, policies, overlays, redirects, interpretations | Restore equivalence mismatch | Block release, retain previous format, migration repair utility | Backup/recovery |

### 45.1 Residual risk

Identity is inherently uncertain because external catalogues describe different grains and can change. Fasti cannot guarantee that every imported record resolves automatically. It can guarantee that uncertainty is visible, original evidence is retained, destructive guesses are avoided, and later knowledge can improve the record without erasing history.

---

## 46. Strategy war game

### 46.1 Scenario: preferred anime mapping source disappears

**Trigger:** AniBridge or another bundle publisher stops releasing updates.

**Impact:** New records have lower automatic coverage. Existing mappings remain usable but age.

**Early signals:** release age, failed checks, growing unresolved count, stale-target ratio.

**Pre-committed response:** keep the last verified bundle, mark age visibly, continue local recording, enable alternative adapters, preserve user overlays, and publish an ecosystem RFC through Scrobble.dev. Never rebuild identity from titles.

### 46.2 Scenario: a bundle release contains a widespread bad edge

**Trigger:** impact monitor finds abnormal merge/move candidates or users report wrong episode topology.

**Impact:** Metadata routes may be wrong; accepted history could be at risk if safeguards fail.

**Pre-committed response:** stop activation, restore prior bundle, revoke assertion IDs, identify decisions made from the bad assertions, generate a user-visible impact report, and do not move existing occurrence interpretations automatically.

### 46.3 Scenario: user changes TV from TMDB to TVDB

**Trigger:** settings preference change.

**Impact:** display and episode ordering may differ; remote tracker routes may no longer round-trip.

**Pre-committed response:** run a dry-run per entity, apply safe metadata claims, retain both external identities, pin unsafe topology, send ambiguous cases to reconciliation, and preserve current Chronicle interpretation.

### 46.4 Scenario: import contains IMDb and Kitsu, while configured anime search expects MAL

**Trigger:** SIMKL import.

**Impact:** Legacy systems produce a dead row.

**Pre-committed response:** create or reuse a Fasti entity from the supplied identities, record the source route, use IMDb for compatible metadata enrichment, use Kitsu for Kitsu-native operations, and seek MAL/AniList mappings asynchronously. The record is usable before healing completes.

### 46.5 Scenario: two devices write different progress while offline

**Trigger:** independent playback sessions or stale device state.

**Impact:** A simple latest timestamp can select the wrong resume position.

**Pre-committed response:** preserve both observations and session IDs, apply resource-specific progress policy, never manufacture a history occurrence from a duplicate delivery, and expose a rare conflict only when automatic policy cannot preserve intent.

### 46.6 Scenario: provider purges thousands of IDs

**Trigger:** sustained 404/410 pattern verified against provider health.

**Impact:** metadata and outward sync routes fail.

**Pre-committed response:** open circuit breaker, stop retry storm, mark assertions stale in batch, keep Fasti entities and last-known-good projections, seek redirects/alternate routes, and create one operational notification rather than thousands.

### 46.7 Scenario: custom namespace collides with a future standard namespace

**Trigger:** installed add-on registers a name later adopted publicly.

**Impact:** IDs become ambiguous.

**Pre-committed response:** require reverse-domain or owner-scoped custom namespace IDs, retain immutable registry IDs separate from display aliases, and migrate only the alias through a versioned registry update.

### 46.8 Scenario: malicious provider returns internal URLs and oversized nested JSON

**Trigger:** compromised add-on or API Connection.

**Impact:** SSRF, memory exhaustion, secret exposure.

**Pre-committed response:** block through Governed Network Boundary, enforce resolved-address and redirect checks, size/depth/time limits, no credential forwarding, source disablement, safe audit event, and no partial domain mutation.

### 46.9 Scenario: mobile device is suspended for several days

**Trigger:** operating-system background limits.

**Impact:** queued operations arrive late and may duplicate remote activity.

**Pre-committed response:** local outbox persists, operation IDs remain stable, server receipts deduplicate, snapshot/delta catches up, and the UI states pending local work without requiring the app to stay open.

### 46.10 Scenario: maintainer capacity falls sharply

**Trigger:** low contributor availability or maintainer absence.

**Impact:** providers and schemas drift; security response slows.

**Pre-committed response:** freeze new adapters, maintain stable core/export/security, use CODEOWNERS and succession rules, keep conformance and release automation executable, and avoid a central hosted service that only one maintainer can run.

---
## 47. Success metrics and user capability outcomes

Fasti must measure whether users become more capable at owning their media record. It must not optimize for time spent in settings, notification volume, or artificial engagement.

### 47.1 North-star capability

> **A user can keep, understand, repair, move, and recover their media record without surrendering control to one provider.**

### 47.2 Product metrics

| Outcome | Metric | Initial target |
|---|---|---:|
| First local value | Median time from installation to first locally accepted occurrence | under 10 minutes |
| Migration safety | Imported source rows classified and reported | 100% |
| Preservation | Ambiguous rows silently discarded | 0 |
| Stable identity | Provider-preference changes that alter Fasti entity IDs | 0 |
| Repairability | Identity review tasks completed correctly without support | at least 85% in moderated UAT |
| Context recovery | Resumed long-running tasks return to the correct checkpoint | 100% tested flows |
| Offline ownership | Local record accepted during provider/network outage | 100% supported clients |
| Sync correctness | Duplicate delivery creates a false rewatch | 0 in conformance suite |
| Restore trust | Backup/restore equivalence across canonical state and identity decisions | 100% release fixtures |
| Provider resilience | Existing records deleted because a provider returned empty/404 | 0 |
| Developer first result | Median time from SDK start to validated fixture-backed provider | under 30 minutes |
| Error usefulness | Top errors with problem, cause, safe state, repair, and doc link | 100% |
| Documentation fidelity | Contract and example drift caught in CI | 100% generated/checked surfaces |
| Accessibility | Release-blocking user journeys keyboard-operable and screen-reader tested | 100% |
| Support learning | Repeated support incidents linked to updated KCS unit | 100% of qualifying incidents |

### 47.3 Meaningful payoff loops

Each major journey must close with a result the user cares about.

| Journey | Practice step | Feedback | Payoff |
|---|---|---|---|
| Import | Review one uncertain record | Fasti explains preserved evidence and effect | Years of history are safe and portable |
| Pair player | Confirm one node and profile | First observation appears locally | Preferred player now contributes to one record |
| Add provider | Run fixture and preview fields | Claim/provenance comparison | Better metadata without identity breakage |
| Repair identity | Compare evidence and accept a bounded assertion | Chronicle remains linked and result is explained | The record becomes more accurate |
| Change provider | Review impact preview | Safe fields change; risky topology stays pinned | User controls presentation without damaging history |
| Restore backup | Run restore validation | Equivalence report | User knows the record can survive failure |

### 47.4 Anti-metrics

Do not optimize for:

- daily active use of settings;
- number of reconciliation decisions;
- notification count;
- provider calls;
- percentage of records forced into one external namespace;
- number of installed add-ons;
- time spent in Fasti instead of the user’s player;
- automatic match rate without false-match and preservation measures.

---

## 48. Definition of Done

No feature is done only because the happy path works.

### 48.1 Domain and data

- The bounded context and vocabulary are stated.
- Domain invariants are tested.
- Provider-specific concepts remain in adapters unless promoted through evidence.
- External IDs do not replace Fasti IDs.
- Original observations remain separate from interpretations.
- Deletion, merge, redirect, revocation, and tombstone behavior are explicit.
- Migration and rollback behavior are documented and tested.

### 48.2 API and contracts

- The capability exists through the application service before UI-specific logic.
- OpenAPI, AsyncAPI, JSON Schema, capability ledger, and generated client surfaces are updated where relevant.
- Idempotency, pagination, batching, scopes, errors, and versioning are defined.
- Examples run in CI.
- A compatibility/deprecation note exists for behavioral changes.

### 48.3 Local-first and distribution

- Core behavior works without internet.
- A provider failure cannot delete canonical user state.
- Packaged and non-Docker paths are tested where the feature applies.
- Long tasks can stop and resume.
- Background concurrency and storage growth are bounded.

### 48.4 Security and privacy

- Threat boundary and data touched are declared.
- Inputs have size, depth, time, and rate limits.
- Network operations use the Governed Network Boundary.
- Secrets are references, not normal YAML/export/log fields.
- Object authorization includes workspace, profile, actor, and client.
- Destructive actions have explicit scopes and user confirmation policy.
- Supply-chain and license effects are reviewed.

### 48.5 Accessibility and AuDHD

- Keyboard, focus, screen-reader name/state, touch alternative, reduced motion, and non-color state are tested.
- The user sees what happened, what is safe, and the next action.
- Long tasks preserve context and checkpoints.
- Advanced detail is progressive, not hidden from access.
- Errors are persistent and recoverable.

### 48.6 Performance and operability

- Queries and provider calls are bounded.
- Reference-library benchmark passes.
- Queue, cache, circuit, and job states are observable.
- Metrics do not expose high-cardinality personal data.
- Failure and recovery are tested, including process termination.

### 48.7 Knowledge and support

- Glossary terms are reused exactly.
- User, operator, and developer knowledge is updated with the feature.
- Each important error has one stable knowledge URL.
- Troubleshooting captures symptoms, environment, cause, resolution, and verification.
- OKF/Markdown outputs are generated or validated.
- Known limitations and unresolved decisions are visible.

### 48.8 User capability

- The feature states the capability the user gains.
- The smallest useful success can be reached without reading all documentation.
- Feedback teaches the correct mental model at the moment it is needed.
- The result has a meaningful payoff outside Fasti.

---

## 49. Governance and operating model

### 49.1 Decision hierarchy

1. User-owned Chronicle safety.
2. Explicit domain invariants.
3. Published contract and conformance behavior.
4. Migration and compatibility.
5. Provider and transport convenience.
6. Implementation preference.

A lower item cannot override a higher item without an RFC and migration plan.

### 49.2 Required records

```text
ADR       architectural decision and alternatives
RFC       proposed behavioral or public-contract change
CAP       capability definition
KCU       knowledge/KCS unit
THREAT    threat model and mitigations
MIG       migration and rollback plan
CONF      conformance case and fixtures
INC       incident and learning record
```

Each record has a stable identifier and links to implementation, tests, docs, and release notes.

### 49.3 Identity governance board function

This does not require a large committee. It requires an explicit review role for changes that can merge, split, reinterpret, or retire identity.

Review checklist:

- What is the entity grain?
- Is the relationship identity, topology, presentation, availability, or user state?
- Is the assertion directional?
- What evidence and derivation root support it?
- Is the action reversible?
- Can it move historical interpretations?
- What does a consumer lose if it cannot express this case?
- What fixture proves the behavior?
- What happens when the source is removed?

### 49.4 Scrobble.dev relationship

Scrobble.dev publishes neutral surfaces:

- activity vocabulary;
- identity and mapping glossary;
- capability definitions;
- provider-neutral examples;
- conformance cases;
- import/export guidance;
- KCS/OKF knowledge;
- interoperability project directory;
- RFC discussion.

Fasti publishes implementation-specific surfaces under its own namespace:

- Fasti deployment;
- Fasti settings;
- Fasti internal IDs;
- Fasti provider SDK;
- Fasti extension permissions;
- Fasti release and migration policy.

Neutral contracts must remain useful to projects that do not run Fasti.

### 49.5 Collaboration with existing projects

- Continue narrow Floppy security, data-loss, migration, export, and conformance work.
- Offer Yamtrack bounded fixes and neutral fixtures without asking it to adopt Fasti architecture.
- Treat Nuvio as a first-class client and sync consumer, not a competitor.
- Use Electric-Town anime-crosswalk-mappings as the identity/topology design and conformance reference.
- Evaluate AniBridge as an optional implementation source and contribute corrections upstream where possible.
- Credit AnimeAPI, Anime-Lists, shinkro/community-mapping, SIMKL, CrossWatch, and other evidence sources accurately.
- Avoid “Fasti fixes what they got wrong” messaging.

### 49.6 Repository dependency policy

- Domain crates cannot depend on adapters, transports, storage, UI, or provider SDKs.
- Public contracts cannot import database row types.
- Adapters implement application ports.
- UI, CLI, MCP, Tauri, and transports call application services.
- Generated artifacts state their canonical source and regeneration command.
- No generic `utils` or `common` dumping ground.
- Duplicate business rules fail review even when written in different languages.

---

## 50. First ninety days

The first ninety days prove the record, not the platform.

### Days 1–15 — constitution and corpus

**Deliver**

- repository and workspace;
- glossary and bounded-context map;
- stable identifier format;
- identity relation vocabulary;
- namespace registry schema;
- evidence/provenance schema;
- lifecycle schema;
- initial cross-domain conformance corpus;
- ADRs for SQLite, observation separation, no primary external ID, and provider claims;
- threat model;
- license/data-source register;
- initial OpenAPI/AsyncAPI/JSON Schema pipeline.

**Gate**

- fifty representative records across initial domains can be expressed without title matching or provider-owned canonical identity.

### Days 16–30 — SQLite identity and Chronicle kernel

**Deliver**

- Rust domain crates;
- SQLite migrations;
- controlled writer actor;
- catalog entities and external identities;
- assertions, evidence, source snapshots, tombstones, redirects, known absences, and negative assertions;
- observed references and Chronicle occurrences;
- local command API;
- export of one unresolved and one resolved record;
- property tests for cycles, duplicate assertions, and atomic receipt/change writes.

**Gate**

- process termination at every transaction boundary cannot create a mutation without its receipt/change record or vice versa.

### Days 31–45 — import and partial identity

**Deliver**

- canonical import record format;
- Floppy and SIMKL fixture adapters;
- exact/reused/candidate/unresolved classification;
- source payload preservation policy;
- resumable import job;
- import preview and report;
- SIMKL IMDb + Kitsu, no MAL, end-to-end case;
- manual record with no URL/provider case.

**Gate**

- every input row is reported; no ambiguous row is silently dropped; local record remains usable without completed enrichment.

### Days 46–60 — metadata claims and purpose resolver

**Deliver**

- provider manifest and adapter port;
- field claim model;
- one video provider and one non-video provider fixture;
- intent-specific resolution plan;
- metadata projection cache;
- user override;
- last-known-good and stale states;
- impact trace API.

**Gate**

- display-provider change does not change entity ID, Chronicle link, or exact tracker-write route.

### Days 61–75 — reconciliation and safe change

**Deliver**

- candidate generation;
- decision policy engine;
- reconciliation case model;
- exact/identifier-only/topology comparison UI;
- negative assertion action;
- provider-switch preview;
- mapping-bundle staging and impact report;
- rollback;
- persistent job/inbox states.

**Gate**

- moderated users complete three identity tasks without losing context or making an unintended merge.

### Days 76–90 — first user capability alpha

**Deliver**

- multi-user workspace/profile minimum;
- Tabler/Svelte Chronicle shell;
- TrailBase human-account boundary and dormant Fasti browser-session foundation; the historical local-auth and allauth bridge proposal is superseded;
- local backup/restore equivalence;
- mDNS discovery and secure-pairing spike;
- one player observation path;
- public provider SDK example;
- KCS/OKF docs bundle;
- signed alpha artifacts for one desktop and one self-hosted native platform.

**Gate**

A real user can:

1. install Fasti;
2. import an existing sample or personal record;
3. record one local occurrence through an external player;
4. see unresolved identity without losing the event;
5. enrich or correct one record;
6. export and restore it;
7. understand what happened without reading architecture documents.

### 50.1 Work explicitly outside the first ninety days

- broad provider catalogue;
- public add-on marketplace;
- executable WASI plugins;
- Cloudflare deployment;
- MQTT commands beyond a narrow proof;
- full mobile background-sync claim;
- all media-domain UIs;
- public automatic overlay submission;
- semantic/vector search;
- alternate canonical database.

---

## 51. Final stack and architecture status

### 51.1 Locked

| Area | Decision |
|---|---|
| Product | Fasti is a self-hosted, local-first Chronicle and interoperability service; not a player. |
| Stable identity | Opaque Fasti IDs independent of providers. |
| Identity model | Typed assertions, evidence, source route, topology, decisions, and lifecycle. |
| History model | Original observation remains separate from interpretation. |
| Database | SQLite through `rusqlite`, one controlled writer, bounded reads. |
| Core | Rust domain/application crates. |
| Server | Axum on Tokio. |
| Frontend | Svelte 5, TypeScript, Vite, Tabler Core and Icons. |
| Packaged apps | Tauri 2. |
| API | Application-service first; OpenAPI 3.1 with Utoipa where suitable. |
| Events | AsyncAPI 3.x from shared event definitions. |
| Schemas | JSON Schema 2020-12. |
| Knowledge | Markdown, OKF, JSON-LD outputs generated from governed definitions. |
| Auth boundary | TrailBase is the selected separate human-account service. Fasti owns subject links, browser sessions, authorization, profiles, grants, scopes, and media identity. The historical django-allauth proposal is superseded. |
| Notifications | First-class Rust domain; Apprise is a private optional delivery adapter. |
| Local discovery | DNS-SD over mDNS, explicit pairing, manual/QR fallback. |
| Sync | Local outbox, idempotency receipts, ordered accepted changes, snapshots/deltas, tombstones. |
| Live transport | WebTransport preferred after proof; HTTP/WebSocket/SSE fallbacks. |
| MQTT | Optional generic Connection adapter; not sync correctness. |
| Provider model | Search/candidate/claim adapters, no direct canonical writes. |
| Anime doctrine | Electric-Town crosswalk schema/policy/conformance lessons. |
| Anime implementation | Optional AniBridge and other bundle adapters after conformance and license review. |
| Custom data | Versioned record-type and field definitions with validated extension documents. |
| AI | MCP over application services; AI may suggest, never silently establish identity truth. |

### 51.2 Provisional

- exact common grain names across all media domains;
- automatic assertion-admission policy;
- AniBridge artifact adapter;
- AniList public-metadata integration after current-terms, limit, and conformance evidence;
- OpenRefine reconciliation compatibility;
- local TLS/pairing UX;
- mobile background reliability;
- WebTransport Rust implementation and deployment profile;
- custom-field indexing implementation;
- Cloudflare edge profile;
- signed WASI plugin host.

### 51.3 Release claims that remain blocked

Fasti must not claim:

- lossless migration before real-source replay passes;
- exact cross-provider anime mapping before conformance and target-validation gates pass;
- full offline multi-device sync before mobile/process-kill chaos tests pass;
- AniList metadata support before current-terms, limit, and adapter tests pass;
- secure plugin ecosystem before signatures, revocation, permissions, limits, and reporting exist;
- low-end performance before published benchmark evidence exists;
- neutral standard ownership before Scrobble.dev governance receives external review.

---

## 52. Controlling principles

1. **Fasti keeps the record stable while its understanding improves.**
2. **Fasti records. Players play.**
3. **An external identifier is a coordinate, not the user’s record.**
4. **A mapping is an assertion with scope and evidence, not a field.**
5. **The original observation is never rewritten by later interpretation.**
6. **A read alias is not automatically a safe write route.**
7. **Provider preference changes views, not history.**
8. **Absence, outage, and cache miss are not deletion.**
9. **A retry is not a rewatch.**
10. **Unresolved is a safe state, not a failed record.**
11. **Gate irreversible actions more strictly than reversible enrichment.**
12. **The user sees the outcome first and evidence when needed.**
13. **The product succeeds when the user can own, repair, move, and recover the record.**
14. **Scrobble.dev belongs to the ecosystem; Fasti is one implementation.**

---

## 53. Source and evidence ledger

### Primary design and project evidence

- Electric-Town/anime-crosswalk-mappings: README, glossary, schema, conformance corpus, namespace/authority registry, policy, governance, and roadmap.
- Electric-Town crosswalk and catalogue-interoperability research from the supplied project files.
- nattadasu/animeApi issue 11 and its mapping/topology discussion.
- Floppy issues 387, 645, 649, 650, 652, 860 and related identity/provider work.
- Floppy current media integration playbook and item model.
- Floppy × Nuvio programme handoff, security review, and conformance work.
- NuvioTV issue 2742 and current per-season anime identity work.
- Current AniList official API documentation, policy, pagination, relation, and rate-limit material.
- Current AniBridge mapping release and provenance material.
- MusicBrainz, Open Library, Podcasting 2.0, and IGDB domain identity references.
- OpenRefine Reconciliation API specifications as a candidate-service interaction reference.
- Supplied gstack plan engineering, developer-experience, and DX Hall of Fame methods.
- Strategy 21 Narrative Builder, Assumption Audit, Initiative Prioritizer, Risk and Mitigation, and War Gaming methods.
- Kathy Sierra, *Badass: Making Users Awesome*, applied through larger-context success, just-in-time knowledge, deliberate practice, cognitive-leak reduction, perceptual exposure, and meaningful payoff.

### Evidence posture

The current source material supports the architecture and test plan. It does
not prove production performance, continuing provider-terms compliance,
automatic-match accuracy, user comprehension, or mobile reliability. Those
claims remain gated by the named spikes and UAT.


## 54. Evidence references

The following references support the externally verifiable design claims in this plan. Source tags in the report point here.

| Tag | Source | Use in this plan |
|---|---|---|
| **S1** | https://github.com/Electric-Town/anime-crosswalk-mappings | Primary identity/topology design, governance, licensing and conformance source. |
| **S2** | https://github.com/Electric-Town/anime-crosswalk-mappings/blob/main/README.md | Crosswalk-not-database boundary; typed range-scoped edges; permanence; irreversibility; provenance. |
| **S3** | https://github.com/Electric-Town/anime-crosswalk-mappings/blob/main/GLOSSARY.md | Entity, mapping, evidence, derivation-root, negative-assertion and lifecycle vocabulary. |
| **S4** | https://github.com/Electric-Town/anime-crosswalk-mappings/blob/main/conformance/README.md | Thirteen structural mapping capabilities and conformance-first method. |
| **S5** | https://github.com/nattadasu/animeApi/issues/11#issuecomment-5289473691 | 1:1 failure, provider topology disagreement, stale/deleted IDs and anime import risks. |
| **S6** | https://github.com/dannyvfilms/Floppy/issues/652 | Provider-neutral identity research, mapping versioning, exact versus lossy routes and no silent history movement. |
| **S7** | https://github.com/dannyvfilms/Floppy/issues/645 | Semantic resolution and collaboration/governance lessons. |
| **S8** | https://github.com/dannyvfilms/Floppy/issues/649 | MAL/AniList/Open Library provider correctness and AniList schedule-pagination lessons. |
| **S9** | https://github.com/dannyvfilms/Floppy/issues/650 | Read-only data audit, recoverable ambiguity and non-destructive migration principles. |
| **S10** | https://github.com/dannyvfilms/Floppy/issues/387 | Provider switching and TMDB/TVDB identity breakage evidence. |
| **S11** | https://github.com/dannyvfilms/Floppy/issues/860 | Google Books provider request and localized/edition coverage evidence. |
| **S12** | https://github.com/NuvioMedia/NuvioTV/issues/2742 | MAL/Kitsu primary IDs with a usable IMDb alias ignored during enrichment. |
| **S13** | https://docs.anilist.co/guide/terms-of-use | Current AniList tracker-use, storage and collection restrictions. |
| **S14** | https://docs.anilist.co/guide/rate-limiting | Current AniList rate-limit posture and degraded-service limit. |
| **S15** | https://docs.anilist.co/guide/considerations | AniList availability and temporary API-disable posture. |
| **S16** | https://docs.anilist.co/guide/graphql/queries/media | AniList ID/type and title-search limitations; relation-query examples. |
| **S17** | https://github.com/anibridge/anibridge-mappings | Optional current mapping implementation source, directional ranges, ratios, daily artifacts and provenance. |
| **S18** | https://musicbrainz.org/doc/Release_Group and https://musicbrainz.org/doc/Release | Music release-group/release identity distinction. |
| **S19** | https://musicbrainz.org/doc/Recording | Music recording versus release-track distinction. |
| **S20** | https://openlibrary.org/dev/docs/api/search | Open Library Work and Edition data distinction. |
| **S21** | https://podcasting2.org/podcast-namespace/tags/guid and https://podcasting2.org/podcast-namespace/tags/podcast-guid | Stable podcast and episode GUID guidance. |
| **S22** | https://api-docs.igdb.com/#external-game | Game/store external identifier model. |
| **S23** | https://www.w3.org/TR/reconciliation-api/ | Candidate reconciliation interaction and batch-service reference. |
| **S24** | https://www.oreilly.com/library/view/badass-making-users/9781491919057/chapter-12.html | User success in the larger context. |
| **S25** | https://www.oreilly.com/library/view/badass-making-users/9781491919057/chapter-31.html | Small deliberate skill-building steps. |
| **S26** | https://www.oreilly.com/library/view/badass-making-users/9781491919057/chapter-51.html | Progress and meaningful payoff loops. |
| **S27** | https://www.oreilly.com/library/view/badass-making-users/9781491919057/chapter-57.html | Reduce cognitive leaks and put knowledge in the environment. |
| **S28** | https://www.oreilly.com/library/view/badass-making-users/9781491919057/chapter-65.html | Just-in-time rather than just-in-case knowledge. |
| **S29** | https://github.com/ryan-winkler/strategy-skills-for-claude/blob/main/skills/01-diagnosis-and-framing/assumption-audit.md | Assumption-audit method. |
| **S30** | https://github.com/ryan-winkler/strategy-skills-for-claude/blob/main/skills/04-operating-model-and-execution/initiative-prioritizer.md | Initiative-prioritization and kill-list method. |
| **S31** | https://github.com/ryan-winkler/strategy-skills-for-claude/blob/main/skills/05-risk-performance-and-value-governance/risk-and-mitigation.md | Risk-register method. |
| **S32** | https://github.com/ryan-winkler/strategy-skills-for-claude/blob/main/skills/05-risk-performance-and-value-governance/war-gaming.md | War-game method. |
| **S33** | https://github.com/ryan-winkler/strategy-skills-for-claude/blob/main/skills/06-alignment-and-executive-communication/narrative-builder.md | Answer-first narrative and hostile-question method. |
