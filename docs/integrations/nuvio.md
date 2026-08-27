# Nuvio and Fasti integration status

## Current Fasti-side status

Fasti now mounts a dedicated Nuvio occurrence-ingress route:

```text
POST /api/v1/integrations/nuvio/webhook
```

It is part of the isolated production integration router and uses the same authenticated, durable observation service as the local observation API. The route does not bypass Fasti identity, authorization, evidence, receipt, or idempotency rules.

Current upstream Nuvio still needs a Fasti tracking provider before it can submit events directly. Fasti therefore reports this integration as `setup_required`, not `active`, until a compatible Nuvio client is configured and has successfully delivered an event.

## Fasti contract

A Nuvio client sends the provider template below as JSON and supplies one scoped Fasti bearer credential in the `Authorization` header.

```http
POST /api/v1/integrations/nuvio/webhook HTTP/1.1
Host: fasti.example.test
Authorization: Bearer <one-time-issued-client-credential>
Content-Type: application/json

{
  "event_id": "nuvio-session-42-completed-episode-7",
  "observed_at": "2026-08-27T12:30:00Z",
  "occurred_at": "2026-08-27T12:29:58Z",
  "media_type": "Episode",
  "title": "Example episode",
  "identifiers": {
    "imdb": "tt1234567",
    "kitsu": "7442"
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

| Delivery | Fasti result |
|---|---|
| First valid delivery | `committed` plus a durable receipt |
| Same event and same evidence | `replayed` with the original receipt |
| Same event ID with different evidence | `409 idempotency_conflict`; previous state remains |

A transport retry does not create another Chronicle occurrence. A genuinely separate consumption occurrence needs a new source event identity.

## Supported semantic boundary

This route records complete consumption occurrences. It does not treat transient position updates as Chronicle events. State such as progress, saved membership, or current watched/completed projection remains a separate capability and must not be invented inside this adapter.

## Network boundary

The general non-loopback listener remains health-only. Production integrations use the dedicated `integration_router`, which exposes only:

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

Fasti-side conformance must cover at least:

```text
missing bearer -> 401
wrong/revoked bearer -> authentication fails
valid scoped bearer + valid template -> durable commit
same event + same evidence -> original receipt replay
same event + changed evidence -> 409 with no second mutation
invalid media type or identifier -> 422 with no occurrence
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
