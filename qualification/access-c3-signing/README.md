# C3 signing qualification

This isolated test package preserves `c3-sign-probe-1`. It is not a production
signing service, approved crypto profile, authenticated backup, or restore gate.
No Fasti production package depends on it.

## Run from the repository root

Prerequisites: Rust 1.97.1, a C compiler, make, and the locked Cargo archives.
The native binding builds its bundled source. On a prepared Linux machine:

```sh
(
set -e
unset SODIUM_LIB_DIR SODIUM_USE_PKG_CONFIG SODIUM_SHARED SODIUM_DIST_DIR
cargo +1.97.1 fetch --locked --manifest-path qualification/access-c3-signing/Cargo.toml
CC=/usr/bin/cc cargo +1.97.1 test --offline --locked --manifest-path qualification/access-c3-signing/Cargo.toml
CC=/usr/bin/cc cargo +1.97.1 test --release --offline --locked --manifest-path qualification/access-c3-signing/Cargo.toml
cargo +1.97.1 fmt --manifest-path qualification/access-c3-signing/Cargo.toml -- --check
CC=/usr/bin/cc cargo +1.97.1 clippy --offline --locked --manifest-path qualification/access-c3-signing/Cargo.toml --all-targets -- -D warnings
)
```

The subshell prevents inherited native-library and source-archive overrides
without changing your interactive shell. Do not set these variables for this
qualification. Retain the binding's Cargo build output: it must name the local
`out/installed/lib` static library and `out/source/libsodium-stable` include
directory. Version/ABI assertions alone cannot distinguish another installation.

Each full test run must report **9 passed** unit tests and **3 passed**
compile-fail doctests, with zero failed, ignored or filtered tests. Do not pass
a test-name filter: Cargo can succeed when it runs zero tests. Compiler errors
inside the three compile-fail doctests are expected assertions, not failures.
Missing archives fail the offline command; fetch the locked graph, then retry.
Keep a failed assertion visible and fix its cause rather than removing the case.

The package owns its workspace and lock. Fasti path dependencies and the v1
manifest fixture resolve inside this checkout, not an older worktree or `/tmp`.
Build outputs stay in the package's ignored `target/`; no accounts, services,
keyring entries or data roots are created. Cleanup needs no product reset.
Offline Cargo resolution alone does not prove network-isolated build scripts.

The [dedicated workflow](../../.github/workflows/access-c3-signing-qualification.yml)
runs these checks and an unsuppressed dependency audit for matching PRs into
`dev` and pushes on `dev`. A topic-branch push alone does not trigger it.
The root workspace tests do not include this isolated package; run its commands
above as well as the required `cargo xtask test pr` gate. To refresh and inspect
advisories for its separate lock from the repository root, run:

```sh
cargo audit --file qualification/access-c3-signing/Cargo.lock
```

This last command requires `cargo-audit` and network access for the refresh.

## What the checks establish

- RFC 8032 section 7.1 vectors 1–3 match exact public keys and signatures.
- Generated keys and 32-byte seed import work; the wrapper does not accept
  arbitrary 64-byte secret material or expose raw key construction, Debug or Clone.
- Wrong key/message, every signature-byte mutation, malformed lengths/values,
  and a 16-KiB probe input ceiling reject as specified.
- The real Fasti canonical projection supplies signed bytes. A valid signature
  does not make whitespace, duplicate fields or noncanonical JSON acceptable.
- The linked native library identifies itself as libsodium 1.0.22, ABI 26.4.

The v1 fixture is not a complete joint Access manifest. These checks do not prove
production key custody, temporary erasure, memory locking, startup/native-failure
cleanup, resource ceilings, other platforms, native-notice completeness, trusted
provenance, current erasure history, source fencing or authorized activation.

## Exact source intake

| Candidate | Version | Registry archive SHA-256 |
| --- | --- | --- |
| libsodium-rs | 0.2.4 | `4b8cd48c80d6c6fa5a4612d242941067219555baea82b0b49c92ea9d8156b59c` |
| libsodium-sys-stable | 1.24.0 | `72b04bf6da2c98b727af37ab62cb505f4d751b975b034a9b9ad491d333b0564e` |
| bundled native archive | 1.0.22 snapshot | `b20a92e7ec25b285eafa349d721a5bb27e3a8ba94c0816630a127883f1d1b3ab` |

The wrapper's [pinned signing source](https://github.com/jedisct1/libsodium-rs/blob/b3ad9336c0aa6f31eb41fc25431fafdc8e1a7632/src/crypto_sign.rs)
is the API authority. Context7's current examples corroborate detached signing
but do not establish equivalence to 0.2.4. Test vectors come from the
[official RFC](https://www.rfc-editor.org/rfc/rfc8032.txt).

The [written slice gate](../../docs/plans/fasti-access-c3-qualification.md)
records source provenance, preservation, delivery and remaining work. The prior
probe is retained unchanged; its old passing results are not results for this
checkout. Dependency review must include this separate lock and dev dependencies.
No advisory exception or production adoption is authorized here.
