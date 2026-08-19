# Floppy × Nuvio Programme — Current Handoff

**Captured:** 2026-08-19 05:20 Europe/Dublin  
**Repository:** `dannyvfilms/Floppy`  
**PR:** #791  
**Branch:** `plan/nuvio-integration-reviewed-2026-08-15`

This checkpoint supersedes the earlier 2026-08-19 handoff where current source disagrees with that document. Current repository source and current CI are authoritative.

## 1. Verified state before this handoff commit

- PR #791: open, draft, unmerged, mergeable.
- Base branch: `latest`.
- Base SHA: `c7bb308806b4a553dbae241acb241df0589d472d`.
- Head SHA before this handoff commit: `3a1e7e420c871153942791fe266f7750982c39d1`.
- PR commits before this handoff commit: 196.
- Changed files before this handoff commit: 71.
- Original PR body remains unchanged by design.

Current CI on `3a1e7e420c871153942791fe266f7750982c39d1`:

- Lint: PASS.
- CodeQL: PASS.
- Docker Image: PASS.
- App Tests: FAIL.

Do not call either release production-ready until the failing App Test job is fixed and the full final gate matrix is current on the final head.

## 2. Important correction to the previous handoff

The previous 2026-08-19 handoff said that the branch already contained an explicit client-namespace migration for `IntegrationEventReceipt`. Current source does not support that claim.

Current `IntegrationEventReceipt` persistence still has a uniqueness constraint on:

`(user, client_event_id)`

`src/integrations/delivery.py` currently avoids cross-client collision by deriving an internal storage ID such as `token-<digest>:<public-key>` when another client already owns the public key.

That compatibility mechanism is useful, but it is not the desired final persistence contract.

Required final hardening:

1. add an explicit stable authenticated-client namespace to the receipt model;
2. migrate old receipts without breaking replay behavior;
3. make uniqueness client-scoped at the database layer;
4. keep the public `Idempotency-Key` unchanged;
5. remove the synthetic storage-key workaround only after migration compatibility is proven;
6. add concurrent same-user/two-client collision coverage;
7. preserve replay evidence if a credential is revoked or later physically deleted.

## 3. Core invariants

Keep these concepts separate:

- playback progress;
- saved-library/watchlist membership;
- current watched state;
- viewing-history occurrence;
- legitimate rewatch;
- Floppy list;
- Nuvio Collection;
- declarative add-on;
- executable plugin;
- provider metadata;
- user metadata override;
- explicit delete/tombstone;
- authenticated client origin.

Rules:

- Progress is not history.
- Watchlist is not watched state.
- Watchlist is not a Nuvio Collection.
- A duplicate delivery is not a rewatch.
- Absence, timeout, provider error, partial page, empty response, or cache miss is not deletion.
- Caller-supplied origin is not authoritative.
- Authoritative writes require exact identity or a verified resolver translation.
- Unknown content types stay unknown; do not guess.
- Add-ons are declarative resources. Plugins are executable code and remain separate.

## 4. Actual PR0–PR10 progress

PR0–PR10 are work-package labels inside PR #791. They are not separate GitHub pull requests.

### PR0 — Evidence, baseline, and conformance protection

**Status: DONE / baseline established.**

Present:

- reviewed programme/PRD documents;
- regression coverage around existing API behavior;
- current Nuvio conformance fixtures;
- explicit compatibility baselines for #417, #429, #619, and #723.

Still improve:

- pin final NuvioTV and Nuvio self-host revisions in release evidence;
- keep the implementation ledger synchronized with source rather than old stage numbering;
- run one final independent diff review after all later work is complete.

### PR1 — Scoped third-party credentials

**Status: IMPLEMENTED; release hardening remains.**

Present:

- named scoped integration credentials;
- digest-only secret persistence;
- expiry/revocation;
- bounded `last_used_at` behavior;
- fail-closed route-to-scope policy;
- canonical resource-oriented scope vocabulary;
- legacy compatibility path;
- recent allauth reauthentication before persistent credential create/revoke.

Still improve:

- finish final OpenAPI security-scheme/scope review;
- verify one-time secret reveal, clipboard fallback, keyboard/focus, and no-store behavior in browser QA;
- ensure physical credential deletion, if added, does not erase receipt/audit evidence;
- document lifecycle/rotation/recovery clearly for third-party developers.

### PR2 — Authenticated client identity and durable idempotency

**Status: IMPLEMENTED FOUNDATION; NOT RELEASE-COMPLETE.**

Present:

- server-derived client origin;
- bounded validated idempotency keys;
- canonical payload digest;
- receipt reservation before protected mutation;
- same-key/same-payload replay;
- same-key/different-payload conflict;
- fail-closed incomplete reservations;
- cross-client fallback namespace behavior.

Release blockers:

- explicit client namespace in persistence is still missing;
- state mutation and final receipt persistence are not one atomic database boundary;
- a crash after mutation commit but before final receipt write remains ambiguous and fails closed;
- replay evidence retention across credential lifecycle needs final proof.

Preferred improvement:

Prepare any network/identity work first, then execute the eligible database mutation and receipt finalization in one transaction so timeout-after-commit replay is deterministic.

### PR3 — Ordered playback-progress changes

**Status: IMPLEMENTED; final validation remains.**

Present:

- current progress state preserved;
- append-only ordered changes;
- explicit upsert/delete;
- signed client-bound cursors;
- cursor bounds/expiry;
- stable snapshot + delta catch-up;
- per-user snapshot indexes;
- pruning/retention machinery.

Still improve:

- prove cursor-expiry/snapshot recovery under process restart;
- verify retention window and tombstone retention are longer than supported offline windows;
- run large-library query-count, memory, and latency measurements on final head;
- verify PostgreSQL and SQLite produce equivalent semantics.

### PR4 — Saved-library/watchlist incremental state

**Status: IMPLEMENTED; final compatibility validation remains.**

Present:

- separate saved-media state/change model;
- snapshot + ordered delta;
- explicit add/remove;
- exact identity serialization;
- no delete-from-absence behavior;
- performance-focused tests.

Still improve:

- verify local unacknowledged writes survive reconnect/reconciliation;
- exercise two-client and profile-switch races;
- prove removing saved membership cannot erase progress, watched state, or history;
- pin Nuvio library semantics used by the conformance fixtures.

### PR5 — Exact watched state and Nuvio conformance

**Status: IMPLEMENTED; native projection edge cases remain.**

Present:

- `WatchedState` and `WatchedStateChange` separate from history;
- exact movie/episode state;
- ordered watched-state changes;
- Nuvio conformance coverage;
- #723 behavior preserved as the regression baseline.

Still improve:

- audit native TV/Season bulk-completion paths;
- mirror only exact native occurrences into current watched projection;
- never infer sibling watched state from one external observation;
- run complete watch/unwatch/rewatch history-isolation tests on final head.

### PR6 — Reconciliation, diagnostics, and recovery UI

**Status: PARTIAL. Backend diagnostics exist; the complete recovery UX is not proven.**

Present:

- integration diagnostics API surface;
- capabilities/snapshot state useful for recovery;
- classified integration state is available to build UI on.

Still required:

- complete dry-run reconciliation and safe apply UX where not already wired;
- unresolved-item reporting with actionable recovery;
- last success/failure/checkpoint presentation;
- feature kill switch/retry/resume presentation;
- destructive previews with exact affected counts;
- browser QA for keyboard, focus restoration, screen reader output, reduced motion, narrow layout, and persistent errors;
- screenshots for normal, loading, offline, error, recovery, and destructive confirmation states.

### PR7 — Offline, packaged-runtime, performance, and Release 1 stabilization

**Status: IN PROGRESS / RELEASE GATE RED.**

Present:

- core sync state is database-backed rather than cache-authoritative;
- pruning path is database-only;
- snapshot/performance regression tests exist;
- merge migration `0166_merge_nuvio_latest_20260819.py` reconciles current application migration leaves;
- current Lint, CodeQL, and Docker checks pass.

Still required:

- App Tests must pass on the current final head;
- strict migration hygiene;
- SQLite upgrade replay;
- PostgreSQL upgrade replay;
- offline write/sync/reconnect matrix;
- Redis unavailable behavior;
- worker/process restart behavior;
- timeout-after-commit proof;
- token revoke mid-run;
- cursor expiry/recovery;
- small/medium/large library performance evidence;
- peak memory/query/write/remote-call/task-duration/storage-growth measurements;
- rollback/kill-switch drill;
- final post-mortem.

The test logs also expose environmental/quality signals worth tracking separately from the Nuvio work: SQLite WAL safety fallback on the runner, a pydantic unresolved-forward-reference warning, database-lock warnings under parallel SQLite tests, and network-tagged provider noise. Do not hide these with blanket suppression. Classify which are test-environment noise and which reveal production risk.

### PR8 — Read-only Floppy Stremio/Nuvio catalog surface

**Status: IMPLEMENTED FOUNDATION; NOT FINAL-PRODUCT COMPLETE.**

Present in `src/lists/stremio.py`:

- public-list-only manifest;
- read-only `catalog` and `meta` resources;
- movie/series publication;
- exact IMDb/TMDB/TVDB-compatible IDs;
- deterministic pagination;
- unresolved-item counts;
- public CORS/cache headers;
- no stream/debrid resource;
- no title-only identity guess.

Still improve:

- confirm public-list share/revocation UX is intentional and discoverable;
- consider a dedicated revocable share grant if list-publicity alone is too broad a security boundary;
- verify leakage review for list title/description/artwork and private fields;
- add cache validators (`ETag`/`Last-Modified`) if measurements show value;
- pin Stremio/Nuvio protocol conformance and malformed-request behavior;
- verify large-list count/pagination query cost.

### PR9 — Safe remote fetch, cache transparency, and metadata projection

**Status: PARTIAL. Security boundary exists; the complete product slice does not.**

Present:

- one HTTPS-only SafeFetch boundary;
- DNS resolution with non-public-address rejection;
- direct connection to validated IP while preserving TLS hostname verification/SNI;
- redirect revalidation;
- bounded URL/body/JSON/timeout/redirect behavior;
- compressed-response rejection;
- stable redacted errors;
- no Floppy credential forwarding;
- Nuvio-compatible content classifier;
- remote Stremio manifest parser.

Still required/enhance:

- persistent last-known-good cache with explicit provenance/freshness where remote metadata/catalog consumption needs it;
- cache key isolation by source/config digest/type/ID/language/region/schema;
- visible fresh/stale/error state in UI;
- normalized read-only metadata projection if still justified by a real caller;
- circuit breaking/single-flight only if measurements show need;
- LAN/self-host connector policy must remain separate from the generic public-network fetcher;
- fuzz/property tests for URLs, redirects, IDNA, response bounds, and parser complexity;
- security review for DNS rebinding and redirect-to-private-address behavior.

### PR10 — Local Shared Media Workspace boundary

**Status: FOUNDATION ONLY; NOT A COMPLETE LOCAL WORKSPACE.**

Present in `src/integrations/workspace_contract.py`:

- versioned reference-only Collection descriptor;
- Nuvio-compatible add-on catalog source shape;
- bounded folders/sources/strings;
- shared content classifier use;
- rejection of URLs, configured URLs, tokens, passwords, plugin/code blobs, and flattened media arrays.

Not yet a full workspace:

- no persisted shared add-on installation registry;
- no encrypted configured-add-on secret vault;
- no per-application enable/order state service;
- no pairing endpoint/flow for workspace resources;
- no Collection binding lifecycle API;
- no cache invalidation/freshness event service;
- no user metadata preference/override synchronization;
- no supported Nuvio self-host server-to-server write adapter;
- no UI for source ownership, permissions, unlink, or conflict handling.

Keep these exclusions:

- no direct Nuvio PostgreSQL access;
- no service-role keys;
- no raw Nuvio password;
- no shared application database tables;
- no executable plugin synchronization;
- no raw whole-record metadata replication.

## 5. Authentication / IDP continuation

Current direction remains:

- django-allauth owns account/session/social identity;
- `IntegrationToken` remains the PAT/offline/automation compatibility path;
- all credential transports consume the same Floppy resource-scope vocabulary;
- allauth Headless is for first-party packaged/mobile Floppy clients;
- allauth OIDC is the preferred delegated pairing layer for Nuvio and similar clients;
- Device Authorization is preferred for TV clients;
- Authorization Code + PKCE is preferred for browser/desktop/mobile clients.

Still required:

- atomic allauth dependency + `uv.lock` upgrade; never hand-edit the generated lock;
- OIDC access-token authorization through the canonical scope policy;
- client provisioning and revocation;
- device flow/PKCE conformance;
- disabled-user behavior;
- refresh rotation/revocation;
- OpenAPI/AsyncAPI discovery/security documentation;
- pairing/re-auth UX.

Later Settings-page enrichment should evaluate and expose only safe, relevant administrator/user controls, including:

- WebAuthn/passkey configuration and status;
- OIDC issuer/client/discovery information and allowed scopes where appropriate;
- `MFA_TRUST_ENABLED` and trusted-browser lifecycle;
- session lifetime/logout timeout with an explicit non-expiring-session option only if security review accepts the risk and the UI explains it;
- QR-code login for normal supported login/pairing flows rather than limiting QR use to recovery.

Do not expose secrets or implementation-only values merely because allauth has a setting for them.

## 6. Outbound User-Agent identity

The programme should standardize outbound HTTP identity rather than letting each provider/integration invent one.

Current SafeFetch sends `User-Agent: Floppy-SafeFetch/1`.

Later hardening should define one central Floppy outbound User-Agent policy for provider POST/GET traffic, similar to the explicit identity used for services such as Open Library:

- identify the application as Floppy;
- include a stable application/version token where available;
- include a project/contact URL only where provider policy recommends it;
- never include user PII, tokens, instance secrets, or private hostnames;
- keep provider-specific additions in adapters rather than duplicating the base identity.

Audit existing POST clients before changing behavior so provider compatibility and rate-limit policies are not broken.

## 7. Final definition of done

Do not call Release 1 or Release 2 production-ready until all applicable gates pass on the final reconciled head:

1. current `latest` reconciliation;
2. full App Tests green;
3. Ruff/lint green;
4. CodeQL green;
5. Docker/package smoke green;
6. strict migration hygiene;
7. SQLite and PostgreSQL migration replay;
8. OpenAPI generation/check;
9. AsyncAPI validation where affected;
10. Stremio/Kodi/CrossWatch regressions;
11. Nuvio progress/saved/watched conformance;
12. offline/reconnect and queue/cache failure matrix;
13. timeout-after-commit and concurrent replay;
14. two-client/user/profile isolation;
15. cursor expiry/snapshot recovery;
16. token revoke/disabled-user behavior;
17. measured performance and memory/storage growth;
18. final security diff scan;
19. final deep security review of auth/API/integrations/SafeFetch/migrations;
20. browser QA and accessibility checks;
21. screenshots for visible changes;
22. developer onboarding/DX review where runnable;
23. canonical issue/upstream relationships current;
24. rollback/kill-switch proof;
25. stage post-mortems current.

## 8. Next-agent order

1. Read this file, then current `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `HANDOFF-QA.md`, and `UPSTREAM_PORTS.md`.
2. Re-read current PR #791 head and base; do not trust stale SHAs in older handoffs.
3. Inspect the current App Test failure and fix the exact failing tests without weakening them.
4. Normalize `IntegrationEventReceipt` to an explicit client namespace and close timeout-after-commit ambiguity.
5. Re-run targeted delivery/concurrency tests and full CI.
6. Audit native TV/Season watched-state projection.
7. Finish Release 1 migration/offline/performance/security/QA gates.
8. Complete the atomic allauth upgrade and OIDC/Headless conformance.
9. Finish PR8–PR10 only to the degree real callers justify: catalog hardening, cache/provenance, workspace persistence/pairing.
10. Keep the PR draft until release evidence is complete.

## 9. Operating rule

Standardize semantics, not vendors. Keep each state concept exact. Keep retries, deletes, authorization, provenance, cache state, and offline recovery explicit. Prefer one shared rule over provider-specific copies. Do not claim a release is ready until current evidence proves it.
