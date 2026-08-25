# Milestone B4 UI/UX Testing & Quality Harness Architecture

This document defines the architectural specification for Fasti's UI/UX quality pipeline, activating when Milestone B4 (the Svelte/Tauri UI Workbench) begins.

---

## 1. Quality Pipeline Architecture

```
Svelte / Tauri UI Presentation Boundary
  │
  ├─► Layer 1: Storybook Component & Interaction Harness
  │     ├── Interaction Tests (User clicks, keyboard navigation, form inputs)
  │     ├── Axe Accessibility Automated Checks (`@axe-core/playwright`)
  │     └── Explicit State Permutations (Loading, error, empty, offline, stale)
  │
  ├─► Layer 2: Playwright Cross-Platform User Journeys
  │     ├── Offline & Local-First Kernel Resilience (Direct local `fastid` execution)
  │     ├── Interruption & Crash Recovery Journey (Crash mid-edit -> Resume exact place)
  │     └── In-Context Accessibility Checks (Axe runs on every journey state)
  │
  ├─► Layer 3: Visual & Artifact Regression (Argos CI — Non-Blocking Advisory)
  │     ├── Multi-theme visual snapshots (Dark, high contrast, reduced motion)
  │     ├── Viewport reflow snapshots (Mobile 320px, tablet, desktop 1440px)
  │     └── Schema / Contract / Markdown artifact snapshot diffs
  │
  └─► Layer 4: Performance & Stability Sentinels (Lighthouse CI)
        ├── Layout Stability (CLS = 0)
        ├── Fast Interaction Response (INP < 100ms)
        └── Cognitive Load & A11y Sentinel Scores
```

---

## 2. Storybook State Permutation Matrix

Every user-facing component must define stories for explicit product states rather than optimistic demo paths. Uncertainty and interruption are first-class product states in Fasti.

Per `ROADMAP.md` (B4), Fasti explicitly excludes playback controls, a `Chronicle` navigation item, instructional dashboard copy, and persistent connectivity badges.

| State Dimension | Required Story Permutations | Acceptance Criteria |
|---|---|---|
| **Data Presence** | Empty state, 1 record, 1,000 records (virtualized list) | Clean call to action, stable scroll position, no UI freeze |
| **Resolution Status**| Fully resolved, ambiguous match, un-enriched scrobble | Amber warning badge, explicit manual reconciliation trigger |
| **Local Kernel Status**| Online, daemon unreachable, database locked, sync pending | Contextual failure guidance, zero silent loss, retry trigger |
| **Interaction States**| Default, hover, keyboard focused, active, disabled | Visible 3px high-contrast focus ring, 44px min touch target |
| **Display Constraints**| Long strings (256+ chars), narrow viewport (320px), 200% zoom | Text wraps cleanly or truncates with tooltip, zero horizontal overflow |
| **Accessibility Modes**| Default theme, High Contrast, `prefers-reduced-motion: reduce` | Animations disabled, color contrast >= 7:1 |

---

## 3. Playwright Journey Contracts

Playwright tests verify resilient human journeys rather than shallow DOM assertions.

### Core Journey 1: First-Launch to Unresolved Scrobble
1. Launch app with fresh workspace.
2. Verify empty state guidance for approved media types (Movies, Shows, Music, Books, Games, Podcasts).
3. Ingest raw scrobble observation through local `fastid` API without external metadata lookup.
4. Verify observation is recorded immediately with `rec_*` key and marked `unresolved`.
5. Verify no blocking spinners, fake missing fields, or playback player widgets.

### Core Journey 2: Offline Resilience & Interruption Recovery (No Client Queues)
1. Ingest 5 observations via local daemon.
2. Sever upstream network connection (local `fastid` remains the local-first authority).
3. Edit record tags, list memberships, and manual reconciliation notes directly against local kernel.
4. Simulate process termination / reload.
5. Verify all local edits and unresolved states are restored from SQLite cleanly.
6. Reconnect external network; verify background reconciliation resumes without overwriting user notes.

---

## 4. In-Context Accessibility (Axe-Core)

Automated accessibility audits are integrated directly into Playwright journey steps via `@axe-core/playwright`:

```typescript
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test('Media view has zero WCAG 2.1 AA violations in all states', async ({ page }) => {
  await page.goto('/activity');
  
  // Test empty state
  const accessibilityScanResults = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();
  expect(accessibilityScanResults.violations).toEqual([]);

  // Trigger daemon unreachable state
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('fasti:daemon-offline')));
  const offlineScanResults = await new AxeBuilder({ page }).analyze();
  expect(offlineScanResults.violations).toEqual([]);
});
```

---

## 5. Visual Regression & Snapshot Strategy (Argos CI + Local Pixelmatch)

* **SaaS Tier (Argos CI)**: 5,000 screenshots per month on OSS tier.
* **Non-Blocking Guardrail**: Argos CI runs as an **advisory check on PRs**. Quota exhaustion or SaaS latency must never block local development or CI merges.
* **Deterministic Fallback**: Local Playwright screenshot assertions and artifact diffs run offline in deterministic CI.
* **Scope**:
  * Core approved views (Media Navigation, Poster/Row Grid, Detail View, Ingest Log, Settings, Export/Import).
  * 3 standard breakpoints (360px, 768px, 1280px).
  * Light & Dark themes.
  * Snapshot diffing of generated Markdown / JSON documentation.

---

## 6. Non-Negotiable Boundaries

* **No UI test blocks headless daemon builds**: B1–B3 engine tests and CLI verification remain decoupled from web/Tauri UI harnesses.
* **No client-side retry/mutation queues**: `fastid` and local SQLite own persistence and synchronization. The UI presentation boundary does not invent offline queues (`ROADMAP.md:95`).
* **No flaky cloud browser dependencies**: Playwright runs against standard local headless Chromium/Firefox/WebKit instances on GitHub Actions runners.
* **Zero telemetry in UI code**: The web frontend contains no analytics trackers, session replay scripts, or external font/CDN calls.
