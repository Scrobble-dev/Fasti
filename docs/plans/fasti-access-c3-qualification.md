# C3 signing qualification: reproducible delivery slice

Status: `IMPLEMENTED_WITH_EXACT_LOCAL_EVIDENCE; DELIVERY_GATES_PENDING`, 2026-09-05.
Base: `62e10d2e9bd738ed5da425c008eb839f89cdbea5`.

## Outcome and ownership

Preserve the completed `c3-sign-probe-1` as a repository-local, independently
locked [test package](../../qualification/access-c3-signing/README.md). The prior
probe requires `/tmp` artifacts and an absolute path into an older worktree.
It cannot be reproduced from a Fasti checkout
alone. This slice fixes that delivery gap; it does not repeat framework selection
or approve a C3 cryptographic profile.

The commander owns only this plan, `qualification/access-c3-signing/`, and a
dedicated qualification workflow. Existing production crates, root Cargo files,
schema, registry, APIs, SDK, Workbench, host and portability remain unchanged.
M4 retains v17/archive v7 and shared-file ownership until its exact merged
handoff. No migration or archive version is allocated by this slice.

## Written gates

1. Preserve source: verify prior probe hashes; copy its bounded wrapper and all
   nine unit and three compile-fail tests. Use checkout-relative Fasti path
   dependencies and fixture paths. Keep exact candidate pins and a separate
   workspace. Add no runtime consumer or alternative manifest parser.
2. Verify current source: run the full unfiltered debug/release test suites,
   formatting and strict Clippy with Rust 1.97.1. Each run must include all nine
   unit tests and three compile-fail tests. Retain failures; investigate any
   change needed for current contracts before editing the assertions.
3. Verify dependency intake: record locked graph, exact native identity,
   archive checksums, licence/source/bans checks and raw advisory result. Do not
   suppress advisories or infer packaged native-notice completeness from SPDX.
4. Review and deliver: independent review of the exact diff and test scope,
   dedicated CI coverage, applicable canonical PR gates, and exact-head evidence.
   Qualification delivery is not production C3 approval or C3 completion.

## Reuse and provenance

Source is `/tmp/fasti-c3-sign-probe-pc6cx4` (retained unchanged). Its frozen
contract is C3 plan section 6.4; retained probe receipt records debug/release
9+3 passes against the older source, not this checkout.

| Input | SHA-256 |
| --- | --- |
| `src/lib.rs` | `48db5c9754d4bd7708eb0878b3f49f0c65505a5988bb2aa90c10d6d061a7d758` |
| `src/tests.rs` | `4a2b0c372c09e5a56d75628ccccc8ce1045ea79e0f0cff6d9181fae725040cb0` |
| `Cargo.lock` | `7677ed643879c877cf6899f1c81f666cbafb06c60f93650d65d71435139ee4de` |

Reuse `CanonicalWorkspaceManifestProjection`,
`VerifiedInboundWorkspaceManifest`, `PortabilityLimits` and the checked-in v1
fixture. The 16-KiB message ceiling is a probe bound, not a production joint
manifest decision. Generated and imported test keys are disposable; RFC 8032
vectors are public interoperability fixtures. No real secret or account is used.

The candidate is libsodium-rs 0.2.4 / libsodium-sys-stable 1.24.0, defaults
disabled, and zeroize 1.9.0. Previous source/Context7 reconciliation is recorded
in the preserved C3 source packet on `codex/fasti-access-c2`; package source
and checksum identity must be checked before using the copied code.

## Limits, rollback and remaining programme

No production API, credential custody, recovery activation or UI changes.
Visual QA and accessibility conformance are not claimed by headless tests.
No performance or cross-platform result follows from functional tests.
Rollback is removal of the isolated qualification slice; no data rollback.
The prior temporary probe and its evidence remain recoverable and unchanged.

C3 still needs its approved complete crypto profile, key inputs, native notices,
resource and cleanup evidence, vault/backup integration, independent disposition
and fencing authorities, and exact recovery/activation tests. A signature does
not establish those facts. E0/E1 decisions remain separate and unanswered.
Packaged Tauri authentication stays deferred; Secure cookies remain unchanged.

## Focused implementation checkpoint

The written gate preceded file creation. The library and standalone lock are
byte-identical to the preserved source hashes. Tests differ only in two relative
fixture includes and formatting. The manifest keeps the original package name,
adds workspace isolation, and declares exact-version local Fasti dependencies
as dev dependencies. Root production files remain byte-unchanged against base.

| Check on this checkout, Rust 1.97.1 / x86_64 Linux | Result |
| --- | --- |
| Full debug and release suites | Each 9 unit + 3 compile-fail passed; zero failed/ignored/filtered |
| Strict all-target Clippy; formatting | Passed |
| Dedicated workflow syntax | `actionlint` passed |
| Native override guard | Clean environment passed; each of four empty-but-set overrides exited 1 |
| Licence/bans/source policy, including dev graph | Passed; 121 package notes, six unused-policy warnings, one duplicate-package warning |
| Raw advisory check without refresh or suppression | Exit 0; zero matches in cached database `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5` |
| Registry archive integrity | All 117 registry archives match the unchanged lock; zero missing/mismatched |

The registry checker ran against the preserved lock after both locks were
verified byte-identical. The advisory result is bounded to the cached database,
not a current remote advisory refresh or proof that dependencies are risk-free.
Cargo policy covers metadata, not complete native/source-bundle notices.

Native debug and release build outputs record static `sodium` from this package's
own `out/installed/lib` and `out/source/libsodium-stable` directories. The bundled
archive matches `b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab`.
Release native library SHA-256 is
`82b5cdd0f3980a65ba7abbb82528abee22a612097968d75d84a0ee68e85dfe61`;
release test executable SHA-256 is
`96096bb0ba9db3d1e158b87983bc43096b7a38e33b7769141953325a4b38f637`.
Compiler flags and the host toolchain are not hermetic distribution proof.

Retained intermediate outcomes: the first build was deliberately interrupted
(exit 130) after noticing it selected the old probe's target directory. All
subsequent runs use this package's target. Original source/lock and recorded
release executable hashes still match. The initial formatting check required
formatting the shorter relative paths. The first bans check rejected unversioned
local path dependencies; exact `=0.1.0` requirements fixed it without changing
policy or the lock.

Independent source review found an inherited-environment gap: a same-version
external sodium installation could satisfy version assertions. The documented
subshell now clears source/library overrides; CI rejects them without printing
values. Both the clean and rejection paths were executed. Correctness review
found no other source/assertion defect. The separate delivery preflight found no
workflow/scope defect; Ponytail found no unnecessary abstraction.

The sole implementation commit was amended only to add its required DCO sign-off:
`5b5f3c2f12ffcdbbe55fb21e43d6d07297a1ac7b` became
`4eae91f762ee41990d9ccfe91b43903112791ffa`, with unchanged tree
`2be0a6117aa365379a5557eef90d02e57f63d52e`. Fresh local receipts on that clean
signed-off commit pass all 27 contract and 11 portable gates. Debug and release
each pass nine unit and three compile-fail tests; formatting and strict Clippy
also pass. A separate unsuppressed advisory refresh loaded 1239 advisories and
scanned 121 dependencies with exit zero. Its database resolved to
`5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`; it does not replace the historical
cached-only result above. Earlier reviews retain their original commit identity.

Next: complete documentation and final review disposition, publish the isolated
PR into `dev`, then verify hosted checks and the integrated commit. Later
documentation commits do not inherit an exact-commit canonical receipt from
`4eae91f7`. Existing Fasti version conventions apply; no generic VERSION file or
version bump is required for this slice. No push, PR, merge, production profile
approval, C3 completion or shared-file release is claimed here.
