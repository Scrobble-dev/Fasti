# Access A+C parallel regressions

Status: bounded header, mobile layout and notice fixes implemented and
independently reviewed. Clean source `63f955c4` passed 142 ordinary browser
cases, two Lighthouse cases, 27 contract and 11 portable gates. Its evidence
retains that source identity. Remaining coverage assertions, screen-reader
evidence and hosted delivery are in progress. Unshipped.
Current base: `4bd84a562e60b04c278173529164f06cc41c7753` (merged C3
qualification only). Clean rebase checkpoint: `211d3ee161285ef5cc6432e0f6e0cdc474718668`.

## Ownership and purpose

### Final coverage closure, 2026-09-05

Independent audit found six unasserted UI path groups, not demonstrated defects.
The bounded source inventory is 18/24 (75%), not instrumented coverage. Close
them before publication, reusing this slice's existing test helpers and fixtures:

1. Extend held confirmation to hold its follow-up projection read and assert
   that the same header and exit controls remain disabled until settlement.
2. Exercise generic confirmation failure, held evidence dismissal and held
   sign-in restart. Assert recovery, busy-state release and explicit notice reset.
3. Complete setup, leave A and return. Assert that its consumed completion
   notice does not replay on remount.
4. Run focused tests, obtain an independent delta review, then verify the final
   committed source and publish. Preserve the existing 142-case and Lighthouse
   reports. Do not change production behavior just to satisfy a fixture.

M4 released one additive Unreleased changelog entry for the bounded fixes and
performance gate; existing entries and version conventions remain unchanged.
It also released only the AGENTS browser-QA paragraph for the command split,
Chrome prerequisite and link to this guide. All other shared ownership remains.
The Orca check uses an isolated desktop session and owned browser only. Its
generated speech output is not human-listening or full-conformance evidence.

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

## Historical GSTACK review report before notice authorization

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

## Current delivery checkpoint, 2026-09-05

The historical decisions above are superseded by the user's notice/version
authorization recorded at the top. They are not renewed approval requests.
Source `20c0b030e4bc31862a012f1b34c81e23774818a3`, tree
`25b235d602be391601c2761cf6e3ef41c789e8e1`, passed the exact clean-source
canonical gates: 27 contract and 11 portable checks. The combined browser suite
passed 52/52; a fresh pending/settled Light/Dark matrix at 320/768/1440 passed 6/6.
All 12 new images were inspected. Independent final source review found no
concrete introduced defect. Final Impeccable and scoped security review completed
with explicit evidence limits; no Codex Security service was used.

Two fresh disposable Chromium traces at 320 and 1440 measured the held fixture
confirmation and subsequent account navigation. Official installed DevTools
insights reported CLS 0 for both routes at both widths. First-run observed INP
was 19.919/24.520 ms; account-transition INP was 47.113/37.855 ms. Each trace had
ten recognized interactions, one fixture-only POST and no page errors. These
are unthrottled light-theme local lab observations, not Lighthouse CI, field
performance or full authentication proof. Initial parser output lacked locale
initialization and is explicitly superseded by reanalysis of the same traces.
Raw trace SHA-256 identities:

- 320: `649be2fe7be1eb1d380921dee424dd617414572c22fb5027c683f8fceef6e347`.
- 1440: `8439181bdd9d2f90ed6d1b6f0dd0110b1fc29b9a04a13d0213660707c3f27954`.

### Persistent performance sentinel gate

M4 explicitly released one isolated Access browser-performance script/config
and the Browser UI job wiring in `.github/workflows/ci.yml`. It retains all
Search tests, runtime/shared ownership, migration v17 and archive v7. Any needed
manifest/lockfile dependency edit requires naming the exact package and files
to M4 first. No installed-host cache may become an undocumented CI dependency.

M4 subsequently released exactly root `package.json` and `pnpm-lock.yaml` for
development-only `lighthouse: 13.4.1` and `puppeteer-core: 25.8.0`. Official npm
metadata confirms Apache-2.0 for both, Lighthouse Node >=22.19 and Puppeteer
Node >=22.12. Lighthouse's Puppeteer range includes the exact selected version.
The existing Node 22 CI line resolves a compatible current patch; local Node is
24.20.0. Use Lighthouse's official timespan user-flow API, not a navigation-only
report or a replacement raw-observer implementation. The Browser UI job runs the
sentinel serially without retries and retains separate evidence. No `@lhci/cli`
service or production instrumentation is proposed; this is Lighthouse in CI.

The first run correctly failed both widths: Playwright's Chromium
151.0.7922.34 lacks the legacy interaction trace event required by Lighthouse
13.4.1, so INP was not applicable despite real fixture interactions. Official
source for that exact Chromium tag proves the event is absent; the exact
152.0.7977.82 source restores it. The local existing stable Chrome is that
152 version. The sentinel therefore launches a fresh owned stable-Chrome profile
through Puppeteer's supported channel option. Ordinary Playwright tests retain
their configured browser. CI refreshes stable Chrome only inside its ephemeral
job using the existing Playwright installer. No local user browser installation
is changed, and no user browser profile is connected. Browser versions are
recorded per run; this is not a pinned-browser guarantee. Missing metrics remain
failures. No synthetic trace event, threshold waiver or alternative metric was
introduced. The original 151 failure reports remain retained.

Focused Lighthouse execution then passed both widths on existing Chrome
152.0.7977.82: CLS 0 at each width, INP 95.946 ms at 320 and 54.199 ms at 1440.
Each recorded three distinct positive interaction IDs, one intercepted fixture
POST and no page error. These are working-source lab measurements, not final
commit or hosted results. A negative control removed only the notice minimum
in browser memory: both cases failed the unchanged CLS assertion, with
0.03542709350585938 and 0.00655845359519676. The temporary test override was
removed and the exact green-test SHA-256 restored:
`35ce3ce30c749068302c6d9df96ef7bdb07610071c14760f8ca63742b99c18cc`.

Green report SHA-256:
`9887ec0d4df91f3bfd0ffd287f51fa1c9359ab87af673a516f0205fcc149a3de`.
Negative-control report SHA-256:
`69dc74aebc0a0f53ec53018679d53e2bc7d5aa666a9aac12d5f68b647ca5a165`.
Independent source review confirmed fixture isolation, missing-metric failures,
owned-browser cleanup and the serial/no-retry CI split. Parsed lock comparison
confirmed 70 additions and no changed prior package or snapshot entry. Fresh
audit output has no unsuppressed advisory entry; its metadata retains two high
image-size findings covered by the unchanged existing patch/policy. No new waiver.

Reproduce with installed Node >=22.19, the frozen pnpm lock and stable Chrome
with the restored trace event (locally verified at 152.0.7977.82):
`pnpm test:ui --grep @performance --workers=1 --retries=0 --output=test-results-performance`.
The existing harness starts its own listeners and refuses occupied ports.
Run ordinary cases separately with `pnpm test:ui --grep-invert @performance`.
The CI-only Chrome refresh is not an instruction to replace a user's browser.
Lighthouse outputs remain fixture-only local Vite timespan evidence. Final
commit verification and exact hosted results remain delivery postconditions.

1. Reuse repository-installed tooling if it can produce the specified metrics.
   Do not call raw shifts CLS or event-duration maxima INP.
2. If the required Lighthouse flow is unavailable, qualify and pin the smallest
   test-only dependency with explicit shared-file coordination before editing.
3. Add a bounded fixture-only delayed-confirmation sentinel, fail on absent
   metrics, and retain its report. No real account mutation or native transport.
4. Independently review it; run focused and final canonical/hosted gates before
   merge. Existing tests and thresholds remain intact.

Manual assistive-technology and full WCAG/EN conformance remain unclaimed.
Packaged Tauri authentication remains deferred. No PR or merge of this browser
slice has occurred; C3 qualification PR 127 merged independently at `4bd84a56`.

### Final source and assistive-technology evidence

Clean `63f955c4` passed 142 ordinary browser cases and two official Lighthouse
timespans. At 320/1440px, CLS was 0/0 and INP was 37.861/35.383 ms. The ordinary
and Lighthouse JSON SHA-256 values are respectively
`f49b5c2868fb107c3fbe9d0a60586ea38f8b36af5abaf438a6251f0c244e4139` and
`e04e668e0e8d460de3900f84196b828e76b62801a6c28ae029c186833e435326`.
Fresh evidence-ledger canonical and ordinary runs also passed before rebase.
Rebase onto merged C3 qualification changed no component or sentinel bytes.

The six-gap coverage extension first passed eight of nine cases. The remaining
case waited for a nonexistent `Home` navigation link. The captured page exposes
`Overview`; correcting that selector changes no product behavior. The first
failure remains in `.gstack/qa-reports/access-coverage-closure-first`.
The corrected remount case passed with no retry (1/1); its JSON and artifacts
are retained as `access-coverage-closure-corrected`. Together with the first
run's eight passing cases, all six identified gaps now have passing focused
checks. This closes the bounded 24/24 source-path inventory, not instrumented
or whole-application coverage. Final combined clean-commit execution remains
required before delivery.

An isolated Orca 50.2 / AT-SPI 2.60.4 / Chrome 152.0.7977.82 run completed six
stages with zero page/probe errors and one intercepted continuation POST.
The empty notice region acquired its confirmation announcement; the disabled
header was announced as a greyed button. Successful POST followed by projection
failure retained confirmation through a held read-only Retry, without a second
confirmation utterance. Recovery and return to A announced their focused headings.
DOM and AT-SPI agreed on disabled and focused states. During held Retry, focus
was temporarily on the document after its button was removed; only settled
heading focus is claimed.

The run used private DBus/XDG/Xvfb and a fresh browser profile on port 4273.
No user settings, shared browser listeners or backend state changed. Speech
synthesis was intentionally disconnected: this is actual Orca-generated speech
text, not audible human testing or complete WCAG/EN conformance. Source was
`211d3ee1` with documented plan/test edits; component SHA-256
`89e8fac7e1f404dd4e42d2959dd26d588c708fe2a4e678a6e1c9688ab4473737`
is unchanged from `63f955c4`. Evidence is in
`.gstack/qa-reports/orca-access-result.md` and `orca-r3/`; result/debug log hashes:
`af6fbf9f27c9680082c0585298f3064e8a9cc1005ed766eb8aabd9b750910ef1`,
`b7cc1ab497b4c435ece5ec670a3ef3ca3990986dbcd026c44d5054b47e839ced`.
Owned processes and the short temporary runtime directory were cleaned up;
all raw evidence, including setup failures, remains retained.
