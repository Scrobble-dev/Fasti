# Fasti Project Governance

This document outlines the governance structure, decision-making framework, and maintainer responsibilities for the Fasti project.

---

## 1. Guiding Principles

1. **Sovereignty & Longevity:** Fasti is built to outlive any single provider, device, or maintainer. Data integrity, exportability, and local-first durability always take priority over convenience features.
2. **Constitutional Neutrality with Scrobble.dev:** Fasti is an open-source reference implementation of the open activity language developed at [Scrobble.dev](https://scrobble.dev). Fasti maintainers do not hold unilateral authority over Scrobble.dev specifications.
3. **Transparency in the Open:** All architectural decisions, roadmap milestones, and policy discussions occur publicly in GitHub Issues, Pull Requests, and Requests for Comments (RFCs).

---

## 2. Maintainer Roles & Responsibilities

```
                      ┌────────────────────────┐
                      │    Project Steward     │
                      │  (Strategic Direction) │
                      └───────────┬────────────┘
                                  │
         ┌────────────────────────┼────────────────────────┐
         │                        │                        │
         ▼                        ▼                        ▼
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│ Core Maintainers │    │Security Maintainer│   │Release Maintainer│
│  (Domain/Store/  │    │  (Vuln Triage &  │    │ (Signing, SBOM,  │
│      Sync)       │    │    Attestation)  │    │   Distribution)  │
└──────────────────┘    └──────────────────┘    └──────────────────┘
         │
         ▼
┌──────────────────┐
│   Contributors   │
│  & Reviewers     │
└──────────────────┘
```

### Roles

* **Project Steward:** Responsible for strategic coherence, brand stewardship, dispute arbitration, and overall project continuity.
* **Core Maintainers:** Maintainers with write and merge access to specific functional areas (`core`, `store`, `sync`, `api`, `ui`, `player`, `connectors`). Core maintainers review PRs, triage issues, and drive milestone delivery.
* **Security Maintainer:** Oversees coordinated vulnerability disclosure, dependency audits, cryptographic boundary reviews, and container hardening.
* **Release Maintainer:** Controls release workflow keys, package registry credentials, artifact signing, SBOM generation, and reproducible build verification.
* **Reviewers & Contributors:** Any community member actively participating in code reviews, issue discussions, and pull requests.

---

## 3. Decision-Making & RFC Process

### Routine Changes (Standard PRs)
Minor bug fixes, documentation additions, connector improvements, and non-breaking UI adjustments require approval from **at least one Maintainer** before merging.

### Structural & Architectural Changes (RFC Required)
A formal **Request for Comments (RFC)** and approval from **at least two Core Maintainers** (plus the Security Maintainer where applicable) is required for changes that:
1. Alter the persistent SQLite event schema or migration engine.
2. Modify the logical synchronization protocol or idempotency contract.
3. Introduce breaking changes to the Fasti HTTP REST API or SDK contracts.
4. Modify authentication, authorization scopes, or cryptographic boundaries.
5. Alter default data retention, privacy, or telemetry policies.
6. Add or modify external runtime dependencies in core crates.

### RFC Lifecycle
1. **Proposal:** Author creates a PR in `docs/rfcs/0000-feature-title.md` using the RFC template.
2. **Discussion:** Community and maintainers review the design, trade-offs, and alternatives in the PR discussion.
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
4. All critical release signing keys, domain names, and repository ownership are managed across at least two authorized maintainers to eliminate single points of failure.
