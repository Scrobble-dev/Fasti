# Account and session UI quality acceptance record

Status: **PENDING — acceptance specification only**.

Earlier wording described planned behavior as implementation evidence. That
wording is superseded. This record does not contain an exact-head test result,
an accessibility conformance claim, or production identity proof.

## Product truth

| Capability | PR A state | Required evidence before activation |
| --- | --- | --- |
| Human sign-in | Unavailable until PR C1 | TrailBase proof exchange, stable subject resolution, membership and role checks, and administrator continuity. |
| Fasti browser session | Dormant foundation only; unavailable on production routes until PR C1 | Direct deterministic domain, application, and store tests in PR A. Route, cookie, CSRF, inventory, and revocation evidence belongs to PR C1. |
| Session location and device | Not recorded | The product must say `Not recorded` until a governed collection contract exists. |
| Profile switching | Unavailable | PR C1 or a later package must prove session-local authorization. |
| TrailBase password and TOTP | Unavailable until the TrailBase package proves support | Current official TrailBase documentation, source, and integration tests. |
| Passkeys and recovery codes | Unavailable until PR D | Server-owned credential lifecycle and recovery evidence. |
| Generic OIDC, Authentik, OAuth, and device authorization | Unavailable until the relevant PR E package | Protocol, management, consent, token, device, and revocation evidence for each named capability. |

PR A must not expose a production `BrowserUser`, local password account,
development account, custom TOTP, simulated passkey, backup-code, or fabricated
OIDC path.

## Interaction acceptance checklist

The first-run guided setup and the persistent account task map are separate
purposes. Both must use Tabler before custom components. They must satisfy these
checks when their owning package implements them.

### AskTog and Gestalt

- Show pending, success, failure, and unavailable states near the affected action.
- Keep control with the person. Never trigger a destructive action on focus.
- Use bounded defaults. Do not offer an indefinite session.
- Keep each account, credential, session, device, or provider action with its subject.
- Use proximity, similarity, common region, continuity, and figure/ground to make groups explicit.
- Keep one primary action for each decision and preserve a clear recovery path.
- Use literal labels. Do not ask a person to remember an opaque identifier.
- Keep interactive targets at least 44 by 44 CSS pixels.

### Nielsen's ten heuristics

| Heuristic | Required evidence |
| --- | --- |
| Visibility of system status | Loading, pending, current, verified, unavailable, failed, and recovery states are visible and announced where needed. |
| Match with the real world | Copy uses account, credential, session, device, provider, revoke, and `Not recorded` accurately. |
| User control and freedom | Close, cancel, back, retry, scoped revoke, and safe exit paths work. |
| Consistency and standards | Tabler and established Workbench patterns are used before custom controls. |
| Error prevention | Bounded input, repeat-action locks, clear targets, and explicit confirmation protect state. |
| Recognition rather than recall | Current account, session, expiry, activity, evidence, and available actions remain visible. |
| Flexibility and efficiency | Keyboard operation and an efficient repeat-user path work without cluttering first-run setup. |
| Aesthetic and minimalist design | Each view contains only the information and actions needed for its task. |
| Error recognition and recovery | Plain errors state what happened, what stayed safe, and the next valid action. |
| Help and documentation | Nearby help explains the boundary and the reason for every unavailable state. |

Relevant IxDF lenses are cognitive load, Hick's law, Fitts's law, error
prevention and recovery, recognition over recall, visual hierarchy,
progressive disclosure, and inclusive design.

## WCAG 2.2 Level AA acceptance matrix

No criterion below has an exact-head pass result in this record.

| Success criterion | PR A result | Required evidence |
| --- | --- | --- |
| 1.3.1 Info and Relationships | Pending | Semantic headings, labels, groups, tabs, lists, and dialog relationships. |
| 1.3.2 Meaningful Sequence | Pending | DOM, reading, and focus order match. |
| 1.3.3 Sensory Characteristics | Pending | Instructions do not depend on position, shape, or color. |
| 1.4.3 Contrast (Minimum) | Pending | Measured text contrast in every supported theme and state. |
| 1.4.10 Reflow | Pending | Narrow viewport and 200 percent zoom proof with no loss of action or content. |
| 1.4.11 Non-text Contrast | Pending | Measured control, focus, border, and state contrast. |
| 1.4.12 Text Spacing | Pending | Required text-spacing overrides do not clip or obscure content. |
| 2.1.1 Keyboard | Pending | Keyboard-only completion of every available task. |
| 2.1.2 No Keyboard Trap | Pending | Focus can enter, move through, and leave each surface. |
| 2.4.3 Focus Order | Pending | Focus follows the task and returns reliably. |
| 2.4.6 Headings and Labels | Pending | Headings and labels state purpose and action. |
| 2.4.7 Focus Visible | Pending | Focus remains visible in every theme and state. |
| 2.4.11 Focus Not Obscured (Minimum) | Pending | Focus is not hidden by overlays or scrolling regions. |
| 2.5.3 Label in Name | Pending | Visible control text is part of the accessible name. |
| 2.5.7 Dragging Movements | Not applicable | The planned flow has no drag-only action. Reassess if the interaction changes. |
| 2.5.8 Target Size (Minimum) | Pending | Measured targets meet the project 44 CSS-pixel requirement. |
| 3.2.1 On Focus | Pending | Focus does not submit, revoke, switch, or close. |
| 3.2.2 On Input | Pending | Input does not cause an unexpected context change. |
| 3.3.1 Error Identification | Pending | Text identifies the failed operation. |
| 3.3.2 Labels or Instructions | Pending | Visible instructions cover inputs and destructive actions. |
| 3.3.3 Error Suggestion | Pending | Safe, useful correction guidance is present. |
| 3.3.4 Error Prevention | Pending | Destructive account-data changes are reviewable and reversible where possible. |
| 4.1.2 Name, Role, Value | Pending | Accessibility-tree inspection covers every interactive state. |
| 4.1.3 Status Messages | Pending | Status is announced without an unexpected focus move. |

## EN 301 549 evidence map

Applicable web clauses include 9.1.3.1, 9.1.4.3, 9.1.4.10, 9.1.4.11,
9.2.1.1, 9.2.4.3, 9.2.4.6, 9.2.4.7, 9.2.5.3, 9.2.5.7, 9.2.5.8,
9.3.2.1, 9.3.2.2, 9.3.3.1, 9.3.3.2, 9.3.3.3, 9.3.3.4, 9.4.1.2,
and 9.4.1.3. Their results are pending. Clause 11 software evidence also
requires the Tauri shell, operating-system accessibility APIs, high-contrast
modes, platform text scaling, and assistive-technology runs. This record is not
EN 301 549 certification.

## Required holistic evidence

The implementing package must record information hierarchy, copy, empty,
loading, error, disabled, destructive, responsive, reduced-motion,
performance, and memory results. It must state the exact head SHA, environment,
viewport, theme, assistive technology, commands, timestamps, and failures.

## Required verification commands

These commands are acceptance requirements. This record does not claim that
they ran or passed on the current exact head.

```text
cargo fmt --all -- --check
cargo clippy -p fasti-application -p fasti-contracts -p fasti-store -p fasti-api --all-targets --all-features -- -D warnings
cargo test -p fasti-application -p fasti-contracts -p fasti-store -p fasti-api
cargo xtask contract verify --locked
cargo xtask test pr
pnpm lint:ui
pnpm build
pnpm test:ui
pnpm audit --prod --audit-level high
pnpm exec playwright test <owning-package-account-and-session-spec>
```
