# C3 final physical-EOF failure regression

Status at the 2026-09-13 checkpoint: FOCUSED_DELIVERY_CHECKS_PASS;
FINAL_CANONICAL_AND_REMOTE_DELIVERY_PENDING.

## Delivery reconciliation: 2026-09-13

PR #136 head `c4561eddcfe00228ddb507acdbef510ed7ddd7ac` was merged with
`dev` commit `f46687cd9425ae725c5a003e5454f18b9f7fab3d` in a separate clean
delivery worktree. The signed-off merge is
`22d6f327fee22e15887ce9fca8ee200e6362c51c`, tree
`15a92f61ebc23edcec6448bc8f4a6dfd80e6ecfc`. The original dirty programme
checkout was not changed. No rebase or force update occurred.

The diff against that `dev` commit still contains only the original five EOF
files. Framing adapter, tests, isolated lock and workflow bytes match the PR
head. The merged base supplies the existing `js-yaml` 4.3.2 override and lock
resolution; this slice adds no dependency or advisory-policy change.

Fresh focused checks on the clean merge above, before this documentation update:

| Check                                               | Result                                                                                    |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Full framing debug and release suites               | Each 22 unit tests and two compile-fail doctests passed; zero failed, ignored or filtered |
| Rust 1.97.1 formatting and strict all-target Clippy | Passed                                                                                    |
| `cargo deny` 0.20.2 licenses, bans and sources      | Passed with existing policy warnings                                                      |
| `cargo audit` 0.22.2, isolated lock                 | Passed; 146 dependencies, 1243 advisories, no reported vulnerability                      |
| `pnpm audit`                                        | Exit 0; two existing ignored high advisories; no unsuppressed finding                     |
| Existing patched-dependency tests                   | All six passed after frozen offline dependency preparation                                |

The RustSec database was `b50980aad8b8f14f77e25a97b32dd94bf008b0af`.
Framing used one build job, one test thread, offline locked resolution, system
CC, the package-local target and the documented four unset SODIUM overrides.
Both debug and release native outputs identify static sodium from their own
`out/installed/lib` and `out/source/libsodium-stable` directories. These are
functional qualification checks, not a memory or production-crypto result.

Raw logs remain in the delivery worktree's `.gstack/reviews/`, including
`eof-delivery-debug.log` (SHA256
`b8ef7d3b74b7c33c8f2987a41f33c8dc84bdf1143a7ff97a119762c4dab2b3cc`)
and `eof-delivery-release.log` (SHA256
`5d31ee1e0eddda3338790e065eff709be4f37a0d69d176393bb27080a162aa30`).
The initial local Cargo-wrapper target error and missing-node-modules test
failure remain retained; explicit target configuration and frozen offline
installation resolved those preparation failures without source or lock edits.
Markdown formatting warnings in the unchanged README and AGENTS also reproduce
on the merged base; the edited gate has its own formatting check.

The final clean delivery commit still requires its canonical PR gate. That
receipt must identify the actual tested commit and tree; neither the focused
merge-tree results nor the historical receipts below replace it. Hosted checks
must cover the submitted head, and review disposition, merge and merged-tree
verification remain required. This checkpoint does not claim those later gates.

## Historical delivery checkpoint: 2026-09-08 and 2026-09-09

Constructor PR #135 merged into `dev` at
416843b40f6112e8498e9da976d48041b54886d5, tree
06e8959ccaec210a9c765264ce5572240faf7588. This EOF branch is reconciled onto
that merged parent. Its pre-documentation-correction tree
681b31e2f4d944518cfb02e3ef19966a1b82984c is byte-identical to the clean tree
that passed the canonical PR gate at 2776704a. The canonical command exited 0;
its retained log is 2026-09-08T16-59-23-260Z-tests-1667901-d1238422.log,
SHA256 da45c195d4300f3c93876fe76cc2c688342f77e769b33ce355794d8e07af00d4.
That clean-source tree's isolated dependency policy and advisory checks also
exited 0 without a new suppression; existing policy warnings remain recorded.
The later clean head `2f06571a52622081b11b666165edf951c3aa72ae`, tree
`73006096022f072df0c2b5175be1f0537564e962`, also passed a separate canonical
run. Its retained log is `2026-09-08T17-21-20-064Z-tests-1760842-d1238422.log`,
SHA256 `9f17c83d4b8f4b0f977c955a9e33cc49d28682e1bd8d885d064f739cff4b8384`.
The evidence record reports exit 0 and a clean source tree. The earlier wording
that this tree had no canonical run was incorrect. This is historical evidence;
it does not describe the later PR documentation commit or the delivery merge.

That documentation correction labelled the earlier checkpoints below as
historical. Source, adapter and dependency inputs were unchanged. The
`access-c3-signing-qualification.yml` workflow input changed with this EOF
regression: its Framing expected unit-test count moved from 21 to 22; the
documentation correction left that workflow change unchanged. Final
documentation verification, hosted checks and merge were still required then.
No complete-C3, production-crypto or packaged-authentication claim follows.

## Historical preparation and verification bases

Initial stacked source base: constructor regression
10b8f7a589c3912f53b5ebba8b345f1ed0c27659, tree
f48d22cfe571647c0cd769765b8b98ffa951d374. That base is not merged yet.
This separate worktree must not change the tree under canonical verification.

Pre-merge verification base: constructor documentation correction
b0ea74cb190a6415ebc26adacb503de07d7f77a9, tree
06e8959ccaec210a9c765264ce5572240faf7588. Reconciliation preserves its corrected
constructor wording and plan spacing alongside this slice's 22 unit tests and
two doctests. The test, adapter and dependency source remain unchanged. The
constructor PR is still unmerged; independent verification can proceed in the
released local resource slot, but delivery remains dependency-gated.

## Need and ownership

The framing README explicitly leaves final physical-EOF read failure untested.
Archive validation and authenticated Final do not replace physical EOF. Close
that existing C3 qualification gap with one real-behavior test, not a new reader.

Worker owns only qualification/access-c3-framing/src/tests.rs and its README.
Commander owns this written gate, AGENTS and the existing CI count field.
Independent review follows before tests. No production adapter, dependency,
lock, fixture framework, schema, contract or M4-owned shared file change.

## Implementation and verification gate

1. Trace FrameReader::finish, physical_eof, FaultSource and existing archive
   helpers. Reuse ArchiveWriter and append_fixture for valid archive bytes.
2. Inject the existing source error at ciphertext.len(), after all frame bytes.
   Prove validate_archive succeeds and authenticated Final has been seen before
   finish attempts the failing physical-EOF read. Assert the source coordinate.
3. Preserve ConnectionReset and the exact injected source error. Require
   poisoned terminal state with stream/plaintext ownership released.
4. Move the source failure boundary beyond EOF. Later reads and finish must
   remain rejected rather than succeeding after the underlying source recovers.
5. Change full-suite counts from 21 to 22, keep both doctests and all existing
   failed/ignored/filtered guards. Correct only the now-covered README limit.
6. Independent source and Ponytail review, then full isolated debug/release 22+2,
   pinned format/strict Clippy and applicable policy/canonical/hosted delivery
   gates. No test/build/service starts while the constructor canonical gate owns
   local resources. Bind all actual outputs to exact source and tool identities.

## Evidence and delivery limits

No physical erasure, native-error cleanup, production crypto profile, resource
ceiling, durable publication or authorized restore claim follows. Original
failure evidence stays intact. Packaged Tauri authentication remains deferred.
Preserve the constructor slice and its running evidence; do not repeat a merged
PR or create a duplicate delivery. Integrate this dependent slice only after
review, keeping exact-head evidence current. Do not promote a release.

## Source review: 2026-09-08

One 39-line test uses existing helpers. A direct underlying read after moving
the fault beyond EOF returns 0 before reader/finish remain rejected, separating
terminal poisoning from repeated source failure. Source has 22 unit tests;
README, AGENTS and CI agree on 22+2, without removing any existing guards.
Independent review found no actionable issues. Pinned 1.97.1 formatting and
git diff --check pass. No build, unit suite or runtime gate has run for this
slice; keep it uncommitted until focused verification. No erasure claim.

Reviewed test SHA256:
bb3e716a82d159419c3f8e8c3ca48ecc64a07acdf3f6b9258bbc265129ea4715.
README SHA256:
6b26ad2da740d2d02fc70e2f7d5280c280f560f8061eb27cb52d470fa2c61d85.

## Focused verification: 2026-09-08

After Metadata released the resource slot, actual session 69603 ran both full
debug and release suites. Each passed 22 unit tests plus two compile-fail
doctests, with zero failed, ignored or filtered tests. Formatting and strict
all-target Clippy passed. The complete command group exited 0, and Metadata
received immediate resource release. This supersedes the untested source-review
checkpoint above, not the still-pending delivery gates.

The unchanged reviewed test source used Rust 1.97.1, Cargo 1.97.1, two jobs,
offline locked resolution, system CC and physical temporary storage. All four
SODIUM source/library overrides were unset. A fresh package-local target built
the bundled native source: debug and release build output separately records
static sodium linking, package-local out/installed/lib and
out/source/libsodium-stable paths. No system-library fallback or new dependency.

Raw command log: 2026-09-08T16-12-06-318Z-eof-focused-1521763-debc5bc8.log
in the local gstack project logs directory; SHA256:
43231117f1d43b40d34a4b7b7532fd51aa7f0178f97d78973699bbd8bea0e69f.
Debug binary SHA256:
0a6b91c796ac2abc962911f921d92be758a61137e49e48c5ad6b73f7441a76de.
Release binary SHA256:
c071e6d7861ff3f626073eb4c9ca342b634c5d7ef1f952ac6d119442607c71f8.
Adapter and lock remain unchanged from the source base. Independent correctness
review and additive Ponytail review found no required changes: existing archive,
encryption and fault-source helpers suffice. The unit-test description is kept
separate from the Debug/Clone compile-fail checks.

At this historical checkpoint, final documentation verification, hosted checks
and merge remained pending. No production crypto, physical-erasure or
packaged-auth claim is made.
