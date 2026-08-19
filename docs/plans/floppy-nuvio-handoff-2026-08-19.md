# Floppy × Nuvio Programme — 2026-08-19 Handoff

**Primary repository:** `dannyvfilms/Floppy`  
**Primary PR:** #791  
**Branch:** `plan/nuvio-integration-reviewed-2026-08-15`  
**Purpose:** Durable implementation handoff for Release 1 and Release 2 production work.

> The original PR #791 body is intentionally unchanged. Use branch source, commits, the living PR ledger, and this file as the current implementation record.

## 1. Verified branch state at handoff creation

Immediately before this file was created:

- `latest`: `c7bb308806b4a553dbae241acb241df0589d472d`
- PR head: `29b85b2e8c215c0a4b1d8c94db860347bb4e65ec`
- merge base: `c7bb308806b4a553dbae241acb241df0589d472d`
- branch behind `latest`: `0`
- branch ahead of `latest`: `193`
- PR state: open, draft, unmerged, mergeable
- changed files: `69`

The branch is therefore based on the current `latest` commit at this checkpoint. Recheck this before every release claim because `latest` can advance.

## 2. One-PR rule

PR #791 is the only implementation vehicle for this programme.

PR0 through PR10 are work-package labels inside PR #791. They are not separate GitHub pull requests.

Do not:

- create a replacement PR;
- rewrite the original PR body;
- force-push without explicit approval;
- merge automatically;
- create a new issue for an ordinary review finding when an existing canonical issue owns the behavior.

Use the living implementation-ledger comment for current status and stage-completion notes.

## 3. Core state invariants

Keep these concepts separate:

- playback progress;
- saved-library/watchlist membership;
- current watched state;
- viewing-history occurrence;
- legitimate rewatch;
- Floppy list;
- Nuvio Collection;
- add-on;
- executable plugin;
- provider metadata;
- user metadata override;
- deletion/tombstone;
- authenticated client origin.

Rules:

- Progress is not history.
- Watchlist is not watched state.
- Watchlist is not a Nuvio Collection.
- A duplicate delivery is not a rewatch.
- Provider metadata is not user-authored truth.
- Absence is not deletion.
- A partial page is not deletion.
- A cache miss is not deletion.
- A timeout is not deletion.
- Ambiguous identity is not a match.
- Caller-provided origin is not authoritative.
- An add-on is not an executable plugin.

## 4. Matching contract

Authoritative writes require an exact supported external identifier or a verified translation through the shared resolver.

Do not mutate user state from title-only matching.

Episode synchronization uses:

- authoritative parent-show identity;
- season number;
- episode number;
- provider namespace and provenance where available.

The source-specific adapter can translate representations, but the underlying identity rule must stay source-neutral.

## 5. Classifier contract

Nuvio-compatible screen-media classification uses the current semantic boundary:

- `movie`
- `series`
- `channel`
- `tv`

Unknown raw types must remain explicit. Do not guess a supported type from a title or arbitrary metadata.

The canonical classifier belongs in one shared integration module. Catalog publication, remote manifest parsing, and later workspace code should consume that same rule.

## 6. Release 1 — implemented foundation

Release 1 is the tracking and synchronization release.

The branch contains implementation for:

- named scoped integration credentials;
- digest-only long-lived secret storage;
- stable client identity;
- server-derived origin;
- exact endpoint scopes;
- durable idempotency receipts;
- explicit client namespace in the receipt persistence model;
- progress current state;
- progress snapshot and ordered delta;
- progress explicit delete;
- saved-library/watchlist snapshot and ordered delta;
- explicit saved-item add/remove;
- separate current watched-state projection;
- watched-state ordered delta;
- stable keyset snapshots;
- signed client-bound cursors;
- cursor input bounds and expiry;
- explicit retention policy and pruning command;
- diagnostics and capability discovery;
- Nuvio conformance fixtures;
- bounded pagination and per-user snapshot indexes.

Do not mark Release 1 production-ready until the final gates in section 15 pass on the final reconciled head.

## 7. Watched state must remain separate from history

`WatchedState` and `WatchedStateChange` represent current state.

Historical Movie/Episode rows represent viewing occurrences.

An external `watched=false` must not delete historical plays.

An external `watched=true` must not manufacture a viewing occurrence unless the operation is explicitly a playback/scrobble event.

This protects the completed #723 correctness baseline.

Audit native TV/Season bulk completion paths before the final release. If native Floppy actions create exact Episode history rows in bulk, mirror only those exact native occurrences into the current-state projection. Do not infer sibling state from an external observation.

## 8. Idempotency contract

Delivery identity is separate from media-occurrence identity.

Required behavior:

- same authenticated client + same key + same payload -> return the prior result;
- same authenticated client + same key + changed payload -> conflict;
- another authenticated client can use the same public key without colliding;
- a distinct occurrence identifier/time can represent a legitimate rewatch;
- origin comes from authenticated client identity;
- public idempotency keys are bounded;
- payload digests are canonical;
- uncertain outcomes fail safely.

The branch contains migration `src/integrations/migrations/0025_integrationeventreceipt_client_namespace.py`. Verify migration replay and concurrency tests before declaring the older per-user uniqueness workaround fully retired.

A process crash after state mutation but before final receipt persistence remains a critical scenario to test. Do not solve it by expiring an ambiguous receipt and blindly replaying a potentially completed mutation.

## 9. Offline and packaged-runtime contract

Local correctness must not depend on:

- Docker service names;
- `/app` paths;
- one reverse proxy;
- Redis availability;
- Celery availability;
- an external Nuvio service;
- a metadata provider being reachable.

Core state transitions must remain callable in-process.

When the network is unavailable:

- local state stays usable;
- user-requested durable writes are not silently lost;
- last-known-good projections remain available where safe;
- empty/error remote responses do not cause deletion;
- synchronization resumes from a durable cursor/checkpoint.

Keep SQLite and PostgreSQL equivalent for correctness-critical behavior.

## 10. Performance contract

Incremental synchronization is the normal path. Full reconciliation is exceptional.

Required properties:

- bounded pages and parser inputs;
- indexed per-user cursor and snapshot queries;
- no O(N^2) library behavior;
- no full-library serialization to return one page;
- bounded retries and concurrency;
- no one-task-per-item explosion without measured evidence;
- bounded receipt/change-log/tombstone storage;
- cache is an optimization, never authoritative user truth.

Before release, measure small, medium, and large libraries for:

- snapshot time;
- incremental sync time;
- p95 page latency;
- query count;
- DB writes;
- remote calls;
- task duration;
- queue wait;
- peak memory;
- retry rate;
- unresolved rate;
- receipt/change-log growth;
- cache hit/miss/stale behavior.

Do not publish invented performance claims.

## 11. Authentication platform

Django-allauth already owns Floppy account/session/social identity.

`IntegrationToken` remains the scoped compatibility credential for:

- CLI;
- automation;
- local/offline clients;
- integrations that do not implement OAuth/OIDC.

Do not replace it wholesale with a first-party session token.

One canonical scope vocabulary must drive:

- PAT authorization;
- OIDC consent/authorization;
- capability discovery;
- OpenAPI;
- AsyncAPI;
- user-facing permission text;
- Nuvio conformance.

Current resource scopes include:

- `scrobble:write`
- `progress:read`
- `progress:write`
- `watchlist:read`
- `watchlist:write`
- `watched:read`
- `watched:write`
- `catalog:read`

Do not add vendor permissions such as `nuvio:write` when the resource capability already exists.

Sensitive PAT create/revoke actions must reuse allauth recent-authentication behavior.

## 12. Release 2 — implemented foundation

Release 2 is the local sharing and standards-based pairing release.

The branch contains foundations for:

- centralized scope policy;
- allauth OIDC scope adapter;
- feature-gated account/OIDC groundwork;
- read-only Floppy Stremio/Nuvio catalog publication;
- safe remote fetch boundary;
- Nuvio-compatible content classification;
- remote Stremio/add-on manifest parsing;
- portable Collection/workspace descriptors and source references.

The current Release 2 direction is:

- allauth Headless for first-party packaged Floppy session flows;
- allauth OIDC provider for delegated third-party pairing;
- Device Authorization for TV-style pairing;
- Authorization Code + PKCE for browser/desktop/mobile clients;
- opaque access tokens by default;
- PAT remains the advanced/manual compatibility path.

Verify the current dependency declaration and `uv.lock` before changing allauth. Do not hand-edit the generated lock file.

## 13. SafeFetch contract

All remote add-on/metadata access must use one outbound boundary.

Generic remote policy must:

- use HTTPS by default;
- bound URL length;
- reject unsafe schemes and credentials in URLs;
- normalize and validate hostnames;
- resolve DNS;
- reject loopback/private/link-local/CGNAT/multicast/cloud-metadata targets;
- connect only to the validated public destination;
- preserve TLS hostname verification/SNI;
- revalidate redirects;
- cap redirects;
- cap connect/read/total time;
- cap compressed/decompressed bytes;
- cap JSON depth/item count;
- strip Floppy credentials;
- redact secrets/query/fragment from errors/logs;
- return stable public errors;
- never execute downloaded plugin code.

A user-approved LAN/self-host connector must use a separate, narrow allowlist policy. Do not weaken the generic fetcher for LAN access.

## 14. Local Shared Media Workspace contract

Share references and normalized projections, not application databases.

Clean ownership:

- Floppy owns Floppy tracking state, lists, and normalized projections.
- Nuvio owns playback-client state, Collection layout, and plugin runtime.

Potential shared primitives:

- catalog references;
- declarative add-on registrations;
- selected Collection descriptors;
- normalized metadata projections;
- user-owned metadata preferences/overrides;
- cache freshness and invalidation.

Reject:

- direct Nuvio PostgreSQL access;
- Nuvio service-role credentials;
- raw Nuvio account passwords;
- direct shared application tables;
- executable plugin synchronization;
- raw whole-record metadata replication;
- hidden configured add-on secrets in URLs/logs/screenshots.

## 15. Production definition of done

Do not call Release 1 or Release 2 production-ready until all applicable evidence is current on the final head.

Required final evidence:

1. Current `latest` comparison shows no unresolved base drift.
2. Full App Tests pass.
3. Ruff/lint passes.
4. CodeQL passes.
5. Docker/package smoke passes.
6. Strict migration hygiene passes.
7. SQLite migration replay passes.
8. PostgreSQL migration replay passes.
9. OpenAPI regeneration/check passes.
10. AsyncAPI validation passes where affected.
11. Current Stremio/Kodi/CrossWatch compatibility regressions pass.
12. Nuvio progress/watchlist/watched conformance passes.
13. Offline/reconnect matrix passes.
14. Timeout-after-commit and concurrent replay tests pass.
15. Two-client/user/profile isolation tests pass.
16. Cursor expiry/snapshot recovery passes.
17. Token revocation behavior passes.
18. Small/medium/large performance evidence is measured.
19. Security diff scan is clean of validated critical/high findings.
20. Deep security review of auth/API/integrations/SafeFetch/migrations is complete.
21. Browser QA is complete for visible token/integration/recovery flows.
22. Keyboard/focus/reduced-motion/narrow-view/accessibility checks pass.
23. Visible changes have screenshots.
24. Developer onboarding/conformance flow has a real DX review where runnable.
25. Canonical issues and upstream counterparts are linked.
26. Rollback/kill-switch path is tested.
27. Stage post-mortems are current.

A warning-heavy test log is not proof of failure. Use the final test summary and failing test names. Likewise, a prior green run on an older head is not proof for the release candidate.

## 16. Current CI note

At the pre-handoff head `29b85b2e8c215c0a4b1d8c94db860347bb4e65ec`:

- Docker Image: PASS
- CodeQL: PASS
- Lint: FAIL
- App Tests: FAIL

The earlier Lint log referenced file names that are no longer present in the current branch tree. Treat that run as evidence to investigate, not as a patch target. This handoff commit creates a fresh head and therefore fresh CI evidence against the current branch state.

Do not weaken tests or suppress rules merely to turn the checks green.

## 17. Canonical relationships

Floppy:

- #532 — Nuvio programme epic
- #636 — scoped third-party credentials
- #635 — read-only Stremio/Nuvio catalogs
- #599 — malformed Stremio/Cinemeta payload behavior
- #598 — recurring Stremio status visibility
- #619 — provider-prefixed identity compatibility baseline
- #723 — watched-state/history correctness baseline
- #429 — durable progress baseline
- #417 — third-party/Kodi API baseline
- #652 — historical architecture context only

Nuvio:

- NuvioMedia/NuvioTV#2935 — Floppy integration
- NuvioMedia/NuvioTV#2484 — external API/device authorization
- NuvioMedia/NuvioTV#2967 — multi-provider scrobbling

Also recheck current CrossWatch, Scrob, Yamtrack, NuvioTV, and Nuvio self-host state before final compatibility claims.

## 18. Next-agent execution order

1. Re-read current `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `HANDOFF-QA.md`, and `UPSTREAM_PORTS.md`.
2. Verify current PR head and `latest` SHA.
3. Inspect fresh CI created by this handoff commit.
4. Fix only current, attributable App Test and lint failures.
5. Verify receipt client-namespace migration and crash/replay semantics.
6. Audit native TV/Season watched-state projection paths.
7. Verify current allauth dependency/lock and Release 2 feature gates.
8. Finish OIDC Device Authorization/PKCE/resource-scope authorization where still incomplete.
9. Verify SafeFetch and remote manifest security against the current complete diff.
10. Verify catalog/workspace protocol compatibility against pinned NuvioTV/Nuvio self-host revisions.
11. Regenerate/check OpenAPI and AsyncAPI.
12. Run migration replay, offline/reconnect, performance, security, QA, and DX gates.
13. Update the living PR ledger with exact commands/results/SHAs.
14. Leave PR #791 draft until all production gates are evidenced.

## 19. Operating principle

Standardize semantics, not vendors.

Keep state concepts exact. Keep retries, deletion, authorization, provenance, and offline recovery explicit. Reuse one rule in every integration surface. Make independent adoption cheap without coupling Floppy to Nuvio internals.
