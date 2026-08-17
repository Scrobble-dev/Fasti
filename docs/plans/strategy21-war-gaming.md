# Strategy21: Floppy–Nuvio Adversarial Review

**Status:** Active security review; implementation hardening in progress  
**Date:** 2026-08-17  
**Repository:** `dannyvfilms/Floppy`  
**Pull request:** #791  
**Branch:** `plan/nuvio-integration-reviewed-2026-08-15`

---

## 1. Review correction

Earlier versions of this document said the programme was "100% hardened" and that all high-severity failure modes had verified code-level mitigations. That statement was not supported by the implementation.

The current review found concrete security gaps in the initial scoped-token and idempotency work. PR #791 is being used to fix those gaps before the next sync package proceeds.

A design mitigation is not complete until the code, tests, migration behavior, and release checks verify it.

---

## 2. Current findings and disposition

| ID | Finding | Risk | Current disposition |
|---|---|---:|---|
| **F1** | Integration tokens authenticated, but normal API endpoints did not enforce the token's scopes. | High | **Fixed on #791.** Integration tokens now use a deny-by-default endpoint/method scope map for the currently supported scrobble and progress surfaces. Legacy account tokens keep their compatibility behavior. |
| **F2** | The original receipt gateway executed the protected mutation before inserting the unique receipt. Concurrent requests or a response-loss window could execute the same mutation more than once. | High | **Fixed on #791.** The receipt is now reserved before execution. An uncertain crash leaves a reservation that blocks automatic replay. |
| **F3** | Idempotency keys had no explicit size or character bound. | Medium | **Fixed on #791.** Keys are bounded and control/whitespace input is rejected before receipt storage. |
| **F4** | Some existing API views return `str(exception)` in JSON error responses. | Medium | **Open.** Remove raw exception strings from public API responses and keep full details in server logs only. |
| **F5** | Receipt uniqueness is currently user + client event ID, not a durable authenticated client namespace. Two integrations for one user can collide if they intentionally or accidentally reuse the same key. | Medium | **Open before broad multi-client release.** Define the client namespace and migrate without breaking existing receipts. |
| **F6** | Receipt rows do not yet have a completed retention/compaction policy. Token deletion can also remove token-linked receipts because the current foreign key cascades. | Medium | **Open before Release 1.0.** Retain enough replay history for supported clients and preserve safety across token lifecycle operations. |
| **F7** | SafeFetch is specified but not implemented. | Critical when remote user-configured URLs are accepted | **Blocked until PR9.** Do not accept generic remote add-on URLs before the safe-fetch boundary exists and passes SSRF tests. |
| **F8** | Ordered progress tombstones, cursor expiry, and snapshot/delta race handling are specified but not implemented. | High for incremental sync correctness | **Next sync work.** PR3 must implement and test these rules before clients depend on incremental changes. |
| **F9** | The future packaged/offline execution seam is planned but not fully implemented. | Medium | **Release 1.0 gate.** Core state transitions must remain usable without making Redis, Celery, or Docker the source of truth. |

---

## 3. Adversarial scenarios

| Scenario | Trigger | Failure if unprotected | Required control | Status |
|---|---|---|---|---|
| **Nuvio discovery changes** | `/.well-known/nuvio` changes shape or is unavailable. | Pairing fails or unsafe fallback guesses configuration. | Version/capability validation and explicit manual fallback. Do not depend on discovery for stored-state correctness. | Planned |
| **Offline for several days** | Client cannot reach Floppy or Nuvio. | Lost local changes, replay storms, or deletion from an empty response. | Durable local intent, ordered replay, idempotency, explicit deletes, bounded backlog, fresh snapshot on expired cursor. | Partial |
| **Concurrent duplicate request** | Same idempotency key arrives in parallel. | Duplicate state mutation or history event. | Commit receipt reservation before protected mutation; unique constraint; retry returns replay/conflict without a second execution. | Implemented on #791; CI pending |
| **Response lost after mutation** | Server commits state, then connection dies before response. | Client retries and applies the mutation again. | Pre-existing reservation blocks automatic second execution when final outcome is uncertain. Reconciliation determines current state. | Implemented on #791; CI pending |
| **Scope escalation** | Read-only integration token calls a mutation or unrelated API. | Third-party client gains account-wide authority. | Deny-by-default route/method scope enforcement and cross-scope tests. | Implemented on #791; CI pending |
| **Malformed/oversized event key** | Client sends a huge or control-character idempotency key. | Storage/log abuse or ambiguous receipt identifiers. | Length and visible-character bounds before storage. | Implemented on #791; CI pending |
| **Sync echo storm** | Two clients reflect each other's writes. | Database growth and repeated writes. | Server-derived client origin, change-feed origin, replay receipts, and client acknowledgment without write-back. | Partial; origin namespace still pending |
| **Clock skew** | Client clock is far ahead or behind. | Timestamp ordering overwrites newer state. | Use monotonic server sequence for transport order. Treat client time as observation metadata only. | Planned for PR3 |
| **Cursor tampering** | Client modifies cursor or uses another user's cursor. | Skipped, replayed, or cross-user events. | Opaque user/resource-bound cursor with validation, bounds, and expiry. | Planned for PR3 |
| **Tombstone compaction** | Client returns after delete events were compacted. | Deleted state can reappear or client cannot converge. | Document retention. Expired cursor forces a fresh snapshot. | Planned for PR3 |
| **Partial/empty remote page** | Provider timeout or partial result looks like absence. | Valid state is deleted. | Delete only from explicit tombstone or approved authoritative reconciliation. | Required invariant |
| **SQLite write pressure** | Several local clients write at once. | Lock errors or latency spikes. | Existing WAL/busy-timeout settings, short database write transactions, bounded batches, measured concurrency tests. | Existing mitigation; not a guarantee |
| **Redis/Celery unavailable** | Cache/broker stops or packaged build omits them. | User-requested work disappears or state becomes unusable. | Durable user state in the database; cache is optional; transition logic callable without scheduler ownership. | Partial; release test pending |
| **Malicious remote add-on** | Configured URL resolves/redirects to loopback, LAN, link-local, or cloud metadata. | SSRF, credential theft, internal network access. | Central safe-fetch boundary with DNS/IP and redirect revalidation, limits, and narrow admin-approved local exceptions. | Not implemented; generic remote URLs remain blocked |
| **Hostile upstream JSON** | Remote response is huge, deeply nested, malformed, or contains unsafe markup/URLs. | Memory/CPU exhaustion or XSS. | Content/type/size/depth/count limits, escaped rendering, safe URL schemes, failure isolation. | Planned for remote add-on work |
| **Token lifecycle change** | Token is revoked/deleted while receipts still protect old deliveries. | Replays can lose deduplication evidence or revoked access can persist through cache. | Revocation is authoritative; receipt retention is independent from token secret lifetime; no auth cache bypass. | Revocation works; receipt lifecycle still open |
| **Large library reconciliation** | Tens of thousands of items are compared. | Worker starvation, O(N²) behavior, memory spikes. | Incremental normal path, bounded chunks/pages, indexes, query budgets, dedicated queue policy, resumable reconciliation. | Release performance gate |

---

## 4. Security invariants

Release 1.0 must prove these behaviors:

1. Every object and state query is scoped to the authenticated user.
2. An integration credential can access only explicitly supported endpoint/method scopes.
3. An undeclared endpoint is denied to integration credentials by default.
4. Revoked and expired credentials cannot authenticate.
5. Raw credentials do not enter logs, metrics, screenshots, cache keys, or normal exports.
6. A known duplicate delivery never executes the protected mutation twice.
7. A request with an uncertain prior outcome fails closed instead of replaying automatically.
8. A changed payload under the same idempotency key returns a conflict.
9. Delivery identity is not used as playback-occurrence identity.
10. A legitimate rewatch remains a distinct occurrence.
11. Delete requires explicit intent or an approved authoritative reconciliation.
12. An outage, empty page, partial page, timeout, or cache miss cannot delete state.
13. Transport ordering uses server sequence, not wall-clock time.
14. Cross-user/profile cursors and state are rejected.
15. Ambiguous or title-only identity cannot receive authoritative state.
16. Cache is not the only copy of user-owned state.
17. Remote configured URLs are not accepted until SSRF-safe fetching is in place.
18. Floppy does not execute downloaded add-on/plugin code.
19. Migrations preserve current SQLite and PostgreSQL user data.
20. Failure is visible and gives the user a safe recovery action.

---

## 5. Release review requirements

Before any package is described as hardened:

- targeted regression tests pass;
- Ruff/lint passes;
- migration hygiene and upgrade replay pass when schema changes;
- the application fast suite passes;
- CodeQL and the repository security checks pass;
- the changed security boundary receives a focused security diff review;
- visible UI changes receive browser QA and accessibility verification;
- performance-sensitive paths have measured results;
- rollback/kill-switch behavior is documented and tested where applicable.

Do not convert this document back to a "100% secure" statement. Security status must reflect actual implementation and test evidence.
