# Fasti Master Integrator Handoff

**Status:** Current onboarding entry point for a new implementation harness  
**Date:** 2026-08-24  
**Repository:** `Scrobble-dev/Fasti`  
**Primary active implementation PR:** [#14](https://github.com/Scrobble-dev/Fasti/pull/14)  
**PR branch:** `security/b1-evidence-hardening-20260822`  
**Target branch:** `release`

---

## 1. Purpose

Use this document when a new engineer or autonomous coding harness joins Fasti without access to prior conversations.

It is a map, not a substitute for the repository. It explains:

- what Fasti is;
- which documents control decisions;
- what is implemented;
- what remains open;
- which boundaries must not change;
- how to review, test, document, and ship work;
- how B4 through B8 fit after the current foundation;
- how to avoid importing obsolete Floppy, Nuvio, or player assumptions.

Before changing code, read this file and then inspect the current PR, branch, checks, and repository guidance. Never treat this document's date-stamped status as more current than the live repository.

---

## 2. Product boundary

> **Fasti records. Players play.**

Fasti is a local-first media Chronicle and provider-neutral identity system.

Fasti accepts observations from players, trackers, imports, local clients, automation, and explicit user actions. It preserves what happened, what evidence was supplied, how the event is currently interpreted, and how that interpretation changed.

Fasti does **not**:

- decode media;
- stream or transcode media;
- select playback sources;
- become Nuvio, Kodi, Stremio, Jellyfin, Plex, or VLC;
- treat one metadata or tracker provider as canonical identity;
- make remote availability a prerequisite for local recording;
- rewrite original evidence when metadata improves;
- infer deletion from absence, timeout, cache miss, or provider failure.

External playback handoff may exist later as an action. It does not prove that consumption occurred.

---

## 3. Source-of-truth hierarchy

When sources disagree, use this order:

1. **Current repository source and current PR diff.**
2. **Current exact-head CI, QA, security, and evidence receipts.**
3. **Approved product constitution and definition of done.**
4. **Approved engineering and test plans.**
5. **Capability, contract, error, and schema registries.**
6. **Current issue and PR decisions.**
7. **Historical research and predecessor-project lessons.**

Do not allow an old report, chat transcript, or roadmap summary to override current source.

### Primary repository documents

On the active implementation branch, read:

- `AGENTS.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `SUPPORT.md`
- `GOVERNANCE.md`
- `docs/constitution.md`
- `docs/glossary.md`
- `docs/definition-of-done.md`
- `docs/capability-ledger.md`
- `docs/architecture/`
- `docs/reviews/`
- `.github/PULL_REQUEST_TEMPLATE.md`
- `.github/workflows/`

### Approved planning and evidence sources

The current programme was derived from these named artifacts. Search the repository, PR, attached evidence, or the original handoff pack for their current location:

- `winks-HEAD-design-20260821-182751(1).md`
- `winks-HEAD-engineering-plan-20260821-201100(1).md`
- `winks-HEAD-eng-review-test-plan-20260821-195019(1).md`
- `winks-vscode-fasti-b0-truth-reset-design-audit-20260822-030500(1).md`
- `winks-vscode-fasti-b0-truth-reset-test-outcome-20260822-063900.md`
- `tasks-ceo-review-20260821-230057-final(1).jsonl`
- `tasks-autoplan-devex-review-20260821-230057(1).jsonl`

If an artifact is not in the repository, ask for it or inspect the PR evidence pack. Do not invent its content.

---

## 4. Current programme state

The live PR remains the authoritative status source. At the time this handoff was written:

| Body | Outcome | Current state |
| --- | --- | --- |
| **B0** | Truth reset and repository alignment | Complete |
| **B1** | Executable contract foundation | Software scope substantially complete; physical evidence remains open |
| **B2** | Local identity and observation kernel | Implemented behind review boundaries; public activation and final evidence remain open |
| **B3** | Correction and portability | Not complete |
| **B4** | Product experience and review workflows | Not started as a production feature set |
| **B5** | Metadata claims and projections | Not started |
| **B6** | Neutral source conformance | Not started |
| **B7** | Nuvio integration lanes | Not started |
| **B8** | Distribution and release hardening | Partial foundations only; not complete |

Do not call B1 or B2 complete merely because code exists. A body is complete only when its required exact-head evidence and hardware gates pass.

### Active PR

- PR: [Scrobble-dev/Fasti#14](https://github.com/Scrobble-dev/Fasti/pull/14)
- Title: `B0-B2: harden the contract foundation and review the local kernel`
- State: Draft
- Base: `release`
- Head: `security/b1-evidence-hardening-20260822`

The PR intentionally keeps production behavior narrow while B2 is reviewed. Do not expose staged B2 behavior through public contracts one surface at a time.

---

## 5. Current implementation boundary

The active branch contains or reviews these foundations:

- provider-neutral typed identity;
- immutable observations;
- separate occurrences and interpretations;
- workspace, profile, client, credential, and grant boundaries;
- loopback-first bootstrap and enrolment;
- credential rotation and revocation;
- SQLite persistence;
- content-addressed evidence storage;
- operation-level idempotency;
- durable receipts and replay;
- bounded receipt streaming;
- review inspect, defer, resume, and resolve flows;
- generated or governed OpenAPI, AsyncAPI, JSON Schema, JSON-LD, OKF, examples, and TypeScript SDK surfaces;
- native and OCI review paths;
- supply-chain and workflow hardening.

Production must not claim more than the composition root actually exposes.

### Remaining B2 gates

- process crash and restart proof;
- commit-before-reply replay proof on the final public path;
- supported power-loss evidence;
- Raspberry Pi 5 ownership, fingerprint, native-memory, and OCI-memory receipts;
- J4125 ownership, fingerprint, native-memory, and OCI-memory receipts;
- final public DTO and transport activation as one governed change;
- exact-head CI, contracts, security, package, and `/qa` evidence after every final write.

Hosted-runner evidence does not replace the two required physical profiles.

---

## 6. Core semantic rules

### 6.1 Stable identity

Fasti owns opaque, permanent IDs. Provider IDs are evidence-bearing coordinates.

```text
Fasti Record
├── IMDb assertion
├── TMDB assertion
├── TVDB assertion
├── MAL assertion
├── AniList assertion
├── Kitsu assertion
└── source-scoped assertion
```

Never use a provider ID as the permanent key for Chronicle state.

Each external-identity assertion must preserve, where applicable:

- namespace;
- normalized identifier;
- entity grain;
- relation type;
- direction;
- coverage or range;
- evidence class;
- provenance;
- source version;
- lifecycle state;
- review state.

Title similarity can rank candidates. It cannot authorize an irreversible merge or state movement.

### 6.2 Observation, occurrence, and interpretation

Keep these separate:

- **Observation:** what evidence arrived;
- **Occurrence:** the durable media-activity event;
- **Interpretation:** which record or segment Fasti currently believes the observation describes;
- **Correction:** a new interpretation revision that preserves the old one.

```text
Original observation
      !=
Current interpretation
```

A corrected interpretation never rewrites the original bytes, timestamps, provenance, or prior decision.

### 6.3 Separate user states

Do not collapse:

- progress;
- completion or watched state;
- saved state;
- history occurrence;
- rating;
- list membership;
- metadata claim;
- user override.

A duplicate delivery is not a rewatch. A progress reset does not delete history. A provider metadata change does not move Chronicle events.

### 6.4 Explicit deletion

Deletion requires an explicit governed operation, tombstone, privacy-erasure path, or approved authoritative reconciliation.

Never delete because:

- an item is missing from a page;
- a provider returned an empty result;
- a request timed out;
- a cache missed;
- metadata returned 404;
- a cursor or page was incomplete.

### 6.5 Idempotency

Idempotency identity is scoped to the authenticated client and operation.

Required behavior:

- same operation + same capability + same semantic digest -> replay original receipt;
- same operation + changed capability or digest -> typed conflict;
- revoked client -> denied before receipt disclosure;
- credential rotation for the same active client -> replay remains available;
- mutation and receipt finalization share one durable transaction boundary on the final path.

### 6.6 Server ordering

Transport ordering uses server-owned monotonic sequence or durable cursor semantics. Client clocks are evidence, not ordering authority.

---

## 7. Architecture and ownership

Use inward dependency direction:

```text
Domain
  <- Application
      <- Contracts
          <- Adapters
              <- HTTP / SSE / CLI / SQLite / Filesystem / UI
```

### Expected ownership

| Area | Owns |
| --- | --- |
| Domain | entities, value objects, invariants, state transitions, domain errors |
| Application | capabilities, orchestration, authorization decisions, ports, transaction boundaries |
| Contracts | versioned DTOs and public serialization bindings |
| Store | SQLite repositories, migrations, writer queue, blob/archive filesystem primitives |
| API | routing, authentication extraction, request limits, problem rendering, SSE |
| CLI | command grammar, structured output, exit codes, local/remote selection |
| `xtask` | contract generation, truth checks, evidence validation, test orchestration |
| UI | user interaction over the same application capabilities |

Domain and application code must not depend on:

- Axum;
- Clap;
- rusqlite;
- Tokio transport types;
- provider SDKs;
- Svelte or Tauri UI types.

Do not create a generic abstraction until a current capability or hostile fixture needs it. Do not remove authorization, validation, durability, accessibility, or recovery as "simplification."

---

## 8. Contracts

A capability is not complete until every applicable public surface agrees.

Applicable surfaces include:

- capability registry;
- Rust DTOs;
- OpenAPI 3.1;
- AsyncAPI 3.x;
- JSON Schema 2020-12;
- JSON-LD 1.1;
- OKF and governed examples;
- TypeScript SDK;
- CLI;
- error documentation;
- permissions and scopes;
- lifecycle declarations;
- conformance fixtures.

Every surface is one of:

- `required`;
- `later body`;
- `N/A` with a reason.

Never update one public contract surface in isolation. Never expose internal staged error codes through generated public output accidentally.

Expected contract verification command:

```bash
cargo xtask contract verify
```

The verifier should prove deterministic generation, zero drift, schema validation, references, examples, SDK behavior, and deliberate mutation failure.

---

## 9. Security model

### Trust boundary

The initial supported deployment is one person or household operating a local node. Hosted multi-tenant operation needs a separate threat model.

### Required controls

- loopback-only uninitialized node;
- short-lived, one-time bootstrap secret displayed by a trusted local surface;
- explicit workspace/profile/client/capability authorization;
- revocation checked before receipt lookup;
- no secrets in URLs, logs, screenshots, metrics, or ordinary command arguments;
- secure OS credential storage where available;
- encrypted headless keystore with external master-key provision where needed;
- owner-only data and export permissions where supported;
- pre-body authorization and bounded upload admission;
- bounded request body, string, nesting, archive, batch, page, queue, and concurrency limits;
- cross-profile and cross-workspace isolation;
- explicit origin derived from authenticated client identity;
- signed or integrity-bound evidence where required;
- private handling for sensitive findings.

### High-risk review areas

- bootstrap races;
- credential rotation and revocation;
- cross-profile enumeration;
- idempotency aliasing;
- commit-before-reply crash ambiguity;
- cursor tampering or reuse;
- filesystem traversal and archive bombs;
- content-addressed blob races;
- secret leakage;
- network egress and future SSRF;
- workflow write permissions;
- mutable action and OCI references;
- dependency and lockfile drift;
- resource exhaustion.

Read `SECURITY.md` before publishing security detail.

---

## 10. Offline, packaging, and performance

### Offline

Local acceptance, receipt replay, review, correction, export, and stopped-node restore must not require:

- cloud access;
- a provider API;
- Redis;
- Celery;
- an MQTT broker;
- an external database;
- a Fasti-operated service.

Network failure is a normal state. Optional network work happens after the local transaction or through a durable outbox.

### Distribution

Fasti must support the same semantic core through:

- native daemon and CLI;
- OCI image;
- packaged application where applicable;
- source build without Docker being the semantic owner.

Do not hard-code container paths, service names, or one process manager into domain behavior.

### Performance

Current budgets and final thresholds live in the approved test plan and evidence manifests. Do not invent results.

Review for:

- one controlled SQLite writer;
- bounded reader pool;
- bounded busy timeout;
- keyset pagination;
- bounded queues;
- no O(N^2) scans;
- no full-library materialization for one page;
- bounded archive and decompression memory;
- no provider call inside a local write transaction;
- no one-task-per-item explosion without measured justification.

Physical Pi 5 and J4125 receipts remain mandatory where the approved plan says they are.

---

## 11. User experience and accessibility

The product should make people capable of keeping, understanding, repairing, moving, and recovering their record. It should not require them to learn the identity architecture.

Every rendered flow must consider:

- visible system status;
- clear grouping and hierarchy;
- predictable interaction;
- keyboard and screen-reader operation;
- touch and remote operation where applicable;
- visible focus;
- focus restoration;
- target size;
- contrast;
- reduced motion;
- persistent errors and next actions;
- saved progress and interruption recovery;
- progressive disclosure;
- low memory burden;
- no color-only state;
- no transient-only critical information.

`Resolve later` is a valid, safe outcome.

UI work requires `/design-review` as well as `/qa`. Do not fabricate screenshots when no product UI exists.

---

## 12. B3 implementation target

B3 proves correction and portability.

### Correction

- append interpretation revisions;
- preserve original evidence and prior interpretations;
- prevent cycles;
- enforce authorization;
- derive one current interpretation deterministically.

### Export

- full-workspace archive for the initial body;
- deterministic ordering;
- bounded memory;
- manifest and per-entity counts;
- stable IDs and references;
- raw evidence digests;
- no credential secrets or active authentication bindings;
- authorization and revocation checks during bounded output.

### Restore

- stopped-daemon restore;
- shared data-root lock;
- preflight paths, versions, limits, disk, and digests;
- safe same-filesystem staging;
- sync files and directories;
- verified activation marker;
- one complete activation or one rejected staging directory;
- startup recovery that never opens unverified staging;
- clean equality verification;
- network denied.

Do not advertise restore support on a platform until its activation primitives are proven.

---

## 13. B4-B8 roadmap

B4-B8 are important, but they may not bypass the current foundation.

### B4: Product experience

Build the smallest real user interface for:

- Chronicle inspection;
- unresolved identity review;
- correction;
- task resume;
- persistent recovery state;
- operator status.

Use the existing capability model. Do not put private business rules in Svelte or Tauri.

### B5: Metadata claims and projections

Metadata is a replaceable projection, not identity.

Implement:

- provider claims with provenance;
- last-known-good state;
- source preferences;
- language and region;
- user overrides separate from provider claims;
- provider failure that cannot remove Chronicle state;
- purpose-specific resolution routes.

### B6: Neutral source conformance

Build source-neutral fake clients and fixtures before privileged production adapters.

Include representative shapes for:

- Floppy;
- SIMKL;
- Nuvio;
- Web Scrobbler;
- at least one non-video activity source when the generic contract claims support.

No vendor-specific exception may weaken the public observation contract.

### B7: Nuvio lanes

Split Nuvio work explicitly:

- **B7a:** pairing, durable outbox, one-way progress/completion observations, reconnect, receipt and error visibility;
- **B7b:** two-way progress, saved state, watched state, explicit deletion, snapshot/delta, reconciliation, unresolved diagnostics;
- **B7c:** catalogs, Collections, metadata projections, add-on references, cache state.

Do not access Nuvio databases directly. Do not use Nuvio provider IDs as Fasti identity. Do not make playback depend on Fasti availability.

### B8: Distribution and release

Separate:

- **B8a development distribution readiness:** native and OCI packages, offline install, migrations, package smoke, hardware evidence;
- **B8b public release readiness:** signing, SBOM, provenance, updates, final release QA.

B7a may depend on B8a. Avoid a B7/B8 dependency loop.

---

## 14. Later connections and network work

Do not start these before the canonical capability and event contracts are stable:

- mDNS/DNS-SD discovery;
- MQTT connection adapter;
- Home Assistant discovery;
- WebTransport live transport;
- Connector Studio;
- executable plugins;
- shared metadata workspace.

When implemented:

- discovery is not authentication;
- MQTT is not database replication;
- transport QoS does not replace domain idempotency;
- broker input enters through authentication, validation, scopes, and canonical commands;
- no personal data in topic names or mDNS TXT records;
- manual URL, QR, and deep-link fallback remains available.

---

## 15. Predecessor and adjacent-project lessons

Use patterns, not architectures.

### Floppy PR #791

Useful lessons:

- scoped clients;
- exact identity;
- separate progress, saved state, watched state, and history;
- durable receipts;
- explicit deletion;
- snapshot + delta;
- offline replay;
- provider-neutral scopes;
- declarative add-ons separated from executable plugins.

Do not copy:

- Django model ownership;
- Celery/Redis assumptions;
- provider-owned identity;
- Nuvio-specific scopes;
- direct Nuvio database coupling;
- the claim that Fasti is a player.

### Crosswalk work

Use:

- typed directional assertions;
- range-scoped coverage;
- explicit cardinality;
- provenance;
- known absence;
- tombstones and revocations;
- stronger gates for irreversible changes.

Do not use unconstrained transitive closure or license-encumbered datasets as canonical truth.

### Scrobble.dev

Scrobble.dev defines neutral language, schemas, and conformance. Fasti implements them. Fasti does not gain authority to call a Fasti-only behavior a standard.

---

## 16. Review and implementation workflow

Use the workflow, not branding:

```text
Think -> Plan -> Build -> Review -> Test -> Ship -> Reflect
```

For material work:

1. Read repository guidance and this handoff.
2. Inspect current source, PR, issues, reviews, and exact-head checks.
3. Identify the owning capability and bounded context.
4. Search for existing behavior before adding code.
5. Review the plan for product, engineering, design, developer experience, security, offline, performance, and rollback.
6. Implement the smallest complete change.
7. Add regression tests before or with the fix.
8. Update every applicable contract and knowledge surface.
9. Run focused tests.
10. Run full `/qa` and applicable design/security reviews.
11. Record exact commands, outputs, evidence, and residual risks.
12. Update the PR ledger and related issues.
13. Add rollback and postmortem notes.

### Commit requirements

Each meaningful commit or commit group should explain:

- what changed;
- why it changed;
- user or operator impact;
- compatibility effect;
- security effect;
- performance effect;
- validation;
- contract disposition;
- rollback;
- related issues and upstream references.

Use ASD-STE100-style clear language where practical. Do not replace clarity with jargon.

### Issue and PR relationships

Use existing canonical issues where one owns the behavior. Do not create a new issue merely to store an ordinary review finding.

Use explicit relationship lines when native fields are unavailable:

- `Implements:`
- `Refs:`
- `Compatibility baseline:`
- `Upstream:`
- `Supersedes:` only with evidence.

Sensitive security findings follow `SECURITY.md`, not a public issue.

---

## 17. Required validation commands

Confirm current commands from the repository before running them. The approved plan expects a discoverable family similar to:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --all-targets --locked
cargo xtask contract verify
cargo xtask test pr
cargo xtask test deep
cargo xtask test milestone --body B0|B1|B2|B3
cargo xtask evidence verify <manifest>
pnpm install --frozen-lockfile
pnpm test
pnpm typecheck
```

Also verify:

- native offline build;
- OCI build and non-root smoke;
- package path;
- documentation links;
- no-publish policy;
- workflow provenance;
- security/dependency audit;
- exact-head check rollup;
- clean working tree.

A missing, skipped, stale, or unavailable required gate does not count as a pass.

---

## 18. First 48 hours for a new harness

### First hour

1. Read this document.
2. Read PR #14 completely.
3. Read `AGENTS.md`, `SECURITY.md`, `CONTRIBUTING.md`, and the active branch's constitution and definition of done.
4. Fetch the active branch and `release`.
5. Record exact SHAs.
6. Inspect CI and unresolved review threads.

### First half-day

1. Run the fast repository checks.
2. Generate and verify contracts.
3. Map each changed file to its owning context.
4. Reconcile this status table with current source.
5. Identify one bounded next deliverable and its acceptance evidence.

### First day

1. Run focused security and failure-path review.
2. Inspect SQLite transaction, receipt, authorization, evidence, and replay boundaries.
3. Check offline/package behavior.
4. Update the handoff if current source has moved.
5. Do not implement Nuvio, MQTT, plugins, or broad metadata work before understanding B2/B3.

### Second day

1. Implement one accepted, complete slice.
2. Add tests and contract changes.
3. Run focused QA.
4. Run independent review.
5. Update PR evidence, relationships, rollback, and postmortem.

---

## 19. Handoff maintenance template

Update this document or add a dated companion handoff after a material session.

Record:

```text
Date:
Branch:
Base SHA:
Head SHA:
PR:
Completed:
Changed files:
Tests run:
Security review:
Contract disposition:
Offline/package disposition:
Performance disposition:
UI/accessibility disposition:
Open blockers:
Next exact action:
Rollback:
Related issues/upstream:
```

Do not write `PASS` without an artifact, command result, CI result, or physical receipt that supports it.

---

## 20. Current immediate recommendation

Continue in this order:

1. Refresh PR #14 against current `release` without losing either side.
2. Close remaining B2 correctness and evidence gaps.
3. Activate B2 public contracts only as one coordinated change.
4. Complete B3 correction, export, restore, and equality proof.
5. Then implement B4 and B5 as a thin usable local Chronicle.
6. Build B6 neutral conformance.
7. Add B8a package proof needed by integration clients.
8. Implement B7a Nuvio observations.
9. Continue B7b/B7c only after the neutral and distribution gates prove the shared primitives.
10. Complete B8b before a public production release.

The durable objective is:

> A person can keep, understand, repair, move, and recover their media record without surrendering it to one provider.
