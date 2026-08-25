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
- Rendered UI or UX changes must complete design review with:
  - Tabler-first component hierarchy compliance: Use Fasti token-skinned `@tabler/core` and `@tabler/icons`; custom Svelte components only when Tabler has zero equivalent, with documented rationale for each exception;
  - Impeccable craft floor verification (CLS = 0, no AI gradients/bubble cards/generic SaaS card walls, 44px min touch targets, no continuous decorative animations);
  - Automated `@axe-core/playwright` accessibility scans with zero violations and visual reflow checks at 320px, 768px, and 1440px in Light and Dark themes (coverage limited to automated tooling; for WCAG 2.2 AA and EN 301 549 full compliance claims, provide documented manual keyboard navigation and screen-reader testing with Orca/NVDA/VoiceOver);
  - Screenshots demonstrating visual conformance across tested viewports.

## Contracts

From B1 onward, every implemented capability has a registry entry and an explicit disposition for domain/application, HTTP/OpenAPI, SSE/AsyncAPI, CLI, JSON Schema, JSON-LD/OKF, SDK, knowledge, package smoke, and UI.

`cargo xtask contract verify` must generate twice identically, find no checked-in drift, validate OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1 expansion, OKF and references, compile generated SDKs, run black-box client tests, and prove deliberate drift fails. API errors must use RFC 9457 Problem Details.

## Local, recovery, and performance evidence

- Applicable core journeys pass with the network denied.
- Failed, interrupted, duplicated, stale, conflicted, oversized, storage-full, and restart paths preserve explicit recoverable state.
- Successful receipts survive the body-specific durability fault model.
- Memory, CPU, startup, latency, throughput, storage, and artifact-size claims include the exact artifact, enforced environment, architecture, repetitions, and named hardware when a claim is device-specific.
- 192 MiB full-process memory is an absolute ceiling; lower 64/96/160 MiB targets remain visible even when the ceiling passes.

## User capability & Interaction Governance

This section applies to user-facing UI/UX capabilities. Visual evidence, touch-target measurements, screenshots, and assistive-technology testing are not required for headless changes.

The primary action is obvious without instructional dashboard copy. Status is persistent where losing it would break trust. Users can safely defer and resume ambiguous work ("Resolve later").

Acceptance evidence requires verified compliance against:
1. **Tabler-First Component Ladder**: Upstream `@tabler/core` & `@tabler/icons` elements used first; custom CSS/Svelte only when no viable alternative exists.
2. **AskTog Interaction Principles**: Anticipation, Fitts's Law (44px hitboxes), latency reduction, data loss protection, state continuity (no element shifting under cursor).
3. **Gestalt Grouping**: Proximity, similarity, common region, continuity, closure, figure/ground, and deliberate focal points.
4. **All 10 Nielsen Norman Heuristics**: System status visibility, real-world match, user control/undo, consistency, error prevention, recognition over recall, flexibility/hotkeys, minimalist design, user-journey error recognition with clear diagnosis and recovery actions, and contextual help.
5. **IxDF Cognitive & Ergonomic Research**: Cognitive load reduction via progressive disclosure, motor precision touch targets, halation-free night mode (`#11110F`).
6. **WCAG 2.2 Level AA Full Matrix**: 3px focus appearance with 2px offset, non-obscured focus, >= 4.5:1 / 7.0:1 contrast, single-pointer alternatives for drag actions, accessible authentication without cognitive tests.
7. **EN 301 549 European Standard**: Full compliance across Clause 9 (Web), Clause 10 (Non-Web Docs), Clause 11 (Software/Desktop Assistive Technology interoperability with Orca/NVDA/VoiceOver), and Clause 12 (Documentation).
8. **Neurodivergent (ADHD / AuDHD) Ergonomics**: Zero gamification streaks, zero vanity scores, persistent status bars (no disappearing toasts), and safe resumable workflows.
