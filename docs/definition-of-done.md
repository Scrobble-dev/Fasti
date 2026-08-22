# Definition of Done

A Fasti body is complete only when all applicable evidence below is current and bound to the reviewed source tree.

## Required for every body

- The body begins from its accepted predecessor and lands independently green.
- Domain meaning follows [the constitution](constitution.md); adapters do not own policy.
- DDD boundaries and semantic DRY ownership are explicit.
- Implemented behavior, README, roadmap, issues, examples, packages, and workflows agree.
- Rust formatting, lint, build, and test gates plus retained JavaScript formatting, strict type, build, and test gates pass from a clean checkout with lockfiles enforced.
- Native and OCI smoke paths fail loudly; no fallback turns a failed build into an empty artifact.
- No public image, binary, package, attestation, or GitHub Release is published before B8 and an explicit release action.
- Mandatory QA completes with a report and regression evidence for each defect fixed.
- Rendered UI or UX changes also complete design review with screenshots and accessibility evidence.

## Contracts

From B1 onward, every implemented capability has a registry entry and an explicit disposition for domain/application, HTTP/OpenAPI, SSE/AsyncAPI, CLI, JSON Schema, JSON-LD/OKF, SDK, knowledge, package smoke, and UI.

`cargo xtask contract verify` must generate twice identically, find no checked-in drift, validate OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1 expansion, OKF and references, compile generated SDKs, run black-box client tests, and prove deliberate drift fails.

## Local, recovery, and performance evidence

- Applicable core journeys pass with the network denied.
- Failed, interrupted, duplicated, stale, conflicted, oversized, storage-full, and restart paths preserve explicit recoverable state.
- Successful receipts survive the body-specific durability fault model.
- Memory, CPU, startup, latency, throughput, storage, and artifact-size claims include the exact artifact, environment, repetitions, and named hardware profile.
- 192 MiB full-process memory is an absolute ceiling; lower 64/96/160 MiB targets remain visible even when the ceiling passes.

## User capability

The primary action is obvious without instructional dashboard copy. Status is persistent where losing it would break trust. Users can safely defer and resume ambiguous work. Keyboard, screen reader, touch, TV-remote where applicable, contrast, reduced motion, focus return, and ADHD/AuDHD state continuity are acceptance evidence.
