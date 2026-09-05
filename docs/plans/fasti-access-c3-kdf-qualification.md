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
qualification. New focused tests must demonstrate both identified defects
before the fix, then pass after it; preserve the failing output.

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
2. Reproduced runner negatives, minimal fix, debug/release checks and intake.
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
