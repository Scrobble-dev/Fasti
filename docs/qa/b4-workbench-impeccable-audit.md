# B4 Workbench Impeccable audit

Date: 2026-08-27

Surface: local Workbench at `/` and service diagnostic at `/status`

Mode: Operate

Status: review evidence, not a B4 completion or WCAG conformance claim

## Audit health score

| #         |                Dimension |     Score | Key finding                                                                                                                        |
| --------- | -----------------------: | --------: | ---------------------------------------------------------------------------------------------------------------------------------- |
| 1         |            Accessibility |       3/4 | Automated Axe scans pass across the Workbench viewport/theme matrix. Manual screen-reader and final keyboard evidence remain open. |
| 2         |              Performance |       3/4 | The production build is bounded and the health poll runs only on `/status`; the collapsing rail still animates `width`.            |
| 3         |        Responsive design |       4/4 | 320, 768, and 1440 px Light and Dark checks pass with no horizontal overflow and no visible undersized button, input, or select.   |
| 4         |                  Theming |       4/4 | Fasti tokens drive both themes. The matrix found and retired the dark desktop sidebar-label contrast defect.                       |
| 5         | Implementation integrity |       3/4 | The restored media Workbench is product-specific and truthful. The detector found two verified craft warnings.                     |
| **Total** |                          | **17/20** | **Good — address the bounded evidence and craft findings before release.**                                                         |

## Implementation integrity verdict

**Pass with two bounded warnings.** The interface expresses Fasti's media
record, local-first, and provider-neutral model. It uses the protected Fasti
tokens and Tabler components. The browser record path calls the generated SDK
with a real scoped bearer. Unsupported mutations do not change mock state.

The Impeccable detector reported:

1. `media-detail-view.svelte`: a four-pixel provider-banner side accent.
2. `nav-sidebar.svelte`: a `width` transition on the collapsing rail.

Both are real. Neither blocks the current read journey. They need a bounded
polish pass because the first is a prohibited generic visual tell and the
second performs layout work during the rail transition.

Disposition: the same review slice removed the width animation and replaced the
one-sided accent with a neutral one-pixel verified-state border. The responsive
matrix must pass again before these findings are closed.

## Executive summary

- Score: **17/20 (Good)**.
- Findings: **P0 0, P1 1, P2 2, P3 0**.
- The Workbench is restored at `/`; the health diagnostic is separate at
  `/status`.
- Light and Dark Workbench checks pass at 320, 768, and 1440 px. Each run checks
  reflow, 44 px controls, and Axe violations.
- Browser authentication uses a memory-only credential. Planned sign-in methods
  remain visible and unavailable until an access-owned backend exists.

## Findings

### [P1] Manual assistive-technology evidence is incomplete

- **Location:** Workbench, authentication dialog, and service status journey.
- **Category:** Accessibility.
- **Impact:** Automated checks cannot prove announcements, reading order,
  disabled-control context, or desktop webview interoperability for an actual
  screen-reader user.
- **Standard:** WCAG 2.2 AA; EN 301 549 Clauses 9, 11, and 12.
- **Recommendation:** Complete a documented keyboard pass and an Orca test on
  Linux. Keep NVDA and VoiceOver as explicit external evidence unless those
  platforms are available. Do not claim full conformance from Axe alone.
- **Suggested command:** `$impeccable audit` after the manual evidence is added.

### [P2] The collapsing rail animates layout width

- **Location:** `packages/ui/src/nav-sidebar.svelte`.
- **Category:** Performance and accessibility.
- **Impact:** Repeated collapse/expand actions cause layout recalculation and the
  motion has no component-level reduced-motion override.
- **Standard:** WCAG 2.2 2.3.3 is advisory for interaction motion; this is also a
  performance craft defect.
- **Recommendation:** Remove the width animation unless a transform-based Tabler
  composition can preserve layout, focus, and hit targets without extra state.
- **Suggested command:** `$impeccable optimize`.

### [P2] Provider banner uses a generic side-tab accent

- **Location:** `packages/ui/src/media-detail-view.svelte`.
- **Category:** Implementation integrity.
- **Impact:** The thick one-sided accent conflicts with the protected flat,
  archival visual language and reads as a generic generated-UI pattern.
- **Recommendation:** Use the existing neutral border and verified-state icon or
  text treatment. Do not add a replacement ornament.
- **Suggested command:** `$impeccable polish`.

## Positive findings

- Product composition and status diagnostics have separate routes and titles.
- The browser clears the retired stored credential and never writes the active
  bearer to browser storage.
- Unavailable actions stay visible, disabled, and named instead of opening fake
  modals or mutating client-only state.
- The dark desktop contrast failure was found by the real matrix and fixed at
  the shared navigation label.
- Build and Svelte checks complete with zero diagnostics.
- The current production bundle is 389.58 kB JavaScript (92.13 kB gzip) and
  615.38 kB CSS (79.22 kB gzip). These are measurements, not release budgets.

## Recommended actions

1. **[P1] `$impeccable audit`:** Record the manual keyboard and available
   screen-reader evidence.
2. **[P2] `$impeccable optimize`:** Remove the rail's layout-property animation.
3. **[P2] `$impeccable polish`:** Replace the provider side-tab accent with the
   established flat verified-state treatment.
4. Re-run the Workbench matrix once after the bounded fixes.
