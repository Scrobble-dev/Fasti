# Fasti External Harness Context Save

**Status:** Dated Handoff Snapshot
**Date:** 2026-08-25  
**Audience:** Antigravity, Claude, Codex, or any autonomous coding harness picking up Fasti in a new conversation  
**Repository:** `Scrobble-dev/Fasti`  
**Integration Branch:** `dev`  
**Active B4 PR:** #44 (`codex/b4-durable-bootstrap`)  
**Canonical Evergreen Reference:** [`FASTI_MASTER_INTEGRATOR_HANDOFF.md`](FASTI_MASTER_INTEGRATOR_HANDOFF.md)

---

## 1. Executive Summary & Status

Fasti Milestone **B4 (Durable Bootstrap & Fasti Workbench)** is under review in PR #44. The implementation candidate provides:

1. **Full Fasti Workbench Application** in `@fasti/ui` (Svelte 5 runes) and `apps/web` (Vite container).
2. **Multi-Domain Tracking Interfaces**: Chronicle timeline, Library catalogue (_Movies, TV, Anime, Books, Games_), Media Details with claims provenance, Review Inbox (Reconciliation), Up Next & Calendar schedule, Connections availability, and Settings & Studio. Controls without host commands remain disabled.
3. **Desktop Host & Container Runtime**: Tauri v2 shell with OS Keyring integration in `apps/desktop/src-tauri` and rootless OCI container (`localhost/fasti:test`) running at **1.35 MiB RSS**.
4. **Verification Gates**: use `PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr` and `cargo xtask contract verify --locked` at the current head. A dated result does not establish merge readiness for a later head.

---

## 2. Pull Request & Branch Topology (2026-08-25)

```
┌────────────────────────────────────────────────────────────────────────┐
│                              GitHub Remote                             │
│                           Scrobble-dev/Fasti                           │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
       ┌────────────────────────────┼────────────────────────────┐
       ▼                            ▼                            ▼
┌──────────────┐             ┌──────────────┐             ┌──────────────┐
│  Branch: dev │             │ PR #20: dev  │             │ PR #44: B4   │
│  (Integration│             │      to      │             │  Workbench   │
│   Head: fd6b)│             │   release    │             │  Bootstrap   │
└──────┬───────┘             └──────────────┘             └──────┬───────┘
       │                                                         │
       ├────────────────────────────┐                            │
       ▼                            ▼                            │
┌──────────────┐             ┌──────────────┐                    │
│ Branch: B2   │             │ PR #35: B3   │                    │
│  Namespace   │             │  Portability │                    │
│  Definition  │             │   (Merged)   │                    │
└──────────────┘             └──────────────┘                    │
                                                                 │
                                                                 ▼
                                                  ┌──────────────────────────────┐
                                                  │ @fasti/ui & apps/web & dev.sh│
                                                  │ Svelte 5 + Tauri v2 + Podman │
                                                  └──────────────────────────────┘
```

### Active Branches & Worktrees:

1. **`dev`**:
   - Integration branch containing B0, B1, and merged B3 portability PR #35.
2. **`codex/b4-durable-bootstrap` (PR #44)**:
   - Contains the full B4 implementation, Svelte 5 `@fasti/ui` suite, `apps/web`, `apps/desktop`, `scripts/dev.sh`, and OCI containerfile.
3. **`codex/b2-namespace-definition` (`.codex/worktrees/b2-namespace-definition`)**:
   - Contains B2 identity namespace registry and governance; ready to open once B4 lands.

---

## 3. Milestone Disposition Matrix (B0–B8 + Goldilocks)

| Milestone      | Scope & Description                                           | Status        | Evidence / Location                                   |
| :------------- | :------------------------------------------------------------ | :------------ | :---------------------------------------------------- |
| **B0**         | Foundation, domain vocabulary, invariant enforcement          | **COMPLETE**  | `crates/fasti-domain`                                 |
| **B1**         | Software contracts, OpenAPI 3.1, AsyncAPI 3.1, JSON-LD, SDK   | **COMPLETE**  | `target/fasti-receipts/b1-contract-verification.json` |
| **B2**         | Namespace definition registration & identity governance       | **READY**     | Branch `codex/b2-namespace-definition`                |
| **B3**         | Portability workspace manifest & JCS verification             | **MERGED**    | PR #35 in `dev`                                       |
| **B4**         | Durable bootstrap, Fasti Workbench UI, Tauri v2, Podman       | **IN REVIEW** | PR #44 (`codex/b4-durable-bootstrap`)                 |
| **B5**         | Multi-provider metadata ingest (TMDB, TVDB, MAL, Kitsu)       | **NEXT**      | Scheduled after B4                                    |
| **B6**         | Ingestion webhooks & desktop observer (Plex, Jellyfin, MPRIS) | **PLANNED**   | Unavailable controls remain disabled                  |
| **B7**         | NuvioTV 2-way sync engine & monotonic cursor outbox           | **PLANNED**   | Outbox contracts defined                              |
| **B8**         | Release candidate stabilization & public gateway              | **PLANNED**   | Release gate scripts ready                            |
| **Goldilocks** | Custom fields, WebAuthn, PAT tokens, Floppy/Yamtrack import   | **BLUEPRINT** | `fasti_batteries_included_goldilocks_plan.md`         |

---

## 4. Design System & Accessibility Governance

- **Standard**: `brand/DESIGN.md` (_The Modern Annal / Living Marginalia_)
- **Typography**: _Newsreader_ (Serif display), _Atkinson Hyperlegible_ (Sans UI), _IBM Plex Mono_ (Evidence)
- **Color Contrast**: 100% WCAG 2.2 AAA certified (`#181716` on `#FFFDF8` = **15.6:1**)
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
