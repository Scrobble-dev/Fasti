# Product Requirements Document and Programme Plan
# Floppy–Nuvio Integration and Declarative Interoperability

**Programme:** Floppy–Nuvio interoperability  
**Status:** Implementation in progress; Release 1.0 security gates remain open  
**Target repository:** `dannyvfilms/Floppy`  
**Integration references:** `NuvioMedia/NuvioTV`, `NuvioMedia/self-host`  
**Delivery branch:** `plan/nuvio-integration-reviewed-2026-08-15`  
**Pull request:** #791

---

## 1. Goal

Make Floppy a reliable state and interoperability service for media clients without making Floppy a playback engine or coupling it to one provider implementation.

Release 1.0 is limited to the state needed for safe tracking interoperability:

- scoped third-party credentials;
- playback progress;
- saved-library/watchlist membership;
- exact watched state needed for consistency;
- explicit reset and removal;
- retry-safe delivery;
- ordered incremental changes;
- loop prevention;
- reconciliation;
- unresolved-item reporting;
- diagnostics;
- offline/reconnect behavior;
- compatibility and performance evidence.

Later work can add read-only catalogs, declarative add-ons, portable collection descriptors, metadata projections, and supported Nuvio self-host pairing.

---

## 2. Design constraints

1. Keep existing Floppy API routes and user data compatible unless an additive contract is required.
2. Keep provider-specific parsing and source semantics at the provider boundary.
3. Put a rule in one shared location only when more than one proven caller needs the same behavior.
4. Use exact external identifiers or verified translations for authoritative writes. Do not write state from title-only matches.
5. Do not create a second database or a separate integration service for this programme.
6. Do not depend on Nuvio database tables, Supabase service-role credentials, internal SQL function names, or raw Nuvio passwords.
7. Progress, watched state, history, saved-library membership, lists, Nuvio Collections, add-ons, plugins, and provider metadata remain separate concepts.
8. A missing remote row, empty page, cache miss, timeout, or provider failure is not a delete.
9. Remote add-on data is untrusted. Floppy does not execute downloaded add-on or plugin code.
10. Core state transitions must remain callable without Docker-specific paths and without making Redis or Celery the source of truth.

---

## 3. Shared vocabulary

| Term | Meaning | Safety rule |
|---|---|---|
| **PlaybackProgress** | Durable resume position for one user and media item. | Do not treat it as history. |
| **Watched state** | Current exact completion state. | Do not infer missing episodes from a series-level signal. |
| **Playback occurrence** | A durable viewing event. | A legitimate rewatch is a new occurrence, not a duplicate delivery. |
| **Saved-library membership** | User intent to save or plan an item. | Do not treat it as a Nuvio Collection. |
| **IntegrationToken** | Named third-party credential with explicit scopes. | Store only a high-entropy token digest and safe display prefix. |
| **IntegrationEventReceipt** | Durable record of one idempotent request key and payload result. | Reserve before protected mutation; never execute a known duplicate twice. |
| **PlaybackProgressChange** | Ordered append-only progress change. | Use a server sequence, not wall-clock time, as the transport cursor. |
| **Tombstone** | Explicit delete event retained for incremental clients. | Do not infer deletion from absence. |
| **Declarative add-on** | Remote HTTP manifest and resources. | Validate, bound, and cache as untrusted data. Never execute it. |
| **Plugin** | Executable extension code. | Outside this programme's automatic sharing model. |
| **Metadata projection** | Normalized provider/add-on metadata with provenance and freshness. | Provider data remains a projection, not user-owned truth. |

---

## 4. Current implementation state

The following behavior already exists on `latest` from merged work and must be treated as the compatibility baseline:

- durable playback progress API (#429);
- provider-prefixed Stremio identity behavior (#619);
- Stremio watched-state/history correction (#723);
- third-party/Kodi API behavior (#417);
- initial `IntegrationToken` model and authentication support;
- initial `IntegrationEventReceipt` and idempotency integration;
- AsyncAPI 3.0 contract surface and schema viewer.

Security review of the initial token and receipt implementation found two release-blocking gaps:

1. scoped integration tokens authenticated successfully but endpoint scopes were not enforced by the default API permission configuration;
2. the original receipt flow executed the protected mutation before inserting the unique receipt, allowing concurrent requests or a timeout-after-mutation window to execute the same operation more than once.

PR #791 now hardens these boundaries by:

- enforcing a deny-by-default route/scope map for integration tokens while keeping legacy account tokens compatible;
- recording bounded `last_used_at` metadata;
- reserving the receipt before the protected mutation;
- rejecting oversized or control-character idempotency keys;
- retaining an incomplete reservation after an uncertain crash so the retry fails closed instead of applying again.

Remaining security work is listed in the programme war-game document and must be resolved or explicitly accepted before the affected release gate.

---

## 5. State and synchronization rules

### 5.1 Snapshot and incremental changes

For each user, bound client/profile, and resource:

1. obtain server cursor `C0`;
2. fetch a bounded snapshot;
3. merge it while preserving unacknowledged local writes;
4. fetch ordered changes after `C0`;
5. apply one page transactionally;
6. persist the next cursor only after that page commits;
7. repeat until no more changes remain;
8. if the cursor expired, start from a new snapshot;
9. if the remote side fails, keep the last valid local state.

Use a monotonic server sequence for transport ordering. Client timestamps are observation metadata, not cursor authority.

### 5.2 Delete rules

Delete only from:

- an explicit removal/tombstone; or
- a user-approved authoritative reconciliation that shows a preview first.

Do not delete from:

- absence from a partial page;
- empty API response;
- timeout;
- provider error;
- cache miss;
- stale cache entry.

### 5.3 Idempotency

Delivery identity is separate from media occurrence identity.

Required behavior:

- reserve the idempotency key before the protected mutation;
- bind the receipt to the authenticated user and, when the client-identity migration lands, the authenticated client namespace;
- store a canonical payload digest;
- same key + same payload returns the recorded result;
- same key + changed payload returns `409 Conflict`;
- a reservation with an unknown final outcome blocks automatic replay until state is checked;
- a distinct occurrence remains a valid rewatch.

### 5.4 Progress

- ordinary updates can advance progress;
- a lower value requires explicit seek/reset intent or an approved conflict policy;
- completion cannot be silently undone by ordinary progress;
- explicit undo can change completion;
- out-of-order writes cannot silently replace newer state.

### 5.5 Origin and loop prevention

Origin must come from the authenticated client binding. Caller-supplied origin data is not authoritative.

A client can recognize its reflected change and acknowledge it without writing the same state back again.

---

## 6. Work packages on PR #791

These are sequential work packages on one branch. They are not separate GitHub pull requests.

### PR0 — Evidence and conformance baseline

- Current behavior fixtures.
- Regression coverage for completed #619, #723, #429, and #417 behavior.
- Current Nuvio source references.
- No intentional compatibility break.

### PR1 — Scoped credentials (#636)

- Named high-entropy integration credentials.
- Digest-only storage.
- Explicit read/write scopes.
- Revocation and optional expiry.
- One-time secret display when the management UI is completed.
- Bounded `last_used_at` writes.
- Legacy token compatibility.
- Endpoint scope enforcement and cross-user tests.

### PR2 — Client identity and delivery receipts

- Stable authenticated client identity.
- Optional idempotency key on compatible writes.
- Receipt reservation before mutation.
- Payload digest and conflict response.
- Crash/timeout safety.
- Client namespace and receipt-retention policy before Release 1.0.

### PR3 — Ordered playback-progress changes

- Additive change-feed endpoint.
- Monotonic sequence.
- Upsert and explicit delete events.
- Opaque, user-bound cursor.
- Cursor expiry and tombstone retention.
- Snapshot fallback.
- Existing progress endpoint remains compatible.

### PR4 — Saved-library/watchlist incremental state

- Snapshot and changes.
- Explicit add/remove.
- Profile/client binding.
- Partial-page and empty-response safety.
- Offline/reconnect recovery.
- No Nuvio Collection semantics in this package.

### PR5 — Exact watched state and Nuvio conformance

- Exact movie and episode completion contract.
- Existing Stremio watched-state behavior remains protected by regression tests.
- Capability discovery.
- Stable errors and examples.
- Nuvio-facing conformance fixtures for progress, saved library, watched state, identity, retry, reconnect, and cursor expiry.

Batch episode actions must reuse the existing episode state transition path rather than adding a second completion implementation.

### PR6 — Reconciliation, diagnostics, and recovery UI

- Dry-run difference report.
- Safe apply.
- Unresolved items.
- Created/updated/skipped/unresolved/failed counts.
- Last success and failure.
- Retry and resume.
- Feature kill switch.
- Keyboard, focus, reduced-motion, screen-reader, and narrow-layout verification.

### PR7 — Offline, packaged-runtime, performance, and Release 1.0 stabilization

- Network partition/reconnect tests.
- Redis unavailable tests.
- Worker/process restart tests.
- SQLite and PostgreSQL parity.
- Upgrade replay.
- Large-library measurements.
- Queue and memory measurements.
- Rollback drill.
- Release security review.

### PR8 — Read-only Floppy catalog surface (#635)

- Revocable read-only share grant.
- Stremio-compatible manifest/catalog.
- Deterministic pagination.
- Verified IDs.
- Private-field exclusion.
- No mutation, streams, or debrid behavior.

### PR9 — Safe fetch, cache transparency, and read-only metadata projection

Proceed only when a real caller requires the shared boundary.

- SSRF-safe destination validation.
- Redirect revalidation.
- Response, time, depth, and concurrency limits.
- Last-known-good cache.
- Source/config/language/region cache isolation.
- Provenance and freshness.
- No provider metadata writeback.

### PR10 — Local Shared Media Workspace

Run a fresh architecture, developer-experience, simplification, and security review before implementation.

Only add primitives proven necessary by PR0–PR9, such as:

- portable Collection descriptors;
- Floppy list to Nuvio Collection source references;
- declarative add-on installation identity;
- per-application enabled/order state;
- encrypted configured add-on references;
- explicit user metadata preferences/overrides;
- cache invalidation/freshness events;
- capability negotiation.

Do not add direct shared database tables or executable plugin synchronization.

---

## 7. Security gates

Release evidence must cover, where relevant:

- object-, property-, and function-level authorization;
- token lifecycle and secret redaction;
- replay and concurrent replay;
- uncertain timeout/crash outcomes;
- cursor tampering and expiry;
- explicit delete retention;
- cross-profile isolation;
- resource and payload limits;
- unsafe upstream payloads;
- injection and metadata rendering;
- SSRF, DNS rebinding, redirect bypass, and cloud metadata targets;
- cache poisoning and cross-user cache isolation;
- migration and partial-transaction failures;
- dependency and action supply-chain review;
- exceptional-condition data loss.

Do not describe the programme as fully hardened until the implementation and release gates have actually passed.

---

## 8. Offline and packaged-runtime behavior

Correctness must not depend on Docker.

- Existing local media state remains usable during external network outages.
- User-requested state is durable before asynchronous execution when loss would otherwise be possible.
- Offline changes resume safely after reconnect.
- Redis/cache failure does not silently drop user-requested work.
- Core transition logic is callable independently from the Celery scheduler.
- SQLite and PostgreSQL keep equivalent user-visible semantics.
- Do not hard-code container-only paths, service names, or reverse-proxy assumptions into the integration model.

Some operations still need remote providers for previously unknown metadata or identity resolution. Offline support must report that limitation instead of guessing.

---

## 9. Performance gates

Performance claims require measurements.

Required properties:

- incremental sync is the normal path;
- full reconciliation is exceptional;
- bounded page and batch sizes;
- no unbounded retries;
- no O(N²) library scans;
- indexes support ordered change queries;
- network work stays outside database write transactions;
- long sync work does not block the interactive queue;
- receipt/change/tombstone storage growth is measured and bounded by a documented retention policy.

Measure small, medium, and large reference libraries for:

- snapshot and incremental sync duration;
- query and write counts;
- remote call count;
- task and queue duration;
- peak memory;
- p95 page latency;
- cache hit/miss/stale rate;
- unresolved and retry rates;
- receipt/change/tombstone storage growth.

Do not promise microsecond transaction times or claim WAL eliminates SQLite contention without measured evidence.

---

## 10. Documentation and review gates

When the contract changes, update the applicable surfaces:

- verified OpenAPI artifact;
- dynamic API schema annotations;
- AsyncAPI only for real asynchronous channels;
- API/MCP wiki;
- durable agent/domain guidance;
- README/config and upgrade guidance;
- `UPSTREAM_PORTS.md` when an upstream Yamtrack outcome is classified;
- JSON-LD only where a public semantic web surface needs it.

For visible UI changes, provide screenshots and verify:

- keyboard operation;
- visible and stable focus;
- no color-only status;
- reduced motion;
- persistent error/recovery state;
- narrow viewport;
- clear destructive preview and affected counts;
- masked secret display.

Use short, direct sentences in public documentation and PR comments.
