## Description

<!-- Provide a brief, clear summary of what this change accomplishes and the user problem it solves. -->

## Required Discussion

<!-- Link the GitHub Discussion where the problem, bounded context, scope, and required gates were aligned before implementation. This is scope alignment, not legal approval. -->

Discussion:

## Target Branch

- [ ] `release` — Standard integration target unless the linked Discussion names another branch. This PR cannot publish a public image, package, attestation, binary, or GitHub Release.

## Category of Change

- [ ] `feat`: New capability or adapter
- [ ] `fix`: Bug fix
- [ ] `docs`: Documentation addition or clarification
- [ ] `refactor`: Internal cleanup without behavioral change
- [ ] `test`: New fixtures or test coverage
- [ ] `ci`: Workflow, release, or packaging update

## Invariant & Impact Checklist

- [ ] **DCO Sign-off:** All commits are signed off with `git commit -s` (`Signed-off-by: Name <email>`).
- [ ] **Discussion-first scope:** The linked Discussion predates implementation and the change stays inside its bounded context and accepted scope.
- [ ] **DDD / DRY:** Domain meaning has one owner; adapters and generated surfaces do not redefine it.
- [ ] **Data Model Invariants:** If changing schema or persistence, an ADR/RFC has been approved and backward compatibility / migration tests are included.
- [ ] **Scrobble.dev Boundary:** This PR does not unilaterally modify normative Scrobble.dev specification definitions.
- [ ] **Contract parity:** Every applicable OpenAPI, AsyncAPI, Schema, JSON-LD, OKF, CLI, SDK, permission, error, example, and knowledge surface agrees or is explicitly `N/A` / later body.
- [ ] **Tests and QA:** Automated evidence is included and mandatory QA has passed.
- [ ] **Accessibility and design (if rendered):** Design review passed with keyboard, screen reader, responsive, 44 px target, reduced-motion, focus-return, and ADHD/AuDHD state-continuity evidence.
- [ ] **Performance and portability:** Applicable memory, artifact, native, OCI, network-denied, packaging, and recovery evidence is attached without estimated pass claims.
- [ ] **Security:** No unauthenticated endpoints, link-local SSRF vulnerabilities, or unredacted log leaks.
- [ ] **Publishing posture:** Repository workflows remain non-publishing before B8 and an explicit release action.

## Test Evidence / Screenshots

<!-- Paste artifact-bound test outputs, QA receipts, performance receipts, or rendered screenshots here. Redact credentials and personal media data. -->
