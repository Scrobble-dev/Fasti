# C2 domain/application foundation delivery gate

Status: `PREPARATION_AUTHORIZED_DELIVERY_GATES_PENDING`

Recorded: 2026-09-05. Owner: Commander, sole integration writer.

## Purpose and unchanged programme

Deliver the completed pure C2 domain/application foundation as a separate
reviewable PR into `dev`. This is a dependency step toward the complete
[authentication programme](trailbase-authentication-remediation.md), not a
replacement for C2 or a claim that all of C2.1 is complete. The full C2 gate
continues to require persistence, transactional authorization, capabilities,
contracts, API, SDK, host, Tabler A+C UI, and runtime evidence.

Gate 10, TrailBase selection, the approved trust profile, Secure cookies,
per-install administrator custody, and per-person accounts remain unchanged.
Packaged authentication remains the unclaimed `C1-TAURI-AUTH` follow-up.

## Exact inputs and ownership

- Base: merged M3 `dev` `df09101028a988a92f4546313c5eed6dd20d238a`,
  tree `5552947a30b82497c7fa279a6932fe7877ed612b`, schema v15/archive v5.
- Source: C2 `5eb3def007f712f62038f4f1bd8c64f76c47098e`,
  tree `2c15dd499281dc91fcbc3bb8325eb8878f1c04fe`.
- Use an isolated `codex/fasti-access-c2-foundation` branch. Preserve the
  original C2 branch and its later C3/D0 research. Do not land its whole HEAD.
- Metadata's 2026-09-05 read-only reconciliation permits preparation of this
  bounded source set. It is not readiness approval or shared-file release.
- M4 retains v16 and shared integration. No new migration is allocated here.
- The ID/secret prerequisites are patch-identical to M4's existing work.
  Preserve one effective change. A later M4 merge must retain its `scr_`
  reserved search ID and registry count 27, plus both `search` and
  `access_credentials` modules. Do not copy this branch over M4's files.

## Production diff allowlist

Exactly these 16 source/test/manifest paths may be imported from the pinned
source. Documentation may additionally record this boundary and its evidence.

```text
Cargo.lock
crates/fasti-application/Cargo.toml
crates/fasti-application/src/access_credentials.rs
crates/fasti-application/src/kernel.rs
crates/fasti-application/src/lib.rs
crates/fasti-application/tests/c2_access_consent_commands.rs
crates/fasti-application/tests/c2_access_inventory.rs
crates/fasti-application/tests/c2_access_issuance_results.rs
crates/fasti-application/tests/c2_access_registration.rs
crates/fasti-application/tests/c2_client_lifecycle_commands.rs
crates/fasti-application/tests/c2_personal_token_commands.rs
crates/fasti-domain/src/access_credentials.rs
crates/fasti-domain/src/ids.rs
crates/fasti-domain/src/lib.rs
crates/fasti-domain/tests/c2_access_consents.rs
crates/fasti-domain/tests/c2_personal_tokens.rs
```

These paths define bounded names, classifications, lifecycle transitions,
token policy, PAT secret custody, consent revision identity, browser-bound
requests, bounded inventory requests, and checked one-time issuance results.
They reuse existing IDs, session evidence, scopes, digest and secret owners.
The shared cleanup fix also affects existing `SecretMaterial` consumers;
typed-ID deserialization also affects existing ID consumers. Full regression
gates therefore remain required, not only the new C2 tests.

No schema, store, registry, generators, scopes, capabilities, ports, API, SDK,
host, Workbench, archive, or runtime activation change belongs in this PR.
There is no new callable PAT authentication or administration operation.
`AccessScopeSet` canonicalizes existing scopes; it does not grant delegability.
M4's reserved `metadata_search` must remain default-denied during integration.

## Ordered delivery tasks

1. Record this gate before creating the delivery tree. Import only the pinned
   allowlist and the C2 implementation plan. Correct stale current-state
   references to M3 without erasing historical review evidence.
2. Prove byte identity for imported source, unchanged prohibited surfaces,
   clean formatting, and dependency/lockfile intent. Commit coherent source
   and documentation slices without rewriting the original branch.
3. Run fresh all-feature domain/application tests and strict all-target
   Clippy. Historical 319-test evidence is context, not final-head proof.
4. Run unchanged canonical `PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr`
   and locked contract verification. Complete applicable `/review`, `/cso`,
   `/qa`, `/devex-review`, `/ship`, and Ponytail review. Independent reviewers
   audit coverage, plan completion, and the exact final diff. Visual design
   and Impeccable polish are not applicable to this headless-only diff;
   ordinary UI regression checks still run through the canonical gates.
5. Require exact-head CI, dependency/security/licence and applicable SBOM
   evidence. Do not add a fake passing C2 milestone or waive its remaining
   runtime, migration, transaction, recovery, or UI gates.
6. Confirm the final unique diff with metadata before merge. Merge only a
   green dependency-ready PR into `dev`, verify the merged tree, then send
   its exact commit/tree and the unchanged M4 ownership boundary.
7. Save context and continue C2 integration when M4 hands off. A foundation
   merge allocates no migration and releases no M4-owned shared surface.

## Rollback and proof limits

### Inherited licence gate correction (2026-09-05)

The exact-head canonical PR gate, RustSec audit and three-commit secret scan
passed. The additional all-target licence gate failed on the existing
`webpki-root-certs 1.0.9` CDLA-Permissive-2.0 declaration. The package and
checksum are unchanged from merged `dev`; no new dependency caused this gap.
Metadata confirmed that no writer owns `deny.toml` or `NOTICE` and released
only this bounded policy/documentation correction to the Commander.

Before delivery, retain the exact upstream agreement and checksum review in
`third_party/webpki-root-certs/`, reference it from `NOTICE`, and add a
crate-and-exact-version licence exception in `deny.toml`. Do not add this
licence to the general allowlist, ignore the scanner, change dependencies,
or touch integration/schema/transport files. Re-run the licence/source gate,
verify the retained text and package hashes, and independently review this
additional diff. The original 16 source/test/manifest blobs remain unchanged.

The all-target graph includes a wasm32-only dependency declared by the
platform verifier. It is absent from the checked Linux target graph. This is
not proof of WebAssembly support or a Linux packaging violation. Any future
artifact that shares this root data must include the retained agreement text;
an SBOM identifier alone does not satisfy that delivery condition. No native
package or packaged-Tauri support is activated by this correction.

This PR writes no new durable data and changes no archive version. Before
downstream consumers land, revert its coherent commits through a reviewed PR;
never reset shared history. After consumers land, coordinate the revert with
those consumers. Do not weaken existing typed-ID or secret-erasure guarantees
to keep a dependent branch compiling.

No fresh tests, PR, merge, production availability, performance improvement,
complete secret-erasure guarantee, or accessibility conformance is claimed
by this preparation record. Append exact evidence as the gates execute.

The developer-reference gate also found two inherited unresolved rustdoc links
in `crates/fasti-application/src/nuvio.rs`. Metadata released only those two
header links for crate qualification. This documentation-only correction adds
no import, runtime behavior or shared contract. Verify with
`RUSTDOCFLAGS="-D warnings" cargo doc -p fasti-domain -p fasti-application --all-features --no-deps --locked --offline`.

### Exact-head JavaScript advisory repair gate (2026-09-05)

PR #125 at `c1396f16` passed local canonical verification, but its remote
JavaScript audit failed on inherited `fast-uri 3.1.5`. The workspace manifest
and pnpm lockfile are unchanged from the merged base. This is an in-scope
security gate failure, not part of the packaged-Tauri deferral.

The official [v3.1.6 release](https://github.com/fastify/fast-uri/releases/tag/v3.1.6)
addresses GHSA-5jgf-p345-68v8, GHSA-fph4-wmhf-6fwf,
GHSA-f65p-4m7j-42xc and GHSA-jqff-g426-hqxp. Its source commit is
`6f970b2951fd896aa0f3a7ff28eeb6640c137d33`; the npm package retains BSD-3-Clause.
AJV supplies the sole installed version to the existing schema, OpenAPI,
AsyncAPI and SBOM tooling. No custom URL parser or suppression is warranted.

The bounded repair order is:

1. Metadata released its existing dependency repair commit
   `06c5b698c2011051b72781dc3ee5e2b75b39b1e0`, limited to `pnpm-lock.yaml`,
   `pnpm-workspace.yaml` and `tests/js/patched-dependencies.test.mjs`.
   Keep every other M4 surface read-only.
2. Reuse that exact patch, rather than the initially proposed `3.1.6` update.
   It selects `fast-uri 3.1.7` within AJV's existing range and narrowly
   overrides affected `qs` versions to `6.16.0`. The newer
   [URI release](https://github.com/fastify/fast-uri/releases/tag/v3.1.7)
   also fixes GHSA-qw65-cvwx-89v3 and GHSA-58mr-gqgx-xq4g; the
   [qs advisory](https://github.com/ljharb/qs/security/advisories/GHSA-x5fp-wj9c-mxmx)
   requires `6.16.0` for bounded comma arrays. Both retain BSD-3-Clause.
   Preserve every unrelated resolution, build policy, patch and audit exception.
3. Verify package integrity against the registry, frozen installation,
   `pnpm audit`, contract validation and the canonical PR gate. Independently
   review the dependency diff and its callers. Keep the old failed receipt.
4. Add a signed forward commit and refresh the PR's exact-head evidence.
   Require fresh remote gates before merge; no force push or CI weakening.

This adds no direct dependency, framework, endpoint, schema or migration.
Do not restore the vulnerable version as a convenience rollback; use a
reviewed non-vulnerable resolution if the patch exposes a compatibility issue.
