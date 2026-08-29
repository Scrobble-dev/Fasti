# Nuvio and Fasti integration status

Fasti and Nuvio do not have a complete tracking or synchronization integration yet.

Fasti has two separate integration primitives: authenticated complete-occurrence ingress, and profile-scoped custom Collections file interchange. Collections configuration is not tracking state, list membership, or Fasti record identity.

Current upstream Nuvio still needs a Fasti tracking provider before it can call that endpoint directly. Its tracking provider registry currently contains Trakt and SIMKL.

## What Fasti implements now

The local Fasti API exposes:

```text
POST /api/v1/integrations/nuvio/webhook
```

It is part of the isolated production integration router and uses the same authenticated, durable observation service as the local observation API. The route does not bypass Fasti identity, authorization, evidence, receipt, or idempotency rules.

- requires a scoped bearer credential;
- accepts requests on the local loopback API, the dedicated integration listener, or through Fasti's explicit trusted HTTPS proxy boundary;
- stores the normalized request as immutable content-addressed evidence;
- authenticates and authorizes the client before mutation;
- derives the Fasti operation identity from the authenticated client, source, and source event identity;
- returns the same durable receipt when the same source event is retried with the same evidence;
- rejects reuse of the same source event identity with different evidence;
- keeps unresolved identity as a valid durable state.

Current upstream Nuvio still needs a Fasti tracking provider before it can submit events directly. Fasti therefore reports this integration as `setup_required`, not `active`, until a compatible Nuvio client is configured and has successfully delivered an event.

## Fasti contract

A Nuvio client sends the provider template below as JSON and supplies one scoped Fasti bearer credential in the `Authorization` header. The current observer credential grants only:

```text
observation_accept
```

The browser workbench cannot create these credentials.

## Custom Collections file interchange

The Workbench can import, export, replace, and clear one Nuvio custom Collections document for the authenticated profile. Both the browser host and trusted Desktop host use the same application contract.

```text
GET    /api/v1/profile/nuvio-collections
PUT    /api/v1/profile/nuvio-collections
DELETE /api/v1/profile/nuvio-collections
```

`PUT` accepts NuvioTV's bare top-level JSON array. Fasti normalizes the document before a transactional upsert and returns the stored array in a response wrapper. The import is bounded to 4 MiB, 64 collections, 1,024 folders, 4,096 sources, 200,000 JSON nodes, depth 16, and 8 KiB strings. Invalid source entries are dropped under Nuvio's source-selection rules; duplicate collection IDs keep the final value at the first position. Unknown extension fields are retained.

The document belongs to `(workspace_id, profile_id)`. Reading, replacing, and clearing it requires the corresponding `profile_state_read` or `profile_state_write` scope. Workspace revision changes only when the stored document changes. Imported image, add-on, and catalog URLs are inert data: Fasti does not request them or render remote content from them.

Compatibility is pinned to [`NuvioMedia/NuvioTV` commit `3f44c404`](https://github.com/NuvioMedia/NuvioTV/tree/3f44c404a73a6152992bffa4538fcf8d42427183). The implementation follows its collection model and `CollectionsDataStore` decode behavior. A supplied 2026-08-27 export (SHA-256 `30d4e4c3041def5f9d280a1bf47e5f6ac3499290e7b07c5709f64e63bef00242`) verified the full path with 16 collections, 601 folders, and 3,059 sources.

Custom Collections interchange does not pair a Nuvio device, send progress, publish a Fasti catalog, or promote Nuvio provider IDs into Fasti identity. Export the document separately before clearing it; archive v2 remains unchanged for compatibility.

## Request example

Use a stable `source_event_id` for the source event. Reuse it when delivery is retried.

```http
POST /api/v1/integrations/nuvio/webhook HTTP/1.1
Host: fasti.example.test
Authorization: Bearer <one-time-issued-client-credential>
Content-Type: application/json

{
  "source_event_id": "nuvio-session-42-completed-episode-7",
  "observed_at": "2026-08-27T12:30:00Z",
  "occurred_at": "2026-08-27T12:29:58Z",
  "item_type": "episode",
  "title": "Example episode",
  "completed": true,
  "provider_ids": {
    "imdb": "tt1234567",
    "kitsu": "7442"
  },
  "series_provider_ids": {
    "imdb": "tt0000000"
  }
}
```

Required behavior:

- authenticate before parsing provider-specific content;
- reject bodies larger than the configured bounded ingress limit;
- reject unknown or incomplete media identities instead of guessing;
- derive operation identity from the authenticated client and stable source event identity;
- store immutable evidence before a successful receipt can reference it;
- replay the original durable receipt for the same event and evidence;
- return `409 idempotency_conflict` when the same event identity is reused with changed evidence;
- keep unresolved identity as valid durable state;
- never put the bearer credential in the body, URL, query string, logs, screenshots, fixtures, or exports.

Create one scoped API client per Nuvio installation or device through **Connections → API clients** in a trusted packaged Fasti host. Independent credentials make revocation and audit boundaries explicit.

## Event identity and retry

Create the Nuvio `event_id` before the first send and keep it stable until the event is acknowledged.

| Delivery                                 | Result                                          |
| ---------------------------------------- | ----------------------------------------------- |
| First valid delivery                     | `committed` and a durable receipt               |
| Same source event and same evidence      | `replayed` with the original receipt            |
| Same source event and different evidence | `409 idempotency_conflict`; prior state remains |

A transport retry does not create another Chronicle occurrence. A genuinely separate consumption occurrence needs a new source event identity.

## Supported semantic boundary

This route records complete consumption occurrences. It does not treat transient position updates as Chronicle events. State such as progress, saved membership, or current watched/completed projection remains a separate capability and must not be invented inside this adapter.

## Network boundary

The general non-loopback listener remains health-only. Production integrations use either the loopback listener, the trusted HTTPS reverse-proxy boundary, or the dedicated `integration_router`, which exposes only:

- health;
- integration status;
- authenticated production provider adapters.

It does **not** expose bootstrap, generic Record mutation, or the generic observation endpoint. Deployments that publish the integration listener must still provide the repository-required protected transport, origin/network policy, secret handling, and reverse-proxy controls. Do not widen the general local application router to obtain remote access.

## Nuvio client work

A compatible Nuvio implementation must use Nuvio's normal tracking-provider abstraction rather than a parallel Fasti-only state machine. It needs:

1. a Fasti provider entry in the tracking registry;
2. protected storage for the Fasti node address and scoped credential;
3. a durable local outbox;
4. stable event identity assigned before enqueue;
5. bounded retry with backoff and jitter;
6. retirement of an outbox item only after a durable Fasti receipt;
7. visible pending, delivered, blocked, and rejected status;
8. secret-safe logs and diagnostics.

Client work belongs in the Nuvio repository and must use its own release and review authority. The Fasti repository must not claim the client as active until that compatible build exists and a real event has been verified.

The current `NuvioMedia/NuvioTV` `dev` branch defines these tracking providers:

```text
Trakt
Simkl
```

The current scrobble coordinator fans occurrence updates to enabled providers in that registry. There is no Fasti provider in the current upstream registry.

Source files checked during this implementation:

- `app/src/main/java/com/nuvio/tv/core/tracking/TrackingProvider.kt`
- `app/src/main/java/com/nuvio/tv/core/tracking/TrackingScrobbleCoordinator.kt`
- `app/src/main/java/com/nuvio/tv/domain/model/Collection.kt`
- `app/src/main/java/com/nuvio/tv/data/local/CollectionsDataStore.kt`

Repository:

```text
https://github.com/NuvioMedia/NuvioTV
```

## What a first Nuvio client integration still needs

The first useful Nuvio slice is one-way and durable:

1. Add a Fasti tracking provider through the normal Nuvio tracking abstraction.
2. Store Fasti node identity and the device credential in Nuvio's protected credential mechanism.
3. Keep a durable Nuvio-side outbox.
4. Create a stable source event identity before the first send.
5. Submit a completed occurrence to the Fasti observation contract.
6. Keep the same source event identity across timeout and reconnect retries.
7. Retire the outbox item only after Fasti returns its durable receipt.
8. Show pending, delivered, blocked, and rejected state to the user.
9. Keep Nuvio's own operation independent of Fasti availability.

This slice must not invent partial progress support. It can submit complete occurrences only until Fasti has a separate progress capability.

## Later work

These are separate implementation gates, not properties of the current route:

- secure LAN discovery and pairing;
- partial progress and resume-state synchronization;
- exact watched-state synchronization;
- saved/watchlist state;
- deletion tombstones;
- snapshot and ordered delta recovery;
- reconciliation and diagnostics;
- catalog publication and collection/list membership synchronization;
- metadata projections;
- two-way synchronization.

The application remains useful without any of these integrations.

## Verification

Fasti-side conformance must cover at least:

```text
missing bearer -> 401
wrong/revoked bearer -> authentication fails
valid scoped bearer + valid template -> durable commit
same event + same evidence -> original receipt replay
same event + changed evidence -> 409 with no second mutation
invalid HTTP media type -> 415 with no occurrence
invalid media identity -> 422 with no occurrence
oversized body -> bounded rejection
integration router -> Nuvio route present
integration router -> bootstrap/records/generic observation absent
```

Run the repository gate on the exact head before publishing a production compatibility claim:

```bash
cargo xtask test pr
pnpm test:ui
```

OpenAPI documents the HTTP route. AsyncAPI documents event/receipt transport semantics and must not duplicate this synchronous HTTP ingress as a fictitious message channel.
