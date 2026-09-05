# E1 OIDC candidate qualification

Status: **10 library checks pass; E1-Q1 remains partial. No runtime adoption.**

This separate Cargo workspace tests `openidconnect =4.0.1` through its public
APIs. It starts no listener and creates no Fasti account, session or database.
The [written E1 gate](../../docs/plans/fasti-access-e1.md) controls integration.

## Run

From the repository root:

```sh
cargo fetch --locked --manifest-path qualification/access-e1/Cargo.toml
cargo test --offline --locked --manifest-path qualification/access-e1/Cargo.toml
cargo clippy --offline --locked --manifest-path qualification/access-e1/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path qualification/access-e1/Cargo.toml -- --check
```

The first command downloads only the locked qualification graph. The checks
are offline. On Linux with unprivileged network namespaces, also run:

```sh
unshare --user --map-root-user --net cargo test --offline --locked --manifest-path qualification/access-e1/Cargo.toml
```

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
- Final ordinary run: 10 passed, 0 failed, 0 ignored; exit 0.
- Network-namespace run: the same 10 passed, 0 failed, 0 ignored; exit 0.
- Strict Clippy passed with warnings denied. These are not performance results.
- Independent source/plan review corrected caller-state and provenance omissions.
  Harness review requested exact URL/client/redirect assertions, now included.
- Ponytail complexity review: no additional abstraction, HTTP client, executor
  dependency, session store or production manifest change is needed.

| Input | SHA-256 |
| --- | --- |
| `Cargo.toml` | `bc7a15fd6aff0fc9678bd3151304fc9d41529ee5c5597bad1276df220079fe95` |
| `Cargo.lock` | `695a432152097118ddd6d97678c07fde9fc472e111aa686e571eeb223e692ee3` |
| `qualification.rs` | `13aa85a36ce23c2e5d3b5a06848faf75a26095f1aa159e6458e28fa3d3bc1b8c` |
| `synthetic-test-key.pem` | `20a63565470ef8c42e48675edd8478c3d2c9c3e9cb727702a64adc48c98660f4` |
| `synthetic-rotation-key.pem` | `d2604c19f88ee96466dce9e6f702516d21a303d78a1f9c7920d69a9b72f00774` |

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

## Still required

Callback state/single-use/cancellation/restart tests through the real ceremony
owner; body/error/endpoint bounds and resource proof; unsupported algorithms
and further malformed-token cases; `azp` policy; assurance policy; token erasure;
userinfo subject binding; refresh/discard lifecycle; logout receivers and token
validation; named provider conformance; full dependency/licence/advisory review.
No test-only callback or store may stand in for these integration checks.

M4 retains shared production files, v17 and archive v7. Access v18 remains
conditional on its exact merged handoff. Root `AGENTS.md` and runtime contracts
are unchanged because no capability is activated; shared guidance must follow
the real integration change. Tauri authentication remains deferred.
