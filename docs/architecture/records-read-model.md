# Records Read Model

## Current status

This document describes the read path that lists Records for display: `metadata_field_claims`/`metadata_field_overrides` persistence in `fasti-store`, the `IdentityPort::list_records` query, and the `list_records` Tauri command. No HTTP route exists; this is Tauri-command-only, matching how `reviews`/`provider-credential` capabilities were wired. No provider adapter exists yet to write real claims -- every Record currently resolves with empty title/poster fields unless a test or a future adapter writes claims directly.

## 1. Why this exists

`Record` (`fasti-domain::identity`) is pure identity: a `RecordId`, a `Grain`, and a status. It never carries a title, poster, or provider coordinate -- that is deliberate, not an omission. Display fields live in `fasti-domain::metadata` as `FieldClaim` (one provider's claim about one field) and `FieldOverride` (a user-owned value), resolved deterministically by `resolve_field()` into a `ResolvedField` with a `FieldResolutionTier` explaining which claim won.

Before this work, `metadata.rs` was pure domain logic with no SQLite persistence, and `IdentityPort` had no read method at all -- there was no way to list Records for a UI. This document covers the layer that closes that gap.

## 2. Persistence

Two tables, added in `schema.rs`'s `migrate_v6`:

- `metadata_field_claims` -- one row per `(record_id, field_key, source, fetched_at)`. Every claim a provider ever supplied is kept as history, never overwritten in place; `resolve_field()` needs the full set to pick the right tier, including expired claims for the `LastKnownGood` fallback.
- `metadata_field_overrides` -- one row per `(record_id, field_key)`. A user override is not versioned history; a later override simply replaces the earlier one.

Both tables carry `workspace_id` directly (matching `external_identifiers`'s convention) so queries scope without an extra join, and both participate in the existing `workspace_revisions` trigger machinery.

Store functions (`fasti-store::metadata`, private to the crate): `write_field_claim`, `write_field_override`, `load_field_claims`, `load_field_override`. The write functions have no production caller yet -- writing real claims is a provider adapter's job, and building a provider adapter that fetches metadata over the network is explicitly a separate, later task. The read functions are the ones `list_records` uses today.

## 3. The query capability

`IdentityPort::list_records` (`fasti-application::kernel`) takes a `ListRecordsQuery` (workspace-scoped access, no filters yet) and returns `Vec<RecordSummary>`. For each active Record in the workspace (bounded to 500 rows, no cursor pagination yet):

1. Resolve `core.title` and `core.poster_url` via `resolve_field()`, using each Record's persisted claims and override. No preferred-provider/locale configuration exists yet, so resolution always falls through to `FallbackProviderClaim` -> `LastKnownGood` -> `Empty`.
2. Load the most recent `Occurrence` touching the Record (by insertion order) and its latest `Interpretation` state, if any, as `RecordActivity`.

A Record with zero claims and zero occurrences still returns a valid `RecordSummary` row: `title`/`poster` resolve to the `Empty` tier (not an error, not a skipped row), and `latest_activity` is `None`. A local-only Record with no metadata is a first-class, valid case.

`core.title` and `core.poster_url` are the two canonical `FieldKey` values this read path resolves (`fasti_domain::{TITLE_FIELD_KEY, POSTER_FIELD_KEY}`). `metadata.rs`'s own doc comment cites `core.title` as the canonical example; `core.poster_url` follows the same convention.

## 4. Tauri surface

`apps/desktop/src-tauri/src/records.rs` exposes `list_records`, following `reviews.rs`'s exact shape: `authenticate()`/`require_access()` from `setup.rs`, `DesktopProblem` error mapping, no HTTP round-trip. Registered in `lib.rs`'s `invoke_handler!`.

The wire `RecordSummary` view carries `grain: Grain` unchanged -- `Grain` is identity granularity (Work/Series/Release/Season/Episode/Film/...), distinct from the frontend's display-oriented `MediaKind` (movie/show/anime/book/...). A later frontend-wiring pass owns the `Grain` -> `MediaKind` projection; this task deliberately does not invent one.

No `get_record` detail-view command exists yet -- the current frontend need is list views (Library/Discover/Chronicle), not a per-record detail fetch. Add one when a detail view actually needs a single-record lookup that `list_records` doesn't already cover.

## 5. Capability registry

`identity.record.list` is registered in `contracts/registry/v1/capabilities.yaml` as `contract_body: b2`, `runtime_availability: fixture_only` -- the same precedent `ConfigureListener` set for a capability whose runtime body owns real behavior but which is reachable only through the Tauri desktop path today, not a routed HTTP operation. No OpenAPI/AsyncAPI path was added; this stays Tauri-command-only.
