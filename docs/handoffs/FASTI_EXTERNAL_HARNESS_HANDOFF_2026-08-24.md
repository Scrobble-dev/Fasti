# Fasti external-harness handoff — 2026-08-24

## Purpose

This file is the self-contained continuation entry point for a new coding harness that has no prior ChatGPT, Codex, Claude, or local terminal context.

Read this file before changing code. Then read `FASTI_MASTER_INTEGRATOR_HANDOFF.md`, `AGENTS.md`, `SECURITY.md`, the current PR conversation, and the referenced review artifacts.

Do not infer completion from plans. Repository state and exact-head evidence control.

---

## Current repository state

Repository: `Scrobble-dev/Fasti`

Primary PR: `#14` — `B0-B2: harden the contract foundation and review the local kernel`

Base branch: `release`

Canonical working branch: `security/b1-evidence-hardening-20260822`

Canonical head when this handoff was written:

```text
12743317c694a8b547cfce8443d49e252d7d900e
```

The older working branch `security/b1-b2-foundation-20260822` has been intentionally fast-forwarded to the same commit. At this handoff both branches are byte-identical at `12743317c694a8b547cfce8443d49e252d7d900e`.

**Do not split future implementation between those branches.** Continue on `security/b1-evidence-hardening-20260822` because PR #14 points there. Treat `security/b1-b2-foundation-20260822` as a synchronized compatibility ref only unless a maintainer explicitly changes this rule.

`security/test-noop-do-not-use` is not an implementation branch.

---

## Exact-head QA receipts

At commit `12743317c694a8b547cfce8443d49e252d7d900e` the required GitHub Actions passed:

| Workflow | Result | Run |
| --- | --- | --- |
| CI | PASS | `32674142123` / run 117 |
| Governed Contract Conformance | PASS | `32674142116` / run 117 |
| Security Audit | PASS | `32674142120` / run 118 |

The CI receipt includes successful repository-truth checks, no-publish policy checks, Rust formatting, Clippy with warnings denied, workspace tests, all-target build, JavaScript formatting/typechecks/tests, exact-source snapshot, OCI build, non-root and health smoke checks, guarded false-success checks, and memory/benchmark sentinels.

The contract receipt verifies the governed OpenAPI, AsyncAPI, JSON Schema, JSON-LD, OKF, examples, and SDK parity.

The security receipt runs `cargo-audit` over the workspace lockfile and the isolated Tauri benchmark lockfile.

A receipt is valid only for the exact commit recorded by that workflow. Any code or docs write requires fresh exact-head receipts.

---

## Product boundary

> Fasti records. Players play.

Fasti is a local-first Chronicle and identity system. It does not own playback, source selection, transcoding, streaming, or another provider's internal database.

External systems submit observations through capabilities and adapters.

---

## Architectural invariants

### Identity

Fasti owns stable internal identity.

External identifiers such as IMDb, TMDB, TVDB, AniList, MAL, SIMKL, Nuvio, or Stremio identifiers are evidence. They are not canonical record identity.

Never silently merge records because provider identifiers appear compatible.

### Chronicle

Keep these concepts separate:

- observation — what a source reported;
- occurrence — the durable viewing/listening event represented by an accepted observation;
- interpretation — the current meaning assigned to that evidence;
- correction — an append-only replacement interpretation.

Do not rewrite original evidence or occurrences to make history look cleaner.

### Dependency direction

```text
Domain
  -> Application
      -> Contracts
          -> adapters
              -> SQLite / HTTP / CLI / UI / provider integrations
```

Domain and application semantics must not depend on Axum, SQLite, Tauri, a provider SDK, or UI types.

### Security

- Fail closed on ambiguous grants, invalid epochs, revoked credentials, invalid scopes, and cross-workspace access.
- Keep credentials out of URLs, logs, examples, and JSON bodies unless a one-time governed response explicitly owns secret delivery.
- Authorize inside the same transaction that performs sensitive local mutations.
- Keep resource use bounded.
- Keep generic integration boundaries provider-neutral.
- Do not enable remote listeners or executable add-ons by accident.

---

## Implementation state by body

| Body | Current state | What remains |
| --- | --- | --- |
| B0 | Implemented and continuously checked | Keep repository truth/no-publish gates green |
| B1 | Software contract foundation green | Physical Pi 5 and J4125 receipts remain external evidence |
| B2 | Local kernel implemented behind application ports; exact-head software QA green | Process crash/restart, supported physical power-cut, and constrained-hardware evidence; public B2 activation remains governed work |
| B3 | **Correction chain implementation is now present** | Export, restore, equality verification, offline restore, crash matrix, final B3 contract activation decision |
| B4 | Not implemented | Product/review UX, recovery and continuity states, a11y validation |
| B5 | Not implemented | Provider-neutral metadata claims/projections without identity ownership |
| B6 | Not implemented | Neutral conformance clients and fixtures for external integrations |
| B7 | Not implemented | Nuvio adaptation after neutral conformance; pairing, observations, sync, later catalogs/collections/metadata |
| B8 | Not implemented | Native/package/OCI release hardening and release evidence |

---

## What changed immediately before this handoff

### B3 correction application boundary

`crates/fasti-application/src/corrections.rs`

The application layer now defines:

- `CorrectionTarget`;
- `AppendCorrectionCommand`;
- `InspectCorrectionChainQuery`;
- bounded correction reason and chain limits;
- correction outcome/view types;
- `CorrectionPort`.

This layer contains no rusqlite/Axum/UI/provider types.

### B3 correction persistence

`crates/fasti-store/src/correction.rs`

The SQLite adapter:

1. authorizes inside the transaction;
2. proves the observation is in the caller's workspace/profile scope;
3. refuses correction while a review item is still open/deferred;
4. finds exactly one current interpretation leaf;
5. appends a replacement interpretation instead of mutating the original;
6. records the actor client and reason;
7. updates a resolved review item's current interpretation pointer;
8. commits atomically.

### Schema v2

`crates/fasti-store/src/schema.rs`

Schema version 2 adds `corrections` with references to the original observation, prior interpretation, replacement interpretation, actor, optional record, reason, and timestamps. Migration tests cover fresh install and v1 -> v2 upgrade.

### Staged correction permissions

`CorrectionRead` and `CorrectionWrite` exist as internal scope keys. They are granted only inside store test fixtures for the staged B3 tests. They are **not** added to the normal B2 admin scope set and are not a public-production permission claim.

---

## Security review of the current B3 slice

### Reviewed boundaries

- correction authorization and workspace/profile scoping;
- record lookup scope;
- append-only interpretation semantics;
- correction-chain leaf selection;
- transaction behavior;
- schema uniqueness and foreign keys;
- bounded reason/page sizes;
- staged scope isolation;
- public-contract non-drift.

### Current disposition

No critical or high-severity reachable vulnerability was identified in this B3 correction slice during the continuation review.

The important properties are:

- `TransactionBehavior::Immediate` serializes the correction write path and limits competing leaf updates;
- the caller cannot correct an observation from another workspace/profile through the application port;
- a target record must exist in the caller's workspace;
- the original observation, evidence, and occurrence remain intact;
- the schema makes each prior and replacement interpretation unique within correction records;
- public B1 contract artifacts remain unchanged while B3 is staged internally.

### Security items for the next harness to verify

1. Add an integrity verification rule that proves every correction's prior/replacement interpretation belongs to the same observation and occurrence. Application code currently establishes this, but B3 export/restore integrity verification should prove it from stored data.
2. Add explicit cross-workspace/cross-profile correction regression tests if not already present after the handoff commit.
3. Add concurrent correction tests that attempt competing replacements and verify exactly one valid leaf chain remains.
4. Keep correction reasons untrusted at presentation boundaries. Future UI must escape them and should reject disruptive control characters at the contract/UI boundary if those surfaces become public.
5. Do not add correction scopes to default production grants merely to make tests easier.

These are B3 hardening requirements, not reasons to remove the append-only design.

---

## QA / testing method to continue

Use the repository's existing gates as the executable `/qa` baseline:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --all-targets --locked
cargo xtask contract verify --locked
pnpm install --frozen-lockfile
pnpm format:check
pnpm typecheck
pnpm test
```

Also run repository-owned truth/no-publish scripts and OCI smoke as CI does.

Follow the gstack QA pattern: test -> fix -> re-test -> record evidence. Do not treat a code change as complete just because it compiles.

For developer-facing work, follow the gstack devex-review principle: test the actual onboarding/CLI/docs path and distinguish measured evidence from inference.

For architecture work, follow the gstack plan-eng-review principle: search for existing capabilities first, challenge duplicate abstractions, keep changes explicit and reversible, and prefer the smallest complete design that fits existing boundaries.

Ponytail is useful as a pressure against unnecessary new abstractions. Reuse existing ports/capabilities before introducing parallel services.

---

## Public contract rule

Do not partially expose staged B2/B3 functionality.

When a capability becomes public, update the governed surfaces together as applicable:

- capability registry;
- OpenAPI;
- AsyncAPI;
- JSON Schema;
- JSON-LD / OKF;
- examples;
- TypeScript SDK;
- CLI;
- permission/problem catalogs;
- documentation.

The current production daemon is still intentionally health-only for these staged capabilities. Internal implementation does not authorize claiming public support.

---

## Offline, package, and performance rules

- The semantic owner is the native local application, not Docker.
- SQLite and local filesystem operation must remain useful without network access.
- Do not add a mandatory broker, Redis, hosted database, cloud identity service, or provider API to the Chronicle core.
- OCI must run the same daemon/CLI semantics and remain non-root.
- Packaged/Tauri work must use the same application ports rather than duplicate business logic.
- Preserve bounded queues, payloads, pages, evidence uploads, retries, and streams.
- Re-run memory and benchmark evidence after material runtime changes.

Current hardware evidence deferrals:

```text
TODO(evidence): Raspberry Pi 5 native + OCI ownership/fingerprint/memory receipts
TODO(evidence): Intel J4125 native + OCI ownership/fingerprint/memory receipts
TODO(evidence): supported physical power-cut durability receipt
```

These are explicit evidence deferrals. They must not block unrelated coding, but they do block the milestone/release claim that depends on them.

---

## B3 next sequence

Do this before starting B4-B8 breadth:

1. Harden correction-chain integrity and concurrency tests.
2. Implement deterministic local export of Chronicle/identity/correction state and an archive manifest.
3. Implement stopped-node restore into a clean target.
4. Add deterministic equality verification between source and restored state.
5. Test offline export/restore.
6. Add crash/interruption tests around export/restore activation.
7. Decide and document whether B3 remains internal or activates governed public contracts in the same change.

### Export must include

- workspaces/profiles needed to interpret data;
- provider-neutral records;
- external identifier claims;
- observations and clues;
- occurrences;
- interpretations;
- review state needed for restoration;
- receipts/operations needed for idempotency semantics;
- corrections;
- evidence manifest/digests and required local evidence content or a clearly versioned inclusion policy.

### Export must not include

- active credential secrets;
- initialization proof material;
- unrelated local machine secrets.

---

## B4-B8 continuation map

### B4 — product experience

Build user-facing review/correction/recovery flows over existing application ports. Requirements include WCAG 2.2 AA intent, keyboard operation, clear focus, predictable grouping, visible state, interruption recovery, saved position, understandable errors, reduced motion support, and low cognitive load. Do not expose provider graph complexity unless the user needs it to make a decision.

### B5 — metadata projections

Metadata is a set of sourced claims/projections attached to Fasti identity. Provider changes must not move history. Store provenance and allow conflicts rather than silently overwriting identity.

### B6 — neutral conformance

Before Nuvio-specific shortcuts, build provider-neutral conformance fixtures that prove the same application contract can be used by multiple client shapes. Reuse capabilities and test retry/idempotency/offline behavior.

### B7 — Nuvio

Implement in layers:

```text
B7a pairing + durable observation submission
B7b progress / watchlist / watched-state reconciliation
B7c catalogs / collections / metadata projections
```

Preserve the product boundary. Do not directly couple Fasti to the Nuvio database or import Nuvio's internal domain into the Chronicle.

Relevant context:

- `dannyvfilms/Floppy#791`
- `dannyvfilms/Floppy#532`
- `dannyvfilms/Floppy#636`
- `NuvioMedia/NuvioTV#2935`
- `FuzzyGrim/Yamtrack`

These are context and compatibility references, not architecture owners.

### B8 — release hardening

Prove native and packaged distribution, OCI parity, offline installation/operation, upgrade/rollback, constrained-hardware budgets, release provenance, and publish controls. Do not enable publishing merely because implementation exists.

---

## Required artifacts from the next harness

For every meaningful section of work, leave enough evidence that another harness can continue without chat history:

1. atomic commit(s) with exhaustive rationale and verification;
2. updated handoff/status section;
3. exact-head CI / contract / security receipts;
4. regression tests for fixed bugs;
5. security disposition and threat boundary notes;
6. contract disposition: changed / unchanged / deferred;
7. offline/package disposition;
8. performance/memory disposition;
9. a11y/UX disposition when UI exists;
10. rollback instructions;
11. post-mortem for regressions or process failures;
12. TODO/evidence deferrals with owner/evidence type rather than vague prose;
13. PR conversation comment that links the durable repository artifact.

Screenshots are required for UI changes when they add evidence. Do not fabricate screenshots for non-UI work.

---

## Known process failure and post-mortem

During B3 continuation, source was written through narrow repository operations without first reproducing the full local formatter/compiler pass. This produced a sequence of exact-head failures: first `cargo fmt`, then Clippy found an unused import and a transaction/statement lifetime error. The branch was repaired and exact-head CI, contract conformance, and security audit are now green at `12743317...`.

Corrective rules:

- run formatter before committing Rust changes;
- do not stop at formatting: run Clippy and focused tests locally when the harness can;
- when remote tooling cannot execute the full suite, use GitHub Actions as the authoritative exact-head executor and inspect the failing job log before the next write;
- make the smallest fix that addresses the observed failure;
- do not weaken warnings, contract checks, or security gates to get green;
- after a write, old receipts are stale.

---

## Startup sequence for a new harness

1. Checkout `security/b1-evidence-hardening-20260822`.
2. Confirm HEAD matches PR #14 and note if it has moved past `12743317...`.
3. Compare `security/b1-b2-foundation-20260822` to the canonical branch. Do not create parallel work if they still match.
4. Read:
   - `AGENTS.md`;
   - `SECURITY.md`;
   - `docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md`;
   - this file;
   - `docs/reviews/2026-08-23-b2-continuation-security-qa.md`;
   - PR #14 conversation.
5. Run the exact local QA suite that the environment supports.
6. Inspect current CI before making changes.
7. Continue B3 from the sequence above.
8. Record hard evidence deferrals instead of blocking all coding on unavailable hardware.
9. Do not mark PR #14 ready or merge while its declared evidence gates remain open unless the PR scope/status is deliberately redefined by a maintainer.

---

## Final rule

A future harness must be able to distinguish three states without guessing:

```text
implemented
verified at exact head
deferred with explicit evidence requirement
```

Never collapse those into a single word such as "done".