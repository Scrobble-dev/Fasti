# Fasti Agent Memory — 26 August 2026

**Purpose:** durable context for a new engineer or autonomous harness that has no access to prior ChatGPT, Codex, Claude, local worktrees, or gstack state.  
**Captured from:** live `dev` source at `b67c1389702cbed5040b2905a8499b25b5ffcc51`, current and historical Fasti pull requests and review comments, the repository handoff set, the private gstack-artifacts decision/learning corpus, and the operator's Fasti environment note.  
**Freshness rule:** this is a dated consolidation. Current source, current PR diffs, and exact-head evidence always override it.

---

## 1. Fasti in one paragraph

> **Fasti records. Players play.**

Fasti is a local-first media Chronicle, provider-neutral identity authority, reconciliation system, and interoperability service. It accepts observations from players, trackers, imports, automation, local clients, and explicit user actions. It preserves the original evidence, records durable activity, resolves identity when it can, keeps uncertainty when it cannot, and allows later correction without rewriting what originally happened. Fasti does not decode, stream, transcode, select playback sources, or make a provider the owner of the user's record.

The product promise is not “support many integrations.” It is: **keep a trustworthy media record that can survive missing IDs, provider changes, retries, disconnections, corrections, and migration.**

---

## 2. Source-of-truth order

When two sources disagree, use this order:

1. Current repository source and current PR diff.
2. Current exact-head CI, QA, security, performance, and evidence receipts.
3. `AGENTS.md`, `docs/constitution.md`, `docs/definition-of-done.md`, and current architecture documents.
4. Capability, problem, schema, and contract registries.
5. Current PR and issue decisions and review threads.
6. `docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md` and the newest dated handoff.
7. Approved gstack engineering/design/DX/test artifacts.
8. Historical Fasti plans and predecessor-project research.

A dated status table is evidence about its date. It is not permission to claim the same status later.

---

## 3. Semantic constitution

### Stable identity

Fasti owns opaque, stable record IDs. Provider identifiers are typed evidence-bearing coordinates, never Chronicle primary keys.

An external identity can carry namespace, normalized value, grain, relation, direction, range/coverage, evidence, provenance, source revision, lifecycle, and review state. Title similarity may rank review candidates. It cannot authorize an irreversible merge or move history.

### Keep the records separate

Do not collapse these concepts:

- Observation — what evidence arrived.
- Occurrence — the durable media-activity event.
- Interpretation — what record/segment the observation currently means.
- Correction — a later interpretation revision that preserves earlier meaning and evidence.
- Progress — current resume state.
- Watched/completed state — current completion projection.
- Saved/library state — user intent to retain or plan.
- List membership — membership/order in a collection.
- Rating/note — user-owned state.
- Metadata claim — provider-owned projection.
- User override — explicit user-owned presentation choice.

A duplicate delivery is not a rewatch. Progress is not history. A provider refresh does not rewrite user overrides. A metadata projection change does not move Chronicle occurrences.

### Uncertainty is valid data

A zero-ID or unresolved observation is valid and must remain usable. Do not invent a Record merely to make an import look complete. Exact conflicting IDs can be accepted as durable evidence plus a review item; direct identifier attachment to a conflicting Record fails without mutation.

### Deletion is explicit

Never infer deletion from absence, timeout, provider 404, cache miss, empty response, partial page, failed refresh, or expired cursor. Deletion requires a governed removal/tombstone/privacy action or an explicitly approved authoritative reconciliation.

### Idempotency and ordering

Operation identity is scoped to the authenticated client. Same operation + capability + semantic digest replays the original receipt; same operation with changed capability/digest conflicts. A revoked client is denied before receipt disclosure. Server-owned sequence/cursor semantics order transport; client time remains evidence rather than total-order authority.

---

## 4. Architecture that must remain intact

Dependency direction is inward:

```text
Domain
  <- Application
      <- Contracts
          <- Adapters
              <- HTTP / SSE / CLI / SQLite / filesystem / Tauri / web
```

- Domain owns entities, values, invariants, transitions, and domain errors.
- Application owns capabilities, authorization decisions, orchestration, ports, and transaction boundaries.
- Contracts own versioned public DTO projections.
- Store owns SQLite, migrations, bounded writer mechanics, evidence/blob/archive filesystem behavior.
- API/CLI/Tauri/web are adapters. They must not invent parallel business semantics.
- `xtask` owns deterministic contract generation, evidence verification, policy checks, and test orchestration.

Do not create a new abstraction because a future integration might need it. Reuse an existing semantic owner first. Do not use simplification to remove authorization, durability, recovery, accessibility, security, or evidence.

### Contracts move together

For every capability change, audit every applicable surface:

- capability registry;
- Rust DTOs;
- OpenAPI 3.1;
- AsyncAPI 3.x;
- JSON Schema 2020-12;
- JSON-LD/OKF;
- TypeScript SDK;
- CLI;
- permissions/scopes;
- typed problems;
- lifecycle declarations;
- examples and knowledge;
- conformance fixtures.

A surface is `required`, `later body`, or reasoned `N/A`. Generated files are outputs, not independent sources of meaning.

---

## 5. Programme evolution and what the PR history taught us

The PR history is part of the design record because several important decisions were discovered by breaking supposedly green paths.

### B0–B3 foundation

- **#14** established the B0-B2 review foundation but remained draft/unmerged. It proved that production must stay narrower than staged code, separated finalized public problems from staged runtime problems, and reinforced exact-head evidence after every write.
- **#18** superseded the original #17 handoff PR and reconciled two divergent `AGENTS.md`/handoff copies by union rather than destructive replacement. `docs/handoffs/README.md` was added so future agents know precedence.
- **#19** added the 126-case identity matrix under `IDF-###` rather than colliding with the existing product-wide `ID-###` matrix. Do not casually merge these namespaces; the older matrix still carries load-bearing B1 capability traces.
- **#20** moved the active implementation topology to `feature -> dev -> release` and later merged the foundation into `release`.
- **#22** fixed a CodeQL-reported substring assertion by checking the JSON-LD `@id` and `rdfs:Class` exactly. Lesson: a scanner can describe the wrong threat but still reveal a real correctness hole. Fix the assertion; do not suppress the signal.
- **#23** recorded dependency-warning disposition instead of silently dismissing fixture-only advisories.
- **#29** proved migrations preserve populated data, are safe to rerun, and refuse to rewrite a newer schema.
- **#35** completed the internal B3 portability implementation: deterministic bounded archive/export/restore mechanics, crash-safe staging/activation, descriptor-rooted filesystem handling, and explicit indeterminate states. Public activation remains a separate decision.

### Verification is a product surface

- **#31/#36** replaced the unsatisfiable “must own a Raspberry Pi 5 and J4125” B1 gate with a repeatable kernel-enforced cgroup-v2 envelope on x86_64 and aarch64. Current B1 qualification uses 192 MiB, one vCPU, zero swap, exact-source/artifact provenance, 600-second warm-up and 900-second measurement. Pi 5/J4125 are optional later comparison profiles, not B1 milestone blockers.
- **#32** found three false greens: incomplete evidence fixtures accepted as real receipts, a benchmark capture path that could never work, and publish-policy patterns that missed valid publish commands.
- **#33** found four assertions that could not prove what their names claimed: a global rather than per-row authorization assertion, a duplicate check after data had already been put in a `Set`, a mutation sentinel that accepted any thrown exception, and scripts that printed `PASS` before knowing the result.
- **#37** added a traversal case that isolated one specific path guard; broad rejection tests did not prove each guard independently.
- **#39** showed that a differential property test is only as strong as its generator. A tie-break mutation survived because every generated claim had the same value; distinct random values made the mutation observable.
- **#42** showed build caches must never restore write-once evidence state. `target/fasti-evidence` must start clean even when compilation caches are restored.

**Standing rule:** a passing test is not evidence that the test can fail. For security, migration, identity, idempotency, verification, and performance boundaries, use deliberate mutations or negative controls where practical.

### B5 metadata

- **#38** introduced field-level provider claims and user overrides. Resolution is: user override -> fresh preferred-provider/locale claim -> other fresh claim -> last-known-good stale claim -> empty. The clock is caller-supplied; refresh failure never erases the last valid value.
- **#39** made that resolver O(1) extra space without changing behavior.

Provider metadata is a projection, not identity and not user-owned truth.

### B6/B7 source and Nuvio work

- **#27** corrected B6's meaning: source neutrality is proved by behavior, not by pushing five cosmetically different vendor payloads through the same opaque evidence contract. Client archetypes differ in operation-ID derivation, time claims, retries, redelivery, and overlap behavior.
- **#43** added Plex/Jellyfin/Emby/MPRIS converters plus Nuvio application-level observation, outbox, state-delta, and catalog models. Review correctly flagged that process-local buffering could lose queued observations after interruption and that the module risked owning player-session behavior outside Fasti's boundary.
- **#62** later corrected the documentation: B6/B7 are **application/conformance-level only**. Production `fastid` still has no Plex/Jellyfin webhook routes, MPRIS observer, mDNS discovery, Nuvio pairing endpoint, Nuvio transport, durable Nuvio persistence, or Nuvio client implementation.

Do not describe B6/B7 model coverage as shipping Nuvio integration.

The long-term lane remains useful as a roadmap distinction:

```text
B6   neutral source conformance
B7a  one-way Nuvio observations and retry-safe delivery
B7b  progress/saved/watched state synchronization + reconciliation
B7c  catalogs/Collections/metadata/shared-media projections
```

A Local Shared Media Workspace comes only after these primitives are proven. Share references/projections through versioned APIs, not one database and not executable plugin synchronization.

### B4/product shell and networking

- **#44** was closed unmerged. It mixed a large workbench with claims that outran the truthful production surface. Treat it as historical design material only.
- **#53** replaced that direction with a truthful health/interface-quality harness. It explicitly kept catalogue/review/provider/account/persistence capabilities absent until governed activation and added strong Playwright/Axe/keyboard/reflow/motion/theme evidence.
- **#58** merged the truthful local product shell and durable bootstrap, keeping fake media/provider results out of the supported surface.
- **#59** merged governed endpoint/provider policy, platform credential handling, bounded Google Books search through the trusted Tauri host, and a deny-wins outbound network policy. Node connection URLs and provider outbound policy are separate concerns.
- **#60** hardened JavaScript package-root/symlink validation and bounded OCI probes.
- **#63** merged web-workbench support into the canonical local development launcher.
- **#65** exposed the real Review/Reconciliation backend to the desktop through authenticated Tauri commands. It intentionally did not fabricate rich titles/posters/reasoning when the backing metadata capability was not present in that slice.

### Open draft #61 — the main current UI risk

#61 is a design/prototype branch, not merge-ready production UI. It still contains browser-owned/mock-backed state for substantial media, Chronicle, library, token, custom-field, and other workbench behavior. Some earlier mock paths have since been replaced with real host wiring, but do not infer full backend coverage from that progress.

Hardening already applied on the branch includes:

- removing simulated authentication success;
- making unsupported integrations show unavailable rather than connected/ready;
- keeping provider secrets and credential-bearing URLs out of browser storage;
- keyboard-operable context menus;
- 44px action targets, visible focus, toggle semantics, and reduced-motion support;
- real provider-credential wiring through the trusted host;
- merging #65's real review backend into the branch.

A later CodeRabbit conflict-resolution commit temporarily restored old `SAMPLE_PROVIDER_KEYS`/mock-shaped credential wiring. Commit `7995c2d` restored the real path. **Future agents must semantically diff any bot-generated merge/conflict resolution. Automated conflict resolution can reintroduce exactly the mock/security behavior the branch exists to remove.**

The rule for #61 is simple: a workbench action must either call a real governed application/host capability or render an honest empty/unavailable/read-only state. Do not build a parallel browser-local tracker.

### Open draft #67 — Android

#67 adds the Tauri Android scaffold and keyring bridge, plus non-Linux compile fixes and network/build hardening. Linux-hosted Rust/desktop checks are useful but are not Android evidence.

No Android support claim is valid until the exact head has:

- `cargo tauri android build` or equivalent governed build evidence;
- an APK/AAB artifact as appropriate;
- emulator or physical-device startup;
- JNI keyring round-trip;
- offline behavior;
- package startup/memory evidence.

CodeRabbit's latest pass had no actionable comments, but its configuration ignores much of `gen/**`, including the Android scaffold. A bot's clean result is therefore not proof that the Android-specific files were reviewed.

---

## 6. Security and network doctrine

- Default uninitialized/durable local surfaces are loopback-first and fail closed.
- Workspace, profile, client, credential, grant, capability, and resource scope are separate authorization dimensions.
- Client/Credential/ProfileGrant lifecycles stay separate.
- Provider credentials stay in OS/platform credential storage or the governed headless store; never in browser `localStorage`, URLs, logs, screenshots, fixtures, or normal exports.
- App-managed provider credentials are scoped to the physical Fasti data-root identity, not merely a path string.
- Outbound provider access uses one governed policy: declarations are maximum grants, operator allow lists narrow them, denies win, DNS/IP answers are validated, unsafe ranges rejected, redirects/proxies disabled where required, bodies/time/retries bounded, secrets loaded only after destination authorization.
- A local Fasti service URL and provider egress allow list are separate settings.
- Future mDNS/DNS-SD is discovery only, never trust. Future MQTT is a bounded connection adapter, never the synchronization or database-replication model.
- Sensitive findings use the private security path. Public PR comments should describe safe remediation without publishing exploit detail that increases risk.

---

## 7. Offline, packaging, and performance doctrine

Local correctness cannot require cloud, provider APIs, Redis, Celery, MQTT, another database, or Docker.

The same semantic core must work through native daemon/CLI, OCI, and later packaged applications. Docker/Podman are delivery environments, not domain owners. Native offline behavior must be tested independently; container network denial does not prove native network independence.

Current memory budgets:

- 64 MiB idle;
- 96 MiB normal;
- 160 MiB heavy target;
- 192 MiB absolute process/cgroup ceiling.

Performance evidence must be exact-source, repeatable, and bound to the measured artifact. Avoid O(N²), full-library materialization for one page, unbounded queues/pages/archive memory, ambient polling, provider I/O in local write transactions, or task-per-item explosions without measurement.

---

## 8. UX, accessibility, and copy

Read `brand/DESIGN.md` before UI changes. Current UI governance is Tabler-first and uses the Fasti design language rather than generic SaaS components.

Every rendered change must be reviewed against:

- AskTog interaction principles;
- Gestalt grouping;
- all ten Nielsen heuristics;
- relevant IxDF interaction/cognitive-load guidance;
- WCAG 2.2 AA;
- EN 301 549 where applicable;
- keyboard, screen reader, touch, and remote interaction where relevant;
- visible focus and focus restoration;
- 44px targets;
- reduced motion;
- reflow and narrow viewports;
- persistent errors/next actions;
- saved position and interruption continuity;
- no color-only meaning;
- progressive disclosure and low memory burden.

Use concise active copy. Critical state is never toast-only. `Resolve later` is a valid safe outcome. Do not use gamification guilt.

UI work requires `/design-review` and `/qa`; headless work states visual evidence as `N/A` rather than fabricating screenshots.

---

## 9. Local operator context

The operator's private Vault records the canonical local environment:

- stable path: `~/code/fasti`, normally on `dev`;
- `fasti` is a shell wrapper around repository-owned `scripts/dev.sh`;
- `fasti --status`, `--open`, `--stop`, `--podman`, `--docker`, and `--self-test` are the normal shortcuts;
- `fastid` prefers `127.0.0.1:8420` and can use an OS-selected fallback;
- the web workbench uses `127.0.0.1:5173` when the current worktree includes `apps/web`;
- the native environment runs inside the `ai-vibe` distrobox with host networking, so it will not appear as a Podman-published container.

Always inspect the current worktree list before assuming an old named UI worktree still owns current code.

---

## 10. Review method expected on every substantial change

Think -> Plan -> Build -> Review -> Test -> Ship -> Reflect.

Use the project's gstack/Ponytail-derived workflow as behavior, not branding:

1. inspect live source, existing ownership, open/closed PRs, related issues, and upstream evidence;
2. identify blast radius and failure modes before implementation;
3. prefer the platform/runtime and existing semantic owner over new dependencies or parallel abstractions;
4. implement the smallest complete capability slice;
5. run diff/security review and validate findings rather than copying scanner language blindly;
6. run exact relevant tests plus mutation/negative controls for safety-critical invariants;
7. run `cargo xtask contract verify` when contracts can be affected;
8. run `cargo xtask test pr` (with documented Linux prerequisites where needed);
9. run `/qa`; UI also gets `/design-review` and accessibility evidence;
10. document user, contract, offline, security, performance, accessibility, rollback, relationships, and postmortem impact;
11. never reuse a receipt after the commit changes.

Review bots are advisory evidence. Their path filters, auto-pause, skipped files, similar-change suppression, and merge-conflict automation must be inspected before treating a clean review as coverage.

---

## 11. High-signal lessons future agents must retain

1. **Truth before feature count.** An honest unavailable state is better than a successful mock.
2. **Current source outranks handoffs.** Preserve old handoffs as history; add a newer dated handoff instead of rewriting history.
3. **Tests must prove they can fail.** Mutation testing repeatedly found false confidence in verifiers and generators.
4. **Evidence is immutable state, not cache.** Never let build caches restore write-once evidence directories.
5. **Generated/bot changes need semantic review.** They can reintroduce mocks, weaken security, or hide conflict choices while compiling cleanly.
6. **Bot “clean” is not coverage.** Check ignored/skipped paths, especially generated Android/platform files.
7. **No primary provider ID.** Fasti identity survives providers; external IDs are assertions.
8. **No delete-by-absence.** Provider and transport failures cannot destroy local truth.
9. **No browser-owned secrets or domain truth.** Trusted host/application capabilities own those paths.
10. **Offline is normal.** Prove native and packaged behavior separately from container behavior.
11. **Performance is correctness.** Use kernel-enforced envelopes and retained exact artifacts, not aspirational device names or measurements that cannot be independently reproduced.
12. **Nuvio application models are not a shipped Nuvio integration.** Pairing/transport/persistence/client work still needs its own production evidence.
13. **Metadata is field-level provenance.** A user override wins; stale last-known-good is preferable to false deletion.
14. **Keep UI state proportional to backend truth.** Rich design can run ahead as a prototype, but it does not become supported until the owning capabilities exist.
15. **One semantic core.** HTTP, CLI, Tauri, web, SDK, events, MCP, and future connections should converge on the same application capabilities.

---

## 12. First steps for the next session

Before changing code:

```text
1. Read AGENTS.md.
2. Read docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md.
3. Read this file.
4. Read docs/handoffs/README.md for precedence.
5. Fetch dev and record its exact SHA.
6. List current open PRs and their exact heads.
7. Read current PR comments/review threads, including bot skipped-path information.
8. Diff the target branch against current dev, not release, for normal feature work.
9. Identify which capability owns the requested behavior and whether it is production-wired, application-only, or prototype-only.
10. Run the smallest relevant baseline before editing.
```

At this capture, the two open drafts needing continued caution are **#61 (workbench prototype convergence)** and **#67 (Android package/device qualification)**. Re-verify that before relying on it.

---

## 13. External context to retain, not copy blindly

Fasti has inherited useful lessons from Floppy PR #791, Nuvio, Scrobble.dev, crosswalk work, and other trackers:

- exact external identity and provider provenance;
- durable idempotency receipts;
- snapshot + ordered-delta recovery;
- client-bound cursors and origin;
- current watched state separate from history;
- explicit tombstones and no delete-by-absence;
- field-level metadata projections;
- declarative add-ons separate from executable plugins;
- local shared-media references instead of direct database coupling.

These are lessons, not permission to copy Django/Celery/Nuvio internals or create vendor-specific scopes.

Scrobble.dev remains the neutral vocabulary/schema/conformance commons. Fasti is one implementation. The durable ecosystem rule is:

> **Standardize semantics, not vendors.**
