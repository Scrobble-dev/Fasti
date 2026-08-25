## Description

<!-- Provide a brief, clear summary of what this change accomplishes and the user problem it solves. -->

## Required Discussion

<!-- Link the GitHub Discussion where the problem, bounded context, scope, and required gates were aligned before implementation. This is scope alignment, not legal approval. -->

Discussion:

## Relationships

Related issues:

Related pull requests:

Accepted plans or reviews:

Relevant upstream work:

Supersedes or follows:

## Target Branch

- [ ] `dev` — Standard integration target. This PR cannot publish a public image, package, attestation, binary, or GitHub Release.
- [ ] `release` — Reserved strictly for release candidate stabilization.

## Category of Change

- [ ] `feat`: New capability or adapter
- [ ] `fix`: Bug fix
- [ ] `docs`: Documentation addition or clarification
- [ ] `refactor`: Internal cleanup without behavioral change
- [ ] `test`: New fixtures or test coverage
- [ ] `ci`: Workflow, release, or packaging update

## Invariant & Impact Checklist

- [ ] **DCO Sign-off:** All commits are signed off with `git commit -s` (`Signed-off-by: Name <email>`).
- [ ] **Scope alignment:** The PR references an open issue, accepted RFC, or Discussion (Tier 3/4 domain/identity changes require prior alignment).
- [ ] **Meaning ownership:** Domain meaning has one owner; adapters and generated surfaces do not redefine it, and dependencies point toward the governed rules.
- [ ] **Identity & Data Invariants:** Fasti records; players play. Provider IDs are claims (`xid_*`), observations (`obs_*`) are immutable evidence, and local record keys (`rec_*`) remain stable.
- [ ] **Scrobble.dev Boundary:** This PR does not unilaterally modify normative Scrobble.dev specification definitions.
- [ ] **Contract parity:** Every applicable OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF 0.2, CLI, and SDK surface agrees or is explicitly `N/A`.
- [ ] **Tests and QA:** Automated evidence is included (`cargo xtask test pr` / `cargo xtask contract verify --locked` / tests) and mandatory QA has passed.
- [ ] **Accessibility and design (if rendered):** Design review passed with keyboard, screen reader, responsive, 44 px target, reduced-motion, focus-return, and ADHD/AuDHD state-continuity evidence.
- [ ] **Performance and portability:** Memory budget respected (64 MiB idle, 192 MiB ceiling). Zero runtime telemetry or phone-home code.
- [ ] **Security:** No unauthenticated endpoints, link-local SSRF vulnerabilities, or unredacted log leaks.
- [ ] **Publishing posture:** Repository workflows remain non-publishing before B8 and an explicit release action.

## Test Evidence / Screenshots

<!-- Paste artifact-bound test outputs, QA receipts, performance receipts, or rendered screenshots here. Redact credentials and personal media data. -->
