<div align="center">

<a href="https://github.com/Scrobble-dev/Fasti">
  <img src="brand/logos/fasti-lockup.svg" alt="Fasti — The Living Media Chronicle" width="460">
</a>

<br/><br/>

### *Every story, kept in time.*

**An identity-first, self-hosted system of record for what you watch, read, hear, and play.**
**Fasti records. Players play.**

<br/>

[![CI](https://github.com/Scrobble-dev/Fasti/actions/workflows/ci.yml/badge.svg)](https://github.com/Scrobble-dev/Fasti/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![Sponsor](https://img.shields.io/badge/Sponsor-GitHub%20Sponsors-EA4AAA.svg?logo=github-sponsors)](https://github.com/sponsors/ryan-winkler)
[![Open Collective](https://img.shields.io/badge/Donate-Open%20Collective-7B8099.svg?logo=open-collective)](https://opencollective.com/scrobble)
[![Ko-fi](https://img.shields.io/badge/Donate-Ko--fi-FF5E5B.svg?logo=kofi)](https://ko-fi.com/ryanw_eu)
[![Revolut](https://img.shields.io/badge/Donate-Revolut-0075EB.svg)](https://revolut.me/ryanwi)

<br/>

[Purpose](#purpose) · [Current status](#current-status) · [Architecture](#current-b0-architecture) · [Contracts](#contract-gates) · [Development](#development) · [Roadmap](#roadmap) · [Contributing](#contributing)

<br/>

<img src="brand/assets/fasti-brand-board.jpg" alt="Fasti Living Media Chronicle Brand Board" width="100%">

</div>

## Purpose

Media history is usually split across services, devices, readers, trackers, and launchers. Each source remembers only part of what happened and usually owns the identity it assigned.

Fasti is being built to keep a provider-neutral local record instead. A stable Fasti Record owns its local identity. Provider identifiers, observations, interpretations, corrections, and evidence attach to that Record without allowing a metadata provider to own it.

An unresolved or conflicted item remains valid data. Fasti must preserve what was observed, state what it does not know, and require an explicit decision when evidence cannot support an automatic link.

Fasti has no playback engine and no transcoding or decoding responsibility. Players, readers, services, and import tools can report observations through governed adapters once those capabilities are implemented.

## Current status

This repository is an engineering baseline, not a supported public release. No published container, package, web application, desktop application, import adapter, replication service, or supported installation exists yet.

The B0 baseline deliberately exposes only behavior it can prove:

| Surface | B0 state |
|---|---|
| `GET /api/v1/health` | Implemented process health response |
| `POST /api/v1/events` | Absent; returns `404` until B2 can persist and replay a durable receipt |
| `fasti export` | Reserved for B3; exits nonzero and changes no data |
| `fasti restore` | Reserved for B3; exits nonzero and changes no data |
| `fasti verify` | Reserved for B3; exits nonzero and emits no success receipt |
| Web UI | Not implemented; B4 owns the approved Tabler-based media interface |
| Desktop packaging | Not implemented; B8 owns packaged application work |
| Public images and binaries | Disabled until the B8 readiness gate and an explicit release action |

The governed identity seed, provider-manifest example, UAT matrix, schema draft, and brand system are inputs to the implementation. Their presence does not claim that their capabilities already exist.

## Constitution

The controlling engineering rules are in [the Fasti constitution](docs/constitution.md). The short form is:

1. Stable local identity belongs to Fasti Records, never to a metadata provider.
2. Evidence and original observations are preserved; interpretations can be revised append-only.
3. Domain meaning has one owner and every adapter consumes it. Domain-driven design and DRY are gates.
4. OpenAPI 3.1 via Utoipa, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, and generated SDK parity must agree before a capability is complete.
5. Local operation, network-denied proof, alternative packaging, low memory use, accessibility, and recovery are release evidence, not aspirations.
6. Every implementation body requires QA. Rendered UI or UX changes also require design review.

See the [glossary](docs/glossary.md), [capability ledger](docs/capability-ledger.md), and [Definition of Done](docs/definition-of-done.md) for the shared vocabulary and gates.

## Current B0 architecture

The active workspace is intentionally small while B1 establishes the final bounded contexts:

```text
apps/fastid          health-only daemon composition root
crates/fasti-api     implemented HTTP health route; unsupported routes remain absent
crates/fasti-cli     explicit nonzero guards for planned B3 operations
crates/fasti-core    draft primitives retained for B1 reconciliation
crates/fasti-activity
crates/fasti-store
crates/fasti-auth    draft scaffolds retained only until B1 folds proven primitives

packages/sdk         minimal typed health client
packages/schemas     governed draft schema input
packages/tokens      approved design-token projection
```

Player, replication, connector, provider-keyed projection, presentation, desktop, and placeholder web packages are not active workspace boundaries. B1 will introduce `fasti-domain`, `fasti-application`, `fasti-contracts`, and the contract-generation task runner before new capabilities are added.

Native `fastid` binds to `127.0.0.1:8420` by default. Set `FASTI_LISTEN` to an explicit `IP:PORT` value when another listener is required. The local OCI image sets `FASTI_LISTEN=0.0.0.0:8420` so an operator can publish the container port deliberately.

## Contract gates

B0 validates the syntax and shape of governed drafts. It does not claim full standards conformance.

B1 must make the following surfaces executable and drift-proof before its capabilities are accepted:

- OpenAPI 3.1 generated from real Utoipa-bound handlers and shared public DTOs;
- AsyncAPI 3.x transport bindings for channels, replay, and message flow;
- JSON Schema 2020-12 for public payloads;
- JSON-LD 1.1 vocabularies and contexts with expansion tests;
- OKF, examples, permissions, errors, and knowledge links;
- a generated TypeScript HTTP/SSE SDK with typed problems and governed retry behavior.

Generation must be deterministic. Checked-in artifacts, executable handlers, CLI behavior, SDK methods, and the [capability ledger](docs/capability-ledger.md) must describe the same capability. See [contracts/README.md](contracts/README.md) for ownership and current locations.

## Performance and portability targets

Fasti targets small self-hosted hardware rather than treating it as an afterthought:

- 64 MiB idle target;
- 96 MiB normal-operation target;
- 160 MiB heavy-operation target;
- 192 MiB absolute process-tree ceiling;
- Raspberry Pi 5 champion profile and a calibrated J4125-class x86 profile;
- Ugoos AM6B+, Xiaomi Box M3, Nvidia Shield, and representative TV hardware as explicit packaging hypotheses.

These are gates only when measured on the named artifact and hardware profile. B0 does not present estimates as passing evidence.

## Development

There is no supported installation yet. Contributors can verify the current source baseline with the tool baselines selected by CI and OCI:

- Rust `1.97.1`;
- Node.js `22`;
- pnpm `11.22.0`.

```bash
git clone https://github.com/Scrobble-dev/Fasti.git
cd Fasti

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

pnpm install --frozen-lockfile
pnpm format:check
pnpm typecheck
pnpm test

bash scripts/check-repository-truth.sh
bash scripts/check-no-publish.sh
node scripts/check-doc-links.mjs

docker build --tag fasti:b0 .
bash scripts/smoke-oci.sh fasti:b0
```

The local image contains `fastid` and `fasti`, runs as the non-root `fasti` user, and is never pushed by repository automation. The shared smoke gate verifies process health, the absent event route, guarded CLI failure, and a 64 MiB B0 idle-memory threshold. That one-shot container sample is a regression sentinel, not the Raspberry Pi 5 or J4125 performance receipt required by later bodies. Override the default command to inspect the guarded CLI, for example `docker run --rm fasti:b0 /usr/local/bin/fasti verify`; it must exit nonzero in B0.

## Brand and design system

The approved [brand and design system](brand/DESIGN.md) is a protected input. B0 preserves its tokens, logos, boards, preview assets, accessibility rules, and ADHD/AuDHD state-continuity requirements byte-for-byte.

The product interface arrives after the headless contract and local kernel. Its fixed direction is media-first navigation, poster and row views, a collapsible rail, visible quick actions for activity, watchlist, collection, and rate/note, and a Tabler-based theme panel governed by Fasti tokens. There is no playback control and no persistent “offline ready” badge.

## Relationship to Scrobble.dev

[Scrobble.dev](https://scrobble.dev) defines neutral vocabularies, schemas, crosswalks, fixtures, and conformance rules. Fasti is an AGPL implementation that must earn compatibility through those independent contracts. Fasti does not control the standard, and another project does not need Fasti to implement it.

## Roadmap

- **B0: Controlling baseline** — remove false claims and public publishing paths; keep native and OCI builds honest.
- **B1: Executable contract spine** — establish bounded contexts and generate the required OpenAPI, AsyncAPI, Schema, JSON-LD, OKF, and SDK surfaces.
- **B2: Local kernel** — persist evidence, identity, access, operations, and durable receipt replay on constrained hardware.
- **B3: Corrections and portability** — append-only interpretation revision plus complete export, clean restore, and equality proof.
- **B4 and later** — implement the approved media UI, provider patterns, packaging, hardware qualification, and release readiness in gated bodies.

Nuvio adaptation does not begin before the B7 provider gate, applicable B8 evidence, and maintainer agreement. See [ROADMAP.md](ROADMAP.md) for the dependency order.

## Contributing

Fasti is an open community project under AGPL-3.0-or-later and DCO 1.1. Collaboration on code, documentation, fixtures, accessibility, security, provider patterns, recipes, and design is encouraged.

Before writing code or opening a pull request, begin or join a GitHub Discussion and align the problem, bounded context, scope, and required review gates. This is technical and product scope alignment, not legal approval. A CLA or legal review is not required.

Then follow [CONTRIBUTING.md](CONTRIBUTING.md), sign commits with the [Developer Certificate of Origin](https://developercertificate.org/) using `git commit -s`, and follow the [Code of Conduct](CODE_OF_CONDUCT.md).

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
