# C3 KDF qualification preservation and runner repair

Status: PACKAGE IMPLEMENTATION RELEASED; production profile and recovery remain unapproved.

## Scope and authority

The Commander uses the user's authorization to continue independent Access
work. Preserve the retained isolated KDF experiment and fix two identified
runner defects before making any fresh qualification claim. This is not
runtime crypto adoption, a new account system or a restart of Gate 0–10.

Base: merged dev `3d775bf7af2dd52fffafeaba24ceea22da1cfcc1`, tree
`6ae3c90d9c5eb5ac29dfc6e48fa72ce45e4a498f`. Worktree:
`/home/ryan/code/fasti-access-c3-kdf-qualification`. Branch:
`codex/fasti-access-c3-kdf-qualification`.

The historical `/tmp/fasti-c3-rustcrypto-kdf-bQV2oH` remains untouched.
Its source SHA-256 is
`e7519bd9c331983e5c6011dcff5d430dea5aa89e6f276d9fa3eb729f29a7eb0c`;
its lock SHA-256 is
`21d0999c317d962d7cf7891f6a0e6edbabdf7b7d973bcd1af2ef9b0318a086f6`.
The original successful run used Rust 1.96.0. Its timing and custody results
are historical evidence only, not proof of a changed runner or toolchain.

## Ownership and smallest implementation

| Owner | Paths |
| --- | --- |
| Commander | This plan; final integration, review and evidence |
| Isolated package writer after plan review | `qualification/access-c3-kdf/Cargo.toml`, `Cargo.lock`, `README.md`, `src/main.rs`, `src/tests.rs` |
| Reserved, not released by this plan | Shared root Cargo/lock, schema, archive, API/SDK, host, UI, AGENTS, CHANGELOG and workflows |

M4 retains migration17/archive7 and its shared surfaces. Framing PR130 retains
its existing qualification workflow until its own reviewed integration.
Package work has no dependency on M4; shared workflow/discovery integration
needs its actual owner's release. Do not create a duplicate workflow.

Use a separate empty Cargo workspace, `publish=false`, existing first-party
AGPL-3.0-or-later metadata, and the unchanged exact dependency pins/lock.
No root production imports, library replacement, generic worker framework,
crypto implementation, version bump or migration. Keep Alkali only as the
existing native oracle/hardened-buffer dependency in this isolated experiment;
this does not rehabilitate its separately rejected framing behavior.

## Verified defects and repair contract

Root read the complete retained source and frozen contract. Independent
read-only review confirmed:

1. Warm startup, pipe and numeric parse errors can propagate through `?`
   without killing/reaping the owned child. Cold input writes and `try_wait`
   errors have the same cleanup gap. Reuse the existing kill-and-wait behavior
   through one local owned-child lifetime guard, covering every return.
   Do not kill another task or introduce process-wide cleanup.
2. A successful cold child with missing/malformed stdout is counted without
   validating its `SAMPLE` record. Require exactly one valid record, valid
   numeric fields and RSS within the existing ceiling before counting it.
   Reuse one parser for warm and cold records; reject extra fields/records.

Preserve all existing timing boundaries, nearest-rank indices, eight excluded
warm-ups, 128 retained warm samples and eight fresh-process cold samples.
Do not replace failures, relax thresholds or log derived bytes.

Additional robustness gaps found by review must be explicit: currently
unbounded output reads/channel, absent oracle trailing-output rejection and
untested timeout/crash/enforcement failures. During implementation, establish
the smallest bounded output handling compatible with the fixed protocol;
do not claim complete hostile-child isolation from a parser alone. Oracle
output must be exactly 32 bytes, with EOF checked within its existing deadline.
Bound both record size and queued output. Collection and EOF, including cold
output after child exit, stay inside the existing watchdog deadline. Reader
cleanup must neither leak a detached allocator nor hang on early rejection.
Any change beyond these protocol/cleanup boundaries needs a written delta
review before code changes.

## Runnable checks before measurements

The retained binary has no unit suite. `cargo test` does not run its supervised
qualification. The original defects are source-confirmed, not reproduced RED:
the retained supervisor reaches kernel enforcement and the native oracle before
either path, with no injectable child seam. Do not present a missing-helper
compile error or a rewritten synthetic baseline as reproduction. New focused
tests must exercise the repaired real child/pipe/parser operations and pass;
preserve all actual failures. Independent review accepted this source-backed
baseline plus executable regression evidence only for the scoped runner repair.

- Valid sample parse; missing/extra fields, invalid/overflowing numbers,
  multiple records and over-limit RSS rejected.
- Owned child is killed and reaped after an injected parse or pipe failure;
  successful exit remains successful. Use only spawned test children.
- Missing/malformed cold output cannot be accepted as a sample.
- Bounded protocol overflow/trailing oracle output rejects; timeout cleanup
  has a deterministic focused check where the existing boundary permits it.

Use standard library process, I/O, owner Drop and test assertions. No new test
framework or fake passing cryptographic operation. Supervision fixtures test
the runner boundary; native/RustCrypto agreement requires an actual separate
run and is never inferred from fixture children.

Choose the repository's pinned Rust 1.97.1 for fresh builds. Keep all new
artifact identities distinct from Rust 1.96.0 history. Use an explicit isolated
target, system C compiler, bundled static native source and cleared override
variables. Reject inherited native source/library overrides in hosted checks.
Run debug/release full focused suites, formatting and strict all-target Clippy.
Capture actual counts rather than promising historical 20+2 framing counts.
No assertion or lint allowance may be removed to force a pass.

Fresh intake must check the actual KDF lock: all 67 third-party archive hashes,
resolved features, native source/build identity, unchanged licence/bans/source
policy and refreshed unsuppressed advisories. The original missing first-party
licence failure remains historical. Do not reuse another package's receipt.

## Measurement gate, distinct from builds/tests

No measurement is authorized to overlap another task's browser or build work.
Builds for this task are also held while M4's isolated browser run owns the
local resource slot; source writing and review proceed independently.
Before a fresh run, coordinate an uncontended owned Linux user scope without
stopping another task or changing global system configuration.

Reproduce the frozen `c3-kdf-probe-1` contract: Argon2id version0x13,
65536KiB, three passes, one lane, 32-byte output, fixed disposable 64-byte
passphrase and 16-byte salt. Use an untimed native/RustCrypto agreement check.
Five-second external per-derivation watchdog; 96MiB whole test cgroup,
one-CPU quota, zero swap, CPU0 affinity and core dumps disabled, all verified
from effective kernel state. Include allocation and scratch wipe in warm
timing; include spawn through successful exit in cold timing.

Warm p50/p95/p99 ceilings remain 500/1000/2000ms; all eight cold samples
must be at most2500ms. RSS and cgroup peak/events remain separate metrics.
Retain all failed runs and host contention evidence. Missing enforcement
means NOT_QUALIFIED, not an unconstrained replacement run. No combined
Fasti/TrailBase192MiB, cross-platform, leak-free or production guarantee follows.

## Delivery gates

1. Written plan and independent review, then named package writer release.
2. Source-confirmed baseline defects, executable repaired-runner negatives,
   minimal fix, debug/release checks and intake; no baseline RED claim.
3. Independent exact-source review and separate properly governed native run.
4. Shared workflow/docs integration after explicit release; canonical PR gate,
   exact-head hosted checks/reviews, PR to dev and exact merged-tree readback.

The package can be preserved with an explicit NOT_QUALIFIED measurement state
if enforcement is unavailable; it cannot claim fresh qualification. A genuine
source/policy defect blocks the affected delivery assertion, not unrelated
programme work. No public binary/native bundle publication or licence
clearance claim. Rollback affects only this isolated package/docs/workflow;
there is no production state migration to undo.

## GSTACK REVIEW REPORT

Independent source review found the two concrete defects above. Independent
written-plan review found no blocking omission and required bounded queued
output plus deadline-covered EOF; both are incorporated above. Commander
review confirms one child owner, shared parser, existing dependencies and no
shared production change. Five named package paths are released to one writer.
Source writing can proceed while local builds/measurements wait for M4's
actual resource-slot release. Test, measurement and delivery gates remain open.

NO UNRESOLVED DECISIONS within this isolated package repair scope. This does
not approve the production crypto profile, recovery policy or measurement result.

Independent implementation review confirms guard, preserved descriptor flags,
bounded records/exact EOF and unchanged timing/crypto boundaries. It requested
a live WouldBlock-to-expiry regression in addition to already-expired checks;
that focused test remains required before the code gate passes. README and
all execution/intake/native measurement gates remain open at this checkpoint.

### Measured source checkpoint

Code source `82a9e21a2946b11d0a3db41201a90b1acfed9561`, tree
`ac68fac4096ee78ea39ba6674dba943262e0e356`, clean. Final independent source
review accepted the live owned-pipe timeout regression and found no concrete
unresolved defect within the repair scope. Debug13/13 and release13/13,
formatting and strict all-target Clippy passed; no ignored or filtered tests.
The live timeout test exercises held-open EOF and requires watchdog rejection
plus ECHILD after cleanup; no instrumented branch-coverage claim is made.

One fresh measured run exited0/SUCCESS, service runtime18.462s, invocation
`ed6f1a9bfc5a4342946c5b9d9e92a4cb`. M4 explicitly released and regained the
CPU slot; no local programme build or browser launch overlapped this measurement.
In-process checks and a separately captured active/running systemd unit agreed
on96MiB, one-CPU quota, zero swap, CPU0 affinity and core0. Active Result fields
were not treated as terminal proof; terminal output established success.

| Independently recomputed observation | Value |
| --- | --- |
| Samples, contiguous and unique | 8 warm-ups;128 warm;8 cold |
| Warm p50/p95/p99 | 123.811338/133.867671/150.894350ms |
| Cold maximum | 124.803407ms |
| Maximum warm/cold high-water RSS | 69861376bytes |
| Oracle high-water RSS | 69971968bytes |
| Whole test cgroup peak | 68235264bytes |
| Swap/OOM events | 0/0 |

No sample failed or was replaced. Host load changed2.31/4.89/4.93 to
2.97/4.91/4.94; a quota/coordination slot is not exclusive ownership of the host.
The existing Linuxbrew loader/libgcc remain linked; this is not hermetic
distribution evidence. Native build output points to this package's own
static sodium archive and bundled1.0.22 source. Bundled source archive hash
remains `b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab`.

Prelaunch tool output recorded executable SHA-256
`8944f5eb0482fc23988b6cf1ca6f1b52189436992a298ff9417373d1f465ea5e`;
postrun and independent hashes match. The measurement ledger itself records
the executable path, not an embedded executable digest; this distinction is
retained. Raw log SHA-256:
`dbe7c1802467a4c0cec2061e82d63292ba085dfd9e113e5e9d30626c7ae83d6d`.
Source/test hashes:
`7519524e6ef70a5d3e870a9249f55e0deabc9239c467fefc73b3f01917db19de` /
`cc4c4a4c0359aa8febb86526072007f63a401e97c11b729d804c2f0133d79477`.

Fresh67-archive verification passed against the actual unchanged lock. Cargo
deny0.20.2 passed licences/bans/sources with eight unmatched-policy warnings
and one duplicate-hashbrown warning; no policy was changed. Unsuppressed
cargo-audit0.22.2 passed against1239 advisories at database
`5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`. This is not native-notice/legal
clearance. Metadata retains the sys package's empty transitive default marker;
no fetch-latest/minimal/optimized/system-source feature is selected.

Evidence ledgers under the project logs directory:

- `2026-09-05T21-00-09-256Z-kdf-intake-222336-bcae576d.log`.
- `2026-09-05T21-01-57-519Z-kdf-build-226284-5bc19d70.log`.
- `2026-09-05T21-02-16-993Z-kdf-measurement-226852-0b407ff4.log`.
- `2026-09-05T21-05-14-375Z-kdf-tests-233102-f6aaa71c.log`.
- `2026-09-05T21-07-43-629Z-kdf-closure-265650-60c202dd.log`.

Initial new-runner compilation and formatting failures remain preserved; they
are not baseline RED evidence. Canonical delivery, shared workflow/discovery
integration, final hosted review/checks and merge remain open. Production C3,
recovery, other hardware and packaged Tauri authentication remain unapproved
or deferred as previously recorded. No migration/archive/shared ownership changes.

## Independent CI preparation release — 2026-09-05

The Commander is the sole writer of the qualification workflow in both PR130
and this worktree. Its reviewed source is frozen at PR130 head
`41a359e5e22ccaa0152d63a6f22b075f7307cf3d`. Preparing the dependent KDF
workflow now does not require waiting for its hosted hardware gates. Integrate
that exact source with a normal local merge after checking the merge tree;
preserve both worktrees and histories. Do not push KDF as a duplicate framing
delivery. Before its own PR, reconcile the actual merged PR130 tree and current
dev, then run final evidence against the resulting KDF delivery source.

After independent delta-plan review, release only the existing qualification
workflow, its AGENTS qualification paragraph and one CHANGELOG entry to the
Commander. No M4-owned production, canonical-plan or other shared path is
released. Preserve M4's later programme-wide tool prohibition when integrating
its source. No migration or archive allocation is made.

Add KDF to the existing explicit test/advisory matrices and both path filters.
Retain pinned actions/toolchain, read-only permissions, native-override
rejection, locked dependencies, isolated target, two build jobs and serial
tests. KDF has 13 unit tests and no doctest target; enforce one summary for
that binary, while signing and framing retain their exact two-summary counts.
Reuse the existing Bash guard with the minimum explicit zero-doctest handling.
No new runner framework, workflow, dependency or hosted measurement is needed.

Verify the exact authored step against all three real packages in debug and
release. Extend the existing synthetic count/exit sentinels for the binary's
one-summary case, including rejection of unexpected zero-test doctest output.
Keep synthetic guard evidence distinct from native tests. Run formatting,
strict Clippy, actionlint, local documentation links and canonical verification
after Metadata's actual browser/CPU release. Existing measured-code evidence
stays bound to 82a9e21a; do not repeat the measurement for documentation or CI
changes. Final source review, hosted checks and merged-tree evidence remain
delivery gates, not reasons to stop independent source preparation.

Independent delta-plan review is clear: the one-summary binary case must reject
extra summaries, existing matrix counts and isolation remain unchanged, and
normal local integration is preparation only. The Commander may now perform
the narrowly named integration and checks above.

## Final delivery test delta — 2026-09-05

Independent coverage review found no demonstrated runner defect but identified
missing direct write-boundary checks. Add one standard-library-only test for
successful output, a short fixed-slice write followed by WriteZero, and an
expired deadline that must leave its destination unchanged. No runner or
dependency change is needed. Update current workflow and discovery counts to
14; preserve every historical 13-test receipt and the measured 82a9e21a source.
Rerun debug/release, strict Clippy, formatting, authored workflow and its
synthetic guards, then canonical delivery checks and independent review.
This test-only addition does not require another native measurement. Retry
branches are not all fault-injected; do not claim instrumented coverage.
