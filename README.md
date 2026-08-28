<div align="center">

<a href="https://github.com/Scrobble-dev/Fasti">
  <img src="brand/logos/fasti-lockup.svg" alt="Fasti — The Living Media Chronicle" width="460">
</a>

<br/><br/>

### _Every story, kept in time._

**An identity-first, self-hosted system of record for what you watch, read, hear, and play.**
**Fasti records. Players play.**

<br/>

[![CI](https://github.com/Scrobble-dev/Fasti/actions/workflows/ci.yml/badge.svg)](https://github.com/Scrobble-dev/Fasti/actions/workflows/ci.yml)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/14234/badge)](https://www.bestpractices.dev/projects/14234)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)

<br/>

[Purpose](#purpose) · [Current status](#current-status) · [Architecture](#current-b0-b4-review-architecture) · [Contracts](#contract-gates) · [Development](#development) · [Roadmap](#roadmap) · [Contributing](#contributing)

<br/>

<img src="brand/assets/fasti-brand-board.jpg" alt="Fasti Living Media Chronicle Brand Board" width="100%">

</div>

## Purpose

Media history is usually split across services, devices, readers, trackers, and launchers. Each source remembers only part of what happened and usually owns the identity it assigned.

Fasti is being built to keep a provider-neutral local record instead. A stable Fasti Record owns its local identity. Provider identifiers, observations, interpretations, corrections, and evidence attach to that Record without allowing a metadata provider to own it.

An unresolved or conflicted item remains valid data. Fasti must preserve what was observed, state what it does not know, and require an explicit decision when evidence cannot support an automatic link.

Fasti has no playback engine and no transcoding or decoding responsibility. Players, readers, services, and import tools can report observations through governed adapters once those capabilities are implemented.

## Current status

This repository is an engineering baseline, not a supported public release. No published container, package, product web application, desktop application, import adapter, replication service, or supported installation exists yet. The production daemon can construct the SQLite kernel for one-time node initialization and first-client enrollment when an operator supplies an explicit data root and uses direct loopback access or an explicitly declared loopback-only container port forward. It mounts durable production routes for observation acceptance and identity records, identifiers, and namespaces on the same local exposure. The local browser root renders the B4 Workbench review surface; its separate `/status` route renders the health diagnostic. The Workbench can read real Records with an `identity_read` bearer held only in tab memory. Unsupported mutations stay visible and disabled. Unpackaged desktop review code remains behind trusted-host setup. Neither interface is deployed or evidence that B4 is complete. Identity review and B3 correction/portability paths remain staged behind internal application ports for review. B1 remains open until all exact-head evidence is assembled and the milestone verifier passes.

The production daemon deliberately exposes only behavior it can prove:

| Surface                                                                                     | Current state                                                                                                                                                       |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `GET /api/v1/health`                                                                        | Implemented in `fastid` and described by the production OpenAPI document, which covers health, durable setup, observations, and records/namespaces routes           |
| One-time node initialization/enrollment                                                     | Durable production routes require `FASTI_DATA_ROOT` and direct loopback or an explicit loopback-only container port forward; one-time secrets remain in JSON bodies |
| B1 conformance HTTP and SSE                                                                 | Executable only in the feature-gated, loopback-only conformance server; all fixture successes declare `fixture_only` availability and `none` durability             |
| `POST /api/v1/observations`                                                                 | Durable production route, bearer-authenticated, mounted alongside node setup on the same local exposure                                                             |
| `POST`/`GET /api/v1/records`, `POST /api/v1/records/identifiers`, `POST /api/v1/namespaces` | Durable production routes, bearer-authenticated, mounted alongside node setup on the same local exposure                                                            |
| B2 local kernel                                                                             | Constructed by `fastid` for durable local setup; evidence, receipt, and identity review operations remain internal                                                  |
| B3 correction and portability                                                               | Implemented behind application ports and Linux SQLite/filesystem adapters for review; not mounted by `fastid` or `fasti` and not a release claim                    |
| `POST /api/v1/events`                                                                       | Absent from production; returns `404` until the B2 public contract and delivery adapter are activated together                                                      |
| `fasti capability list/show`                                                                | Reads the generated public capability registry locally; it does not activate later-body runtime behavior                                                            |
| `fasti export`, `restore`, and `verify`                                                     | Reserved for B3; exit nonzero and change no data                                                                                                                    |
| Browser Workbench                                                                           | Local-only B4 review surface at `/`; reads real Records with a memory-only scoped bearer and keeps unavailable mutations explicit                                   |
| Service-status diagnostic                                                                   | Local-only health-contract surface at `/status` for responsive, theme, keyboard, and accessibility evidence                                                         |
| Desktop interface                                                                           | Trusted-host review candidate only; unavailable commands remain disabled and B8 still owns supported packaging and release evidence                                 |
| Public images and binaries                                                                  | Disabled until the B8 readiness gate and an explicit release action                                                                                                 |

The feature-gated B1 fixture exists to execute contract semantics without pretending to be the local kernel. Its state is bounded, in-memory, and discarded when the fixture process exits. It is not mounted by `fastid` and is not a persistence or production-readiness claim.

The trusted desktop review host can store data-root-scoped Google Books and
TMDB credentials and return bounded neutral search candidates. Provider search
is local Tauri IPC, not a daemon HTTP route. The browser does not accept secrets
or execute provider requests. See [network configuration](docs/network-configuration.md#provider-network-policy).

The B2 review implementation adds local access, SQLite persistence, content-addressed evidence, review state, and durable receipts behind application ports. Node initialization, first-client enrollment, observation acceptance, and identity records/identifiers/namespaces are available through the production composition root. The remaining paths (evidence, receipts, identity review) are exercised by the Rust suites but stay unavailable through production HTTP. Their problems stay staged until the owning public surfaces are activated.

## Constitution

The controlling engineering rules are in [the Fasti constitution](docs/constitution.md). The short form is:

1. Stable local identity belongs to Fasti Records, never to a metadata provider.
2. Evidence and original observations are preserved; interpretations can be revised append-only.
3. Each rule and public meaning has one owner. Adapters consume that owner, and dependencies point toward the governed rules.
4. OpenAPI 3.1 via Utoipa, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, and generated SDK parity must agree before a capability is complete.
5. Local operation, network-denied proof, alternative packaging, low memory use, accessibility, and recovery are release evidence, not aspirations.
6. Every implementation body requires QA. Rendered UI or UX changes also require design review.

See the [glossary](docs/glossary.md), [capability ledger](docs/capability-ledger.md), and [Definition of Done](docs/definition-of-done.md) for the shared vocabulary and gates.

## Current B0-B4 review architecture

The active workspace has an inward-facing ownership spine and executable B1 contract surfaces:

```text
apps/fastid          production health plus explicit locally exposed durable composition root
apps/web             local Workbench at / plus the explicit /status diagnostic; not packaged or deployed
apps/desktop         trusted-host desktop review candidate; not packaged or released
crates/fasti-domain  typed IDs, time values, and domain invariants
crates/fasti-application
                     use cases, authorization, B1 fixture behavior, B2 ports, and typed problems
crates/fasti-contracts
                     shared public DTOs and generated capability identifiers
crates/fasti-api     production Utoipa health/setup/observations/records/namespaces API plus a separately gated loopback fixture
crates/fasti-cli     local capability discovery and explicit B3 nonzero guards
crates/fasti-store   B2 kernel mounted for setup, observations, records, and namespaces; evidence, receipt, review, and B3 adapters remain staged

contracts            authoritative registry, authored semantics, examples, and generated artifacts
packages/sdk         generated typed TypeScript HTTP/SSE client
packages/schemas     governed JSON Schema 2020-12 inputs
packages/tokens      approved design-token projection
packages/ui          Workbench and status presentation boundary; no domain, authorization, or retry policy
xtask                deterministic generation and fail-closed verification
```

Player, replication, connector, and provider-keyed projection packages are not active workspace boundaries. The Workbench consumes generated SDK and host surfaces; it does not own domain, authorization, retry, or offline policy. The B4 presentation and desktop packages are review candidates, not supported runtime capabilities. The retired core, activity, and auth scaffolds are also gone; their unsafe or duplicate models did not become compatibility aliases. B2 extends the existing domain and application boundaries instead of creating provider-specific paths or a second rule set. See [authentication boundaries](docs/architecture/authentication.md) for the implemented credential models and the preserved sign-in TODOs.

Native `fastid` binds to `127.0.0.1:8420` by default. Set `FASTI_LISTEN` to an explicit `IP:PORT` value when another listener is required. Port collisions fail closed by default. With `FASTI_PORT_FALLBACK=auto`, an occupied loopback port can recover to an OS-assigned port; public and wildcard listeners always fail closed. The local OCI image sets `FASTI_LISTEN=0.0.0.0:8420` so an operator can publish the container port deliberately. See [network configuration](docs/network-configuration.md) for custom domains, `.internal`, system CA trust, public URLs, loopback aliases, Docker, Podman, and collision behavior.

## Contract gates

B1 has a machine-readable capability registry as the authoritative public ledger. Deterministic generation projects that meaning into:

- a production OpenAPI 3.1 document covering health, durable setup, observations, Records, identifiers, and namespaces;
- a separate OpenAPI 3.1 document for real, feature-gated conformance handlers and shared public DTOs;
- AsyncAPI 3.x transport binding for the `receipt.stream` SSE channel;
- JSON Schema 2020-12 for public payloads;
- JSON-LD 1.1 vocabularies and contexts with expansion tests;
- OKF, semantic examples, permissions, problems, and knowledge links;
- a generated TypeScript HTTP/SSE SDK with typed parsing, problems, bounded input, and governed reconnect behavior;
- local `fasti capability list` and `fasti capability show <id>` views of the same public registry.

`cargo xtask contract verify --locked` fails closed on registry, generation, drift, semantic examples, standards, SDK, Rust, package, and repository-truth gates and emits a software receipt only after all checks pass. The B2 implementation does not silently add its reserved failures to the B1 public output. Exact-head CI verifies that OpenAPI, AsyncAPI, JSON Schema, JSON-LD, OKF, examples, and the SDK remain unchanged unless their owning public contract changes. Historical receipts do not close B1: the milestone verifier requires current contract, QA, raw-gate, Tauri, and two-architecture low-hardware envelope evidence. See [contracts/README.md](contracts/README.md) for ownership and current locations.

## Performance and portability targets

Fasti targets small self-hosted hardware rather than treating it as an afterthought:

- 64 MiB idle target;
- 96 MiB normal-operation target;
- 160 MiB heavy-operation target;
- 192 MiB absolute process-tree ceiling;
- a kernel-enforced 192 MiB, one-vCPU, zero-swap CI envelope with a 600-second warm-up and 900-second route-less idle measurement on x86_64 and aarch64;
- optional Raspberry Pi 5 and J4125 comparison specifications;
- Ugoos AM6B+, Xiaomi Box M3, Nvidia Shield, and representative TV hardware as explicit packaging hypotheses.

The B1 performance gate retains the exact measured release daemon, raw idle observations, OCI image, and contract pack. It recomputes the kernel controls, memory, CPU, architecture, and applicable artifact budgets. Only two receipts that declare the same workflow run attempt for one exact `dev` push can qualify. Pull-request runs are regression checks, not milestone evidence. The optional device profiles do not block B1.

## Development

See [docs/dev-loop.md](docs/dev-loop.md) for the day-to-day dev loop (`bash scripts/dev.sh`), how to QA it, and how Docker/Podman fit in.

There is no supported installation yet. The shortest contributor path runs the production daemon's one truthful capability. With the Rust toolchain and dependencies available:

```bash
cargo run --locked -p fastid
```

In a second terminal:

```bash
curl --fail --silent http://127.0.0.1:8420/api/v1/health
```

The exact response is `{"status":"healthy","version":"0.1.0"}`.

Without `FASTI_DATA_ROOT`, that proves only the health-only production composition root. To activate durable one-time setup, bind to loopback and supply a private data directory:

```bash
FASTI_LISTEN=127.0.0.1:8420 \
FASTI_DATA_ROOT=/path/to/private/fasti-data \
cargo run --locked -p fastid
```

The production OpenAPI document defines the initialization and enrollment requests. The TypeScript SDK exposes them as `initializeDurableNode` and `enrollDurableFirstClient`. Keep the returned proof and credential out of logs, URLs, shell history, and browser storage. A non-loopback bind remains health-only even when a data root is present, unless a wildcard container listener has an explicit loopback-only `FASTI_EXTERNAL_BIND_IP` assertion. Stop the daemon with `Ctrl-C`. The same local exposure also activates observation acceptance and identity records, identifiers, and namespaces; it does not activate identity review, portability, installation, or release readiness.

The scoped launcher supplies its private data root in native and container modes. It refuses to report success unless both health and the real durable router are present:

```bash
FASTI_PORT=19420 ./scripts/dev.sh
FASTI_PORT=19420 ./scripts/dev.sh --podman
FASTI_PORT=19420 ./scripts/dev.sh --docker
./scripts/dev.sh --status
./scripts/dev.sh --stop
```

`FASTI_PORT` sets the native or host port. `FASTI_LISTEN` and `FASTI_API_URL` can override the native listen address and probe URL. `FASTI_PUBLIC_URL` records a separate reverse-proxy origin and can omit the port when HTTPS uses port 443. Port collisions fail closed by default; set `FASTI_PORT_FALLBACK=auto` to allow safe loopback recovery. Native mode requires a user cgroup v2 scope. Native and container modes read the 192 MiB ceiling from the governed performance budget and disable swap. The canonical benchmark remains the owner of the separate 64 MiB idle measurement. Container mode uses the documented `fasti:b0` image and publishes only to host loopback; `FASTI_IMAGE` can select another local image. The launcher tracks only this worktree's process and container.

With `fastid` still running on its default port, the local browser Workbench uses the generated health and Records contracts:

```bash
pnpm install --frozen-lockfile
pnpm dev:web
```

Open `http://127.0.0.1:5173/` for the Workbench or `/status` for the service diagnostic. Vite forwards same-origin `/api` requests to the active local daemon during development. The browser can save a different non-secret service URL, but it never stores a bearer. A cross-origin browser URL requires an HTTPS reverse proxy that explicitly permits the Workbench origin through CORS. Direct `fastid` does not provide CORS. These client settings do not bind a listener or configure DNS, TLS, a reverse proxy, or a public deployment. See the [Workbench Impeccable audit](docs/qa/b4-workbench-impeccable-audit.md) and the [superseded status-only evidence](docs/qa/b4-truthful-shell-evidence.md) for their separate scopes.

To inspect the governed public capability identifiers without starting a service:

```bash
cargo run --locked -p fasti-cli -- capability list
```

Contract authors can run the loopback-only, nondurable B1 fixture with:

```bash
cargo run --locked -p fasti-api \
  --features conformance-fixture \
  --bin b1-conformance-server -- 127.0.0.1:8421
```

The fixture prints one JSON readiness line declaring `"availability":"fixture_only"` and `"durability":"none"`. Its command help is available through the same Cargo invocation with `-- --help`. See the [contract ownership guide](contracts/README.md) and the [local TypeScript SDK guide](packages/sdk/README.md) for the bounded integration-author path.

The full source baseline uses the tool versions selected by CI and OCI:

- Rust `1.97.1`;
- Node.js `22`;
- pnpm `11.22.0`.

```bash
git clone https://github.com/Scrobble-dev/Fasti.git
cd Fasti

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask contract verify --locked

pnpm install --frozen-lockfile
pnpm format:check
pnpm typecheck
pnpm test
pnpm test:ui

bash scripts/check-repository-truth.sh
bash scripts/check-no-publish.sh
node scripts/check-doc-links.mjs

podman build --tag fasti:b0 .
bash scripts/smoke-oci.sh fasti:b0 "" podman
```

The staged B3 portability slice has runnable internal gates. These commands do not activate the guarded CLI:

```bash
cargo test -p fasti-store archive::tests::filesystem_destination_sigkill_matrix --locked -- --exact
cargo test -p fasti-store restore_import::tests::full_restore_sigkill_matrix --locked -- --exact
cargo test -p fasti-store restore_import::tests::full_import_activation_survives_owner_drop_and_opens_exact_database --locked -- --exact
cargo test -p fasti-store stopped_portability::tests::stopped_adapter_restore_refuses_a_live_data_root_before_archive_input --locked -- --exact
```

The local image contains `fastid` and `fasti`, runs as the non-root `fasti` user, and is never pushed by repository automation. The shared smoke gate accepts Docker or Podman explicitly. It verifies health-only behavior without a data root, the real protected durable router through an explicit loopback-only port forward, guarded CLI failure, and a 64 MiB host-side idle-memory threshold. The B1 deep gate binds Podman and its exact version in the receipt. That one-shot container sample is a regression sentinel, not the two-architecture envelope package required by the B1 milestone verifier. Override the default command to inspect a guarded command, for example `podman run --rm fasti:b0 /usr/local/bin/fasti verify`; it must exit nonzero until B3 public activation.

## Brand and design system

The approved [brand and design system](brand/DESIGN.md) is a protected input. B0 preserves its tokens, logos, boards, preview assets, accessibility rules, and ADHD/AuDHD state-continuity requirements byte-for-byte.

The local Workbench is the B4 media-interface review surface. It preserves media-first navigation, poster and row views, a collapsible rail, visible quick actions for activity, watchlist, collection, and rate/note, and a Tabler-based theme panel governed by Fasti tokens. Controls remain disabled until their host capability is real. The separate `/status` route remains bounded health evidence. There is no playback control and no persistent “offline ready” badge.

## Relationship to Scrobble.dev

[Scrobble.dev](https://scrobble.dev) defines neutral vocabularies, schemas, crosswalks, fixtures, and conformance rules. Fasti is an AGPL implementation that must earn compatibility through those independent contracts. Fasti does not control the standard, and another project does not need Fasti to implement it.

## Roadmap

- **B0: Controlling baseline** — remove false claims and public publishing paths; keep native and OCI builds honest.
- **B1: Executable contract spine** — software surfaces are executable and drift-proof; closure still requires a current aggregate manifest with QA, Tauri, and same-attempt x86_64/aarch64 envelope receipts.
- **B2: Local kernel** — observation acceptance and identity records/identifiers/namespaces are publicly activated on the durable loopback bind; evidence, receipts, and identity review remain behind internal ports for review; full milestone evidence and constrained-hardware qualification remain open.
- **B3: Corrections and portability** — internal append-only correction, deterministic export, clean restore, equality verification, crash recovery, and credential re-bootstrap are implemented for review; public activation and milestone evidence remain open.
- **B4: Product experience** — the approved media Workbench is restored as local review code and real record reads are connected; mutation contracts, full accessibility evidence, packaging, and milestone acceptance remain open.
- **B5 and later** — implement provider patterns, packaging, hardware qualification, and release readiness in gated bodies.

Nuvio adaptation does not begin before the B7 provider gate, applicable B8 evidence, and maintainer agreement. See [ROADMAP.md](ROADMAP.md) for the dependency order.

## Contributing

Fasti is an independent, community-driven project under AGPL-3.0-or-later and DCO 1.1. We are **actively seeking co-maintainers, code reviewers, and contributors** across Rust systems engineering, TypeScript/UI presentation, OpenAPI contracts, and hardware benchmarks.

### 🤝 Maintainers & Contributors Wanted

We are intentionally expanding the maintainer team to improve the project's bus factor and code-review coverage:

- **Co-Maintainers & Reviewers**: We welcome maintainers familiar with async Rust (`axum`, `rusqlite`), Domain-Driven Design, OpenAPI 3.1 / AsyncAPI 3.x schema validation, and Svelte / Tauri presentation.
- **Good First Issues**: Check out our curated list of [Good First Issues](https://github.com/Scrobble-dev/Fasti/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22) for well-scoped onboarding tasks with step-by-step instructions and test commands.
- **Hardware Receipts**: Help us test and benchmark Milestone B1 memory envelopes (<= 64 MiB idle) on diverse physical hardware (Raspberry Pi 5, Intel J4125, Apple Silicon, TV boxes per [#49](https://github.com/Scrobble-dev/Fasti/issues/49) and [#50](https://github.com/Scrobble-dev/Fasti/issues/50)).

To contribute:

1. Browse open [Issues](https://github.com/Scrobble-dev/Fasti/issues) or start a [GitHub Discussion](https://github.com/Scrobble-dev/Fasti/discussions) to align on scope.
2. Follow [CONTRIBUTING.md](CONTRIBUTING.md) and sign your commits with the [Developer Certificate of Origin](https://developercertificate.org/) (`git commit -s`).
3. **All active development and pull requests must target the `dev` branch.**
4. Adhere to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Security

No Fasti release is currently supported for production use. Please still report suspected vulnerabilities privately through the process in [SECURITY.md](SECURITY.md); do not open a public issue for an undisclosed security problem.

## Supporting Fasti

Fasti is independent and community-driven. Financial support helps fund infrastructure, accessibility testing, hardware qualification, and open vocabulary work:

- [GitHub Sponsors](https://github.com/sponsors/ryan-winkler)
- [Open Collective](https://opencollective.com/scrobble)
- [Ko-fi](https://ko-fi.com/ryanw_eu)
- [Revolut](https://revolut.me/ryanwi)

## Licence

Fasti is open-source software licensed under the [GNU Affero General Public License v3.0 or later](LICENSE), identified as `AGPL-3.0-or-later`. The current contribution terms are AGPL plus DCO. Any possible future noncommercial or paid-commercial model is an owner policy question and is not a present collaboration gate.

Scrobble.dev specification assets use the licence designated by their source repository. Files copied into Fasti remain governed by the licence notices included with those files.
