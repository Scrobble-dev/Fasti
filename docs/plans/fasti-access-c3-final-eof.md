# C3 final physical-EOF failure regression

Status: FOCUSED_GATES_PASS; DELIVERY_PENDING.

Initial stacked source base: constructor regression
10b8f7a589c3912f53b5ebba8b345f1ed0c27659, tree
f48d22cfe571647c0cd769765b8b98ffa951d374. That base is not merged yet.
This separate worktree must not change the tree under canonical verification.

Current verification base: constructor documentation correction
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

Remaining: clean-source canonical and dependency gates, applicable pre-landing
and documentation review, final merged-base reconciliation, hosted checks and
merge. No production crypto, physical-erasure or packaged-auth claim is made.
