# E1 OIDC candidate qualification

Status: **20 library checks pass; E1-Q1/Q2 remain partial. No runtime adoption.**

This separate Cargo workspace tests `openidconnect =4.0.1` through its public
APIs. It starts no listener and creates no Fasti account, session or database.
The [written E1 gate](../../docs/plans/fasti-access-e1.md) controls integration.

## Run

Use the repository's contributor toolchain, Rust `1.97.1`, with Cargo,
Clippy and rustfmt available. See the [repository setup](../../README.md#quick-start).
The explicit toolchain below does not change the machine's default. No TrailBase
service, account, browser, database or provider credential is needed.

From the repository root:

```sh
cargo +1.97.1 fetch --locked --manifest-path qualification/access-e1/Cargo.toml
cargo +1.97.1 test --offline --locked --manifest-path qualification/access-e1/Cargo.toml
cargo +1.97.1 clippy --offline --locked --manifest-path qualification/access-e1/Cargo.toml --all-targets -- -D warnings
cargo +1.97.1 fmt --manifest-path qualification/access-e1/Cargo.toml -- --check
```

The first command downloads only the locked qualification graph. The checks
are offline. On Linux with unprivileged network namespaces, also run:

```sh
unshare --user --map-root-user --net cargo +1.97.1 test --offline --locked --manifest-path qualification/access-e1/Cargo.toml
```

Expected full-suite result: **20 passed, 0 failed, 0 ignored, 0 filtered out**.
If an offline check reports a missing package, run the locked fetch with network
access, then retry the offline check. Do not remove `--locked` or regenerate the
lock to bypass that error. If this host forbids network namespaces, the ordinary
offline command remains useful, but does not prove network isolation.

Both PEM files are synthetic test-only RSA keys generated for this harness.
They are intentionally public fixtures, not installation or person credentials.
Never use them in a real service. Dates, identity URLs, tokens, nonce and state
are synthetic; no credential is printed. No production erasure claim follows
from dropping the candidate's test objects.

## Evidence recorded 2026-09-05

Base commit: `62e10d2e9bd738ed5da425c008eb839f89cdbea5`.
These are file-bound working-tree results, not clean-head PR or delivery receipts.
Rust `1.96.0 (ac68faa20 2026-05-25)`; Cargo `1.96.0 (30a34c682 2026-05-25)`.

- Initial offline lock resolution succeeded; execution stopped because three
  transitive archives were absent. A locked fetch populated them.
- Initial compile found the response-builder alias error; the harness now uses
  the upstream `http::Response::builder` constructor.
- First committed segment `de691062` passed 10 tests on clean source.
- Extended ordinary run: 15 passed, 0 failed, 0 ignored; exit 0.
- Latest network-namespace run, including four token-rejection checks:
  19 passed, 0 failed, 0 ignored; exit 0.
- Strict Clippy passed with warnings denied. These are not performance results.
- Independent source/plan review corrected caller-state and provenance omissions.
  Harness review requested exact URL/client/redirect assertions, now included.
  Separate review of the five token/userinfo checks found no actionable issue;
  public-client and JSON-only coverage limits remain explicit.
- A separate writer added the four token-rejection checks against unchanged
  helpers. Commander reviewed the leaf and ran the full combined suite.
- Ponytail complexity review: no additional abstraction, HTTP client, executor
  dependency, session store or production manifest change is needed.

| Input | SHA-256 |
| --- | --- |
| `Cargo.toml` | `b7b1c9d658adf8897dab86ee3b043e1d17ad2d8e1d01beae2d6ec9ef13d68016` |
| `Cargo.lock` | `695a432152097118ddd6d97678c07fde9fc472e111aa686e571eeb223e692ee3` |
| `qualification.rs` | `48ebfb322e80d9843729a5c339d991b3a16a1d867f3a37b12b73425efe1cb81a` |
| `token_rejection.rs` | `45bb6bde0d75ba2215d7b55748a326ebe12b2dd38e5bb7d983f07fb603dbb48c` |
| `synthetic-test-key.pem` | `20a63565470ef8c42e48675edd8478c3d2c9c3e9cb727702a64adc48c98660f4` |
| `synthetic-rotation-key.pem` | `d2604c19f88ee96466dce9e6f702516d21a303d78a1f9c7920d69a9b72f00774` |

## Confidential-client evidence recorded 2026-09-06

One parameterized check now covers Basic and request-body client authentication,
each with a successful JSON token response, a JSON provider error and a lost
transport response. It uses only the existing in-memory seam and public
synthetic client ID/secret values containing reserved and non-ASCII characters.
No dependency, lockfile, key fixture or production code changed.

The exact locked `oauth2` 5.0.0 `src/endpoint.rs:114–143` separately form-encodes
the client ID and secret before Basic encoding, or puts them in the form body.
`openidconnect` 4.0.1 exposes this choice through `set_auth_type` and the existing
client-secret constructor. Literal expected wire bytes check these behaviors,
the POST endpoint, content type, redirect, code and RFC 7636 verifier. Each
credential appears exactly once in its selected location, with no duplicate
Authorization header or credential form fields. All six cases dispatch once.

Observed boundaries remain explicit: the Basic header is not marked sensitive;
the parsed provider error retains the synthetic encoded-secret echo, including
in its `Display` output. The test inspects that public fixture text in memory,
not a real secret. Request-body credentials remain in the outbound byte buffer.
No safe logging or zeroization claim follows. A successful OAuth response still
need not contain an ID token; this is not a Fasti sign-in result.

The combined 20 tests passed both offline and in a fresh Linux network namespace
on Rust 1.97.1. Strict Clippy and formatting passed. Qualification source SHA-256:
`fd5f692465d01c11d97c2a06dc016a871797d1ba32f8e5c90c19c402feae854b`.
These are file-bound dirty-worktree results, not clean-head delivery receipts.
The earlier 19-check records above remain historical evidence.

The advisory audit was repeated with `--no-fetch` against the existing database
commit `5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`. It again exited 1 for
`rsa 0.9.10` / `RUSTSEC-2023-0071`, with no ignored advisory. This is a cached
database result, not a claim that current advisory data was fetched. The
workflow's unsuppressed advisory job is unchanged and remains a delivery gate.

## Dependency and delivery checks

The dedicated [CI workflow](../../.github/workflows/access-e1-qualification.yml)
runs this isolated suite, formatting and strict Clippy on relevant `dev` pull
requests and pushes. A separate job audits this lockfile without suppression.
Workflow syntax has been checked locally; live CI execution is not yet proven.
The known RSA advisory therefore still prevents a green delivery claim.

The 2026-09-05 independent full-diff review at `c09e2c5c` found no
actionable test or evidence-consistency issue. Subsequent dependency checks
found the harness's missing licence declaration. It now declares the same
`AGPL-3.0-or-later` licence as Fasti; no dependency version or lock changed.
The combined 19 tests, strict Clippy and formatting passed after this correction.

Run the existing dependency policy with test-dependency licence coverage enabled:

```zsh
cargo deny --locked --offline --manifest-path qualification/access-e1/Cargo.toml \
  --config <(sed '/^\[licenses\]$/a include-dev = true' deny.toml) \
  check licenses bans sources --show-stats
cargo audit --file qualification/access-e1/Cargo.lock --json
```

The first command derives a stricter policy in memory; it does not edit the root
policy or add exceptions. With cargo-deny `0.20.2`, licence checks exclude
dev-dependencies by default, even when the graph includes them. The explicit
check passed: licences zero errors, seven unused-policy warnings and 147 notes;
bans and sources zero errors or warnings. This covers its 147-crate resolved
graph, not every optional package in the 180-package lock inventory.

**The advisory audit is not green.** It exited 1 for `rsa 0.9.10`,
[RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html).
The fetched advisory database was commit
`5a0ebedfe8bdd2e295b171f4162f8c977bcad9a5`; no patched version was listed.
The harness signs only with the two public synthetic fixtures and starts no
listener. There is no private installation/person key to recover here.
That limits this test package's exposure; it does not fix the dependency,
approve production use, or justify suppressing the advisory. No ignore was added.
Independent CSO diff review traced reachable RSA private-key operations during
test signing; it did not find a concrete exploit against this public-fixture,
listener-free package. Reassess key custody and exposure before runtime adoption.
This scoped AI-assisted review is not a professional security audit.

The first canonical `cargo xtask test pr` run stopped in four existing snapshot
tests: SQLite refused the symlinked `/home/ryan/.cache/tmp` path with `CannotOpen`
(extended code 1550). The unchanged store suite passed with the physical path:
`TMPDIR=/mnt/secondary-ssd/cache/home/tmp cargo test --locked --offline -p fasti-store --lib --quiet`
(291 passed, zero failed, three ignored). No SQLite flag or source changed.
On the next run, the contract subprocesses passed, but the receipt writer correctly
refused the uncommitted licence/documentation changes. After committing them,
the canonical gate passed on clean `b94776219676e246d20557c4a67a478dff841acb`,
tree `04a2bcb552b1962070d237cd7455964c01ac2b5a`: 27 contract gates and 11 portable
gates, all with exit 0. This is historical exact-source evidence, not proof for a
later edit. Rerun on the final clean delivery head:

```sh
# On this prepared host, TMPDIR is the existing physical temporary directory.
TMPDIR=/mnt/secondary-ssd/cache/home/tmp PKG_CONFIG=/usr/bin/pkg-config cargo +1.97.1 xtask test pr
```

On another host, use its prepared physical temporary directory and repository
prerequisites; do not create this machine-specific path. Inspect
`target/fasti-receipts/b1-contract-verification.json` and `b1-portable.json` for
the exact commit, tree, clean-source status and individual results. These
canonical receipts do not run the separate E1 workspace; the explicit 20-check
command above is also required. No PR, merge or production support follows from
this record. Packaged Tauri authentication is not covered by these checks.

## What these checks establish

- Async discovery checks exact issuer before requesting JWKS; malformed
  discovery stops before JWKS. Neither proves bounded production transport.
- Authorization URL has the expected endpoint, client, redirect, code flow,
  RFC 7636 S256 vector, state and nonce; the verifier is not placed in the URL.
- Signed identity/nonce/audience/expiry mutations and damaged signatures fail.
- Explicit old-only, overlap and new-only key sets have the expected acceptance
  matrix. Network key refresh, cache admission and retirement scheduling remain open.
- Default issued-at validation accepts out-of-window values; an explicit test
  hook rejects them. The numeric test window is not approved production policy.
- Wrong `azp` is exposed but accepted by the library. A reviewed caller check
  remains mandatory; this passing negative control is not a sign-in approval.
- Access-token hash computation distinguishes substitution when the caller
  compares it. The library does not perform that comparison for the caller.
- A logout URL can omit ID-token hints. Actual provider acceptance is unproven.
- Public-client token exchange sends the expected form POST without a client
  secret. Parsing succeeds even without an ID token; Fasti must reject that case.
- An injected transport failure results in one request, not automatic retry.
  This does not prove server-side code consumption or durable recovery.
- Token parse errors retain the response bytes. Fasti must bound and redact the
  error boundary; logging the vendor error is not a safe conversion.
- JSON userinfo rejects a changed subject when bound to verified ID-token claims.
  Omitting that binding permits the changed subject. Signed userinfo is untested.
- The library's bearer header lacks the sensitive flag. A newline-bearing token
  panics before dispatch. These negative controls expose ingress/normalization
  obligations; test `catch_unwind` is not an approved production recovery path.
- Malformed compact segments fail parsing. Unsigned `alg:none` reaches validation
  and fails with `NoSignature`. Missing required identity/time claims fail the
  claims parser; absent audience defaults empty and fails verification.
- A correctly signed RS384 token succeeds only when explicitly permitted and
  fails the default RS256 policy. This is not approval to widen Fasti's profile.

## Still required

Callback state/single-use/cancellation/restart tests through the real ceremony
owner; body/error/endpoint bounds and resource proof; remaining JWE/profile
qualification; `azp` policy; assurance policy; scoped custody and disposal;
production userinfo binding and signed-userinfo profiles; refresh/discard lifecycle; logout receivers and token
validation; named provider conformance; full dependency/licence/advisory review.
No test-only callback or store may stand in for these integration checks.
The plan's Q2 source map also records dynamic endpoint ownership, missing governed
POST support, end-to-end deadlines and unproven erasure of transient vendor copies.
That last limit is not a new universal-erasure prerequisite. The approved gate
requires bounded zeroizing Fasti-owned custody, no persistence and narrow secret
lifetimes; ordinary transient buffer disposal must not be called zeroization.

M4 retains shared production files, v17 and archive v7. Access v18 remains
conditional on its exact merged handoff. Root `AGENTS.md` and runtime contracts
are unchanged because no capability is activated; shared guidance must follow
the real integration change. Tauri authentication remains deferred.
