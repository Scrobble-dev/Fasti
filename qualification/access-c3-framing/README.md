# C3 framing qualification

This isolated package preserves `c3-frame-probe-2`. It is not a production
encryption adapter, approved crypto profile, joint backup format or restore
activation gate. No Fasti production package depends on it.

## Run from the repository root

Prerequisites: Rust 1.97.1 with rustfmt and Clippy, a C compiler, make and the
locked Cargo archives. The native binding builds its bundled source. Run on a
prepared Linux host, separately from resource measurements:

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
)
```

Each full debug/release run must report **20 passed unit tests and two passed
compile-fail doctests**, with zero failed, ignored or filtered tests. Do not
pass a test-name filter. Expected compiler errors in the two doctests prove
that the opaque key cannot be formatted with Debug or cloned.

The subshell clears native source/library overrides without changing the
interactive shell. CI rejects even empty-but-set overrides without printing
their values. Retain fresh debug/release build output: it must identify this
package's static `out/installed/lib` and `out/source/libsodium-stable` paths.
Version and ABI assertions alone cannot distinguish another installation.
Do not substitute a system library, replacement source or binary fallback.
Offline Cargo resolution does not prove that build scripts are network-isolated.

The package has its own workspace and lock. Its test-only `fasti-store` path
resolves inside this checkout; the tests use the existing public archive writer
and validator. No external fixture checkout, account, service, keyring or data
root is needed. Output stays in this package's ignored `target/` directory.
Missing cached archives fail offline commands; fetch only the locked graph.
Keep failures and investigate their cause instead of removing assertions.

The [dedicated qualification workflow](../../.github/workflows/access-c3-signing-qualification.yml)
runs the signing and framing packages independently on matching PRs into `dev`
and pushes on `dev`. A topic-branch push alone does not trigger it.
CI checks both test-result summaries against each package's exact unit and
doctest counts; removed, ignored, filtered or failed tests cannot pass that gate.
Raw output remains visible in the job log. Root
workspace tests exclude both packages; their focused checks supplement, not
replace, the required `cargo xtask test pr` gate.

Inspect this separate lock and dev dependencies with the repository policy:

```sh
cargo deny --manifest-path qualification/access-c3-framing/Cargo.toml check licenses bans sources
cargo audit --file qualification/access-c3-framing/Cargo.lock
```

These commands require the named Cargo tools; the advisory refresh requires
network access. Record tool/database identities and raw results. Do not add
policy allowances or advisory suppression to obtain a pass.

## What the checks establish

- Backup alignment at 0, 1, 65535, 65536, 65537 and 131072 plaintext bytes;
  single-Final record bounds and the narrower 4096-byte provider limit.
- Exact frame/plaintext/ciphertext admission, overflow rejection, partial and
  repeated flush handling, with capacity reserved for Final.
- The real `ArchiveWriter::finish()` flushes through framing; the real archive
  validator and separate framing completion require valid archive data,
  authenticated Final and physical EOF. Neither check replaces the other.
- Wrong key/AAD, modification, missing/truncated Final, trailing bytes,
  reordering, duplication, cross-stream frames and unsupported tags reject.
- Short/interrupted I/O preserves identity; partial output and source errors
  poison the owner; unfinished Drop never finalizes; terminal state cannot
  advance; rejected readers release state/plaintext before caller Drop.
- Runtime native identity is libsodium 1.0.22, ABI 26/4, nonminimal.

The test-only native fixture produces valid authenticated unsupported tags.
The unknown-tag test uses `catch_unwind` only to assert that rejection does
not panic. Neither FFI substitution nor panic recovery is an adapter strategy.
The original Alkali run's 15 passes and one panic failure remain preserved;
this package does not rehabilitate that candidate.

## Provenance and limits

The [frozen delivery gate](../../docs/plans/fasti-access-c3-framing-qualification.md)
records original source, lock, failed-case hashes and the complete test matrix.
The adapter, tests and lock were initially copied byte-for-byte. The tests
later gained explicit native-FFI safety comments; assertions remain unchanged.
The native prefix fixture remains independent of the writer's encoding so a
shared helper cannot hide the same encoding defect in both paths. Strict Clippy
on Rust 1.97.1 then required one adapter spelling change:
`pmax % CHUNK as u64 != 0` became `!pmax.is_multiple_of(CHUNK as u64)`.
The fixed divisor remains 65536; checked ceiling arithmetic and limits are
unchanged. No lint allowance, test or lock change was made. The original adapter
SHA-256 is `e60f1d823c292a5d1b811d6eb5c9d6884b9d101c2f11e0a290163e3a222d87fb`;
the delivered adapter is `16794e3fb1b5cf4288e2b0d0e5a30207fd458a75f18accc02728425d6e6491eb`.
The manifest adds workspace isolation, the repository's first-party licence
and a checkout-relative, exact-version test dependency. Old probe results are
not verification of a later checkout or toolchain.

| Candidate | Exact version | Registry archive SHA-256 |
| --- | --- | --- |
| libsodium-rs | 0.2.4, defaults disabled | `4b8cd48c80d6c6fa5a4612d242941067219555baea82b0b49c92ea9d8156b59c` |
| libsodium-sys-stable | 1.24.0, defaults disabled | `72b04bf6da2c98b727af37ab62cb505f4d751b975b034a9b9ad491d333b0564e` |
| zeroize | 1.9.0, `alloc` | `e13c156562582aa81c60cb29407084cdb54c4164760106ab78e6c5b0858cf64e` |

The sys package's bundled 1.0.22 snapshot has SHA-256
`b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab`.
Use the [pinned wrapper source](https://github.com/jedisct1/libsodium-rs/blob/b3ad9336c0aa6f31eb41fc25431fafdc8e1a7632/src/crypto_secretstream/xchacha20poly1305/mod.rs)
for API identity. The provider's raw key has Debug/Clone; this probe keeps it
private. Its allocations are not hardened or memory-locked.

The envelope is opaque fixture input and the `{}` manifest is a low-level
archive fixture, not a complete joint Access manifest. Publication is an
in-memory sink. The source-error fixture interrupts ciphertext-body reads;
an I/O failure at the final physical-EOF check is not injected. Passing tests
do not establish durable publication, production
key custody, compiler-temporary erasure, startup/native-failure handling,
abort cleanup, resource ceilings, throughput, other platforms, native-notice
completeness, licence clearance, disposition freshness, fencing or authorized
restore activation. Qualification creates no production data to roll back;
retained probes and failure evidence must not be deleted.
