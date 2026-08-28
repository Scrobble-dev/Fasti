# Records Read Model

## Current status

This document describes the Record metadata write and read paths. The trusted Desktop host can search Google Books or TMDB, fetch the selected provider item again, and submit provider-neutral claims through `ProviderMetadataPort`. The SQLite store commits the Record, external identifier, and initial claims atomically, or appends refreshed claims to an existing Record. `IdentityPort::list_records` projects that state through the `list_records` Tauri command and the bearer-authenticated `GET /api/v1/records` HTTP route.

## 1. Why this exists

`Record` (`fasti-domain::identity`) is pure identity: a `RecordId`, a `Grain`, and a status. It never carries a title, poster, or provider coordinate -- that is deliberate, not an omission. Display fields live in `fasti-domain::metadata` as `FieldClaim` (one provider's claim about one field) and `FieldOverride` (a user-owned value), resolved deterministically by `resolve_field()` into a `ResolvedField` with a `FieldResolutionTier` explaining which claim won.

Provider calls remain outside the local write transaction. A network failure cannot partially create a Record or remove existing Chronicle state.

## 2. Persistence

Two tables, added in `schema.rs`'s `migrate_v6`:

- `metadata_field_claims` -- one row per `(record_id, field_key, source, fetched_at)`. Every claim a provider ever supplied is kept as history, never overwritten in place; `resolve_field()` needs the full set to pick the right tier, including expired claims for the `LastKnownGood` fallback.
- `metadata_field_overrides` -- one row per `(record_id, field_key)`. A user override is not versioned history; a later override simply replaces the earlier one.

Both tables carry `workspace_id` directly (matching `external_identifiers`'s convention) so queries scope without an extra join, and both participate in the existing `workspace_revisions` trigger machinery.

`fasti-store::metadata` owns `write_field_claim`, `write_field_override`, `load_field_claims`, and `load_field_override`. Its `ProviderMetadataPort` implementation validates a bounded set of unique field keys, requires every claim source to match the attached provider namespace, and writes through one immediate transaction. An invalid field, identifier, namespace, or authorization proof rolls back the complete mutation.

The trusted Desktop adapter is the current production caller. Google Books supplies book title, description, publication year, and thumbnail claims. TMDB supplies movie or TV title, original title, overview, release year, and poster claims. Search responses are only choices: the host fetches the exact provider ID again before it constructs claims or opens the transaction.

When the exact item contains a poster, the Desktop host downloads it through a
separate `metadata.artwork` policy grant before the metadata write. It retains
the remote URL as provider evidence, but the Desktop read projection adds an
optional owner-only cache path only after revalidating the cached file. The web
adapter converts that path to Tauri's narrowly scoped local asset URL and
replaces the remote poster value before any component renders it. The public
HTTP projection remains provider-claim data and has no filesystem field.

## 3. The query capability

`IdentityPort::list_records` (`fasti-application::kernel`) takes a `ListRecordsQuery` (workspace-scoped access, no filters yet) and returns `Vec<RecordSummary>`. For each active Record in the workspace (bounded to 500 rows, no cursor pagination yet):

1. Resolve `core.title`, `core.original_title`, `core.overview`, `core.release_year`, and `core.poster_url` via `resolve_field()`, using each Record's persisted claims and override. No preferred-provider/locale configuration exists yet, so resolution falls through to `FallbackProviderClaim` -> `LastKnownGood` -> `Empty`.
2. Load external identifiers in deterministic namespace/grain/value order.
3. Load the most recent `Occurrence` touching the Record (by insertion order) and its latest `Interpretation` state, if any, as `RecordActivity`.

A Record with zero claims and zero occurrences still returns a valid `RecordSummary` row: resolved fields use the `Empty` tier, identifiers are empty, and `latest_activity` is `None`. A local-only Record with no metadata is a first-class, valid case.

The canonical field keys are owned once in `fasti-domain::metadata`; provider adapters do not invent UI-specific field names.

## 4. Tauri and HTTP surfaces

`apps/desktop/src-tauri/src/records.rs` exposes `list_records`, `create_provider_record`, and `apply_provider_metadata`, following the same authenticated local-kernel pattern as the other Desktop commands. `track_provider_candidate` and `apply_provider_metadata` first perform the authorized provider read in `providers.rs`, cache validated artwork when present, then call these local operations. They make no daemon HTTP round-trip.

Discover creates a Record only through `track_provider_candidate`. The existing
`CreateRecordView.record_id` result returns to the candidate row and remains
visible after success. The UI does not fall back to separate `create_record`,
`register_namespace`, and `attach_identifier` calls because that sequence is not
the provider operation's atomic boundary and can report a partial result.
`provider:kind:provider_id` identifies transient search candidates, including
TMDB movie and show IDs that share the same number. It is not canonical Record
identity.

`crates/fasti-api/src/records.rs` exposes the same query as `GET /api/v1/records`, bearer-authenticated the same way as every other production-runtime route. This is the surface the browser-hosted web app uses -- the Tauri command above is desktop-only and never reachable from a browser tab.

The wire `RecordSummary` view carries `grain: Grain` unchanged -- `Grain` is identity granularity (Work/Series/Release/Season/Episode/Film/...), distinct from the frontend's display-oriented `MediaKind` (movie/show/anime/book/...). `record-projection.ts` owns that presentation mapping and keeps provider identifiers separate from Fasti Record IDs.

No public metadata mutation route or `get_record` detail command exists. The Workbench reloads the bounded list after a trusted-host mutation. Add a single-record query only when the list contract becomes measurably insufficient.

## 5. Capability registry

`identity.record.list` is registered in `contracts/registry/v1/capabilities.yaml` as `contract_body: b1`, `lifecycle.contract_state: finalized`, `lifecycle.runtime_availability: implemented`, under `surface_profile: b1_records`. That profile declares `http_openapi: required`, so the additive resolved fields and identifier array are generated into the production OpenAPI and TypeScript SDK. The Desktop-only provider mutations do not create a public HTTP route, event, JSON-LD entity, or public capability.
