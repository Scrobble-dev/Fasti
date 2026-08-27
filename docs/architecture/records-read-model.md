# Records Read Model

## Current status

This document describes the read path that lists Records for display: `metadata_field_claims`/`metadata_field_overrides` persistence in `fasti-store`, the `IdentityPort::list_records` query, the authenticated HTTP `GET /api/v1/records` route, and the matching Tauri command.

The HTTP and Tauri surfaces call the same application capability. Neither surface owns a separate record rule. Provider adapters can add evidence and metadata claims through their owning application boundaries; the record read model remains provider-neutral.

## 1. Why this exists

`Record` (`fasti-domain::identity`) is pure identity: a `RecordId`, a `Grain`, and a status. It never carries a title, poster, or provider coordinate. Display fields live in `fasti-domain::metadata` as `FieldClaim` (one provider's claim about one field) and `FieldOverride` (a user-owned value), resolved deterministically by `resolve_field()` into a `ResolvedField` with a `FieldResolutionTier` explaining which claim won.

Before this read path, `metadata.rs` was pure domain logic with no SQLite persistence and `IdentityPort` had no record-list query. The current implementation closes that gap without making metadata the identity of a Record.

## 2. Persistence

Two tables, added in `schema.rs`'s metadata migration, own display claims:

- `metadata_field_claims` -- provider/source claims with provenance and freshness. Previous claims remain available for last-known-good resolution.
- `metadata_field_overrides` -- explicit user-owned values for a record field.

Both tables carry `workspace_id` so authorization remains explicit at the storage boundary and both participate in workspace revision tracking.

Store functions in `fasti-store::metadata` load and write these claims. A provider adapter does not bypass this model by putting provider data directly on `Record`.

## 3. The query capability

`IdentityPort::list_records` (`fasti-application::kernel`) takes a `ListRecordsQuery` with an authenticated workspace/profile context and returns `Vec<RecordSummary>`. The current bounded page contains **active** Records only, with a hard maximum of 500 rows. Cursor pagination remains a later change when real library evidence requires it.

For each row:

1. Resolve `core.title` and `core.poster_url` through `resolve_field()` and the persisted claims/override model.
2. Load the most recent applicable `Occurrence` and its latest `Interpretation` state when one exists.

A Record with no metadata claims or occurrences is still valid. Its fields resolve to the `Empty` tier instead of disappearing or becoming an error.

The HTTP response currently returns `status: "active"` because the storage query deliberately selects only `records.status = 'active'`. That value is an invariant of this specific read model, not a fabricated lifecycle projection. When non-active lifecycle states become part of a real list use case, the query and DTO must expand together.

## 4. Public surfaces

### HTTP

The loopback application API mounts:

- `GET /api/v1/records`
- `POST /api/v1/records`
- `POST /api/v1/records/identifiers`
- `POST /api/v1/namespaces`

The routes require scoped bearer credentials. They are documented by generated OpenAPI and use the same application commands/queries as the desktop host.

### Tauri

`apps/desktop/src-tauri/src/records.rs` exposes the same record capabilities through trusted host commands. Authentication remains host-side; raw persistent credentials do not enter the Svelte component tree.

The wire `RecordSummary` carries `grain` as identity granularity (`Work`, `Series`, `Release`, `Season`, `Episode`, `Film`, and related values). `packages/ui/src/record-projection.ts` owns the display projection into frontend `MediaKind` values. It must not reinterpret provider identity.

## 5. Capability registry and contract ownership

`identity.record.list` is registered in `contracts/registry/v1/capabilities.yaml` with:

- `contract_body: b1`
- `runtime_body: b2`
- `contract_state: finalized`
- `runtime_availability: implemented`
- `surface_profile: b1_records`
- `identity_read` scope

The `b1_records` surface requires the HTTP/OpenAPI projection as well as the generated SDK contract. Contract verification must fail when the registry, route, schema, SDK, examples, or documentation drift from one another.

No AsyncAPI channel is added for a synchronous record query. AsyncAPI documents message/event transports, not every HTTP capability.
