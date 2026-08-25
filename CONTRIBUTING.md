# Contributing to Fasti

Thank you for your interest in contributing to Fasti!

Fasti is an open-source, identity-first local system of record for media activity. **Fasti records; players play.** We welcome collaboration from developers, designers, archivists, integrators, accessibility practitioners, security researchers, and writers of all experience levels.

---

## 1. Developer Certificate of Origin (DCO)

To ensure that all contributions can be freely redistributed under our open-source licence ([AGPL-3.0-or-later](LICENSE)), Fasti uses the **Developer Certificate of Origin (DCO 1.1)** rather than a Contributor Licence Agreement (CLA).

### What This Means For You
* You retain copyright in your contributions.
* You certify that you wrote the code or have the legal right to submit it under the project's open-source licence.
* You sign off on every commit by adding `Signed-off-by: Name <email>` to the commit message.

### How to Sign Off Commits
When using Git, pass the `-s` or `--signoff` flag:

```bash
git commit -s -m "feat(activity): add idempotent deduplication check"
```

If you forgot to sign off a commit on a branch:
```bash
git commit --amend --no-edit -s
# Or for multiple commits against dev:
git rebase --signoff origin/dev
```

### Full DCO 1.1 Text
```
Developer Certificate of Origin
Version 1.1

Copyright (C) 2004, 2006 The Linux Foundation and its contributors.

By making a contribution to this project, I certify that:

(a) The contribution was created in whole or in part by me and I
    have the right to submit it under the open source license
    indicated in the file; or

(b) The contribution is based upon previous work that, to the best
    of my knowledge, is covered under an appropriate open source
    license and I have the right under that license to submit that
    work with modifications, whether created in whole or in part
    by me, under the same open source license (unless I am
    permitted to submit under a different license), as indicated
    in the file; or

(c) The contribution was provided directly to me by some other
    person who certified (a), (b) or (c) and I have not modified
    it; and

(d) I understand and agree that this project and the contribution
    are public and that a record of the contribution (including all
    personal information I submit with it, including my sign-off) is
    maintained indefinitely and may be redistributed consistent with
    this project or the open source license(s) involved.
```

---

## 2. Constitutional Separation from Scrobble.dev

Please note the foundational boundary between Fasti and [Scrobble.dev](https://scrobble.dev):

* **Scrobble.dev** defines the normative vocabularies, JSON schemas, activity profiles, and independent conformance test suites under Community Specification and permissive licences.
* **Fasti** is an open-source application and reference implementation of Scrobble.dev concepts.

> **Important:** Changes to Fasti code cannot silently alter the Scrobble.dev activity standard. If you want to propose a new activity semantic or vocabulary field, please submit an RFC to the Scrobble.dev specification working group first.

---

## 3. Core Domain & Identity Rules

Fasti's domain model is governed by explicit identity rules defined in [`contracts/identity/identity-contract-seed.yaml`](contracts/identity/identity-contract-seed.yaml):

1. **Stable Local Record Key**: Every Fasti record has an immutable, opaque local ID (`rec_*`).
2. **Provider Independence**: No external provider ID (IMDb, TVDB, Trakt, Spotify, MusicBrainz, Steam, AniList) is ever the canonical record key. Provider IDs are typed claims attached to the record (`xid_*`).
3. **Unresolved Records are Valid Data**: Incomplete, ambiguous, or un-enriched scrobbles are valid, preserved data. Fasti never drops or hides an observation because metadata lookup failed.
4. **Original Observations are Immutable**: Raw scrobble observations (`obs_*`) are append-only, immutable evidence.
5. **Metadata is a Disposable Projection**: Enriched details and posters are cached projections. They can be invalidated and recomputed from evidence without data loss.
6. **Directional Mappings**: Cross-namespace mappings are directional, typed, scoped assertions with clear provenance.
7. **Absence != Deletion**: Absence of a media item from an upstream provider does not delete history in Fasti.
8. **Provider Changes Do Not Move History**: Upstream provider ID merges, splits, or renames never rewrite or relocate Fasti chronicle history.
9. **Explicit Reconciliation**: Irreversible operations (splits, merges, destructive overrides) require preview and stronger evidence receipts.

---

## 4. Contract-Led Architecture & Schema Supremacy

Fasti is strictly **Contract-Led**. The machine-authored registry at [`contracts/registry/v1/capabilities.yaml`](contracts/registry/v1/capabilities.yaml) is the authoritative ledger for capability IDs, bounded-context ownership, scopes, and problems.

Domain rules own meaning; application services own capabilities; contracts project that meaning into all surfaces:

```
                  Authored Capability Registry & Seeds
               (contracts/registry/, contracts/identity/)
                                  │
      ┌───────────────────────────┼───────────────────────────┐
      ▼                           ▼                           ▼
OpenAPI 3.1                 AsyncAPI 3.x                 JSON-LD 1.1
REST Operations & DTOs      SSE & Event Channels         Linked Data Vocabulary
(`fasti-api`, `fastid`)     (`receipt.stream`)           (`contracts/jsonld/`)
      │                           │                           │
      ▼                           ▼                           ▼
JSON Schema 2020-12            OKF 0.2                    TypeScript SDK & CLI
Archive & Payload Schemas   Operational Knowledge        Generated Client & Parsers
(`contracts/portability/`)  (`contracts/okf/`)           (`packages/sdk/`, `fasti-cli`)
```

### Projections vs. Sources of Truth
* **Never edit generated files directly** (e.g. `contracts/generated/*`, `packages/sdk/src/generated/*`).
* Modify the authored YAML/JSON contracts in `contracts/registry/`, `contracts/identity/`, `contracts/asyncapi/`, `contracts/jsonld/`, or `contracts/okf/`.
* Run deterministic generation and verification:
  ```bash
  cargo xtask contract generate
  cargo xtask contract verify --locked
  ```

---

## 5. Contribution Tiers & Entry Gates

To balance architectural rigor with low contributor friction, Fasti uses a tiered entry model:

| Contribution Tier | Examples | Required Entry | Review Requirement |
|---|---|---|---|
| **Tier 1: Fast-Track** | Typo, broken link, documentation clarification, formatting | Direct Pull Request to `dev` | Doc links & truth checks |
| **Tier 2: Issue-Bound** | Bug fix, test fixture, error diagnostic improvement, CLI command | Claim open GitHub/YouTrack Issue → PR to `dev` | Automated test suite & QA |
| **Tier 3: Capability & Adapter** | Ingest webhook adapter, storage driver, portability manifest | Claim Issue / RFC draft → PR to `dev` | Contract verification, fault evidence, QA |
| **Tier 4: Domain, Identity & Schema** | New capability, identity rule, schema mutation, auth/crypto | GitHub Discussion / Architecture RFC before code | Domain ownership review, contract verification, multi-surface parity |

---

## 6. Development Workflow

### Getting Started
1. **Fork and clone** the repository:
   ```bash
   git clone https://github.com/<your-username>/Fasti.git
   cd Fasti
   ```
2. **Create a topic branch from `dev`**:
   ```bash
   git checkout -b feat/my-new-feature origin/dev
   ```
3. **Verify the environment**:
   ```bash
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
   ```

### Branch Naming Conventions
* `feat/<short-description>`: New capabilities or adapters
* `fix/<short-description>`: Bug fixes
* `docs/<short-description>`: Documentation additions or corrections
* `refactor/<short-description>`: Code improvements without behavioral changes
* `test/<short-description>`: New unit, integration, or chaos fixtures

### Commit Message Guidelines
We follow Conventional Commits format with mandatory DCO sign-off:
```
<type>(<scope>): <short summary in imperative mood>

[optional longer body explaining context, rationale, and tradeoffs]

Signed-off-by: Jane Developer <jane@example.com>
```
*Types:* `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.  
*Scopes:* `domain`, `application`, `contracts`, `store`, `api`, `cli`, `sdk`, `tokens`, `design`, `docs`, `infra`.

---

## 7. Code Quality & Standards

### Rust Code (`crates/*`, `apps/fastid`)
* Format code with `cargo fmt --all`.
* Ensure `cargo clippy --workspace --all-targets --locked -- -D warnings` passes cleanly.
* Keep domain policy out of adapters and preserve one semantic owner for invariants, capabilities, public types, permissions, and problems.
* Maintain error transparency: use typed errors for libraries and avoid bare `.unwrap()` in production codepaths.

### TypeScript (`packages/*`)
* Keep strict typing enabled and consume generated contract types from `packages/sdk`.
* Do not redefine domain, permission, error, retry, idempotency, or offline behavior in a client package.
* Format code with Prettier and verify with `pnpm format:check`, `pnpm typecheck`, and `pnpm test`.

### Rendered UI and UX
* Do not add a placeholder web application before B4.
* Follow [`brand/DESIGN.md`](brand/DESIGN.md) and the approved media-first interaction contract.
* Every rendered change requires design review and QA, including keyboard, screen reader, responsive, 44 px target, reduced-motion, focus-return, and ADHD/AuDHD state-continuity evidence.

---

## 8. Submitting a Pull Request

1. **Target Branch**: Open all PRs against **`dev`** (our integration branch). `release` is reserved for verified release candidates.
2. **Link Issues / Discussions**: Reference the claimed issue or linked Discussion (Tier 3/4).
3. **Run PR Gate**: Execute `cargo xtask test pr` locally before submission to verify all required checks pass.
4. **Complete the PR Template**: Ensure all invariant checkboxes and DCO sign-offs are completed in [.github/PULL_REQUEST_TEMPLATE.md](.github/PULL_REQUEST_TEMPLATE.md).
5. **CI & Verification**: All deterministic checks must pass. Automated AI/SaaS tools provide suggestions but do not block PR merges.

---

## 9. Governance, Roles & Access Continuity

### Governance Model
Fasti operates as a maintainer-driven open-source project with consensus-seeking review. Architectural proposals and milestone capability decisions follow open discussions in GitHub Discussions and RFC issues before code changes begin.

### Key Project Roles
* **Project Lead / Core Maintainer**: Ryan Winkler (`@ryan-winkler` / `hi@ryanw.eu`). Responsible for overall system architecture, release tagging, security triage, and contract verification.
* **Component Reviewers & Maintainers**: Reviewers with commit/approval authority across specific bounded contexts (`domain`, `application`, `store`, `api`, `contracts`, `ui`).
* **Contributors**: Anyone submitting issues, documentation, code, hardware receipts, or UX improvements under DCO 1.1.

### Access Continuity & Succession
To ensure continuity if any maintainer is incapacitated or unavailable:
* All project repositories, DNS, and hosting assets are owned under the **`Scrobble-dev`** GitHub organization.
* Organization administrator credentials and emergency recovery keys are stored securely in escrow.
* The governance policy permits designated organization administrators to assume maintenance duties, manage issue queues, accept pull requests, and publish emergency releases within one week of confirmed unavailability.

---

Thank you for helping Fasti keep media records trustworthy, portable, and provider-neutral!
