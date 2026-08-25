# Truthful browser harness QA evidence

Checked: 2026-08-25

## Scope and disposition

This report covers the local Fasti browser harness at `http://127.0.0.1:5173/`.
The harness consumes the generated `system.health` SDK binding. It adds no
product capability, API shape, desktop behavior, domain state, or release claim.

The fixed Vite proxy from `/api` to `127.0.0.1:8420` is local evidence tooling.
Playwright uses isolated port `4173` and a bounded health stub on `18422`. It
refuses to reuse occupied ports, so it cannot silently test another worktree.
Custom domains, `.internal`, certificate authorities, container ports, Tauri
ports, and runtime listener settings remain outside this change.

The harness removes the earlier mock catalogue, review actions, provider screens,
token controls, overlays, stock imagery, and client-owned media state. Those
surfaces cannot return until their bounded contexts and generated contracts are
active.

## Automated evidence

Run from a clean checkout:

```bash
pnpm install --frozen-lockfile
pnpm format:check
pnpm typecheck
pnpm test
pnpm test:ui
```

The 17 Playwright tests use the installed Chrome channel locally and a pinned
Playwright Chromium build in CI. Isolated generated-contract fixtures cover
healthy, invalid, and network-unavailable responses. A bounded loopback stub
also proves the Vite proxy path. The native and OCI smoke gates own live-daemon
health proof. The UI suite retains light and dark screenshots at 320, 768, and
1440 CSS pixels. It also checks:

- zero Axe violations in each tested state;
- one H1 and a descriptive document title;
- no horizontal overflow;
- 44 by 44 CSS pixel minimum controls, which is a Fasti product rule rather than
  a general WCAG claim;
- skip-link focus, keyboard order, next-action theme labels, persistence, and
  retry recovery, including restored focus and superseded-request isolation;
- saved-theme colors before the application module loads;
- system dark mode when reading local theme storage is unavailable, theme
  changes when writing it is unavailable, and both toggle directions;
- 200% text enlargement and the WCAG text-spacing override;
- reduced-motion removal of the loading animation;
- forced-colors status and control visibility in Chromium;
- no request to a third-party origin;
- absence of unauthorised catalogue, review, and connection destinations.

## AskTog interaction principles

| Principle | Evidence or disposition |
| --- | --- |
| Aesthetics | The editorial field-guide tokens, typography, rules, and whitespace form one restrained visual system. |
| Anticipation | Each state includes the status and the next safe action. Recovery does not require a documentation search. |
| Autonomy | The user can reverse the theme choice. The harness performs no data write and has no trapping flow. |
| Color | Status uses icon, heading, and text. Color is never the only signal. |
| Consistency | Native buttons, one focus treatment, shared tokens, and stable placement work across viewports and themes. |
| Defaults | The first visit follows the system color preference. An explicit choice then persists. |
| Discoverability | Both controls are visible and use action labels. There are no hidden menus or gesture-only actions. |
| Efficiency of the user | The page makes one bounded health request. The healthy path requires no action. |
| Explorable interfaces | The reversible theme preference is safe to explore. Unsupported or destructive actions are absent. |
| Fitts's Law | The two controls meet the 44 CSS pixel product target and remain isolated from adjacent targets. |
| Human-interface objects | Native button semantics preserve platform keyboard, pointer, and accessibility behavior. |
| Latency reduction | Loading feedback appears immediately. The request times out after three seconds and does not retry automatically. |
| Learnability | One task, concrete nouns, visible status, and one recovery action require no learned navigation model. |
| Metaphors | The surface uses literal service and response terms. It does not invent a dashboard or inbox metaphor. |
| Protect the user's work | The harness is read-only. Retry aborts the old request before starting another request. |
| Readability | A bounded reading column, short paragraphs, clear headings, and tested text spacing preserve scanning. |
| Simplicity | Only the active health capability is present. Complexity is removed, not hidden. |
| State tracking | Loading, healthy, invalid-response, and unavailable states are explicit. The theme state persists locally. |
| Visible interfaces | Primary state and actions stay in the normal document flow. No overlay obscures context or focus. |

## Gestalt grouping

| Principle | Evidence |
| --- | --- |
| Proximity | State icon, heading, explanation, details, and recovery action are grouped by task. |
| Similarity | Buttons share shape, type, focus, and target rules. Status rows share one definition-list pattern. |
| Common region | Header, state band, details list, and scope note use boundaries only where the relationship changes. |
| Continuity | One aligned reading column and horizontal rules provide a stable top-to-bottom path. |
| Figure and ground | Semantic surface, text, border, action, verified, and attention tokens preserve separation in both themes. |
| Pragnanz | The simplest valid interpretation is also the visible one: this page reports one local service status. |

## Nielsen's ten heuristics

| Heuristic | Evidence |
| --- | --- |
| 1. Visibility of system status | Loading, available, invalid-response, and unavailable states are explicit and announced. |
| 2. Match with the real world | Copy names the local service, health response, repository command, and unavailable later capabilities. |
| 3. User control and freedom | Theme selection is reversible. Retry is explicit. There is no trapping overlay or data mutation. |
| 4. Consistency and standards | Native controls, Fasti tokens, one focus treatment, and stable state placement follow platform conventions. |
| 5. Error prevention | Only the generated health operation is called. Unsupported actions are absent. Automatic retries cannot duplicate work. |
| 6. Recognition rather than recall | The next action and exact recovery command remain visible in the error state. |
| 7. Flexibility and efficiency | Keyboard, pointer, touch-size controls, system theme, and stored theme work without alternate workflows. |
| 8. Aesthetic and minimalist design | One task and one status replace the dense mock workbench. |
| 9. Recognize, diagnose, and recover | Plain-language states distinguish no response from an invalid contract response and give safe recovery. |
| 10. Help and documentation | The recovery command and scope boundary are next to the affected state; README holds the full local workflow. |

Related Nielsen guidance is applied through plain error language, recognition
over recall, stable targets, minimal irrelevant content, and direct feedback. A
heuristic review does not replace usability testing with representative users.

## Relevant IxDF research topics

| Topic | Evidence or limit |
| --- | --- |
| Cognitive load | One task, one primary status, short state-specific copy, and no false navigation reduce working-memory demand. |
| Affordances and signifiers | Visible native buttons, action labels, pointer cursor, and focus treatment show what can be operated. |
| Feedback | Loading and result states close the action-feedback loop without relying on color. |
| Mental models | The UI reflects the real generated health boundary. It does not teach users that catalogue or review features exist. |
| Visual hierarchy | Brand, H1, current state, evidence fields, and scope note have a deliberate descending emphasis. |
| Progressive disclosure | Recovery controls appear only when recovery is necessary. Product capability is not hidden behind progressive disclosure. |
| Accessibility and inclusion | Automated checks cover code-detectable issues. Representative disabled-user testing remains required for a product release. |

## Architecture, contracts, performance, and delivery

| Rule | Evidence or disposition |
| --- | --- |
| DDD | The surface presents only the `system.health` bounded capability and keeps transport failures in a local view model. |
| DRY | The browser uses the generated SDK and health validator. It does not duplicate a health DTO or parser. |
| Modular and reusable | App orchestration, status presentation, design tokens, and generated transport remain separate existing workspace packages. |
| Ponytail / YAGNI | No router, global state library, client database, settings model, product navigation, or speculative capability is added. |
| Standard library and platform first | `AbortController`, `fetch`, native buttons, CSS media queries, and `localStorage` cover the required behavior. |
| Investigate | Browser failure was fixed at the shared SDK root: native `fetch` is bound to its platform receiver. A regression test covers the default transport. |
| Engineering blast radius | The functional path is limited to generated health SDK -> app orchestration -> status component. Runtime listener ownership is unchanged. |
| Developer ergonomics | README and `AGENTS.md` provide exact commands. Playwright uses isolated port `4173` to avoid concurrent-worktree collisions. |
| OpenAPI | No API is added. The existing generated OpenAPI health contract remains authoritative. |
| AsyncAPI | Not applicable: the harness adds no event or stream operation. |
| JSON-LD | Not applicable: the harness adds no linked-data entity or vocabulary. |
| Offline behavior | Browser tests make no third-party request. The local service path remains loopback-only in this harness. |
| Request resources | One request, one abort controller, one three-second timeout, and no automatic retry bound work. Retry and unmount abort stale work. |
| Bundle evidence | Production build: 3.03 kB HTML, 5.03 kB CSS, and 141.84 kB JavaScript; JavaScript is 35.48 kB gzip. |
| Memory claim | Cleanup paths are present, but one browser snapshot cannot prove zero leaks. Repeated packaged navigation remains a release gate. |

## WCAG 2.2 Level AA implementation map

This is scoped implementation evidence, not a declaration of whole-product or
legal conformity. Axe catches only a subset of accessibility defects. The table
accounts for every WCAG 2.2 Level A and AA success criterion; obsolete criterion
4.1.1 is not part of WCAG 2.2.

| Success criteria | Disposition and evidence |
| --- | --- |
| 1.1.1 | The brand mark is decorative because adjacent text supplies the name. State icons are decorative because text supplies the status. |
| 1.2.1-1.2.5 | Not applicable: there is no audio or video. |
| 1.3.1, 1.3.2 | Header, main, section, headings, paragraphs, definition list, and source order preserve structure and sequence. |
| 1.3.3 | Instructions do not depend on shape, position, sound, or sensory characteristics. |
| 1.3.4 | The fluid layout has no orientation lock. |
| 1.3.5 | Not applicable: there is no user-information input. |
| 1.4.1 | Icon, heading, and text duplicate every color-coded state. |
| 1.4.2 | Not applicable: there is no audio. |
| 1.4.3 | Axe checks text contrast in healthy and error states for both themes. |
| 1.4.4 | Automated 200% text enlargement retains content and operation. |
| 1.4.5 | The decorative mark is not required to convey text; the adjacent brand name is real text. |
| 1.4.10 | The 320 CSS pixel tests find no horizontal content overflow. |
| 1.4.11 | Axe and visual inspection cover state icons, boundaries, controls, and focus treatment in both themes. |
| 1.4.12 | The prescribed line, paragraph, letter, and word spacing override retains content and operation without horizontal overflow. |
| 1.4.13 | Not applicable: hover and focus do not reveal extra content. |
| 2.1.1, 2.1.2 | Native controls and the skip link work by keyboard; the page has no keyboard trap. |
| 2.1.4 | Not applicable: there are no character-key shortcuts. |
| 2.2.1, 2.2.2 | There is no user time limit. The bounded loading animation ends within the three-second request timeout. |
| 2.3.1 | No content flashes. |
| 2.4.1 | A first-focus skip link moves focus to main content. |
| 2.4.2 | The document title is `Local service status · Fasti`. |
| 2.4.3 | Keyboard tests verify the predictable source-order focus path. |
| 2.4.4 | The only link is `Skip to main content`; its purpose is explicit. |
| 2.4.5 | Not applicable: this is one local page, not a set of pages. |
| 2.4.6 | One H1 and state-specific H2 labels describe the content and current condition. |
| 2.4.7 | A visible three-pixel focus outline with two-pixel offset is defined for interactive elements. |
| 2.4.11 | No author overlay or sticky content can entirely obscure focused content. |
| 2.5.1 | Not applicable: there are no multipoint or path gestures. |
| 2.5.2 | Native click activation completes on release and supports cancellation before release. |
| 2.5.3 | Each visible action label is also its accessible name. |
| 2.5.4 | Not applicable: no operation uses device or user motion. |
| 2.5.7 | Not applicable: no operation requires dragging. |
| 2.5.8 | Every target is at least 44 by 44 CSS pixels, above the 24 CSS pixel WCAG minimum. |
| 3.1.1 | The document declares English. |
| 3.1.2 | Not applicable: there is no passage in another language. |
| 3.2.1 | Focus alone changes no context. |
| 3.2.2 | Not applicable: there is no form input that changes context. |
| 3.2.3, 3.2.4 | Not applicable to this single page; identification of its repeated controls is consistent within the page. |
| 3.2.6 | Not applicable: no repeated help mechanism exists in a set of pages. Context recovery stays beside the error. |
| 3.3.1-3.3.4 | Not applicable: there is no user input or legal, financial, or data submission. Service failure guidance is still explicit. |
| 3.3.7 | Not applicable: the harness collects no information. |
| 3.3.8 | Not applicable: the harness has no authentication step. |
| 4.1.2 | Native buttons provide name, role, and state. Axe reports no violations. |
| 4.1.3 | Loading and healthy messages use `status`; failures use `alert`; live output is atomic without nested live regions. |

The loading animation also stops for `prefers-reduced-motion`, which exceeds the
Level AA scope because WCAG 2.3.3 Animation from Interactions is Level AAA and
the spinner is not interaction-triggered.

## EN 301 549 evidence

Published EN 301 549 V3.2.1 is the formal baseline used here. Its web clause 9
maps to WCAG 2.1, not WCAG 2.2. The current V4.1.0 approval-vote draft adds the
WCAG 2.2 web requirements, including 9.2.4.11 Focus Not Obscured and 9.2.5.8
Target Size. The draft is used only as forward evidence and is not represented as
a published or harmonised conformity baseline.

| EN surface | Evidence or disposition |
| --- | --- |
| V3.2.1 clause 9 web | The applicable WCAG 2.1 A/AA clauses are covered by the WCAG map above. Chrome, keyboard, Axe, reflow, contrast, semantics, status, and text-spacing evidence is retained. |
| V4.1.0 draft clause 9 | The additional WCAG 2.2 requirements are checked independently, including focus not obscured and 24 CSS pixel target size. |
| Clause 10 non-web documents | Not applicable: the application delivers no non-web document. This repository report is engineering evidence, not application content. |
| Clause 11 non-web software | Not assessed: the reviewed surface is a web page. Tauri and packaged software remain separate product bodies. |
| Clause 12 documentation and support | README provides the developer start, URL, scope, and recovery path. Public product support is not claimed before its milestone. |
| Clause 13 relay and emergency services | Not applicable to this local media-record service. |

## Manual evidence still required before a product release

- Orca and packaged-WebKit screen-reader checks.
- Windows forced-colors and platform high-contrast checks.
- TV remote and packaged touch-device traversal when those targets exist.
- Repeated packaged navigation and memory sampling.
- Usability sessions with representative disabled and neurodivergent users.

These items do not block this private diagnostic harness. They remain mandatory
when their product surfaces and packaging bodies become active.

## Primary references

- [AskTog First Principles of Interaction Design](https://asktog.com/atc/principles-of-interaction-design/comment-page-1/)
- [Nielsen Norman Group: 10 Usability Heuristics](https://www.nngroup.com/articles/ten-usability-heuristics/)
- [IxDF: Cognitive Load](https://www.interaction-design.org/literature/topics/cognitive-load)
- [IxDF: Affordances and Design](https://www.interaction-design.org/literature/article/affordances-and-design)
- [IxDF: Visual Hierarchy](https://www.interaction-design.org/literature/topics/visual-hierarchy)
- [W3C WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [W3C Understanding Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html)
- [ETSI EN 301 549 V3.2.1](https://www.etsi.org/deliver/etsi_en/301500_301599/301549/03.02.01_60/en_301549v030201p.pdf)
- [ETSI EN 301 549 V4.1.0 approval-vote draft](https://www.etsi.org/deliver/etsi_en/301500_301599/301549/04.01.00_30/en_301549v040100va.pdf)
