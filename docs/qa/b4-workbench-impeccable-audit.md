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

### AskTog interaction principles

| Principle | Workbench evidence |
| --- | --- |
| Anticipation | Each unavailable or rejected state keeps its page context and gives the next safe action. |
| Autonomy | Theme, density, navigation, and provider selection are reversible. Browser credentials remain tab-scoped. |
| Color | Text, icons, borders, and accessible state attributes repeat every color-coded meaning. |
| Consistency | Shared Tabler controls, tokens, focus treatment, and 44 pixel targets apply across bounded contexts. |
| Defaults | System theme is the first default. Discover chooses the first configured supported provider only when the user has not selected one. |
| Discoverability | Primary routes, current context, recovery actions, and disabled reasons remain visible in normal flow. |
| Efficiency | Compact navigation, direct route changes, retained selections, and bounded loading avoid repeated setup and recall. |
| Explorable interfaces | Reversible presentation settings can be tried without domain mutation. Unsupported operations do not simulate success. |
| Fitts's Law | The automated shared-target regression and the provider-selector regression verify at least 44 by 44 CSS pixels. |
| Human-interface objects | Native buttons, links, inputs, selects, and tab semantics preserve platform keyboard and assistive behavior. |
| Latency reduction | Loading feedback appears immediately; retries are explicit and duplicate requests are prevented. |
| Learnability | Literal media and service terms, one current route, and local recovery copy reduce hidden rules. |
| Protect the user's work | Origin changes clear origin-bound bearer state. Browser mode cannot write media or provider credentials. |
| Readability | Short sections, descriptive headings, stable regions, and tested text spacing support scanning. |
| State tracking | Loading, empty, unavailable, disconnected, rejected, disabled, and selected states are explicit. |
| Visible interfaces | Product context stays visible during recovery. Modals name their purpose, return focus, and do not replace the shell. |

### Gestalt grouping

| Principle | Workbench evidence |
| --- | --- |
| Proximity | Page headings, status, actions, and evidence are grouped by task. |
| Similarity | Navigation items, settings choices, state notices, and media rows use consistent visual and semantic patterns. |
| Common region | The primary rail, current-view toolbar, settings navigation, editor, and content surface have stable boundaries. |
| Continuity | A consistent rail-to-toolbar-to-content reading path survives mobile and tablet reflow. |
| Closure | Disabled sections remain complete enough to explain purpose, state, and activation requirement. |
| Figure and ground | Light, Dark, forced-colors, selected, focus, and notice states retain tested separation. |
| Pragnanz | Each page presents one primary task and one current route without ornamental competing actions. |

### Nielsen's ten heuristics

| Heuristic | Workbench evidence |
| --- | --- |
| 1. Visibility of system status | Route, authentication, loading, service, provider, empty, and unavailable states stay visible. |
| 2. Match with the real world | Copy uses Record, provider, media, service URL, credential, and diagnostic summary for the data shown. |
| 3. User control and freedom | Reversible settings, explicit retry, dismissible authentication, retained product context, and browser-history sync preserve control. |
| 4. Consistency and standards | Native semantics, Tabler components, shared tokens, one Settings composition, and one route predicate prevent parallel patterns. |
| 5. Error prevention | Contract parsing rejects invalid health; unsupported actions are disabled; provider state is authoritative; origin changes clear bearer state. |
| 6. Recognition rather than recall | Current route, selected settings area, selected provider, capability limits, and recovery actions remain visible. |
| 7. Flexibility and efficiency | Keyboard tabs, compact rail, persistent non-secret preferences, and remembered explicit provider selection reduce repeated work. |
| 8. Aesthetic and minimalist design | Flat editorial grouping, restrained borders, no gradients, no ornamental motion, and no fake dashboards keep focus on tasks. |
| 9. Recognize, diagnose, and recover from errors | Invalid credentials, invalid health contracts, unavailable APIs, and unconfigured providers have distinct plain-language recovery. |
| 10. Help and documentation | Setup requirements, browser limits, service URL behavior, provider documentation links, and the diagnostic route are adjacent to need. |

### Relevant IxDF research topics

| Topic | Workbench evidence or limit |
| --- | --- |
| Cognitive load | Stable page structure, explicit current context, progressive disclosure, and action-first recovery reduce working-memory demand. |
| Affordances and signifiers | Native controls, labels, focus, selected state, and disabled explanations show available actions. |
| Feedback | Loading, success, rejected, empty, and unavailable states close each action-feedback loop without color-only meaning. |
| Mental models | Browser client configuration and trusted-node configuration are separated; summaries do not claim complete Record detail. |
| Motor precision | Shared 44 pixel targets exceed WCAG 2.2's 24 pixel minimum and reduce precise pointing demand. |
| Visual hierarchy | Brand, route title, state, content, and supporting evidence have deliberate descending emphasis. |
| Progressive disclosure | Recovery and setup details appear when needed while the primary product structure remains visible. |
| Neurodiversity | Stable layouts, no streaks or vanity scores, persistent status, short task paths, and resumable selections support ADHD and AuDHD use. |

This evidence does not replace testing with representative users. It also does
not prove packaged Tauri or platform screen-reader behavior.

## WCAG 2.2 Level A and AA implementation map

This is scoped implementation evidence, not a declaration of whole-product or
legal conformity. Axe detects only a subset of accessibility defects. Obsolete
criterion 4.1.1 is not part of WCAG 2.2.

| Success criteria | Disposition and evidence |
| --- | --- |
| 1.1.1 | Icons are decorative where adjacent text supplies meaning. Media images use text alternatives; projected summaries omit untrusted poster URLs. |
| 1.2.1-1.2.5 | Not applicable: the reviewed Workbench provides no audio or video content. |
| 1.3.1, 1.3.2 | Landmarks, headings, lists, tables, forms, tabs, labels, and source order preserve structure and sequence. |
| 1.3.3 | Instructions do not depend only on shape, position, sound, or sensory characteristics. |
| 1.3.4 | The fluid layout has no orientation lock. |
| 1.3.5 | Credential and service fields expose programmatic labels; release evidence must still verify input-purpose metadata in packaged WebKit. |
| 1.4.1, 1.4.2 | State is not color-only. The interface emits no audio. |
| 1.4.3, 1.4.5 | Automated and manual checks cover text contrast. Required text is rendered as text, not an image. |
| 1.4.4, 1.4.10, 1.4.12 | Automated checks cover 200% text, 320 pixel reflow, and WCAG text spacing without lost content or horizontal page overflow. |
| 1.4.11 | Automated and rendered checks cover controls, boundaries, selected state, focus, and semantic action foregrounds. |
| 1.4.13 | Hover and focus do not reveal persistent content that cannot be dismissed or reached. |
| 2.1.1, 2.1.2, 2.1.4 | Native controls and the modal tab pattern work by keyboard, with no trap or character-key shortcut. |
| 2.2.1, 2.2.2 | The Workbench sets no user time limit. Reduced motion removes non-essential animation. |
| 2.3.1 | No content flashes. |
| 2.4.1-2.4.7 | Skip navigation, descriptive titles and headings, predictable focus, explicit link purpose, one current route, and visible focus are tested. |
| 2.4.11 | Stable layout and normal-flow status regions prevent author content from entirely hiding focus. |
| 2.5.1-2.5.4, 2.5.7 | No multipoint, path, motion, or drag-only operation exists; native controls activate on release and visible labels are accessible names. |
| 2.5.8 | Shared-control and provider-selector tests verify 44 pixel targets, above the 24 CSS pixel minimum. |
| 3.1.1, 3.1.2 | The document declares English and contains no unmarked foreign-language passage. |
| 3.2.1-3.2.4 | Focus and input do not unexpectedly change context; repeated navigation and settings choices are identified consistently. |
| 3.2.6 | Contextual help stays in a predictable settings or recovery location. |
| 3.3.1-3.3.4 | Form errors identify the affected input and give correction text. Credential and endpoint changes require explicit actions and do not perform media-domain writes. |
| 3.3.7 | Previously entered non-secret settings and explicit provider selection are retained where reuse is safe. |
| 3.3.8 | Authentication uses a pasteable bearer value and imposes no cognitive-function test. |
| 4.1.2 | Native controls, tabs, current-route, selected, pressed, expanded, and disabled states expose name, role, and value. Axe reports no violations in tested states. |
| 4.1.3 | Loading, success, and failure messages use programmatic status or alert semantics where immediate announcement is required. |

## EN 301 549 evidence boundary

Published EN 301 549 V3.2.1 is the formal baseline used here. Its web clause 9
maps to WCAG 2.1. The V4.1.0 approval-vote draft adds the WCAG 2.2 web
requirements. The draft is forward evidence only and is not represented as a
published or harmonised conformity baseline.

| EN surface | Evidence or disposition |
| --- | --- |
| V3.2.1 clause 9 web | The applicable WCAG 2.1 A and AA criteria are included in the map above. Chrome, keyboard, Axe, reflow, contrast, semantics, forced-colors, and text-spacing evidence is retained. |
| V4.1.0 draft clause 9 | Focus Not Obscured and Target Size are checked independently with the WCAG 2.2 additions. |
| Clause 10 non-web documents | Not applicable to the reviewed Workbench; exported diagnostic JSON is data, not user documentation. |
| Clause 11 non-web software | Not yet evidenced. Packaged Tauri interoperability with Orca, NVDA, and VoiceOver remains a release gate. |
| Clause 12 documentation and support | README and local docs record startup, routes, capability limits, recovery, and accessibility evidence boundaries. Public support-service conformity is not claimed. |
| Clause 13 relay and emergency services | Not applicable to this local media-record application. |

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
