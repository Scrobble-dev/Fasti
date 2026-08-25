# Fasti External Harness Context Save

**Status:** Active handoff snapshot  
**Date:** 2026-08-24  
**Audience:** Claude, Codex, or another implementation harness with no access to prior chats, private scratchpads, or local gstack state  
**Repository:** `Scrobble-dev/Fasti`  
**Canonical evergreen handoff:** [`FASTI_MASTER_INTEGRATOR_HANDOFF.md`](FASTI_MASTER_INTEGRATOR_HANDOFF.md)

> This file is a dated context save. It does not replace current source, current pull-request state, or exact-head evidence.

---

## 1. Start here

Before changing code:

1. Read this file.
2. Read [`FASTI_MASTER_INTEGRATOR_HANDOFF.md`](FASTI_MASTER_INTEGRATOR_HANDOFF.md).
3. Read [`../constitution.md`](../constitution.md).
4. Read [`../definition-of-done.md`](../definition-of-done.md).
5. Read [`../capability-ledger.md`](../capability-ledger.md).
6. Read `AGENTS.md`, `CONTRIBUTING.md`, and `SECURITY.md`.
7. Inspect pull requests #14 and #17.
8. Fetch the live `release` branch and all active branch heads.
9. Inspect exact-head checks, review threads, and changed files.
10. Run repository checks before changing code.

Do not rely on a copied status table after the repository changes. Rebuild the status from GitHub and the checkout.

---

## 2. Fixed product boundary

> **Fasti records. Players play.**

Fasti is a local-first Chronicle and identity system.

Fasti does not:

- decode media;
- stream or transcode media;
- select playback sources;
- become a Kodi, Plex, Stremio, Jellyfin, or Nuvio player;
- treat a provider identifier as permanent Fasti identity;
- require a cloud service for local correctness.

External systems may submit observations. A playback handoff can be considered later as a context action, but it cannot prove that consumption occurred.

If a proposed change makes Fasti a player, imports provider-owned identity into the core, or makes local recording depend on a remote service, stop and re-evaluate the change.

---

## 3. Source-of-truth order

When sources disagree, use this order:

1. Current repository source and migrations.
2. Current active pull request and exact diff.
3. Current exact-head CI, contract, security, QA, and evidence receipts.
4. Approved constitution, engineering plan, and test plan in the repository.
5. Current ADRs, capability registry, error registry, contract sources, and knowledge files.
6. This dated context save.
7. Historical research, prior chats, local task logs, and external planning documents.

Historical reports are useful evidence. They are not permission to restore removed player scope or to claim future work as implemented.

---

## 4. GitHub topology at this save point

### Pull request #14

- Title: `B0-B2: harden the contract foundation and review the local kernel`
- Base: `release`
- Head: `security/b1-evidence-hardening-20260822`
- State: open, draft, unmerged
- Reviewed head when this save was prepared: `4a66b5e96e05f4ce8ac7061f4f4913d4fc675801`
- Purpose: main B0-B2 implementation and review surface

PR #14 states that production remains health-only. B2 exists behind application ports and adapters for review. The production composition root does not activate the B2 public surface.

### Pull request #17

- Title: `docs: add master integrator handoff for external harnesses`
- Base: `release`
- Head: `docs/master-integrator-handoff-20260824`
- State when this save was prepared: open, unmerged
- Purpose: canonical external-harness onboarding and agent discovery

This file is committed to the PR #17 branch. The live branch head will be newer than the SHA recorded before this file was added.

### Relationship between the PRs

- #14 owns implementation review and runtime evidence.
- #17 owns durable onboarding and discoverability.
- #17 must not be used to imply that #14 is complete.
- Documentation-only checks on #17 do not satisfy implementation gates on #14.

---

## 5. Current programme status

| Body | Current status | What is true | What is still required |
| --- | --- | --- | --- |
| B0 | Complete | Repository truth reset and false-success cleanup passed applicable QA | Preserve the boundary and regression checks |
| B1 | Software scope largely complete | Contract spine, fixture path, generated artifacts, SDK checks, and software QA have evidence | Physical Raspberry Pi 5 and J4125 evidence remains open |
| B2 | Implemented for review; not released | Local identity, observations, credentials, receipts, SQLite, evidence storage, and review workflows exist behind ports/adapters | Complete process-crash, restart, durability, public-contract activation, constrained-hardware, and exact-head proof |
| B3 | Not complete | Approved design and test plan exist | Implement correction, export, restore, recovery, equality, and offline proof |
| B4 | Not complete | Product-experience requirements exist | Implement review/correction/recovery UI after entry gates |
| B5 | Not complete | Metadata and projection direction exists | Implement provider-neutral projections without moving history |
| B6 | Not complete | Neutral conformance scope is defined | Build source-neutral fake-client fixtures before a privileged adapter |
| B7 | Not complete | Nuvio sequencing is defined | Implement B7a, then B7b, then B7c behind proven contracts |
| B8 | Not complete | Native/OCI/package and public-release requirements are defined | Split and prove B8a distribution readiness and B8b public release hardening |

Do not mark a body complete because code exists. Completion requires its declared evidence.

---

## 6. Correct execution sequence

The approved dependency sequence is:

```text
B0 -> B1 -> B2 -> B3 -> B4 -> B5 -> B8a
                              |       |
                              v       |
                             B6 ------+
                              |
                              v
                             B7a -> B7b -> B7c
                              |
                              v
                             B8b
```

Interpretation:

- Finish B2 evidence and B3 portability before broad product expansion.
- B4-B8 remain important. Importance does not remove entry gates.
- B8a provides native, OCI, package, offline-install, migration, and hardware proof needed by later integrations.
- B6 proves that the contract works for neutral client shapes.
- B7 adds Nuvio-specific behavior only after neutral conformance.
- B8b owns signing, provenance, updates, final release QA, and public-release controls.

Planning, fixture mining, and isolated contract design for later bodies can proceed in parallel when they do not activate unsupported behavior or create a second semantic core.

---

## 7. Architecture ownership

Expected dependency direction:

```text
fasti-domain
    ^
fasti-application
    ^
fasti-contracts
    ^
adapters: SQLite / filesystem / HTTP / SSE / CLI / UI / package
```

### Domain owns

- stable Fasti identity;
- Chronicle concepts;
- state transitions;
- invariants;
- correction meaning;
- explicit uncertainty;
- typed domain errors.

### Application owns

- capabilities;
- commands and queries;
- authorization decisions;
- transaction orchestration;
- ports;
- idempotency orchestration;
- review and recovery flows.

### Contracts own

- versioned DTOs;
- stable capability IDs;
- serialized fields;
- cross-surface references;
- public problems;
- event identities;
- generated artifact inputs.

### Adapters own

- SQL and migrations;
- filesystem and archive mechanics;
- HTTP, SSE, CLI, and UI translation;
- platform credential storage;
- native, OCI, and package composition.

Domain code must not depend on Axum, rusqlite, Clap, Tokio, Tauri, provider SDKs, UI frameworks, or generated transport models.

Do not create a new abstraction merely because a future provider might need it. Add a boundary when a real capability, hostile fixture, or second implementation requires it.

---

## 8. Core domain invariants

### Identity

- Fasti owns opaque stable identifiers.
- External identifiers are evidence-bearing coordinates.
- A provider identifier is not the canonical key of a Chronicle record.
- Matching is purpose-specific.
- Exact comparison is grain-aware.
- Ambiguity is preserved.
- Title similarity may rank review candidates. It cannot authorize an irreversible merge or history movement.
- Provider switching changes projections and safe lookup routes. It does not re-key history.

### Chronicle

Keep separate:

- original observation;
- occurrence;
- interpretation;
- correction;
- progress;
- saved state;
- current watched/completed state;
- history occurrence;
- rating;
- metadata projection;
- user override.

Original evidence remains readable. A correction appends a new interpretation. It does not rewrite the observation.

### Deletion

The following are not deletion:

- missing provider data;
- an empty page;
- a timeout;
- a 404 from an enrichment provider;
- a cache miss;
- stale metadata;
- incomplete pagination;
- an unavailable mapping bundle.

Deletion or retirement requires an explicit governed operation, tombstone, revocation, or approved reconciliation decision.

### Idempotency

- Operation identity is distinct from media occurrence identity.
- Same client + operation + capability + semantic digest replays one durable receipt.
- Same operation with a changed digest or capability fails without mutation.
- A new occurrence is not a duplicate merely because it concerns the same media.
- Receipt replay remains authorization-bound.
- Credential rotation for the same active client may preserve replay access.
- Revocation is checked before receipt disclosure.

### Profiles and workspaces

- Profiles share provider-neutral Record identity, not personal Chronicle state.
- Each operation binds workspace, authenticated client, profile, capability, and resource scope.
- Cross-profile reads, writes, search results, errors, exports, receipts, and timing behavior must not disclose another profile.

---

## 9. Public contract rule

A public capability is complete only when all required surfaces agree.

Applicable surfaces include:

- domain and application semantics;
- capability registry;
- OpenAPI;
- AsyncAPI;
- JSON Schema;
- JSON-LD and OKF;
- problems and error documentation;
- examples;
- CLI;
- generated TypeScript SDK;
- package smoke;
- UI, when applicable.

Each capability declares `required`, `later body`, or reasoned `N/A` for each surface.

Do not expose an internal B2 DTO or route alone. Activate B2 public DTOs, OpenAPI, AsyncAPI, schemas, JSON-LD, SDK, CLI, examples, scopes, errors, and conformance as one governed change.

Do not update generated contracts merely to describe internal review code that is not a supported public surface.

---

## 10. Security model

### Main trust boundaries

- unauthenticated bootstrap;
- first-client enrollment;
- browser session and CSRF boundary;
- bearer-client authentication;
- workspace/profile authorization;
- credential rotation and revocation;
- raw evidence upload;
- idempotency and receipt replay;
- SQLite writer transaction;
- receipt streaming and cursors;
- export archive creation;
- restore archive parsing and activation;
- future provider and broker egress;
- CI and publishing authority.

### Required properties

- Default daemon listener is loopback-only.
- Remote listening is opt-in and requires authenticated administration plus TLS or a declared trusted proxy.
- Bootstrap secrets are short-lived, one-time, and not returned by an unauthenticated API response.
- Secrets are never logged or placed in URLs.
- Credentials are hashed or encrypted as appropriate.
- Plaintext secret fallback is forbidden.
- Authorization occurs before body/temp-file acceptance and again in the authoritative mutation boundary.
- Request, decoded payload, archive entry, nesting, batch, queue, and concurrency limits are explicit.
- Uploads are streamed, bounded, hashed, synchronized, and promoted safely.
- Content-addressed deduplication verifies existing bytes.
- Receipts bind workspace, profile, client, operation, capability, and digest.
- Cursor reuse across clients or profiles is denied.
- Public errors are stable and do not expose raw exception detail.
- Pull-request workflows remain read-only.
- External actions and OCI inputs remain immutable and reviewed.
- Publishing stays disabled until the B8 release gate and an explicit release action.

### Security review focus for the next harness

1. Bootstrap reuse, expiry, and first-client races.
2. Credential rotation overlap and revocation timing.
3. Cross-profile and cross-workspace isolation.
4. Receipt aliasing and commit-before-reply behavior.
5. Temporary evidence, quota release, and orphan cleanup.
6. SQLite authorization recheck and transaction boundaries.
7. Cursor integrity and receipt-stream authorization.
8. Archive traversal, links, duplicate paths, bombs, and zstd window limits.
9. Secret redaction in logs, errors, metrics, examples, and screenshots.
10. Future DNS, redirect, broker, webhook, and local-discovery boundaries.
11. Workflow permissions, mutable dependencies, generated artifacts, and publishing paths.

A documentation-only handoff must not include exploitable secret material or unredacted sensitive findings.

---

## 11. SQLite and filesystem rules

- SQLite is canonical for the first native product.
- Use one controlled bounded writer.
- Use bounded readers, busy timeouts, keyset pagination, and query limits.
- Require foreign keys.
- Require the reviewed journal and synchronization settings for supported storage.
- Fail startup when required settings cannot be established.
- Provider and network work must not hold the only writer transaction.
- Receipt finalization and governed local mutation must share the required atomic boundary.
- A successful receipt is returned only after required database and filesystem durability work completes.
- WAL is not a promise for arbitrary network filesystems.
- Power-loss claims require the exact filesystem and physical storage profile to honor flush semantics.

Never replace physical durability evidence with hosted-runner evidence.

---

## 12. Offline and distribution rules

Local correctness must not depend on:

- a Fasti-operated cloud service;
- a provider API;
- Redis;
- Celery;
- an MQTT broker;
- an external database;
- Docker-specific paths;
- a reverse proxy;
- another Fasti node.

The same semantic core must support:

- native daemon and CLI;
- source build;
- OCI;
- later packaged Tauri administration shell.

Tauri is not a second business-logic implementation.

Core local actions commit before optional network work. A remote outage preserves the last valid local state. Queue failure must remain visible and recoverable.

---

## 13. Performance rules

Performance is part of correctness.

Preserve:

- one bounded writer;
- bounded readers and pools;
- bounded queues and retries;
- keyset pagination for mutable large sets;
- bounded archive memory;
- streaming evidence and archives;
- no provider call in local transactions;
- no O(N²) reconciliation or unindexed global scan;
- no one-task-per-item fan-out without evidence;
- no claim based on an invented benchmark.

Required physical profiles:

- Raspberry Pi 5 champion;
- calibrated J4125-class x86_64 profile.

Record:

- hardware ownership and fingerprint;
- OS, filesystem, storage, and kernel/runtime details;
- native memory;
- OCI memory;
- latency and throughput as defined by the approved test plan;
- sustained memory and WAL/checkpoint behavior;
- exact commit and build artifact.

The missing physical receipts remain a real milestone blocker.

---

## 14. Accessibility and attention continuity

For every rendered surface, apply:

- WCAG 2.2 AA;
- visible and persistent system state;
- clear grouping and hierarchy;
- predictable navigation and actions;
- keyboard, screen-reader, touch, and remote support where applicable;
- adequate target size and contrast;
- reduced-motion support;
- reliable focus movement and focus return;
- persistent errors and recovery actions;
- saved progress and review position;
- interruption recovery;
- progressive disclosure;
- outcome-first wording;
- no color-only meaning;
- no transient-only critical state;
- no requirement to remember a toast or hidden identifier.

The goal is to make users capable of keeping, understanding, repairing, moving, and recovering their record. Do not make them study the identity architecture during ordinary setup.

No product UI exists in the current B0-B2 production surface. Do not fabricate product screenshots or accessibility results.

---

## 15. QA and evidence workflow

The approved command family is:

```text
cargo xtask test pr
cargo xtask test deep
cargo xtask test milestone --body B0|B1|B2|B3
cargo xtask contract verify
cargo xtask evidence verify <manifest>
```

Also run repository-defined formatting, linting, build, dependency, JavaScript, native, package, OCI, and policy checks.

### Required review sequence

```text
Think -> Plan -> Build -> Review -> Test -> Ship -> Reflect
```

Use the repository equivalents of:

- `/office-hours` for product framing;
- `/autoplan` for strategy, design, engineering, and developer experience;
- `/plan-eng-review` for architecture, migrations, transactions, performance, and tests;
- `/plan-devex-review` and `/devex-review` for real onboarding and time to first success;
- `/review` for exact diff review;
- Codex Security diff/deep review for changed security-sensitive scope;
- `/qa` for runnable journeys and evidence;
- `/design-review` for rendered UI changes;
- `/ship` only after exact-head gates pass;
- `/retro` or a postmortem after each body;
- `/context-save` at every substantial handoff.

Use the review method, not the branding. If a named skill is unavailable, reproduce its required evidence explicitly and state that the skill itself did not run.

### Exact-head rule

A receipt applies only to the commit it records.

Do not merge or claim completion when a required check is:

- pending;
- stale;
- skipped;
- unavailable;
- failed;
- recorded for another commit.

Optional or unavailable tools remain explicit. They are not converted into passing evidence.

---

## 16. Known evidence

### B0

- QA status: complete for applicable B0 scope.
- Health score reached 100/100 after the OCI no-network evidence gap was fixed.
- Product UI was not applicable.

### B1

- Mandatory applicable software QA passed.
- Browser fixture and CLI journeys passed for the recorded exact heads.
- Contract, SDK, repository, package, and performance-sentinel checks have recorded evidence.
- Physical Pi 5 and J4125 native/OCI memory receipts remain missing.

### B2

Substantial implementation and review work exists for:

- node initialization;
- client enrollment;
- credential lifecycle;
- profile-bound authorization;
- SQLite persistence;
- evidence storage;
- provider-neutral records and external identifiers;
- observation acceptance;
- operation idempotency;
- durable receipts;
- receipt streaming;
- review inspect/defer/resume/resolve;
- native, package, and OCI composition.

This does not prove the B2 milestone complete. Re-run exact-head checks and close the remaining process-crash, restart, durability, physical, and public-contract gates.

---

## 17. B3 target

B3 must deliver a complete correction and portability slice.

### Correction

- Append interpretation revisions.
- Preserve original evidence and occurrence.
- Keep a readable correction chain.
- Prevent cycles.
- Preserve stable IDs and authorization.

### Export

- Capture a bounded point-in-time SQLite snapshot.
- Release the live database before archive generation.
- Stream stable ordered output with bounded memory.
- Include counts, references, raw-payload digests, manifest digests, contract versions, and migration versions.
- Exclude active credentials and secrets.
- Recheck authorization during bounded output.
- Remove incomplete output on failure or revocation.

### Restore

- Require a stopped daemon and the shared data-root lock.
- Preflight version, paths, links, duplicates, archive size, decompression ratio, zstd window, disk capacity, migrations, and digests.
- Restore into same-filesystem staging.
- Synchronize files and directories.
- Write and synchronize an activation marker.
- Activate atomically on supported Linux.
- Refuse unsupported platform activation before mutation.
- Recover deterministically from every tested crash point.
- Prove clean equality while excluding node-local authentication bindings.
- Work with network access denied.

B3 is not complete with export-only code or a success message. It requires a tested clean restore.

---

## 18. B4-B8 target map

### B4 — Product experience

Implement the first real chronicle administration and repair interface:

- unresolved observations;
- review queue;
- correction flow;
- recovery state;
- persistent next action;
- interruption continuity;
- accessible responsive layout.

Do not expose raw identity topology by default. Explain it only when the user must make a decision.

### B5 — Metadata projections

Implement replaceable provider-neutral projections:

- source and provenance;
- last-known-good value;
- freshness and failure state;
- language and region;
- safe preference changes;
- separate user overrides;
- impact preview where topology could differ.

Metadata failure must not remove Chronicle truth.

### B6 — Neutral conformance

Use one public Fasti contract with representative client shapes:

- Floppy-like;
- SIMKL-like;
- Nuvio-like;
- Web Scrobbler-like;
- explicit user action;
- local file/import evidence.

No production Nuvio shortcut belongs in B6.

### B7a — Nuvio observations

First Nuvio-specific slice:

- pairing;
- durable Nuvio-side outbox;
- partial-progress and completion observations;
- safe retry and reconnect;
- receipt visibility;
- connection and queue diagnostics;
- playback independent of Fasti.

### B7b — Nuvio state synchronization

Later, after B7a proof:

- progress;
- saved state;
- exact watched state;
- explicit removals;
- snapshot and ordered delta;
- origin and loop prevention;
- reconciliation;
- unresolved-item diagnostics.

Keep progress, saved state, watched state, and history separate.

### B7c — Shared media features

Later:

- Fasti catalog publication;
- Collection source references;
- normalized metadata projections;
- declarative add-on references;
- cache freshness and invalidation;
- user preference and override boundaries.

Do not share databases, service-role credentials, raw account passwords, or executable plugins.

### B8a — Development distribution readiness

Prove:

- native install;
- OCI;
- source build;
- offline install/update path where applicable;
- migrations;
- package smoke;
- rollback;
- physical hardware evidence;
- package and runtime memory.

### B8b — Public release readiness

Prove:

- signed artifacts;
- SBOM;
- provenance and attestations;
- update and rollback path;
- final security review;
- final accessibility and design review;
- final release QA;
- release notes and support guidance;
- explicit publishing authorization.

---

## 19. Related work and lineage

### Fasti

- Main implementation and review: `Scrobble-dev/Fasti#14`
- External-harness onboarding: `Scrobble-dev/Fasti#17`

### Floppy

- Integration programme PR: `dannyvfilms/Floppy#791`
- Nuvio programme issue: `dannyvfilms/Floppy#532`
- Scoped credentials: `dannyvfilms/Floppy#636`
- Read-only catalogs: `dannyvfilms/Floppy#635`
- Identity baseline: `dannyvfilms/Floppy#619`
- Watched-state/history baseline: `dannyvfilms/Floppy#723`
- Durable progress baseline: `dannyvfilms/Floppy#429`
- Third-party API baseline: `dannyvfilms/Floppy#417`

### Nuvio

- Direct integration counterpart: `NuvioMedia/NuvioTV#2935`
- External API/device authorization: `NuvioMedia/NuvioTV#2484`
- Multi-provider scrobbling: `NuvioMedia/NuvioTV#2967`
- Self-host repository: `NuvioMedia/self-host`

### Other context

- Yamtrack: `FuzzyGrim/Yamtrack`
- Ponytail: `DietrichGebert/ponytail`
- gstack: `garrytan/gstack`
- Scrobble.dev: neutral vocabulary, knowledge, schema, and conformance work

These references provide lessons and compatibility context. They do not authorize copying another project's internals or claiming its issue complete.

---

## 20. Historical source corpus

The original research corpus included:

- Fasti product/platform/API blueprint;
- Fasti product constitution, PRD, architecture, and delivery plan;
- identity-first design and structured identity contract work;
- B0-B3 engineering plan and test plan;
- B0 and B1 QA outcomes;
- B2 security and continuation reports;
- Floppy/Nuvio strategy, programme handoff, and master implementation prompt;
- crosswalk architecture and licensing audits;
- gstack engineering, design, executive, developer-experience, and QA task artifacts;
- Ponytail engineering and portability lessons.

Do not require access to those local filenames for ordinary implementation. Their accepted decisions must be represented by current repository documents, code, contracts, fixtures, and tests.

When a historical claim is not represented by current source or an approved repository document, treat it as unverified context.

---

## 21. Immediate continuation plan

A new harness should perform these actions in order:

1. Clone `Scrobble-dev/Fasti` and fetch all branches.
2. Inspect `release`, PR #14 head, PR #17 head, and their merge bases.
3. Read repository guidance and both handoff documents.
4. Check `git status --short --branch` and preserve unrelated work.
5. Inspect PR #14 comments, reviews, review threads, checks, and exact changed files.
6. Run the PR-level repository, Rust, JavaScript, contract, package, and policy checks supported by the environment.
7. Rebuild a current threat model and run an exact diff security review.
8. Produce a current evidence matrix: pass, fail, blocked, unavailable, stale, or not applicable.
9. Close verified B2 software defects directly on the active implementation branch with regressions.
10. Do not weaken the physical evidence gate. Prepare the exact Pi 5/J4125 runbook if hardware is not attached.
11. After B2 software and evidence are coherent, implement B3 correction/export/restore as one governed programme with bounded commits.
12. Run `/qa` or the host-equivalent journey/evidence review after each body.
13. Add `/design-review` only when rendered UI changes.
14. Keep PR comments linked to commits, issues, evidence, rollback, and lessons.
15. Update this context save or create a newer dated successor before handing off.

Do not begin by adding Nuvio code, MQTT, mDNS, metadata federation, or a plugin system.

---

## 22. Required handoff output from the next harness

Before another transfer, publish:

- repository and branch;
- base and head SHAs;
- exact pull request;
- clean/dirty working-tree state;
- completed commits;
- changed-file list;
- tests and commands actually run;
- exact-head workflow links;
- security coverage and findings;
- contract disposition;
- migration disposition;
- offline/package disposition;
- performance evidence or explicit gap;
- accessibility/design disposition;
- unresolved review threads;
- known blockers;
- next three concrete actions;
- rollback instructions;
- postmortem or lessons;
- machine-readable context envelope.

Never say `all green`, `production ready`, or `milestone complete` unless every declared gate is current and supported by evidence.

---

## 23. Machine-readable context envelope

```yaml
schema: fasti-context-save/v1
saved_at: 2026-08-24
repository: Scrobble-dev/Fasti
canonical_branch: release
active_prs:
  - number: 14
    role: implementation-review
    head: security/b1-evidence-hardening-20260822
    state_at_save: open-draft-unmerged
    reviewed_head_at_save: 4a66b5e96e05f4ce8ac7061f4f4913d4fc675801
  - number: 17
    role: external-harness-onboarding
    head: docs/master-integrator-handoff-20260824
    state_at_save: open-unmerged
product_boundary: Fasti records. Players play.
programme:
  B0: complete
  B1: software-complete-hardware-open
  B2: implemented-for-review-evidence-open
  B3: not-complete
  B4: not-complete
  B5: not-complete
  B6: not-complete
  B7: not-complete
  B8: not-complete
blocking_evidence:
  - raspberry-pi-5-fingerprint
  - raspberry-pi-5-native-memory
  - raspberry-pi-5-oci-memory
  - j4125-fingerprint
  - j4125-native-memory
  - j4125-oci-memory
  - b2-process-crash-and-restart
  - b2-supported-power-loss
  - b2-public-contract-activation
  - b3-correction-export-restore-equality
next_actions:
  - refresh-live-repository-and-pr-state
  - run-exact-head-pr-contract-security-and-qa-gates
  - close-b2-verified-defects-and-evidence-gaps
  - implement-b3-correction-export-restore
  - preserve-entry-gates-before-b4-b8
forbidden_shortcuts:
  - restore-player-scope
  - provider-id-as-fasti-identity
  - direct-nuvio-database-coupling
  - infer-delete-from-absence
  - network-required-local-commit
  - plaintext-secret-fallback
  - workflow-write-permission-for-pr-validation
  - stale-evidence-as-current-proof
```

---

## 24. Context-save repository note

The intended external archive repository was:

`ryan-winkler/gstack-artifacts-winks`

The GitHub connector used for this save could not resolve or access that repository. It returned `Not Found`, and the repository did not appear in the connector's installed-repository search.

This Fasti repository file is therefore the durable fallback context save. To mirror it later:

1. grant the GitHub App access to `ryan-winkler/gstack-artifacts-winks`, or confirm the repository's current name;
2. copy this file without changing its content;
3. store it under a dated Fasti project path;
4. record the Fasti source repository, branch, path, blob SHA, and commit SHA;
5. do not treat the mirror as newer authority than the Fasti repository.

---

## 25. Rollback

This context save changes documentation only.

Rollback:

1. revert the commit that added this file;
2. remove any agent link added in the same documentation PR;
3. do not alter runtime code, migrations, contracts, generated artifacts, or evidence receipts.

A rollback of this file does not roll back #14 implementation work.

---

## 26. Postmortem

The project accumulated valuable context across chats, local gstack artifacts, uploaded research, generated reports, and a large implementation branch. The first master handoff reduced this risk, but a new harness also needs a dated operational snapshot that states exact branch topology, current milestone truth, evidence gaps, and the next execution order.

Prevention rules:

- keep one evergreen master handoff;
- add dated context saves at substantial transfer points;
- link both from `AGENTS.md`;
- store accepted decisions in current repository artifacts;
- make claims exact-head and evidence-bound;
- never let a handoff replace tests, contracts, source, or live GitHub state;
- mirror context externally only as a backup, not as a second authority.
