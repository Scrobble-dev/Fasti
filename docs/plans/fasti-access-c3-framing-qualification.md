# Framing qualification delivery preflight

Date: 2026-09-05. Status: `QUALIFICATION_DELIVERY_GATE_APPROVED; FRESH_VERIFICATION_PENDING`.

## Decision and boundary

Deliver the already-written `c3-frame-probe-2` as a checkout-local isolated
qualification package. Preserve its adapter, all 20 unit tests and two
compile-fail doctests. This is reproducibility work, not a new framing design,
cryptographic profile selection, production library, authenticated joint backup
or recovery activation. No runtime crate may depend on the qualification package.

The commander has allocated the seven-file set below; M4 acknowledged that
release and remains read-only on those exact paths. Freeze this plan in the
new isolated checkout before implementation. M4 retains shared runtime/schema ownership;
no migration, archive version, root dependency or root lock change is proposed.

## Read-only evidence checked

- Controlling frozen file:
  `/tmp/fasti-c3-frame-probe2-TEPkWF/frozen-fasti-access-c3.md`, sections 3,
  6.2 and 6.3. These define bounded framing, real archive-owner reuse, every
  negative case and the alternative binding's custody/finalization guards.
- Current C2 C3 plan and source packet:
  `/home/ryan/code/fasti-access-c2/docs/plans/fasti-access-c3.md` and
  `fasti-access-c3-crypto-evidence.md`, sections 12–13. The original Alkali
  failure remains evidence, not a candidate pass.
- Complete retained framing adapter, tests, manifest, README and receipt;
  lock pins, archive identities, path references and final result lines.
- Existing merged signing package and dedicated workflow. The inspected
  qualification checkout is clean at `1e80fbb72602faf1e395baa39e3318f9feb4c102`;
  those signing package/workflow bytes equal merge `4bd84a562e60b04c278173529164f06cc41c7753`.
- Framing's original read-only Fasti dependency was
  `94ddc8d170c69b7041930efa15ccbc9fca5b83b1`, tree
  `a0b84bcd8b0327ae617ca44c4fd0db41ba6a74b2`. Its archive source and relevant
  Cargo manifests are unchanged at `4bd84a56`. The only diff under `crates/`
  between those commits is two rustdoc link qualifications in application
  `nuvio.rs`; this is not fresh compilation or test proof.

No tests, build scripts, native measurements, servers or dependency fetches
were executed by this preflight. Cached archive hashes were read and checked.

### Recomputed retained identities

All following hashes match the retained framing receipt/source packet.

| Input under `/tmp/fasti-c3-frame-probe2-TEPkWF` | SHA-256 |
| --- | --- |
| `frozen-fasti-access-c3.md` | `e61c7ce9afd0ed78306848cffdea25178a0d9f341371faef79ab1645e52c8762` |
| `Cargo.toml` | `91510f06386e771eaaaf65ab02f1ea0136a35042dec5c23296f3f9310da49aa5` |
| `Cargo.lock` | `7e60010dccc40d0319180b52cb0faa39ec5193df73b25660996d66258edbeab0` |
| `src/lib.rs` | `e60f1d823c292a5d1b811d6eb5c9d6884b9d101c2f11e0a290163e3a222d87fb` |
| `src/tests.rs` | `22a6d0df376d6f89384c6a6ac95b32af01d6b4ea5199cc3bd55581460adf4e11` |
| `receipt.md` | `8aa417f292a5a20bec152cee054b87e1d63a80859a20896b2db3559e924b7f51` |
| `debug-run-final.log` | `1bb68fceb95b7c42b44dcb24256f11ca123d7f00c87f50fe0132219e24c17341` |
| `release-run-final.log` | `8d65caadd0a757d3285d7b7b0b5f45e957204cf865824db9251cb5175697c624` |

Both old final logs report 20 unit and two doctest passes with zero failures,
ignored or filtered tests. They used Rust 1.96.0, not the proposed current
1.97.1 delivery toolchain. Do not reuse them as new-checkout receipts.

### Original failed-case retention

Retain `/tmp/fasti-c3-frame-probe-hpHQY4` unchanged. Recomputed identities:

| Input | SHA-256 |
| --- | --- |
| `src/lib.rs` | `345b741d6eeb71e9a65b715fcdb62f92ffce540f13af214ecff19ea1d46b1a53` |
| `src/tests.rs` | `afead8dd25eb240d0aa3a1a3e44a29b0163a24a5b012ab501a1cc1f621e495be` |
| `receipt.md` | `6cf89be591d2f9d51224b888dddd00df4c89f38ebaab7386c37b259235e1857b` |
| `framing-run-1.log` | `5e48dbbfe7f53b23a7370f3cbc65e7797cf487192c5aec2d40ed83e9d323566a` |
| `unknown-tag-repro.log` | `553f0765d1cc5c36e1c3bd9c2b23dcbc025ea7127876b8668e9895884895a11a` |

The first full run reports 15 passed and one failed. Its valid authenticated
tag `0x7f` reaches Alkali's unreachable tag conversion and panics. Preserve
the same named rejection test in the new package. Its test-only `catch_unwind`
asserts absence of panic; it is not a runtime panic-recovery strategy. The
test-only native fixture creates authenticated tags; do not move that FFI
into the adapter or remove it to obtain a pass. Do not rerun the known failed
probe merely to recreate already-retained evidence.

## Gate 0 — before any tracked implementation write

1. Commander records the chosen clean base and confirms one writer for the
   exact seven-file proposal below. Current Access publication does not release
   shared files implicitly. Recheck current `dev`; do not silently rebase or
   switch this worktree as part of implementing the plan.
2. Freeze a tracked plan at the proposed path before copying code. State that
   only qualification delivery is authorized and that all production gates
   remain open. Retain the old contract, probes and failures unchanged.
3. Explicitly release the existing dedicated qualification workflow for the
   small two-package matrix extension. If that file is not available, pause
   only CI wiring; do not duplicate it in a second workflow as a workaround.
4. Confirm the manifest-only portability changes and first-party licence
   declaration below. Any required adapter/assertion/dependency change stops
   for concrete source reconciliation before proceeding. Do not relax a test.

No additional product crypto/recovery decision is needed to prepare or verify
this isolated experiment. Production adoption needs the still-unapproved
complete profile and recovery policy; that authority is outside Gate 0.

## Minimal delivery file set

| File | Change and owner |
| --- | --- |
| `docs/plans/fasti-access-c3-framing-qualification.md` | New commander-owned frozen scope, provenance, gates, results and limits. Include enough of the frozen framing contract that the checkout does not require the unpublished C2 plan to understand its tests. |
| `qualification/access-c3-framing/Cargo.toml` | New isolated manifest, adapted only as specified below. |
| `qualification/access-c3-framing/Cargo.lock` | Preserve the framing probe's own resolved graph; do not substitute the signing lock. |
| `qualification/access-c3-framing/src/lib.rs` | Byte-identical retained framing adapter initially. |
| `qualification/access-c3-framing/src/tests.rs` | Byte-identical retained 20-test matrix initially; no absolute fixture includes exist. |
| `qualification/access-c3-framing/README.md` | Checkout-root commands, 20+2 expected counts, dependency/native isolation and explicit non-production limits. |
| `.github/workflows/access-c3-signing-qualification.yml` | Extend the existing owner to run both independent packages using a small matrix; retain signing checks and audit. No general workflow framework. |

Do not modify the signing library, its tests/lock, root Cargo files, shared
AGENTS/changelog, archive code, generator, schema, runtime, API, SDK or UI.
Any later documentation-release edit outside these seven files needs its own
exact ownership release. No version bump or new migration is proposed.

### Manifest changes only

- Keep package name `fasti-c3-frame-probe2`, version `0.0.0`, edition 2021,
  `publish = false`; doctests already use this crate name.
- Add an empty `[workspace]` for isolation, as in merged signing qualification.
- Add first-party `license = "AGPL-3.0-or-later"`, following the repository
  and merged qualification owner. The temporary manifest omits a licence;
  do not mistake this expected metadata repair for third-party licence approval.
- Keep exact `libsodium-rs = 0.2.4` and `libsodium-sys-stable = 1.24.0`, both
  with defaults disabled; keep `zeroize = 1.9.0` with `alloc` as a normal
  dependency because the framing adapter uses it directly.
- Move the test-only `fasti-store` dependency to `[dev-dependencies]`, with
  `version = "=0.1.0"`, `path = "../../crates/fasti-store"`. Only tests import
  the real archive API. There is no alternate archive implementation or schema.
- Copy the old lock unchanged first. Validate with `--locked`; if current
  source makes it stale, retain the failure and review the exact isolated-lock
  delta before accepting it. Never regenerate root or signing locks.

Keep the existing redundant `pending.clear()` after `zeroize()` in this
portability copy. Prior Ponytail review identified it as removable, not a
correctness defect; optional cleanup would obscure byte-preservation evidence.

## Matrix that must remain intact

| Contract group | Retained test evidence (`src/tests.rs`) |
| --- | --- |
| Native version/ABI/nonminimal | line 12 |
| Backup alignment and exact wire equation | line 53: 0, 1, 65535, 65536, 65537, 131072 |
| Single Final record and narrower provider limit | line 69: 1/65536; reject 0/65537; provider 4096/4097 |
| Partial/repeated flush, reserved Final, frame exhaustion | line 103 |
| Real archive finish/flush and exact compressed-byte limits | line 160 |
| Archive success cannot hide physical trailing ciphertext | line 203 |
| Valid encryption cannot make invalid archive publishable | line 220 |
| Wrong key, envelope, header, ciphertext, AAD and purpose | line 228 |
| Missing/truncated Final, trailing, reordered, duplicate, cross-stream | line 261 |
| Unsupported defined tags, empty ordinary and record mismatch | line 350 |
| Authenticated unknown tag must reject without panic | line 385 |
| Envelope/frame lengths and counters reject before body reads | line 411 |
| Exact frame, plaintext and ciphertext caps | line 474 |
| Short/interrupted input, output and flush | line 532 |
| Partial sink error poisons writer without resealing | line 598 |
| Flush error and unfinished Drop cannot publish | line 625 |
| Source error survives, reader poisoned and owners released | line 677 |
| Final consumes state and later operations cannot advance | line 702 |
| Failed reader releases owners before caller Drop | line 739 |
| Extra plaintext after a valid archive is rejected | line 764 |
| Opaque key cannot expose Debug or Clone | two compile-fail doctests in `src/lib.rs` |

The `{}` manifest is a low-level archive fixture, not a canonical joint Access
manifest. The envelope is caller-validated opaque fixture bytes. Input bounds,
Final and physical EOF checks must remain separate from archive validation.

## Gates 1–3 — runnable verification after release

Use the merged signing owner's Rust 1.97.1 toolchain, system C compiler, make
and locked archives. No service, keyring, account, data root or browser is needed.
Run sequentially in a coordinated build slot, not during a resource measurement.
Commands below are the implementation plan, not commands run by this preflight.

```sh
(
set -e
unset SODIUM_LIB_DIR SODIUM_USE_PKG_CONFIG SODIUM_SHARED SODIUM_DIST_DIR
export CARGO_TARGET_DIR="$PWD/qualification/access-c3-framing/target"
cargo +1.97.1 fetch --locked --manifest-path qualification/access-c3-framing/Cargo.toml
CC=/usr/bin/cc cargo +1.97.1 test --offline --locked --manifest-path qualification/access-c3-framing/Cargo.toml -j 2 -- --test-threads=1
CC=/usr/bin/cc cargo +1.97.1 test --release --offline --locked --manifest-path qualification/access-c3-framing/Cargo.toml -j 2 -- --test-threads=1
cargo +1.97.1 fmt --manifest-path qualification/access-c3-framing/Cargo.toml -- --check
CC=/usr/bin/cc cargo +1.97.1 clippy --offline --locked --manifest-path qualification/access-c3-framing/Cargo.toml --all-targets -j 2 -- -D warnings
cargo +1.97.1 tree --offline --locked --manifest-path qualification/access-c3-framing/Cargo.toml -e features
cargo +1.97.1 metadata --offline --locked --manifest-path qualification/access-c3-framing/Cargo.toml --format-version 1
)
```

Each unfiltered debug/release run must independently report exactly 20 unit
and two compile-fail doctest passes, zero failed/ignored/filtered tests. Preserve
failed builds, policy results and assertion logs. Do not assert fresh success
from source hashes, metadata resolution or filtered tests. Build outputs must
stay in the new package's ignored target, not a retained probe target.

Use existing intake tooling/policy against this separate lock and dev graph:

```sh
cargo deny --manifest-path qualification/access-c3-framing/Cargo.toml check licenses bans sources
cargo audit --file qualification/access-c3-framing/Cargo.lock
```

Capture policy tool versions and selected repository policy; make no allowance,
licence-choice or advisory suppression to force success. The audit refresh
requires network; record exact database identity. Reuse the existing registry
archive-integrity verifier only after `cmp` proves the delivered lock is byte
identical to `/tmp/fasti-c3-frame-probe2-TEPkWF/Cargo.lock`. The retained verifier
resolves `Cargo.lock` relative to its own script, not its working directory;
changing the working directory alone does not verify a new lock. If equality
fails, retain the failure and review the exact graph delta before retargeting
the same verifier in ignored scratch evidence. Do not add a second tracked
verifier. Historical 141-archive verification is not a fresh
intake check for an altered graph. Offline Cargo does not sandbox build scripts.

## Native and licence assumptions

Cached package archives and the bundled native member were freshly hashed:

| Component | Exact version / SHA-256 | Declared licence boundary |
| --- | --- | --- |
| libsodium-rs | 0.2.4 / `4b8cd48c80d6c6fa5a4612d242941067219555baea82b0b49c92ea9d8156b59c` | MIT; packaged revision `b3ad9336c0aa6f31eb41fc25431fafdc8e1a7632` in retained source evidence |
| libsodium-sys-stable | 1.24.0 / `72b04bf6da2c98b727af37ab62cb505f4d751b975b034a9b9ad491d333b0564e` | MIT OR Apache-2.0 metadata; not full native notices |
| zeroize | 1.9.0 / `e13c156562582aa81c60cb29407084cdb54c4164760106ab78e6c5b0858cf64e` | Apache-2.0 OR MIT metadata |
| bundled `LATEST.tar.gz` | 1.0.22 snapshot / `b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab` | Top-level ISC does not exhaust embedded notices |

Retain fresh debug/release build output identifying this package's static
`out/installed/lib` and native `out/source/libsodium-stable`. Reject inherited
source/library overrides using the existing four-variable guard, including
empty-but-set values without printing values. Do not infer bundled-source
identity solely from runtime version/ABI. Reject environment-selected system
libraries, downloaded replacement source or a binary fallback in the new build.
Record resolved features; do not enable fetch-latest, optimized, minimal,
use-pkg-config or another feature to make the build pass.

The existing ignored native-notices packet already has 48 selected archive
members, four official reference texts and qualified Bootstrap helper
comparisons. Reuse its provenance; do not repeat the inventory or call it
exhaustive. Per-artifact/platform notice inclusion and licence-alternative
decisions remain separate. Source metadata checks do not provide legal clearance
or authorize a packaged native distribution.

## CI reuse and independent review

After the named workflow release, use two explicit matrix entries (signing,
framing), with distinct job labels and `fail-fast: false`. Keep each standalone
lock, toolchain, native override guard, full debug/release/fmt/Clippy commands
and unsuppressed audit. Preserve signing's existing 9+3 counts. Add the framing
directory and `crates/fasti-store/**` to both PR/dev-push path filters; retain
domain/application/contracts/root-manifest triggers. Do not rename the existing
workflow file or introduce a generic runner script merely for two entries.
Use read-only permissions and existing pinned actions. Workflow syntax must pass
`actionlint`; verify both matrix entries actually run on the final PR head.

Independent review must check: source/test identity; real archive calls and
flush/EOF sequencing; all 20+2 assertions; unknown-tag fixture versus adapter
separation; immediate failed-reader custody and terminal guards; exact pins,
dev-only Fasti dependency and lock isolation; native-source proof; both workflow
matrix entries and path triggers; truthful limits and failure retention.

Run the repository's unchanged canonical PR gate on the final clean commit
through the integration owner, with its existing prerequisites/coordination.
Root tests do not include this isolated workspace. This lane does not install
Tauri or reinterpret any canonical gate. Applicable hosted and merged-tree
checks are delivery postconditions, not inherited from PR 127 or the old probe.
Visual/accessibility QA is not applicable to this headless-only slice; no UI
or conformance result is claimed. Rollback removes the isolated delivery files
and corresponding matrix entry; no product data rollback or probe deletion.

## Concrete blockers and limits

1. **Narrow implementation authority recorded:** the commander owns the tracked
   plan; one assigned worker owns the other six files after the plan is frozen.
   Base: merged `dev` at `4bd84a562e60b04c278173529164f06cc41c7753`, tree
   `fee25a2ddb810ce01acb1d0c0ca87fc9388c1ad0`. Recheck this identity before
   creating the isolated checkout. No M4 shared-file release is implied.
2. **Fresh qualification pending:** no 1.97.1 build, isolated-lock portability,
   current intake, workflow or final-head test result exists from this preflight.
   The manifest's absolute path/missing licence/workspace isolation are concrete
   delivery repairs, not reasons to change crypto or production source.
3. **Production remains blocked:** profile, master-key inputs, complete cleanup,
   startup/native failure, locking, integrated 192-MiB process-tree resource
   evidence, supported platforms, distribution notices, joint manifest, trusted
   disposition/fencing authorities and restore activation are not approved by
   this package. No C3.1–C3.4 implementation is authorized here.

No new framework selection, E1 advisory investigation, secret material,
runtime/schema implementation or Codex Security work is part of this plan.

## Engineering review and execution record

The user delegated ordering and routine implementation decisions. This review
does not reopen Gate 0–10 or approve a production crypto profile. The seven-file
qualification preservation scope is accepted. No new public package, runtime
interface, generic runner, storage model or distribution is proposed.

The isolated checkout was created from locally verified commit
`4bd84a562e60b04c278173529164f06cc41c7753`, tree
`fee25a2ddb810ce01acb1d0c0ca87fc9388c1ad0`. A subsequent remote freshness
check failed because github.com could not resolve. This known base is sufficient
for isolated work; remote freshness and exact merge reconciliation remain gates.

Architecture: existing framing adapter, store archive owner and qualification
workflow are reused. Cargo's isolated workspace and GitHub's two-entry matrix
supply the needed platform behavior:
[Cargo workspace reference](https://doc.rust-lang.org/cargo/reference/workspaces.html),
[GitHub matrix reference](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/run-job-variations).
No new architecture issue was found in this preservation scope.

Code quality: preserve adapter/tests first; make only declared manifest
portability changes. An independent reviewer found one evidence defect:
the retained registry verifier binds its own lockfile. The explicit lock
equality gate above resolves it without a new tracked tool. This is a routine
verification correction under the user's delegated authority, not a claim
that the user individually approved a new recovery or cryptographic decision.

Tests: the 20 required behavior groups and two compile-fail checks above
remain mandatory. The dependency graph is:
```text
exact source + lock equality
  -> current-toolchain isolated debug/release tests
  -> native-source, intake, format and lint evidence
  -> independent final diff review
  -> exact-head CI and merge evidence
```
The real archive success path must still require framing Final and physical EOF.
Retained source tests cover malformed lengths, counter bounds, authentication,
unsupported tags, short/interrupted I/O, partial writes, poisoned states and
terminal reuse. No instrumented branch-coverage percentage is claimed.
Unforced native initialization/provider errors and complete custody/resource
qualification remain explicit limits, not silently passing tests. Independent
source review also distinguished the tested ciphertext-body source error from
an unforced source error during the final physical-EOF probe. No complete
instrumented branch-coverage claim follows from the retained matrix.

Performance: bounded frame and buffer behavior is tested; the 192-MiB integrated
resource target is not qualified here. No new cache, concurrency abstraction,
benchmark or throughput claim. Run the qualification builds with two workers
and serialize them with resource measurements.

Parallel work: the commander remains Access integration owner and framing-plan
owner; one worker writes only the six framing implementation paths; an
independent agent reviews that diff. M4 continues its released domain/shared
work. Final qualification tests precede review and delivery; unrelated Access
CI does not block preparation.

No new TODOS.md format or speculative cleanup task is introduced. Existing
programme planning remains the backlog. Optional duplicate-helper cleanup,
production adoption, packaged distribution and full recovery approval are not
bundled. No Codex Security, Tauri transport or E1 advisory work is authorized.

### Current-toolchain reconciliation

The byte-identical adapter passed the fresh debug and release 20+2 suites.
Strict Rust 1.97.1 Clippy then rejected the remainder expression in
`Limits::new` as `manual_is_multiple_of`; the original failure remains in
`qualification/access-c3-framing/target/clippy-run-1.log`.
The commander authorizes only `pmax % CHUNK as u64 != 0` becoming
`!pmax.is_multiple_of(CHUNK as u64)`. CHUNK is the fixed nonzero value 65536;
the quotient, checked additions, frame reserve and ciphertext cap are unchanged.
This uses the [standard integer method](https://doc.rust-lang.org/std/primitive.u64.html#method.is_multiple_of),
not a new arithmetic helper, dependency, allowance or crypto-profile decision.
Retain initial source hashes and record the final adapter hash separately.
Rerun the full debug/release suites and strict formatting/Clippy after this
one-expression change. No assertion, test count, lock or error path may change.

### Implementation checkpoint

The remote freshness retry confirmed `dev` still at the recorded base; no
rebase or shared-file change was needed. One worker completed the six named
implementation paths. Root reviewed the complete workflow, manifest, README
and one-expression adapter delta. Independent retained-source review found no
concrete defect; no second independent manifest/workflow review is claimed.

On the corrected working tree, full debug and release each passed 20 unit and
two compile-fail checks, zero failed/ignored/filtered. Formatting, strict
all-target Clippy, actionlint and whitespace checks passed. Clean-commit and
hosted gates remain pending; these working-tree runs do not replace them.

Evidence under `qualification/access-c3-framing/target/`:

| Artifact | SHA-256 |
| --- | --- |
| `debug-run-2.log` | `49041a95d0b86695b66110efff51a3658e7f6891bf227a12bb859d01d6323034` |
| `release-run-2.log` | `2dfd86f80180ef564e437835ce6f401e670ce11b2d68ce416fa73d0d7fe785b7` |
| `clippy-run-2.log` | `d7a9cb80688bcf9ba0917e2c175f0c7f1e86642386f94bd506708dfc021a4d6b` |
| `advisories-refreshed-run-1.log` | `64fccde8189332f7036fea2221d78ff3e5d5830183ce658c17e3fe0467c977ec` |

Delivered adapter SHA-256:
`16794e3fb1b5cf4288e2b0d0e5a30207fd458a75f18accc02728425d6e6491eb`.
Tests and lock retain their original byte-identical hashes. The unchanged lock
comparison gates the reused verifier: all 141 cached archives matched, none
missing. Existing licence/source/bans policy passed with existing warnings and
no allowance. Fresh unsuppressed advisory refresh scanned 146 dependencies and
1239 advisories, with no matches; database commit
`5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`. This is not a legal-clearance claim.

Debug/release native outputs select static sodium under the package-local
bundled-source build. The actual workflow guard accepted the clean environment
and rejected each of the four empty-but-set native overrides without printing
values. Initial host-wrapper target-selection failures and the original Clippy
failure remain retained; neither was removed or relabelled as passing.

### Clean-commit gate and documentation release

Clean commit `316d1dca061e3c16ceb0fd9403eb9c886e3944f9`, tree
`1133470ecd0036def4d0359618b4371f481861c8`, passed the complete isolated
debug20+2, release20+2, formatting and strict Clippy gate. Its unchanged
canonical PR run also passed all 27 contract and 11 portable gates; both
receipts bind this clean commit/tree. Evidence ledgers:

- `2026-09-05T20-22-55-894Z-qualification-18464-1a0c1b6a.log`
- `2026-09-05T20-22-56-984Z-tests-18729-d505be5e.log`

The metadata owner explicitly released two additional documentation regions:
AGENTS qualification instructions and one Unreleased CHANGELOG qualification
bullet. The follow-up adds only the framing command/count, discovery link and
non-production boundary, plus this plan checkpoint. No other AGENTS/changelog
content, version, runtime behavior or ownership changes. Existing Fasti version
conventions remain unchanged. This is a factual documentation update, not an
expansion of the seven-file implementation or production authorization.

The package README and plan cover reference, complete contributor commands and
explanation. No product diagram entity moved or changed. Local links across
100 Markdown files passed before the follow-up; check them again after editing.
Qualification evidence remains distinct from full resource, platform, native
notice and recovery approval. Hosted checks and merged-tree proof remain open.

## GSTACK REVIEW REPORT

Late PR130 workflow review amendment: both required root-manifest trigger paths
already exist in pull_request (lines 26–27), so no duplicate paths are added.
The required test-count contract is checked by local reviewers but not yet
mechanically enforced in hosted steps. Add explicit matrix counts of 9 unit
tests + 3 doctests for signing and 20 unit tests + 2 doctests for framing;
reuse one Bash debug/release loop in the existing
job. Preserve raw output and pipefail, require exactly two success summaries
with the frozen unit/doctest counts and zero failed/ignored/filtered cases.
Do not add a separate runner framework, change assertions or rename status jobs.
Verify both actual package suites and mutate retained summary input to prove
missing, ignored, filtered, removed or failing tests reject before pushing.
Completed for the working tree: exact authored workflow step passed both
packages in debug/release (signing: 9 unit tests + 3 doctests;
framing: 20 unit tests + 2 doctests). Raw logs are retained at
`/tmp/fasti-c3-count-workflow-HoCUZw`; evidence ledger
`2026-09-05T21-05-13-089Z-qualification-counts-232841-c047b1a8.log`.
All 18 separately labelled synthetic count/exit sentinels passed, including
Cargo exit 7 propagation. Those sentinels are not native test executions.
Independent exact-delta review found no concrete defect. Frozen counts do not
prove individual test identities or assertion quality; source review remains.

PR130 review reconciliation: native FFI calls now document aligned/capacity,
initialization, lifetime/non-aliasing and wipe invariants verified against the
exact binding and bundled native source. The fixture remains an independent
wire constructor; sharing writer encoding would weaken negative-test independence.
Only safety comments change in the test source; all assertions remain intact.
The advisory matrix now uses the same Detached-signature label as its test job.
README provenance records the comment delta. Independent exact-delta review
confirmed all six fixture call sites, native field initialization and unchanged
assertions. Full debug20+2/release20+2, formatting and strict all-target Clippy
passed for this working tree; actionlint and all100 Markdown link checks passed.
Qualification ledger: `2026-09-05T20-52-42-587Z-qualification-154958-1a0c1b6a.log`.
Commented test source SHA-256:
`2bacef53c8553c3c77def10cf2be5da66bc3da32d96bc5bcb240a9a54790da1a`.
Clean-commit canonical and hosted verification remain required before delivery;
prior source identities and failed evidence remain preserved.

| Review | Runs | Status | Findings |
| --- | --- | --- | --- |
| Scope and architecture | 1 | Clear for isolated preservation | Seven exact files; existing adapter/archive/workflow reused |
| Code quality and evidence | 1 | Corrected | Registry checker requires byte-identical lock proof |
| Tests | 1 | Plan complete; execution pending | 20 unit groups and two compile-fail tests; fresh results required |
| Performance | 1 | Bounded scope | No integrated memory, throughput or platform claim |
| Independent native agent | 1 | Finding incorporated | One verified checker-path issue; no cross-model review claimed |
| Outside CLI voice | 0 | Not used | Optional AGY did not replace independent scrutiny |

VERDICT: ENG CLEARED for qualification delivery only. Production profile,
recovery authority and native distribution remain outside this authorization.
One finding incorporated; zero unresolved decisions within the seven-file scope.

NO UNRESOLVED DECISIONS
