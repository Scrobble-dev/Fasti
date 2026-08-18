# Floppy–Nuvio programme handoff — 2026-08-18

This file is a durable checkpoint for PR #791. The original PR body is intentionally unchanged and now describes only the initial planning state. Current branch source and this checkpoint are the operational record.

## Repository checkpoint

Before this handoff commit the branch head was `d4e4addabf8e83bac7051e6e3e4a2d713a3736b1` on `plan/nuvio-integration-reviewed-2026-08-15`.

PR #791 remains the only implementation vehicle. PR0–PR10 are work packages inside it, not separate pull requests.

The programme is not yet release-ready. Reconcile the branch with current `latest`, rerun the full validation matrix, and keep the PR draft until the release gates are satisfied.

## Durable architecture decisions

- Keep one source-neutral integration contract inside Floppy.
- Authoritative writes require an exact supported external identifier or a verified translation. Do not mutate user state from title-only matching.
- Keep playback progress, saved-library membership, current watched state, viewing-history occurrences, and legitimate rewatches separate.
- Nuvio saved-library membership is not a Nuvio Collection.
- Current watched state must not delete or manufacture viewing history.
- Missing data, a partial page, a timeout, a cache miss, or an upstream outage is not deletion.
- Delete only from an explicit remove/tombstone or an explicitly approved authoritative reconciliation.
- Derive origin from the authenticated credential. Do not trust caller-provided origin for loop prevention.
- Use server sequence for transport ordering. Client timestamps are observation metadata.
- Keep existing API routes compatible and add synchronization surfaces additively.
- Do not couple Floppy to Nuvio PostgreSQL tables, service-role credentials, internal SQL/RPC names, or raw account passwords.
- Add-ons are declarative remote resources. Plugins are executable code and are not automatically synchronized or executed by Floppy.

## Release 1 state

Substantial implementation exists for:

- scoped integration credentials;
- client identity and origin;
- durable idempotency receipts;
- progress snapshots and ordered changes;
- saved-library/watchlist snapshots and ordered changes;
- exact watched-state projection and ordered changes;
- stable keyset reconciliation snapshots;
- client-bound signed cursors;
- cursor input bounds;
- retention/compaction policy and a database-only maintenance command;
- diagnostics;
- accessible integration-token management UI;
- large-library query/index improvements;
- Nuvio conformance fixtures.

### Important remaining Release 1 work

1. Reconcile with current `latest` and resolve conflicts without dropping either side.
2. Normalize idempotency receipt persistence. The current DB constraint is still per `(user, client_event_id)` and the delivery layer uses a server-derived storage namespace when two scoped clients reuse the same public key. The final schema should express client namespace directly while preserving old receipts.
3. Audit native TV/Season bulk Episode-history creation so exact native completions mirror into current watched-state projection without recreating the completed #723 importer bug.
4. Run the final SQLite/PostgreSQL migration replay and strict migration hygiene.
5. Run full App Tests, lint, CodeQL, Docker/package smoke, OpenAPI/AsyncAPI checks, offline/reconnect cases, timeout-after-commit, duplicate/concurrent delivery, cursor expiry, revoke, two-client isolation, and large-library performance measurements.
6. Run browser `/qa`, accessibility review, developer-experience review, final security diff/deep scan, rollback drill, and post-mortem.

## Synchronization contract

For each user, client/profile binding, and resource:

1. capture server cursor/checkpoint `C0`;
2. fetch a bounded stable snapshot;
3. preserve local unacknowledged writes;
4. pull ordered changes after `C0`;
5. apply one page transactionally;
6. persist the next cursor only after commit;
7. continue until no changes remain;
8. if the cursor expired, take a fresh snapshot;
9. if the remote source fails, preserve last valid local state.

Idempotency rules:

- same key + same payload → replay prior result;
- same key + changed payload → conflict;
- a new viewing occurrence is not a duplicate delivery;
- interrupted reservations fail closed until state is reconciled safely.

## Matching and classifier rules

- Keep one shared external-media identity boundary.
- Adapters translate Nuvio, Stremio, and provider representations into that boundary.
- Preserve provider namespaces and provenance.
- Episode synchronization uses exact parent-show identity plus episode coordinates.
- Ambiguous identity remains unresolved.
- Do not make a remote mapping service an availability dependency for Floppy state.
- Do not force video seconds onto non-video domains. A later general activity contract must declare progress units.

## Performance and offline rules

- Incremental synchronization is the normal path; full reconciliation is exceptional.
- Avoid full-library serialization for one page.
- Use bounded pages, batching, indexed lookups, bounded retries, and bounded memory.
- Correctness must not depend on Docker service names, `/app` paths, Redis availability, Celery, one reverse proxy, or an external provider being online.
- Core state transitions must remain callable in a packaged/in-process runtime.
- Keep SQLite and PostgreSQL equivalent for correctness-critical behavior.
- Measure snapshot time, delta time, p95 page latency, query count, writes, worker duration, queue wait, memory, retries, unresolved rate, and change-log/receipt growth before making performance claims.

## django-allauth decision and Release 2 direction

Floppy already uses django-allauth for browser account/session/social identity. Third-party `IntegrationToken` credentials are Floppy scoped credentials and are not allauth tokens.

Do not collapse account authentication and delegated integration authorization.

One canonical resource-scope vocabulary must drive PAT validation, capability discovery, OpenAPI, AsyncAPI, OIDC consent/permissions, Nuvio conformance, and user-facing permission text.

Release 2 direction:

- allauth Headless session tokens for first-party packaged/mobile Floppy clients;
- allauth OIDC provider for delegated Nuvio pairing;
- Device Authorization Grant for TV flows;
- Authorization Code + PKCE for browser/desktop/mobile flows;
- opaque access tokens by default;
- refresh rotation and revocation;
- PAT remains an advanced/offline compatibility path.

Current branch work includes a durable authentication-boundary document, centralized scope policy, feature-gated Headless/OIDC settings, runtime entry points using that settings overlay, a Floppy OIDC adapter, and feature-gated Headless/OIDC routes.

The dependency remains locked to django-allauth `65.15.0`. A dependency-only 65.18 change was reverted because `uv.lock` was not atomically regenerated. Complete the upgrade with the repository-pinned resolver; do not hand-edit the lock file.

Release 2 still needs full OIDC token-to-Floppy-scope enforcement, client provisioning, device/PKCE conformance, refresh/revoke/disabled-user tests, OpenAPI/AsyncAPI updates, pairing/re-auth UX, and complete QA/security validation.

## Local Shared Media Workspace direction

Later work should share references and projections, not application databases.

Best first slice: expose Floppy as a read-only Stremio-compatible local add-on with selected list/Discover catalogs and normalized metadata. This aligns with #635.

Later primitives may include declarative add-on installation identity, portable Collection descriptors, metadata projections, user metadata overrides, cache freshness/invalidation, and paired Nuvio self-host APIs.

Do not implement unrestricted metadata writeback, direct shared DB tables, automatic plugin execution, plugin binary sync, stream/debrid functionality in Floppy, or raw Nuvio secrets.

## Security invariants

- Scope every object access through the authenticated user/client.
- Scoped credentials fail closed on undeclared routes.
- Store long-lived integration secrets as digests; return raw secret once.
- Raw secrets do not enter logs, metrics, cache keys, screenshots, errors, or normal exports.
- Cursors and receipts are client-bound.
- Bounded body/parser/page/retry/concurrency limits are release requirements.
- Public errors are stable and do not expose raw exceptions or provider/database internals.
- Later remote add-on/metadata fetches require one SSRF-safe fetch boundary with DNS and redirect revalidation.
- Floppy executes no downloaded add-on/plugin code.

## UX rules

Normal users should not have to understand OAuth headers, API token formats, or sync internals.

The target experience is: Connect Nuvio → approve clear capabilities → select profile/directions → see reconciliation → persistent health/recovery.

Manual PAT creation belongs under an advanced path.

Use one primary action, stable terminology, persistent error/recovery information, keyboard access, visible focus, reduced motion, narrow-layout support, progressive disclosure, and explicit affected counts before destructive actions.

## Canonical relationships

Floppy: #532, #636, #635, #599, #598. Completed compatibility baselines: #619, #723, #429, #417. #652 is historical context only.

Nuvio: NuvioTV #2935, #2484, #2967.

Before final release, search current Yamtrack/upstream issues/PRs and update `UPSTREAM_PORTS.md` for accepted/adapted/superseded/deferred/discarded outcomes.

## Release definition

Do not call Release 1 or Release 2 production-ready until the final reconciled head has:

- full green tests;
- migration replay;
- lint and contract checks;
- CodeQL/security review;
- measured performance evidence;
- offline/reconnect evidence;
- SQLite/PostgreSQL evidence;
- UI `/qa` and accessibility evidence;
- OpenAPI/AsyncAPI/docs updates;
- screenshots for visible changes;
- relationship links;
- rollback evidence;
- a post-mortem.

Compatibility claims must be pinned to explicit NuvioTV and Nuvio self-host revisions, not unknown future versions.

## Operating principle

> Standardize semantics, not vendors. Keep each state concept exact. Make retries, deletes, offline recovery, and permissions explicit. Give integrations one stable contract and make independent adoption cheap.

Refs #532 #636 #635 #599 #598 #619 #723 #429 #417.
