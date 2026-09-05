# C3 KDF qualification

This Linux-only, isolated package preserves the RustCrypto/native experiment
`c3-kdf-probe-1` and repairs its process supervision. It is not imported by
Fasti. It does not approve production cryptography, passphrase policy, recovery
or packaged authentication.

One isolated measurement passed at clean source
`82a9e21a2946b11d0a3db41201a90b1acfed9561` on 2026-09-05 under the required
kernel controls. It is not a production or whole-application qualification.
Unit tests do not derive keys or execute that measurement. Run any later
measurement only after a coordinated uncontended resource slot is released.

## Source and dependency boundary

The original source and every historical run remain untouched at
`/tmp/fasti-c3-rustcrypto-kdf-bQV2oH`. Original source SHA-256:
`e7519bd9c331983e5c6011dcff5d430dea5aa89e6f276d9fa3eb729f29a7eb0c`.
The unchanged lock SHA-256 is
`21d0999c317d962d7cf7891f6a0e6edbabdf7b7d973bcd1af2ef9b0318a086f6`.
The original compiler was Rust 1.96.0; fresh checks use Rust 1.97.1.
Never relabel historical timings as results from this source or compiler.

All five direct dependency pins remain unchanged: Argon2 0.5.3 with zeroize,
zeroize 1.9.0 with alloc, Alkali 0.3.0 with std and no defaults,
libsodium-sys-stable 1.24.0 with no defaults, and libc 0.2.189.
Those are direct manifest declarations. The resolved sys graph also includes
its transitive `default` marker, which is empty in 1.24.0; it enables no
fetch-latest, optimized, minimal or system-library selection feature.
The manifest adds only separate-workspace and first-party licence metadata.
Alkali remains the native oracle and hardened-buffer dependency here. That does
not rehabilitate its rejected, separate framing behavior.

## Runner repair

One owner kills and reaps each process it spawns on every return path.
Warm and cold records share one strict parser: exactly
`SAMPLE <u128 nanoseconds> <u64 RSS bytes>\n`. Numeric overflow, extra fields,
incomplete records and RSS above 100663296 bytes fail. Cold success also
requires exactly one record and physical EOF before its original deadline.

Owned Linux pipes use nonblocking I/O through the existing libc dependency.
There are no reader threads or application output queues. Each record is
bounded to 128 bytes; the kernel pipe provides finite backpressure. Oracle
output must be exactly 32 bytes plus EOF within the five-second deadline.
Pipe errors, crashes, excess output and expired deadlines fail without a
replacement sample. The owner controls only its direct child, not arbitrary
descendants. This is not a hostile-process sandbox or a leak-freedom proof.

## Focused checks

Run from the repository root after obtaining the local build resource slot:

```bash
(
  set -e
  unset SODIUM_LIB_DIR SODIUM_USE_PKG_CONFIG SODIUM_SHARED SODIUM_DIST_DIR
  export CARGO_TARGET_DIR="$PWD/qualification/access-c3-kdf/target"
  export CC=/usr/bin/cc
  cargo +1.97.1 test --offline --locked --manifest-path qualification/access-c3-kdf/Cargo.toml -j 2 -- --test-threads=1
  cargo +1.97.1 test --release --offline --locked --manifest-path qualification/access-c3-kdf/Cargo.toml -j 2 -- --test-threads=1
  cargo +1.97.1 fmt --manifest-path qualification/access-c3-kdf/Cargo.toml -- --check
  cargo +1.97.1 clippy --offline --locked --manifest-path qualification/access-c3-kdf/Cargo.toml --all-targets -j 2 -- -D warnings
)
```

An offline cache miss means dependencies must be prepared separately with the
same locked manifest; it does not authorize removing `--locked` or selecting
another native source. Hosted checks must reject inherited native overrides,
including empty-but-set values. Before delivery, independently check the actual
67 third-party archive hashes, feature graph, native source/build identity,
licence/bans/source policy and refreshed unsuppressed advisories.

Tests use disposable owned shell children to check parser, pipe and process
lifetime failures. Their synthetic byte strings are protocol fixtures, not
cryptographic agreement evidence. The original defects were identified in
source inside `supervise()`, after enforcement and the native oracle.
New passing helper regressions alone do not prove a before-fix failing run.

The suite contains 13 focused tests: strict sample fields and RSS, complete
cold output, record and UTF-8 bounds, exact oracle framing, parse/pipe failure
cleanup, successful/nonzero child exits, missing cold records, immediate
deadline rejection, held-open EOF timeout, and preserved nonblocking flags.
The EOF timeout uses a real owned child that keeps stdout open; it does not
infer timeout behavior from an already-expired timestamp alone.

The existing Access C3 qualification workflow runs these 13 tests in debug
and release, checks formatting and strict Clippy, and audits the exact lock
without suppression. This binary has no doctest target. The summary guard
requires exactly one successful 13-test summary per profile and rejects failed,
ignored, filtered, missing or extra summaries. It does not run a measurement.

## Separate governed measurement

Build the release binary outside the measurement cgroup. Run it with **no
arguments** only inside a dedicated, verified Linux user cgroup named with
`fasti-c3-kdf-qualify`. The runner checks effective `cpu.max=100000 100000`,
`memory.max=100663296`, `memory.swap.max=0`, CPU 0 affinity and core limit 0.
Do not stop another task, change global controls, or substitute an unconstrained
run when enforcement is unavailable. Child modes are internal protocol
operations, not independent qualification commands.

Preserved contract:

- Argon2id version 0x13, 65536 KiB, three passes, one lane, 32-byte output.
- Disposable password: 64 bytes of 0x41; salt: 16 bytes of 0x42.
- One untimed native/RustCrypto agreement check. Derived bytes use the private
  child pipe and hardened owners; they are never logged.
- Eight excluded warm-ups, then all 128 warm samples. Timing includes scratch
  allocation, derivation and explicit zeroizing Drop, not output comparison.
- Eight cold processes, timed from launch through successful exit.
- Five-second per-derivation watchdog. Nearest-rank sorted positions 64, 122, 127
  for warm p50/p95/p99; inclusive ceilings 500/1000/2000 ms. Each cold sample
  must remain at most 2500 ms. No failed sample is replaced.
- Record child high-water RSS and whole-cgroup peak/events separately.
  Swap, OOM, excessive memory or missing controls fail qualification.

Retain source, lock, features, native archives/build outputs, compiler,
executable hashes, raw logs, failures and actual kernel controls for each run.
The [package plan](../../docs/plans/fasti-access-c3-kdf-qualification.md) owns
delivery gates. No isolated result proves the joint Fasti/TrailBase 192 MiB
envelope, other architectures, complete temporary erasure, memory locking,
production recovery or distribution readiness.

## Recorded isolated result

The [package plan](../../docs/plans/fasti-access-c3-kdf-qualification.md#measured-source-checkpoint)
records the exact source/tree, executable and raw-log hashes, active-unit
controls and independent recount. One run retained eight warm-ups, 128 warm
samples and eight cold samples. Warm p50/p95/p99 were
123.811338/133.867671/150.894350ms; cold maximum was124.803407ms.
Whole-cgroup peak was68235264bytes; swap and OOM events were zero.
These are this host's isolated fixture observations, not production claims.
CI integration is implemented locally. Canonical delivery, hosted checks and
merge remain open; a source change is not a claim those gates passed.
