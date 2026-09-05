# AGENTS.md

## Start here

Before planning or implementation, read these in order:

1. [`docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md`](docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md)
2. [`docs/handoffs/FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md`](docs/handoffs/FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md)
3. [`README.md`](README.md)
4. [`docs/dev-loop.md`](docs/dev-loop.md)
5. [`docs/constitution.md`](docs/constitution.md)
6. [`docs/definition-of-done.md`](docs/definition-of-done.md)
7. [`ROADMAP.md`](ROADMAP.md)
8. [`docs/capability-ledger.md`](docs/capability-ledger.md)
9. [`contracts/README.md`](contracts/README.md)
10. [`SECURITY.md`](SECURITY.md)

The master handoff defines the durable product boundary, source-of-truth order, architecture and security invariants, programme model, required evidence, and the first 48-hour onboarding sequence.

The dated context save records the active pull-request topology, current B0-B8 disposition, known evidence gaps, exact continuation order, and required handoff output for a harness with no access to prior chat or local gstack state.

Both are maps. Neither replaces the detailed engineering, test, security, and design documents.

Then inspect current repository state, active pull requests, exact-head checks, and controlling plans. A dated handoff never overrides newer source or evidence.

See [`docs/handoffs/README.md`](docs/handoffs/README.md) for handoff precedence.

## Product boundary

Fasti records. Players play.

Do not add playback, transcoding, decoding, stream selection, or player claims.

Provider identifiers are evidence, not canonical identity.

## Architecture rules

- Domain rules own meaning.
- Application services own capabilities and authorization.
- Contracts project the same meaning into APIs, events, schemas, SDKs, and docs.
- Adapters must not redefine business rules.
- Reuse existing ownership before creating new abstractions.
- Before adding a framework, service, database, queue, or authentication library, read [`docs/architecture/adr-0005-framework-and-auth-adoption.md`](docs/architecture/adr-0005-framework-and-auth-adoption.md) and the approved [TrailBase authentication programme](docs/plans/trailbase-authentication-remediation.md). ADR-0005 records the earlier evaluation; its optional TrailBase and django-allauth conclusions are superseded. TrailBase is the selected separate human-account service. Loco remains a developer-experience reference, not a Fasti runtime dependency. TrailBase cannot own Fasti state, sessions, scopes, grants, profiles, or object authorization. A later change requires an updated ADR, migration and rollback proof, and the applicable repository guard change in the same pull request.
- TrailBase runtime work must use [`third_party/trailbase/release.json`](third_party/trailbase/release.json), [`scripts/dev.sh`](scripts/dev.sh), and the [TrailBase runbook](docs/operations/trailbase.md). Keep the exact release, licence text, separate process, owner-only depot, loopback/private admin boundary, and one launcher. Do not use a floating tag, direct TrailBase database access, or TrailBase Record APIs for Fasti data. Run `cargo xtask test milestone --body B` on a prepared machine after any change to this boundary.
- Keep provider integrations modular.
- Keep wire provider IDs separate from external identifier namespaces. Reuse
  `fasti_application::provider_identity_mapping` for Google Books and TMDB
  coordinates in every adapter; do not add adapter-local TMDB identifier fallbacks.
- Route governed outbound access through application policy. Provider declarations are maximum grants; operator allow lists only narrow them, and denies win.

## Contract changes

Any capability change must consider:

- OpenAPI 3.1;
- AsyncAPI 3.x;
- JSON Schema 2020-12;
- JSON-LD;
- SDK;
- CLI;
- permissions;
- typed problems;
- examples;
- documentation.

Generated files are outputs, not sources of truth.

## Offline, security, and performance

- Local operation must work without external services.
- Fail closed on missing authorization, stale state, missing evidence, or unsafe input.
- Keep secrets out of logs, URLs, fixtures, and documentation.
- Mount durable local routes only for direct loopback access or an explicitly declared loopback-only port forward inside a detected container boundary, with an explicit `FASTI_DATA_ROOT`. Keep bootstrap routes on those local exposures. Require `FASTI_REMOTE_TRUSTED_PROXY=true` plus an absolute HTTPS `FASTI_PUBLIC_URL` before mounting the authenticated non-loopback router. Never infer a data directory.
- Resolve provider hosts once, reject every unsafe answer, disable redirects and system proxies, and pin the authorized addresses before loading a credential.
- Treat `TMDB_API_READ_ACCESS_TOKEN` as a TMDB API Read Access Token. Send it only in a sensitive `Authorization: Bearer` header; never fall back to the v3 `api_key` URL parameter.
- Keep provider credentials in environment variables or the platform credential store. Never return them to Svelte, browser storage, logs, URLs, screenshots, fixtures, or proof bundles.
- Mount C1 human-account and browser-session routes only when the durable listener is requested and bound as exactly `127.0.0.1:8420` with no fallback. Keep the route set mounted when TrailBase is unavailable so the projection can report the safe state. Require a verified installation receipt and persisted active activation before exchange or new session issuance. Every alternate-loopback, generic, integration, wildcard or container forwarding, and remote router must omit C1. Do not add or enable a development browser account as a substitute.
- Scope app-managed provider credentials to the physical Fasti data root. They are node-wide across profiles until a real authenticated profile-private provider capability exists. Never fall back to an unscoped account.
- Derive app-managed credential accounts from `SqliteKernel::data_root_identity()`, not from a configured path. Bind the identity to the opened root descriptor and its persisted random lock nonce. Renaming an opened root must keep its account; replacing that path with another root must select another account.
- Keep node connection settings separate from provider outbound policy. A provider allow list must not block an operator-selected `.internal` Fasti service URL.
- Bound memory, files, requests, archives, and retries.
- Keep data-root lock release with the existing `LockedDataRoot` owner and its failed-acquisition cleanup. Preserve `KernelInner.data_root` as the last dropped field. Inherited descriptors must not prolong a completed owner; a live Rust guard/kernel must not transfer or run its destructor across fork. Do not replace this boundary with acquisition retries or test serialization.
- Validate recovery and interruption paths.

Performance targets remain:

- 64 MiB idle;
- 96 MiB normal operation;
- 160 MiB heavy operation;
- 192 MiB absolute ceiling.

Qualifying B1 performance receipts must bind the exact release daemon and declare the same CI workflow run attempt for one exact `dev` push on x86_64 and aarch64. Each architecture must complete the governed 600-second warm-up and 900-second route-less idle measurement. The verifier recomputes memory, CPU, artifact sizes, architecture, and the kernel-applied 192 MiB, one-vCPU, zero-swap controls. Pi 5 and J4125 profiles are optional comparison specifications, not milestone gates.

## QA

Run the canonical gate:

```bash
cargo xtask test pr
```

On Linuxbrew hosts, use the system `pkg-config` so the Tauri gate can find the
APT-installed GTK and WebKit metadata:

```bash
PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr
```

The Tauri performance gate inside that command needs prerequisites. Install them once:

```bash
sudo apt-get install --yes --no-install-recommends build-essential curl file libayatana-appindicator3-dev librsvg2-dev libssl-dev libwebkit2gtk-4.1-dev libxdo-dev wget
```

Then populate both locked dependency graphs before the gate runs offline:

```bash
cargo fetch --locked
cargo fetch --manifest-path benchmarks/b1/tauri-shell/src-tauri/Cargo.toml --locked
```

Without the second fetch the gate fails with a misleading `swift-rs` version-resolution error.

Listener, public URL, loopback alias, container port, and certificate-trust
ownership is documented in [`docs/network-configuration.md`](docs/network-configuration.md).
Do not merge the bind address, client URL, and public reverse-proxy URL into one
setting.

Android sandbox selection, data-root locking, and descriptor-rooted kernel
directories are implemented in source but are not Android build- or
device-verified. The Gradle project scaffold
(`apps/desktop/src-tauri/gen/android/`) and the `io.crates.keyring.Keyring`
JNI bridge that Android's keyring store calls into are checked in, but
`cargo tauri android build` has never been run against them — this host has
no Android SDK, NDK, or JDK. Building for Android requires Java, the Android
SDK/NDK, and `@tauri-apps/cli` (`npx @tauri-apps/cli@2.11.4 android build`
from `apps/desktop/src-tauri`, after `pnpm --filter @fasti/web build`). Do not
claim Android package support until the NDK build and device evidence pass.
B3 restore and startup recovery remain Linux-only.

Run the Linux Desktop review host with
`FASTI_DATA_ROOT=/path/to/private/data ./scripts/dev.sh --desktop`. This path
builds the static Workbench and starts only the Tauri host with its embedded
kernel. It must not start `fastid`, Vite, or a browser credential fallback.

Also run focused checks for changed surfaces. Add regression tests for fixed defects.

- For the isolated [C3 signing qualification](qualification/access-c3-signing/README.md), follow its native-override isolation steps and run `cargo +1.97.1 test --offline --locked --manifest-path qualification/access-c3-signing/Cargo.toml`, plus the documented release, formatting, Clippy and advisory checks. Root workspace tests do not include this package. Qualification does not approve a production crypto profile or recovery capability.
- The isolated [C3 KDF runner](qualification/access-c3-kdf/README.md) has 13 unit tests and no doctest target. Its debug/release, formatting, Clippy and advisory checks join the existing qualification workflow; they do not run the native measurement. Run that measurement separately only with the documented kernel controls and a coordinated resource slot. Do not infer production, whole-application or other-hardware qualification from its recorded local fixture result.
- For the isolated [C3 framing qualification](qualification/access-c3-framing/README.md), follow its target and native-override isolation steps, then run `cargo +1.97.1 test --offline --locked --manifest-path qualification/access-c3-framing/Cargo.toml -j 2 -- --test-threads=1` and the documented release, formatting, Clippy and advisory checks. Each full debug/release suite must pass 20 unit tests and two compile-fail doctests. Root workspace tests exclude this package; it does not activate production encryption, joint backups or recovery.

UI changes require design review. Headless changes should state when visual evidence is not applicable.

The local browser QA harness may expose only generated capabilities that are
active in the capability ledger. Run its Chrome, Axe, keyboard, reflow, motion,
and theme checks with:

```bash
pnpm test:ui
```

For ordinary cases, use `pnpm test:ui --grep-invert @performance`.
Run Lighthouse separately with `pnpm test:ui --grep @performance --workers=1 --retries=0 --output=test-results-performance`.
The latter requires Node >=22.19 and an installed stable Chrome with the
qualified interaction trace event; see the [Access performance guide](docs/plans/fasti-access-parallel-regressions.md#persistent-performance-sentinel-gate).
CI refreshes Chrome only on its ephemeral runner, not on a contributor's host.

The runner uses `127.0.0.1:4173` and a bounded health stub on
`127.0.0.1:18422`, and it refuses to reuse occupied ports. It does not take over
the documented local development URL at `127.0.0.1:5173`. A QA harness is
evidence tooling. It does not activate a product milestone, runtime listener
setting, or desktop surface.

## Design system & UI Component Standards

Read [`brand/DESIGN.md`](brand/DESIGN.md) before making any visual or UI decision.

### UI Invariants & Requirements:

<!-- FASTI_TABLER_POLICY_START -->

- **Tabler-First Policy**: Always use upstream Tabler (`@tabler/core` and `@tabler/icons`) layout, grid, typography, cards, tables, forms, modals, and badge classes first.
- **Component Decision Hierarchy**:
  1. Tabler Core Component (direct usage)
  2. Tabler Pattern Composition
  3. Fasti Token-Skinned Tabler Element (`brand/tokens/tokens.json`)
  4. Custom Svelte Component (STRICT EXCEPTION: only if Tabler has zero equivalent; requires explicit documented architectural rationale).
- **Permanent gate**: `pnpm lint:ui` must pass. It rejects non-Tabler icon systems, unapproved raw SVG, removal of the Tabler Core stylesheet, and removal of these managed policy markers. Do not weaken its allowlist to land UI work.
- **Workbench shell contract**: Use one `.page` root with an adjacent `.navbar.navbar-vertical.navbar-expand-lg.offcanvas-lg` and `.page-wrapper`. Below `lg`, navigation is closed by default and the page wrapper owns the full viewport. Settings uses `.container-fluid`, list-group links on wide screens, and a labelled `.form-select` on constrained screens. Theme settings uses the Tabler `.offcanvas` and writes every exposed choice to the documented Tabler and Fasti data attributes. Finite Fasti component radii must consume `--tblr-border-radius-scale`; focus indicators must use `--fasti-focus`, never a selectable accent. `pnpm lint:ui` enforces these rules and rejects the prior fixed mobile rail, compensating page margin, centered Settings ceiling, and bespoke drawer fallback.

<!-- FASTI_TABLER_POLICY_END -->

- <!-- FASTI_CHESTERTON_POLICY_START --> **Chesterton's fence**: Before deleting, hiding, or replacing an existing UI control, trace its callers, history, tests, screenshots, and intended capability. Mature fake behavior behind a governed host capability. If the capability is not ready, keep the affordance visible with a precise unavailable state and add an owned TODO. Removal requires explicit user approval plus a documented replacement or migration.

<!-- FASTI_CHESTERTON_POLICY_END -->

- <!-- FASTI_AUTH_BOUNDARY_START --> **Authentication boundary**: Follow [`docs/architecture/authentication.md`](docs/architecture/authentication.md). TrailBase human credentials, dormant and active Fasti browser sessions, scoped API client credentials, packaged-host administrator credentials, passkeys, device authorization, and OpenID Connect tokens are distinct credential models. Never collapse them into one token or simulate a backend flow in Svelte.

<!-- FASTI_AUTH_BOUNDARY_END -->

- **Impeccable Craft Floor**: Surface mode must be `Operate` (workbench, triage, settings) or `Read` (annal, chronicle, markdown docs). Zero layout shifts (`CLS = 0`), no purple/violet AI gradients, no generic SaaS card walls, no continuous decorative animations, and strict 44px min touch targets.
- **Interaction & Usability Standards**: All UI flows must be audited against:
  - AskTog interaction principles (anticipation, Fitts's law, latency reduction, user work protection, state continuity)
  - Gestalt grouping principles (proximity, similarity, common region, continuity, closure, figure/ground)
  - All 10 Nielsen Norman usability heuristics
  - IxDF research topics (cognitive load reduction, progressive disclosure, dark mode halation prevention, motor precision)
- **Accessibility & Regulatory Conformance**:
  - WCAG 2.2 Level AA is the required target (3px high-contrast focus rings with 2px offset, >= 4.5:1 text contrast / 7.0:1 on paper cards, 44px hitboxes, non-obscured focus). Do not claim conformance without exact-head automated and manual evidence.
  - EN 301 549 Clauses 9 (Web), 10 (Non-Web Docs), 11 (Desktop/Software Assistive Tech Interoperability), and 12 (Documentation) require a clause-to-evidence record. Do not claim conformance from Axe or browser fixtures alone.

In QA mode, flag any code that does not match `brand/DESIGN.md` or violates the Tabler-first ladder.

## Skill routing

When the user's request matches an available skill, invoke it via the Skill tool. When in doubt, invoke the skill.

Key routing rules:

- Product ideas/brainstorming → invoke /office-hours
- Strategy/scope → invoke /plan-ceo-review
- Architecture → invoke /plan-eng-review
- Design system/plan review → invoke /design-consultation or /plan-design-review
- Full review pipeline → invoke /autoplan
- Bugs/errors → invoke /investigate
- QA/testing site behavior → invoke /qa or /qa-only
- Code review/diff check → invoke /review
- Visual polish → invoke /design-review or /impeccable polish
- Ship/deploy/PR → invoke /ship or /land-and-deploy
- Save progress → invoke /context-save
- Resume context → invoke /context-restore
- Author a backlog-ready spec/issue → invoke /spec

## Review and handoff

The [C2 foundation delivery gate](docs/plans/fasti-access-c2-foundation.md)
is a pure domain/application dependency slice, not C2 runtime completion.
It adds no PAT authentication route, administration port, migration or archive
version. Preserve M4's shared integration ownership and reserved search scope;
require its exact merged handoff before C2 activation work. Run the unchanged
canonical PR gate for the shared typed-ID and secret-erasure changes.

Document:

- user impact;
- contract impact;
- offline impact;
- security impact;
- performance evidence;
- design & Tabler-first component compliance;
- accessibility evidence (WCAG 2.2 AA & EN 301 549 audit + Axe-core zero-violation report);
- tests;
- rollback.

For C1, `cargo xtask test milestone --body C1` writes the in-scope delivery
receipt. It proves the prepared TrailBase service, generated contracts and SDK,
trusted-host source tests, and browser A+C fixture on
the exact source tree. It does not prove packaged Tauri authentication.
`C1-TAURI-AUTH` owns that deferred WebView, cross-platform, and packaged
assistive-technology evidence. Do not weaken Secure cookies or claim packaged
desktop authentication before that follow-up passes. Exact-head review, CI,
merge, and merged-tree evidence remain required for C1 delivery.

## Quality and Security Invariants

- **Single Integration Branch**: All active development and pull requests target `dev`. `release` is reserved strictly for release candidate stabilization.
- **Zero Deployment Blockers**: External SaaS/AI tools (CodeRabbit, Codacy, Codecov, Scorecard) provide advisory feedback and must never block emergency hotfixes, local builds, or CI deployments.
- **Strict Bounded Performance**: The daemon (`fastid`) must strictly observe memory ceilings: 64 MiB idle, 96 MiB normal, 192 MiB process tree ceiling.
- **Zero Runtime Telemetry**: The daemon and client libraries must never include phone-home code, tracking SDKs, or external analytics.
- **YouTrack Workflow**: Fasti uses YouTrack (`fasti.youtrack.cloud`) for sprint execution, milestone tracking (B0–B8), and hardware receipts. Prefix commit messages and PR titles with YouTrack IDs (`FASTI-###`).
- **No-Publish Guardrail**: Workflows in `.github/workflows/` must have strictly read-only permissions before Milestone B8 release readiness.

## GBrain Search Guidance (configured by /sync-gbrain)
<!-- gstack-gbrain-search-guidance:start -->

GBrain is set up and synced on this machine. The agent should prefer gbrain
over Grep when the question is semantic or when you don't know the exact
identifier yet.

**This worktree is pinned to a worktree-scoped code source** via the
`.gbrain-source` file in the repo root (kubectl-style context).
`gbrain code-def`, `code-refs`, `code-callers`, `code-callees`, `search`, and
`query` from anywhere under this worktree route to that source by default —
no `--source` flag needed (gbrain >= 0.41.38.0; on older gbrain the call-graph
commands need `--source "$(cat .gbrain-source)"`). Conductor sibling worktrees
of the same repo each have their own pin and their own indexed pages, so
semantic results match the code on disk here.

Call-graph queries (`code-callers`/`code-callees`) also need the graph to be
built first — run `/sync-gbrain --dream` (or `--full`) if they return
`count: 0`. This only works if this source's gbrain schema pack extracts code
symbols; on a non-code-aware pack `--dream` completes but the graph stays empty
and reports a WARN. `code-def`/`code-refs` need the same extraction.

Two indexed corpora available via the `gbrain` CLI:
- This worktree's code (auto-pinned via `.gbrain-source`).
- `~/.gstack/` curated memory (registered as `gstack-brain-<user>` source via
  the existing federation pipeline).

Prefer gbrain when:
- "Where is X handled?" / semantic intent, no exact string yet:
    `gbrain search "<terms>"` or `gbrain query "<question>"`
- "Where is symbol Y defined?" / symbol-based code questions:
    `gbrain code-def <symbol>` or `gbrain code-refs <symbol>`
- "What calls Y?" / "What does Y depend on?":
    `gbrain code-callers <symbol>` / `gbrain code-callees <symbol>`
- "What did we decide last time?" / past plans, retros, learnings:
    `gbrain search "<terms>" --source gstack-brain-<user>`

Grep is still right for known exact strings, regex, multiline patterns, and
file globs. Run `/sync-gbrain` after meaningful code changes; for ongoing
auto-sync across all worktrees, run `gbrain autopilot --install` once per
machine — gbrain's daemon handles incremental refresh on a schedule.

Safety: don't run `/sync-gbrain` while `gbrain autopilot` is active — the
orchestrator refuses destructive source ops when it detects a running autopilot
to avoid racing it (#1734). Prefer registering user repos with
`gbrain sources add --path <dir>` (no `--url`): URL-managed sources can
auto-reclone, and the sync code walk for them requires an explicit
`--allow-reclone` opt-in.

<!-- gstack-gbrain-search-guidance:end -->
