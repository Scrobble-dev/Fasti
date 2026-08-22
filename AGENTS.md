# AGENTS.md

## Skill routing

When the user's request matches an available skill, invoke it. When in doubt, use the review or planning skill before implementation.

Key routing rules:

- Product ideas or brainstorming → `/office-hours`
- Strategy or scope → `/plan-ceo-review`
- Architecture → `/plan-eng-review`
- Design-system or design-plan review → `/design-consultation` or `/plan-design-review`
- Full review pipeline → `/autoplan`
- Bugs or errors → `/investigate`
- QA or behavior testing → `/qa` or `/qa-only`
- Code or diff review → `/review`
- Visual polish → `/design-review`
- Shipping, deployment, or pull requests → `/ship` or `/land-and-deploy`
- Save progress → `/context-save`
- Resume context → `/context-restore`
- Backlog-ready specification or issue → `/spec`

## Read before changing the repository

Read these surfaces before planning or implementation:

1. [`README.md`](README.md)
2. [`docs/constitution.md`](docs/constitution.md)
3. [`docs/definition-of-done.md`](docs/definition-of-done.md)
4. [`ROADMAP.md`](ROADMAP.md)
5. [`docs/capability-ledger.md`](docs/capability-ledger.md)
6. [`contracts/README.md`](contracts/README.md)
7. [`SECURITY.md`](SECURITY.md)

The current branch is a headless engineering baseline. B1 software, QA, and developer-experience checks have prior evidence. B1 remains open because the named physical Raspberry Pi 5 and J4125 receipts do not exist. B2 code may be developed on a draft branch under explicit owner direction, but it must not be merged, described as complete, or used to close B1 until those receipts pass.

## Product boundary

- Fasti records media activity. Players, readers, and services perform playback or consumption.
- Do not add playback, transcoding, decoding, or player claims.
- Production `fastid` exposes only behavior backed by current authorization, persistence, replay, failure, and recovery evidence.
- The B1 HTTP/SSE server is a feature-gated, IPv4-loopback, in-memory conformance fixture. It is not a production API, persistence layer, or release-readiness signal.
- An unavailable capability must stay absent or return its governed typed failure. Never replace missing behavior with optimistic success, an empty artifact, or a silent fallback.

## Meaning and dependency rules

- Each invariant, capability, public type, permission, problem, idempotency rule, retry rule, and offline rule has one authoritative owner.
- `fasti-domain` owns stable identity, evidence, observations, time claims, and invariants.
- `fasti-application` owns use cases, authorization, capability policy, ports, and typed problems.
- Contract and delivery layers project those decisions into public representations. They do not redefine them.
- Storage, HTTP, CLI, SDK, provider, package, and later presentation code depend on the inner rules. Inner code must not import their types or behavior.
- Provider input is evidence. A provider identifier must not become the canonical Fasti identity.
- Reuse an existing type, rule, validator, or generated surface before adding another one. Remove parallel definitions instead of synchronizing duplicates by hand.
- Keep provider adapters modular. Do not add provider-specific policy to shared code or claim compatibility before the applicable gate.

## Contract changes

Start from the authored capability registry and the real handler or use-case owner. Account for every applicable surface:

- production and conformance OpenAPI 3.1;
- AsyncAPI 3.x;
- JSON Schema 2020-12;
- JSON-LD 1.1 and OKF;
- permissions and typed problems;
- CLI and SDK;
- semantic examples and fixtures;
- knowledge and contributor documentation;
- packaging and smoke paths;
- later UI disposition.

Use `N/A` or name the later body when a surface does not apply. Do not edit generated files as the source of truth. Run deterministic generation and prove that checked-in output has no drift.

## Offline, recovery, and distribution

- Core local journeys must work with the network denied, or fail locally with a typed and recoverable result.
- Native daemon and CLI behavior is primary. OCI and future packages must wrap the same governed binaries and semantics.
- Do not require a CDN, cloud account, hosted API, remote asset, telemetry service, or update service for local operation.
- Do not hide a failed native or package build behind a web fallback.
- Bound memory, request bodies, stream replay, queues, archives, retries, and temporary files.
- Preserve explicit state across interruption, restart, duplicate requests, storage pressure, conflicts, and recovery.

## Performance evidence

Keep these visible targets:

- 64 MiB idle;
- 96 MiB normal operation;
- 160 MiB heavy operation;
- 192 MiB absolute full-process ceiling.

A performance claim must identify the exact source tree, artifact, environment, repetitions, measurement method, and named hardware profile. A host estimate, virtual machine, portable sentinel, or software receipt cannot replace the physical Raspberry Pi 5 or J4125 evidence.

Prefer small standard-library or existing-workspace solutions. New dependencies must have a concrete benefit that exceeds their binary, memory, build, security, and maintenance cost.

## Security rules

- Fail closed when identity, authorization, grant state, credential generation, limits, durability, evidence, or source identity is missing or stale.
- Keep secrets out of URLs, command arguments, logs, screenshots, fixtures, evidence bundles, and generated documentation.
- Treat HTTP input, provider data, archives, file paths, JSON, YAML, headers, cursors, manifests, and retained evidence as attacker-controlled.
- Enforce bounds before allocation or persistence.
- Do not add a production route before real authorization, persistence, replay, failure, and recovery behavior exists.
- Keep the production router and conformance router separate at compile time and composition time.
- Pin or verify build inputs where practical. Do not publish from pull-request or routine CI workflows.

## QA, design, and accessibility

Run the applicable clean-checkout gates. The canonical portable gate is:

```bash
cargo xtask test pr
```

Also use the focused Rust, JavaScript, contract, repository-truth, non-publishing, native, and OCI checks named by the changed surface. Add a regression test for each fixed defect.

There is no product UI before B4. For rendered work, read [`brand/DESIGN.md`](brand/DESIGN.md), include screenshots, and test keyboard use, screen readers, responsive layout, 44 px targets, contrast, reduced motion, focus return, error recovery, persistent status, and ADHD/AuDHD state continuity. For headless changes, state that visual evidence is not applicable instead of fabricating screenshots.

Use short sentences, concrete verbs, stable terms, and explicit outcomes. Prefer plain technical English over slogans or unexplained jargon.

## Pull-request traceability

- Preserve another requester's original issue or pull-request body. Add review information in comments or a new pull request.
- Link the accepted Discussion, plan, design review, QA receipt, security review, related issues, related pull requests, and relevant upstream work.
- Use relationship fields when the repository exposes them. Do not invent an issue merely to populate a field.
- Explain user impact, boundary impact, contract disposition, offline behavior, performance evidence, security controls, accessibility disposition, tests, residual risk, and rollback.
- All commits require a DCO sign-off.
