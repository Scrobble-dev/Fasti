# Nuvio and Fasti integration status

Fasti and Nuvio do not have a complete tracking or synchronization integration yet.

Fasti has two separate integration primitives: authenticated complete-occurrence ingress, and profile-scoped custom Collections file interchange. Collections configuration is not tracking state, list membership, or Fasti record identity.

Current upstream Nuvio still needs a Fasti tracking provider before it can call that endpoint directly. Its tracking provider registry currently contains Trakt and SIMKL.

## What Fasti implements now

The local Fasti API exposes:

```text
POST /api/v1/observations
```

The route:

- requires a scoped bearer credential or an authenticated browser session;
- accepts requests on the local loopback API or through Fasti's explicit trusted HTTPS proxy boundary;
- stores the normalized request as immutable content-addressed evidence;
- authenticates and authorizes the client before mutation;
- derives the Fasti operation identity from the authenticated client, source, and source event identity;
- returns the same durable receipt when the same source event is retried with the same evidence;
- rejects reuse of the same source event identity with different evidence;
- keeps unresolved identity as a valid durable state.

An external observer credential can be created from **Connections → API clients** in the trusted packaged Fasti host. Create one credential per client or device so it can be revoked independently.

The current observer credential grants only:

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
POST /api/v1/observations HTTP/1.1
Host: 127.0.0.1:8420
Authorization: Bearer <one-time-issued-client-credential>
Content-Type: application/json

{
  "kind": "consumption_occurrence",
  "source": "nuvio",
  "source_event_id": "session-42:stop:episode-7",
  "observed_at": "2026-08-26T18:10:00Z",
  "occurred_at": "2026-08-26T18:09:58Z",
  "target_grain": "episode",
  "identifiers": [
    {
      "namespace": "imdb.title",
      "grain": "series",
      "value": "tt1234567"
    },
    {
      "namespace": "kitsu.anime",
      "grain": "release",
      "value": "7442"
    }
  ],
  "title": "Example episode",
  "progress_percent": 100,
  "position_seconds": 1440,
  "duration_seconds": 1440
}
```

The complete normalized request is evidence. Do not place the bearer credential in the body, URL, query string, logs, screenshots, fixtures, or export files.

## Retry behavior

A source retry must keep these values stable:

```text
authenticated client
source
source_event_id
```

Fasti derives its operation identity from that tuple.

Expected results:

| Delivery                                 | Result                                          |
| ---------------------------------------- | ----------------------------------------------- |
| First valid delivery                     | `committed` and a durable receipt               |
| Same source event and same evidence      | `replayed` with the original receipt            |
| Same source event and different evidence | `409 idempotency_conflict`; prior state remains |

A retry is not a rewatch.

A real repeat consumption needs a new source event identity.

## Partial progress is not accepted by this route

`POST /api/v1/observations` accepts only a complete, durable consumption occurrence.

A request with `progress_percent` below 100 is rejected. Fasti does this deliberately: an incomplete occurrence must never become false Chronicle history.

A record of partial progress needs a separate capability and persistence contract, not this route.

## Network boundary

The durable API is available on loopback. Its authenticated non-bootstrap routes can also run behind Fasti's explicit trusted HTTPS proxy boundary.

```text
127.0.0.1 / ::1
        │
        ▼
production local Fasti API
```

Do not expose the loopback router directly on `0.0.0.0` or a LAN address. Remote Nuvio client support still requires pairing and device-grant work: authenticated node identity, explicit user approval, device-bound grants, revocation, replay tests, and network-policy evidence.

## Current upstream Nuvio boundary

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

For the current Fasti-side ingress, verify at least:

```text
missing bearer -> 401
valid scoped bearer -> durable commit
same source event + same evidence -> original receipt replay
same source event + changed evidence -> 409 with no mutation
partial progress -> 422 with no occurrence
non-loopback health router -> observation route absent
revoked API client -> authentication fails
```

Run the repository gate on the exact head before changing this document from implementation status to a release claim:

```bash
cargo xtask test pr
```
