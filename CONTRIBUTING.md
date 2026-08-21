# Contributing to Fasti

Thank you for your interest in contributing to Fasti!

Fasti is an open-source, self-hosted-first media chronicle and player. We believe personal media history should remain durable, correctable, and owned by the individual. We welcome contributions from developers, designers, archivists, and writers of all experience levels.

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

## 3. Contribution Lanes

We organize contributions into explicit lanes to set expectations on review requirements:

| Lane | Scope | Review Requirement |
|---|---|---|
| **Documentation & Guides** | Typos, architectural explanations, deployment tutorials, API documentation | 1 Maintainer review |
| **Test Fixtures** | Sample media activity, import test cases, sync chaos fixtures | 1 Domain review |
| **Importers & Connectors** | Adding or improving integrations (Plex, Trakt, Jellyfin, Floppy, etc.) | 1 Integration maintainer |
| **UI & Accessibility** | Web client components, keyboard navigation, screen reader support, design tokens | 1 UI / Accessibility maintainer |
| **Core & Sync Engine** | Event ledger, SQLite persistence, replica sync protocol, idempotency | 2 Core maintainers + RFC if modifying invariants |
| **Security & Auth** | Authentication, token scoping, container permissions, cryptography | 1 Security maintainer |

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
   cargo check --workspace
   cargo test --workspace

   # TypeScript / Node check
   pnpm install
   pnpm test
   pnpm lint
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
*Scopes:* `core`, `activity`, `store`, `sync`, `api`, `player`, `web`, `desktop`, `tokens`, `sdk`, `cli`.

---

## 5. Code Quality & Standards

### Rust Code (`crates/*`, `apps/fastid`)
* Format code with `cargo fmt --all`.
* Ensure `cargo clippy --workspace --all-targets -- -D warnings` passes cleanly.
* Document public types and traits with clear docstrings and usage examples.
* Maintain error transparency: use `thiserror` for library crates and avoid bare `.unwrap()` in production codepaths.

### TypeScript / Frontend (`packages/*`, `apps/web`, `apps/desktop`)
* Strictly typed: Avoid `any`. Use runtime schema validation (`zod` or generated schema types) at API boundaries.
* Accessibility first: Ensure interactive controls satisfy WCAG 2.2 AA and our 44px touch target baseline.
* Format code with Prettier and verify with ESLint (`pnpm lint`).

---

## 6. Submitting a Pull Request

1. Push your branch to your fork:
   ```bash
   git push origin feat/my-new-feature
   ```
2. Open a Pull Request against `dev` (or `release` for release promotions) on `Scrobble-dev/Fasti`.
3. Complete the provided [Pull Request Template](.github/PULL_REQUEST_TEMPLATE.md), documenting:
   * The problem being solved.
   * Behavioral & data-model impact.
   * Test evidence (unit, integration, or manual verification).
   * Accessibility and security considerations.
4. Ensure all CI checks are green.
5. Address reviewer feedback constructively.

Thank you for helping make Fasti the best home for personal media chronicles!
