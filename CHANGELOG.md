# Changelog

All notable changes to the Fasti project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
* Initial project governance, AGPL-3.0 licensing, and DCO 1.1 sign-off workflow.
* Strict Rust, retained TypeScript, documentation, workflow-policy, and OCI verification paths.
* Brand guidelines and design system specifications adhering to W3C Design Tokens Community Group (DTCG 2025.10).
* Constitution, glossary, capability ledger, contract ownership, UAT ownership, and Definition of Done.
* Health-only daemon surface and explicit nonzero guards for planned B3 CLI operations.
* Executable, loopback-only B1 conformance fixture with governed HTTP and SSE behavior.
* Deterministically generated TypeScript contract SDK and focused local integration-author guide.
* GitHub issue forms, PR review checklist, CODEOWNERS, and non-publishing CI/security workflows.

### Changed

* Reset product and architecture copy to the identity-first boundary: Fasti records; players play.
* Marked the current repository as a development baseline with no supported public install.
* Made the OCI image daemon-and-CLI only, non-root, lockfile-bound, and free of web-build fallbacks.
* Required a linked Discussion and scope alignment before implementation or a pull request.
* Pinned external workflow actions and normal OCI base images to immutable revisions.
* Made runner handoff snapshots reject source mutation during copy.
* Replaced generic agent routing with phase, contract, offline, security, performance, accessibility, and traceability rules.
* Expressed architecture gates through concrete ownership and dependency rules across contributor surfaces.

### Removed

* Placeholder player, replication, connector, provider-keyed projection, web, shared-presentation, and desktop packages.
* The false `POST /api/v1/events` committed receipt and mirrored SDK submission method.
* Automated GHCR pushes, release attestations, and GitHub Release creation before B8 readiness.
