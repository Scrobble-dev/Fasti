# Fasti Project Governance

This document outlines the governance structure, decision-making framework, and maintainer responsibilities for the Fasti project.

---

## 1. Guiding Principles

1. **Sovereignty & Longevity:** Fasti is built to outlive any single provider, device, or maintainer. Data integrity, exportability, and local-first durability always take priority over convenience features.
2. **Constitutional Neutrality with Scrobble.dev:** Fasti is an open-source reference implementation of the open activity language developed at [Scrobble.dev](https://scrobble.dev). Fasti maintainers do not hold unilateral authority over Scrobble.dev specifications.
3. **Transparency in the Open:** Architectural decisions, roadmap milestones, and policy discussions occur publicly in GitHub Discussions, Issues, Pull Requests, and Requests for Comments (RFCs), except for embargoed security or private personal information.
4. **Truth Before Promotion:** Planned capability, compatibility, performance, and release claims require executable evidence. No workflow publishes a public artifact before B8 and an explicit release action.
5. **Open Collaboration:** Discussion-first scope alignment is required before implementation or a pull request. It is not legal approval and does not add a CLA gate.

---

## 2. Maintainer Roles & Responsibilities

```
Project Steward
  -> bounded-context maintainers and reviewers
  -> security response role
  -> B8 release gate owner when distribution begins
  -> contributors and community participants
```

### Roles

* **Project Steward:** Responsible for strategic coherence, brand stewardship, dispute arbitration, and overall project continuity.
* **Bounded-Context Maintainers:** Earned roles for a named domain, application, contract, persistence, delivery, design, or provider-conformance boundary. Ownership follows the constitution rather than the retired scaffold crate names.
* **Security Response Role:** Receives private reports, coordinates disclosure, and requests the evidence required by the affected boundary. The role may be held by the Project Steward until delegated publicly.
* **B8 Release Gate Owner:** A role activated when supported distribution begins. B0-B7 have no authority to publish public binaries, packages, images, attestations, or GitHub Releases.
* **Reviewers & Contributors:** Any community member participating in Discussions, fixtures, documentation, design, accessibility, issue triage, code review, or scoped pull requests.

---

## 3. Decision-Making & RFC Process

### Discussion Before Implementation
Every change begins in a GitHub Discussion unless it is an embargoed security fix. The Discussion names the problem, bounded context, scope, contract surfaces, performance or accessibility gates, and whether an RFC is required. Implementation and pull requests begin only after that scope is aligned.

### Routine Changes (Standard PRs)
Small truth corrections, documentation updates, fixtures, and bounded implementation changes follow the linked Discussion and the repository's required checks. A maintainer with responsibility for the affected boundary decides whether evidence is sufficient to merge.

### Structural & Architectural Changes (RFC Required)
A formal **Request for Comments (RFC)** and explicit written decision from the Project Steward plus the maintainers responsible for affected boundaries is required for changes that:
1. Alter the persistent SQLite event schema or migration engine.
2. Modify idempotency, durability, correction, evidence, or future replication semantics.
3. Introduce breaking changes to the Fasti HTTP REST API or SDK contracts.
4. Modify authentication, authorization scopes, or cryptographic boundaries.
5. Alter default data retention, privacy, or telemetry policies.
6. Move domain meaning across bounded contexts or add an external runtime dependency to a domain or application boundary.

### RFC Lifecycle
1. **Discussion:** Author and community align the problem, boundary, outcomes, and evidence in GitHub Discussions.
2. **Proposal:** Author creates a documentation PR in `docs/rfcs/0000-feature-title.md` and links the Discussion.
3. **Consensus / Decision:**
   * **Approved:** Merged into `docs/rfcs/` with an assigned number and marked `Accepted`.
   * **Postponed / Rejected:** Closed with a clear, written rationale preserved in the PR history.
4. **Implementation:** Implementation PRs cross-reference the approved RFC.

---

## 4. Conflict Resolution & Successor Plan

In the rare event of an unresolvable technical disagreement among maintainers:
1. The decision is elevated to an open RFC deliberation with an explicit comparison of tradeoffs against the Guiding Principles.
2. The Project Steward serves as the tie-breaking arbiter for the 0.x release cycle.
3. If a maintainer steps down or becomes inactive for 6+ months, existing maintainers can vote by simple majority to transition their permissions to an active contributor.
4. Before B8 activates public distribution, the release RFC must document signing-key custody, repository recovery, domain continuity, and removal of single-person failure modes. B0 does not claim those controls already exist.
