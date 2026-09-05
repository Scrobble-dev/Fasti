# Changelog

All notable changes to the Fasti project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

- Initial project governance, AGPL-3.0 licensing, and DCO 1.1 sign-off workflow.
- Strict Rust, retained TypeScript, documentation, workflow-policy, and OCI verification paths.
- Brand guidelines and design system specifications adhering to W3C Design Tokens Community Group (DTCG 2025.10).
- Constitution, glossary, capability ledger, contract ownership, UAT ownership, and Definition of Done.
- Health-only daemon surface and explicit nonzero guards for planned B3 CLI operations.
- Executable, loopback-only B1 conformance fixture with governed HTTP and SSE behavior.
- Deterministically generated TypeScript contract SDK and focused local integration-author guide.
- GitHub issue forms, PR review checklist, CODEOWNERS, and non-publishing CI/security workflows.
- B2 application ports and local SQLite/filesystem adapters for node initialization, first-client enrollment, credential lifecycle, profile selection, evidence upload, provider-neutral records, observations, review state, idempotent receipts, replay, and bounded receipt streaming.
- Regression coverage for consumed bootstrap proofs, explicit profile-bound credential authentication, ambiguous active grants, ungranted profiles, and cross-workspace grant rejection.
- Retained B1 daemon, OCI image, contract pack, source snapshot, and raw one-second performance observations with verifier-owned integrity checks.
- Review-candidate web and desktop presentation surfaces backed by the retained contract SDK and explicit host capabilities.
- B8b non-publishing release-readiness evidence: per-architecture checksums, CycloneDX SBOM (Rust and npm), an in-toto/SLSA-shaped provenance statement, a `cargo-deny` final security review, a manual rollback runbook, and mechanical release-notes extraction, gated behind a fail-closed `cargo xtask test milestone --body B8b`.
- `cargo-deny` (`deny.toml`) license, advisory, and source policy for the main workspace and the isolated Tauri benchmark shell.
- Exact TrailBase `v0.33.5` native and OCI development packaging, private lifecycle operations, full-depot backup and restore, hermetic account/social/TOTP conformance, combined resource enforcement, and a test-only `v0.33.4` adjacent upgrade and rollback fixture.
- Checked domain/application models for named clients, personal access tokens, consent revisions, bounded inventories, and one-time issuance results. These are an internal [C2 foundation](docs/plans/fasti-access-c2-foundation.md), not callable token or client-administration operations.
- Contributor-only [C3 signing qualification](qualification/access-c3-signing/README.md) with a separate locked test package and dedicated CI, so its checks can run from a checkout without the prior temporary probe. This does not approve production signing or recovery.
- Contributors can reproduce the retained [C3 framing qualification](qualification/access-c3-framing/README.md) from a separate locked package and CI matrix, including malformed-input and interrupted-I/O checks. This does not approve production encryption or recovery.

### Changed

- Reset product and architecture copy to the identity-first boundary: Fasti records; players play.
- Marked the current repository as a development baseline with no supported public install.
- Made the OCI image daemon-and-CLI only, non-root, lockfile-bound, and free of web-build fallbacks.
- Required a linked Discussion and scope alignment before implementation or a pull request.
- Pinned external workflow actions and normal OCI base images to immutable revisions.
- Made runner handoff snapshots reject source mutation during copy.
- Replaced generic agent routing with phase, contract, offline, security, performance, accessibility, and traceability rules.
- Expressed architecture gates through concrete ownership and dependency rules across contributor surfaces.
- Separated finalized public problem output from staged B2 runtime failures so internal fail-closed paths can report the correct safe state without changing the B1 OpenAPI, AsyncAPI, JSON Schema, JSON-LD, CLI, or SDK contract.
- Updated repository status and security documentation to distinguish implemented B2 review code from production activation and milestone completion.
- Replaced hypothetical physical-device B1 qualification with same-attempt x86_64 and aarch64 cgroup-v2 envelope evidence while preserving the governed memory, CPU, and timing budgets.
- Made unavailable workbench actions fail closed and removed sample media state from product surfaces.
- Kept TrailBase account and OAuth routes loopback-only because the pinned release accepts unsafe protocol-relative redirects; the exact limitation and recovery action are part of Access B conformance evidence.
- Recorded merged ordinary-browser C1 delivery across the status guides while keeping packaged Tauri authentication and public-release support unclaimed.
- Retained the exact licence text and a version-specific licence exception for the already-locked `webpki-root-certs 1.0.9` dependency; no dependency version changed.

### Fixed

- Preserved the contributor-only [C3 KDF runner](qualification/access-c3-kdf/README.md), with bounded pipe handling, owned-child cleanup and strict cold-sample validation. Its isolated CI checks and recorded local measurement do not enable runtime cryptography or recovery.
- Kept Account and security confirmations visible during retries, protected pending first-run actions, and corrected mobile notice and choice layout. A dedicated browser CI check guards the delayed-confirmation layout and fixture interaction-latency limits; see the [Access verification guide](docs/plans/fasti-access-parallel-regressions.md).
- Released data-root locks when their final kernel owner drops, even while a child temporarily retains an inherited descriptor. Failed identity initialization also releases its acquired lock without hiding the original error.
- Prevented consumed enrollment proofs from panicking when the kernel returns `bootstrap_closed`.
- Prevented ambiguous or cross-workspace credential grants from panicking when authentication fails closed.
- Recovered the capability policy source after a malformed formatting-only write; the recovery changed no behavior.
- Made the local developer launcher track and stop only the process groups it starts.
- Canonicalized unknown web routes to the workbench root.
- Rejected wrong-kind identifiers during typed deserialization and used the existing zeroization dependency for owned secret cleanup.
- Resolved two Nuvio application-reference links so warning-denied Rust documentation builds pass.
- Updated the existing schema URI resolver and documentation query parser to patched versions, with regression checks through their actual consumers. The existing image-parser patch and audit policy remain unchanged.
- Kept documentation builds working with symlinked output directories and made loading-state tests wait for intercepted requests before continuing them.

### Removed

- Placeholder player, replication, connector, and provider-keyed projection packages.
- The false `POST /api/v1/events` committed receipt and mirrored SDK submission method.
- Automated GHCR pushes, release attestations, and GitHub Release creation before B8 readiness.
