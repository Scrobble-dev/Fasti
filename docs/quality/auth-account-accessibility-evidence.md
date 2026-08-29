# Account and session UI quality evidence

Status: implementation evidence for the account and browser-session surface.

This record covers browser sign-in, the account dialog, session inventory,
session revocation, and browser-user administration. It does not claim product-
wide accessibility certification. A release audit must also test the native
shell, platform assistive technology, zoom, contrast modes, and translated copy.

## Product truth

| State | User-facing behavior | Evidence |
| --- | --- | --- |
| Browser password session | Available. The server creates, authenticates, lists, and revokes the session. | Rust application, store, API, SDK, and host tests. |
| Current and other sessions | Available. The dialog identifies the current session and gives scoped revoke actions. | Account-dialog end-to-end proof and API integration tests. |
| Session location and device | Not recorded. The product does not infer these values. | The application boundary returns `Not recorded` until a real collection contract exists. |
| Profile switching | Unavailable. The runtime fails closed and does not create a grant or change all active sessions. | Store guard and the unavailable UI state. |
| Passkey, OIDC, and device authorization | Visible but unavailable. Controls state what is missing and do not claim success. | Account-dialog and runtime-contract tests. |
| TOTP, PIN, browser-local profiles, and local OIDC setup | Not present. The UI does not simulate server-owned security state. | Source review and runtime-conformance tests. |

## AskTog interaction principles

| Principle | Evidence |
| --- | --- |
| Anticipation | The dialog loads session state when it opens and keeps pending, success, and failure feedback near the action. |
| Autonomy | A person can close the dialog, keep the current session, revoke one other session, or revoke all other sessions. Focus does not start a destructive action. |
| Consistency | Tabler buttons, forms, alerts, badges, spacing, and icon treatment match the Workbench. |
| Defaults | Session duration uses bounded choices. There is no indefinite duration. |
| Efficiency | The current session is identified in the list. Related account and session actions stay in one dialog. |
| Explorable interfaces | Planned methods remain visible with an unavailable state. They do not become dead links or fake forms. |
| Fitts's law | Interactive targets have at least a 44 by 44 CSS-pixel hit area. Actions stay close to their subject. |
| Latency reduction | Pending labels and disabled repeat actions give immediate feedback while the host request runs. |
| Learnability | Labels use user terms such as `Current session`, `Revoke`, and `Not recorded`. |
| Readability | Headings, short paragraphs, grouped fields, and concise alerts support scanning. |
| Track state | The UI shows the selected method, signed-in user, current session, loading state, error state, and completion state. |

## Gestalt grouping

| Principle | Evidence |
| --- | --- |
| Proximity | Each label, explanation, value, and action stays in one account or session group. |
| Similarity | Repeated sessions use the same row. Available and unavailable methods use consistent status treatment. |
| Common region | The dialog, tab panels, cards, alerts, and bordered rows define clear regions. |
| Continuity | The order is identity, method, session state, then administration. Focus follows this order. |
| Figure and ground | The modal surface separates account work from the Workbench. Focus and disabled states stay visible. |
| Prägnanz | Each row has one subject and one bounded action. The UI does not add speculative setup steps. |

## Nielsen's ten heuristics

| Heuristic | Evidence |
| --- | --- |
| 1. Visibility of system status | Loading, pending, current-session, success, and error states are visible. Dynamic status uses a live region. |
| 2. Match with the real world | Copy says `session`, `password`, `current`, `revoke`, and `not recorded`. It does not present guessed metadata as fact. |
| 3. User control and freedom | The dialog has a close action. A person can cancel destructive confirmation and revoke one session instead of all sessions. |
| 4. Consistency and standards | Native dialog behavior, Tabler components, standard labels, and Workbench patterns are used. |
| 5. Error prevention | Duration is bounded. Repeat submission is blocked while pending. Destructive account actions need explicit confirmation. Unsafe profile switching fails closed. |
| 6. Recognition rather than recall | The UI shows the account, current session, expiry, last activity, and valid actions. A person does not need to remember a session ID. |
| 7. Flexibility and efficiency | Native controls support keyboard operation. Administrator controls do not obstruct the normal session path. |
| 8. Aesthetic and minimalist design | Planned methods have one clear unavailable state. Fake setup forms, generated secrets, and duplicate profile managers are absent. |
| 9. Error recognition and recovery | Errors appear in the affected flow, use plain language, and preserve the current account state and retry action. |
| 10. Help and documentation | The authentication boundary and contract disposition are documented. Planned methods state the missing capability. |

## Relevant IxDF topics

| Topic | Application |
| --- | --- |
| Cognitive load | Method tabs use progressive disclosure. Normal session work is separate from administration. |
| Hick's law | The UI offers only valid duration choices and relevant actions. Planned methods do not expose incomplete option trees. |
| Fitts's law | Targets meet the 44 CSS-pixel minimum and destructive actions stay inside their session row. |
| Error prevention and recovery | Bounded input, pending locks, confirmation, scoped revoke actions, and preserved retry state reduce errors. |
| Recognition over recall | Session facts and status labels stay visible at the decision point. |
| Visual hierarchy | Dialog title, method, section, session identity, metadata, and action follow a consistent hierarchy. |
| Progressive disclosure | Browser-user administration appears only for an administrator. Planned methods show status before setup details. |
| Inclusive design | Native semantics, keyboard access, live status, non-color labels, target size, and concise copy support varied attention, motor, and vision needs. |

## WCAG 2.2 Level AA evidence

| Success criterion | Result in this scope | Evidence |
| --- | --- | --- |
| 1.3.1 Info and Relationships | Covered | Native headings, labels, buttons, dialog semantics, tabs, lists, and grouped controls convey structure. |
| 1.3.2 Meaningful Sequence | Covered | DOM order follows the visible reading and focus order. |
| 1.3.3 Sensory Characteristics | Covered | Instructions use text and state words, not only position, shape, or color. |
| 1.4.3 Contrast (Minimum) | Shared-token evidence | The component uses the established Tabler token surface. Release evidence must include the product-wide contrast report for each theme. |
| 1.4.10 Reflow | Covered in tested viewport scope | Content wraps and uses bounded responsive regions. The Android account proof covers the narrow layout. |
| 1.4.11 Non-text Contrast | Shared-token evidence | Focus, borders, controls, and state treatment use the established control tokens. |
| 1.4.12 Text Spacing | Covered | Fixed text-height containers do not clip content. Controls can grow with text. |
| 2.1.1 Keyboard | Covered | Native buttons, inputs, tabs, and dialog controls are keyboard operable. |
| 2.1.2 No Keyboard Trap | Covered | The native dialog can close. No custom key trap is present. |
| 2.4.3 Focus Order | Covered | Focus follows identity, method, fields, sessions, then administration. |
| 2.4.6 Headings and Labels | Covered | Headings and labels describe purpose and action. |
| 2.4.7 Focus Visible | Covered | The Workbench focus treatment remains active for each control. |
| 2.4.11 Focus Not Obscured (Minimum) | Covered in dialog scope | Focused controls stay inside the scrollable dialog and are not behind a sticky overlay. |
| 2.5.3 Label in Name | Covered | Visible button text is part of the accessible name. |
| 2.5.7 Dragging Movements | Not applicable | The flow has no drag-only action. |
| 2.5.8 Target Size (Minimum) | Covered | Controls use at least 44 by 44 CSS pixels, above the 24 by 24 AA minimum. |
| 3.2.1 On Focus | Covered | Focus does not submit, revoke, switch, or close. |
| 3.2.2 On Input | Covered | Input does not cause an unexpected context change. |
| 3.3.1 Error Identification | Covered | Failed operations produce a text error in the affected flow. |
| 3.3.2 Labels or Instructions | Covered | Password, duration, session, and destructive actions have visible instructions. |
| 3.3.3 Error Suggestion | Covered where safe | Validation states state what must change without exposing sensitive detail. |
| 3.3.4 Error Prevention (Legal, Financial, Data) | Covered for account data | Destructive changes use explicit confirmation and scoped actions. |
| 4.1.2 Name, Role, Value | Covered | Native controls expose name, role, state, selection, and disabled state. |
| 4.1.3 Status Messages | Covered | Asynchronous status and errors are announced without moving focus. |

## EN 301 549 evidence map

The relevant web clauses map to the WCAG evidence above. The principal clauses
for this change are 9.1.3.1, 9.1.4.3, 9.1.4.11, 9.2.1.1, 9.2.4.3,
9.2.4.6, 9.2.4.7, 9.2.5.3, 9.3.2.1, 9.3.2.2, 9.3.3.1, 9.3.3.2,
9.3.3.3, 9.3.3.4, 9.4.1.2, and 9.4.1.3.

The account flow also supports clause 11 software evidence through exposed
names, roles, values, focus, and status. This file is implementation evidence,
not EN 301 549 certification. Product-level evidence must include the Tauri
shell, operating-system accessibility APIs, high-contrast modes, platform text
scaling, and assistive-technology runs.

## Holistic polish sweep

| Area | Result |
| --- | --- |
| Information hierarchy | One dialog title, one selected method, and bounded sections. |
| Copy | Active, literal, and short. There is no fake success or vague security promise. |
| Empty state | The session list can state that no other sessions exist. |
| Loading state | Pending work has visible feedback and blocks repeat action. |
| Error state | Error text stays with the task and preserves retry control. |
| Disabled state | Planned methods explain why they are unavailable. Disabled controls do not look active. |
| Destructive state | The UI identifies the target and asks for confirmation before account-data changes. |
| Responsive behavior | Content wraps, actions remain reachable, and the Android proof exercises the narrow dialog. |
| Motion | Required meaning does not depend on animation. Reduced-motion behavior comes from the shared system. |
| Performance | The dialog loads bounded data on demand. It adds no polling, browser database, or duplicate security state. |

## Verification commands

```text
cargo fmt --all -- --check
cargo clippy -p fasti-application -p fasti-contracts -p fasti-store -p fasti-api --all-targets --all-features -- -D warnings
cargo test -p fasti-application -p fasti-contracts -p fasti-store -p fasti-api
cargo xtask contract verify
cargo xtask test pr
pnpm lint:ui
pnpm build
pnpm test:ui
pnpm audit --prod --audit-level high
pnpm exec playwright test tests/e2e/android-auth-account.spec.mjs
```
