# Accessibility

## Target

The documentation portal targets WCAG 2.2 Level AA and applicable EN 301 549 web
and documentation requirements. Automated checks support this target. They do
not prove complete conformance by themselves.

## Required behavior

- Semantic landmarks and one clear page heading.
- Keyboard access to every task and control.
- Visible focus that is not hidden by sticky content.
- Targets of at least 44 by 44 CSS pixels.
- Text and non-text contrast that meets the documented threshold.
- No color-only meaning.
- Reflow at 320 CSS pixels and at 400 percent zoom.
- Reduced motion and no continuous animation.
- Stable layouts and no movement under active focus.
- Descriptive links, field labels, errors, and recovery actions.

## Cognitive accessibility

Primary paths use short task sequences, persistent state, clear current context,
progressive disclosure, and limited simultaneous calls to action. AuDHD and
screen-reader personas are cross-cutting in `docs/personas.yaml`.

## Evidence boundary

Playwright and axe check automated rules. Manual review covers keyboard order,
screen-reader announcements, zoom, reflow, contrast, motion, link purpose,
Nielsen heuristics, Gestalt grouping, AskTog principles, and applicable EN 301
549 clauses.

Packaged NVDA, VoiceOver, and Orca evidence is not claimed unless a named human
review record exists.

## Report a problem

Use the Fasti issue tracker for a public accessibility defect. Do not include a
credential, private data-root path, or private media history.

Content state: STE-controlled draft.
