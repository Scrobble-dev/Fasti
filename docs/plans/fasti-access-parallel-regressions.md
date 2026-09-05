# Access A+C parallel regressions

Status: bounded header fix and seven regressions implemented and independently
reviewed. Existing C1 plus new cases pass 31/31. Canonical verification passed on
implementation commit `bf8526ad`; remaining delivery gates are pending. Unshipped.
Base: `62e10d2e9bd738ed5da425c008eb839f89cdbea5`.

## Ownership and purpose

The metadata commander explicitly released a new isolated
`tests/e2e/access-parallel-regressions.spec.ts` for Access tests on 2026-09-05.
This is not a shared-file handoff. M4 retains migration v17, archive v7 and
all shared production surfaces. Commander is the only integration writer.

Prove the already-prepared A+C interaction cases against committed browser UI.
Use the existing Playwright configuration, health-stub projection, generated SDK
parser and contract problem catalogue. Do not duplicate the full projection,
change existing helpers, add dependencies, change production UI or alter gates.
No packaged Tauri authentication or real backend authentication claim follows.

## Written gates

1. Confirm ownership and exact clean base. Inspect existing tests and route flow.
2. Add one isolated test file for confirmed-state resume, pending-confirmation
   exits, idle/absolute expiry without reread, post-commit read retry and keyboard
   choice revision. Count requests and inspect focus, not only visible text.
3. Run focused tests with the existing browser harness. Retain failures honestly;
   do not mark expected failures, skip tests or weaken assertions to get green.
   Reproduce any candidate defect before calling it verified. Do not fix shared
   production files before M4 releases them.
4. Obtain independent test review. Record exact results and remaining limits.
   Keep a failing test-only slice uncommitted and unshipped until its owning fix
   can land with it. Run applicable canonical/review/delivery gates then.

## Evidence limits and rollback

Controlled HTTP fixtures prove browser requests and rendered interaction only.
They do not prove TrailBase, session persistence, server authorization, recovery
durability, real factor enrollment, or WCAG/EN 301 549 conformance. Keyboard and
focus assertions contribute bounded accessibility evidence; manual assistive
technology and full viewport/theme review remain separate gates.

No API, SDK, OpenAPI, AsyncAPI, JSON-LD, schema or archive changes are proposed.
No runtime performance claim is made. All fixture state is per-test memory.
Before integration, rollback is exclusion of this whole slice. After integration,
revert its header guard and associated regression additions together, preserving
unrelated work and historical failure evidence. This restores the prior header
exit defect; do not describe that rollback as a repaired first-run flow.

## Scoped production-fix authorization, 2026-09-05

After the reproduced finding below, M4 explicitly released only
`packages/ui/src/account-security-view.svelte` for this bounded fix and agreed to
remain read-only on it during the allocation. The new test and this Access plan
remain owned here. No other shared file, migration or archive ownership changed.

Root-cause trace: the header directly invokes `onOpenAccountSecurity`, which
selects settings and unmounts first run. `completeContinuation` sets `busy` before
awaiting its POST and clears it in `finally`; the existing footer uses this same
state to disable Save/Cancel. The header omits that guard. The original C1 commit
already contains the omission. The held-response test proves the missing guard
and navigation before any production edit. There is no matching existing TODO.

Fix gate: apply the existing native `disabled={Boolean(busy)}` pattern to the
header button. No navigation abstraction, global navigation lock, backend change
or replacement state system. Trace sibling exits, run the seven regressions and
existing C1 UI suite, formatting/type checks, independent review and canonical
PR gates. Return exact commit and file hash before integration. Full applicable
UI, security and delivery gates remain required; this scoped release does not
bypass them or the existing unanswered shipping-convention question.

## Execution checkpoint, 2026-09-05

Final focused run: seven tests, six passed, one failed, zero skipped or flaky.
Command: `PLAYWRIGHT_JSON_OUTPUT_NAME=.gstack/parallel-reviewed.json pnpm test:ui tests/e2e/access-parallel-regressions.spec.ts --output=.gstack/parallel-reviewed-results --reporter=list,json`.
Duration: 40.814 seconds for the full local harness invocation, not a product
performance measurement. Source-test SHA-256:
`198afc34c63f6f5c59ec64e0d8eb4d9a11cd4f74a378b18c36b80fe16f64d098`.
Formatting and focused TypeScript compilation pass. Offline frozen installation
used the existing lock without changes. Both harness listeners terminated.

Passing cases: confirmed-step resume, cancellation dismissal, idle expiry,
absolute expiry, post-revocation read retry, keyboard choice revision. Their
scope is the controlled browser fixture, not live authentication. Resume includes
one automated Axe scan and horizontal-overflow assertion; this is not whole-UI
accessibility conformance or manual assistive-technology evidence.

Failing case: while the continuation POST is held, the header **Manage existing
access** stays enabled. Keyboard Enter leaves C for `/settings/account`; the
first-run view unmounts and its confirmation notice is not visible after the held
response is released. Footer Save/Cancel remain disabled. The initial run found
the enabled button; both subsequent runs reproduced the keyboard navigation.
No production code was changed. Source owner is
`packages/ui/src/account-security-view.svelte:619` and its navigation callback.

Retained local evidence:

- `.gstack/parallel-reviewed.json`: final machine-readable results and errors.
- `.gstack/parallel-reviewed-results/access-parallel-regression-ee7c9-ile-confirmation-is-pending-chrome/confirmation-pending.png`: held confirmation.
- Same directory, `after-header-keyboard.png`: premature return to A.
- Same directory, `trace.zip`, `error-context.md`, `test-failed-1.png`.
- `test-results/` and `.gstack/parallel-repeat-results/`: earlier evidence,
  preserved rather than overwritten.

The second run also exposed a test mistake: it required retention of the consumed
`auth=continue` URL marker. Corrected to assert the first-run pathname; no product
claim was made for that initial test failure. Independent review also tightened
the tests to ledger every non-GET Access request and require exact POST paths.
The final reviewer found no remaining concrete test defect. GET counters remain
Access-prefix-wide; resume proves a full reload and expiry proves displayed
Discover authority, not backend state.

Ponytail review: no dependency, fixture-framework or speculative abstraction to
remove. Existing harness projection and generated parsers are reused. Complexity
review is not shipping approval. Canonical, full UI and delivery gates remain
unrun for this red test-only slice. The metadata owner received the exact finding
and ownership disposition; no shared-file handoff was inferred.

## Scoped fix verification

The released header now uses `disabled={Boolean(busy)}`, matching the existing
Tabler footer controls. No other production file changed. The guard stays in
place through confirmation and the subsequent projection or choice refresh;
existing `finally` cleanup restores normal interaction. Automatic completed-
setup navigation remains unchanged. The first-run ready-state exits do not
render during the pending continuation state.

Combined browser command:
`PLAYWRIGHT_JSON_OUTPUT_NAME=.gstack/parallel-fixed.json pnpm test:ui tests/e2e/access-parallel-regressions.spec.ts tests/e2e/access-c1.spec.ts --output=.gstack/parallel-fixed-results --reporter=list,json`.
Result: **31 passed**, zero failures, skips or flaky cases. The unchanged test
that failed on the base now passes with the one-line production fix. Existing
light/dark/night viewport matrices, forced colors, text spacing, reduced motion,
callback, session and recovery cases also pass. Raw JSON, fixed-state screenshots
and earlier failing evidence remain local and distinct.

`git diff --check`, focused Prettier and `pnpm lint:ui` pass. Independent native
source review found no concrete correctness issue and no new dead end; Ponytail
review confirms reuse of existing state and native HTML without new dependencies
or abstractions. This does not claim real-backend authentication, packaged
desktop support, manual assistive-technology evidence or whole-product conformance.
Canonical PR verification passed on clean implementation commit
`bf8526ad4ecbfc270a475428d9120708c7ac60cc`, tree
`a002ced61f6998299a1033ef6adb171360416e42`. Its 27 contract and 11 portable gates
passed. Four ignored tests and the existing Tauri unused-app warning remain in
the raw log; this is not a zero-warning or zero-ignore claim.

A fresh post-commit browser run passed the same 31 cases with zero failures,
skips or flaky cases. Its JSON is `.gstack/parallel-exact-bf8526ad.json`, SHA-256
`f2c8498b581113ba1d9ed95660ab678640fd43516337030a8dc8073b6788335d`.
An additional pass reused the existing resume and pending-confirmation tests at
1440 and 375 CSS pixels: four passed. Rendered screenshots preserve the Tabler
controls and disabled header state. The Impeccable detector returned no findings.
These checks do not substitute for manual assistive-technology, all-browser,
layout-shift or complete accessibility proof.

This documentation follow-up does not change those verified UI or test bytes.
Remaining applicable reviews, final-head gates, the shipping-convention
disposition, hosted CI, PR, merge and integrated-tree proof remain pending.

## Mobile layout correction gate, 2026-09-05

M4 extended the exclusive component allocation to the existing mobile
breakpoint rules for `.access-heading > div` and `.task-copy`. M4 remains
read-only on this component. All other shared files, v17 and archive v7 remain
M4-owned. The separate delayed-notice layout shift is not in this release.

The existing `flex: 1 1 18rem` becomes a vertical allocation when the parent
changes to a column. Source and restored DOM experiments attribute the gaps
to that retained basis, not the disabled header. Desktop geometry is unchanged
by the experimental basis reset. These rules are identical in the current base.

1. Add an isolated browser regression using the existing authenticated Access
   helper. Leave existing tests and shared helpers unchanged during QA.
2. Prove failure before editing production CSS. Cover all five task-copy
   wrappers and the first-run heading, not only the original header symptom.
3. Set `flex-basis: auto` for both selectors inside the existing mobile query.
   Preserve Tabler markup, all controls, desktop sizing, focus and trust rules.
4. Verify content-relative vertical spacing at 320/375px and the unchanged
   desktop row/basis at 768/1440px, Light/Dark and enlarged text spacing. Reuse
   existing C1 and pending-confirmation cases for interaction evidence.
5. Obtain independent diff/test review, commit the coherent correction, and
   rerun applicable exact-head gates. Return commit, tree, component hash and
   viewport evidence before handback. This does not waive any delivery gate.

No API, SDK, schema, archive, credential or dependency change is needed. Native
CSS corrects the existing Tabler composition; no custom component is added.
Rollback reverts only this mobile rule and its regression, retaining the
pending-header fix. That would restore the mobile gap and is not a UI repair.
The notice-shift finding and packaged Tauri follow-up remain separately open.

### Mobile correction verification

Before the CSS edit, the new Light 320px test failed on the expected `auto`
basis versus actual `288px`. The failure screenshot and trace remain in
`.gstack/qa-reports/layout-red-f60c3da0-results`; its JSON SHA-256 is
`ad85561587e6d04714af0454fff58e98f3668a240d84a5d4c7581255f364043a`.

After the five-line mobile rule, the combined existing C1, pending/resume and
new layout suites passed **41/41**, with zero failed, skipped or flaky cases
and no runner errors. Ten new cases cover Light/Dark at 320/375/768/1440px
plus 320px enlarged text. They reuse `mockAuthenticatedAccess`, check all five
task-copy wrappers and the first-run heading, and verify zero Access mutations.
The geometry check permits normal inline leading; it does not prove absence
of vertical clipping or whole-product accessibility conformance.

Command: `PLAYWRIGHT_JSON_OUTPUT_NAME=.gstack/qa-reports/layout-fixed-f60c3da0.json pnpm test:ui tests/e2e/access-layout-regressions.spec.ts tests/e2e/access-c1.spec.ts tests/e2e/access-parallel-regressions.spec.ts --output=.gstack/qa-reports/layout-fixed-f60c3da0-results --reporter=list,json`.
The actual invocation used the absolute JSON path. JSON SHA-256:
`ac1d0e4d35d82a0b465197d43a073de2305a7d4422f10e8e581b3435f3556e4e`.
The 141.299556-second run measures the test invocation, not product performance.
Both harness listeners closed. This is pre-commit working-source evidence.

All 20 new screenshots were inspected. Enlarged-text full-page captures retain
the current SPA scroll offset and fixed toolbar; they are not focus-occlusion
or manual assistive-technology proof. Existing raw screenshots remain intact.
Independent native source and test reviews found no concrete introduced issue.
Ponytail review found no unnecessary dependency, abstraction or duplicate fixture;
the fix uses native CSS at the shared cause. `pnpm lint:ui`, focused Prettier and
`git diff --check` passed. Canonical and review receipts for the earlier head
do not qualify the new head; exact-head delivery verification remains required.
