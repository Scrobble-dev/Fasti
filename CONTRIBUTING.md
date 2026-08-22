# Contributing to Fasti

Thank you for your interest in contributing to Fasti!

Fasti is an open-source, identity-first local system of record for media activity. Fasti records; players play. We welcome collaboration from developers, designers, archivists, integrators, accessibility practitioners, security researchers, and writers of all experience levels.

> **Discuss before you implement or open a pull request.** Begin or join a GitHub Discussion and align the problem, bounded context, scope, and required review gates first. This is technical and product scope alignment, not legal approval. Fasti does not require a CLA or legal review before community collaboration.

---

## 1. Developer Certificate of Origin (DCO)

To ensure that all contributions can be freely redistributed under our open-source licence ([AGPL-3.0-or-later](LICENSE)), Fasti uses the **Developer Certificate of Origin (DCO 1.1)** rather than a Contributor Licence Agreement (CLA).

### What This Means For You
* You retain copyright in your contributions.
* You certify that you wrote the code or have the legal right to submit it under the project's open-source licence.
* You sign off on every commit by adding `Signed-off-by: Name <email>` to the commit message.

### How to Sign Off Commits
When using Git, simply pass the `-s` or `--signoff` flag:

```bash
git commit -s -m "feat(activity): add idempotent deduplication check"
```

If you forgot to sign off a commit on a branch:
```bash
git commit --amend --no-edit -s
# Or for multiple commits:
git rebase --signoff origin/release
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

## 3. Contribution Lanes

We organize contributions into explicit lanes to set expectations on review requirements:

| Lane | Scope | Review Requirement |
|---|---|---|
| **Documentation & Guides** | Truth corrections, architecture, recovery, examples, and task-first guides | Documentation checks and QA |
| **Governed Fixtures** | Provenance-labelled media observations, identity cases, and hostile inputs | Domain review and executable schema checks |
| **Domain & Application** | Invariants, typed values, capabilities, ports, and problems | DDD/DRY review, contract mutation gates, and QA |
| **Contracts & SDK** | OpenAPI, AsyncAPI, Schema, JSON-LD, OKF, examples, and generated clients | Deterministic generation, parity checks, and QA |
| **Delivery & Persistence** | HTTP/SSE, CLI, SQLite/files, OCI, recovery, and performance | Native/OCI tests, fault evidence, and QA |
| **Design & Accessibility** | Tokens and later media UI, keyboard, screen reader, touch, TV remote, ADHD/AuDHD | Approved design contract, design review, accessibility evidence, and QA |
| **Provider Patterns** | Neutral manifests, recipes, and conformance fixtures | B5/B6 gate; no compatibility claim or provider-specific code early |
| **Security** | Access, secrets, limits, container permissions, and cryptography | Threat evidence, private disclosure where needed, and security review |

---

## 4. Development Workflow

### Getting Started
1. **Fork and clone** the repository:
   ```bash
   git clone https://github.com/<your-username>/Fasti.git
   cd Fasti
   ```
2. **Create a topic branch**:
   ```bash
   git checkout -b feat/my-new-feature
   ```
3. **Verify the environment**:
   ```bash
   # Rust workspace check
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --locked -- -D warnings
   cargo test --workspace --locked

   # TypeScript / Node check
   pnpm install --frozen-lockfile
   pnpm format:check
   pnpm typecheck
   pnpm test

   # Repository truth and portability checks
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
We follow the Conventional Commits format:
```
<type>(<scope>): <short summary in imperative mood>

[optional longer body explaining context, rationale, and tradeoffs]

Signed-off-by: Jane Developer <jane@example.com>
```
*Types:* `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.  
*Scopes:* `domain`, `application`, `contracts`, `store`, `api`, `cli`, `sdk`, `tokens`, `design`, `docs`, `infra`.

---

## 5. Code Quality & Standards

### Rust Code (`crates/*`, `apps/fastid`)
* Format code with `cargo fmt --all`.
* Ensure `cargo clippy --workspace --all-targets --locked -- -D warnings` passes cleanly.
* Document public types and traits with clear docstrings and usage examples.
* Keep domain policy out of adapters and preserve one semantic owner for invariants, capabilities, public types, permissions, and problems.
* Maintain error transparency: use typed errors for libraries and avoid bare `.unwrap()` in production codepaths.

### TypeScript (`packages/*`)
* Keep strict typing enabled and consume generated contract types once B1 establishes them.
* Do not redefine domain, permission, error, retry, idempotency, or offline behavior in a client package.
* Format code with Prettier and verify with `pnpm format:check`, `pnpm typecheck`, and `pnpm test`.

### Rendered UI and UX
* Do not add a placeholder web application before B4.
* Follow [`brand/DESIGN.md`](brand/DESIGN.md) and the approved media-first interaction contract.
* Every rendered change requires design review and QA, including keyboard, screen reader, responsive, 44 px target, reduced-motion, focus-return, and ADHD/AuDHD state-continuity evidence.

---

## 6. Submitting a Pull Request

1. Link the GitHub Discussion where the problem, bounded context, scope, and gates were aligned.
2. Push your branch to your fork:
   ```bash
   git push origin feat/my-new-feature
   ```
3. Open a Pull Request against `release` unless the linked Discussion names a different integration branch. Repository automation does not publish a release from the pull request.
4. Complete the provided [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md), documenting:
   * The problem being solved.
   * Behavioral & data-model impact.
   * Test evidence (unit, integration, or manual verification).
   * Accessibility and security considerations.
5. Run every gate named by the linked Discussion. QA is mandatory; rendered UI/UX also requires design review.
6. Ensure all CI checks are green.
7. Address reviewer feedback constructively.

Thank you for helping Fasti keep media records trustworthy, portable, and provider-neutral.
