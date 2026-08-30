<!-- /autoplan approved 2026-08-30; restore=/home/ryan/.gstack/projects/Scrobble-dev-Fasti/codex-nuvio-metadata-programme-m0-autoplan-restore-20260829-232303.md -->
# Fasti Metadata, Discovery, Library, Collections, and Nuvio Compatibility Plan

**Date:** 2026-08-30

**Repository:** `Scrobble-dev/Fasti`

**Target branch:** `dev`

**Planning base:** `origin/dev@adbdef3038786b0efb2ec615bce080e3eaa9361f`

**Status:** M0 planning approved; implementation may begin from the accepted exact base

**CEO review mode:** Hold scope
**Product boundary:** **Fasti records. Players play.**

## 1. Decision

Build one source-neutral media surface inside Fasti.

The surface has five separate user capabilities:

1. **Search** finds candidate media.
2. **Discover** presents governed catalog rails.
3. **Library** presents local user-owned records and state.
4. **Collections** organize records and bind external catalog sources.
5. **Metadata enrichment** adds field-level provider claims without changing Fasti identity.

Nuvio is the first named compatibility profile.

Nuvio does not own the Fasti domain.

TMDB is an enrichment and catalog provider.

MDBList is a ratings and optional catalog provider.

Trakt is an optional adapter. Trakt is not a required collection spine.

The authentication programme remains separate. This programme consumes its approved session, client, scope, secret-store, and device-pairing capabilities. This programme does not reopen the authentication architecture.

## 2. Product outcomes

A user must be able to:

- search local records and configured sources;
- add a result to Fasti without losing its source identifiers;
- browse useful Discover rows;
- keep a local library without requiring a TMDB, IMDb, Trakt, MAL, or Kitsu ID;
- enrich one record from several metadata providers;
- see the source and freshness of each visible metadata field;
- choose how anime is grouped and exported to Nuvio without re-keying Fasti history;
- create and edit Collections;
- install a versioned Collection pack;
- use a Collection without Trakt;
- connect Nuvio;
- expose selected Fasti lists and Discover rows to Nuvio;
- use cached metadata while a provider is unavailable;
- understand what failed, what stayed safe, and what to do next.

## 3. Current observed baseline

### 3.1 Fasti

Current Fasti `dev` has:

- provider credential status and protected credential writes;
- working TMDB and Google Books search on the trusted desktop host;
- an honest Discover result action that creates a Record only when the host can save the Record, identifier, and metadata in one governed operation;
- a Library view that accepts up to 500 records and filters them in the browser;
- disabled metadata preference controls;
- profile-scoped Nuvio Collections JSON import, export, replace, and clear;
- a 4 MiB Nuvio Collections import limit;
- hard-coded Kaptain and AIO sample pack data inside the Svelte settings component;
- one-way complete-occurrence Nuvio webhook ingress;
- no native Nuvio pairing;
- no Fasti catalog publication;
- no complete progress, saved-state, watched-state, or Collection synchronization.

The existing Nuvio Collections capability is external file interchange. It is not tracking sync, library membership, or Fasti identity.

### 3.2 Nuvio Desktop behavior to learn from

At the pinned Nuvio Desktop revision, the Integrations page separates:

- TMDB Enrichment;
- MDBList Ratings;
- Connected Services.

The tracking settings separately define:

- connected tracking accounts;
- Library source;
- Watch Progress source;
- recommendation source;
- Anime ID preference.

Nuvio Collections use a `Collection -> Folder -> Source` shape.

The source model supports:

- add-on catalogs;
- TMDB list;
- TMDB collection;
- TMDB company;
- TMDB network;
- TMDB Discover;
- TMDB person;
- TMDB director;
- Trakt public lists.

The Nuvio importer also preserves unknown source fields when it round-trips a document.

### 3.3 Nuvio metadata pipeline

Nuvio Desktop uses this broad pipeline:

```text
add-on metadata
    -> TMDB enrichment
    -> MDBList ratings
    -> screen projection
```

Useful behavior:

- base metadata and screen-enriched metadata are cached separately;
- the screen cache uses a settings fingerprint;
- TMDB enrichment can update artwork, localized text, release data, credits, companies, networks, episodes, recommendations, collections, and trailers;
- TMDB can provide a fallback when an add-on cannot provide metadata;
- MDBList rating providers can be enabled separately.

Defects that Fasti must not copy:

- TMDB lookup can stop at the primary MAL or Kitsu content ID and ignore a valid IMDb alias;
- MDBList code places the API key in the request URL;
- MDBList cache identity includes the raw API key;
- choosing one canonical external anime ID can make other valid routes harder to use;
- Trakt can become a hidden collection dependency;
- a TMDB-first search path can hide content that has no TMDB match.

### 3.4 Nuvio Cloud synchronization benchmark

The pinned Nuvio Desktop source and the independent Scrob implementation prove that the official Nuvio Cloud API supports authenticated, profile-scoped synchronization now. This is not an unavailable future contract.

The official surface uses Supabase-compatible password exchange and rotating refresh tokens, then PostgREST RPCs for:

- profile discovery;
- library snapshot, monotonic delta cursor, ordered delta, batched upsert, and explicit delete;
- watched snapshot, monotonic delta cursor, ordered delta, batched upsert, and explicit delete;
- progress snapshot, monotonic delta cursor, ordered delta, batched upsert, and explicit delete.

The official endpoint is `https://api.nuvio.tv`. Self-hosted Nuvio endpoints are allowed only through the governed provider-network policy. The password is used once by the trusted host and is never persisted. All operations for one connection serialize around the single-use refresh token. A successful refresh is written as a provisional next-generation vault entry, reconciled with a durable pending-attempt row, activated by compare-and-set, and only then used by later RPCs. A crash or lost response after the remote service spends the old token but before the new token reaches durable local storage cannot be made atomic across systems; that narrow ambiguity fails closed to `reauthorization_required` instead of pretending it was prevented.

Scrob at `1c4d775b70f489ca0531376b2c3de6a8c3de2a2b` is a behavioral benchmark, not a code source. Its useful evidence is password-to-refresh-token exchange, per-profile connections, merge-safe writes, batching, identifier normalization, and immediate rotated-token persistence. Fasti does not copy its GPL implementation.

## 4. Domain boundaries

## 4.1 Identity

Identity owns:

- `RecordId`;
- external identifier assertions;
- relation type;
- entity grain;
- direction;
- coverage;
- evidence;
- provenance;
- lifecycle;
- ambiguity;
- resolution plans.

Identity does not own provider metadata.

Fasti has no primary external identifier.

## 4.2 Metadata

Metadata owns:

- `ProviderDefinition`;
- `ProviderCapabilityDefinition`;
- `CredentialRequirement`;
- `MetadataClaim`;
- `RatingClaim`;
- `AvailabilityClaim`;
- `MetadataProjection`;
- `EnrichmentPolicy`;
- `MetadataCacheEntry`;
- field provenance;
- locale;
- region;
- freshness;
- last-known-good state.

Metadata does not own Chronicle state or Record identity.

## 4.3 Search

Search owns:

- query validation;
- search scope;
- provider fan-out;
- local search;
- candidate normalization;
- candidate grouping;
- result pagination;
- result provenance;
- exact action routes.

Search results are not Records.

A search candidate becomes a Record only through the approved Record capability.

## 4.4 Discover

Discover owns:

- `DiscoverRailDefinition`;
- rail source bindings;
- profile context;
- source refresh;
- ranking;
- rail pagination;
- source state;
- fallback state.

Discover is not Search.

Discover is not the Library.

## 4.5 Library

Library owns the query projection over:

- Records;
- profile saved state;
- progress;
- watched or completed state;
- ratings;
- notes;
- Collection membership;
- selected metadata projection.

Library does not own remote provider catalogs.

Library records do not require an external provider identifier.

## 4.6 Collections

Collections owns:

- `Collection`;
- `CollectionFolder`;
- ordered membership;
- source binding;
- source authority;
- source health;
- import and export;
- versioned packs;
- unlink behavior.

A Collection is not a watchlist.

A Collection is not watched state.

A Collection source reference is not copied membership unless the user selects a materialization operation.

## 4.7 Connections

Connections owns:

- endpoint;
- credential reference;
- requested capabilities;
- granted capabilities;
- profile mapping;
- health;
- last success;
- last failure;
- retry policy;
- remote version;
- remote capabilities.

A Connection is not a credential.

## 4.8 Nuvio compatibility

The Nuvio adapter owns:

- Nuvio document translation;
- Nuvio catalog projection;
- Nuvio metadata projection;
- Nuvio client capability negotiation;
- Nuvio change transport;
- official Nuvio Cloud authentication and rotating-token lifecycle;
- official Nuvio Cloud snapshot, delta, upsert, and deletion translation;
- the separate Fasti provider contract consumed by Nuvio clients;
- pinned compatibility fixtures.

The Nuvio adapter does not own Fasti business rules.

Direct Nuvio Cloud synchronization and the Fasti provider contract are separate adapters over the same application capabilities. The former lets Fasti synchronize with released Nuvio Cloud now. The latter lets Nuvio Desktop and NuvioTV synchronize with a Fasti node without direct database access.

## 5. Provider registry

Create one provider registry.

Do not define provider behavior inside Svelte.

The concrete composition owner is one reusable in-repository crate, `crates/fasti-provider-runtime`. It owns the provider registry, governed HTTP transport, credential-reference port, request budgets, and bounded dispatcher used by both `fastid` and the Tauri host. `fastid` and Tauri each compose this same crate with the SQLite kernel and the data-root vault; neither owns a second provider implementation. Domain and application crates remain synchronous and transport-free, the store remains network-free, and provider runtime code never moves into Axum route handlers or Svelte. The existing Tauri provider modules are migrated into this owner rather than copied.

Each provider declares:

```text
provider_id
display_name
provider_kind
documentation_url
attribution
supported_media_grains
supported_capabilities
network_hosts
credential requirements per capability
rate-limit policy
request limits
cache policy
locale support
region support
identity namespaces
health-test capability
credential-test capability
offline behavior
licence and terms disposition
```

Credential requirements are per capability.

One provider can have:

- public metadata search with no credential;
- application metadata search with an API key;
- account list access with OAuth;
- account list writes with a stronger OAuth scope.

Use these credential requirement values:

```text
none
optional_api_key
api_key
bearer_token
basic_auth
oauth2
user_agent_only
custom_header
operator_secret_mount
```

Use these credential states:

```text
not_required
optional
missing
stored_unverified
valid
invalid
expired
unavailable
revoked
```

Do not show `Not configured` for a capability that needs no secret.

## 6. Credential vault

Use one `CredentialReference`.

The database stores the reference and status.

The secret store stores the secret.

Use:

- operating-system credential storage for desktop and mobile;
- the approved encrypted headless vault for native server and OCI;
- operator secret mounts for operator-managed deployments;
- no plaintext fallback.

The browser can submit a replacement secret to an authorized same-origin Fasti host over HTTPS.

The browser must not retain the secret.

The browser must never read a stored secret back.

Do not put secrets in:

- Fasti URLs;
- browser storage;
- logs;
- diagnostics;
- screenshots;
- fixtures;
- cache keys;
- normal exports;
- plain SQLite fields.

Provider credentials are never placed in a URL, including at the final request boundary. A provider whose only supported credential transport is a query parameter remains unavailable unless a separately approved constitutional security change replaces this invariant. Redaction is defence in depth, not authority to construct a secret-bearing URI.

Credential release is bound to the authorized provider configuration digest and exact scheme, host, and port. The trusted adapter resolves and authorizes every address before reading the vault, uses the pinned proxy-free and redirect-free client, and never forwards the credential after any origin or configuration drift.

## 7. Metadata claim model

Store field-level claims.

Example:

```text
MetadataClaim
- claim_id
- record_id
- field_key
- value
- provider_id
- source_namespace
- source_identifier
- locale
- region
- fetched_at
- expires_at
- source_version
- evidence_digest
- status
```

Use these statuses:

```text
fresh
stale
invalid
revoked
superseded
unavailable
```

Resolve a visible field in this order:

1. user override;
2. fresh claim from the preferred provider and locale;
3. another fresh compatible claim;
4. last-known-good stale claim;
5. empty state.

A failed refresh must not erase a valid prior claim.

A provider switch changes the projection and route policy.

A provider switch does not change the Fasti Record ID.

A provider switch does not move Chronicle history.

## 8. Purpose-specific identifier resolution

Do not ask:

> What is this Record's primary ID?

Ask:

> Which verified route is safe for this operation?

Define resolution intents:

```text
metadata_search
metadata_lookup
metadata_enrichment
rating_lookup
catalog_lookup
display_projection
nuvio_export
nuvio_import_attachment
tracker_read
tracker_write
segment_translation
deduplication_review
```

For TMDB metadata enrichment, use this route order:

1. exact compatible TMDB ID;
2. exact compatible IMDb ID through TMDB Find;
3. exact compatible TVDB or Wikidata ID through TMDB Find when TMDB supports that grain;
4. accepted crosswalk assertion to TMDB;
5. title and year search as a review candidate only.

Do not attach a new identity from title similarity without explicit evidence or approval.

Add a golden fixture:

```text
primary content ID: mal:49894
IMDb alias: tt28254942
expected result: TMDB route uses the IMDb alias without changing Fasti identity
```

## 9. Anime compatibility preference

Keep all known external IDs.

Do not select one external ID as Fasti identity.

Replace the ambiguous setting with:

> **Anime grouping and export preference**

Scope it per profile and per connection.

Use these options:

```text
group_by_tv_work
  Use IMDb, TMDB, or TVDB-style grouping when a compatible route exists.

keep_mal_releases_separate
  Use MAL release IDs for Nuvio and tracker projection when available.

keep_kitsu_releases_separate
  Use Kitsu release IDs for Nuvio and tracker projection when available.

automatic
  Use the connection compatibility profile and the safest accepted mapping.
```

This preference can change:

- Nuvio catalog IDs;
- outbound tracker route;
- season grouping;
- display grouping;
- external deep links.

It cannot change:

- Fasti Record ID;
- original Observation;
- Chronicle Occurrence;
- prior Interpretation;
- accepted external identity evidence.

Before applying a change, show:

- affected Records;
- safe display changes;
- unresolved routes;
- possible season regrouping;
- rollback.

## 10. TMDB enrichment

Implement TMDB as an enrichment adapter.

Use the TMDB API Read Access Token in the `Authorization` header.

Support separate field groups:

```text
artwork
basic_info
details
release_dates
credits
production_companies
networks
episodes
season_artwork
recommendations
collections
trailers
watch_providers
```

Each field group is independently enabled.

Use:

- profile language;
- profile region;
- user override;
- provider preference;
- field freshness;
- last-known-good fallback.

Cache keys include:

```text
provider_id
credential_reference_version
record_id
resolved provider route
media grain
locale
region
field group
settings fingerprint
schema version
```

Do not include a raw secret in a cache key.

Add explicit tests for:

- MAL primary plus IMDb alias;
- Kitsu primary plus IMDb alias;
- locale fallback;
- English fallback;
- provider outage;
- stale last-known-good;
- episode numbering;
- safe network or company navigation;
- no user-override replacement.

## 11. MDBList ratings and catalogs

Treat MDBList ratings and MDBList lists as separate capabilities.

### 11.1 Ratings

Implement `ratings.read`.

Store each provider score as a separate `RatingClaim`.

Do not flatten several ratings into one field.

Support approved rating sources such as:

- IMDb;
- TMDB;
- Rotten Tomatoes;
- Metacritic;
- Trakt;
- Letterboxd;
- audience score;
- MAL.

Use the current MDBList API schema.

Prefer batch requests where available.

Use bounded concurrency.

Respect the account request limit.

Do not place the API key in Fasti logs or cache identity.

### 11.2 Lists and catalog sources

Implement the current MDBList list and catalog APIs as direct Collection and Discover sources.

Do not route MDBList through Trakt when a direct supported path exists.

Treat account watchlist or watched-state operations as separate connection capabilities.

Do not enable those writes because ratings are enabled.

### 11.3 Account synchronization

Implement separate grants for:

```text
ratings.pull
ratings.push
watchlist.pull
watchlist.push
watched.pull
watched.push
collection.pull
collection.push
dropped.pull
dropped.push
scrobble.push
```

Use the current `/sync/watched`, `/sync/ratings`, `/sync/collection`, `/sync/dropped`, `/watchlist/items`, and scrobble operations. Each capability has its own enablement, direction, cursor or baseline receipt, request budget, failure state, and reconciliation preview. Pulling public aggregate scores never enables account-state reads. Reading account state never enables writes. A delete is an explicit remove request, never inferred from an absent page.

MDBList responses remain account and purpose scoped. Fasti does not publish them through a public Stremio catalog, reuse them across profiles, or retain them beyond the allowed cache and evidence policy.

## 12. Search

Create one Search application capability.

Search scopes:

```text
local
configured_metadata_providers
addon_catalogs
connected_services
all_available
```

Search must:

- always search the local Fasti index offline;
- query enabled remote sources only when allowed;
- return source provenance;
- retain all exact identifiers;
- group likely duplicate candidates without merging Records;
- use bounded provider fan-out;
- use keyset pagination;
- allow the user to filter by media domain and source;
- let one governed action create or attach a Record;
- expose unresolved and partial identity.

Each result opens a real details route without first creating a Record:

```text
/explore/{source}/{grain}/{candidate_receipt_id}/{slug}
```

`candidate_receipt_id` is opaque and resolves to a durable, bounded provider-candidate receipt containing the exact governed re-fetch route and provenance. A receipt expires after 24 hours, carries at most 64 KiB of normalized candidate data plus bounded identifiers, and records the query digest, safe provider-configuration digest, grant digest, response digest, provider terms revision, and creating actor/profile. Replay re-authorizes the current actor, profile, provider capability, grant, and configuration; a digest or authorization mismatch fails closed and offers a fresh Search. Expired and unreferenced receipts are garbage-collected in bounded keyset pages; receipts attached to an operation or Record retain only the minimal provenance required by that durable owner. No credential, raw secret-bearing request, or unrestricted provider body enters a receipt. The slug is presentation-only. On Record creation or attachment, the stable route becomes:

```text
/records/{grain}/{record_id}/{slug}
```

Changing title or slug redirects to the canonical Record route. Record URLs never use a provider ID as the durable key.

Do not make TMDB the only search path.

Direct catalog search must remain available for content that has no TMDB match.

## 13. Discover

Create a source-neutral `DiscoverRailDefinition`.

Example:

```text
rail_id
title
description
source_binding
media_grains
profile_scope
filters
sort
page_size
cache_policy
fallback_policy
visibility
```

Supported source bindings:

```text
fasti_smart_query
fasti_collection
tmdb_discover
tmdb_list
tmdb_collection
tmdb_company
tmdb_network
tmdb_person
tmdb_director
mdblist_list
addon_catalog
nuvio_catalog_reference
static_versioned_pack
```

Trakt can be an optional source binding.

Trakt is not a required source binding.

Each rail shows:

- source;
- last refresh;
- fresh, stale, or unavailable state;
- retry;
- explanation;
- local fallback when available.

Do not construct Discover rails in Svelte.

## 14. Library

Library is a local query over Fasti-owned state.

Implement server-side:

- keyset pagination;
- search;
- media-grain filter;
- saved-state filter;
- progress-state filter;
- watched-state filter;
- rating filter;
- Collection filter;
- unresolved-identity filter;
- metadata-source filter;
- stable sort.

Remove the 500-record presentation ceiling as a product limit.

The UI can use a bounded page.

Do not load the complete library to filter one page.

A Record with no provider ID remains a valid Library item.

Keep these states separate:

```text
saved
progress
watched_or_completed
history
rating
collection_membership
```

## 15. Collections and packs

Use a portable Collection descriptor.

```text
Collection
- collection_id
- title
- description
- layout
- folders
- owner
- version

CollectionFolder
- folder_id
- title
- presentation
- source_bindings

CollectionSourceBinding
- binding_id
- adapter
- source_kind
- adapter_config
- read_authority
- write_authority
- refresh_policy
- status
```

Do not store hard-coded Collection packs in a Svelte component.

Move sample and curated packs to versioned data files.

Each pack has:

- schema version;
- pack ID;
- pack version;
- source URL;
- digest;
- licence;
- author;
- minimum Fasti version;
- minimum Nuvio compatibility revision;
- source requirements;
- credential requirements;
- preview;
- rollback data.

Installation modes:

```text
preview
merge
replace
install_as_reference
```

Never replace an existing document without a preview and confirmation.

Preserve unknown Nuvio fields for round-trip compatibility.

Imported source URLs and URL-like extension values remain inert and disabled until a separate activation authorizes them. Import rejects user information, fragments, secret-bearing query parameters, non-HTTPS schemes, and unsafe origins. Credentials are represented only by vault references. Activation and every refresh re-run the current scheme, host, port, DNS, address-class, redirect, grant, and provider-configuration-digest policy before any request. Unknown extension fields round-trip as data but never become executable adapter configuration.

Reject duplicate Collection IDs and duplicate Folder IDs.

Bound:

- file size;
- collection count;
- folder count;
- source count;
- JSON nodes;
- depth;
- string length.

## 16. Kaptain Collection compatibility

Add a Kaptain Collection importer and validator.

Do not copy its content into Svelte constants.

The importer must:

- parse the Nuvio-native JSON;
- preserve unknown extension fields;
- classify every source;
- show required providers;
- show required credentials;
- show Trakt dependencies;
- show unsupported source types;
- show the exact count of Collections, folders, and sources;
- offer a dry run;
- store the source digest;
- support revalidation;
- support rollback.

For a Trakt source:

1. use a direct non-Trakt equivalent when the pack declares one;
2. use a verified TMDB or MDBList translation when semantics are equivalent;
3. keep the source disabled with a clear reason when no safe equivalent exists;
4. never silently delete the folder;
5. never silently change list meaning.

Issue `ImKaptain/Kaptain-Collection#9` is a required regression scenario.

CI uses only synthetic structure-equivalent fixtures. M7 cannot be declared complete from those fixtures alone: release acceptance also checks the exact pinned public Kaptain document from a temporary source checkout, records only its commit, file digest, counts, classification/loss report digest, and pass/fail receipt, then removes the checkout. No Kaptain content or asset is copied into Fasti, an artifact, a log, or a test result. The real-file check must classify every source and round-trip unknown fields; an unavailable or changed pinned source blocks the compatibility claim rather than becoming a deferral.

## 17. Nuvio compatibility profile

Create a versioned Nuvio compatibility profile.

Pin every claim to:

- NuvioTV commit;
- Nuvio Desktop commit;
- Nuvio Collection schema fixture;
- Fasti contract version.

## 17.1 Existing slice

Preserve and harden:

- complete-occurrence webhook;
- stable source event identity;
- durable Fasti receipt;
- profile-scoped Nuvio Collections JSON;
- unknown-field preservation;
- strict limits;
- no URL fetch during import.

## 17.2 Catalog and metadata publication

Implement Fasti as a read-only local Stremio-compatible add-on.

Publish:

```text
/manifest.json
/catalog/{type}/{catalog_id}.json
/meta/{type}/{external_id}.json
```

Do not publish streams.

Expose only explicitly shared:

- Fasti lists;
- selected Collections;
- selected Discover rails;
- normalized metadata projections.

Use a revocable publication descriptor, not an authentication claim. Standard Stremio requests provide no Fasti authorization-header contract, so every published field is intentionally readable by anyone who can reach that listener and URL. The settings preview must say this before activation and list the selected fields, Collections, rails, listener scope, and cache lifetime.

Do not put a long-lived secret inside an add-on URL. Use an opaque, non-sequential descriptor ID only to prevent enumeration; it is not a bearer credential and must never be presented as access control. The URL shape is `/addons/{descriptor_id}/manifest.json` with matching catalog/meta routes under that descriptor. Publication requires an explicitly configured listener. Revocation returns the governed unavailable response and purges the descriptor partition. Private Library, progress, watched, ratings, notes, credentials, and account-scoped MDBList data use the authenticated Fasti provider contract and are never published through Stremio.

## 17.3 Direct Nuvio Cloud connection

Implement a real Nuvio Cloud connection through the official public API surface used by the pinned clients.

The trusted host:

1. validates the official or operator-approved self-hosted HTTPS endpoint through the governed outbound policy;
2. exchanges email and password at `/auth/v1/token?grant_type=password`;
3. discards the password immediately;
4. stores only the refresh token in the approved credential vault;
5. loads the account's Nuvio profiles and binds one explicit profile per Fasti Connection;
6. records independent grants for library, watched, and progress pull and push.

Refresh tokens are rotating and single-use. Every connection has one durable lease/lock and a SQLite refresh-attempt row containing the current generation, proposed next generation, attempt ID, state, and safe timestamps. The sequence is:

```text
durably mark attempt pending
  -> call refresh once with current vault generation
  -> write and fsync provisional next-generation token in the vault
  -> compare-and-set SQLite generation and mark next token active
  -> retire the old vault generation
  -> continue profile/pull/delta/push/delete work
```

Restart reconciles a provisional vault entry with its pending attempt and can finish activation without calling refresh again. Failure before the request is retryable. A timeout/lost response, or a crash after the remote service spends the old token but before the next token is durably written, is intrinsically ambiguous and enters `reauthorization_required`; it never loops on the possibly spent token. Fault injection covers every arrow. No prose or receipt claims cross-system atomicity.

Use the current pinned RPC families:

```text
sync_pull_profiles
sync_pull_library
sync_get_library_delta_cursor
sync_pull_library_delta
sync_push_library_items
sync_delete_library_items
sync_pull_watched_items
sync_get_watched_items_delta_cursor
sync_pull_watched_items_delta
sync_push_watched_items
sync_delete_watched_items
sync_pull_watch_progress
sync_get_watch_progress_delta_cursor
sync_pull_watch_progress_delta
sync_push_watch_progress
sync_delete_watch_progress
```

All mutations include a stable, non-secret Fasti origin-client ID. The adapter uses snapshots for bootstrap and cursor recovery, deltas for steady-state pull, explicit remote deletes for tombstones, bounded batches, and compare-before-apply receipts. Empty pages never imply deletion.

Each Library, watched, progress, rating, and Collection direction is configured as `pull`, `push`, or `two_way`. Activation records a baseline receipt. Reconciliation is a three-way comparison of baseline, current local, and current remote state; wall-clock last-write-wins is forbidden because clocks and offline duration are untrusted. If only one side changed, that change applies under the granted direction. Equal changes converge. Two different changes produce a conflict receipt and preserve both values. Explicit delete/reset is a change, not absence; progress is never merged by numeric maximum because rewatches and resets are valid. Ambiguous identity creates a review item and cannot overwrite either side. Push uses the new baseline only after the local receipt and remote acknowledgement are both durable.

This adapter does not access Nuvio tables, use a service-role credential, persist a password, or reproduce Nuvio's storage schema in Fasti.

## 17.4 Fasti provider contract for Nuvio clients

Publish a versioned Fasti-owned contract for Nuvio Desktop and NuvioTV. Pair through the authentication programme's Authorization Code with PKCE or Device Authorization flow. Store a revocable native credential and send it only in the authorization header.

The contract provides capability discovery, profile grants, bounded snapshots, ordered deltas, explicit deletion tombstones, mutation with compare-and-set revisions, acknowledgements, idempotency receipts, cursor recovery, conflict receipts, and diagnostics for Library, watched, progress, ratings, Collections, catalog references, and metadata freshness.

Fasti derives every idempotency scope server-side from workspace, profile, client, credential epoch, exact grant ID and version, capability, object and operation, connection and lane, contract revision, request digest, and restore generation. A scope mismatch returns a fresh authorization or idempotency conflict without revealing the prior receipt.

The Nuvio client commits each local change and immutable operation-journal entry in one local transaction. Its dispatch outbox is derived from unacknowledged journal entries and can be rebuilt. Playback never waits for Fasti. Cursor expiry performs a bounded snapshot at a named sequence, reconciles stable state IDs and revisions, reapplies unacknowledged operations by idempotency key, and resumes after the snapshot sequence.

Fasti uses a separate shared synchronization substrate for its outbound MDBList and Nuvio Cloud work and its provider-side acknowledgements. The SQLite owner stores immutable operations, attempts and `next_attempt_at`, remote receipts, acknowledgements, per-lane cursors, explicit tombstones, and renewable fenced connection leases. A local mutation and journal entry commit in one transaction. The derived outbox selects bounded due pages ordered by `(next_attempt_at, operation_id)`. Network work starts only after that transaction and holds no SQLite lock; a remote receipt, acknowledgement, and cursor advance then commit atomically. Restart reconstructs all work from durable rows. The bounded dispatcher drains cleanly on shutdown and an expired lease cannot commit because every transition verifies the fencing generation.

The 30-day window applies only to compactable server delta payloads and inactive cursors. Durable state, explicit tombstones needed by retained state, and mutation idempotency receipts are not deleted by feed compaction. A client journal entry has no time-based deletion: it remains until the matching server receipt and local acknowledgement are durable or the user explicitly retires the client through a reviewed discard flow. Per-workspace, profile, client, and lane row and byte quotas are checked before mutation admission. Acknowledged receipt bodies compact to the minimum replay digest, and a bounded sweeper retires only policy-eligible evidence. Quota exhaustion rejects before mutation, exposes an operator-visible quarantine or client-retirement action, and leaves existing state writable and recoverable. A cursor outside the window receives `nuvio_cursor_expired` and recovers by snapshot. Feed compaction proves that a minimum retained snapshot sequence still exists before deleting deltas; revoked clients lose access immediately but their audit/receipt evidence follows the normal archive retention policy.

M11 owns seven independently landable review units: M11a shared Fasti journal/cursor/lease substrate; M11b Nuvio Cloud authorization, profile discovery, and snapshot pull; M11c Cloud delta, push, explicit delete, and three-way reconciliation; M11d Fasti provider v1 server and conformance fixture; M11e Nuvio Desktop client; M11f NuvioTV client; and M11g exact-revision cross-client evidence. Direct Cloud and Fasti-provider lanes are independently activatable and releasable. Upstream acceptance cannot block the safe Fasti Cloud lane or Fasti provider server. End-to-end fixtures use the same contract revision. A Fasti lane is `implemented` when its own branch and required integration tests pass; a client is `released in Nuvio` only after the upstream change is accepted in a published Nuvio build.

## 18. Cache policy

Use provider response headers where safe.

Apply Fasti maximums.

Initial policy:

| Resource | Fresh | Stale while refreshing | Stale on error |
| --- | ---: | ---: | ---: |
| Add-on manifest | 6 hours | 1 hour | 7 days |
| Metadata field claims | 24 hours | 12 hours | 7 days |
| Discover or catalog page | 15 minutes | 15 minutes | 24 hours |
| Search response | 2 minutes | none | 10 minutes |
| Positive identity route | 7 days | 1 day | 30 days |
| Negative identity result | 15 minutes | none | none |

A provider can shorten these values.

A provider cannot extend them above the Fasti safety cap without an approved policy change.

Cache identity must include:

- provider or add-on ID;
- safe configuration digest;
- media grain;
- identifier namespace;
- identifier value;
- language;
- region;
- schema version.

Do not key only on content ID.

## 19. UI information architecture

Use these destinations:

```text
Explore
├── Search
└── Discover

Library
├── All Records
├── In progress
├── Saved
├── Completed
└── Needs review

Collections
├── My Collections
├── Source packs
└── Nuvio compatibility

Settings
├── Metadata and ratings
│   ├── Enrichment
│   ├── Ratings
│   ├── Language and region
│   └── Provider health
│
├── Connections
│   ├── Nuvio
│   ├── Tracking services
│   ├── Metadata services
│   └── Add-on catalogs
│
└── Identity and compatibility
    └── Anime grouping and export preference
```

Replace the provider credential card wall with a compact Tabler table or list.

Columns:

- Provider;
- Purpose;
- Credential requirement;
- Status;
- Source;
- Last validated;
- Last success;
- Expiry;
- Last error;
- Actions.

Actions:

- Configure;
- Connect;
- Replace;
- Test;
- Reauthorize;
- Remove;
- Documentation;
- Safe details.

Never provide plaintext read-back.

## 20. First-run and setup

Use a separate resumable setup journey.

Steps:

1. choose language and region;
2. configure one useful metadata route;
3. test Search;
4. create the first Record;
5. choose optional rating enrichment;
6. choose anime grouping and export behavior when relevant;
7. install or import an optional Collection pack;
8. connect Nuvio when available;
9. verify backup and recovery.

Each completed step links to the permanent settings destination.

Do not make all providers mandatory.

Do not make Nuvio mandatory.

## 21. UX and copy rules

Apply Kathy Sierra's principles.

The user goal is:

> Find media, keep an accurate local Library, enrich it safely, and use it in Nuvio without losing identity or history.

Use:

- just-in-time explanations;
- progressive disclosure;
- one primary action;
- persistent errors;
- saved progress;
- exact next actions;
- no guilt;
- no transient-only security or connection status;
- no protocol terms unless the user must make a protocol decision.

For provider or anime preference changes, state:

- what changes;
- what does not change;
- affected count;
- safe state;
- rollback.

Use ASD-STE100 Simplified Technical English.

## 22. Accessibility

Meet WCAG 2.2 Level AA for applicable web surfaces.

Record EN 301 549 evidence for applicable web, software, and documentation clauses.

Test:

- keyboard;
- focus order;
- focus entry and return;
- Escape;
- visible focus;
- 44 by 44 CSS-pixel targets;
- contrast;
- no color-only status;
- 320-pixel reflow;
- 200-percent zoom;
- text spacing;
- reduced motion;
- forced colors;
- screen-reader names;
- live status;
- persistent errors;
- accessible authentication;
- interruption recovery.

Audit with:

- AskTog principles;
- Gestalt grouping;
- all ten Nielsen heuristics;
- relevant IxDF research;
- Impeccable;
- Axe;
- Playwright;
- manual assistive-technology review.

## 23. Security

Data classification is enforced at every storage, cache, export, publication, synchronization, and sharing boundary:

| Class | Data | Required handling |
| --- | --- | --- |
| Restricted | Passwords, provider tokens, refresh tokens, API keys, and recovery material | Vault only; never URLs, browser storage, logs, diagnostics, fixtures, receipts, or normal exports. |
| Confidential | Profile Library, watched/progress/rating/note/history state, account-scoped responses, raw envelopes, journals, receipts, share membership, and private Collection state | Profile/workspace authorization, encrypted authenticated backup, purpose-partitioned caches, explicit grants, and no public projection. |
| Internal | Safe external identifiers, non-secret provider configuration, bounds, compatibility revisions, redacted diagnostics, and provenance digests | Integrity-bound and least-privilege access; may appear in support evidence only when redacted. |
| Public | Provider-public metadata and fields explicitly selected in an active Stremio/publication descriptor | Allowlisted projection only; revocation purges caches. |

Normalization or projection never lowers a classification. Only an explicit reviewed publication descriptor can select Public fields, and it cannot select Restricted or account/profile-private source data.

Threat-model:

- provider credential leakage;
- query-string provider credentials;
- cache-key credential leakage;
- diagnostics leakage;
- SSRF;
- DNS rebinding;
- redirect to private address;
- oversized response;
- deep JSON;
- decompression bomb;
- malicious Collection pack;
- malicious add-on manifest;
- cross-profile credential use;
- cross-profile Collection access;
- metadata cache cross-contamination;
- stale claim replacing user override;
- unsafe identity attachment;
- title-only merge;
- source dependency disappearance;
- provider rate exhaustion;
- unbounded fan-out;
- navigation using invalid enriched IDs;
- remote image tracking;
- secret duplication in configured add-on URLs.
- stale authority on long-lived streams or receipt replay;
- durable journal exhaustion;
- stale grants, journals, or descriptors after restore.

Add negative controls.

Provider credential changes, Nuvio Cloud password exchange, sync-direction changes, client grants/revocation, publication activation, Collection replacement, and local-sharing changes require the authentication programme's recent-authentication gate plus its request-boundary CSRF/Origin/Host policy. A Nuvio password is accepted only through Tauri/loopback or an authenticated HTTPS browser session, uses a secret input, is zeroized by the trusted host after the one exchange, and never enters application state, receipts, diagnostics, retry bodies, or browser storage. A self-hosted Nuvio endpoint shows and reconfirms the exact validated origin before any password is submitted.

The Stremio publication surface is security-reviewed as public read-only content on its configured listener. Opaque descriptor IDs prevent enumeration but do not authorize. Negative fixtures prove that no private profile state, account-scoped provider response, notes, unresolved evidence, credential reference, sequential internal ID, or stream route can enter its manifest, catalog, metadata, cache, error, or logs.

A gate passes only when a deliberate defect makes the test fail.

## 24. Contracts

The developer-facing integration contract reuses the existing authored provider manifest at `contracts/addons/manifests/google-books.provider.yaml`, the capability registry, generated SDK, and `cargo xtask`. M1 makes that currently isolated manifest a validated source rather than introducing a plugin framework. A provider manifest is declarative validated input, not an executable plugin language. Provider runtime code remains an in-repository reviewed adapter that uses the existing application, identity, credential, and governed-egress owners. Add one schema-owned fixture convention under `contracts/addons/fixtures/{provider_id}/` and one repository-local command:

```text
cargo xtask integration check <manifest-or-client-fixture>
```

The input declares its own kind, so the command dispatches to the narrow metadata-source, public-catalog, or Fasti-state-client conformance lane without a second configuration file. It validates only authored sources and deterministic fixtures, never rewrites generated output. It follows the existing Fasti CLI spelling `--output human|json`; it does not add `--format`. Exit `0` means every requested check passed, `2` means manifest/fixture/compatibility validation failed, and `1` means the local tool or environment failed. `cargo xtask contract generate`, `cargo xtask contract verify --locked`, and `cargo xtask test pr` retain their current meanings. The narrow check uses the existing Clap/xtask process, Rust contract types, loopback fixtures, and problem catalog; it adds no daemon, package, template engine, or telemetry service.

The minimum metadata-source example contains one manifest and four bounded response fixtures: success, empty, rate-limited, and invalid response. The command proves manifest shape, source/licence declaration, capability ownership, identifier/grain mapping, normalization, pagination bounds, safe transport policy, secret placement, empty-versus-delete semantics, and typed error mapping. Live provider calls and credentials are optional smoke evidence and never part of first success or CI.

The same command accepts a Fasti provider-client fixture in M11 and proves capability discovery, version negotiation, snapshot, delta, mutation, tombstone, acknowledgement, idempotency, cursor recovery, and revocation against the versioned loopback conformance server. Nuvio Desktop and NuvioTV use their installed native HTTP and serialization stacks against the plain contract; Fasti does not add a speculative Kotlin SDK.

The new, original interoperability specification, schemas, examples, and compatibility fixtures live under `contracts/interoperability/fasti-provider/v1/` and are dual-licensed `Apache-2.0 OR AGPL-3.0-or-later`. Each authored file carries that SPDX expression; the subtree contains the Apache licence and a scope notice; `CONTRIBUTING.md` explicitly states that contributions to it are offered under both choices. Fasti implementation code and the existing generated SDK remain AGPL-3.0-or-later. Upstream Nuvio clients may consume the Apache-2.0 specification/fixtures or generate native types from that permissive OpenAPI, but they never copy Fasti implementation or SDK code. The source package is authored for this contract and must not be generated from or copy an AGPL-only file. The contract gate validates SPDX scope, licence files, provenance, generated-output lineage, and absence of symlink/path escape before external reuse. Apache-2.0 is an SPDX-listed OSI licence and is compatible with GPLv3 according to the GNU project's GPLv3 guidance; this licensing decision is limited to the new interoperability subtree and does not relicense existing work.

For each capability, inspect:

- capability registry;
- OpenAPI 3.1;
- AsyncAPI 3.x;
- JSON Schema 2020-12;
- JSON-LD or reasoned `N/A`;
- SDK;
- CLI;
- permissions;
- typed problems;
- lifecycle;
- examples;
- KCS knowledge;
- conformance.

Candidate capability IDs:

```text
provider.list
provider.credential.configure
provider.credential.test
provider.health.read
metadata.search
metadata.claim.refresh
metadata.projection.read
metadata.projection.configure
rating.read
catalog.source.preview
catalog.source.refresh
discover.rail.list
library.query
collection.create
collection.update
collection.source.bind
collection.pack.preview
collection.pack.install
collection.export
collection.import
nuvio.catalog.publish
nuvio.collection.project
connection.test
connection.health.read
mdblist.rating.pull
mdblist.rating.push
mdblist.watchlist.pull
mdblist.watchlist.push
mdblist.watched.pull
mdblist.watched.push
mdblist.collection.pull
mdblist.collection.push
mdblist.dropped.pull
mdblist.dropped.push
mdblist.scrobble.push
nuvio.cloud.connect
nuvio.cloud.profile.list
nuvio.cloud.snapshot.pull
nuvio.cloud.delta.pull
nuvio.cloud.state.push
nuvio.cloud.state.delete
nuvio.provider.snapshot.read
nuvio.provider.delta.read
nuvio.provider.state.mutate
nuvio.provider.state.delete
nuvio.provider.acknowledge
nuvio.provider.reconcile
workspace.local.share.configure
workspace.local.discovery.read
workspace.local.member.grant
workspace.local.projection.read
```

Candidate events:

```text
provider.health.changed
metadata.claim.updated
metadata.projection.changed
rating.claim.updated
catalog.refresh.completed
discover.rail.updated
collection.changed
connection.health.changed
```

Do not invent AsyncAPI channels for synchronous provider search.

## 25. Error catalog

Add stable problems:

```text
provider_unavailable
provider_credential_missing
provider_credential_invalid
provider_credential_expired
provider_rate_limited
provider_response_invalid
provider_route_unavailable
metadata_claim_stale
identity_route_ambiguous
identity_route_missing
search_source_unavailable
search_query_invalid
catalog_source_invalid
catalog_source_dependency_missing
catalog_refresh_failed
collection_pack_invalid
collection_pack_dependency_missing
collection_pack_conflict
collection_source_quarantined
nuvio_compatibility_mismatch
connection_reauthorization_required
nuvio_refresh_token_spent
nuvio_cursor_expired
nuvio_revision_conflict
nuvio_idempotency_conflict
nuvio_remote_contract_changed
nuvio_partial_batch_failed
sync_storage_quota_exhausted
stream_authority_expired
restore_generation_stale
workspace_share_not_enabled
workspace_share_grant_denied
workspace_share_discovery_invalid
workspace_share_projection_private
```

Each problem includes:

- safe message;
- safe state;
- retryability;
- exact next action;
- correlation ID;
- documentation link.

Developer tools render the same problem in two projections. The default terminal projection puts the code and exact remediation first, includes the safe actual value or JSON/YAML pointer, names the affected capability, and links its documentation. `--output json` emits the governed problem unchanged for automation. Stack traces and secret-bearing values remain hidden by default; maintainers can use standard `RUST_BACKTRACE` after the safe summary. At minimum, conformance evidence exercises these three paths:

| Path | Required developer result |
| --- | --- |
| Invalid provider manifest or fixture | Stable code, file and JSON/YAML pointer, expected shape, safe received value, exact edit, documentation link, exit status `2`. |
| Denied provider address, redirect, or response | Stable transport problem, provider/host and denied address class without credentials, retained local state, retryability, next diagnostic action, correlation ID. |
| Unsupported Nuvio/Fasti contract or expired cursor | Client and server revision/range, whether snapshot recovery is safe, exact compatible action, retained journal state, documentation link. |

One-line `PASS` output names the integration ID, kind, contract revision, checks run, and fixture digest. A failed check prints the first actionable cause before any subordinate context. Partial multi-fixture results are preserved so a contributor fixes one failure at a time.

## 26. Performance and offline

Use the live Fasti budgets.

Current targets:

- 64 MiB idle;
- 96 MiB normal;
- 160 MiB heavy;
- 192 MiB absolute process-tree ceiling.

Measure:

- local Search;
- provider Search;
- Discover rail;
- Library page;
- Collection page;
- TMDB enrichment;
- MDBList ratings;
- pack import;
- Nuvio catalog page;
- Nuvio metadata response;
- cache refresh;
- offline startup;
- reconnect;
- backup;
- restore.

Use:

- bounded concurrency;
- bounded pages;
- keyset pagination;
- no full-library materialization;
- no per-item task explosion;
- no network call inside the SQLite writer transaction;
- no provider requirement for local correctness.

Offline behavior:

- local Search works;
- Library works;
- Collections work;
- last-known-good metadata works;
- stale rails remain visible with status;
- remote refresh fails safely;
- no local data is removed.

## 27. Test matrix

Required fixtures:

1. MAL primary ID plus valid IMDb alias enriches through TMDB.
2. Kitsu primary ID plus valid IMDb alias enriches through TMDB.
3. Record with no provider ID remains in Library.
4. Local search returns a Record while TMDB is offline.
5. Direct add-on catalog search returns content that TMDB cannot find.
6. User title override survives provider refresh.
7. Fresh preferred claim wins.
8. Stale last-known-good survives provider failure.
9. MDBList API key does not enter logs, errors, metrics, or cache keys.
10. Duplicate Collection ID is rejected.
11. Duplicate Folder ID is rejected.
12. Unknown Nuvio fields survive round-trip.
13. Trakt source without list ID is rejected.
14. Kaptain pack without Trakt reports exact disabled sources.
15. Collection source absence does not delete a Collection.
16. Provider empty response does not delete metadata.
17. Cache from one add-on configuration is not served for another.
18. Anime preference changes projection, not Record ID or Chronicle.
19. Nuvio compatibility mismatch fails closed.
20. Cross-profile Collection access is denied.
21. Pack URL SSRF is denied.
22. Oversized pack is denied before unbounded allocation.
23. Library keyset pagination does not duplicate or skip stable rows.
24. Discover stale-on-error state remains visible.
25. Browser UI never stores a provider secret.
26. A rotated Nuvio refresh token is durably stored before a later RPC failure.
27. Two concurrent operations on one Nuvio connection cannot redeem the same refresh token.
28. Nuvio library, watched, and progress snapshots preserve unrelated local state.
29. Nuvio deltas apply once in sequence and cursor replay is idempotent.
30. Nuvio explicit deletes create tombstones; empty pages delete nothing.
31. Nuvio push batches include the stable origin-client ID and suppress reflected changes.
32. Cursor expiry restores from a named snapshot and reapplies the local journal once.
33. Fasti provider mutations reject stale revisions with a conflict receipt.
34. Revocation denies Fasti provider reads, writes, acknowledgements, and receipt disclosure.
35. Every Search result opens a details route without creating a Record.
36. A changed details slug redirects without changing the Record or candidate identity.
37. MDBList ratings, watchlist, watched, collection, dropped, and scrobble grants cannot authorize one another.
38. MDBList explicit removals are replay-safe and an absent page deletes nothing.
39. Local workspace discovery contains no profile, title, history, credential, or stable personal identifier.
40. A local workspace member can read only explicitly shared projections and never another profile's private Library state.
41. Nuvio refresh fault injection before request, after send, after response, after provisional vault write, after generation compare-and-set, and before old-token retirement yields the specified retry, recovery, or reauthorization state without a second blind refresh.
42. Nuvio three-way reconciliation covers unchanged/changed, equal concurrent changes, conflicting changes, explicit delete, progress reset, ambiguous identity, and reflected-origin suppression for every granted lane.
43. A Stremio descriptor is labelled public, works without a secret URL, exposes only its approved fields, and returns unavailable with a purged cache after revocation.
44. Delta compaction retains a recoverable snapshot sequence, durable state, required tombstones, and idempotency receipts; an expired cursor recovers without dropping an unacknowledged client journal entry.
45. M7 checks the exact pinned Kaptain source from a temporary checkout and retains only a digest/count/classification receipt; no source content enters Git, CI artifacts, or logs.
46. The new interoperability subtree passes SPDX, dual-licence scope, provenance, generated-lineage, symlink, and path-escape checks; upstream clients contain no copied Fasti implementation or SDK code.
47. Every integration command and problem documentation link executes against the built docs artifact; public SDK operations remain fully typed without an unknown-record escape.
48. Real credentialed TMDB, MDBList, and Nuvio Cloud acceptance runs against pinned/current compatible endpoints with redacted receipts and confirms the deterministic behavior in live transport.
49. Pinned Nuvio Desktop and NuvioTV builds consume one real local Fasti node through their native clients; no Fasti-only test double is accepted as end-to-end evidence.
50. The browser E2E suite uses a disposable real `fastid`, real SQLite data root, supported account/session bootstrap, and deterministic loopback provider servers; the current health-only stub cannot prove a programme flow.
51. Library and Collection query-plan fixtures prove the intended compound indexes, bounded query count, and no per-row provider/metadata lookup at 0, 100, and 10,000 Records.
52. Idle, normal, heavy, and absolute memory gates independently enforce 64, 96, 160, and 192 MiB in every valid repetition; early exit, swap, missing samples, or workload drift fails.
53. A query-key-only provider remains unavailable, and planted secrets or their digests never enter a dispatched URI, log, error, cache key, diagnostic, fixture, or receipt.
54. Imported userinfo, fragments, secret query parameters, non-HTTPS URLs, and unsafe origins are rejected; accepted URL-like extensions stay inert until a fresh activation policy check and never execute from unknown fields.
55. An active local-workspace stream closes on session expiry, role or membership change, grant narrowing, credential rotation, restore, or client revocation; resume under a mismatched scope fails without replay, and browser transport never uses a URL token.
56. An idempotency key cannot replay across workspace, profile, client, credential epoch, grant ID/version, capability, object/operation, connection/lane, contract revision, request digest, or restore generation, and a mismatch discloses no prior receipt.
57. Per-workspace, profile, client, and lane row/byte quotas reject before mutation; acknowledged bodies compact to replay digests, the bounded sweeper preserves required evidence, and existing state remains writable and recoverable at quota.
58. Archive v3-v7 refuse to operate before `C3-CRYPTO`; restore advances the generation and leaves connections, publication, sharing, and dispatchers quarantined until recent-auth review, fresh snapshot, and conflict reconciliation succeed.
59. Non-loopback local sharing fails without the configured HTTPS/trusted-proxy/public-origin/OS-trusted-CA contract; QR and discovery payloads contain no bearer, profile, title, history, credential, or stable personal identifier.
60. Restricted, Confidential, Internal, and Public data fixtures cannot cross a lower-class projection or cache partition except through an active field-allowlisted public descriptor.

## 28. Delivery roadmap

Use separate reviewable PRs.

### PR M0 — Planning and truth map

- canonical plan;
- Context Manifest;
- source ledger;
- current capability map;
- UX map;
- threat model;
- contract disposition;
- dependency graph;
- no production code.

### PR M1 — Provider registry and credentials

- shared `fasti-provider-runtime` composition root used by `fastid` and Tauri;
- provider capability registry;
- one governed transport, credential-reference port, bounded dispatcher, and request budgets;
- per-capability credential requirements;
- credential status;
- credential vault integration;
- compact settings UI;
- health and credential tests.

### PR M2 — Metadata claims, projections, and cache

- field-level claims;
- projection policy;
- freshness;
- last-known-good;
- cache keys;
- provenance UI;
- contract surfaces.

### PR M3 — Purpose-specific identity routing and anime policy

- resolution intents;
- TMDB alias routing;
- anime grouping and export preference;
- impact preview;
- golden Nuvio bug fixtures.

### PR M4 — Search

- local index;
- multi-source query;
- result grouping;
- keyset pagination;
- Record action;
- Search UI.

### PR M5 — Library

- server-side Library query;
- keyset pagination;
- state filters;
- offline behavior;
- Library UI convergence.

### PR M6 — Discover

- rail definitions;
- local smart rails;
- TMDB Discover;
- source state;
- stale fallback;
- Discover UI.

### PR M7 — Collections and pack registry

- Collection model;
- source bindings;
- versioned packs;
- dry run;
- merge and replace;
- Kaptain importer;
- remove hard-coded Svelte packs.

### PR M8 — Complete TMDB enrichment

- field groups;
- localization;
- episodes;
- companies;
- networks;
- collections;
- trailers;
- route tests;
- attribution.

### PR M9 — MDBList ratings, catalogs, and account synchronization

M9 lands as two independently activatable review units:

- **M9a public/read provider lane:** `RatingClaim`, provider priority, bounded batch calls, request budgets, and direct read-only list/catalog adapters;
- **M9b private account-state lane:** independently granted watchlist, watched, collection, dropped, and scrobble pull/push; account-partitioned retention; explicit removal; reconciliation preview; baseline receipt; and loop suppression;
- neither lane has a hidden Trakt dependency and neither grant authorizes the other.

### PR M10 — Nuvio catalog and metadata profile

- pinned compatibility profile;
- read-only Stremio manifest;
- Fasti catalog resources;
- metadata resources;
- Collection source projection;
- no streams.

### PR M11 — Paired Nuvio workspace

- **M11a substrate:** Fasti immutable journal, attempts, retry schedule, receipts, acknowledgements, per-lane cursors, tombstones, fenced leases, derived outbox, bounded dispatcher, restart, and clean shutdown;
- **M11b Cloud bootstrap:** official Nuvio Cloud sign-in, profile selection, rotating-token persistence, health, reauthorization, capability discovery, and bounded snapshot pull;
- **M11c Cloud steady state:** independent library, watched, and progress pull/push grants; delta, upsert, explicit deletion, three-way reconciliation, and diagnostics;
- **M11d Fasti provider:** device pairing from the authentication programme plus profile grants, revisions, idempotency, receipts, acknowledgements, cursor recovery, and conformance fixture;
- **M11e Desktop:** durable Nuvio Desktop journal/outbox and matching native client;
- **M11f TV:** durable NuvioTV journal/outbox and matching native client;
- **M11g cross-client evidence:** exact-revision fixtures for progress, saved state, watched state, ratings, Collections, metadata freshness, cache invalidation, revocation, restart, and offline recovery.

The pinned Nuvio Cloud RPC surface is a compatibility profile, not a promise of public stability, unless an authoritative published stability and terms statement is recorded. Schema-drift probes run before activation and any incompatible surface fails closed without consuming the old token or changing local state.

### PR M13 — Local Shared Media Workspace

M13 retains the complete approved scope but lands in independently reviewable slices:

- **M13a discovery and enablement:** explicit administrator consent, manual URL and QR connection, and optional mDNS/DNS-SD containing only a random rotating service hint; non-loopback use reuses the existing absolute HTTPS public URL, trusted-proxy, DNS, and OS-trusted-CA contract; QR data contains only the authorized origin plus a non-secret rotating hint or device-flow code, never a bearer credential, and trust-on-first-use is prohibited;
- **M13b authorization:** member/device grants from the authentication programme and authenticated capability discovery;
- **M13c read projection:** explicitly shared metadata projections, Discover rails, Collections, and aggregate household views, with profile-private state denied by default;
- **M13d mutations and deltas:** server-owned revisions, authorized mutations, snapshot/delta, revocation, audit, and authority-bound streaming; browser streams use authenticated fetch or an exact same-origin cookie and never URL tokens; each stream and resume cursor is bound to workspace, profile, client, capability, session/auth/grant/credential epochs, restore generation, and expiry, rechecked before every event or bounded heartbeat, and closed on any invalidation;
- **M13e offline and release evidence:** bounded offline cache, reconnect, authenticated encrypted backup, restore-generation quarantine, diagnostics, browser, Desktop, OCI, accessibility, performance, security, and exact-head conformance; this slice is blocked on the authentication programme's `C3-CRYPTO` gate.

The local shared workspace is not a player, tracker, social feed, or database replica. Discovery is not authentication. Every read and mutation still passes Fasti application authorization. MQTT, WebTransport, and a second database are not required; use the existing HTTPS contract and SSE only where a durable event stream is actually needed.

### PR M12 — Integrated hardening and final programme gate

M12 is numbered before M13 for stable programme references but merges after every capability slice, including M13. It contains no new product capability.

- full contract parity;
- security;
- performance;
- offline;
- native;
- OCI;
- Tauri;
- accessibility;
- backup;
- restore;
- real-provider and real-client acceptance;
- postmortem.

Each PR updates its applicable contracts, tests, documentation, rollback, and exact-head evidence.

## 29. Required workflow

Run:

```text
/sync-gbrain
/context-restore
/investigate
/office-hours
/autoplan
/plan-ceo-review
/plan-eng-review
/plan-devex-review
/plan-design-review
/cso
@ponytail full
@ponytail-review
```

Planning is a gate.

Do not implement production code before the final plan review is approved.

After approval, use:

```text
/review
/cso
/qa
/design-review
/impeccable polish
/devex-review
/ship
/retro
/context-save
```

Use Context7 before any external library API.

Current source and pinned source win over memory.

Do not guess.

## 30. Definition of done

The programme is complete only when:

- Search is multi-source and local-first;
- Discover uses governed rails;
- Library is server-paginated and provider-independent;
- Collections use governed source bindings;
- hard-coded Svelte packs are gone;
- provider credential states are correct;
- secrets remain outside browser and logs;
- metadata is field-level and provenance-aware;
- TMDB uses verified aliases;
- MDBList ratings are separate claims;
- Trakt is optional;
- anime preference changes projection only;
- Nuvio can consume selected Fasti catalogs and metadata;
- Nuvio pairing and state sync pass when that phase is active;
- direct official Nuvio Cloud profile, library, watched, and progress synchronization passes end to end;
- Fasti provider synchronization passes against both pinned Nuvio clients with durable offline replay;
- MDBList ratings, catalogs, watchlist, watched, collection, dropped, and scrobble capabilities pass as independent grants;
- every Search candidate and Record has a canonical, keyboard-accessible details route;
- the Local Shared Media Workspace passes explicit sharing, discovery privacy, member isolation, revocation, offline, backup, and restore gates;
- OpenAPI, AsyncAPI, schemas, SDK, CLI, errors, and docs agree;
- the dual-licensed provider interoperability contract passes scope/provenance checks and is consumed by both upstream clients without copying Fasti implementation or SDK code;
- real credentialed TMDB, MDBList, and Nuvio Cloud acceptance passes with redacted receipts, and both pinned Nuvio clients pass against a real durable Fasti process;
- the exact pinned Kaptain source passes the lossless temporary-checkout acceptance with only its digest/count/classification receipt retained;
- security negative controls pass;
- offline behavior passes;
- performance and memory pass;
- WCAG 2.2 AA evidence passes;
- EN 301 549 evidence and limits are recorded;
- every implementation PR is merged to `dev`;
- `dev` is verified;
- a clean supported developer environment reaches a deterministic provider-contract pass in three documented steps and no more than five minutes with a warm locked toolchain cache;
- every integration example and copied command is executed in CI, and the focused integration check completes within 60 seconds warm without network access or credentials;
- integration errors pass the human and `--output json` golden fixtures with no secret, query text, personal title, or token disclosure;
- `/retro` and `/context-save` are complete.

## 31. Primary source pack

### Fasti

- https://github.com/Scrobble-dev/Fasti
- https://github.com/Scrobble-dev/Fasti/blob/dev/packages/ui/src/runtime-settings-view.svelte
- https://github.com/Scrobble-dev/Fasti/blob/dev/packages/ui/src/discover-view.svelte
- https://github.com/Scrobble-dev/Fasti/blob/dev/packages/ui/src/library-view.svelte
- https://github.com/Scrobble-dev/Fasti/blob/dev/apps/desktop/src-tauri/src/providers.rs
- https://github.com/Scrobble-dev/Fasti/blob/dev/docs/integrations/nuvio.md
- https://github.com/Scrobble-dev/Fasti/blob/dev/docs/capability-ledger.md

### Nuvio Desktop pinned source

- Revision: `ab498c9378aebf1a81cff104b3069eb6ac7701dc`.
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/settings/IntegrationsSettingsPage.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/mdblist/MdbListSettings.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/mdblist/MdbListMetadataService.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/details/MetaDetailsRepository.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/tmdb/TmdbMetadataService.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/tmdb/TmdbService.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/settings/TrackingSettingsPage.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/simkl/SimklProjections.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/collection/CollectionModels.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonTest/kotlin/com/nuvio/app/features/collection/CollectionSourceSerializationTest.kt

### Nuvio issues

- NuvioTV audited revision: `7c3baa16e491aeec5ee017dd867a271568ecfba3`.
- https://github.com/NuvioMedia/NuvioTV/issues/1281
- https://github.com/NuvioMedia/NuvioTV/issues/2484
- https://github.com/NuvioMedia/NuvioTV/issues/2531
- https://github.com/NuvioMedia/NuvioTV/issues/2742
- https://github.com/NuvioMedia/NuvioTV/issues/2935
- https://github.com/NuvioMedia/NuvioTV/issues/2967

### Kaptain Collection

- Audited revision: `fdb7a91e545f18f8a67aab49d4742b217fc02e2c`.
- https://github.com/ImKaptain/Kaptain-Collection
- https://github.com/ImKaptain/Kaptain-Collection/issues/9
- https://github.com/ImKaptain/Kaptain-Collection/blob/main/ROADMAP.md

### Provider documentation

- https://developer.themoviedb.org/docs/authentication-application
- https://developer.themoviedb.org/docs/finding-data
- https://developer.themoviedb.org/reference/find-by-id
- https://developer.themoviedb.org/reference/search-multi
- https://developer.themoviedb.org/reference/discover-movie
- https://developer.themoviedb.org/reference/discover-tv
- https://docs.mdblist.com/docs/api
- https://api.mdblist.com/docs/
- https://openlibrary.org/developers/api
- https://kitsu.docs.apiary.io/
- https://docs.anilist.co/
- https://musicbrainz.org/doc/MusicBrainz_API
- https://thetvdb.github.io/v4-api/
- https://developers.google.com/books/docs/v1/using

### Stremio compatibility

- Audited protocol revision: `2728da3ee853207cd5ee200aabe15a08cc1d01d1`.
- https://github.com/Stremio/stremio-addon-sdk/blob/master/docs/protocol.md
- https://github.com/Stremio/stremio-addon-sdk/blob/master/docs/api/responses/manifest.md

### Nuvio Cloud synchronization benchmark

- https://github.com/ellite/scrob/tree/1c4d775b70f489ca0531376b2c3de6a8c3de2a2b
- https://github.com/ellite/scrob/blob/1c4d775b70f489ca0531376b2c3de6a8c3de2a2b/backend/core/nuvio.py
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/library/sync/SupabaseLibrarySyncAdapter.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/watching/sync/SupabaseWatchedSyncAdapter.kt
- https://github.com/NuvioMedia/NuvioDesktop/blob/ab498c9378aebf1a81cff104b3069eb6ac7701dc/composeApp/src/commonMain/kotlin/com/nuvio/app/features/watching/sync/SupabaseProgressSyncAdapter.kt

### Project context

- https://github.com/ryan-winkler/gstack-artifacts-winks
- https://github.com/ryan-winkler/gstack-artifacts-winks/blob/main/projects/Scrobble-dev-Fasti/checkpoints/20260826-182700-fasti-full-context-and-agent-handoff.md

### Developer experience and interoperability licensing

- https://stremio.github.io/stremio-addon-sdk/
- https://stremio.github.io/stremio-addon-guide/sdk-guide/step1
- https://supabase.com/docs/guides/local-development/cli/getting-started
- https://spdx.org/licenses/
- https://www.gnu.org/licenses/quick-guide-gplv3.en.html

## 32. CEO review system audit

The review used live source and exact references rather than the dated plan as proof.

| Evidence | Observed result |
| --- | --- |
| Worktree | Isolated `/home/ryan/code/fasti-nuvio-metadata-programme-m0`; branch `codex/nuvio-metadata-programme-m0`. |
| Exact base | `origin/dev@adbdef3038786b0efb2ec615bce080e3eaa9361f`. |
| Production changes in M0 | None. Only planning and design artifacts are allowed. |
| Root checkout | Separately owned and conflicted; never used for writes. |
| Provider runtime | Real Google Books and TMDB only in trusted Tauri; browser shows honest unavailable states. |
| Search and Discover | Local filtering and bounded candidate discovery exist; no shared multi-source composition root. |
| Library | Profile state exists, but the UI materializes a bounded set and lacks the required server query. |
| Collections | Lossless profile-scoped Nuvio JSON interchange exists; source-neutral Collection entities do not. |
| Nuvio | One real occurrence ingress plus process-local conformance models; no durable paired transport. |
| Nuvio Cloud | Pinned Nuvio clients expose profile-scoped snapshot/delta/upsert/delete RPCs; Scrob independently proves password exchange, rotating refresh-token handling, and bidirectional use. |
| Authentication | PR #93 is merged; this programme consumes the approved Access plan and does not edit its canonical document. |

CEO mode is **HOLD SCOPE**. The plan already describes a complete product programme. The review restores details routes, complete MDBList account synchronization, direct Nuvio Cloud synchronization, and the Local Shared Media Workspace because they were present in the mission but under-specified in the baseline. These are omission fixes, not optional expansions.

The user pre-authorized every recommended review answer. Every finding below therefore records the recommended complete option as selected. There are no deferred TODOs and no unresolved product decisions.

## 33. What already exists

| Existing surface | Reuse decision |
| --- | --- |
| Stable Record IDs and typed identifier assertions | Reuse unchanged as the only identity owner. |
| `fasti_application::provider_identity_mapping` | Reuse for every Google Books/TMDB coordinate and add purpose-specific routes there. |
| Metadata claims, overrides, and SQLite repositories | Extend with lifecycle, profile projection, provenance, and cache policy; do not create a second metadata store. |
| Governed provider egress policy | Reuse resolve-once, all-address authorization, pinned proxy-free clients, redirect denial, limits, and secret-after-authorization order. |
| Data-root-scoped platform credential storage | Reuse as the only provider and rotating-token vault. |
| Profile tracking disposition | Extend into independent saved, progress, watched, rating, note, and membership owners. |
| Nuvio Collection import/export | Preserve the exact bare-array contract, raw envelope, bounds, and unknown fields while adding normalized Collection entities. |
| Nuvio occurrence ingress | Preserve its stable source-event identity and receipt path; route new client observations through the same canonical operation boundary. |
| `NuvioOutbox`, state-delta, and catalog conformance models | Use as behavioral fixtures only; replace process-local storage at production composition with durable journal/outbox repositories. |
| Capability registry, generated OpenAPI/Schema/SDK/CLI | Extend from authored sources; never hand-edit generated outputs. |
| `contracts/addons/manifests/google-books.provider.yaml` | Make it the first validated provider-authoring example; add schema-owned deterministic fixtures instead of inventing another manifest shape. |
| `cargo xtask` | Extend with one focused `integration check` command; retain the existing generation, locked verification, PR, deep, and milestone commands. |
| Workbench and Tabler shell | Mature in place. Do not replace navigation, settings, or details surfaces with a second UI. |
| Archive v2 | Advance through immutable incremental versions: v3 projection policy/overrides, v4 Library, v5 Collections/pack receipts, v6 connections/journals/tombstones/acknowledgements, and v7 local-share policy. Preserve every prior prefix and v1/v2 compatibility. |

## 34. NOT in scope

No requested capability is deferred. These items are constitutional non-goals, prohibited data uses, or separate release authority:

- Playback, decoding, transcoding, stream selection, or stream publication: players own playback.
- Direct Nuvio, TrailBase, or provider database access: integrations use documented authenticated contracts.
- Nuvio service-role credentials or persisted Nuvio passwords: only publishable client configuration plus vault-held rotating refresh tokens are permitted.
- A public mirror of account-scoped MDBList responses: account data remains private and purpose-scoped.
- Bundling Kaptain content or assets without a licence: compatibility is implemented against the format and user-supplied documents.
- Provider-owned canonical identity: all external identifiers remain evidence.
- Public package or release publication: this programme lands and verifies `dev`; publication still requires the repository's explicit B8 release action.
- New databases, brokers, plugin frameworks, or telemetry systems: the existing SQLite writer, HTTPS/SSE contracts, and local diagnostics cover the requested behavior. The missing SQLite journal/outbox schema is implemented as the shared M11a substrate, not asserted as existing infrastructure.

## 35. Ownership, archive, and migration lock

| State | Owner | Isolation and archive rule |
| --- | --- | --- |
| Record and external identity evidence | Workspace | Shared local truth; preserved in every additive archive version. |
| Provider metadata and public aggregate claims | Workspace | Partitioned by safe provider configuration and terms revision. |
| Projection policy and user overrides | Profile | Never changes shared evidence or another profile's projection. |
| Provider application credential | Physical node/data root | Vault-only; archive carries reconnect metadata, never the secret. |
| Provider account grant | Connection plus profile grant | Each capability and direction is independently authorized. |
| Saved, progress, watched, history, rating, and note state | Profile, separately | No state is inferred from another. |
| Collection and ordered membership | Profile | Local and provider-independent. |
| Collection source binding | Profile Collection | Live reference until explicitly materialized. |
| Pack descriptor | Immutable registry/cache | Version, digest, source, author, licence, requirements, rollback. |
| Pack receipt | Profile | Preview, merge/replace, rollback, and exact source digest. |
| Nuvio raw envelope | Profile | Lossless original plus extension bag; normalized state is separate. |
| Nuvio Cloud cursor/token generation | Connection | Cursor per state lane; token in vault; generation in SQLite. |
| Fasti synchronization journal/receipt | Profile, connection, lane, and client as applicable | Immutable and idempotent; compactable delta payloads use the 30-day window, while unacknowledged operations, required tombstones, state, and idempotency receipts follow their durable acknowledgement/retention rules. |
| Local workspace share | Workspace with explicit profile grants | Private by default; archive includes policy and grants without credentials. |

Archive versions are additive and immutable: v3 adds projection policy and profile overrides; v4 adds independent Library state; v5 adds Collections and pack receipts; v6 adds raw Nuvio envelopes, connections without secrets, cursors, tombstones, journals, and acknowledgements; v7 adds local-share policy and grants. Archive v3-v7 and M13e are blocked on the authentication programme's `C3-CRYPTO` gate. Each version preserves the exact stream prefix and order of every preceding version inside one authenticated encrypted joint manifest. Restore and rollback fixtures cover v1, v2, and each new version with missing, extra, reordered, and unknown stream failures. Cursors, receipts, grants, descriptors, journals, and idempotency scopes bind to the global restore generation. Restore reconnects a credential reference only when the destination vault proves the same data-root identity, and restores all connections, publication descriptors, sharing, and dispatchers quarantined and disabled. Reactivation requires recent authentication, a reviewed authority check, a fresh remote snapshot, and conflict reconciliation; no restored operation can execute under an earlier generation.

The legacy `metadata_field_overrides` table has no profile owner. M2 inspects the populated v10 root before changing it. With zero profiles, the value remains in a non-editable migration-review owner. With exactly one unambiguous eligible profile, it migrates to that profile with a receipt. With multiple or ambiguous profiles, it remains losslessly retained in the migration-review state until an authorized owner selects one destination. Fasti never copies an override to every profile and never discards it. Interrupted migration is idempotent and preserves unrelated rows byte-for-byte.

Every storage slice allocates its migration only after fetching current `dev` and the authentication migration ledger. One writer owns each migration. Network calls never occur inside the SQLite writer transaction. Before migration, Fasti verifies a compatible backup receipt. Rollback restores the prior application and compatible backup; no destructive down migration is used.

## 36. Architecture review

### 36.1 Full system architecture

```text
 Browser Workbench / Tauri Host / CLI / Nuvio Desktop / NuvioTV
                              |
                     generated SDK/contracts
                              |
                 HTTP + SSE / Tauri IPC adapters
                              |
             +----------------+----------------+
             | Fasti application capabilities |
             +----+--------+--------+----------+
                  |        |        |
       Identity --+   Metadata      Profile state
          |            |             |
          |       Claims/Projection  +-- Library query
          |            |             +-- Collections/packs
          |            |             +-- Journal/delta/tombstones
          +------------+-------------+
                       |
               one bounded SQLite writer
                       |
         SQLite + content-addressed evidence + vault refs
                       |
       +---------------+----------------------------+
       | governed outbound provider policy          |
       +--------+----------+-----------+-------------+
                |          |           |
              TMDB      MDBList    Nuvio Cloud
                                      |
                    official auth + public RPCs

 Fasti read publication: Stremio manifest/catalog/meta (no streams, no private state)
 Fasti state publication: versioned provider contract (scoped Nuvio client grants)
 Local Shared Workspace: same application capabilities, explicit member/device grants
```

The dependency direction remains Domain <- Application <- Contracts <- Adapters. Provider adapters translate only. No adapter decides identity, merge, deletion, projection precedence, or conflict policy.

### 36.2 Data paths and shadow paths

```text
 query/input -> bounds -> authorize -> resolve route -> fetch -> validate
     |            |          |              |            |        |
   nil/empty   too large   denied       ambiguous     timeout   malformed
     |            |          |              |            |        |
 typed error  typed error  no work      review item   stale LKG typed error
                              |
                           transform -> short transaction -> receipt -> projection
                               |              |              |          |
                           unknown ID     conflict/full   retry-safe   stale/partial
                               |              |              |          |
                           unresolved      no mutation     replay      visible state
```

Happy path commits the local state and receipt, then schedules optional network work. Nil and empty inputs produce stable typed problems or a valid empty page according to the contract. Upstream errors preserve local truth and last-known-good claims. An empty remote page never causes deletion. A local persistence failure never returns remote or UI success.

### 36.3 Stateful objects

```text
Provider credential:
 missing -> stored_unverified -> valid -> expired/invalid/revoked
    ^               |              |             |
    +---- replace --+--------------+-------------+

Metadata claim:
 fresh -> stale -> superseded
   |        |          |
   +------ revoked/invalid

Sync connection:
 unconfigured -> authorizing -> active -> degraded -> reauthorization_required
                       |          |          |                 |
                       +----------+-- revoke -> revoked <------+

Pack install:
 uploaded -> validated -> previewed -> applied -> rolled_back
              |             |            |
            rejected      cancelled     conflict_receipt

Fasti provider mutation:
 received -> authorized -> revision_checked -> committed -> acknowledged
    |            |                |               |
 rejected     denied          conflict          replayed
```

Invalid transitions fail closed. A revoked credential cannot return a prior receipt. A stale revision cannot overwrite current state. A revoked claim cannot become fresh without a new claim. A pack cannot apply without a preview receipt and explicit mode.

### 36.4 Scaling and single points

- At 10x load, provider rate limits and SQLite writer contention fail first. Bounded fan-out, keyset pages, per-connection serialization, and short transactions contain them.
- At 100x load, the single-node product reaches its declared hardware envelope. Requests reject with retry guidance before unbounded queueing; the plan does not add a distributed database to hide a single-node limit.
- The SQLite writer is intentionally singular and protected by durable backup/restore.
- A provider outage is not a local availability outage.
- A spent rotating token affects one Connection only and moves it to reauthorization, not global failure.

Architecture review disposition: **3 issues found and resolved** — split the two Nuvio directions, add direct Nuvio Cloud sync, and restore the Local Shared Workspace to the roadmap.

## 37. Error and rescue registry

| Method or codepath | Failure class | Rescued | Rescue action | User impact |
| --- | --- | --- | --- | --- |
| Provider route authorization | `UnsafeProviderAddress` | Yes | Reject before credential access; record safe diagnostic. | Source blocked with exact policy reason. |
| Provider DNS resolution | `ProviderResolutionFailed` | Yes | No request; bounded retry. | Local results remain; source unavailable. |
| Provider HTTP request | `ProviderTimeout` | Yes | Bounded retry when idempotent; stale-on-error. | Partial results or last-known-good. |
| Provider HTTP request | `ProviderRateLimited` | Yes | Honor bounded retry time; stop fan-out. | Retry time is persistent. |
| Provider response decode | `ProviderResponseInvalid` | Yes | Reject response; retain prior state. | Provider error; no data loss. |
| Search validation | `SearchQueryInvalid` | Yes | Reject before fan-out. | Field error and allowed limits. |
| Candidate re-fetch | `SearchCandidateExpired` | Yes | Re-run exact source query. | Details page offers refresh. |
| Candidate action | `IdentityRouteAmbiguous` | Yes | Create review item; no attach. | Resolve later or inspect evidence. |
| Record transaction | `StorageUnavailable` | Yes | Roll back; return no success receipt. | Persistent retry/recovery guidance. |
| Claim refresh | `MetadataClaimStale` | Yes | Retain and label last-known-good. | Stale provenance remains visible. |
| Explicit retraction | `MetadataClaimRevoked` | Yes | Append lifecycle event and reproject. | Field changes with source explanation. |
| MDBList request | `ProviderCredentialInvalid` | Yes | Disable only the failed grant; require replacement. | Other capabilities remain active. |
| MDBList batch | `PartialBatchFailed` | Yes | Retain per-batch receipts and retry failed subset. | Exact completed/failed counts. |
| Collection parse | `CollectionPackInvalid` | Yes | Reject before persistence. | Line/path and schema problem. |
| Collection source activation | `CollectionSourceQuarantined` | Yes | Keep imported URL inert; require safe-origin validation and an explicit grant. | Source remains disabled without losing the envelope. |
| Collection bounds | `CollectionPackLimitExceeded` | Yes | Abort bounded parser and delete staging. | Exact exceeded limit. |
| Collection apply | `CollectionPackConflict` | Yes | No partial activation; produce conflict preview. | Merge/replace choice remains safe. |
| Nuvio sign-in | `NuvioAuthenticationFailed` | Yes | Discard password and any incomplete session. | Credentials not saved; retry guidance. |
| Nuvio refresh | `NuvioRefreshTokenSpent` | Yes | Stop connection; require reauthorization. | Other connections unaffected. |
| Nuvio profile lookup | `NuvioProfileMissing` | Yes | Preserve mapping; block sync. | Select a current profile. |
| Nuvio RPC | `NuvioRemoteContractChanged` | Yes | Fail closed and preserve cursor/state. | Compatibility mismatch shown. |
| Nuvio snapshot | `NuvioSnapshotTooLarge` | Yes | Stop at bounds; no apply. | Increase not offered silently. |
| Nuvio delta | `NuvioCursorExpired` | Yes | Named snapshot recovery and journal replay. | Resynchronizing state shown. |
| Nuvio mutation | `NuvioRevisionConflict` | Yes | Keep committed value and both evidence; conflict receipt. | User can resolve or retry. |
| Nuvio mutation | `NuvioIdempotencyConflict` | Yes | Reject changed digest. | No duplicate or overwrite. |
| Synchronization admission | `SyncStorageQuotaExhausted` | Yes | Reject before mutation; expose bounded retirement/quarantine. | Existing state remains writable and recoverable. |
| Nuvio acknowledgement | `NuvioAckPersistenceFailed` | Yes | Do not advance client cursor; replay safely. | Sync remains pending. |
| Public catalog | `PublicationGrantRevoked` | Yes | Remove descriptor and purge partition. | Resource returns typed unavailable. |
| Local workspace discovery | `DiscoveryAdvertisementInvalid` | Yes | Ignore unauthenticated hint. | Manual URL/QR remains available. |
| Local workspace authorization | `WorkspaceShareGrantDenied` | Yes | Deny before lookup. | No cross-profile disclosure. |
| Local workspace stream | `StreamAuthorityExpired` | Yes | Close before emission; require a newly authorized scoped resume. | No events leak under stale authority. |
| Backup/restore | `ArchiveIntegrityFailed` | Yes | Keep active root untouched. | Restore rejected with digest evidence. |
| Post-restore operation | `RestoreGenerationStale` | Yes | Keep connection, publication, sharing, and dispatcher quarantined. | Recent-auth review and reconciliation required. |

No catch-all rescue may convert an unknown failure into success. Unknown failures add operation context, rollback local work, and surface the correlation ID. Secret values, URLs containing credentials, request bodies, and private media data are redacted before logging.

## 38. Failure modes registry

| Codepath | Failure mode | Rescued? | Test? | User sees? | Logged? |
| --- | --- | --- | --- | --- | --- |
| Multi-source Search | One source times out | Yes | Yes | Partial results and failed source | Safe structured event |
| Record action | Source changed after Search | Yes | Yes | Candidate changed; review again | Receipt ID only |
| Claim refresh | Empty provider response | Yes | Yes | Prior claim remains | Provider/status only |
| Library query | Cursor reused with changed filters | Yes | Yes | Restart page | Safe query digest |
| Discover refresh | Cache stale and provider down | Yes | Yes | Stale label and retry | Rail ID/status |
| Pack import | Deep/oversized hostile JSON | Yes | Yes | Limit problem | Digest/limit only |
| Pack apply | Process exits mid-transaction | Yes | Yes | Prior state remains | Recovery receipt |
| MDBList sync | Capability budget exhausted | Yes | Yes | Retry time and retained state | Grant ID only |
| Nuvio token refresh | Two operations race | Yes | Yes | One serialized operation | Connection ID only |
| Nuvio token refresh | Crash after remote rotation | Yes | Yes | Reauthorization only if durable commit cannot be proven | Generation receipt |
| Nuvio delta | Duplicate/reordered events | Yes | Yes | No duplicate visible | Sequence counters |
| Nuvio delete | Old delete arrives after recreate | Yes | Yes | Conflict receipt | Revisions only |
| Nuvio client | Offline journal/outbox restart | Yes | Yes | Pending count | Local diagnostics |
| Public catalog | Private state selected accidentally | Yes | Yes | Publication rejected | Descriptor ID |
| Shared workspace | Revoked device retries | Yes | Yes | Access removed | Audit receipt |
| Migration | Interrupted upgrade | Yes | Yes | Safe recovery mode | Migration/backup receipt |
| Restore | Wrong vault/data-root identity | Yes | Yes | Reconnect required | Identity digest only |

There are **0 critical gaps**: no row is unrescued, untested, and silent.

## 39. Security and threat model review

| Threat | Likelihood | Impact | Required mitigation |
| --- | --- | --- | --- |
| Provider or account secret in URL/log/cache | Medium | High | Headers/vault only; final URL and cache negative controls. |
| SSRF, DNS rebinding, redirect, or proxy escape | High | High | Resolve once, authorize every address, pin, no redirects/system proxy, credential loaded last. |
| Cross-profile credential or state access | Medium | High | Workspace/profile/connection/grant authorization before lookup and again before commit. |
| Rotating Nuvio token race | High | High | Durable per-connection lease and token-generation transaction. |
| Malicious pack/add-on response | High | High | Streaming bounds, depth/node/string caps, schemas, inert imported URLs, separate activation authorization. |
| Candidate title collision attaches wrong identity | Medium | High | Exact route re-fetch; title similarity cannot authorize attach. |
| Public catalog leaks private Library data | Medium | High | Explicit publication descriptor, field allowlist, cache partition, negative fixtures. |
| Idempotency-key replay with stale or changed authority | Medium | High | Server-derived workspace/profile/client/credential/grant/capability/object/lane/revision/digest/restore scope; reject without prior receipt disclosure. |
| Cursor tampering, long-lived stream leakage, or cross-grant reuse | Medium | High | Integrity-bound scope/epoch digest, browser-safe authentication, bounded recheck, close on every invalidation. |
| Remote image tracking | Medium | Medium | Governed proxy/cache, content-type/size validation, no credential forwarding. |
| MDBList account data redistribution | Low | High | Account/purpose cache partitions; deny public projection. |
| Unlicensed Kaptain content copied into Fasti | Low | High | Synthetic fixtures and user-supplied ignored file only. |
| Local discovery exposes household media | Medium | Medium | Rotating random hint only; no profile/title/history data; discovery is unauthenticated. |
| Journal/receipt storage exhaustion | Medium | High | Admission quotas by workspace/profile/client/lane, receipt compaction, bounded sweeper, quarantine/revoke. |
| Stale restored authority executes | Medium | High | `C3-CRYPTO`, authenticated encrypted joint manifest, restore-generation binding, disabled/quarantined reactivation. |

Security review disposition: **7 issues found and resolved**, including token rotation, direct Nuvio Cloud trust, provider-contract authorization, private publication, import limits, account-data segregation, and discovery privacy. Formal `/cso` review remains a required planning gate and implementation gate.

## 40. Data and interaction edge cases

| Interaction | Edge case | Handling |
| --- | --- | --- |
| Search submit | Empty, over 256 bytes, double submit | Field error; cancellation and one active query generation. |
| Search navigation | Candidate receipt expired | Same route shows refresh; no Record created. |
| Details action | User navigates away during fetch | Abort network work; no transaction started. |
| Record create/attach | Duplicate click or retry | Operation idempotency replays one receipt. |
| Library page | Zero or 10,000+ results | Purposeful empty state; bounded keyset pages. |
| Library page | Rows change mid-page | Stable tuple cursor; new changes appear on refresh, not under focus. |
| Claim refresh | Provider removes one field | Requires explicit retraction evidence; empty omission does nothing. |
| Collection import | Duplicate IDs or unknown fields | Duplicate rejected; unknown fields round-trip. |
| Collection import | URL-bearing source or extension | Store only as inert data after safe-shape validation; separate activation reauthorizes current origin and DNS. |
| Collection replace | Process exits | Single transaction; prior document remains. |
| Background refresh | Queue delayed for hours | Stale state remains visible; no local deletion. |
| Nuvio sync | 3 of 10 batches fail | Per-batch receipt; retry only failed batches. |
| Nuvio profile removed | Existing cursor remains | Block connection and request explicit profile selection. |
| MDBList push | One grant disabled | Other grants continue; no implied permission. |
| Local share | Member removed while reading | Epoch/revocation recheck denies next page and receipt lookup. |
| Local share stream | Any authority epoch or restore generation changes | Close before the next event or bounded heartbeat; scoped resume is rejected. |
| Sync admission | Durable quota reached | Reject before mutation; retain writable existing state and offer reviewed retirement/quarantine. |

All input paths specify nil, empty, wrong type, length, Unicode, concurrency, stale, duplicate, timeout, and partial-result behavior. Section 4 produced **14 edge cases and 0 unhandled cases**.

## 41. Code quality, DRY, DDD, and Ponytail review

- Add no runtime framework, database, broker, cache server, or alternate vault.
- Reuse the existing provider identity mapping, egress policy, typed problem catalogue, capability registry, SQLite writer, receipt/idempotency model, archive framework, SDK generator, Tabler shell, and design tokens.
- Keep bounded contexts explicit: Identity, Metadata, Search, Discover, Library, Collections, Connections, Synchronization, and Access.
- Share application commands and state-transition rules between HTTP and Tauri; adapters must not duplicate business policy.
- Use one generic governed HTTP transport, but keep provider-specific request/response translation concrete. Do not create an interface with one implementation or a provider DSL before two real adapters need it.
- Use SQLite constraints and transactions for uniqueness, sequence, compare-and-set revision, receipt, and tombstone invariants.
- Keep functions at one responsibility. A parser, transport call, mapper, and transaction are separate because their error and trust boundaries differ, not for speculative abstraction.
- Every non-trivial parser, branch, loop, and security path gets the smallest runnable regression check that proves its invariant.

Code-quality review disposition: **4 issues found and resolved** — duplicate provider policy, adapter-owned domain rules, speculative transport infrastructure, and full-library materialization.

## 42. Test topology

Detected test owners at the M0 base are Rust `#[test]`/Tokio unit and integration tests, Node's built-in test runner, Playwright, and Axe through Playwright. Preserve those owners; add no test framework. The current Playwright server uses `tests/e2e/health-stub.mjs`, which is adequate only for existing shell truth. Programme E2E tests must start a disposable real `fastid` with a temporary SQLite data root and supported account/session bootstrap, plus deterministic loopback provider servers. A mocked UI response, health-only stub, in-memory conformance model, or successful HTTP status cannot prove durable Search, Library, Collection, connection, or sharing behavior.

```text
NEW UX FLOWS
  Search -> candidate details -> Record action
  Metadata provenance and provider preferences
  Library filters/pages/details
  Discover rails and stale states
  Collection preview/apply/rollback
  MDBList connection and independent grants
  Nuvio Cloud connection/profile/sync/reconcile
  Fasti provider pairing/conflict/recovery
  Local workspace share/connect/revoke

NEW DATA FLOWS
  provider -> claims -> projection
  candidate -> exact re-fetch -> Record transaction
  account state -> connection grant -> profile state
  profile state -> journal/delta -> Nuvio client
  profile state -> approved read projection -> Stremio/local share

NEW ASYNC WORK
  bounded provider fan-out and refresh
  durable sync dispatch and retry
  cache eviction and publication purge
  pack validation/apply receipts

NEW EXTERNAL CALLS
  TMDB, MDBList, Nuvio Cloud, approved add-on origins

NEW FAILURE PATHS
  every typed row in Sections 37 and 38
```

Unit tests own pure normalization, routing, projection, cursor, revision, tombstone, and parser rules. Store integration tests own migration, transaction, restart, contention, receipt, journal, and archive behavior. Provider integration tests use deterministic local transports with exact request/response fixtures. System tests compose real Fasti processes and generated clients. A small E2E set covers each critical user journey. Live provider checks are separate operator-owned acceptance and never the deterministic CI oracle.

Deterministic CI and live acceptance are both mandatory and prove different things. Before a provider or compatibility profile is reported complete, redacted operator-owned acceptance uses real credentials/accounts for TMDB, MDBList, and Nuvio Cloud against the exact approved endpoints, and pinned Nuvio Desktop/TV builds connect to one real local Fasti process. Receipts name source revision, endpoint origin without secrets, contract revision, fixture/data classification, command, time, and result. Live latency does not decide product overhead, and live failures never delete local data; missing live authority blocks the corresponding `working end to end` claim.

The Friday-at-2am gate is a clean isolated data root that completes Search, creates a Record, refreshes claims, queries Library, imports/rolls back a pack, synchronizes Nuvio Cloud both ways, replays an offline Nuvio client journal, revokes it, shares a workspace projection, backs up, restores, and proves semantic equality under the memory ceiling.

The hostile QA gate mutates every scope, cursor, revision, digest, origin, profile, page, pack bound, redirect, DNS answer, token generation, and archive reference. The chaos gate kills the process at each durability boundary and repeats provider timeouts, 429s, malformed JSON, out-of-order delta, duplicate delivery, disk-full, and cursor expiry.

Exact new test owners include:

- `crates/fasti-store/tests/sync_journal_restart.rs`: kill after local mutation, journal insert, remote success, receipt insert, acknowledgement, and cursor advance; rebuild outbox and prove no loss or duplicate mutation;
- `crates/fasti-store/tests/provider_state_migrations.rs`: populated v10 roots; zero/one/multiple-profile override migration; interrupted retry; unrelated-row equality;
- `crates/fasti-store/tests/archive_version_compatibility.rs`: v1/v2 and every new version; exact prefix/order; missing, extra, reordered, and unknown streams; rollback semantic equality;
- `crates/fasti-application/tests/sync_state_machine.rs`: delete-versus-recreate, progress reset, conflict, digest mismatch, cursor expiry, compaction, and unacknowledged retention;
- `crates/fasti-api/tests/provider_contract_e2e.rs`: scope/profile/client isolation; snapshot/delta/mutate/delete/ack; stale revision; replay; cursor recovery; and revocation including receipt disclosure;
- provider-runtime transport tests: authorize every DNS answer, deny redirects, ignore proxy environment, bound response/decompression, and load credentials only after route authorization;
- Nuvio Cloud tests: every rotating-token fault boundary, lease expiry/fencing, concurrent refresh, remote-success/local-crash reconciliation, empty page, explicit delete, reordered delta, cursor expiry, removed profile, and partial batches;
- native Desktop/TV tests: mutation+journal atomicity, restart rebuild, acknowledgement/cursor atomicity, revocation, version overlap, and playback independence offline;
- performance receipt mutation tests for all four memory caps on x86_64 and aarch64;
- old-binary/new-schema overlap tests that exercise writes as well as reads before any overlap is approved.

Test review disposition: **17 gaps found and resolved** by adding provider-runtime composition, durable Fasti synchronization, rotating-token crash/race, direct-cloud delta/delete, canonical details-route, candidate-receipt lifecycle, independent MDBList grants, local-share revocation, incremental archive equality, legacy-override ownership, cross-version writes, and executable resource evidence.

## 43. Performance plan

| Path | Target and bound |
| --- | --- |
| Local Search, Library, Collection page | p95 below 250 ms at 10,000 Records; bounded page 100. |
| Cached catalog/meta | p95 first byte below 500 ms. |
| Provider orchestration | Under 100 ms Fasti overhead excluding upstream latency; at most four concurrent sources. |
| Nuvio/MDBList batch | At most 500 items or provider's smaller current limit; no per-item task explosion. |
| Pack import | 4 MiB, 250,000 nodes, depth 32, 16,384 sources, streaming/bounded staging. |
| Memory | `fastid` process tree: 64 MiB idle, 96 MiB normal, 160 MiB heavy, 192 MiB absolute. Tauri/WebView/browser processes are measured separately against their own recorded baseline and are not hidden inside this daemon budget. |

The idle workload runs the configured production process with its real routes and settled background workers for the canonical idle-settle interval and must remain at or below 64 MiB. The normal workload uses 10,000 Records, five requests per second, ten Collections per profile, local Search/Library/Collection/projection reads, and one bounded refresh for 15 minutes and must remain at or below 96 MiB. The heavy workload runs four provider searches, a 100-result projection, a 4 MiB/3,059-source dry run, a 100-item Nuvio page, metadata refresh, and cache eviction concurrently for 15 minutes and must remain at or below 160 MiB. Every sample in every workload must also remain below the 192 MiB absolute `fastid` process-tree ceiling. Five process-isolated repetitions report p50/p95/p99/max. OOM, swap, missed samples, early exit, workload drift, failure to remain alive through the sampling interval, or any workload-specific/absolute ceiling breach fails.

The existing harness is extended rather than replaced: `scripts/bench-daemon-normal.sh` and `scripts/bench-daemon-heavy.sh` drive new `canonical-normal` and `canonical-heavy` profiles in `bench-envelope.sh`; the existing canonical idle owner remains. Receipt verification checks workload digest, 15-minute duration where applicable, five isolated runs, complete samples, process-tree membership, zero swap/OOM, workload target, and the 192 MiB ceiling. Mutation tests change workload, target, duration, sample count, and process-tree identity and must fail. Tauri/WebView/browser evidence records a separate baseline and regression budget before a UI performance claim; it cannot consume the daemon allowance.

M5/M7 add only the compound SQLite indexes justified by the final keyset predicates and stable sort. Query-plan fixtures use `EXPLAIN QUERY PLAN` plus counted repository calls at 0, 100, and 10,000 Records to reject full scans on the bounded hot paths, per-row metadata/provider lookups, and N+1 Collection membership reads. Indexes that do not serve a measured query are not added.

Background work uses the durable journal/receipt tables as its queue and reads bounded pages ordered by `next_attempt_at` plus stable ID. Network work occurs before any SQLite writer transaction. At most four provider reads, one operation per rotating-token Connection, and the provider-documented smaller batch limit run concurrently. Interactive local reads and state writes are admitted independently of remote waits; when the bounded background admission limit is full, background work stays durable and deferred instead of spawning tasks or blocking the foreground path.

Performance review disposition: **3 issues found and resolved** — explicit provider/batch concurrency, workload-specific memory gates, and deterministic latency fixtures.

## 44. Observability and operations

Fasti retains zero runtime telemetry. All observability is local and operator-controlled.

Each operation records a correlation ID, capability ID, safe actor/profile/connection identifiers, state transition, attempt, duration, item counts, cache result, provider status, cursor sequence, receipt ID, and typed problem. It never records secrets, password fields, raw tokens, private response bodies, provider URLs containing credentials, or media history beyond the minimum safe local diagnostic reference.

Local metrics expose bounded counters and histograms for provider calls, cache outcomes, queue depth, oldest pending operation, delta lag, conflict count, token generation, reauthorization count, pack validation, publication, and share revocation. Health panels answer whether each capability is configured, authorized, fresh, degraded, stale, rate-limited, or blocked and show the exact next action.

Runbooks cover credential replacement, Nuvio spent token, provider contract mismatch, cursor recovery, stuck outbox, pack rollback, cache purge, storage full, migration recovery, share revocation, backup, and restore. Diagnostics export redacts by construction and has a negative fixture for every secret class.

Observability review disposition: **4 gaps found and resolved** — cursor/token generation, per-capability provider health, conflict receipts, and local-share audit visibility.

## 45. Deployment sequence and rollback

```text
M0 approved
  -> M1 shared provider runtime/registry/vault
  -> M2 claims/projection/archive-v3
  -> M3 routing/anime
  -> M4 Search/details
  -> M5 Library
  -> M6 Discover
  -> M7 Collections/packs
  -> M8 TMDB
  -> M9a MDBList public reads
  -> M9b MDBList private account state
  -> M10 catalog/meta publication
  -> M11a Fasti synchronization substrate
  -> M11b Nuvio Cloud bootstrap/snapshot
  -> M11c Nuvio Cloud delta/write/reconcile
  -> M11d Fasti provider v1
  -> M11e Nuvio Desktop client
  -> M11f NuvioTV client
  -> M11g exact-revision cross-client evidence
  -> M13a-e Local Shared Media Workspace
  -> M12 integrated hardening and final programme gate
  -> exact merged-dev verification
```

M8 can branch after M3 and M9a after M4. M9b requires the M11a shared synchronization substrate, so M11a may land immediately after M4 without activating a remote lane. M11b requires M11a; M11c requires M11b; M11d requires M11a but is independent of the Cloud lane; M11e and M11f require M11d; M11g requires M11d-f. M13a-e are ordered discovery, authorization, reads, mutations/deltas, then offline/release evidence. M8, M9a-b, M10, M11a-g, and M13a-e all rejoin before M12. One writer owns each shared migration, registry, generated contract set, SDK, and Workbench composition slice. Every Fasti PR targets `dev` and starts from its accepted predecessor; upstream Nuvio PRs use their exact pinned repository bases. Authentication-owned files remain outside this programme unless the auth programme has merged and released ownership.

```text
Failure detected
  -> stop new capability dispatch
  -> preserve journals/receipts/tombstones
  -> is schema unchanged or additive-compatible?
       yes -> revert application/adapter and retain data
       no  -> verify pre-migration backup receipt
               -> restore compatible app + data root
  -> run integrity/equality checks
  -> re-enable prior capability set
  -> retain failed-operation diagnostics for retry/review
```

Old and new binaries may overlap only when exact read-and-write tests prove the old binary neither corrupts nor silently drops state in the new additive schema. “Old readers ignore it” is not evidence. Capability activation follows migration and contract deployment, never precedes them. Public publication descriptors and sync grants are individually revocable feature switches, not fake availability toggles.

Deployment review disposition: **5 risks found and resolved** — migration ownership, old/new overlap, capability activation order, cross-repository Nuvio sequencing, and backup-bound rollback.

## 46. Long-term trajectory and dream-state delta

Reversibility is **4/5**. Provider adapters, publication descriptors, grants, and projections can be disabled without changing Record identity or Chronicle evidence. Additive schema and immutable archive v3-v7 preserve rollback. The remaining one-way element is accepted user data written after migration; rollback preserves it through backup/restore rather than destructive down migration.

The complete M0-M13 programme reaches the requested 12-month state: provider-neutral identity, real Search/Discover/Library/Collections, complete metadata/MDBList/Nuvio integration, durable offline sync, and an authorized local shared workspace. No requested product capability remains as a TODO.

The residual delta is external status, not missing implementation:

- Nuvio Desktop and NuvioTV upstream maintainers control acceptance and release timing; Fasti still produces complete branches, PRs, fixtures, and end-to-end evidence.
- Public Fasti package publication remains a separate explicit release action even after package evidence passes.
- Additional providers can reuse the registry and adapter boundary, but no speculative adapter is added without a named source and terms review.

Long-term review disposition: **2 issues found and resolved** — the missing M13 delivery body and explicit separation of implemented/upstream-accepted/released states.

## 47. Design and UX review

### 47.1 Screen hierarchy

Every operational screen has one primary task and a fixed first/second/third scan order.

| Screen | First | Second | Third | Constraint |
| --- | --- | --- | --- | --- |
| Search | Query and enabled-source summary | Candidate results with identity confidence | Per-source status and provenance | Candidate inspection never mutates the Library. |
| Candidate details | Title, grain, year, and ambiguity state | Exact identifiers and selected projection | Add/attach/resolve-later actions and evidence | The primary action is unavailable until revalidation succeeds. |
| Record details | Stable local title and Record identity | Library state and Collection membership | Metadata claims, overrides, and sync evidence | Evidence is progressive disclosure, not a second destination. |
| Discover | Rail name, source, and freshness | Stable ordered candidates | Rail-local failure and refresh controls | One failed rail never replaces or moves another rail. |
| Library | Current profile and filter | Stable keyset page | Per-Record quick actions and stale labels | Background refresh never reorders under focus. |
| Collections | Local Collections and installed packs | Selected Collection members/bindings | Dry-run, receipt, rollback, and source evidence | Import starts with preview, never apply. |
| Metadata settings | Provider, purpose, and status table | Configure/test/replace actions | Safe health evidence and documentation | No plaintext secret read-back or card wall. |
| Connections | Connection identity, profile, and grants | Sync state, queued operations, and last success | Reauthorize/reconcile/revoke actions | Each capability is granted and degraded independently. |
| Local workspace | Sharing status and chosen projections | Members/devices and current grants | Discovery, audit, revoke, and recovery actions | Credentials, notes, and provider evidence remain private by default. |

The Workbench shell stays the single navigation owner. Search and Discover sit under Explore; Records, Collections, Settings, Account, and security retain their existing destinations. Canonical candidate and Record routes remain linkable and survive refresh, back/forward navigation, and direct entry.

```text
Explore/Search
  -> results [loading | empty | partial | error | success]
  -> candidate details [provenance | ambiguity | exact action]
  -> Record details [saved local identity]
       -> Library state
       -> Collection membership
       -> evidence/provenance

Settings/Connections
  -> provider list
  -> configure/test/grant
  -> persistent health and last error
  -> sync preview/reconcile/revoke

Collections
  -> source/pack/import
  -> bounded dry-run preview
  -> apply/rollback receipt

Local workspace
  -> enable sharing
  -> choose projections and members
  -> connect by URL/QR/discovery hint
  -> audit/revoke
```

| Feature | Loading | Empty | Error | Success | Partial |
| --- | --- | --- | --- | --- | --- |
| Search | Stable skeleton/list position | Query guidance | Per-source problem | Results with provenance | Local results plus failed-source status |
| Details | Existing heading retained | Missing provider field explanation | Re-fetch/review action | Canonical Record/candidate evidence | Unresolved identity stays usable |
| Library | Bounded page status | Clear first action | Persistent retry | Stable page and filters | Stale metadata labelled |
| Discover | Rail-local status | Source-specific empty | Stale fallback/retry | Deterministic rail | Other rails remain usable |
| Collections | Dry-run progress | No sources explanation | Path-specific validation | Receipt and rollback | Disabled dependencies retained |
| Connections | Step status | No connection prompt | Exact reauthorize/test action | Last success and grants | One capability degraded, others active |
| Local share | Connection status | No shared projections | Denied/revoked recovery | Explicit shared content | Some sources stale/private |

### 47.2 Journey and trust arc

| Horizon | User action | Intended feeling | Concrete support |
| --- | --- | --- | --- |
| First 5 seconds | Opens Search or a saved deep link. | Oriented and safe. | One H1, current profile/source status, one primary action, and the explicit statement that searching changes no Library state. |
| First 5 minutes | Searches, reviews evidence, creates or attaches a Record, and sees it in Library. | In control, not tricked by provider matching. | Revalidated candidate receipt, exact identifiers, Resolve later, atomic save receipt, persistent success, and a direct Record link. |
| First connection | Grants a Nuvio or MDBList capability and previews synchronization. | Confident about scope and reversibility. | Separate grants, affected counts, local/remote direction, dry-run, queued-operation view, revoke, and recovery guidance. |
| First failure | A provider times out, token rotates, or one batch fails. | Able to continue. | Local and last-known-good state remains visible; the failed scope, exact next action, and retry time remain persistent. |
| Long-term use | Changes providers, anime export preference, Collections, or shared members. | Trusts Fasti as the stable book. | Record identity does not move; impact preview, receipts, audit history, rollback, and source evidence explain every change. |

No copy uses guilt, urgency, fake confidence, or protocol vocabulary when a user-level term exists. The safe exit is always visible for ambiguous identity and destructive synchronization work.

### 47.3 Responsive and focus contract

| Viewport/input | Required composition |
| --- | --- |
| `>= 992px` | Existing Tabler vertical navbar remains visible. Operational content uses `.page-body` and `.container-fluid`; dense provider and Library data uses responsive tables or stable lists. |
| `768-991px` | Tabler owns the offcanvas navigation. Two-pane details may remain only when each pane keeps a readable measure; otherwise evidence follows the primary projection. |
| `320-767px` | One task column. Candidate and Record rows become labelled lists, not squeezed tables. Primary action precedes secondary evidence. Navigation uses the existing offcanvas trigger. No horizontal page scroll. |
| Keyboard | Explicit submit may move focus to the result-summary heading; background refresh never moves focus. Dialogs and offcanvas surfaces trap focus and return it to the invoker. Escape closes only the top dismissible layer. |
| Screen reader | Async regions use scoped `aria-live`/`role=status`; bulk progress is throttled to meaningful count changes. Errors reference their field or operation and remain in the document until resolved. |
| Touch/pointer | Every action is at least 44 by 44 CSS pixels. Drag/reorder behavior has button alternatives. Hover never carries unique information. |
| Zoom/contrast/motion | 320px reflow, 200% zoom, forced colors, high contrast, text spacing, and reduced motion preserve order, names, status, and actions. Sticky surfaces must not obscure focus. |

Use Tabler primitives first: navbar/offcanvas, list groups, tables, forms, alerts, badges, progress, modals, and pagination. Details pages use one strong title, a compact action bar, a primary metadata projection, and progressive disclosure for evidence. Apply the existing Newsreader, Atkinson Hyperlegible, and IBM Plex Mono roles and Fasti tokens. No generic card wall, AI gradient, transient-only error, focus loss, ornamental motion, or layout reshuffle is allowed.

### 47.4 Route ownership and migration

| Current route or surface | Final route or state | Navigation group | Desktop placement | Narrow placement | Owner and compatibility rule |
| --- | --- | --- | --- | --- | --- |
| `home` / Overview | `/` | Overview | Existing primary navigation | Existing offcanvas item | Workbench owns it; this programme does not replace it. |
| `discover` | `/explore/discover` | Explore | Explore group, Discover item | Explore group in offcanvas | M6 owns rails; retain a redirect from the former route until saved links migrate. |
| New provider Search | `/explore/search` | Explore | Explore group, Search item | Explore group in offcanvas | M4 owns provider fan-out and candidate routes. |
| `library` | `/library` with `state`, `kind`, and `review` filters | Library | One Library destination | One Library item; filters live in labelled disclosure | M5 owns Library state and keyset pages. In progress, Saved, Completed, and Needs review are presets, not permanent nav children. |
| `calendar` | `/library/calendar` | Library | Secondary Library action | Library action menu | Existing Calendar behavior remains; no new top-level destination. |
| `detail` / Media Detail | `/records/{grain}/{record_id}/{slug}` | None | Deep-link from Search, Library, Discover, Collections | Same route and content order | M4/M5 own the route. Remove the sidebar item after redirect evidence passes. |
| New candidate detail | `/explore/{source}/{grain}/{candidate_receipt_id}/{slug}` | None | Deep-link from Search/Discover | Same route in one column | M4 owns it. It never appears as a sidebar item. |
| `reconciliation` / Review Inbox | `/library?review=needs-review` | Library | Needs review preset | Needs review preset | Keep `/reconciliation` as a redirect/alias during the migration. |
| New Collections | `/collections`, `/collections/packs`, `/collections/nuvio` | Collections | Top-level destination with local subnavigation | One Collections item and in-page select/tabs | M7 owns Collections. Tabs use real links or correct Tabler tab semantics. |
| `connections` | `/connections` | Connections | Existing top-level destination | Existing offcanvas item | M9/M11/M13 own capability panels within the current page. |
| Nuvio Cloud account sync | `/connections/nuvio-cloud` | Connections | Connections subsection | In-page route/select | M11 owns the external account connector. |
| Nuvio client pairing | `/connections/nuvio-clients` | Connections | Connections subsection | In-page route/select | M11 owns the Fasti provider-client connector. |
| Local workspace | `/connections/sharing` | Connections | Connections subsection | In-page route/select | M13 owns sharing; it is not a Settings duplicate. |
| `settings` | `/settings` plus `/settings/metadata` and `/settings/identity` | Settings | Existing top-level destination | Existing offcanvas item | Settings links to Connections for account/client state and does not duplicate it. |

### 47.5 Two Search surfaces

| Contract | Global Search | Explore Search |
| --- | --- | --- |
| Purpose | Launch an existing local Record or a Workbench destination. | Find external and local candidates for inspection. |
| Existing owner | `packages/ui/src/global-search.svelte` | M4 Search vertical slice. |
| Placeholder | `Search records or commands` | `Search titles, people, or identifiers` |
| Shortcut | `Ctrl/Cmd+K`; remains a combobox launcher. | No conflicting global shortcut; route input receives focus only by explicit navigation or user action. |
| Data | Current local Records and visible navigation commands only. | Local index plus enabled provider/add-on sources through governed fan-out. |
| Result action | Open the selected Record or destination. | Open a durable candidate-details route; no Library mutation. |
| Status copy | `No records or commands match.` | Exact completed/failed source count, result count, freshness, and `Searching does not change your Library.` |
| Telemetry | No analytics or remote lookup. | Local operational counters only; no query text in logs or metrics. |

### 47.6 Candidate, Record, saved state, and Collections

Candidate details expose exactly three identity actions: **Create Fasti record**, **Attach to existing record**, and **Resolve later**. A successful create says `Record created. Tracking and saved state did not change.` and links to **View record**. A separate **Save to Library** action owns saved intent; progress, watched state, rating/review, notes, and Collection membership remain separate actions and transactions. Candidate details never show Library state before a Record exists. Record details order content as title/artwork/status, compact state actions, chosen metadata, then collapsed provenance; ambiguity opens the relevant evidence by default.

### 47.7 Two Nuvio directions

| User-facing journey | Remote party | Authentication | Capabilities | Status and recovery |
| --- | --- | --- | --- | --- |
| **Sync this Fasti profile with Nuvio Cloud** | Official Nuvio Cloud account and selected profile | Password is exchanged once and discarded; rotating refresh token stays in the vault. | Separately grant Library, watched, progress, ratings, and Collection directions supported by the official contract. | Own health, cursor, queued work, last success/error, reauthorize, conflict review, reconcile, and revoke. |
| **Connect Nuvio to this Fasti server** | Nuvio Desktop/TV client using the Fasti provider contract | Authorization Code with PKCE or Device Authorization from the authentication programme. | Separately grant catalog, metadata, Library, watched, progress, Collections, and supported writes. | Own client identity, grant list, journal/ack state, last success/error, reconcile, rotate/revoke, and device removal. |

No screen combines these into one `Nuvio connected` badge. Failure or revocation in one direction does not imply failure in the other.

### 47.8 Visible route-state contract

| Surface and state | Headline and explanation | Primary / secondary action | Retained data | Focus and announcement | Safe retry or cancellation |
| --- | --- | --- | --- | --- | --- |
| Search initial load | `Search metadata sources` / `Searching does not change your Library.` | Search / choose sources | Query and source choices | H1 on route entry; no automatic live message | N/A |
| Search first request | `Searching 4 sources…` / completed count remains visible | Cancel search / view local results | Query, filters, reserved result geometry | Search result region `aria-busy=true`; one polite milestone region | Cancel aborts pending reads and keeps completed results. |
| Search next page | `Loading more results…` | Cancel / keep current page | Existing rows, cursor, and focused item | Append after current list; never replace focus | Cursor-bound retry is safe. |
| Search refresh | `Checking for newer results…` | Cancel / use current results | Existing rows and selection | No focus movement; announce completion only | Safe read retry. |
| Search partial | `Results from 3 of 4 sources` / name failed source and next retry | Retry failed source / use these results | Local and completed-source candidates | Persistent region-local status | Retry failed subset only. |
| Search empty | `No candidates found` / suggest spelling, source, language, or identifier changes | Change search / search local Records | Query and filters | Empty-state heading receives focus after explicit submit | Safe. |
| Candidate expired | `This candidate needs to be checked again` / source receipt expired | Re-check candidate / Resolve later | Displayed evidence marked stale | Error heading after attempted action; one alert | Re-fetch is safe and does not write. |
| Candidate ambiguous | `More than one identity may match` / no automatic attach | Compare evidence / Resolve later | Every candidate and identifier | Evidence opens; focus conflict heading | No mutation until explicit choice. |
| Record created | `Record created` / `Tracking and saved state did not change.` | View record / Save to Library | Atomic receipt and source evidence | Success heading receives focus; polite announcement once | Idempotency returns the same receipt. |
| Record stale metadata | `Showing last known metadata` / source and age are visible | Refresh source / choose another source | Record, overrides, and evidence | Status near metadata owner; no route focus reset | Safe read retry. |
| Discover rail loading/failed | `Loading {rail}` or `{rail} could not refresh` | Retry rail / use last known items | Other rails and stale rail items | `aria-busy` on one rail; no carousel-wide alert | Retry one rail; cancellation keeps current items. |
| Library first use | `Your Library is ready` / explain saved, progress, watched, and Collections as separate state | Search metadata sources / import a Collection | Current profile | Empty-state heading after route entry | N/A |
| Library filter empty | `No saved records match these filters` / records are unchanged | Clear filters / Search metadata sources | Filters and full Library | Result summary after explicit filter change | Safe. |
| Library page/refresh | `Loading more records…` or `Checking for updates…` | Cancel / keep current page | Existing stable tuple page and focus | Append pages; background refresh never moves focus | Safe keyset retry. |
| Collection preview | `Review this Collection change` / show additions, changes, disabled sources, removals, loss risk, and unknown fields | Merge / replace / keep as reference / cancel | Original file/source and dry-run result | H1 on route; validation summary links to paths | Preview is safe and repeatable. |
| Collection apply | `Applying approved changes…` / show completed and remaining counts | Cancel before commit boundary / continue | Preview digest and selected mode | One polite milestone region; focus stays on progress owner | After commit boundary, finish transaction then offer rollback. |
| Collection result/rollback | `Collection changes applied` or `Rollback completed` | View receipt / Roll back / View Collection | Receipt, original envelope, disabled items | Result heading focused; one status message | Same digest/idempotency replays receipt. |
| Provider configure/test | `Configure {provider}` then `Credential saved; testing access…` | Save and test / cancel / replace / remove | Credential reference only; never plaintext | Error summary links to field; success focuses status notice | Test is safe; removal requires recent auth and impact preview. |
| Anime impact preview | `Review anime identifier changes` / show affected Records and unchanged local identity/history | Apply preference / cancel | Current preference and affected counts | Preview heading; no live chatter | Compare-and-set revision; safe retry. |
| Nuvio Cloud auth/sync | `Choose a Nuvio profile`, `Sync preview`, or `Sync paused` | Connect/authorize/reconcile / cancel/revoke | Local state, vault token reference, journal, cursors | Blocking errors use one alert; progress is polite | Password discarded; token rotation persisted before continued work. |
| Nuvio client pairing | `Connect Nuvio to this Fasti server` / show client and requested access | Approve / deny / revoke | Current grants and device record | Route H1 or device-code field; expiry announced once | Device flow can restart; no partial grant. |
| First-run resume | `Continue setup` / show completed, optional, blocked, and current steps | Continue / Save and exit / Skip for now when optional | Every completed receipt and entered non-secret choice | Current step H1; errors link to owner | Each step is independently resumable. |
| Local sharing | `Choose what members can see` / name always-private data | Save sharing / manage members / disable sharing | Current grants, members, audit, offline cache policy | Change summary focused after submit | Compare-and-set; revoke closes new reads and invalidates cache. |

Every blocking form error has a summary that links to the invalid control. `role=alert` fires only for a newly introduced blocking error. Progress uses one polite live region per active route and announces meaningful milestones, not every item. Skeletons reserve final dimensions, stop animating under reduced motion, and never replace focused content.

### 47.9 First-run resumption

The setup journey has an eight-step progress tracker, current-step H1, completed receipts, and **Save and exit** on every step. Language/region and one useful metadata route are the minimum completion path. Ratings, anime preference, Collection packs, Nuvio, and local sharing show **Skip for now** when not applicable. A failed credential test preserves non-secret form state and offers retry, replace, documentation, and exit. Offline setup offers local-only Search/Library use and records the next online step. With no provider configured, the finished state explains local capability and links to permanent Metadata settings. Completion returns to the Workbench with one next action, not a forced tour.

### 47.10 Collection safety sequence

```text
select file/source
  -> bounded size/depth/shape/digest validation
  -> dependency + licence + downstream-loss report
  -> diff: add/change/disable/remove/unknown fields
  -> choose merge | replace | keep as reference
  -> confirm affected counts + what stays unchanged + rollback
  -> apply [cancel allowed before transaction boundary]
  -> durable receipt
  -> view Collection | rollback
```

Disabled and unsupported sources remain visible, labelled, and filterable. Replace never means silent deletion: the confirmation names membership, bindings, folders, unknown fields, and source documents that change or remain. Cancellation after the transaction boundary waits for the atomic commit and then offers rollback.

### 47.11 Tabler component and owner map

| Surface | Reuse first | Required maturation | Owner |
| --- | --- | --- | --- |
| Shell/navigation | Existing Workbench `.page`, vertical navbar, offcanvas, toolbar | Route groups and redirects only; no second shell | Shared UI owner |
| Global Search | Existing `global-search.svelte` combobox and shortcut | Update route command set only | Shared UI owner |
| Explore Search/results | Tabler input group, form controls, list group or table, alert, pagination | Candidate summary rows and stable status region; no generic card grid | M4 |
| Candidate/Record details | Existing Media Detail and action patterns; Tabler page header, button group, list/table, collapse/details | Canonical routes and evidence disclosure | M4/M5 |
| Library | Existing `library-view.svelte`; Tabler poster cards only when poster is the interaction; `.table.table-responsive` for list mode | Server keyset filters, stable page/refresh behavior, mobile labelled list | M5 |
| Discover | Existing `discover-view.svelte`; Tabler list/card composition | Rail-local controls, explicit previous/next buttons, stale/partial status | M6 |
| Collections | Existing `collection-modal.svelte`; Tabler modal/offcanvas, form, alert, progress | Source/binding/pack pages and safe import preview | M7 |
| Metadata settings | Existing settings section and native credential handling; Tabler responsive table/list group | Replace `.provider-key-card` wall with compact status rows and one action owner | M1/M8/M9 |
| Connections | Existing `connections-view.svelte`, `api-clients-panel.svelte`, Tabler status/table/list | Replace connector-card grid with task rows; split Nuvio Cloud, Nuvio clients, and sharing | M9/M11/M13 |
| Reconciliation | Existing `reconciliation-view.svelte`; Tabler alert, table/list, modal | Route under Library, persistent conflict receipts | M3/M9/M11 |
| Status | Shared semantic badge/alert mapping | Text plus icon, forced-colors shape, no color-only state | Shared UI owner |

Tabs use real routes when content must be deep-linkable. If a Tabler tab control remains in one route, it uses `tablist`, `tab`, `tabpanel`, arrow keys, Home/End, selected state, and deterministic focus. Horizontally scrollable Discover rails expose labelled Previous/Next buttons and a non-drag path.

### 47.12 User-facing vocabulary

| Internal term | Primary UI term | Where the internal term may appear |
| --- | --- | --- |
| claim | metadata source or source evidence | Expanded technical details and diagnostics |
| projection | chosen metadata or what members can see | Expanded evidence and API docs |
| grant | access | Expanded security/client details |
| cursor | sync position | Diagnostics and API docs |
| receipt | change details or audit entry | Technical details, exports, and API docs |
| capability | action or access area | Connection/API-client details when needed |
| materialize source binding | add selected source items to this Collection | Technical details only |

Use **See metadata sources and sync**, **Choose what members can see**, and **Details**. Use Record, saved, Collection, source, connection, and sync direction consistently.

The reviewed artifacts are `docs/designs/nuvio-metadata-flow-wireframe.html` and `docs/designs/nuvio-metadata-route-reference.html`. The first is explicitly flow-only. The second fixes route-level information order at 1440px and 375px for Search/candidate evidence, Record provenance, Library filter/empty state, Collection preview/partial state, both Nuvio directions, and local sharing. Both are planning references; final implementation must use the existing Tabler shell and components.

Design review disposition before `/autoplan`: **5 issues found and resolved** — details-route ownership, state coverage, focus preservation during refresh, compact provider settings, and explicit sync/reconciliation status. The full review adds the fixed per-screen hierarchy, journey/trust arc, breakpoint behavior, focus/live-region rules, and the missing Nuvio/local-workspace wireframe states. Post-implementation `/design-review` remains a mandatory evidence gate.

### 47.13 Dual-voice design review result

The in-host review and an independent design-review agent evaluated the same plan and source surfaces. They reached consensus on all seven review dimensions. The independent pass reported twelve concrete gaps; the in-host pass confirmed the same underlying problems. Every gap was resolved in Sections 47.1-47.12 and the route reference before this gate closed.

| Dimension | Initial | Final | Resolution |
| --- | ---: | ---: | --- |
| Information hierarchy | 6/10 | 10/10 | Fixed route ownership, scan order, navigation grouping, and canonical deep links. |
| States and edge cases | 6/10 | 10/10 | Added visible loading, empty, partial, stale, blocked, success, retry, and cancellation contracts. |
| Journey coherence | 5/10 | 10/10 | Connected Search, candidate evidence, Record creation, Library state, Collections, and both Nuvio directions. |
| Product specificity | 7/10 | 9/10 | Replaced generic integration language with Fasti vocabulary and source-specific actions; final copy remains an implementation QA concern. |
| Design-system reuse | 7/10 | 10/10 | Assigned existing Tabler and Fasti component owners; prohibited a second shell and generic card walls. |
| Responsive and accessibility mechanics | 6/10 | 10/10 | Added breakpoint composition, focus ownership, live-region, touch, zoom, contrast, and reduced-motion rules. |
| Decision closure | 4/10 | 10/10 | Closed the route, Search, state-ownership, Nuvio-direction, first-run, Collection-safety, and component-owner decisions. |

Overall design readiness improved from **6/10 to 9/10**. The remaining point is intentionally reserved for implementation evidence: generated UI, browser interaction, assistive-technology behavior, and final production copy have not yet been built. The planning gate has zero unresolved design decisions.

The seven-point visual litmus is **6/7 materially demonstrated** in the route reference: visual hierarchy, grouping, task order, responsive collapse, explicit states, and constrained component reuse are visible. Ornamental motion is deliberately absent; functional motion and reduced-motion behavior remain implementation checks rather than invented mockup evidence. The design generator was unavailable, so the checked HTML references are explicitly flow and route-order evidence, not production-layout approval.

### 47.14 Design-derived implementation checks

- [ ] **D1 — Route and Search ownership:** migrate the current route aliases, keep Global Search local, add Explore Search, and prove refresh/back/forward/deep-link behavior.
- [ ] **D2 — State, focus, and accessibility:** implement the Section 47.8 state contract, 44px targets, focus return, bounded live regions, 320px reflow, 200% zoom, forced colors, and reduced motion.
- [ ] **D3 — Resumable setup:** persist each completed first-run receipt, preserve non-secret input, and prove Save and exit, optional skip, offline, and retry paths.
- [ ] **D4 — Collection safety:** implement bounded validation, loss/diff preview, explicit merge/replace/reference choice, transaction-boundary cancellation, receipt, and rollback.
- [ ] **D5 — Nuvio direction split:** render Nuvio Cloud account sync and Nuvio client-to-Fasti pairing as separate routes, grants, health states, recovery actions, and revoke paths.
- [ ] **D6 — Reference conformance:** compare implementation screenshots and accessibility trees at 1440px and 375px with the route reference while preserving the existing Tabler Workbench shell.

Design review disposition after `/autoplan`: **12 issues found and resolved; 0 unresolved decisions**. Independent and in-host reviewers agreed on all seven dimensions. No user challenge or adjudication was required because the user had already authorized every recommended decision.

```text
+====================================================================+
|         DESIGN PLAN REVIEW — COMPLETION SUMMARY                    |
+====================================================================+
| System Audit         | DESIGN.md read; material UI scope           |
| Step 0               | 6/10; routes, states, trust, responsive     |
| Pass 1  (Info Arch)  | 6/10 -> 10/10 after fixes                  |
| Pass 2  (States)     | 6/10 -> 10/10 after fixes                  |
| Pass 3  (Journey)    | 5/10 -> 10/10 after fixes                  |
| Pass 4  (AI Slop)    | 7/10 -> 9/10 after fixes                   |
| Pass 5  (Design Sys) | 7/10 -> 10/10 after fixes                  |
| Pass 6  (Responsive) | 6/10 -> 10/10 after fixes                  |
| Pass 7  (Decisions)  | 12 resolved, 0 deferred                    |
+--------------------------------------------------------------------+
| NOT in scope         | written (8 items)                           |
| What already exists  | written                                    |
| TODOS.md updates     | 0; required work stays in programme         |
| Approved Mockups     | 0; two checked planning references          |
| Decisions made       | 9 design decisions added to plan            |
| Decisions deferred   | 0                                           |
| Overall design score | 6/10 -> 9/10                                |
+====================================================================+
```

The plan is design-complete. Run `/design-review` after implementation for visual QA.

## 48. Stale diagram audit

| Diagram | Status after this plan |
| --- | --- |
| `docs/designs/nuvio-metadata-flow-wireframe.html` | Current for Search -> evidence -> local ownership plus M11 Nuvio and M13 local-workspace states; implementation must replace its standalone shell with the existing Tabler Workbench. |
| `docs/architecture/nuvio-integration.md` architecture | Historically accurate for the process-local conformance model, but incomplete for production; M11 must update it with Nuvio Cloud and Fasti provider lanes before activation. |
| `ROADMAP.md` B0-B8 flow | Current historical programme ordering, but incomplete for this approved M0-M13 programme; roadmap update lands in the first applicable implementation/documentation slice. |
| This plan's architecture/data/state/deployment/rollback/user-flow diagrams | Current and controlling for the programme. |

## 49. Implementation tasks

Synthesized from the CEO review. All are in scope and must ship.

- [ ] **T1 (P1, human: ~1d / Codex: ~2h)** — Contracts — Freeze the M0 source, ownership, capability, error, threat, PR, and evidence ledgers.
  - Surfaced by: Architecture and deployment reviews.
  - Files: `docs/plans/`, `docs/designs/`, `contracts/registry/v1/`, `tests/conformance/uat-matrix.csv`.
  - Verify: plan reviews clear; deterministic contract preview; no production change in M0.
- [ ] **T2 (P1, human: ~5d / Codex: ~1d)** — Providers — Extract the existing Tauri provider behavior into the concrete `fasti-provider-runtime` crate and compose its single registry, vault-reference port, governed transport, budgets, health, and bounded dispatcher from both `fastid` and Tauri.
  - Surfaced by: Security and DRY reviews.
  - Files: domain/application/store/contracts/API/Tauri/UI provider owners.
  - Verify: credential isolation, SSRF negative controls, restart, browser non-disclosure.
- [ ] **T3 (P1, human: ~6d / Codex: ~1d)** — Metadata — Implement claim lifecycle, projection policy, archive v3, safe legacy-override ownership migration, cache partitions, provenance, and overrides; later slices own archive v4-v7 when their state exists.
  - Surfaced by: Data, migration, and offline reviews.
  - Verify: stale/retraction/locale/profile isolation and v1/v2/v3 restore equality.
- [ ] **T4 (P1, human: ~4d / Codex: ~1d)** — Identity — Implement purpose routing, TMDB aliases, anime projection policy, and impact preview.
  - Surfaced by: Architecture and identity-threat reviews.
  - Verify: MAL/Kitsu plus IMDb fixtures; no Record re-keying.
- [ ] **T5 (P1, human: ~7d / Codex: ~2d)** — Search — Implement local-first multi-source Search, durable candidate receipts, canonical details routes, and atomic Record actions.
  - Surfaced by: UX and data-flow reviews.
  - Verify: offline/partial/duplicate/expired candidate, route/slug, 10k Record latency.
- [ ] **T6 (P1, human: ~5d / Codex: ~1d)** — Library and Discover — Implement server keyset Library queries and governed Discover rails.
  - Surfaced by: Performance and UX reviews.
  - Verify: stable pages, filters, stale rails, no full materialization.
- [ ] **T7 (P1, human: ~7d / Codex: ~2d)** — Collections — Implement Collection entities, bindings, packs, Kaptain-compatible lossless import/export, receipts, and rollback.
  - Surfaced by: Import security and portability reviews.
  - Verify: generated 3,059-source fixture, hostile bounds, unknown-field round trip.
- [ ] **T8 (P1, human: ~5d / Codex: ~1d)** — TMDB — Implement all approved field groups, localization, episode/company/network routes, caching, and attribution.
  - Surfaced by: Product completeness review.
  - Verify: field-group, alias, outage, override, and attribution gates.
- [ ] **T9 (P1, human: ~7d / Codex: ~2d)** — MDBList — Land M9a ratings/catalog reads separately from M9b private account-state synchronization; implement independent grants, partitions, budgets, explicit deletes, and three-way reconciliation on the shared synchronization substrate.
  - Surfaced by: Scope and data-classification reviews.
  - Verify: independent-grant matrix, pull/push/remove, loop suppression, leakage negatives.
- [ ] **T10 (P1, human: ~5d / Codex: ~1d)** — Nuvio publication — Implement pinned Stremio manifest/catalog/meta and private-state leak controls.
  - Surfaced by: Compatibility and publication-threat reviews.
  - Verify: pinned Desktop/TV/Stremio fixtures; no streams or secrets.
- [ ] **T11 (P1, human: ~12d / Codex: ~3d)** — Nuvio synchronization — Land M11a-g independently: shared Fasti journal/cursor/lease substrate; Cloud bootstrap; Cloud steady-state reconciliation; Fasti provider v1; durable Desktop and TV clients; exact-revision cross-client evidence; and upstream PRs.
  - Surfaced by: Nuvio contract omission and rotating-token review.
  - Verify: crash at every local/remote durability boundary; snapshot/delta/write/delete/ack/restart/revoke/compaction/idempotency/lease fixtures; independent Cloud/provider activation; exact native clients against a real durable Fasti process.
- [ ] **T12 (P1, human: ~7d / Codex: ~2d)** — Integrated hardening — Complete contracts, packaging, security, accessibility, performance, offline, backup, restore, and exact-head evidence.
  - Surfaced by: Test, deployment, and Definition of Done reviews.
  - Verify: `cargo xtask test pr`, deep/milestone gates, UI/AT evidence, x86_64/aarch64 receipts.
- [ ] **T13 (P1, human: ~8d / Codex: ~2d)** — Local workspace — Land M13a-e independently: explicit enablement and safe discovery; authorization; read projections; mutation/delta/revocation; offline cache and integrated evidence.
  - Surfaced by: Mission completeness and long-term review.
  - Verify: privacy, isolation, revoke-during-read, discovery leakage, backup/restore, UI and performance gates.

## 50. CEO review completion summary

| Review area | Result |
| --- | --- |
| Mode | HOLD SCOPE; no capability cuts and no deferrals. |
| System audit | Exact base, isolated ownership, current runtime truth, and external source revisions recorded. |
| Architecture | 3 issues found and resolved. |
| Error map | 28 codepath/failure classes mapped; 0 gaps. |
| Security | 13 issues found and resolved across architecture and CSO review; high-impact threats have explicit negative controls. |
| Data and UX | 14 edge cases mapped; 0 unhandled. |
| Code quality | 4 issues found and resolved through existing owners and platform features. |
| Tests | Full topology added; 6 gaps resolved. |
| Performance | 3 issues resolved with bounds and reproducible workloads. |
| Observability | 4 gaps resolved with local-only structured diagnostics. |
| Deployment | 5 risks resolved; sequence and rollback diagrams recorded. |
| Long term | Reversibility 4/5; 2 omissions resolved. |
| Design | 5 issues resolved; complete state map and user flow recorded. |
| NOT in scope | 8 constitutional/prohibited/separate-authority items; no requested capability deferred. |
| TODO proposals | 0. Every necessary item is in the implementation programme. |
| Implementation tasks | 13, all P1 programme work. |
| Diagrams | Architecture, data/shadow paths, state machines, deployment, rollback, and user flow. |
| Critical gaps | 0. |
| Unresolved decisions | 0. |

The planning review chain is complete. M0 is approved under the user's all-recommended authority, so implementation continues without another scope prompt.

## 51. Developer experience review

### 51.1 Product type, mode, and primary persona

This programme is an **API/service plus repository-local SDK and integration platform**. `/plan-devex-review` runs in **DX POLISH** mode: all eight developer touchpoints are in scope, but the review adds no speculative hosted sandbox, plugin marketplace, code generator, or language SDK.

| Persona field | Primary answer |
| --- | --- |
| Developer | An open-source media integration developer who knows HTTP/JSON and one of Rust, TypeScript, or Kotlin, but does not yet know Fasti's bounded contexts. |
| Job | Add or update a metadata source, public catalog projection, or Fasti state client without moving Record identity, leaking a credential, or inventing retry semantics. |
| First success | Their renamed deterministic fixture normalizes into a source-neutral candidate or state operation and `cargo xtask integration check` prints a digest-bound pass. |
| Constraints | No live account, provider key, internet access, personal media data, generated-file edits, or production service is required for first success. |
| Trust need | The tool must say which source is authoritative, what it checked, what remains optional/live, and the exact safe next action on failure. |

Empathy narrative: this contributor arrives with useful provider knowledge, not Fasti architecture knowledge. Today they must infer the difference between authored and generated contracts, fixture-only and production behavior, provider metadata and Record identity, and narrow versus aggregate gates. The polished path lets them prove one meaningful integration behavior first, then reveals domain, security, delivery, and release detail only when it becomes relevant.

### 51.2 Competitive benchmark and selected tier

| Tool/path | Official first-success shape | Useful choice | Source |
| --- | --- | --- | --- |
| Stremio Addon SDK | Define a manifest and handler, install the SDK, run the file, receive a local install URL. | One concrete media integration and immediate local feedback. | https://stremio.github.io/stremio-addon-sdk/ |
| Stremio addon bootstrap | Bootstrap, install, and start with launch; the client opens with the add-on installed. | A golden path chooses defaults and demonstrates real client consumption. | https://stremio.github.io/stremio-addon-guide/sdk-guide/step1 |
| Supabase CLI | `supabase init`, then `supabase start`. | Project-scoped state and a reproducible local environment in two commands. | https://supabase.com/docs/guides/local-development/cli/getting-started |
| Current Fasti provider path | Manifest, runtime source, fixtures, validator, docs, and correct gate must be discovered separately; no complete golden path exists. | Strong contract ownership, but no provider-authoring first success. | Current source at the M0 base. |
| Approved Fasti target | Copy one minimal authored example, change manifest and fixture values, run one focused check. | Real contract semantics, offline and credential-free, using existing repository owners. | This plan. |

The selected target is **Competitive tier: three steps and at most five minutes** after checkout with supported prerequisites and a warm locked toolchain cache. Champion-tier under two minutes would require a prebuilt published binary or hosted playground; neither is authorized by this repository-only programme. The full cold Rust/Node dependency build is reported separately and cannot be hidden inside the TTHW claim.

### 51.3 Three-step golden path and magical moment

```text
1. git clone --branch dev https://github.com/Scrobble-dev/Fasti.git && cd Fasti
2. cp -R contracts/addons/examples/minimal-metadata-source contracts/addons/examples/hello-fasti-provider
3. cargo xtask integration check contracts/addons/examples/hello-fasti-provider/provider.yaml
```

Before step 3, the guide points to the four values a developer changes for a real local experiment: `metadata.id`, `metadata.name`, primary-source declaration, and fixture payload values. Git and the pinned Rust toolchain are the only first-success prerequisites. Node, pnpm, browser QA, Tauri, GTK/WebKit, live credentials, and the aggregate PR gate appear later at the stage that needs them.

The example is a deterministic contract fixture, not a fake production provider. The check executes normalization, empty response, rate limit, invalid response, identifier/grain, bounds, secret-placement, and deny-wins transport assertions against loopback fixtures. The magical moment is a source-neutral candidate preview followed by:

```text
PASS integration=<id> kind=metadata_source contract=<revision> checks=<count> fixtures=4 digest=sha256:<digest>
```

The next line links to the real-adapter step and states that live provider access has not been tested. The Nuvio state-client lane uses the same command and output grammar against its versioned loopback server; it does not pretend a metadata fixture proves sync.

### 51.4 Nine-stage developer journey

| Stage | Developer question | Planned surface and acceptance condition |
| --- | --- | --- |
| 1. Discover | Where do integrations begin? | `CONTRIBUTING.md` links one Integration authoring entry; it identifies metadata source, public catalog, and state client without a terminology maze. |
| 2. Choose | Which lane matches this work? | A three-row decision table names data read/write scope, trust boundary, example, and required review tier. |
| 3. Prepare | What must be installed or configured? | Exact supported Rust/Node versions and locked commands; first success needs no account, key, network, container, or global tool. |
| 4. Prove the example | Does the contract actually run? | The checked-in minimal example passes through the same parser and conformance code used by CI. |
| 5. Customize | Which files are mine to edit? | Authored manifest and fixtures are listed; generated paths are labelled and rejected with a corrective action. |
| 6. Debug | What failed and what remains safe? | Human and JSON problems state code, location, expected/received, retained state, next action, docs, and correlation ID where applicable. |
| 7. Verify | Which command is enough now? | Focused integration check first; locked contract verification and `cargo xtask test pr` appear only at the review/PR stage. |
| 8. Submit | What evidence and governance are required? | Existing contribution tier, Discussion/RFC, DCO, PR template, source/licence record, threat boundary, fixture digest, and focused/full command output. |
| 9. Maintain | How does a client survive upgrades? | Version discovery, compatibility matrix, additive-v1 policy, explicit deprecation, pinned fixtures, and old-client conformance evidence. |

### 51.5 API, CLI, SDK, and naming contract

| Concern | Decision |
| --- | --- |
| One semantic owner | `contracts/addons` owns provider manifests; the capability registry owns public capability meaning; domain/application own behavior; generated OpenAPI/SDK/CLI project those sources. |
| Command | `cargo xtask integration check PATH` is the only new developer entry point. The manifest or fixture declares its kind; no duplicate config or global install exists. |
| Defaults | Offline deterministic fixtures, human output, no mutation, no live credential, no generated rewrite. |
| Automation | `--output json` returns the governed result/problems and stable non-zero exit status; it does not define a second error schema. Exit `0` is pass, `2` is validation/compatibility failure, and `1` is local tool/environment failure. |
| Identifiers | Preserve typed prefixes and source-neutral IDs. Provider IDs remain evidence; sample names cannot imply canonical identity. |
| Retry and writes | Reads, idempotency, acknowledgement, revisions, tombstones, and cursor recovery come from the public contract. SDK/client examples do not add local retry rules. |
| Language support | The generated TypeScript SDK remains the supported generated client. Nuvio Kotlin clients use their existing native HTTP/serialization stack plus plain OpenAPI/fixtures; no speculative Kotlin SDK. |
| Progressive disclosure | The minimal example proves one search or state round trip. Pagination, partial failure, revocation, recovery, performance, and publication follow in linked guides and conformance cases. |

Contributor edit ownership is explicit:

| Change | Authored owner |
| --- | --- |
| Provider declaration and terms | `contracts/addons/manifests/` plus the provider's primary-source ledger. |
| Identity and grain mapping | Domain/application identity routing; never the adapter DTO. |
| Trusted transport | The shared governed-egress adapter and registry policy. |
| Normalization | The provider adapter projecting source payloads into domain-owned candidate/claim input. |
| Typed problems | Capability/problem registry and application problem mapping. |
| Deterministic fixtures | `contracts/addons/fixtures/{provider_id}/` or the permissive provider-client interoperability corpus. |
| Public capabilities | `contracts/registry/v1/`; generated files remain read-only. |
| Generated OpenAPI and SDK | Existing deterministic contract generator only. |
| Verification | Focused `integration check`, then locked contract verification and the PR gate. |

### 51.6 Error and debugging trace

| Path | Current experience | Required experience |
| --- | --- | --- |
| Malformed manifest/fixture | No focused provider check; a contributor can encounter a broad validator failure or discover drift later. | `provider.manifest.invalid` first, file plus pointer/line, safe actual value, expected shape, exact edit, docs, exit `2`; all other fixture results retained. |
| Denied provider network route | Runtime has governed denial, but author-facing evidence is spread across code, docs, and UI. | `provider.network.denied`, provider/host and denied address class, redirect/DNS stage, unchanged local state, retryability, correlation ID, exact safe diagnostic; secrets removed. |
| Nuvio contract/cursor mismatch | The production contract is not implemented, so no consolidated client remediation exists. | `nuvio.remote_contract_changed` or `nuvio_cursor_expired`, server/client revisions, supported range, retained journal/ack state, snapshot recovery eligibility, and one next command/action. |

Human output is Rust-style annotated and action-first. JSON output remains RFC 9457/governed schema style. Framework stack traces are never the first diagnostic. `RUST_BACKTRACE` remains the maintainer escape hatch, so no custom debug subsystem is added.

### 51.7 Documentation, versioning, and community path

The documentation order is:

```text
README.md / CONTRIBUTING.md / contracts/README.md
  -> docs/integrations/README.md                              lane chooser
      -> contracts/addons/README.md                           authored manifest + first pass
      -> docs/integrations/developing-metadata-provider.md    real adapter and safe transport
      -> docs/integrations/developing-fasti-contract-client.md state client, versioning, recovery
      -> generated OpenAPI / SDK reference                    exhaustive operations and types
```

Each page links rather than redefines Record, identifier, claim, capability, grant, receipt, and Collection semantics. Every copy-paste command and example runs in CI. Version-specific pages name the contract revision and exact tested Nuvio client commits. A local deterministic harness replaces an online playground because it works offline, handles sensitive integration shapes safely, and reuses production contract types.

Provider manifests retain `api_version`, `compatibility.fasti`, and `compatibility.manifest_schema`. Fasti state clients first call capability discovery and compare the returned minimum/maximum contract revision with their own minimum/maximum before snapshot or mutation. A client selects the highest overlap or fails with both ranges and one compatible action. Version 1 changes are additive or clarification-only. A breaking change uses a new `/api/v2` contract and fixture set; minimum and maximum supported revision fixtures remain in conformance for each release. Deprecation names the replacement, last supported Fasti release, client impact, and migration guide. There is no speculative calendar support promise and no codemod until a real mechanical migration exists.

The existing contribution tiers, Discussion/RFC rule, DCO, CODEOWNERS, and PR template remain the community path. Add one integration-specific issue form using the current GitHub issue-form system. It captures primary source and terms, capabilities and data classes, identity/grain mapping, credentials, network hosts, bounds, empty/delete semantics, rate behavior, fixture plan, threat boundary, and maintainer owner. A new issue system or marketplace is unnecessary.

Every emitted problem documentation URL is checked against the built documentation artifact, not only for syntactic shape. The SDK guides include runnable retryability, idempotency, cursor recovery, cancellation, and `FastiProblemError.problem` examples. Every new public operation has generated typed methods; `Record<string, unknown>` is not an accepted public escape hatch.

### 51.8 Measurement and scorecard

No product telemetry or phone-home code is added. Evidence is repository-local or CI-bound:

| Measure | Gate |
| --- | --- |
| Time to first working contract | Three documented steps; <=5 minutes in the supported developer container with warm locked caches; cold dependency time reported separately. |
| Focused feedback loop | Ten warm deterministic runs; p95 <=60 seconds; no internet, account, key, or personal data. |
| Example truth | 100% of checked-in integration examples execute in CI against current authored/generated contracts. |
| Diagnostic quality | Human and JSON golden fixtures cover pointer, code, safe values, next action, built docs link, exit status, and redaction. |
| Cognitive load | One lane chooser, one authored manifest owner, one focused command, one full PR gate; no duplicated setup choice. |
| Continuity | A failed fixture run preserves every passed result; client recovery preserves journals, acknowledgements, cursors, and exact revision evidence. |

| Pass | Initial | Final plan | Finding closed |
| --- | ---: | ---: | --- |
| Getting started | 4/10 | 9/10 | Added a three-step credential-free golden path and measurable TTHW. |
| API/CLI/SDK | 6/10 | 9/10 | Added one focused command, kind-owned dispatch, stable output, and explicit language boundary. |
| Errors/debugging | 7/10 | 9/10 | Traced three failures and specified action-first human plus governed JSON output. |
| Documentation/learning | 5/10 | 9/10 | Added a findable progressive path with executable examples and canonical links. |
| Upgrade/migration | 5/10 | 9/10 | Added discovery, supported ranges, additive-v1 rule, pinned old-client tests, and explicit deprecation. |
| Environment/tooling | 5/10 | 9/10 | Reused xtask and offline fixtures for a narrow <=60-second feedback loop. |
| Community/ecosystem | 6/10 | 9/10 | Reused governance and added an integration proposal checklist and ownership route. |
| Measurement | 3/10 | 9/10 | Added local/CI TTHW, loop, truth, diagnostic, cognitive-load, and continuity gates. |

Independent review initially scored the plan **5.5/10** and identified ten P1 plus five P2 amendments. The in-host review confirmed all of them. The amended plan reaches **9/10**: every recommended amendment is incorporated, including the extension boundary, exact CLI convention, contributor edit map, stage-specific prerequisites, licence scope, version overlap, issue form, typed SDK examples, built-link checks, and local measurement. The unclaimed point is implementation evidence: the command, examples, docs, timings, and diagnostics do not exist until their owning slices land. There are zero unresolved DX decisions and zero deferred TODOs.

### 51.9 Developer-experience implementation tasks

- [ ] **DX1 (P1, human: ~1d / Codex: ~2h)** — Contracts/xtask — Validate the existing provider manifest source and add `cargo xtask integration check PATH` over deterministic kind-specific fixtures.
  - Surfaced by: Getting started and tooling — no complete provider-authoring first success exists.
  - Files: `contracts/addons/`, `xtask/src/`, focused conformance tests.
  - Verify: minimal metadata-source example passes offline; malformed/unsafe mutations fail.
- [ ] **DX2 (P1, human: ~1d / Codex: ~2h)** — Problems/CLI — Render action-first human diagnostics and governed `--output json` results for integration checks.
  - Surfaced by: Error trace — validation, network denial, and contract recovery lack one contributor-facing projection.
  - Files: `xtask/src/`, `crates/fasti-contracts/`, golden fixtures.
  - Verify: three error paths and redaction mutations pass with stable exit status.
- [ ] **DX3 (P1, human: ~1d / Codex: ~2h)** — Documentation/community — Add the lane chooser, two developer guides, integration issue form, executable copy-paste examples, SDK recovery examples, and built problem-link checks.
  - Surfaced by: Documentation journey — authoritative material exists but is not arranged around first success.
  - Files: `CONTRIBUTING.md`, `docs/integrations/`, `contracts/addons/README.md`, doc-link/example validators.
  - Verify: all commands execute in CI; a contributor reaches the right reference in under two minutes.
- [ ] **DX4 (P1, human: ~1d / Codex: ~2h)** — Compatibility/licensing — Publish the new dual-licensed interoperability subtree and project contract discovery, overlapping supported ranges, deprecation, and pinned boundary-client fixtures through OpenAPI, SDK, CLI, and docs.
  - Surfaced by: Upgrade review — version fields exist but the M11 client migration path is not yet executable.
  - Files: `contracts/interoperability/fasti-provider/v1/`, registry/authored contracts, `CONTRIBUTING.md`, `crates/fasti-api/`, `packages/sdk/`, Nuvio compatibility docs and fixtures.
  - Verify: licence/SPDX/provenance gate passes; minimum and maximum supported client fixtures pass; unsupported or non-overlapping revision fails with exact remediation.
- [ ] **DX5 (P1, human: ~1d / Codex: ~2h)** — Nuvio clients — Run the same provider-client conformance lane from Desktop and TV native test suites.
  - Surfaced by: Completeness — Fasti-only fixtures cannot prove client ergonomics or recovery.
  - Files: Fasti conformance fixture plus upstream Nuvio Desktop/TV test sources.
  - Verify: snapshot/delta/write/delete/ack/recovery and problem parsing pass at one exact contract revision.
- [ ] **DX6 (P1, human: ~4h / Codex: ~1h)** — Evidence — Record cold prerequisites separately and enforce warm TTHW, focused-loop, example-truth, and diagnostic-quality receipts.
  - Surfaced by: Measurement — current developer timing is unmeasured.
  - Files: existing xtask evidence owner, CI conformance job, programme evidence manifest.
  - Verify: three-step <=5-minute warm run, focused p95 <=60 seconds, zero network/credential dependency.

```text
+====================================================================+
|          DEVELOPER EXPERIENCE REVIEW — COMPLETION SUMMARY          |
+====================================================================+
| Product type          | API/service + SDK/integration platform     |
| Mode                  | DX POLISH                                  |
| Primary persona       | OSS media integration developer           |
| Benchmark tier        | Competitive, 2-5 minutes                   |
| Current TTHW          | Undefined; no complete provider path       |
| Target TTHW           | 3 steps, <=5 minutes warm                  |
| Pass 1 Getting started| 4/10 -> 9/10                               |
| Pass 2 API/CLI/SDK    | 6/10 -> 9/10                               |
| Pass 3 Errors         | 7/10 -> 9/10                               |
| Pass 4 Docs           | 5/10 -> 9/10                               |
| Pass 5 Upgrades       | 5/10 -> 9/10                               |
| Pass 6 Tooling        | 5/10 -> 9/10                               |
| Pass 7 Community      | 6/10 -> 9/10                               |
| Pass 8 Measurement    | 3/10 -> 9/10                               |
| Implementation tasks  | 6 P1; all mapped to M1/M10/M11/M12        |
| Deferred TODOs        | 0                                          |
| Unresolved decisions  | 0                                          |
| Overall score         | 5/10 -> 9/10                               |
+====================================================================+
```

## 52. Engineering review

### 52.1 Mode, verdict, and source-grounded architecture

The engineering review used **FULL REVIEW** because the programme crosses the domain, application, store, contracts, API, CLI, Tauri, browser, provider, and upstream native-client boundaries. The first pass scored **6.5/10 — HOLD**. The final plan scores **9/10 — CLEAR FOR SECURITY REVIEW** after resolving every P1 and P2 finding. This remains planning evidence; implementation and runtime gates are still open.

Exact-base findings that control the design:

- Axum currently composes only `Arc<dyn LocalKernel>` and `fastid` constructs the SQLite kernel;
- Tauri directly owns the only concrete provider credentials and transport;
- the current `NuvioOutbox` and Nuvio state models are process-local conformance fixtures, not restart-safe production state;
- the store serializes one SQLite connection, so network work must never hold its lock;
- archive v1/v2 stream inventories are immutable and fail closed on ordering/format drift;
- the legacy metadata-override key has no profile owner;
- the current benchmark harness proves only startup/idle profiles;
- the current Playwright server is a health stub and cannot prove programme behavior.

The amended architecture therefore reuses existing bounded-context rules while adding only two missing concrete infrastructure owners: `fasti-provider-runtime` for shared provider composition and M11a's SQLite synchronization substrate. No broker, second database, dynamic plugin system, Kotlin SDK, test framework, or runtime telemetry is introduced.

### 52.2 Responsibility and coverage map

```text
Existing stable identity/domain rules [EXISTING ★★★]
  -> application commands and profile authorization [PLANNED ★★★] [→E2E]
     -> SQLite repositories + immutable sync journal [PLANNED ★★★] [→E2E]
        -> contracts/API/CLI generated from authored sources [PLANNED ★★★] [→E2E]
           -> browser/Tauri/native clients [PLANNED ★★☆] [→E2E]

fasti-provider-runtime [PLANNED ★★★] [→E2E]
  -> one registry + governed transport + vault-reference port
  -> composed by fastid
  -> composed by Tauri
  -> TMDB / MDBList / Nuvio Cloud / approved add-on origins

M11a durable synchronization substrate [PLANNED ★★★] [→E2E]
  -> M9b MDBList private account state
  -> M11b-c Nuvio Cloud
  -> M11d Fasti provider receipts/deltas
     -> M11e Desktop + M11f TV [PLANNED ★★★] [→E2E]

Approved read projection
  -> public Stremio manifest/catalog/meta [PLANNED ★★★] [→E2E]
  -> authorized local workspace M13a-e [PLANNED ★★★] [→E2E]

Archive v3 -> v4 -> v5 -> v6 -> v7 [PLANNED ★★★] [→E2E]
  projection   Library   Collections  sync     sharing
```

Stars show planned test depth, not current implementation. There is no LLM evaluation surface. Current coverage is strongest for identity, authorization fixtures, archive v1/v2, Nuvio document parsing, and shell accessibility. Production provider composition, durable synchronization, real programme E2E, archive v3-v7, normal/heavy memory envelopes, and native clients remain implementation work.

### 52.3 Failure modes and amendments

| Finding | Resolution in this plan | Required proof |
| --- | --- | --- |
| Fasti-side outbound work had no durable owner. | M11a owns immutable operations, attempts, receipts, acknowledgements, cursors, tombstones, fenced leases, derived outbox, and bounded dispatch. | Kill at every transaction/remote boundary; reconstruct solely from SQLite; no loss or duplicate mutation. |
| Provider behavior was Tauri-only. | M1 creates one concrete `fasti-provider-runtime` composed by both `fastid` and Tauri. | Identical registry/transport policy fixtures in both composition roots; no copied adapters. |
| Archive v3 speculatively froze future state. | v3-v7 land incrementally with each state owner and preserve every prior prefix/order. | Every-version restore/rollback and hostile stream mutations. |
| Legacy overrides lacked a profile owner. | Auto-migrate only one unambiguous owner; otherwise retain a non-editable migration-review item. | Zero/one/multiple-profile, interruption, retry, and unrelated-row equality. |
| Memory limits were prose only. | Existing harness gains canonical normal/heavy drivers and receipt verifier mutations. | Five isolated 15-minute runs where applicable; exact workload/process tree; x86_64 and aarch64. |
| M11 was not reviewable as one unit. | M11a-g separate substrate, Cloud bootstrap, Cloud steady state, provider server, two clients, and cross-client evidence. | Each lane independently activates, releases, rolls back, and reports status. |
| Nuvio RPC stability was assumed. | Treat pinned RPC as a compatibility profile unless a primary stability/terms statement exists. | Pre-activation schema drift probe; incompatible profile fails closed. |
| Search receipts were only called bounded. | Fix 24-hour TTL, 64 KiB cap, digests, replay authorization, and bounded garbage collection. | TTL/size/digest/grant/config/replay/GC negative tests. |
| MDBList reads and account writes shared a milestone. | M9a public/read and M9b private account-state lanes have separate activation, partition, and authority. | Cross-grant denial and failure-containment matrix. |
| Local sharing was one large PR. | M13a-e land discovery, authorization, reads, mutations/deltas, then offline/release evidence. | Active-stream revocation, private-field denial, reconnect, backup/restore. |
| Old/new schema overlap was assumed safe. | Overlap requires exact old-binary read/write evidence. | Reject silent state loss or incompatible writes before deployment. |
| Public Stremio could not satisfy private auth. | It is explicitly public, read-only, non-secret publication; private state uses the authenticated provider contract. | Negative leakage fixtures across payload, cache, error, and logs. |
| Rotating token handoff cannot be cross-system atomic. | Provisional local generation plus CAS activation; ambiguous remote-spent/local-lost state requires reauthorization. | Fault injection at every arrow, concurrent refresh, and lease fencing. |
| Wall-clock merge and numeric-max progress lose valid intent. | Three-way baseline/local/remote comparison; divergent edits become conflict receipts. | Delete/recreate, reset, offline divergence, cursor recovery, and partial batches. |

### 52.4 Test and delivery strategy

The test artifact is [the engineering QA plan](/home/ryan/.gstack/projects/Scrobble-dev-Fasti/ryan-codex-nuvio-metadata-programme-m0-eng-review-test-plan-20260830-004324.md). It enumerates affected routes, interactions, edge cases, durability boundaries, resource workloads, and critical journeys.

Existing Rust unit/Tokio integration, Node test, Playwright, and Axe owners remain. Store tests own migrations, restart, transaction, journal, and archive invariants. Application tests own pure state machines. Provider-runtime tests own transport and secret-order invariants. API tests compose a real disposable `fastid`. Playwright uses that real process for programme journeys. Native Desktop/TV tests use their native HTTP/serialization stacks. Deterministic CI proves behavior; separately authorized live tests prove current TMDB, MDBList, Nuvio Cloud, and native-client interoperability before any end-to-end claim.

Parallel implementation uses isolated worktrees, but only one writer may own a migration number, registry source, generated contract set, SDK surface, or Workbench composition at a time. Shared schema and contract slices land before dependent worktrees rebase from the accepted exact head. Upstream Nuvio clients use separate exact pinned repository branches. No agent edits the authentication plan or another owner's checkout.

### 52.5 Engineering implementation tasks

- [ ] **E1 (P1)** — Runtime — Create and compose `fasti-provider-runtime`; migrate, do not copy, Tauri provider behavior.
- [ ] **E2 (P1)** — Durability — Land M11a journal/cursor/receipt/tombstone/fenced-lease schema, repositories, dispatcher, restart, and clean shutdown.
- [ ] **E3 (P1)** — Migration/archive — Implement safe legacy-override ownership and immutable archive v3-v7, each only with its owning state slice.
- [ ] **E4 (P1)** — Nuvio delivery — Land M11b-g with independent Cloud/provider activation and exact upstream clients.
- [ ] **E5 (P1)** — Evidence — Make all daemon memory profiles and receipt mutations executable on x86_64 and aarch64; baseline UI processes separately.
- [ ] **E6 (P1)** — System QA — Replace health-only proof for new journeys with real disposable-daemon E2E and separately authorized live acceptance.
- [ ] **E7 (P2)** — Boundary hardening — Enforce candidate-receipt lifecycle, pinned-RPC drift probes, M9 separation, M13 slicing, and active stream revocation.
- [ ] **E8 (P2)** — Deployment — Prove old-binary/new-schema read/write overlap, exact-head lineage, rollback, and independently releasable status.

```text
+====================================================================+
|                  ENGINEERING REVIEW — COMPLETION                   |
+====================================================================+
| Mode                  | FULL REVIEW                               |
| Initial verdict       | HOLD, 6.5/10                              |
| Final plan verdict    | CLEAR FOR SECURITY REVIEW, 9/10           |
| P1 amendments         | 6 resolved                                |
| P2 amendments         | 5 resolved                                |
| Existing frameworks   | Rust/Tokio, Node test, Playwright, Axe    |
| New frameworks        | 0                                         |
| Test artifact         | eng-review-test-plan-20260830-004324.md   |
| Implementation tasks  | 8                                         |
| Deferred TODOs        | 0                                         |
| Unresolved decisions  | 0                                         |
+====================================================================+
```

## 53. Decision Audit Trail

<!-- AUTONOMOUS DECISION LOG -->

| Timestamp | Phase | Decision | Rationale | Alternative rejected | Risk | Reversible | User override |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-08-30 | CEO | Hold the complete M0-M13 scope. | The user explicitly prohibited capability cuts, shortcuts, and deferrals. | Reduce to metadata reads or a compatibility shim. | Programme breadth. | Yes, before implementation slices. | `/goal` instruction. |
| 2026-08-30 | CEO | Treat direct Nuvio Cloud sync and the Fasti provider contract as separate required lanes. | Source evidence supports both products and neither replaces the other. | Use Stremio read transport as a write-sync substitute. | Two integration surfaces. | Yes, by independent capability switches. | User selected B and approved all recommendations. |
| 2026-08-30 | CEO | Keep MDBList ratings, catalogs, and account-state operations as separately granted capabilities. | Their data classes, permissions, budgets, and failure effects differ. | One broad MDBList connection toggle. | More explicit setup. | Yes. | User approved recommended answer. |
| 2026-08-30 | CEO | Implement lossless user-supplied Kaptain import/export without bundling third-party data. | This provides real compatibility while preserving provenance and licence boundaries. | Ship copied collection packs. | Some imports may retain disabled sources. | Yes. | User approved recommended answer. |
| 2026-08-30 | CEO | Restore the local shared workspace as M13. | It was explicitly requested and is not equivalent to public publication or Nuvio sync. | Defer local sharing. | Security and privacy surface. | Yes, feature-gated and revocable. | User prohibited deferral. |
| 2026-08-30 | Design | Use one Workbench shell and migrate route ownership instead of adding a second navigation model. | Existing Tabler navigation already owns the product shell. | Standalone programme shell. | Redirect compatibility. | Yes. | Recommended answer authority. |
| 2026-08-30 | Design | Keep Global Search local and add provider fan-out under Explore Search. | The two surfaces have different trust, latency, and mutation expectations. | Make the command palette call remote providers. | Two named Search surfaces. | Yes. | Recommended answer authority. |
| 2026-08-30 | Design | Give candidates and Records durable canonical routes. | Evidence review and saved local identity must survive refresh and direct entry. | Modal-only details. | Route migration. | Yes, with aliases. | User selected durable routes. |
| 2026-08-30 | Design | Keep Record identity, Library state, progress, ratings, and Collection membership separate. | These are distinct domain owners and must not mutate as a side effect of identity resolution. | One Add action that collapses all state. | More explicit actions. | Yes. | Domain constraints and recommended answer authority. |
| 2026-08-30 | Design | Split Nuvio Cloud sync from Nuvio client pairing in navigation and status. | They use different remote parties, authentication, grants, journals, and recovery. | One Nuvio connected badge. | Additional route. | Yes. | Recommended answer authority. |
| 2026-08-30 | Design | Make first-run setup resumable at every step. | Provider failure or offline use must not discard completed work or block local capability. | One all-or-nothing wizard. | Persistent setup state. | Yes. | Approved A+C account/setup pattern. |
| 2026-08-30 | Design | Require preview, loss report, explicit mode, receipt, and rollback for Collection changes. | Import and replacement can otherwise cause silent loss. | Immediate apply after parsing. | Additional confirmation step. | Yes. | Recommended answer authority. |
| 2026-08-30 | Design | Use Tabler components and current Fasti tokens before custom UI. | The repository already has accessible shell, form, table, modal, and status owners. | New component system. | Existing components may need maturation. | Yes. | Project requirement: Tabler first. |
| 2026-08-30 | Design | Reserve final visual approval for implemented browser and assistive-technology evidence. | The generator was unavailable and planning HTML cannot prove runtime behavior. | Claim production approval from wireframes. | Implementation gate remains. | Yes. | Evidence-first policy. |
| 2026-08-30 | DX | Target the competitive three-step, <=5-minute warm first success. | It matches official short-path benchmarks without requiring an unauthorized hosted sandbox or published binary. | Hide cold build time or target an unproven sub-two-minute path. | Toolchain cache variability. | Yes. | User approved recommended answers. |
| 2026-08-30 | DX | Reuse `contracts/addons`, `cargo xtask`, contract types, and loopback fixtures for one `integration check` path. | The required owners already exist; a plugin framework or generator would duplicate them. | Add a new framework, daemon, or global CLI. | xtask scope grows. | Yes. | Ponytail and recommended answer authority. |
| 2026-08-30 | DX | Keep provider manifests declarative and runtime adapters in-repository. | Manifests describe policy and normalization; reviewed Rust owns executable trust-boundary behavior. | Dynamic executable plugins in this programme. | Adapter contributions require repository review. | Yes. | Recommended answer authority. |
| 2026-08-30 | DX | Use existing `--output human|json` and exit codes 0/2/1. | This preserves CLI consistency and separates validation from environment failure. | Add `--format json` or an ad hoc result schema. | Existing xtask output requires maturation. | Yes. | Independent review recommendation. |
| 2026-08-30 | DX | Dual-license only the new original provider interoperability subtree as `Apache-2.0 OR AGPL-3.0-or-later`. | Upstream GPLv3 Nuvio clients and other clients need reusable specs/fixtures; Fasti implementation and SDK stay AGPL. | Copy AGPL implementation/SDK into clients or leave fixture reuse ambiguous. | Licence-scope mistakes. | Yes, before files are published. | Maintainer authority inherited from the user's all-recommended approval. |
| 2026-08-30 | DX | Use native Nuvio HTTP/serialization stacks, not a new Kotlin SDK. | The plain permissive OpenAPI and fixtures are sufficient and both clients already have native stacks. | Add and maintain another SDK. | Some client boilerplate. | Yes. | Ponytail and recommended answer authority. |
| 2026-08-30 | DX | Advertise and test explicit minimum/maximum contract revisions per release with no calendar promise. | Clients can negotiate real overlap and fail clearly without inventing an unsupported policy. | Promise an arbitrary pre-1.0 support duration. | Multiple boundary fixtures. | Yes. | Independent review recommendation. |
| 2026-08-30 | DX | Measure DX locally and in CI with zero runtime telemetry. | Warm/cold timing, fixture truth, errors, and docs can be proven without collecting user behavior. | Add analytics or phone-home code. | Clean-room evidence costs CI time. | Yes. | Project privacy rule and recommended answer authority. |
| 2026-08-30 | Engineering | Add one shared `fasti-provider-runtime` composition root. | The exact base has concrete provider behavior only in Tauri; fastid and Tauri need the same governed registry, transport, vault reference, and budgets. | Copy Tauri providers into Axum or add provider logic to routes. | New workspace crate boundary. | Yes. | Independent review recommendation. |
| 2026-08-30 | Engineering | Add M11a as the Fasti-side durable synchronization substrate. | Process-local Nuvio fixtures and the single SQLite mutex cannot provide restart-safe outbound delivery. | Treat the current in-memory outbox as production or add a broker. | Migration and dispatcher complexity. | Yes. | Independent review recommendation. |
| 2026-08-30 | Engineering | Use immutable incremental archive v3-v7. | Each version now contains only state that exists in its owning slice and preserves every earlier prefix. | Freeze future M5/M7/M11/M13 streams in v3. | More compatibility fixtures. | No after publication; additive before publication. | Ponytail and independent review. |
| 2026-08-30 | Engineering | Retain ambiguous legacy overrides for explicit owner review. | The v10 table has no profile key; copying or discarding would corrupt user intent. | Copy to all profiles or choose an arbitrary profile. | A migration-review state must be exposed. | Yes, before owner selection. | Data-loss prevention. |
| 2026-08-30 | Engineering | Scope daemon memory caps to the `fastid` process tree and baseline UI processes separately. | Existing numbers are plausible daemon budgets but cannot silently include Tauri/WebView/browser variability. | Publish one ambiguous whole-product cap. | Two evidence reports. | Yes. | Independent review recommendation. |
| 2026-08-30 | Engineering | Split M9, M11, and M13 into independently landable units without removing scope. | Public/private data, Cloud/provider lanes, native repositories, and sharing stages have different authorities and rollback boundaries. | One cross-repository mega-PR per milestone. | More PR sequencing. | Yes. | User's full-scope authority plus reviewability requirement. |
| 2026-08-30 | Engineering | Treat pinned Nuvio RPC as a compatibility profile and fail closed on drift. | Source observation does not prove a stable public API promise. | Claim permanent API stability from client code. | Provider changes can block activation. | Yes when primary stability evidence exists. | Independent review recommendation. |
| 2026-08-30 | Engineering | Make Stremio public read-only and reserve private state for the authenticated provider contract. | Standard Stremio clients do not send Fasti authorization headers. | Put a long-lived secret in an add-on URL. | Published fields are intentionally public. | Yes through descriptor revocation. | Protocol evidence and security review. |
| 2026-08-30 | Engineering | Use local provisional token generations and require reauthorization for ambiguous cross-system rotation. | Remote token consumption and local durability cannot be one atomic transaction. | Blind retry or claim exactly-once cross-system rotation. | Rare manual reauthorization. | No for an already spent token. | Data-loss and credential-safety requirement. |
| 2026-08-30 | Engineering | Use three-way synchronization and preserve divergent edits. | Clocks, offline duration, rewatches, and resets make LWW and numeric maximum unsafe. | Wall-clock LWW or max-progress merge. | Visible conflict receipts. | Yes per reviewed conflict. | Domain correctness. |
| 2026-08-30 | Security | Preserve the repository-wide ban on credentials in URLs without provider exceptions. | Upstream logs and tracing are outside Fasti's redaction boundary. | Append query keys only at dispatch. | Query-key-only providers remain unavailable. | Only by a separately approved constitutional change. | Existing `SECURITY.md` and network policy. |
| 2026-08-30 | Security | Quarantine imported URL-bearing configuration until explicit activation. | Lossless round-trip data is not authority to execute attacker-selected configuration. | Treat validated pack fields as active configuration. | One additional activation step. | Yes. | Independent CSO review. |
| 2026-08-30 | Security | Bind streams, cursors, receipts, journals, descriptors, and grants to complete authority and restore generations. | Revocation, expiry, regrant, or restore must not revive prior authority. | Recheck only at request start or bind only client/profile. | More scope digests and invalidation fixtures. | No for stale receipts. | Independent CSO review. |
| 2026-08-30 | Security | Enforce durable synchronization row and byte quotas before mutation. | A valid but unacknowledged client could otherwise exhaust the single SQLite data root. | Rely on request rate limits or time retention. | Bounded retirement UI and sweeper. | Yes. | Independent CSO review. |
| 2026-08-30 | Security | Block archive v3-v7 and M13e on `C3-CRYPTO`. | Structural integrity does not provide confidentiality or safe post-restore authority. | Ship plaintext or structurally signed programme archives. | Dependency on the authentication crypto gate. | No after archive publication. | Canonical authentication plan. |
| 2026-08-30 | Ponytail | Retain the two missing concrete infrastructure owners and add no speculative platform. | One shared provider runtime removes duplicate Tauri/daemon policy; one SQLite journal substrate serves several required lanes. Existing platform features cover every other need. | Copy provider implementations, add a broker, add a plugin runtime, or collapse distinct trust boundaries to reduce file count. | Two focused new owners must be maintained. | Yes before their migrations/contracts publish. | User required full scope and `/ponytail`. |

## 54. CSO security review

Daily full mode reviewed the exact committed base `adbdef3038786b0efb2ec615bce080e3eaa9361f` (tree `a7a1f661ae1b0ef4470ba736d65942f54793d1b0`) and independently threat-modelled this untracked M0 plan. The production-base scan found **0 reportable Critical, High, Medium, or Low vulnerabilities at the 8/10 confidence gate**. This is planning and exact-base evidence, not a substitute for the implementation security gates or a professional external audit.

### Attack surface and trust boundaries

| Surface | Current base | Programme control |
| --- | --- | --- |
| HTTP/API | Public health/integration descriptions plus bootstrap-bound and bearer-authorized application routes | Generated contract, workspace/profile/client/grant checks, typed problems, bounded input. |
| Webhooks | Five authenticated ingress routes | Existing bearer or trusted-proxy-injected authorization; no anonymous receipt path. |
| Provider egress | Google Books and TMDB through governed adapters | Resolve once, authorize every address, pin, proxy-free, redirect-free, credential-after-authorization, exact origin/configuration binding. |
| Desktop IPC | Packaged Tauri content and IPC | Existing CSP and generated/application capability boundaries. |
| Synchronization/sharing | Not yet active | M11 quotas/authority scopes and M13 HTTPS, pairing, streaming, revocation, and restore-generation gates. |
| File/import/archive | No active upload endpoint; existing archive owners | Streaming import bounds, inert URL quarantine, additive authenticated encrypted archives blocked on `C3-CRYPTO`. |

### STRIDE result

| Class | Principal control |
| --- | --- |
| Spoofing | Vault credentials, exact-origin binding, OAuth/device pairing, scoped grants and epochs. |
| Tampering | Immutable evidence/journals, request digests, compare-and-set revisions, authenticated manifests. |
| Repudiation | Correlation IDs, receipts, acknowledgements, and redacted provenance. |
| Information disclosure | Four data classes, vault-only secrets, private-by-default projection, purpose cache partitions. |
| Denial of service | Streaming limits, bounded fan-out/pages/dispatch, row+byte admission quotas, memory gates. |
| Elevation of privilege | Workspace/profile/client/capability authorization before lookup and commit; stream and restore-generation invalidation. |

### Supply-chain and CI evidence

- 476 tracked files, 532 reachable commits, six GitHub Actions workflows, and two Dockerfiles were inspected from exact Git objects.
- All 91 external Actions references use full commit SHAs; no `pull_request_target` workflow exists; both runtime bases are digest-pinned and non-root.
- Root Rust audit: 222 packages, zero advisories or warnings. Desktop: 543 packages; benchmark: 415 packages. The documented Tauri/GTK transitive `RUSTSEC-2024-0429` exception, unmaintained GTK3 family, and yanked `chacha20 0.10.1` remain accepted exact-base supply-chain debt with no verified direct Fasti exploit path.
- Cached advisory data exposed no freshness timestamp, and offline OSV/npm databases were unavailable. npm/OSV vulnerability status is therefore **not verified**, not reported green.
- Historical secret scan processed 458 commits and returned four removed mock-token false positives; current high-confidence secret patterns were zero.
- Global AI skill scanning was not run because it is outside repository scope. The repository contains no local `SKILL.md` package and no product LLM/runtime surface.

### Plan findings resolved

| Priority | Finding | Resolution |
| --- | --- | --- |
| P2 | Query-string credential exception | Removed; query-key-only providers fail unavailable. |
| P2 | Imported URLs could become executable | Inert quarantine, safe-shape rejection, vault references, fresh activation authorization. |
| P2 | Long-lived stream authority incomplete | Browser-safe transport, full scope/epoch binding, bounded recheck, close on every invalidation. |
| P2 | General receipt replay under stale authority | Complete server-derived idempotency scope and non-disclosing mismatch. |
| P2 | Journal/receipt storage unbounded | Pre-mutation row/byte quotas, compacted receipts, bounded sweeper, quarantine/retirement. |
| P2 | Archive lacked crypto/restore dependency | `C3-CRYPTO`, authenticated encryption, restore-generation fencing, disabled/quarantined reactivation. |

Implementation security tasks are the negative tests 53-60 plus the exact-head `/cso` implementation gate. **Security plan verdict: CLEAR FOR PONYTAIL REVIEW, 9/10, 0 unresolved security decisions.**

## 55. Ponytail review

Full-mode complexity review found no removable implementation layer:

- every requested capability is explicit user scope, so YAGNI cannot delete a milestone;
- provider behavior is consolidated into one concrete shared runtime used by two existing composition roots, not a one-implementation interface;
- synchronization uses the existing SQLite writer, HTTPS/SSE, standard/native client stacks, existing generated TypeScript SDK, and current `cargo xtask` rather than a broker, second database, plugin runtime, Kotlin SDK, telemetry service, or new test framework;
- provider-specific translation stays concrete until a second real implementation proves a shared abstraction;
- database constraints and transactions own persistence invariants; existing Tabler/browser features own UI behavior;
- each non-trivial or security-sensitive path has one smallest runnable invariant check, while the wider conformance matrix proves cross-boundary behavior once.

Review output: **Lean already. Ship. Net: -0 lines possible without removing requested scope or a required correctness, security, accessibility, recovery, or evidence control.**

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
| --- | --- | --- | ---: | --- | --- |
| CEO Review | `/plan-ceo-review` | Scope and strategy | 1 | CLEAR | Mode HOLD_SCOPE; 0 critical gaps; full M0-M13 scope retained. |
| Codex Review | `/codex review` | Independent second opinion | 0 | SKIPPED | Running inside Codex; nested Codex pass prohibited by the review preflight. |
| Eng Review | `/plan-eng-review` | Architecture and tests (required) | 1 | CLEAR | 6 P1 and 5 P2 amendments resolved; readiness 6.5/10 -> 9/10; 0 unresolved decisions. |
| Design Review | `/plan-design-review` | UI and UX gaps | 1 | CLEAR | 12 gaps resolved; dual-voice consensus 7/7; readiness 6/10 -> 9/10; 0 unresolved decisions. |
| DX Review | `/plan-devex-review` | Developer experience gaps | 1 | CLEAR | 15 amendments resolved; dual-voice consensus 8/8; readiness 5/10 -> 9/10; TTHW target 3 steps/<=5 minutes; 0 unresolved decisions. |
| Security Review | `/cso` | Exact-base scan and programme threat model | 1 | CLEAR | 0 current reportable vulnerabilities; 6 P2 plan gaps resolved; accepted Tauri/GTK debt retained; npm/OSV status explicitly unverified. |
| Ponytail Review | `/ponytail-review` | Over-engineering and unnecessary-dependency review | 1 | CLEAR | Lean already; 0 removable layers; 0 new frameworks/databases/brokers/runtime dependencies; net -0 lines. |

**VERDICT:** ALL PLANNING GATES CLEARED — M0 APPROVED FOR IMPLEMENTATION.

NO UNRESOLVED DECISIONS
