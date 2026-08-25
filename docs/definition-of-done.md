# Definition of Done

A Fasti body is complete only when all applicable evidence below is current and bound to the reviewed source tree.

## Required for every body

- The body begins from its accepted predecessor and lands independently green.
- Domain meaning follows [the constitution](constitution.md); adapters do not own policy.
- Meaning ownership and dependency direction are explicit.
- Implemented behavior, README, roadmap, issues, examples, packages, and workflows agree.
- Rust formatting, lint, build, and test gates plus retained JavaScript formatting, strict type, build, and test gates pass from a clean checkout with lockfiles enforced.
- Native and OCI smoke paths fail loudly; no fallback turns a failed build into an empty artifact.
- No public image, binary, package, attestation, or GitHub Release is published before B8 and an explicit release action.
- Mandatory QA completes with a report and regression evidence for each defect fixed.
- User-facing rendered UI or UX changes must satisfy the user capability and interaction governance below. Include screenshots and reflow evidence at 320px, 768px, and 1440px in light and dark themes.
- For a headless change, mark visual, touch-target, screenshot, and UI assistive-technology evidence `Not applicable` and state why.

A local pre-body diagnostic harness may exercise an already implemented capability before the next body starts. It must be private, unpackaged, contract-backed, and explicit about unavailable later capabilities. Its checks can retire interface defects, but the harness does not satisfy predecessor acceptance, activate a capability, or count as body completion evidence.

## Contracts

From B1 onward, every implemented capability has a registry entry and an explicit disposition for domain/application, HTTP/OpenAPI, SSE/AsyncAPI, CLI, JSON Schema, JSON-LD/OKF, SDK, knowledge, package smoke, and UI.

`cargo xtask contract verify` must generate twice identically, find no checked-in drift, validate OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1 expansion, OKF and references, compile generated SDKs, run black-box client tests, and prove deliberate drift fails.

## Local, recovery, and performance evidence

- Applicable core journeys pass with the network denied.
- Failed, interrupted, duplicated, stale, conflicted, oversized, storage-full, and restart paths preserve explicit recoverable state.
- Successful receipts survive the body-specific durability fault model.
- Memory, CPU, startup, latency, throughput, storage, and artifact-size claims include the exact artifact, enforced environment, architecture, repetitions, and named hardware when a claim is device-specific.
- 192 MiB full-process memory is an absolute ceiling; lower 64/96/160 MiB targets remain visible even when the ceiling passes.

## User capability and interaction governance

These requirements apply to user-facing UI and UX. The primary action is obvious without instructional dashboard copy. Status is persistent where losing it would break trust. Users can safely defer and resume ambiguous work ("Resolve later").

Acceptance evidence requires verified compliance against:

1. **Tabler-first component ladder**: Use upstream `@tabler/core` and `@tabler/icons`, then Fasti token-skinned Tabler, then composed Tabler components. Use custom CSS or Svelte only when Tabler has no equivalent, and record the reason. Do not use generic SaaS card walls or continuous decorative animation.
2. **AskTog Interaction Principles**: Anticipation, Fitts's Law (44px hitboxes), latency reduction, data loss protection, state continuity (no element shifting under cursor).
3. **Gestalt Grouping**: Proximity, similarity, common region, continuity, closure, figure/ground, and deliberate focal points.
4. **All 10 Nielsen Norman Heuristics**: System status visibility, real-world match, user control and undo, consistency, error prevention, recognition over recall, flexibility and shortcuts, minimalist design, user-visible error recognition, diagnosis and recovery, and contextual help.
5. **IxDF Cognitive & Ergonomic Research**: Cognitive load reduction via progressive disclosure, motor precision touch targets, halation-free night mode (`#11110F`).
6. **WCAG 2.2 Level AA**: Record each applicable success criterion. Include focus appearance and non-obscured focus, AA contrast, single-pointer alternatives for drag actions, and accessible authentication without cognitive tests. Automated scans supplement manual checks; they do not prove conformance.
7. **EN 301 549 European Standard**: Record each applicable requirement across Clause 9 (Web), Clause 10 (Non-Web Documents), Clause 11 (Software), and Clause 12 (Documentation). Manually verify keyboard access and assistive-technology interoperability on each supported target.
8. **Neurodivergent (ADHD / AuDHD) Ergonomics**: Zero gamification streaks, zero vanity scores, persistent status when losing it would break trust, and safe resumable workflows.

API errors use RFC 9457 Problem Details where the API contract requires them. UI acceptance separately proves that a user can recognize the error, understand the next action, recover, and resume the journey.
