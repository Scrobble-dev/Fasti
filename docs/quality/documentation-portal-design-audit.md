# Documentation portal design audit

Status: implementation verified locally; pull request and exact-head CI review are pending.

This record covers the Fasti documentation portal at
https://fasti.scrobble.dev. It does not cover the Fasti Workbench or claim
product release readiness.

## Scope

The audit reviewed these public routes:

- Start
- Choose a path
- Status
- Deployment planner
- Contract inventory
- Accessibility
- Search

The baseline pass covered 23 route, viewport, and theme states. It used desktop,
tablet, and mobile widths. It also checked light and dark themes.

## Fixed findings

| Finding | Result | Commit |
|---|---|---|
| Status data moved the page after load. | The table is present in the first render. Successful loads measure zero cumulative layout shift. | 3b76de37 |
| Search started as an empty region. | The page reserves the field space and gives the Pagefind field a searchbox role and name. | f43e9c06 |
| Three Docusaurus controls were smaller than 44 by 44 CSS pixels. | The breadcrumb home link, mobile table-of-contents control, and code copy button meet the target. | 34403e2e |
| The planner command lost its boundary in dark mode. | The existing border token defines the command surface. | 33b370eb |
| Status lost its table when registry data failed. | The table stays visible with unknown counts and an error action. | 642d4721 |
| Search had no recovery state when Pagefind failed. | A labelled fallback, failure action, and no-JavaScript action are present. | f1f83a9b |

## Verification

The focused documentation checks passed:

- Docusaurus production build and Pagefind index
- documentation TypeScript check
- Tabler-first UI policy
- documentation navigation style tests
- zero automated axe violations on the changed Status and Search routes
- zero cumulative layout shift on successful Status and Search loads at 768
  and 375 CSS pixels
- search success, asset-failure, and no-JavaScript states
- Status registry-failure state
- dark planner command boundary

The Status error state has a measured cumulative layout shift of 0.065. The
error adds required recovery text. The value remains below 0.1, and the table
does not move out of the task flow.

## Review results

The Impeccable detector found no listed design anti-pattern in the changed
files.

The independent GSD six-pillar review scored the first confirmation build
20/24. It gave full scores for copy, visuals, and typography. Its two
experience-design findings were the missing Search failure state and the Status
error-state table removal. Both findings are fixed in the commits above.

The GSD review also recommends a later token cleanup for inherited Docusaurus
blue shades and off-scale spacing values. That cleanup is not required to fix
the reported regressions. It must use a separate visual review because it can
change the full documentation system.

## Evidence limits

Automated axe checks do not prove WCAG 2.2 Level AA or EN 301 549 conformance.
This record does not claim packaged NVDA, VoiceOver, or Orca evidence. A named
human review is still required before a conformance claim.

This change does not add or change an API. OpenAPI, AsyncAPI, JSON Schema, and
JSON-LD files do not need an update.

Codacy, CodeRabbit, and pull request checks are pending until the branch is
published.
