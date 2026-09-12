# C2-I browser client inventory

Status: `IMPLEMENTING; DELIVERY_GATES_OPEN`.

## Resumed delivery: 2026-09-12

The user approved capability-level delivery with no new qualification-only
subproject unless a concrete delivery failure requires one. Preserve this
implemented inventory slice, existing tests and evidence. The integration writer
will finish combined review, commit coherent source, build exact artifacts and run
the existing real-browser inventory gate before canonical delivery and merge.
Recent-authentication and C3 work remain separate; neither blocks inventory reads.
Migration18 and archive7 ownership/compatibility remain unchanged. Historical
checks below are not final-head evidence. No full C2 or packaged-Tauri claim.

Combined read-only integration review found no verified defect on 2026-09-12.
Harness tests: 23 passed. Initial JavaScript batch: 119 passed, one failed because
SDK dist lacked the source's new inventory example. Rebuilt the existing SDK;
all 69 SDK tests then passed, including the live Rust conformance fixture.
This is an artifact-preparation correction, not a production policy change.
The four existing inventory screenshot directories are preserved under
`~/.gstack/projects/Scrobble-dev-Fasti/reviews/c2-inventory-preserved-20260912/`.
No screenshot or failed evidence was deleted. Real-browser evidence remains open.

## Current implementation checkpoint

Saved 2026-09-09. Source remains uncommitted at `d274e037`; nothing is staged.
Current continuation: combined source review and coherent commit, clean-source
daemon/CLI/web builds, actual inventory browser proof, then final delivery gates.
No test rerun or production change occurred during this documentation save.

Latest source-review checkpoint: all three independent scoped reviewers found
no verified defects in migration/persistence/authorization, API/contracts/SDK,
and A+C UI/browser harness/receipt handling. This is source-only clearance.
The commander's final combined review, coherent commit, clean-source builds,
actual browser proof and final delivery gates remain open.

Metadata's Search source-limit window is now terminal and released to Access.
Local Git confirms base `7f05568d9fca874d5777b65191b1f611fab751ff`, head
`87da076d1b8106545a285e18458cd75524d1b10a`, tree
`710f97f98c731e3bcad961efa6e610da577c1091`. The owner reports its focused tests,
lint and browser checks passed; Access has not rerun them. The generated diff
touches only Search response fields: OpenAPI seven insertions and SDK nine
changed lines. This local commit is not a merged handoff. Preserve source
ownership and regenerate from combined sources before integration.

### Contract closure and ordinary-browser receipt integration

The reviewed contract/example batch is now **101 passed, zero failures/skips**,
including all 32 example tests and five new inventory security mutations.
Log `inventory-contracts-reviewed.log` SHA256
`b3d0feba2d3896c9acd2371da45cfb027d268750a71820b8a38c668bd0e5c376`.
The earlier five contract failures below are closed. Independent source review
cleared the exact route/profile/count/browser/problem assertions and example checks.

Metadata released its exact `afea9643` harness plus four unchanged IGDB helper/test
leaves. These are adopted with the reviewed opt-in inventory patch. Node syntax,
16 smoke/evidence tests and seven adopted IGDB fixture tests pass. The IGDB
application journey has not run here and still requires its M4 runtime predecessor.
All four adopted helper/test leaves match exact base blob identities.

Implemented receipt gate: strict ordinary-browser validation now matches actual
default output (three artifact hashes and mandatory typed M3 policy evidence).
Optional typed inventory evidence is required for the inventory-only proof.
It reuses existing streaming hashing, gate execution and receipt publication.
The authored command is `cargo xtask test access-client-inventory`, with its own
opt-in runtime receipt. Negative tests cover missing/false/unknown evidence,
changed artifacts and wrong process arguments. Unknown-field rejection,
clean-source checks and the Tauri deferral stay.
This command proves only inventory runtime; it does not replace canonical PR gates.
Metadata released the command-dispatch files and hashing-helper visibility change.
The integration is authored and independently reviewed. The corrected full
task-runner suite passes **109/109**; strict API/xtask all-target/all-feature lint
passes. The new fixed-digest nested-sibling regression covers Python component-wise
path ordering. Independent review closed that final finding. Missing/false/unknown
receipt fields, changed artifacts and wrong process arguments fail their tests.
The three artifact hashes bind observed files, not compiler provenance; build
daemon/CLI/web from the recorded clean source and retain that evidence separately.
No new package or hashing implementation was added. C1's eight gates are unchanged.

| Latest receipt | SHA256 |
| --- | --- |
| inventory-xtask-reviewed.log | 8bbae7a29e0835b5b17c093e8588e9414d72cfa087709b1925c30fc14ad7d63f |
| inventory-api-xtask-reviewed-clippy.log | 4a8cec51e1ebcae6dbe7837c8ac5db9559578f1f0eb1ed970e7260978365bdce |
| inventory-adopted-igdb-fixture-tests.log | 720b85dd953cc219fddaf4dd5d87e2b2f5163a4314458efb4ca988760b03f34b |

Next: coherent source review/commit and clean-source artifact preparation, then
the real ordinary-browser inventory gate. Capability activation, final combined
PR gates, exact CI/merge and Metadata handoff remain open. The integration is
still uncommitted; no runtime or full-C2 claim follows from verifier tests.
All Access workloads are terminal. Metadata's subsequent finite Rust `-j2`
Search source-limit window also completed on 2026-09-09 and is now released to
Access. No resource wait remains for the next coordinated build/browser run.
Metadata also owns Search-only contract/API/Desktop leaves and the two approved
false-argument test-call updates. The new narrow release permits only its Search
parser guard in SDK transport, Discover source-panel status and focused Search
tests in its isolated checkout. Generated Search diffs require an exact base/head
handoff and combined-source regeneration by the integration writer; never copy
older generated files over Access outputs. No inventory, account, host, registry,
migration18 or archive7 ownership transferred.

The integrated three-file patch sent to Metadata is
`/home/ryan/.gstack/projects/Scrobble-dev-Fasti/reviews/c2-i-browser-runtime-integrated-20260908.patch`,
SHA256 `fec5a430d1da887ba6eabc327beb749d2e521bf631f8c303930b6dc2cf37f8d6`.
It is an overlay against exact `afea9643`, not a merged handoff. Prior fixture
investigation bookkeeping is closed; its finding was logged through the installed
gstack repository after the skills alias lacked `lib/jsonl-store.ts`.

### Historical verified progress: 2026-09-08T22:37Z

The table below is the preceding checkpoint, retained as dated evidence.

The three UI findings below are fixed. The corrected browser batch passed all
10 tests, including all32 A+C theme/viewport cells. Independent finish review
verified the source, four refreshed screenshots and log hash: **SHIP, inventory
UI finish gate only**. No full conformance or delivery claim follows.
Log `inventory-ui-reviewed.log` SHA256
`5074268316950f3264704a89b9b87d18e383fa2aa089dc3c650a6dc4c521966d`.

| Gate | Evidence now | Still open |
| --- | --- | --- |
| UI finish | 10 tests passed; independent bounded review clear | Full accessibility and final combined-source evidence |
| API | 114 unit and 7 integration tests passed; one existing live-provider test ignored | Final combined-source checks |
| Governed examples | 29 tests passed; OKF validation passed | Three tests added afterward have not run |
| Authored/generated contracts | 59 passed, 5 failed | Correct exact generated route/profile/count/security assertions; do not relax checks |
| Ordinary-browser inventory | Additive patch proposal prepared against Metadata `afea9643`; applicability check only | Commander review, integration, strict receipt tests and real runtime proof |
| Delivery | Local source remains uncommitted and Reserved/Guarded | Activation evidence, canonical gates, exact review/CI, merge and handoff |

The HTTP fixture now covers cookie-only success, malformed/absent/unknown/duplicate
cookies, exact Host admission, bearer-only denial and typed private/no-store
errors. It revokes the actual session after existing Metadata assertions and
proves retained-cookie inventory denial. The initial failure exposed a fixture
clock error: its synthetic session was created two seconds in the future. Moving
the synthetic ceremony start ten seconds into the past fixed the fixture; no
production clock, cookie, session or authority rule changed. Preserve red logs.

The inventory example uses the supplied root's generated schema and production
operation. Exact calendar/epoch checks reuse AJV. Only the schema-validated public
credential epoch field is exempted from secret-name detection; other secret checks
remain intact. Generation wrote 22 artifacts. Metadata released narrow C2 changes
in the example, OKF and generated-contract validators, their named tests, lifecycle
vocabulary and strict ordinary-browser receipt verifier. Preserve frozen legacy
UAT, all M4 entries and strict validation. No missing runtime proof may be waived.

Next, in order:

1. Update the released generated validator's exact inventory route, profile,
   counts and browser-security assertions; run all 32 example tests and contract tests.
2. Review the three-file browser patch against exact Metadata `afea9643`, not the
   older Access harness. Metadata integrates the reviewed patch. Add strict typed
   receipt checks without removing unknown-field rejection or M3/M4 coverage.
3. Run opt-in real TrailBase login, cookie-only inventory and second-session
   revocation proof. Then finalize only the evidenced inventory capability.
4. Run combined canonical/delivery gates, review the final source, commit coherent
   slices, merge when green, and send exact commit/tree/schema/archive/file handoff.

Resources: all Access test processes are terminal. Metadata now owns the next
finite Rust-only `-j2` window for native Music tests/Clippy; Access performs this
documentation save without builds or listeners. Coordinate the next window.
Migration18 and agreed C2 shared files remain Access-owned. Archive7 stays frozen.
Metadata also has permission for only the adjacent Music archive-test include in
`restore_import.rs` plus its disjoint test leaf, not production restore changes.

Receipts under `/mnt/secondary-ssd/cache/tmp/fasti-c2-inventory.LeBUwY`:

| Receipt | SHA256 |
| --- | --- |
| inventory-api-final.log | 545210a66c4df85112e4f3065228c408777b3972b2055cad76b72897d458690f |
| inventory-example-tests-corrected.log | a5a034e4b419010ad412cb4c59dc2801327da123ed24238d00a694ef505de050 |
| inventory-okf-check.log | c12fe71ed0fb0dd812969b62fa821fef9da24dc3c7ca7d847d9da4560f19b2c3 |
| inventory-contract-tests.log | 4b02c4527681a6783560783dd9a78a94a56a3f02b9972b07e7fb7b9082ad61dc |

These receipts describe different dirty-source checkpoints, not one final head.
The older sections below retain historical failures and ownership snapshots.
Current state and next actions above supersede their pending-work wording.

### Historical A+C review checkpoint: 2026-09-08

Source remains dirty and uncommitted at base `d274e037`; no delivery or merge is
claimed. UI/web typechecks and Tabler policy passed. The distinct generated
`ui:access-client-inventory` witness preserves C1's projection-only witness;
its cross-capability regression passed 1/1. The real authorized Store demotion,
old-session denial and new member-session visibility regression passed 1/1.
Its test-only recent-proof prerequisite is not genuine fresh authentication.
Strict Store/xtask all-target/all-feature lint passed; generation wrote 22 artifacts.

Latest browser batch: **8 passed, 2 failed**. Six functional tests cover bounded
paging/exact values, retry, cancellation, authority loss, sign-out and an old401
after a newer identity. Light and forced-colors each passed eight A+C viewport
cells. C dark/night contrast fails at 3.97:1. Fixtures do not prove actual human
sign-in, full accessibility conformance or production readiness.

Independent finish review: **FIX**, with three bounded changes still unapplied:

1. Skin the first-run inventory card with the existing archive surface token.
2. Use `Inspect ${name ?? client_id}` as the disclosure's accessible name.
3. Change the first-run phrase `registered clients` to `new client registration`.

Next: apply this batch, update matching test selectors, rerun the same ten tests
to a new evidence directory, refresh A+C screenshots, and ask the same reviewer
for a verdict. Then complete combined checks and exact delivery gates. Retain
all failed logs/screenshots; the filename `inventory-ui-final.log` is not a pass.

Resources: Metadata may run Rust-only -j2 application/runtime/API checks now,
without frontend builds or listeners. Access retains the next finite browser
recheck. Coordinate any new Access Rust build. Release the full browser/daemon
window after cleanup, not after C2 merge. Shared files and migration18 remain
Access-owned; archive7 remains frozen. Metadata's only additional two-leaf release
is the UserAgentOnly initial-state predicates and tests in API/Tauri providers.rs.

Receipts in `/mnt/secondary-ssd/cache/tmp/fasti-c2-inventory.LeBUwY`:

| Receipt | Result | SHA256 |
| --- | --- | --- |
| inventory-authorized-demotion.log | 1 passed | 92c2143625836dbe756c3894ee0ddb750bc85fd9afb76ce2363e72ed4ebd9f53 |
| inventory-ui-binding.log | 1 passed | 991a1074040555b8e6b0393de3ffc44b3a830dffef1afe605225730a60d23eec |
| inventory-ui-store-xtask-clippy.log | passed | e448f1ac96efa71b1f65d06b2a352daf60a0bf0939c6aa0b6b6765d11ff17ed1 |
| inventory-ui-final.log | 8 passed, 2 failed | 6806864cd950998dabcdaede4c5f2349ad4abee3d364177c9ad3b4e5e2164da7 |

### Historical A+C source and resource handoff: 2026-09-08

The shared Tabler inventory snippet is authored in `account-security-view.svelte`
for A (permanent settings) and C (first run). It uses explicit bounded reads,
native details, paging, cancellation and live-parent identity invalidation.
Existing Connections controls remain; inspection does not complete device setup.
This patch is unverified: formatting, UI/web typechecks, browser tests and the
inventory-specific generated UI witness remain open. Prior checks below do not
cover this view. No production code changed during this context-save.

Independent read-only review identified the next checks: use live parent
authority, guard success/error/finally against stale requests, invalidate on
unmount even if the host ignores abort, and retry the failed page exactly.
Also add the real Store demotion -> changed authorization epoch -> old-session
denial -> new member-session visibility regression. A test-only recent-proof
precondition must not be represented as genuine TrailBase fresh authentication.

Metadata reports its finite window terminal and released at local commit
`2b17511081a56c54025d4228f8528ad0eb693a27`, tree
`0557c2780783ac2b4309ac90825ad2361ab1f673`. Access takes the next finite host/A+C
verification window. Metadata's tests are owner-reported, not Access test evidence.
This releases resources only: Access retains migration18 and the agreed shared
files; archive7 remains frozen. No Metadata merge or new migration allocation
is implied. The commander's roadmap and canonical progress sections are updated
separately in `/home/ryan/code/fasti-access-c3-final-eof`.

Next: close the view review and tests, add the distinct inventory witness without
weakening C1's projection witness, run combined focused/canonical gates, commit
coherent slices, review the exact final source, then deliver and hand off the
verified merged commit/tree and explicit schema/archive/file disposition.

### API/SDK integration

Authored inventory DTOs, fixed-listener route, OpenAPI, generator and SDK method
are implemented locally. Canonical generation completed (22 artifacts) and SDK
build passed. All 68 SDK tests passed, with no failures or skips, including the
loopback Rust conformance fixture. The five inventory tests cover exact time/
epoch, cookie-only transport, Unicode names, request-bound descending pagination
and asynchronous request mutation. Both SDK source-review findings below are
closed by red-before-fix/green-after-fix regressions and independent source review.

Existing real-bootstrap cookie, exact-listener route exclusion and OpenAPI tests
each passed. These are focused checks, not current whole-programme verification.
Strict all-target/all-feature API/xtask lint passed (session63852 terminal0).
At this earlier checkpoint the finite Access build window was released to
Metadata; the latest resource state is recorded above. Shared-file ownership
is retained.
No full C2-I runtime/UI, canonical PR, CI or merge proof exists. Registry stays
Reserved/Guarded. Host forwarding and generated query/response host types are
now authored. After that addition, six focused inventory/host SDK checks passed,
including fixed browser origin, no scoped bearer and cancellation. This no-server
run did not reclaim Metadata's build/browser window. UI/web typechecks and a
full-suite rerun after host/view integration remain open. A+C view wiring was
subsequently authored as recorded above; its distinct generated UI witness
remains open. Preserve C1's projection
witness unchanged. The real demotion/epoch/new-session regression remains open.

Logs in `/mnt/secondary-ssd/cache/tmp/fasti-c2-inventory.LeBUwY`:

| Receipt | SHA256 |
| --- | --- |
| inventory-sdk-full-final.log | 17b432316ddcb06c81a406eec507ab65dfc53879ba948132bfba6bec2b68389a |
| inventory-api-cookie.log | 9d39fe95a712b7a6d6bacdcb5cbb691cf44ac81e93c7014aa5f24447e5f977a6 |
| inventory-api-listener.log | 4e72b7f6adfd57e81aee1e96af4a93c8d37090f29a521cab612bcbe154d67979 |
| inventory-api-openapi.log | b05fa8df45620b1e310333c2668e17d5dd2c8b36cf9f4ef1ea4e1090e2249178 |
| inventory-api-xtask-clippy.log | 57df9e282cadbb17bc21d5012364849e09f752b564e0a30702cb5eb55ba65c6b |
| inventory-sdk-host-focused-final.log | 7895d59e5fe79be219f0d6a9127308be8cdf360417b2e4aeee1e9d96302c4267 |

Preserve earlier red SDK logs and failed generation receipts. Generation exposed
the omitted expected profile and documented501 response; wider SDK tests exposed
missing C2 discovery enums and stale counts. These are corrected, not waived.
No dependency or lockfile change was needed. The offline frozen install only
populated this checkout's dependencies. Formatting and diff whitespace checks
passed before the later Svelte patch; they do not qualify that patch.

### Prior bounded-read checkpoint

The browser inventory read now reuses `AccessAdministrationPort`. It authenticates
the current browser session at transaction execution time, reloads workspace role,
filters owner/workspace before indexed pagination and commits activity updates.
Five focused Store tests passed: browser authority/activity/expiry, mixed-precision
paging, extreme years/leap seconds, full index seeks and corrupt-row rejection.
Strict all-target/all-feature Store lint passed after two test-layout corrections;
the final focused rerun passed five tests (517 filtered; no failures or ignores).
Receipts under `/mnt/secondary-ssd/cache/tmp/fasti-c2-inventory.LeBUwY`:
`inventory-read-tests-final.log` SHA256
`5032e3d16e4ff9a3447fe6a61c8f292612465058ad5f031a993309b99f2c37a2`;
`inventory-read-clippy-corrected.log` SHA256
`88e871a1fa3e3951125ffb9383daf4414526a266bd8113a2d47d5b170c92c6d0`.
Those read checks predate the API edits. Earlier compile/lint failure
logs remain alongside the final receipts. Access then owned the finite
API/generator/SDK resource window; shared-file ownership is separate.

At that checkpoint the capability was `Reserved/Guarded`, not delivered or
activated, and canonical generation was pending. The newer API/SDK results above
supersede that pending status, not the remaining UI gate. The existing generated
`ui:account-security` witness is projection-only. Add an inventory-specific witness
with real A+C behavior during UI integration; do not broaden that check into a bypass.

Metadata explicitly released the existing `AccessAdministrationPort` forwarding
method in store/access.rs and the single `CapabilityBody::C2` Reserved match arm
in application/problems.rs. No broader behavior or file ownership was released.
Independent source review found one corrupt-row error mapping issue; it is fixed
and covered by the oversized timestamp regression. No other actionable finding
was reported. These checks are not full C2-I, canonical PR, UI or merge proof.

### API checkpoint: 2026-09-08T21:19Z

Two focused API tests passed (session87713 terminal0; 113 unit and seven
integration tests filtered): exact bounded cursor parsing and lossless DTO time/
epoch projection. Log `inventory-api-tests-corrected.log`, SHA256
`8ae6f46800c57ecd5d3132fb9fdc836c021ee627096beb8e48e6bb4d794f3779`.
The first API compile failed because a test assertion required Debug on the
private HTTP error. Only the assertion changed; retain the failed log.

SDK source-review findings at that checkpoint (now fixed and verified above):

1. Bind response validation to the requested page limit and cursor. Reject
   oversized or non-advancing pages using exact year/tail/client-ID ordering.
2. Match Rust name-boundary Unicode White_Space semantics. JavaScript trim()
   incorrectly rejects a valid leading U+FEFF name; add a regression.

Next at that historical checkpoint: fix these two findings, run existing real-bootstrap/cookie and exact-listener
tests, regenerate contracts, build/test SDK, then complete host and Tabler A+C
wiring with an inventory-specific witness. Keep the real demotion/epoch/new-session
regression and all exact delivery gates open. Metadata requests its next finite
Store/provider/browser window after Access's terminal resource release; do not
confuse that release with shared-file or migration ownership.

## Prior persistence checkpoint: 2026-09-08T20:42Z

- Migration18, client metadata, node classification and historical archive7/schema17
  acceptance are implemented locally and uncommitted. Published migrations1-17
  and archive7 wire shape remain unchanged.
- Corrected full Store run: 511 unit and 3 integration tests passed; zero failures;
  six unit tests and one doctest ignored. Log:
  `/mnt/secondary-ssd/cache/tmp/fasti-c2-inventory.LeBUwY/store-tests-corrected.log`,
  SHA256 `382b9744fd7440d55aacfabbc9b0d6caef3dc800b50ef5422a82605bb1ea4e1b`.
- Preserve the initial ten-failure log. Six failures exposed historical seconds-only
  timestamps; four required owner-approved fixed-v17/current-version test separation.
  Focused schema rerun: 51 passed. Strict Store Clippy passed before the final
  Metadata test correction; rerun it on the final slice before claiming exact lint.
- Next: final slice review/lint and coherent commit; then transaction-authorized
  bounded read, public contracts/API/generated SDK, Tabler A+C and delivery gates.
  No inventory capability, C2 completion, push or merge is claimed.

This is the written execution gate for the inventory capability, not another
premise review or completion of C2. The approved whole-C sequencing amendment
remains at `docs/plans/fasti-access-c-delivery-reaudit.md` in the Access commander's
preserved documentation worktree until that amendment is delivered separately.

## Exact base and ownership

PR128 merged at `d274e037daa6566e223f8309a7f314fee5b60384`, tree
`b9da6d6358082d5afbcb22a26a2f513dfc656318`. Source is schema17/archive7.
Metadata explicitly released the C2-I minimum writer report on 2026-09-08,
SHA256 `fb3d911ff26897e8857d1e2f6c844cc809c0f141ade3921b4aa9058da8047d30`,
plus store/access.rs's two node bootstrap/recovery inserts and focused tests,
and a narrow AGENTS QA note. C2-I owns append-only migration18. One integration
writer owns these files. Metadata retains all excluded files. It released a
finite focused Store verification window during implementation; coordinate
each later build window rather than treating that as an ongoing reservation.

Metadata also released only `metadata_policy_migration_tests.rs` for separating
its fixed v16-to-v17 byte/schema assertions from current-schema restore/reopen
checks. Original predecessor fixtures, fingerprints and every metadata
byte/policy assertion stay. No Metadata production leaf is released.

Never amend migrations1-17. Keep archive7 wire/entity shape and add exact
historical archive7/schema17 fingerprint acceptance. Capture that digest with
the existing schema-fingerprint owner; do not infer it from a commit/tree hash.
The exact predecessor regression captured
`sha256:7b481b2bf2a23ad261884c171710c7ceece6bd70312d8dca6a034a4f830c4649`.
The source now pins it and preserves the historical acceptance branch; full
restore regression results remain a separate proof obligation.

## User outcome and contract

One real `GET /api/access/v1/clients` operation, capability `access.client.list`,
operation `list_access_clients`, generated SDK `listAccessClients`. Inspect the
same bounded inventory in permanent Account and security (A) and first run (C).
B supplies shared details, not a new destination. Use Tabler first; preserve
existing Connections controls and distinguish inspection from unavailable later
administration. Do not mark the full Devices and clients setup step complete.

Reuse `ApplicationClient`, `AccessInventoryPage<ClientId>`, browser read-boundary
validation and the existing client store. Inside one transaction authenticate
the current session at trusted execution time, reload membership/role, then
apply workspace and ownership before cursor and LIMIT. Administrators inspect
all workspace rows; members inspect only their own. Ownerless historical/system
clients are administrator-only. No recent-auth or CSRF mutation envelope for a
read, no machine/PAT bearer substitution, no new listener or transport.

Default page32, maximum100, read at most limit+1; cursor is the last returned
creation time and typed client ID. Preserve revoked and epoch-zero truth without
claiming credential validity. Return no secret/digest or credential/grant graph.
Private no-store responses; exact fixed C1 route exposure only.

Wire precision: inventory times use a bounded canonical UTC-microsecond string,
not strict RFC3339 `date-time` or JavaScript Date. Preserve signed/expanded years
and second60 with matching Rust and generated validation. Cursor strings echo
unchanged through URLSearchParams, including encoded leading `+`. If exposed,
credential epochs use canonical decimal strings through i64::MAX, not lossy
JavaScript numbers. These are new DTO spellings, not narrowed domain types.
Metadata explicitly released `packages/sdk/src/transport.ts` for C2-I imports
and `listAccessClients(query/options)` only, using the existing cookie GET,
generated parser, URLSearchParams and cancellation. Generic transport policy,
bearer/CSRF and Metadata/Search methods remain untouched.

## Persistence and ordering

Add confidential/integration defaults, nullable subject owner and name, exact
classification and 128-byte name constraints. Backfill only same-workspace
node_state.client_id to first_party/node. Preserve lifecycle, epoch, time and
all authority. Future bootstrap/recovery inserts explicitly select node; generic
metadata inserts keep safe defaults. Restored shells remain ownerless/unnamed
confidential/integration at epoch0. Immutable client identity/ownership/classification
must not block existing lifecycle/epoch changes or provisional recovery deletion.

Plain timestamp TEXT is not chronologically ordered across accepted signed and
expanded years. Do not narrow dates or collapse leap seconds into epoch micros.
Use SQLite's indexed generated-column mechanism for a numeric signed year and
the UTC-microsecond month/day/time tail. Preserve published seconds-only UTC
storage bytes too: the computed tail appends zero microseconds without rewriting
created_at. Admit only exact Micros/Z or Secs/Z storage round trips; wire cursors
still emit and require Micros/Z. Apply the same tuple
to the cursor. Verify exact admin/member tuple seeks and full Chrono ordering
before freezing the read. No hand-coded collation, whole-inventory sort, offset
paging, stored duplicate timestamp or new dependency.

SQLite primary references: [generated columns](https://www.sqlite.org/gencol.html)
and [row-value keyset comparisons](https://www.sqlite.org/rowvalue.html).
Context7 was queried for these mechanisms. The existing lock pins libsqlite3-sys
0.38.2, bundled SQLite3.53.2 source ID
`d6e03d8c777cfa2d35e3b60d8ec3e0187f3e9f99d8e2ee9cac695fd6fcdf1a24`.
Scratch query-plan evidence does not replace tests against that bundled engine.

## Execution and proof gates

1. Schema/restore: capture predecessor fingerprint; implement migration18,
   backfill, node inserts, immutable metadata and indexes. Test exact17-to18,
   fresh-equivalent fingerprint, rollback, byte-name bound, classification,
   same-workspace node matching, generic defaults, lifecycle/epoch updates,
   recovery deletion and authority-free archive7 restore. Do not deliver a
   schema bump without its historical archive acceptance.
2. Read: application port/result and transaction-bound bounded store read;
   direct member/admin/cross-workspace/ownerless/demotion/stale-session tests,
   exact cursor boundaries, equal times, extremes/leap seconds, malformed stored
   values and indexed deep-page seeks. No fabricated browser authority.
3. Public surfaces: author contract/capability/DTO/route before generation;
   regenerate SDK/OpenAPI/capability/problem/OKF examples deterministically.
   Read-only operation has no new async event or JSON-LD identity vocabulary;
   document applicability rather than inventing them.
4. UI: existing host wiring, Tabler table/details, loading/empty/error/next-page
   states, cancellation and identity-change invalidation. Verify actual A+C
   behavior, keyboard, reflow, themes, accessibility and no secret leakage.
5. Delivery: applicable Ponytail/source/security/DX/UI reviews, focused checks
   and unchanged canonical PR gate; exact-head CI, review reconciliation and
   verified dev merge. Packaged Tauri auth remains deferred separately. No release.
   Hand Metadata the exact merged commit/tree and explicit file/schema/archive
   disposition; do not infer another migration number before coordination.

Rollback: preserve the old root backup and receipts; no reverse migration or
old-binary opening of an upgraded live root. Keep all C2-M/P and C3/other MVP
requirements active. This slice issues, rotates and revokes no credential.
