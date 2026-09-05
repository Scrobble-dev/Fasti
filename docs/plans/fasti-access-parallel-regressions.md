# Access A+C parallel regressions

Status: bounded header, mobile layout and notice fixes implemented and
independently reviewed. Latest combined browser run passes 52/52. Earlier
41/41, 6/6 and canonical results retain their original source identities.
Final-commit canonical and remaining delivery gates are pending. Unshipped.
Base: `62e10d2e9bd738ed5da425c008eb839f89cdbea5`.

## Ownership and purpose

### Resumed notice and delivery authority, 2026-09-05

The user approved resuming with Fasti's existing version conventions and the
minimal reserved-space notice correction, preserving confirmation text, focus,
recovery controls and all remaining gates. No generic VERSION file or runtime
dependency will be added. This resolves the earlier shipping-convention stop;
it does not approve C3 runtime, waive advisory policy or release M4 shared files.
The metadata commander has been notified of the bounded component extension.

Written execution gates:

1. Prove confirmed-result loss on a read-only Retry with a new browser regression.
   Seed initialNotice once at mount, not in every load. Preserve sign-in's explicit
   reset and all existing mutation/error/focus paths. Commit this verified fix.
2. Keep an empty, persistent status region at the existing notice location. Use
   the existing Tabler alert inside it, with its original focus ID and no nested
   live roles. Reserve typography-relative space with natural growth, not fixed
   clipping, hidden acknowledgement, overlays or delayed confirmation.
3. Verify notice insertion and recovery at 320/375/768/1440px in Light/Dark, plus
   enlarged text. Retain existing C1 and header/mobile tests unchanged. Measure
   geometry without calling raw displacement a CLS/INP score. Longer content
   must remain visible even if it exceeds the reserved minimum.
4. Independent source review and applicable QA/design/Impeccable, developer,
   security, Ponytail and exact-head canonical/CI gates precede merge into dev.
   Tauri authentication stays deferred. C3 qualification delivery may proceed
   independently; no speculative production writer crosses M4's ownership.

Rollback each notice correction independently while preserving its evidence.
No API, SDK, schema, archive, authorization or credential changes are involved.

Notice-retention checkpoint: the new regression first failed at the held Retry
assertion because `#access-notice` disappeared. The earlier attempt returned an
incorrect 204 fixture for revocation and is retained as a test-fixture failure,
not product evidence. Moving initial notice seeding from `load()` to `onMount`
then passed the new regression and all seven existing parallel regressions
(8/8, zero skipped). Independent read-only review found no introduced defect.
Evidence: `.gstack/qa-reports/notice-retention-red-contract.json` and
`.gstack/qa-reports/notice-retention-green.json`. These are controlled browser
tests, not real authentication or full accessibility conformance. Layout and
exact final delivery gates remain pending.

Notice-layout checkpoint: persistent polite/atomic live-region markup now owns
the existing Tabler alert. Its status role is present only when it has a notice;
empty reserved space is not another semantic status or keyboard stop. A minimum
of two text lines (three on narrow layouts), plus Tabler's padding and borders,
reserves space without clipping longer content. The first 52-test run passed
49 and failed three: an extra empty status broke an existing first-run assertion,
and both enlarged-text cases exposed a continuation-choice flex item overflowing
by 20px. The final markup removes the empty status role while retaining the live
region. The shared choice text now permits flex shrink and word wrapping.
Existing tests and helpers are unchanged.

The corrected focused run passed 12/12. Eight normal-text Light/Dark cases at
320/375/768/1440px measured zero change in the choice's document position after
the acknowledgement appeared. At 320px with 200% text and increased spacing,
the region grew by 192px to keep the whole notice visible; horizontal overflow
and clipping checks passed. This is measured geometry, not CLS/INP or universal
zero-shift conformance. Screenshots were inspected at narrow/wide Light and
narrow enlarged Dark. Evidence remains under
`.gstack/qa-reports/notice-layout-first` and `notice-layout-corrected`, with their
JSON reports. The full corrected combined run now passes 52/52, zero failed,
skipped or flaky, in `.gstack/qa-reports/notice-layout-combined.json`.
Independent source and screenshot review found no concrete introduced defect.
The existing C1 cases include forced colors, Night, reduced motion, Axe, focus,
session expiry and callback recovery. This remains bounded browser-fixture
evidence, not screen-reader announcement verification or WCAG/EN conformance.
Final-commit canonical verification and applicable delivery review remain next.

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

## Exact implementation checkpoint and parallel preparation

Clean implementation commit `3cbd52b65d26b12eb57f00b8a16d16e2a1bf4fd8`, tree
`e6d31d26db96390d0b5c4127f48df44dcce33577`, passed all 27 contract and 11 portable
checks with exit zero. Receipt source bindings identify that commit/tree and
`dirty:false`; execution is local and unpublished. Canonical log SHA-256:
`2ae28e5ef3e72d519a276115bc13166e3dd0a1e4203ff890837f333925e1e90e`.

The fresh Light/Dark pending matrix at 320/768/1440 passed six cases, zero failed,
skipped or flaky. Pending-state Axe violations/incomplete checks and console
errors were zero. JSON SHA-256:
`ae981df3f23013cce5e063c8c97346d60584e8edfced8d34b4a90f7f0a1a69bd`.
Playwright JSON does not embed Git identity; provenance uses the recorded clean
execution boundary. The earlier 41 passes remain pre-commit evidence, not a new
exact-commit rerun. Notice displacement remains 106px at 320 and 82px in affected
wider cases. Raw shift/event observations are not canonical CLS or INP results.

Developer-experience review covered local contributor commands, three settled
public-docs searches, error recovery, contribution surfaces and repository
artifacts. It completed with concerns, score 6.875/10 as bounded reviewer judgment.
Cold installation, real account setup and human time-to-first-success were not
measured. The deployed docs were not bound to this unpublished source. Existing
docs friction does not authorize expanding this component patch into shared docs.

Two read-only preparation lanes completed alongside that review:

- Notice: a persistent typography-relative status region is the smallest
  candidate. Preserve Tabler alerts, message timing, focus, natural expansion,
  announcements and all recovery controls. The region alone cannot guarantee
  zero shift for arbitrary content. Existing Retry calls `load()`, which resets
  notice text; retention through retry needs an explicit behavior decision.
  No notice implementation or design approval follows from preparation.
- Vault/recovery: isolated signing qualification already has exact local
  receipts. Native-notice inventory is already 48 entries; do not repeat its
  older 45-to-48 task. Next independent work is full referenced licence texts,
  hashes, provenance and remaining-header coverage. This does not approve a
  crypto/recovery profile, dependency adoption or shared-file access.

This checkpoint changes documentation only. It does not claim the resulting
documentation tree already has fresh canonical receipts. Preserve all raw
failure evidence. No PR, push, merge, migration/archive allocation or shared-file
release has occurred here; packaged Tauri authentication stays deferred.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
| --- | --- | --- | --- | --- | --- |
| CEO Review | Not rerun | Approved premise retained | 0 here | Not reopened | No framework or programme change |
| Eng Review | `/review` | Exact diff and tests | 1 logged | STALE TREE | f60c3da0 review predates mobile CSS/test addition |
| Adversarial Review | Native subagent | Independent scrutiny | 1 logged plus bounded follow-up | PARTIAL CURRENT | No concrete mobile-source finding; not a full refreshed gate |
| Design Review | QA/design workflow | UI and performance evidence | In progress | OPEN | Separate notice shift and final disposition remain |
| DX Review | `/devex-review` | Contributor experience | 1 | DONE WITH CONCERNS | 6.875/10; six observed/partial, two inferred dimensions; TTHW unmeasured |

**VERDICT:** Not cleared for merge; fresh engineering review and remaining
applicable delivery gates are required. Canonical implementation evidence is
green on 3cbd52b6, not a blanket qualification of later documentation commits.

**UNRESOLVED DECISIONS:**
- Shipping-convention disposition for the helper's missing VERSION assumption.
- Separate notice design/behavior scope and performance-gate disposition.
