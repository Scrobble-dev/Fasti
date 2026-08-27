# Nuvio and Fasti integration status

Fasti and Nuvio do not have a complete production integration yet.

Fasti now has one real integration primitive: an authenticated local endpoint that can accept a complete consumption occurrence and return a durable receipt.

Current upstream Nuvio still needs a Fasti tracking provider before it can call that endpoint directly. Its tracking provider registry currently contains Trakt and SIMKL.

## What Fasti implements now

The local Fasti API exposes:

```text
POST /api/v1/observations
```

The route:

- requires a scoped bearer credential;
- accepts only on the local loopback API;
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

| Delivery | Result |
|---|---|
| First valid delivery | `committed` and a durable receipt |
| Same source event and same evidence | `replayed` with the original receipt |
| Same source event and different evidence | `409 idempotency_conflict`; prior state remains |

A retry is not a rewatch.

A real repeat consumption needs a new source event identity.

## Partial progress is not accepted by this route

`POST /api/v1/observations` accepts only a complete, durable consumption occurrence.

A request with `progress_percent` below 100 is rejected. Fasti does this deliberately: an incomplete occurrence must never become false Chronicle history.

A record of partial progress needs a separate capability and persistence contract, not this route.

## Network boundary

The durable local API is loopback-only today.

```text
127.0.0.1 / ::1
        │
        ▼
production local Fasti API
```

A non-loopback Fasti listener exposes the health surface only. The occurrence route is not available to another device on the LAN.

Do not work around this by binding the local mutation router to `0.0.0.0` or a LAN address.

Remote Nuvio support requires its own secure listener and pairing work: authenticated node identity, explicit user approval, device-bound grants, TLS or an equivalent protected transport, revocation, replay tests, and network-policy evidence.

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
- catalogues and Collections;
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
