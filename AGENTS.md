# AGENTS.md

## Start here

Before planning or implementation, read these in order:

1. [`docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md`](docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md)
2. [`docs/handoffs/FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md`](docs/handoffs/FASTI_EXTERNAL_HARNESS_CONTEXT_SAVE_2026-08-25.md)
3. [`README.md`](README.md)
4. [`docs/constitution.md`](docs/constitution.md)
5. [`docs/definition-of-done.md`](docs/definition-of-done.md)
6. [`ROADMAP.md`](ROADMAP.md)
7. [`docs/capability-ledger.md`](docs/capability-ledger.md)
8. [`contracts/README.md`](contracts/README.md)
9. [`SECURITY.md`](SECURITY.md)

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
- Mount durable local routes only on loopback with an explicit `FASTI_DATA_ROOT`; never infer a data directory.
- Resolve provider hosts once, reject every unsafe answer, disable redirects and system proxies, and pin the authorized addresses before loading a credential.
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

## Design system

Read [`brand/DESIGN.md`](brand/DESIGN.md) before making any visual or UI decision. Font choices, colors, spacing, and aesthetic direction are defined there. Do not deviate without explicit approval.

Use the smallest applicable component in this order:

1. upstream Tabler;
2. Tabler with Fasti tokens;
3. composed Tabler components;
4. custom Svelte only when Tabler has no equivalent, with a recorded reason.

Do not use generic SaaS card walls, decorative gradients or glows, or continuous decorative motion. User-facing UI must meet the applicable WCAG 2.2 Level AA and EN 301 549 requirements. Automated accessibility scans supplement manual keyboard and target-platform assistive-technology checks; they do not prove conformance.

In QA mode, flag any code that does not match `brand/DESIGN.md`.

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
- Visual polish → invoke /design-review
- Ship/deploy/PR → invoke /ship or /land-and-deploy
- Save progress → invoke /context-save
- Resume context → invoke /context-restore
- Author a backlog-ready spec/issue → invoke /spec

## Review and handoff

Preserve issue and PR history. Do not edit another contributor's original body.

Document:

- user impact;
- contract impact;
- offline impact;
- security impact;
- performance evidence;
- accessibility impact;
- tests;
- rollback.

All active development pull requests target `dev`. The `release` branch is reserved for release-candidate stabilization. Before B8, repository workflows must use strictly read-only permissions.
