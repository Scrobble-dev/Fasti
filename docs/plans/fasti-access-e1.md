# Fasti Access E1 — generic OIDC implementation gate

Status: `QUALIFICATION_ACTIVE`; no production OIDC implementation or support claim.

Started 2026-09-05 from merged `dev`
`62e10d2e9bd738ed5da425c008eb839f89cdbea5`, tree
`d6fcea1563b673f83cb4cabe1ef50d1c6dc5c087`.

The [canonical programme](trailbase-authentication-remediation.md) remains
controlling, especially sections 6, 12, 15 and 16. Gate 10 A+C is final.
This is a work-package implementation gate, not a new premise or framework review.

## Outcome and ownership

A person can use an explicitly configured OIDC identity to access their linked
Fasti account. Fasti preserves account-link integrity, checks current application
authority and creates only its existing opaque browser session. TrailBase remains
the human account foundation; an external identity never creates a workspace
role, selects a profile or grants a client scope by itself.

E1 depends on merged C1, not C2 runtime. Full activation still needs shared
persistence, contracts and UI integration. M4 retains all schema/store,
registry/generators, API/SDK, host/Workbench and portability files, including
v17/archive v7. Access v18 is conditional on its exact merged handoff; this
document reserves no migration and grants no shared-file release.

The Commander owns this file and the subsequent explicitly scoped E1
qualification harness in `codex/fasti-access-e1`. E0 owns separate OAuth-server
qualification in another worktree. Reviewers are read-only. Do not edit root
manifests, lockfiles, capability registries or production files during this gate.
No test harness may masquerade as a Fasti sign-in service.

## Existing owners to reuse

- `fasti-domain/src/access.rs`: `AuthSubject`, authentication/authorization epochs,
  browser sessions and `AuthCeremony`. Its current protocol enum is TrailBase-only.
- `fasti-application/src/outbound_access.rs`: outbound policy and address-class
  authorization. Provider metadata does not authorize a discovered endpoint.
- `fasti-provider-runtime/src/transport.rs`: bounded resolution, pinned addresses,
  proxy/redirect denial and bounded response reads. Its authorized-client surface
  currently exposes GET; do not presume it already supports credentialed OIDC POST.
- Existing C1 ceremony, process-memory PKCE and session transaction owners must be
  extended at integration, not copied into an independent OIDC session system.

No provider SDK types enter the domain. Do not add a generic identity framework,
alternate vault, session store, scheduler or hand-written JWT/OAuth implementation.

## Pinned dependency intake

Candidate: `openidconnect =4.0.1`, MIT, declared Rust minimum 1.65.
Its crate SHA-256 is
`0d8c6709ba2ea764bbed26bce1adf3c10517113ddea6f2d4196e4851757ef2b2`;
embedded source commit is `b639b5d39eac6903238867aeb2b29326502e6b26`.
The live [official sparse index](https://index.crates.io/op/en/openidconnect)
matched the cached archive hash and reported this version not yanked.
The crates.io version REST request returned 403; it is not evidence of a failed
crate or a reason to alter credentials. No credential was supplied.

Context7 `/ramosbugs/openidconnect-rs` was resolved and queried for custom HTTP,
discovery, token verification and logout. Its main-branch examples are discovery
guidance only. Exact crate source overrides those examples; never copy their
token-printing examples into Fasti.

The qualification harness should first test `default-features = false` with a
custom in-memory HTTP client. This is a candidate feature profile, not a changed
workspace dependency. Production TLS/HTTP must reuse the governed transport after
its POST/error-boundary review. The harness README records the resolved-graph
licence check and an unresolved RSA advisory; neither approves runtime adoption.
Secret custody, resource use and full production feature/lock qualification
remain open.

| Requirement | Exact source finding | Qualification obligation |
| --- | --- | --- |
| Discovery | `discover_async` requests metadata, checks issuer, then fetches JWKS | Bound and authorize each request before dispatch; reject substituted issuer before JWKS fetch |
| Signature, issuer, audience, expiry, nonce | Secure verifier defaults exist; other audiences are rejected by default | Positive and independent negative controls; never disable verification |
| Issued-at and assurance | Default `iat`, `auth_time` and `acr` hooks accept values | Use explicit reviewed time/assurance policy; test future, old and boundary values |
| `azp` | The apparent check is inside a block comment; no implementation | Reject unsafe `azp` through checked claims; do not claim the library checks it |
| Access-token hash | Library supplies `AccessTokenHash`; caller compares it | Matching/mismatched hash and signing-algorithm tests; no token disclosure |
| `nbf`, encrypted tokens | No built-in not-before verifier surface; JWE unsupported | Keep requiring profiles unavailable until separate pinned support is qualified |
| RP-initiated logout | `ProviderMetadataWithLogout` and `LogoutRequest` discover/build navigation | Prove behavior without retaining ID tokens; do not use an ID-token URL hint by default |
| Front/back-channel logout | No receiver or logout-token verifier in this crate | Separate pinned receiver/JWT qualification; never repurpose ID-token validation as logout validation |

Pinned references:

- [Manifest and features](https://docs.rs/crate/openidconnect/4.0.1/source/Cargo.toml).
- [Discovery](https://github.com/ramosbugs/openidconnect-rs/blob/b639b5d39eac6903238867aeb2b29326502e6b26/src/discovery/mod.rs).
- [Verification](https://github.com/ramosbugs/openidconnect-rs/blob/b639b5d39eac6903238867aeb2b29326502e6b26/src/verification/mod.rs).
- [Claims](https://github.com/ramosbugs/openidconnect-rs/blob/b639b5d39eac6903238867aeb2b29326502e6b26/src/id_token/mod.rs).
- [Logout](https://github.com/ramosbugs/openidconnect-rs/blob/b639b5d39eac6903238867aeb2b29326502e6b26/src/logout.rs).

## Dependency-ordered gates

1. **E1-Q1 — exact dependency and hostile-input qualification.** Independently
   review this plan. Build a small isolated, locked, network-free test harness
   against the exact candidate. Test discovery request order, issuer substitution,
   S256 PKCE request shape, nonce, signature, audience, `azp`, expiry, explicit
   issued-at checks, malformed/oversized input and key-set replacement. Use
   synthetic secrets only. Record actual commands, source hashes and limits.
   A green harness qualifies only tested library behavior, not durable sign-in.
   State comparison is caller-owned: matching/missing/mismatched callback state
   must prove zero token requests for invalid state when the existing ceremony
   owner is integrated. A test-only state guard cannot close that production gate.
2. **E1-Q2 — transport and lifecycle contract.** Map every outbound request,
   credential attachment, bounded body/error path, key refresh and cancellation
   to an existing owner. Avoid retrying a consumed code. Resolve receiver support,
   endpoint policy, token-discard-compatible logout and current account-lifecycle
   proof. This gate may name unsupported profiles but cannot drop required MVP
   capabilities. No default administrative credential is an integration token.
3. **E1-I1 — shared domain and persistence.** After M4 handoff, allocate forward
   migrations with C2 coordination. Add unique external `(issuer, subject)` links,
   durable single-use state/nonce bindings and session provenance/indexes through
   existing owners. Preserve every published migration/archive. Link only to the
   expected subject after the original active session and recent authentication
   pass same-site completion. Email/username/group equality never authorizes a link.
   Determine unlinked-account provisioning through documented TrailBase APIs;
   do not invent an account API or silently change identity ownership.
4. **E1-I2 — orchestration and contracts.** Claim the ceremony once; take its
   zeroizing process-memory-only PKCE verifier; validate provider proof outside
   SQLite; discard provider tokens; recheck installation, subject, membership,
   epochs and ceremony in the final transaction before issuing the existing
   session. Reuse the 64-entry PKCE bound and fail-closed restart behavior. Stage
   identity-link proof for same-site completion; never commit linking in callback.
   Generate applicable OpenAPI, JSON Schema, SDK, problems and examples together.
5. **E1-I3 — logout and A+C.** Persist only non-secret provenance: external link,
   issuer, subject, optional `sid`, `auth_time`, approved `acr` and `amr`,
   authentication method, mapped assurance and verification time.
   Prove local logout plus provider logout, `sid` family and subject-wide fan-out,
   one-use logout `jti`, and repeated/outage behavior. Use Tabler first in permanent
   A and separate resumable C; B remains in-context evidence. Account controls
   remain visible with precise unsupported-state reasons and next actions.
6. **E1-D — exact delivery.** Focused/hostile tests, real ordinary-browser flow,
   concurrent tabs, callback copying, lost response, cancellation, restart,
   disable/delete/link conflict, revoked membership, offline existing sessions,
   secret scan, resource bounds, migration/restore and applicable canonical PR
   gates must pass on the final reviewed tree. Run independent review, CSO,
   Ponytail, developer-experience and applicable QA/design/Impeccable gates.
   Commit coherent slices; merge a green dependency-ready PR to `dev` and verify
   the merged tree. Do not promote `release`.

## Evidence and exclusions

### Q2 source map — integration remains gated

Read-only review used Fasti base `62e10d2e`, pinned `openidconnect 4.0.1`
and its locked `oauth2 5.0.0` dependency. OAuth2 archive SHA-256
`51e219e79014df21a225b1860a479e2dcd7cbd9130f4defd4bd0e191ea31d67d`
matches the isolated lock; embedded VCS is
`f3424b4b2190c83c6d031fdc71eed2351d49e0df`.

| Operation | Existing owner and source | Required integration work |
| --- | --- | --- |
| Discovery/JWKS GET | `fasti-provider-runtime/src/transport.rs:146–179`; `fasti-application/src/outbound_access.rs:100–142`; OIDC `discovery/mod.rs:308–336` | Authorize each configured/discovered endpoint independently. Existing transport declarations require static lifetimes; give dynamic identity configuration a real owner, never leak strings or inherit metadata-provider authority. |
| Code exchange POST | OAuth2 `token/mod.rs:190–235`, `endpoint.rs:72–154`; C1 `fasti-api/src/trailbase.rs:929–960` | Extend the existing governed owner after handoff. Current authorized client exposes GET only (`transport.rs:199`). Bind exact endpoint and method, not only origin; authorize before loading client credentials or constructing secret-bearing requests. |
| Userinfo GET | OIDC `user_info.rs:176–187,302–330` | Preauthorize the endpoint; pass the subject from verified ID claims. Validate token header syntax before invoking the library. Its bearer constructor can panic and does not mark the header sensitive. Normalize sensitive headers in the governed conversion. |
| Browser authorization/logout | Library URL builders and existing C1 ceremony/browser owner | Validate destination and return URI independently of server-fetch policy. Token-free logout construction does not prove provider acceptance. Do not retain an ID token just for a logout hint. |
| Refresh/revocation POST | OAuth2 `token/mod.rs:310–339`, `revocation.rs:255–307`; OIDC `client.rs:511,1278` | Same governed POST owner. Revocation URL needs explicit configuration; do not invent discovery support. Default sign-in retains no refresh token; persistence requires the separate named capability and C3 custody gate. |
| Response/error processing | `transport.rs:105–124`; OAuth2 `error.rs:111–134`; OIDC `user_info.rs:200–204` | Bound stream reads before vendor parsing. Vendor errors can retain raw response bytes. Convert to fixed Fasti errors; never debug/log/serialize vendor errors or secret-bearing requests. |
| Cancellation and one use | C1 `trailbase.rs:500–508,628–699,1107–1149`; store `human_access.rs:325–405` | Reuse claim/take/cancel and the 64-entry vault. Dispatched-but-unknown exchanges cannot be retried. Recheck final authority/epochs and account lifecycle; no new ceremony store. |

The current resolver allows at most eight answers and five seconds; transport
concurrency is four (`transport.rs:11–14`). These separate queue/DNS/HTTP bounds
are not one end-to-end ceremony deadline. Preserve them and define the complete
deadline before integration. No new numeric production limit is chosen here.

**Scoped custody review remains open.** `oauth2::PkceCodeVerifier` is an ordinary String
wrapper (`types.rs:417–423`, macro `154–175`), and endpoint serialization creates
ordinary encoded String/Vec buffers (`endpoint.rs:149–154`) before HTTP dispatch.
The C1 zeroizing vault cannot establish erasure of those hidden transient copies.
Likewise, `IdToken::into_claims` does not itself prove token-buffer erasure.
No-persistence/discard and in-memory erasure are distinct claims. Preserve
bounded zeroizing Fasti-owned PKCE custody, one-use transfer, no persistence,
narrow secret lifetimes, validated ingress and secret-safe transport/errors.
Dropping ordinary transient buffers does not prove their erasure. Universal
erasure of hidden vendor/network copies is unproven and must not be claimed,
but it is not an approved prerequisite for runtime adoption. No test-owned
protocol substitute or weakening of the explicit Fasti-owned custody rule.

Independent requirement-fidelity review corrected the earlier overbroad gate:
canonical section 6.1 requires at most 64 zeroizing custody entries; the C1 gate
at lines 239–244 permits vendor values to be zeroized or dropped at the narrowest
boundary. Merged C1 already serializes borrowed zeroizing values into bounded
ordinary request bytes (`trailbase.rs:934–945,1228–1230`). Its source blob
`bf263bfad92e19b9c5d0f0bc2c29e5159e02d3ef` matches merged C1. This is evidence
of the approved boundary, not proof that all C1 or OIDC memory is erased.

These source findings permit further isolated qualification, not production I1.
The minimum eventual integration is the existing transport's governed POST and
bounded/redacted request conversion plus its existing ceremony owner. It is not
another OAuth engine, DNS resolver, HTTP pool, scheduler or session platform.

### Completed isolated check segment

Before Q2 transport integration, the isolated harness was extended to
verify the exact token POST and PKCE/client/redirect form, missing-ID-token caller
obligation, one request on lost-response failure, retained vendor error bodies,
and user-info subject matching against verified ID-token claims. Use synthetic
inputs and the existing in-memory HTTP seam. Do not add a test-owned callback,
transport policy, token store, custom protocol or network service. A passing
wire-shape check cannot prove credential-read ordering or production erasure.
Run the same isolated offline/network-namespace, lint and independent-review
checks, then record exact source and remaining Q1/Q2 obligations.

### A+C integration test reuse — read-only preparation

The 2026-09-05 parallel preparation review mapped existing owners on base
`62e10d2e`; it changed no UI and ran no browser tests. Extend the existing
`packages/ui/src/account-security-view.svelte` and
`tests/e2e/access-c1.spec.ts` after M4 releases shared files.

| Required behavior | Existing owner to extend | Remaining proof |
| --- | --- | --- |
| Permanent A, separate resumable C | `task_map` / `first_run`, server-projected `firstIncompleteStep`; A/C separation test | OIDC linking resumes only confirmed server state; no second wizard store. |
| Leave versus cancel | Existing preserve-versus-cancel test and completion notice focus | Real ceremony cancellation and expired/lost callback behavior, not fixture DELETE alone. |
| Failure and recovery | Persistent problem/next-action UI and stale-result tests | Denied linking, provider outage and expired recent authentication preserve safe state. |
| Named Authentik management | Existing visible external-identity region | Implement the planned states/actions through real management authority; its current unavailable control is not management support. |
| Remove management credential | Canonical section 12.2's separate OIDC and management ownership | `Manual management` preserves OIDC/public discovery; repair is unavailable, not falsely reported complete. Removal is not disconnect or account deletion. |
| Keyboard and accessibility | Existing focus/target helpers, theme/reflow/Axe matrix and shell skip-link | Extend to new controls; real accessible-authentication journeys and clause evidence remain required. Fixture/Axe passes alone do not establish conformance. |

Intake performed: official sparse-index identity matched archive SHA-256; nine
selected source files were compared byte-for-byte with archive members, including
manifest, VCS metadata, licence, verifier, discovery, claims, logout, client and
library root. The subsequent isolated [qualification harness](../../qualification/access-e1/README.md)
now has nineteen passing library checks, also run in a fresh network namespace, with
strict Clippy passing. Its README binds results to file hashes and lists the
remaining Q1/Q2 evidence. This is not a clean-head delivery or runtime claim.

No API/SDK/schema/manifest/lock/runtime/UI changes occur in Q1's initial document.
Independent source/plan review on 2026-09-05 confirmed Q1/Q2 independence and the
listed public API findings. Its two corrections, caller-owned callback-state
verification and complete provenance, are incorporated above. The next bounded
writer scope is `qualification/access-e1/` only: an isolated Cargo workspace,
its own lockfile and test files; root Cargo files remain unchanged.
That scope now also contains its README and two synthetic signing-key fixtures.
Independent final review found no actionable issue in the request-shape and
explicit key-replacement checks. Caller-owned state, governed transport, real
provider conformance and all production gates remain open.
AsyncAPI and JSON-LD activation are not applicable to this source-intake artifact;
their final E1 disposition follows the capability registry, not this document.
Headless qualification does not prove accessibility or runtime performance.
Rendered A+C must later pass keyboard, reflow, focus, accessible-authentication,
AskTog/Gestalt/Nielsen/IxDF, WCAG 2.2 AA and applicable EN 301 549 evidence.

Packaged Tauri authentication, local HTTPS/certificates and WebKit transport remain
deferred under C1-TAURI-AUTH. Keep Secure cookies and per-install TrailBase admin
versus per-person credential boundaries unchanged. Codex Security is prohibited;
ordinary source/security reviews remain required. AGY is optional and additive.

Rollback of qualification is removal of its isolated harness only. Preserve
evidence; do not reset another worktree. Production rollback must be specified
with the actual migration and session invalidation design before I1 starts.

## Qualification CI delivery segment

The Commander also owns the new, isolated
`.github/workflows/access-e1-qualification.yml`. Reuse the repository's pinned
checkout/toolchain/install actions and Rust 1.97.1. Run the locked offline
19-check suite, formatting and strict Clippy explicitly; root workspace tests
do not include this package. Keep read-only permissions and no services or
credentials. Run the isolated lockfile's advisory audit as a separate visible,
fatal check with no ignore or continue-on-error. An unresolved advisory cannot
be represented as a green delivery. Validate workflow syntax and execute the
same local commands before commit. This adds no runtime integration or shared
file release and leaves existing workflows unchanged.

## GSTACK REVIEW REPORT

2026-09-05 developer-experience audit covers the isolated qualification package,
not Fasti's user onboarding or deployed documentation. Root README and CI select
Rust 1.97.1; the run instructions now select it explicitly. The prepared host ran
all 19 checks in a fresh network namespace, strict Clippy and rustfmt successfully.
The first 1.97.1 test build plus execution took 9.44 seconds with cached dependency
archives. This is not clean-machine setup time or a runtime performance result.
The earlier warm 1.96.0 run took 0.41 seconds. Neither is a cold-install benchmark.

| DX dimension | Score | Evidence and remaining gap |
| --- | --- | --- |
| Getting started | 8/10, tested | Explicit toolchain and commands work on the prepared host; pristine-machine installation is untested. |
| CLI ergonomics | 8/10, tested | Reuses Cargo help and test listing; no custom command wrapper. |
| Errors | 8/10, tested/partial | Invalid flags give usage/help. An unknown test filter exits 0 with 19 filtered tests; README requires 19 passed and none filtered. Missing-cache recovery is documented from the earlier observed failure. |
| Documentation | 8/10, source-reviewed | Setup, expected output, public fixtures and runtime exclusions are explicit; no new web surface to browse. |
| Upgrade path | Not applicable | No runtime adoption or database upgrade in this package; version changes must repeat qualification. |
| Environment | 8/10, tested/partial | Offline and namespace checks pass; root canonical checks do not run this isolated workspace. At this audit CI coverage was absent. The dedicated workflow is now added; live execution remains unproven. |
| Community and DX measurement | Not applicable | No new community or telemetry surface. Existing repository contribution paths remain unchanged. |

No comparable E1 plan-DX score was present in the branch review log. Independent
read-only onboarding review found no additional wording issue after corrections.
Ponytail review retained standard Cargo commands; no wrapper or new dependency.

**VERDICT:** DX `DONE_WITH_CONCERNS` for qualification only. Existing source review
predates these documentation edits and needs final-tree reconciliation. The
historical clean `b9477621` canonical receipts passed 27 contract and 11 portable
gates; a new delivery head needs fresh receipts and PR evidence.

**UNRESOLVED DECISIONS:**
- The RSA advisory remains unsuppressed and the advisory gate is not green; no runtime adoption is approved.
- Production E1-Q1/Q2 completion and E1-I1/I2/I3 remain open, with shared integration subject to M4's exact handoff.
