# B4 Workbench Impeccable audit

Checked: 2026-08-28

Surface: local Workbench at `/` and service diagnostic at `/status`

Status: review evidence, not a B4 completion, WCAG conformance, or EN 301 549
conformity claim

## Audit health score

| Dimension                |     Score | Evidence or limit                                                                                                                                           |
| ------------------------ | --------: | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Accessibility            |       3/4 | Axe, keyboard, focus, contrast, reflow, target-size, and rendered checks pass. A manual screen-reader pass remains open.                                    |
| Performance              |       4/4 | Progress uses transform animation. Reduced motion removes non-essential motion. Browser tests use two workers to remain deterministic on constrained hosts. |
| Responsive design        |       4/4 | Light and Dark checks at 320, 768, and 1440 CSS pixels pass without horizontal overflow or undersized shared controls.                                      |
| Theming                  |       4/4 | Fasti tokens drive Light and Dark themes. Text and control contrast checks pass in the tested matrix.                                                       |
| Implementation integrity |       4/4 | The preserved Workbench uses real host data, truthful disabled states, shared Tabler controls, and no client-owned media mutations.                         |
| **Total**                | **19/20** | **Good. Manual assistive-technology evidence remains required before release.**                                                                             |

## Detector evidence

The required Impeccable detector returned `[]` for every changed UI source at
exact commit `5052bb7b`. The detector declared `DEGRADED` because its optional
parser modules were unavailable. The empty result is not standalone proof.
Manual inspection, rendered review, Playwright, Axe, target-size, reflow, and
contrast evidence support it.

Resolved craft findings include:

- progress animation now uses `transform: scaleX()` instead of animating width;
- setup and runtime-setting notices use restrained full borders and background
  grouping instead of one-sided attention tabs;
- the navigation rail no longer animates layout width;
- provider verification uses the established flat border and semantic state.

## Product and interaction truth

- `/` renders the preserved media Workbench. `/status` renders the diagnostic.
- The browser reads real Record summaries through the generated SDK. It shows
  at most 500 Records because the current operation has no cursor.
- The browser keeps the active bearer in tab memory. It stores only non-secret
  display preferences and the selected client service URL.
- Unsupported mutations stay visible, disabled, and named. They do not change
  mock or browser-owned domain state.
- Settings that require the trusted host remain disabled in the browser.
- Loading, empty, unavailable, invalid-credential, and disabled states give a
  direct next action. State does not rely on color alone.

## Review lenses

The rendered review applied AskTog interaction principles, Gestalt grouping,
Nielsen's ten heuristics, relevant IxDF topics, WCAG 2.2 Level AA criteria, and
EN 301 549 evidence boundaries. The checks cover system status, user control,
error prevention and recovery, recognition over recall, proximity, similarity,
common region, continuity, figure and ground, cognitive load, affordances,
feedback, focus, semantics, contrast, reflow, target size, and reduced motion.

This evidence does not replace testing with representative users. It also does
not prove packaged Tauri or platform screen-reader behavior.

## Open evidence

- Complete an Orca pass on the packaged Linux interface.
- Record NVDA and VoiceOver evidence when those release platforms are
  available.
- Test packaged forced-colors, touch, TV remote, and repeated-navigation memory
  behavior when those targets enter the release scope.
- Run usability sessions with representative disabled and neurodivergent
  users before a public product claim.

## Contract disposition

This UI work adds no route, event, domain entity, or linked-data term. OpenAPI
governs the existing health, setup, observation, Record, identifier, and
namespace operations. AsyncAPI and JSON-LD are not applicable to browser client
settings or presentation state.
