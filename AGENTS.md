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
- Keep provider integrations modular.
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
- Mount durable local routes only for direct loopback access or an explicitly declared loopback-only container port forward, with an explicit `FASTI_DATA_ROOT`. Keep bootstrap routes on those local exposures. Require `FASTI_REMOTE_TRUSTED_PROXY=true` plus an absolute HTTPS `FASTI_PUBLIC_URL` before mounting the authenticated non-loopback router. Never infer a data directory.
- Resolve provider hosts once, reject every unsafe answer, disable redirects and system proxies, and pin the authorized addresses before loading a credential.
- Treat `TMDB_API_READ_ACCESS_TOKEN` as a TMDB API Read Access Token. Send it only in a sensitive `Authorization: Bearer` header; never fall back to the v3 `api_key` URL parameter.
- Keep provider credentials in environment variables or the platform credential store. Never return them to Svelte, browser storage, logs, URLs, screenshots, fixtures, or proof bundles.
- Scope app-managed provider credentials to the physical Fasti data root. They are node-wide across profiles until a real authenticated profile-private provider capability exists. Never fall back to an unscoped account.
- Derive app-managed credential accounts from `SqliteKernel::data_root_identity()`, not from a configured path. Bind the identity to the opened root descriptor and its persisted random lock nonce. Renaming an opened root must keep its account; replacing that path with another root must select another account.
- Keep node connection settings separate from provider outbound policy. A provider allow list must not block an operator-selected `.internal` Fasti service URL.
- Bound memory, files, requests, archives, and retries.
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

Also run focused checks for changed surfaces. Add regression tests for fixed defects.

UI changes require design review. Headless changes should state when visual evidence is not applicable.

The local browser QA harness may expose only generated capabilities that are
active in the capability ledger. Run its Chrome, Axe, keyboard, reflow, motion,
and theme checks with:

```bash
pnpm test:ui
```

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

- <!-- FASTI_AUTH_BOUNDARY_START --> **Authentication boundary**: Follow [`docs/architecture/authentication.md`](docs/architecture/authentication.md). Browser sessions, scoped API client credentials, packaged-host administrator credentials, passkeys, passwords, device authorization, and OIDC tokens are distinct credential models. Never collapse them into one token or simulate a backend flow in Svelte.

<!-- FASTI_AUTH_BOUNDARY_END -->

- **Impeccable Craft Floor**: Surface mode must be `Operate` (workbench, triage, settings) or `Read` (annal, chronicle, markdown docs). Zero layout shifts (`CLS = 0`), no purple/violet AI gradients, no generic SaaS card walls, no continuous decorative animations, and strict 44px min touch targets.
- **Interaction & Usability Standards**: All UI flows must be audited against:
  - AskTog interaction principles (anticipation, Fitts's law, latency reduction, user work protection, state continuity)
  - Gestalt grouping principles (proximity, similarity, common region, continuity, closure, figure/ground)
  - All 10 Nielsen Norman usability heuristics
  - IxDF research topics (cognitive load reduction, progressive disclosure, dark mode halation prevention, motor precision)
- **Accessibility & Regulatory Conformance**:
  - WCAG 2.2 Level AA full compliance (3px high-contrast focus rings with 2px offset, >= 4.5:1 text contrast / 7.0:1 on paper cards, 44px hitboxes, non-obscured focus).
  - EN 301 549 compliance across Clause 9 (Web), Clause 10 (Non-Web Docs), Clause 11 (Desktop/Software Assistive Tech Interoperability), and Clause 12 (Documentation).

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

## Quality and Security Invariants

- **Single Integration Branch**: All active development and pull requests target `dev`. `release` is reserved strictly for release candidate stabilization.
- **Zero Deployment Blockers**: External SaaS/AI tools (CodeRabbit, Codacy, Codecov, Scorecard) provide advisory feedback and must never block emergency hotfixes, local builds, or CI deployments.
- **Strict Bounded Performance**: The daemon (`fastid`) must strictly observe memory ceilings: 64 MiB idle, 96 MiB normal, 192 MiB process tree ceiling.
- **Zero Runtime Telemetry**: The daemon and client libraries must never include phone-home code, tracking SDKs, or external analytics.
- **YouTrack Workflow**: Fasti uses YouTrack (`fasti.youtrack.cloud`) for sprint execution, milestone tracking (B0–B8), and hardware receipts. Prefix commit messages and PR titles with YouTrack IDs (`FASTI-###`).
- **No-Publish Guardrail**: Workflows in `.github/workflows/` must have strictly read-only permissions before Milestone B8 release readiness.
