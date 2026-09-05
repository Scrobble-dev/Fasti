# Fasti External Harness Context Save

> Status correction, verified 2026-08-25: this dated context save originally overstated B6 and B7. Current source contains application-level payload converters and in-memory conformance models. Production `fastid` has no Plex or Jellyfin webhook routes, MPRIS D-Bus observer, mDNS discovery, Nuvio pairing endpoint, Nuvio transport, or callers for these converters. Use the master handoff, capability ledger, current source, and live checks for current status.

**Date:** 2026-08-25T08:10:00Z  
**Branch:** `dev`  
**Milestone Disposition:**
- B0 (Ledger & Truth): Complete
- B1 (Hardware Envelopes & Verifiable Receipts): Complete (PR #36 merged into `dev` at `6e18c18a`)
- B2 (Namespace Governance): Branch `codex/b2-namespace-definition` verified against `dev`
- B3 (Portability Archive v1 & Restore): Complete (PR #35 merged into `dev` at `fd6bdd60`)
- B4 (UI / Fasti Workbench): merged into `dev` via PR #58 and PR #59 on 2026-08-26; truthful empty-state baseline, no mock data
- B5 (Metadata Resolution & Claims): Single-pass $O(1)$ space optimization implemented in PR #39
- B6 (Multi-Client Conformance & Ingest): source-neutral archetype fixtures and Plex, Jellyfin/Emby, and Linux MPRIS payload converters exist; production transports are absent
- B7 (Nuvio Deep Integration): application models cover observation, replay, state-delta, and catalog behavior; production pairing, persistence, transport, and Nuvio client work are absent
- B8 (Packaging & Platform Readiness): Clean compilation and zero-network reproducible contract verification

This file is a historical snapshot. Do not use its PR, branch, or next-step statements as current instructions. Verify live source, pull requests, and exact-head evidence first.

---

## 1. Executive Summary & Status

- `crates/fasti-application/src/nuvio.rs`: `NuvioPlaybackSession`, `NuvioOutbox`, `NuvioWatchedState`, `NuvioChangeDelta`, `NuvioStateSyncEngine`, `NuvioCatalogDescriptor`, `NuvioCatalogItem`, `NuvioCatalogProjectionStore`.
- `crates/fasti-application/src/ingest.rs`: `PlexWebhookPayload`, `JellyfinWebhookPayload`, `MprisMediaEvent`.
- `crates/fasti-application/tests/b7_nuvio_observations.rs`: 14 feature-gated conformance tests.
- `crates/fasti-application/tests/b6_ingest_webhooks.rs`: 16 feature-gated converter and conformance tests.

Run these suites explicitly:

```bash
cargo test --locked -p fasti-application --features conformance-fixture --test b6_ingest_webhooks
cargo test --locked -p fasti-application --features conformance-fixture --test b7_nuvio_observations
```
- `docs/architecture/nuvio-integration.md`: Architectural specification and invariant documentation.

---

## 2. Pull Request & Branch Topology (2026-08-26)

`dev` is the integration branch (B0, B1, B3, B4, B5, B6, B7 application-level work). `release` only advances via a deliberate promotion PR (last one: #20) and lags `dev` significantly — always diff against `origin/dev` for current status, not `release`.

---

## 3. Milestone Disposition Matrix (B0–B8 + Goldilocks)

| Milestone      | Scope & Description                                           | Status          | Evidence / Location                                                   |
| :------------- | :------------------------------------------------------------ | :-------------- | :-------------------------------------------------------------------- |
| **B0**         | Foundation, domain vocabulary, invariant enforcement          | **COMPLETE**    | `crates/fasti-domain`                                                 |
| **B1**         | Software contracts, OpenAPI 3.1, AsyncAPI 3.1, JSON-LD, SDK   | **IN PROGRESS** | Contract receipt only; aggregate exact-head evidence was not recorded |
| **B2**         | Namespace definition registration & identity governance       | **READY**       | Branch `codex/b2-namespace-definition`                                |
| **B3**         | Portability workspace manifest & JCS verification             | **MERGED**      | PR #35 in `dev`                                                       |
| **B4**         | Durable bootstrap, Fasti Workbench UI (empty-state), network settings | **MERGED** | PR #58, PR #59 in `dev`                                        |
| **B5**         | Metadata resolution and claims                                | **MERGED**      | PR #39 in `dev`                                                       |
| **B6**         | Ingestion webhooks & desktop observer (Plex, Jellyfin, MPRIS) | **APPLICATION-LEVEL ONLY** | `crates/fasti-application/src/ingest.rs`; no production transports |
| **B7**         | NuvioTV sync engine, cursor outbox, and catalog projections   | **APPLICATION-LEVEL ONLY** | `crates/fasti-application/src/nuvio.rs`; no production pairing/transport |
| **B8**         | Release candidate stabilization & public gateway              | **PLANNED**     | Release gate scripts ready                                            |
| **Goldilocks** | Custom fields, WebAuthn, PAT tokens, Floppy/Yamtrack import   | **BLUEPRINT**   | `fasti_batteries_included_goldilocks_plan.md`                         |

---

## 4. Design System & Accessibility Governance

- **Standard**: `brand/DESIGN.md` (_The Modern Annal / Living Marginalia_)
- **Typography**: _Newsreader_ (Serif display), _Atkinson Hyperlegible_ (Sans UI), _IBM Plex Mono_ (Evidence)
- **Measured Color Contrast**: `#181716` on `#FFFDF8` = **15.6:1**. This measurement is not a whole-product conformance claim.
- **Touch Targets**: Strict 44px minimum touch boundaries with 3px Horological Gold focus rings.
- **Neurodivergent Standards**: Persistent non-toast error headers, zero gamification guilt, safe `Resolve later` deferrals.

---

## 5. Verification Commands for Subsequent Sessions

```bash
# 1. Run the canonical PR verification gate (all suites, Tauri, performance, contracts):
PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr

# 2. Run deterministic contract verification & JS mutation tests:
cargo xtask contract verify --locked

# 3. Check UI diagnostics & typechecking:
pnpm --filter @fasti/ui typecheck && pnpm --filter @fasti/web typecheck

# 4. Launch local development environment (Daemon + Svelte 5 Web Workbench):
./scripts/dev.sh

# 5. Launch in rootless Podman:
./scripts/dev.sh --podman

# 6. Launch native Tauri desktop application:
./scripts/dev.sh --desktop
```

---

## 6. Key Files & Artifact Reference

- **Local-only review artifacts** (not stored in this repository):
  - `fasti_qa_audit.md` — PR gate and mutation test receipts.
  - `fasti_design_review.md` — Visual design and accessibility review.
  - `fasti_claude_outside_voice_review.md` — Independent outside-voice review.
  - `fasti_ecosystem_comparative_analysis.md` — Ecosystem feature matrix.
  - `fasti_batteries_included_goldilocks_plan.md` — Future capability blueprint.
- **Component Suite**:
  - [`packages/ui/src/fasti-workbench.svelte`](../../packages/ui/src/fasti-workbench.svelte) — Master UI layout shell.
  - [`packages/ui/src/chronicle-view.svelte`](../../packages/ui/src/chronicle-view.svelte) — Chronicle timeline feed.
  - [`packages/ui/src/library-view.svelte`](../../packages/ui/src/library-view.svelte) — Media catalogue with search and filters.
  - [`packages/ui/src/media-detail-view.svelte`](../../packages/ui/src/media-detail-view.svelte) — Media details, identity claims, and episode checklists.
  - [`packages/ui/src/reconciliation-view.svelte`](../../packages/ui/src/reconciliation-view.svelte) — Review Inbox and candidate diffs.
  - [`packages/ui/src/settings-view.svelte`](../../packages/ui/src/settings-view.svelte) — Settings and capability availability.
- **Application-level capabilities (not yet production-wired)**:
  - [`crates/fasti-application/src/ingest.rs`](../../crates/fasti-application/src/ingest.rs) — Plex, Jellyfin/Emby, and MPRIS observation converters.
  - [`crates/fasti-application/src/nuvio.rs`](../../crates/fasti-application/src/nuvio.rs) — Nuvio observation ingress, outbox, watched-state sync, and catalog projections.
  - [`crates/fasti-application/tests/b6_ingest_webhooks.rs`](../../crates/fasti-application/tests/b6_ingest_webhooks.rs) and [`b7_nuvio_observations.rs`](../../crates/fasti-application/tests/b7_nuvio_observations.rs) — Conformance evidence.
  - [`docs/architecture/nuvio-integration.md`](../architecture/nuvio-integration.md) — B7 architecture and invariants.
