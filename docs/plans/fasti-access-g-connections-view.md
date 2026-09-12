# G — compact Connections readiness view

Status: `IMPLEMENTED_LOCALLY; DELIVERY_GATES_OPEN`. This is the approved G presentation slice,
not completion of configured connection lifecycle or authentication.

## Scope and ownership

The existing Connections view calls `host.listIntegrations()` through the real
Workbench/SDK reader. Its response currently describes adapter readiness, not
proof of a configured live service. Replace the card grid with a compact Tabler
table using existing tokens and components. Preserve all status labels, detail,
setup instructions, endpoint/platform facts, Refresh, errors, empty state and
`ApiClientsPanel`. Do not invent live connections or add disabled CRUD controls.

One delegated writer owns only `packages/ui/src/connections-view.svelte` and
focused existing browser regression tests. The commander owns this plan,
integration and review. No API/SDK/host, schema, Metadata application/runtime,
dependency or capability activation change. Keep this coherent commit separate
from E1 persistence; it has no dependency on migration19 or OIDC adoption.

## Design and gate

Operate mode: compare reported readiness and find the next setup action.
Reuse Tabler `.table` and responsive composition before custom CSS. Keep Fasti
fonts/tokens, text plus status icons, native table headings and existing focus
styles. At narrow widths preserve every fact without page-wide overflow.
Use concise labels that distinguish readiness from live connection health.
Impeccable distillation preserves behavior and content; no visual-world redesign.

Before commit: UI typecheck, formatting, Tabler guard, targeted browser checks
for normal/error/empty/refresh recovery and keyboard/reflow behavior, and one
batched desktop/mobile/light/dark visual inspection. Check automated accessibility
in the same run. Record observed results and remaining manual/whole-product
limits, not blanket WCAG or EN301549 conformance. Run Impeccable detector once
after implementation and independent diff review. No runtime performance claim
or packaged Tauri claim follows from this view change.

Rollback is the isolated UI commit. No data migration or user state is changed.

## Local verification — 2026-09-13

Implemented the native Tabler table and focused browser test leaf. Source review
found no concrete regression. All seven reported states, setup/details, endpoint
and platform facts, Refresh, error/empty behavior and API clients remain. The
table scrolls locally on small screens; it does not widen the page. This remains
readiness supplied by the existing reader, not configured-service health.

Final browser run: **9 passed**, no retries. The existing runner rebuilt tokens,
SDK and UI successfully before Chrome ran. This resolved stale local SDK build
outputs without source or dependency changes. Prettier, whitespace and Tabler
boundary checks pass. The initial Impeccable detector reported no findings;
rendered QA then found real contrast and wrapping defects, which were corrected
instead of relying on that detector. Refresh now uses Fasti semantic tokens
through Tabler's own state variables; transitions do not invert contrast.

The automated checks cover seven state projections, pending refresh continuity,
contract-error recovery, empty state, retained API-client panel, 44px Refresh,
focus visibility, keyboard scrolling, page reflow and scoped Axe in light/dark
at 320/768/1440. A separate focused-button Axe assertion covers keyboard focus.
Desktop, tablet and mobile captures were inspected; final tablet/mobile captures
confirm intact short labels. Native screen-reader operation, forced colors,
whole-product WCAG/EN301549 conformance, production health and performance are
not established by this slice. Existing remaining programme gates still apply.

Nielsen's ten heuristics were reviewed through status visibility, literal
readiness language, recovery/user control, consistent native
interaction, error prevention, recognition, keyboard efficiency, minimal layout,
error diagnosis and retained setup/help information. Grouped rows keep related
facts together (Gestalt proximity/similarity). AskTog's work protection and
target-size principles, IxDF cognitive-load guidance and Sierra's user-mastery
principle inform the retained setup actions and stable refresh behavior. No
claim of measured user task-time improvement is made.

Final fixture evidence: `target/g-connections-browser-release.log`, SHA-256
`b973ffd2f41b37af79e565d2fd52c4d8178eb228276b4407cb7ef94c6492caca`.
Six screenshots are under `target/g-connections-evidence/test-results-g-connections-release/`.
Earlier failing runs remain alongside that directory. These are local
file-bound evidence, not canonical/CI/PR delivery receipts. Full PR gates and
merge are still open; no packaged desktop authentication claim is added.
