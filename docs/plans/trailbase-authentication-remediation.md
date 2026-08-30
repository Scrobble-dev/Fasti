# Fasti Access and TrailBase authentication programme

Status: `APPROVED`

Owner: Commander / Mothership

Target: PR #93 first, then dependency-ordered PRs to `dev`

Last source refresh: 2026-08-30

Implementation state: `GATE_11_IN_PROGRESS`

This is the canonical execution plan. Apply the comprehensive Fasti Access correction first. Apply the Authentik management correction second. The two corrections are cumulative.

No production implementation can start until the final plan gate is `APPROVED`. Every work package has a written plan, review, QA, rollback, and merge gate. A skipped plan is a failed gate.

## 1. Evidence and precedence

Labels:

- `OBSERVED`: current Fasti source, exact PR diff, live GitHub state, or exact-head evidence.
- `VERIFIED`: current primary documentation and pinned source or artifact agree.
- `PROPOSED`: required design that is not implemented or proven.
- `BLOCKED`: safe work cannot continue until named evidence or authority exists.
- `REJECTED`: inspected and deliberately excluded.
- `SUPERSEDED`: replaced by a later explicit user decision.

Source order:

1. Current Fasti source and exact PR #93 head.
2. Exact-head CI, QA, review, security, package, and performance evidence.
3. Current Fasti constitution, architecture, capability ledger, contracts, and security rules.
4. This approved canonical plan.
5. The user's cumulative prompts and current explicit corrections.
6. Current official protocol, TrailBase, library, provider, Authentik, and Nuvio sources.
7. The attached authentication report as research evidence.
8. Branch-matched gstack artifacts and checkpoints as historical context.
9. The behavioral Nuvio branch as behavioral evidence only.
10. Older reports, plans, and chats.

Do not use a prior assistant response as source evidence. Do not invent an API, route, configuration key, field, framework capability, test result, performance result, security result, or accessibility result.

## 2. Context manifest

| Item                                 | Evidence                                                                                              | State                                                  |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Remote PR #93 head                   | `2605819740cd49f4002ee533be0e0b7180828c55`                                                            | `OBSERVED`                                             |
| Local Gate 11 implementation content | `99c10c068110bacb8a98bcbf487ccd34c0d03f2e`; tree `beefcdea2e1148a4e87646510d72c31470831fc0`           | `OBSERVED`; not pushed                                 |
| Live `origin/dev`                    | `d035933bd2b804f23db1a5402ee564eba7ce5b0c`                                                            | `OBSERVED`                                             |
| Merge base                           | `0d1c729389a281afe0e4e8557fb30708f4c5d33d`                                                            | `OBSERVED`                                             |
| Divergence                           | PR has 42 unique commits; `dev` has 5                                                                 | `OBSERVED`                                             |
| Mergeability                         | Open draft; remote head remains `CONFLICTING` / `DIRTY` until the reconciled implementation is pushed | `OBSERVED`                                             |
| Existing checks                      | All 19 reported checks pass on the exact PR head; none covers reconciliation with current `dev`       | `OBSERVED`; reconciliation unproven                    |
| Review threads                       | 46 total; 29 unresolved: 21 current and 8 outdated                                                    | `OBSERVED`                                             |
| Safe worktree                        | `/home/ryan/code/fasti/.claude/worktrees/fasti-port-conflict-f3c778`                                  | `OBSERVED`                                             |
| Quarantined checkout                 | `/home/ryan/code/fasti` is detached, dirty, and mid-rebase                                            | No writes                                              |
| Release status                       | No product releases; workspace `0.1.0`, `publish = false`; Workbench is pre-production                | `OBSERVED`                                             |
| TrailBase candidate                  | `v0.33.5`, tag `b4c85d5152d4e5f472e0b5da5303f7c938e3a083`                                             | `VERIFIED` candidate                                   |
| TrailBase OCI index                  | `sha256:43677ebfb5493a2bdb85212570d970ef6c7bdb5392f9295f37683a2f2de149e6`                             | `VERIFIED` candidate                                   |
| Linux x86_64                         | SHA-256 `30ee948182a2b05698767b3b2bfb2d57ef0e92b0546fd0f1dc6ca03949b34db6`                            | `VERIFIED` candidate                                   |
| Linux arm64                          | SHA-256 `846733ac61e40d6972fd2e02303730022f7f946cb825656d48efa6821d5dbfd9`                            | `VERIFIED` candidate                                   |
| TrailBase status                     | Project is labelled alpha                                                                             | Operator and release risk                              |
| Attached research                    | `Fasti Rust Authentication Architecture.md`                                                           | Defect evidence; framework recommendation `SUPERSEDED` |
| Historical artifacts                 | `gstack-artifacts-winks`                                                                              | Reference only; may be stale                           |

Refresh live refs, releases, issues, digests, and checks at the start of each package. Historical green evidence does not prove a later head.

### 2.1 Gate 11 implementation checkpoint

Recorded: 2026-08-29

| Item                                     | Exact evidence                                                                                                                                                                                                                                         | State                                      |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------ |
| Approved design                          | `approved.json`; every recorded artifact SHA-256 matched the current file                                                                                                                                                                              | `VERIFIED`                                 |
| Remote PR #93 head before reconciliation | `2605819740cd49f4002ee533be0e0b7180828c55`; tree `b787bc5da5f5c2c80cdd0833596b3d9a0274874f`                                                                                                                                                            | `OBSERVED`                                 |
| Verified implementation head             | `24fcc39a9276d480cefa18ad29635dd96a7953b2`; tree `5b9db5903446499a0768cb5a3ff2f92aaeeecc3b`; clean source-bound contract receipt and 82/82 Playwright tests                                                                                            | `COMPLETE_WITH_LOCAL_EVIDENCE`; not pushed |
| Current `origin/dev`                     | `d035933bd2b804f23db1a5402ee564eba7ce5b0c`                                                                                                                                                                                                             | `OBSERVED`                                 |
| Merge base and divergence                | `0d1c729389a281afe0e4e8557fb30708f4c5d33d`; 5 `dev` commits and 42 PR commits                                                                                                                                                                          | `OBSERVED`                                 |
| Reconciliation                           | Merge commit `fbfdb954c3a478f9b68ecddd30a70b66b896b8ba` has parents `3c45c2bdfd1d9092e25bbe9ae7db6e350014b93b` and `d035933bd2b804f23db1a5402ee564eba7ce5b0c`; the only content conflict was `.codacy.yml`, resolved by retaining both engine policies | `COMPLETE_WITH_LOCAL_EVIDENCE`             |
| PR state                                 | Open draft; `CONFLICTING` / `DIRTY`; base `dev`                                                                                                                                                                                                        | `OBSERVED` after draft conversion          |
| Exact-head checks                        | 19 successful checks on the current PR head; no check proves reconciliation with current `dev`                                                                                                                                                         | `OBSERVED`                                 |
| Delivery boundary                        | Implement PR A in this isolated worktree. Use later isolated worktrees only after dependency and file ownership freeze.                                                                                                                                | `IN_PROGRESS`                              |

PR A implementation review checkpoint, before this plan-only update:

| Area                      | Exact local evidence                                                                                                                                                                                        | State                                                                      |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Truth reset               | Fake password, passkey, TOTP, recovery-code, OIDC, development-account, and browser-session HTTP surfaces removed; representative routes return 404                                                         | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| DDD session core          | `AuthSubject` and `FastiBrowserSession` invariants in `fasti-domain`; commands and ports in `fasti-application`; digest-only secrets and transactions in `fasti-store`                                      | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| Authorization             | Session grants are explicitly subject-owned; creation and every authentication recheck workspace, grant, and owning-client state                                                                            | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| Browser mutation boundary | Origin, Host, and CSRF proof required by dormant mutation commands; missing and mismatch negative tests pass                                                                                                | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| Session policy            | Idle, absolute, remembered, and bounded last-seen behavior implemented with no default; zero, subsecond, and invalid ordering fail before storage                                                           | `COMPLETE_WITH_LOCAL_EVIDENCE`; exact production values remain `C1-POLICY` |
| Migration and rollback    | Populated v9 forward migration, injected atomic failure and retry, restart, closed-copy v9 rollback, and unrelated-row preservation pass                                                                    | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| Developer reset           | `--reset-access` reports Access-only reset unavailable without mutation; separately confirmed `--full-dev-root` retains a recoverable backup and rebuilds through normal daemon migration and public probes | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| Gate 10 A+C               | Permanent Account and security task map plus separate first-run guided setup; no actionable fake authentication controls                                                                                    | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| UI QA                     | Full Playwright UI suite: 82 passed at `24fcc39a9276d480cefa18ad29635dd96a7953b2`; Tabler, axe, forced-colors, reduced-motion, keyboard, and 320-1920 px coverage retained                                  | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| Reviews                   | Independent session-security and contract/UI reviews cleared exact implementation head `24fcc39a9276d480cefa18ad29635dd96a7953b2`; no code-level blocker remains                                            | `COMPLETE_WITH_LOCAL_EVIDENCE`                                             |
| Delivery                  | Local only. No push, remote review-thread resolution, CI, merge, or verified `dev` evidence yet.                                                                                                            | `PENDING`                                                                  |

Current file ownership:

| Owner                       | Paths or state                                                                                                                                                                                                                                                                                                                                                                              | Rule                                                                                                                                                                                  |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Commander                   | This plan, Git and GitHub state, integration, PR A package sequencing, and exact-head evidence                                                                                                                                                                                                                                                                                              | Sole writer in the PR #93 worktree until the PR A gap audit fixes file ownership.                                                                                                     |
| `pr93_live_inventory`       | GitHub and PR evidence                                                                                                                                                                                                                                                                                                                                                                      | Read-only.                                                                                                                                                                            |
| `pr93_slice_gap_audit`      | PR #93 source and requirement audit                                                                                                                                                                                                                                                                                                                                                         | Read-only.                                                                                                                                                                            |
| `fasti_handoff_evidence`    | Handoffs and historical context                                                                                                                                                                                                                                                                                                                                                             | Read-only.                                                                                                                                                                            |
| `pr93_core_session`         | `crates/fasti-domain/src/ids.rs`, required existing domain re-export only, `crates/fasti-application/src/browser_auth.rs`, `crates/fasti-application/src/capabilities.rs`, `crates/fasti-application/src/authorization.rs`, `crates/fasti-application/src/problems.rs`, `crates/fasti-store/src/browser_auth.rs`, `crates/fasti-store/src/schema.rs`, and focused Rust tests in those files | Isolated branch `codex/fasti-pr93-core`; sole owner of the session model, policy, persistence, migration, capabilities, problems, and store invariants.                               |
| `pr93_ui_truth`             | `packages/ui/src/auth-modal.svelte`, `packages/ui/src/runtime-settings-view.svelte`, `packages/ui/src/fasti-workbench.svelte`, `packages/ui/src/types.ts`, `apps/web/src/web-host.ts`, and `tests/e2e/workbench-regressions.spec.ts`                                                                                                                                                        | Isolated branch `codex/fasti-pr93-ui`; preserve Gate 10 A+C and remove false active behavior. Do not edit generated SDK or registry files.                                            |
| `pr93_docs_truth`           | `AGENTS.md`, `SECURITY.md`, `docs/architecture/authentication.md`, `docs/capability-ledger.md`, `docs/quality/`, `docs/reviews/pr-93-auth-session-review.md`, and `contracts/seed/fasti_fresh_blind_master_plan_v2_2026-08-21.md`                                                                                                                                                           | Isolated branch `codex/fasti-pr93-docs`; correct false claims and record implemented versus unavailable state. Do not edit registry or generated files.                               |
| `pr93_stale_docs`           | `README.md`, `packages/sdk/README.md`, `docs/network-configuration.md`, and `docs/architecture/adr-0005-framework-and-auth-adoption.md`                                                                                                                                                                                                                                                     | Follow-up on isolated branch `codex/fasti-pr93-docs`; remove superseded development-account/session instructions and state the final TrailBase decision without changing other files. |
| Commander after core freeze | `crates/fasti-contracts`, `crates/fasti-api`, `contracts/registry`, `contracts/generated`, `xtask`, `packages/sdk`, contract tests, and integration                                                                                                                                                                                                                                         | Begin only after the core owner freezes identifiers, policy, capabilities, and problems.                                                                                              |

PR A source-gap gate, recorded 2026-08-29:

- Exact audit basis: remote PR head `2605819740cd49f4002ee533be0e0b7180828c55` against `origin/dev` `d035933bd2b804f23db1a5402ee564eba7ce5b0c`.
- Result: `FAIL_EXPECTED_BEFORE_IMPLEMENTATION`. Fake passkey, TOTP, recovery, and OIDC success remains mounted; historical v8 is edited; `BrowserUser` owns identity; final session policy, exact public IDs, complete revocation, rotation, Origin/Host checks, session-local authorized profile selection, truthful contracts, and meaningful UI tests are absent.
- Retained evidence: Account and Sessions information architecture, digest-backed secret storage, secure cookie and CSRF foundations, and administrator-continuity behavior have value and must not be discarded.
- Dependency result: no external dependency blocks PR A. TrailBase runtime and protocol work remain in B and C and must not be simulated here.
- Implementation order: remove false surfaces; restore migration history and add a forward migration; define the final dormant session model and policy; implement store invariants; add request-boundary checks; regenerate contracts and SDK; apply the persistent Gate 10 A+C unavailable state; add focused tests; correct documentation and executable truth gates.

### 2.2 PR B implementation checkpoint

Recorded: 2026-08-29

| Item                    | Exact evidence                                                                                                                                                                                                                                        | State                                     |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| Gate 10 approval        | `approved.json` records `A+C`; SHA-256 values for variant A, variant C, the 1440 board, design HTML, CSS, and Gate 10 review match the current files                                                                                                  | `VERIFIED`                                |
| Canonical plan          | This file has SHA-256 `34df49d0a458ea1b2bb08500880acc6736bd38cf42f3b6e9bcab82ea5aa1e890`, equal to the approved PR #93 planning copy                                                                                                                  | `VERIFIED` before this checkpoint edit    |
| PR A delivery           | PR #93 merged to `dev` as `adbdef3038786b0efb2ec615bce080e3eaa9361f`; tree `a7a1f661ae1b0ef4470ba736d65942f54793d1b0`                                                                                                                                 | `COMPLETE_WITH_EVIDENCE`                  |
| PR B worktree           | `/home/ryan/code/fasti/.claude/worktrees/fasti-trailbase-runtime-b`; branch `codex/fasti-trailbase-runtime-b`                                                                                                                                         | `IN_PROGRESS`                             |
| PR B base               | Local `HEAD` and `origin/dev` are both `adbdef3038786b0efb2ec615bce080e3eaa9361f` with tree `a7a1f661ae1b0ef4470ba736d65942f54793d1b0` after `git fetch --all --prune`                                                                                | `VERIFIED`                                |
| Release boundary        | The only open pull request is PR #89 from `dev` to `release`; PR B does not modify, merge, or promote it                                                                                                                                              | `OUT_OF_SCOPE`                            |
| TrailBase release       | `v0.33.5`; tag commit `b4c85d5152d4e5f472e0b5da5303f7c938e3a083`; exact native archives and executables, OCI index/platform graph, and reviewed licence digest are enforced by `third_party/trailbase/release.json`                                   | `VERIFIED` runtime inputs                 |
| Implementation boundary | Extend existing launcher, verification, packaging, contract, and documentation owners. Do not add a second supervisor, application framework, authentication platform, or Fasti session exchange in PR B.                                             | `IN_PROGRESS`                             |
| Prepared-machine gate   | `cargo xtask test milestone --body B` at `7d5f5265bae9b4bba92a000ddee8eb935dbc48d6`; source tree `10b18c720d697ad569d153906f72668c6876d215`; all eight gates and 39 account, social, recovery, security-boundary, upgrade, and rollback checks passed | `VERIFIED` before this checkpoint update  |
| Resource boundary       | The combined Fasti and TrailBase startup-smoke peaked at 110 MiB under the unchanged 192 MiB aggregate ceiling with zero extra swap and one CPU                                                                                                       | `VERIFIED`; startup-smoke, not idle/soak  |
| Upgrade and rollback    | Exact test-only v0.33.4 artifacts; stopped digest-bound full-depot backup; isolated v0.33.5 activation/restart; old binary starts only against a fresh restore of the untouched old backup; schemas are identical                                     | `VERIFIED`; no database migration claimed |
| Runtime limits          | Social callbacks do not prove TOTP; refresh does not rotate; tokens omit `iss`, `aud`, `kid`, and `jti`; remote redirect-taking routes and isolated-admin MFA are unavailable; no passkeys, recovery codes, or documented per-account disable         | `VERIFIED`; keep unavailable or bounded   |
| Architecture evidence   | Native and OCI lifecycle executed on Linux x86_64; exact Linux arm64 native artifact and OCI platform graph are locked, but runtime execution requires a native arm64 runner                                                                          | `PARTIAL`; do not claim arm64 execution   |
| Competitor baseline     | [Ryot v10.5.0, Cinephage v0.16.0, and Yamtrack v0.26.3 exact-release comparison](../reviews/2026-08-30-access-competitor-comparison.md)                                                                                                               | `VERIFIED`; refresh at MVP close          |

PR B file ownership during source audit:

| Owner                     | Paths or state                                                                                                                         | Rule                                                                      |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Commander                 | Canonical plan, integration branch, Git and GitHub state, implementation commits, exact-head evidence, PR creation, and merge decision | Sole writer until the source audit freezes disjoint implementation lanes. |
| `pr93_dev_reset`          | Existing Fasti launcher, package, OCI, backup, restore, and supervision patterns                                                       | Read-only audit.                                                          |
| `pr93_session_review`     | Exact TrailBase `v0.33.5` source, routes, configuration, lifecycle, account flows, and limitations                                     | Read-only audit.                                                          |
| `pr93_contract_ui_review` | Contract, documentation, verifier, licence, artifact-pin, and negative-control matrix                                                  | Read-only audit.                                                          |

### 2.3 C1 implementation checkpoint

Recorded: 2026-08-30

| Item | Exact evidence | State |
| --- | --- | --- |
| C1 base | `origin/dev` `4546459105c8c762886b32cdbd580be3e039736c`; tree `6ccfa5d96064b51f3dcd80dfb95f00cd60ce5a55` | `VERIFIED` |
| Gate 10 artifacts | All six SHA-256 values in `approved.json` match the current A, C, board, HTML, CSS, and design-review artifacts | `VERIFIED` |
| Package B | PR #114 merged to `dev`; exact merged tree equals the reviewed PR tree | `COMPLETE_WITH_EVIDENCE` |
| C1 written gate | [`fasti-access-c1-trust-gate.md`](fasti-access-c1-trust-gate.md) | `BLOCKED_PRIMARY_SOURCE_CONFLICT` |
| TrailBase verification keys | Exact `v0.33.5` public OpenAPI has no JSON Web Key Set or public verification-key route. The source-only key route is administrator and CSRF protected. No supported overlap, version, rotation, or retirement API exists. | `BLOCKED` |
| TrailBase account state | The public status route can recheck only the current refresh-token subject and email-verification state. It has no arbitrary account lifecycle lookup or disabled/suspended state. | `BLOCKED` |
| Browser callback | Exact source supports Proof Key for Code Exchange and a code-only redirect. TrailBase does not round-trip Fasti state. The approved separate browser-binding cookie still requires real-browser proof. | `PARTIAL`; cannot repair the trust blocker |
| Metadata M2 ownership | Active uncommitted M2 owns migration version 12 and overlaps C1 schema, registry, generator, API, SDK, host, and Workbench files. | `WAITING_FOR_HANDOFF_OR_MERGE` |
| Safe state | No C1 production code, schema, contracts, routes, or runtime behavior changed. A documentation-only isolated gate worktree records the conflict. | `VERIFIED` |

Stop result: C1 and every package that depends on C1 must remain fail closed.
TrailBase remains the selected account platform. Do not replace it, read its
depot or private keys, call undocumented endpoints, or weaken `C1-TB-TRUST`.
Resume only when pinned official evidence supplies the approved public trust
and account-state capability, then reconcile with the Metadata M2 owner.

### 2.4 Primary source registry

Use exact versions and current primary sources:

- TrailBase: [release v0.33.5](https://github.com/trailbaseio/trailbase/releases/tag/v0.33.5), [authentication](https://trailbase.io/documentation/auth/), [OpenAPI](https://trailbase.io/api/), [login parameters](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/login_params.rs), [authorization response](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/login.rs), [JWT claims](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/jwt.rs), [refresh](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/tokens.rs), and [licence](https://github.com/trailbaseio/trailbase/blob/v0.33.5/LICENSE).
- Passkeys: [webauthn-rs 0.5.5](https://crates.io/crates/webauthn-rs/0.5.5), [exact docs](https://docs.rs/webauthn-rs/0.5.5/webauthn_rs/), and [WebAuthn Level 3](https://www.w3.org/TR/webauthn-3/).
- Rejected alternative: [oauth2-passkey 0.6.1](https://docs.rs/crate/oauth2-passkey/0.6.1) and its [architecture](https://github.com/ktaka-ccmp/oauth2-passkey/blob/v0.6.1/docs/src/getting-started/architecture.md).
- Generic OIDC: [openidconnect 4.0.1](https://github.com/ramosbugs/openidconnect-rs/tree/4.0.1), [exact docs](https://docs.rs/openidconnect/4.0.1/openidconnect/), [exact verifier source](https://github.com/ramosbugs/openidconnect-rs/blob/4.0.1/src/verification/mod.rs), and [exact logout source](https://github.com/ramosbugs/openidconnect-rs/blob/4.0.1/src/logout.rs).
- Fasti OAuth candidate: [oauth-as 0.9.3](https://docs.rs/crate/oauth-as/0.9.3). Treat its beta statement and “not battle hardened” limitation as part of E0.
- Authentik: [server 2026.8.0](https://github.com/goauthentik/authentik/releases/tag/version%2F2026.8.0), [server source](https://github.com/goauthentik/authentik), [OAuth provider](https://docs.goauthentik.io/add-secure-apps/providers/oauth2/), [provider creation](https://docs.goauthentik.io/add-secure-apps/providers/oauth2/create-oauth2-provider/), [dynamic registration](https://docs.goauthentik.io/add-secure-apps/providers/oauth2/dynamic-client-registration/), [exact DCR scope constant](https://github.com/goauthentik/authentik/blob/version/2026.8.0/authentik/common/oauth/constants.py), [logout](https://docs.goauthentik.io/add-secure-apps/providers/oauth2/frontchannel_and_backchannel_logout/), [API root](https://api.goauthentik.io/), [API authentication](https://api.goauthentik.io/authentication/), [API clients](https://api.goauthentik.io/clients/), [authentik-client crate](https://crates.io/crates/authentik-client), [authentik-client 2026.8.0 docs](https://docs.rs/authentik-client/2026.8.0/authentik_client/), [client release](https://github.com/goauthentik/client-rust/releases/tag/version%2F2026.8.0), and [canonical Rust client 2026.8.0](https://github.com/goauthentik/client-rust/tree/version/2026.8.0). The former `authentik-community/client-rust` URL redirects to this maintained `goauthentik` repository; use the maintained canonical source.
- OAuth and OIDC: [RFC 6749](https://www.rfc-editor.org/rfc/rfc6749), [RFC 7009](https://www.rfc-editor.org/rfc/rfc7009), [RFC 7591](https://www.rfc-editor.org/rfc/rfc7591), [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636), [RFC 7662](https://www.rfc-editor.org/rfc/rfc7662), [RFC 8414](https://www.rfc-editor.org/rfc/rfc8414), [RFC 8628](https://www.rfc-editor.org/rfc/rfc8628), [RFC 9207](https://www.rfc-editor.org/rfc/rfc9207), [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700), [OIDC Core](https://openid.net/specs/openid-connect-core-1_0.html), [Discovery](https://openid.net/specs/openid-connect-discovery-1_0.html), [RP logout](https://openid.net/specs/openid-connect-rpinitiated-1_0.html), [front-channel logout](https://openid.net/specs/openid-connect-frontchannel-1_0.html), and [back-channel logout](https://openid.net/specs/openid-connect-backchannel-1_0.html).
- Providers: [Open Library](https://openlibrary.org/developers/api), [Kitsu](https://hummingbird-me.github.io/api-docs/), [AniList](https://docs.anilist.co/), [MusicBrainz](https://musicbrainz.org/doc/MusicBrainz_API), [TMDB](https://developer.themoviedb.org/docs/authentication-application), [TVDB v4](https://thetvdb.github.io/v4-api/), [Google Books](https://developers.google.com/books/docs/v1/using), [MyAnimeList v2](https://myanimelist.net/apiconfig/references/api/v2), [RAWG](https://rawg.io/apidocs), [IGDB](https://api-docs.igdb.com/), [ComicVine](https://comicvine.gamespot.com/api/), and [Podcast Index docs](https://github.com/Podcastindex-org/docs-api).
- Nuvio: [NuvioTV](https://github.com/NuvioMedia/NuvioTV), [issue 2484](https://github.com/NuvioMedia/NuvioTV/issues/2484), [issue 2935](https://github.com/NuvioMedia/NuvioTV/issues/2935), and [issue 2967](https://github.com/NuvioMedia/NuvioTV/issues/2967).
- Competitor evidence: [dated exact-release comparison](../reviews/2026-08-30-access-competitor-comparison.md), [Ryot authentication](https://docs.ryot.io/guides/authentication), [Cinephage authentication schema](https://docs.cinephage.net/reference/database/schema-overview#authentication-better-auth), and [Yamtrack social authentication](https://github.com/FuzzyGrim/Yamtrack/wiki/Social-Authentication-in-Yamtrack).

## 3. Final decisions

1. TrailBase is the selected private, local human identity platform. Framework selection is closed.
2. TrailBase runs as a separate, unmodified executable and process with a separate data root. Pin one exact version and checksum or OCI digest. Do not use a floating tag.
3. Fasti uses documented TrailBase public APIs only. It does not access TrailBase tables or use TrailBase Record APIs for Fasti data.
4. TrailBase owns proven human account functions. Fasti Access owns application authorization, sessions, passkeys, clients, tokens, devices, scopes, grants, and audit.
5. Fasti is unreleased. Do not build a compatibility layer for PR #93 authentication. Do not add dual authentication, old-token translation, aliases, wrappers, shadow writes, or a user migration programme for users who do not exist.
6. Preserve unrelated developer data. Provide a deterministic fresh schema, development reset, fixtures, restart proof, and rollback. Unreleased does not authorize silent data loss.
7. DRY and Domain-Driven Design are mandatory. Each term, invariant, transition, capability, and public contract has one semantic owner.
8. Use existing Fasti owners, TrailBase, platform features, the Rust standard library, and mature focused dependencies before custom code.
9. Use Tabler before custom UI. Do not introduce another component system.
10. Apply Chesterton's Fence before deleting, hiding, or replacing a control. Trace callers, history, tests, screenshots, and user outcome. Preserve useful behavior and accessibility work.
11. Do not fake a security capability. A control can be visibly `Unavailable` while its implementation PR is unmerged. It cannot be absent at MVP completion.
12. Do not implement cryptographic primitives or protocol parsing by hand.
13. Use ASD-STE100 Simplified Technical English for plans, UI copy, documents, commits, PR comments, and reports.
14. Update applicable contracts and documentation in the same PR as behavior. Record an explicit `N/A` reason for a surface that does not apply.
15. Do not proceed to the next segment until the current segment's written plan and evidence gate pass.

### 3.1 Terms and plan-language record

| Term       | Meaning                                                                                                |
| ---------- | ------------------------------------------------------------------------------------------------------ |
| API        | Application programming interface                                                                      |
| AS         | OAuth authorization server                                                                             |
| ASD-STE100 | Simplified Technical English specification                                                             |
| CI         | Continuous integration                                                                                 |
| CLI        | Command-line interface                                                                                 |
| CSRF       | Cross-site request forgery                                                                             |
| DCR        | Dynamic client registration                                                                            |
| DDD        | Domain-Driven Design                                                                                   |
| DRY        | Do not repeat yourself                                                                                 |
| EN 301 549 | European accessibility requirements for information and communication technology products and services |
| JWT        | JSON Web Token                                                                                         |
| JWKS       | JSON Web Key Set                                                                                       |
| MFA        | Multi-factor authentication                                                                            |
| OAuth      | Open Authorization protocol family                                                                     |
| OCI        | Open Container Initiative artifact format                                                              |
| OIDC       | OpenID Connect                                                                                         |
| PAT        | Personal access token                                                                                  |
| PKCE       | Proof Key for Code Exchange                                                                            |
| PR         | Pull request                                                                                           |
| QA         | Quality assurance                                                                                      |
| RFC        | Request for Comments standards document                                                                |
| RP         | OpenID Connect relying party                                                                           |
| SDK        | Software development kit                                                                               |
| SSRF       | Server-side request forgery                                                                            |
| TOTP       | Time-based one-time password                                                                           |
| TTL        | Time to live                                                                                           |
| UI / UX    | User interface / user experience                                                                       |
| WCAG       | Web Content Accessibility Guidelines                                                                   |

Plan-language review record: on 2026-08-29, the design-plan review checked this canonical plan for direct language, active voice, explicit actors, defined acronyms, and one decision per requirement. Result: `PASS_WITH_TECHNICAL_EXCEPTIONS`. Exact type names, protocol names, standards titles, command names, paths, problem identifiers, evidence labels, and source quotations remain technical because changing them would reduce precision. Package plans repeat this review for new prose. User-facing copy has a separate stricter gate in section 10.6.

## 4. Root-cause investigation

PR #93 copied vocabulary and behavior from unrelated Django/allauth history without bringing a Django runtime or django-allauth. It then filled the gap with custom Rust password, TOTP, WebAuthn-shaped, OIDC-shaped, migration, and UI behavior. Conflicting historical plans and missing capability ownership let presentation, storage, and contracts claim features that no real service or protocol adapter provided.

| Area              | Current defect                                                                               | Required correction                                                                                                 |
| ----------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Migration         | Five auth tables were added by editing historical v8 while schema version stayed 9           | Remove unsupported additions. Establish a clean unreleased baseline or deliberate new migration for retained state. |
| Passkeys          | Client fallback and server storage can report success without a verified ceremony            | Remove in PR A. Rebuild with `webauthn-rs` in PR D.                                                                 |
| TOTP              | SHA1 URI conflicts with SHA256 verification; factor is not enforced; backup codes are unused | Delete custom TOTP. Use proven TrailBase TOTP with explicit assurance limits.                                       |
| OIDC              | Discovery endpoints are fabricated; protocol validation is absent                            | Remove in PR A. Rebuild the RP with `openidconnect` in PR E.                                                        |
| Authorization     | Mutations use a generic read capability                                                      | Give each mutation one first-class capability and transaction-bound check.                                          |
| Sessions          | Public IDs are digest prefixes; metadata is fabricated; last use is stale                    | Add exact opaque IDs and nullable observed metadata.                                                                |
| Profile selection | Selection is user-global and can create grants                                               | Make it session-local and limited to an existing authorized grant.                                                  |
| UI                | Direct requests, retained secrets, and false success bypass governed hosts                   | Use generated host capabilities, clear secrets, and present persistent truth.                                       |
| Connections       | Static status DTOs imply state without a governed aggregate                                  | Add one `Connection` aggregate in PR G.                                                                             |
| Evidence          | A review file says `passed` while the head conflicts and findings remain                     | Bind every result to exact source and artifact.                                                                     |

Prevention:

- Make the capability ledger machine-readable and authoritative.
- Add a vendor-adaptation ledger. Map behavior to Fasti owners without importing frameworks.
- Fail CI when active UI lacks backend proof, a write uses a read capability, a historical migration changes, a contract drifts, or evidence is stale.
- Add negative controls. A gate passes only when the deliberate defect makes the test fail.
- Require Context7 plus official pinned source before code uses an external API. If neither defines the API, stop the package.

## 5. User outcome: make users capable

Apply Kathy Sierra's _Badass: Making Users Awesome_. Success is the person's ability to complete and recover from a real task, not a feature count.

| Person                    | Performance context                 | Required result                                                                                                                                         |
| ------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| New person                | Establish secure access             | Register, verify email, sign in, add a factor, prepare recovery, and reach the intended Fasti task.                                                     |
| Returning person          | Resume safely                       | Recognize profile, session, expiry, and next action without learning protocol terms.                                                                    |
| Security-conscious person | Improve protection                  | Add TOTP and passkeys, name credentials, inspect use, and revoke suspicious access.                                                                     |
| Device user               | Pair Nuvio or a CLI                 | See client, scopes, profile, expiry, approve or deny, and later revoke.                                                                                 |
| Operator                  | Install and maintain                | Start Fasti and TrailBase, verify versions, back up, restore, upgrade, roll back, and diagnose without SQL.                                             |
| Authentik operator        | Connect an existing identity estate | Choose manual or managed mode, preview changes, protect unrelated objects, test sign-in, remove the management credential, and repair drift explicitly. |

Interaction rules:

- One primary action per step.
- Persistent state and errors. Do not depend on transient toasts.
- Save progress and restore context after interruption.
- Prefer recognition over recall and use progressive disclosure.
- Explain protocol terms only when the user must decide.
- State the safe state, reason, and exact next action.
- Do not use guilt, urgency, gamification, vague copy, or artificial time pressure.

### 5.1 Task storyboards

Before a package changes UI, its design artifact instantiates this storyboard. The artifact names the user's action, expected confidence, visible proof, interruption behavior, resume point, and recovery. Protocol names stay in operator detail unless the user must make a protocol decision.

| Journey                                        | First 5 seconds                                                                                 | First 5 minutes                                                                                                      | Long-term proof                                                                                         | Interruption and resume                                                                     | Recovery                                                                                                    |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| First secure account setup                     | Show the account task, current step, and one primary action                                     | Register, verify, sign in, add a supported factor, prepare the applicable recovery method, and enter Fasti           | Account, methods, recovery readiness, membership, and current session remain inspectable                | Save only non-secret progress; return to the last safe incomplete step                      | State what completed, what stayed safe, and the next available action                                       |
| Returning sign-in and expired-session recovery | Explain whether access expired, was revoked, or needs stronger authentication                   | Reauthenticate through the established method and return to the intended Fasti task                                  | Session inventory shows creation, last observed use, expiry, and revocation truth                       | Preserve the intended destination and non-secret form work                                  | Put focus on the recovery heading; never turn expiry into an unexplained redirect loop                      |
| Factor enrollment and lost-device recovery     | Distinguish sign-in method, second factor, and recovery                                         | Enroll, verify, name, save recovery material once, and confirm readiness                                             | Factor, passkey, and recovery inventories show independent state and last use where proven              | An unverified ceremony expires safely and can restart without a partial credential          | Offer only a method that can recover this exact loss; require recent authentication for destructive changes |
| Suspicious-session review                      | Mark the current session and the suspicious facts without inventing device or location          | Inspect, revoke one or all other sessions, and understand affected access                                            | Persistent audit and inventory show the outcome                                                         | A refresh or navigation preserves the selected session and confirmation target              | A failed revoke leaves the session state truthful and gives a retry or operator path                        |
| Nuvio or CLI pairing                           | Show client, scopes, profile, expiry, and approve or deny                                       | Complete device authorization, verify connection, and show the new device                                            | Device, client, grant, token expiry, last use, and revocation remain inspectable                        | Polling survives normal navigation; expired or denied codes do not silently restart         | Start a new code from the requesting device; never request a Fasti password there                           |
| Provider or service repair                     | Show whether the failure is credential, transport, permission, expiry, health, or configuration | Test, replace or refresh the governed credential, then run a separate connection test                                | Health, last success, last error, expiry, and provenance stay visible                                   | Clear secrets after every attempt; preserve non-secret endpoint and diagnostic context      | Keep cached data without treating absence as deletion; give the exact repair action                         |
| Authentik manual or managed setup              | Make mode, server, ownership boundary, and current step explicit                                | Validate, preview exact changes, approve, apply, test sign-in, and remove the management credential when appropriate | Version, owned object IDs, configuration drift, last validation, and rollback record remain inspectable | Persist a redacted dry-run and completed object IDs; resume at the next unapplied safe step | Roll back only Fasti-owned changes; report unrelated objects as untouched                                   |

### 5.2 Recovery boundaries

| Recovery path                        | Can recover                                                                                 | Cannot recover                                                                                          | User-visible next action                                                             |
| ------------------------------------ | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| TrailBase password reset             | A TrailBase password account when current TrailBase evidence proves the reset flow          | Lost OIDC-provider access, a Fasti passkey by itself, a removed membership, or an administrator lockout | Start the TrailBase reset flow and return to the intended Fasti task                 |
| Fasti recovery code                  | Fasti passkey access for the same linked `AuthSubject`, within the approved recovery policy | A TrailBase password, an external OIDC account, email ownership, or workspace membership                | Use one code, require replacement after use, and show remaining recovery readiness   |
| OIDC-provider recovery               | Access to that external provider under the provider's policy                                | TrailBase credentials, unrelated linked identities, or Fasti authorization                              | Recover at the provider, then retry the linked sign-in method                        |
| Another registered passkey or factor | Loss of one authenticator when another proven method remains                                | Loss of every sign-in and recovery method                                                               | Authenticate with the remaining method, then revoke and replace the lost credential  |
| Administrator recovery               | Membership and administrative continuity under the approved operator runbook                | Proof of a person's external identity without source evidence                                           | Use the audited break-glass procedure; never edit TrailBase or Fasti tables directly |

Default copy never calls a Fasti recovery code a password recovery code. Every recovery screen says exactly which account, factor, or authorization state it can restore.

## 6. Domain model and ownership

```text
TrailBase process
  human account credentials and proven account lifecycle
        |
        | documented APIs only
        v
Identity Integration adapters
  translate TrailBase and external OIDC types
        |
        v
Fasti Access
  AuthSubject, ExternalAuthLink, Membership, Role
  FastiBrowserSession, RecentAuthentication
  PasskeyCredential, RecoveryCode
  PersonalAccessToken, ApplicationClient, DeviceGrant
  CapabilityScope, ProfileGrant, Consent, Audit
        |
        +--> Credential Vault: CredentialReference and secret backends
        +--> Connections: lifecycle and health
        +--> Metadata: provider registry and attribution
        +--> Nuvio Interoperability: pairing, delivery, sync, recovery

Chronicle remains independent
  media identity, evidence, observations, occurrences, corrections,
  progress, saved state, watched state, lists, and receipts
```

Layer ownership:

```text
Domain       terms, invariants, and state transitions
Application  capabilities, authorization, orchestration, and ports
Contracts    projections of the same capabilities
Adapters     TrailBase, Authentik, HTTP, SQLite, Tauri, CLI, browser
UI           invokes host capabilities and presents state
```

TrailBase, Authentik generated-client, HTTP, SQLite, and Svelte types do not enter the domain.

Durable identity:

- `AuthSubject` is Fasti's stable local human reference.
- Every human `AuthSubject` has exactly one durable TrailBase anchor record. Its lifecycle is `active`, `disabled`, `deleted`, or `recovery_pending`. TrailBase remains the account foundation when the person signs in with Authentik, generic OIDC, or a Fasti-owned passkey.
- `ExternalAuthLink` uses `issuer identity + subject` for additional sign-in methods.
- Generic OIDC uses the exact validated issuer.
- TrailBase v0.33.5 tokens have no `iss`. Fasti assigns the TrailBase installation one stable `TrailBaseInstanceId`. It is separate from the configured origin, depot location, signing keys, and restore generation. Do not claim JSON Web Token issuer validation.
- Store TrailBase verification keys as versioned material with activation, overlap, retirement, source, and audit. A signing-key rotation does not create a new TrailBase installation or subject.
- A declared restore preserves `TrailBaseInstanceId` and advances its activation generation. A cloned or restored copy starts authentication-disabled and cannot issue a Fasti session until an operator explicitly activates it and proves the former deployment is fenced.
- Email, username, groups, display name, Authentik database ID, and TrailBase database ID are attributes, not durable identity.
- A claim change must not create a second person or silently grant an administrator role.

Identity-link rules:

- First account establishment proves or creates the TrailBase anchor through an exact supported TrailBase API. If the pinned release cannot safely create an externally authenticated anchor, OIDC-first and SSO-only enrollment are `BLOCKED`; do not create an OIDC-only `AuthSubject`.
- Link a later OIDC identity only after recent authentication to the existing anchor or an administrator-approved one-use link ceremony.
- Never auto-link by email, username, preferred username, group, or display name.
- Enforce uniqueness for the TrailBase anchor and every external `issuer + subject`.
- Losing or disabling an external provider does not delete the TrailBase anchor, Fasti subject, or Chronicle data.
- Unlink requires recent authentication and cannot remove the last usable sign-in and recovery path.
- SSO-only presentation can hide local sign-in, but it does not remove the TrailBase anchor.
- Duplicate, missing, changed, or deleted external subjects stop linking and require audited recovery.

Membership is separate from identity:

```text
unaffiliated
invited
pending_approval
active
suspended
removed
```

- First-local-administrator bootstrap uses PR C's separate one-use `access.identity.bootstrap` operation. The trusted CLI or packaged host must read the existing owner-only `<data_root>/bootstrap.secret`, verify descriptor-root ownership and permissions, and hold the existing exclusive data-root bootstrap lock. TCP loopback reachability is not authority. The operation proves one TrailBase anchor, verifies that no membership exists, and binds that anchor to the existing local workspace and administrator role in one transaction. It reuses the authority primitive but does not reopen or invoke the consumed first-client bootstrap endpoint.
- Concurrent first-administrator attempts permit one winner. Losing attempts change no identity, membership, role, profile, or grant.
- Later people enter through invitation or administrator approval. Acceptance is explicit and audited.
- An authenticated but unaffiliated person gets no Chronicle capability. The UI shows the safe state and exact invitation or approval action.
- Suspension revokes new access and follows the approved active-session revocation policy. Removal does not delete Chronicle data.
- A membership change never creates a media-profile grant implicitly.
- Reject any transition that removes the final viable administrator.

Browser sign-in:

```text
Browser -> Fasti creates one-use browser binding, PKCE verifier, return target
Browser -> pinned TrailBase authorization UI
TrailBase -> short-lived authorization code
Fasti -> server-side exchange through hardened client
Fasti -> validates bound instance and subject
Fasti -> checks subject, membership, role, profile grant, auth epoch
Fasti -> revokes the new TrailBase refresh session
Fasti -> discards TrailBase tokens
Fasti -> creates or rotates opaque Fasti browser session
Browser <- Secure, HttpOnly, SameSite cookie
```

If refresh-session cleanup fails, Fasti does not create a session. Existing valid Fasti sessions continue during a TrailBase outage until local expiry or revocation. New sign-in, factor change, linking, and recent authentication fail closed.

### 6.1 Authentication ceremony and callback boundary

The Fasti browser session cookie remains `SameSite=Strict`. A return from TrailBase or an external OpenID provider must not depend on that cookie.

Use one bounded durable `AuthCeremony` domain aggregate with protocol-specific wire bindings:

```text
purpose
protocol_kind
browser_binding_digest
optional_oidc_state_digest
optional_oidc_nonce_digest
pkce_verifier_protection
provider_or_trailbase_instance
allowlisted_return_target
created_at
expires_at
consumed_at
optional_bound_session_id
optional_bound_subject_id
validated_proof_state
```

- Accept a callback only on the exact configured HTTPS public origin and exact callback path.
- Every sign-in ceremony creates a high-entropy pre-authentication browser secret. Store only its digest. Send the secret in a separate `Secure`, `HttpOnly`, host-only, no-`Domain`, narrow callback-`Path`, one-use cookie. Its `SameSite` value and the provider response mode must be proven together in a real browser; the proposed top-level redirect profile uses `SameSite=Lax`. Clear it on success, denial, error, expiry, cancellation, and retry.
- TrailBase v0.33.5 does not accept or return caller `state`; its password/TOTP authorization response returns only the code. Fasti does not claim TrailBase state validation. Locate the TrailBase ceremony by the browser-binding digest, validate and atomically consume the binding before code exchange, then use the ceremony's protected PKCE verifier. If the exact TrailBase flow cannot preserve this cookie and binding on the exact callback origin/path, C1 is `BLOCKED`.
- Generic OpenID Connect requires state, nonce, S256 PKCE, and the browser binding. Validate all four at their owning boundary.
- Protect Proof Key for Code Exchange material as a short-lived secret. Consume the ceremony once in the same transaction that claims the validated proof.
- A sign-in callback can establish a new Fasti session without the Strict Fasti session cookie only after it validates the browser binding, ceremony, provider, proof, account state, and membership.
- An identity-link callback stages the validated external proof. It cannot commit the link. A same-site completion request must present the original active Fasti session, the expected subject, and recent authentication. A changed, missing, revoked, or expired session cancels the staged link.
- Two tabs cannot consume one ceremony or attach one proof to different subjects. Callback retry returns the stored terminal outcome without repeating a remote exchange or link mutation.
- Test a real browser with the Strict Fasti cookie absent on callback, valid pre-auth cookie, copied callback in a clean browser, missing or mismatched binding, sibling-subdomain cookie injection, replay, two tabs, changed session, lost cookie, expired ceremony, hostile return target, wrong callback origin/path, and callback retry.

### 6.2 Authentication assurance

Do not infer assurance from an enrolled factor or identity-provider claim. Record how the current session authenticated.

| Method                       | Session assurance                                                   | Recent-auth eligibility           | Sensitive-action rule                                                                               |
| ---------------------------- | ------------------------------------------------------------------- | --------------------------------- | --------------------------------------------------------------------------------------------------- |
| TrailBase password           | Single factor                                                       | Actions that permit single factor | Cannot satisfy an MFA requirement.                                                                  |
| TrailBase password plus TOTP | Password plus verified TOTP for this event                          | MFA-required actions              | Only when the exact password-login flow proves TOTP completion.                                     |
| TrailBase social             | External social event; no TrailBase TOTP proof                      | Single-factor level only          | Never inherit TOTP assurance from enrollment or the `mfa` claim.                                    |
| Fasti passkey                | Phishing-resistant factor plus active TrailBase-anchor check        | Approved passkey actions          | Account lifecycle check must pass before a new session.                                             |
| Fasti recovery code          | Recovery event, not normal MFA                                      | Restricted                        | Force factor review, session rotation, code regeneration, and audit before other sensitive actions. |
| Generic OIDC / Authentik     | Exact `acr` and `amr` only when the profile validates and maps them | Policy-specific                   | Missing or unapproved assurance remains single factor. Groups and roles do not raise assurance.     |

Each sensitive capability declares its minimum assurance: password change, TOTP/passkey removal, recovery regeneration, identity unlink, other-session revocation, PAT/client creation or revocation, administrator/role change, disable, and delete.

### 6.3 Existing Access disposition

Do not create a parallel Access spine. Restore migration v8 to the exact `origin/dev` definition, then evolve retained owners through the next forward migration.

| Current primitive                   | Disposition                          | Final owner and rule                                                                                                                                                                                                 |
| ----------------------------------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `clients`                           | Evolve                               | Persistence for the `ApplicationClient` domain owner. Add client type, registration, lifecycle, and approved metadata through forward migrations. Do not add a second application-client table.                      |
| `credentials`                       | Evolve and narrow                    | Registered-client credential epochs and digest-only client secrets. Personal access tokens use their own subject-owned credential lifecycle. Provider and service secrets use `CredentialReference`, not this table. |
| `profile_grants`                    | Reuse and evolve                     | One application-authorization grant owner for client-to-profile access. Device approval and consent create or change it only through an authorized transaction.                                                      |
| `grant_scopes`                      | Reuse and evolve                     | One generated scope vocabulary and grant mapping. No adapter or `Connection` creates another scope store.                                                                                                            |
| PR-only `browser_users`             | Remove from v8; replace forward      | `AuthSubject`, TrailBase anchor, `Membership`, and `Role`. No password hash or `is_admin` flag remains in Fasti.                                                                                                     |
| PR-only `browser_sessions`          | Remove from v8; replace forward      | Final `FastiBrowserSession` schema. PR A creates the dormant schema but cannot issue a production session. PR C activates issuance after identity bootstrap and TrailBase exchange pass.                             |
| PR-only `browser_auth_bootstrap`    | Remove                               | It does not become the new identity bootstrap. PR C adds the distinct one-use `access.identity.bootstrap` operation.                                                                                                 |
| PR-only `user_passkeys`             | Remove from v8; add forward in PR D  | Final WebAuthn credential model owned by Fasti Access.                                                                                                                                                               |
| PR-only `user_totp`                 | Remove                               | TrailBase owns Time-based one-time password data. Fasti stores only proven status and audit projections.                                                                                                             |
| PR-only `user_backup_codes`         | Remove from v8; add forward in PR D  | Final Fasti `RecoveryCode` lifecycle for Fasti passkey recovery only.                                                                                                                                                |
| PR-only `oidc_provider_configs`     | Remove from v8; add forward in PR E1 | OpenID provider configuration belongs to Identity Integration; client secret is a `CredentialReference`.                                                                                                             |
| PR-only `auth_ephemeral_challenges` | Remove from v8; replace forward      | Purpose-specific `AuthCeremony` and WebAuthn ceremony state with bounded one-use transitions.                                                                                                                        |

`Connection` does not own external OpenID sign-in providers, Fasti application clients, device grants, profile grants, or native first-party clients. OpenID identity remains `ExternalAuthLink` plus provider configuration. Nuvio health may project Access-owned client and grant state, but it cannot own or duplicate it.

## 7. Component decisions

| Capability           | Owner                  | Component                                                                                                               | Decision                                                                                                                                                                                                                        |
| -------------------- | ---------------------- | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Human accounts       | TrailBase              | TrailBase `v0.33.5` candidate                                                                                           | Qualify runtime, licence, backup, and resources. OSL-3.0 server stays separate and unmodified.                                                                                                                                  |
| Passkeys             | Fasti Access           | `webauthn-rs =0.5.5`, checksum `6c548915e0e92ee946bbf2aecf01ea21bef53d974b0793cc6732ba81a03fc422`                       | `PROPOSED`. Bind to `AuthSubject`; store and consume server ceremony state; persist counters.                                                                                                                                   |
| Alternate passkeys   | N/A                    | `oauth2-passkey 0.6.1`                                                                                                  | `REJECTED`. It duplicates users, links, sessions, databases, and cache.                                                                                                                                                         |
| Generic OIDC RP      | Identity Integration   | `openidconnect =4.0.1`, checksum `0d8c6709ba2ea764bbed26bce1adf3c10517113ddea6f2d4196e4851757ef2b2`                     | `PROPOSED`. RP only. Reject unsupported JWE and unsafe multi-audience cases unless `azp` is verified.                                                                                                                           |
| Authentik management | Infrastructure adapter | Authentik and `authentik-client =2026.8.0`, checksum `ad417c23df7586c134dc3d4dd1a9c4a7910a15810c15f205f8f7327dbba8b70b` | Candidate. Pin OCI `sha256:7421753cfea67e89a6d295a1f0173ccea3866b33768c88dad90453b151cdcfd5`. Test pull, conformance, resources, socket leak, and soak.                                                                         |
| Fasti OAuth AS       | Fasti Access           | `oauth-as 0.9.3` evaluation candidate                                                                                   | `BLOCKED` for implementation selection. It covers the profile but is a new beta. Run independent security, source, storage, conformance, interop, fault, and maintenance review. If it fails, return for a dependency decision. |
| Credential storage   | Credential Vault       | Existing OS keyring plus encrypted headless vault and operator secret input                                             | One `CredentialReference`; no plaintext fallback.                                                                                                                                                                               |

Context7 is a Commander/package research gate at a dependency-version decision, not a build or runtime dependency. Query the exact version and feature once, compare the exact official source, and commit a dependency decision with version, checksum, feature flags, API names, source links, licence, and known limits. Ordinary build, test, package, and runtime paths never call Context7. Re-run the gate only when the selected version or feature set changes. Context7 absence is not permission to guess.

### 7.1 `C1-TB-TRUST` TrailBase trust gate

TrailBase v0.33.5 tokens lack issuer, audience, key ID, and token ID claims. Current evidence does not establish a documented public JSON Web Key Set or verification-key API. Before C1 code, approve a source-backed root-of-trust record:

- owner-authorized initial `TrailBaseInstanceId`, activation generation, exact public origin, exact pinned release/artifact, and initial verification-key fingerprint;
- the documented public API or proof that supplies the verification key and account state. Trust on first network use is prohibited;
- permitted signing algorithm, token type, subject, time, and proof validation despite the missing claims;
- owner-authorized key rotation, overlap, retirement, rollback, unknown-key, and unavailable-source behavior;
- the tuple `{TrailBaseInstanceId, activation_generation, proof_key_version, subject}` stored with each TrailBase anchor and authentication provenance and checked before a Fasti session or passkey session is created;
- restore activation and a concrete former-deployment fencing receipt that rejects an older generation.

Do not read TrailBase depot tables, private key files, or undocumented internal endpoints. If no supported public method can establish and rotate this trust or check account lifecycle, C1 and Fasti passkey sign-in remain `BLOCKED` and return under the primary-source-conflict rule.

PR B publishes a route-exposure matrix for native, OCI, and any approved remote topology. The browser-facing proxy exposes only exact proven authentication routes. TrailBase admin and Record APIs remain private and are never generically proxied. First-start administrator credentials are delivered only to the owning local operator through the proven TrailBase mechanism and are redacted from supervisor, container, status, and support logs.

### 7.2 `C3-CRYPTO` vault and backup gate

Before C3 code, approve one cryptographic profile. It pins mature authenticated-encryption and passphrase key-derivation dependencies, exact versions, checksums, licences, reviewed features, and source evidence. Do not implement encryption, key derivation, or nonce generation by hand.

The decision defines:

- master-key source for Desktop, native headless, and OCI; data-encryption-key and key-encryption-key hierarchy; separation by data root and backup;
- authenticated-encryption algorithm, nonce generation and uniqueness, associated data, envelope fields, format version, size bounds, and downgrade rejection;
- passphrase key-derivation parameters when a passphrase is allowed, memory/CPU resource proof, recovery, rotation, crash boundaries, and plaintext/temporary-file cleanup;
- an authenticated joint manifest that covers the Fasti database, TrailBase depot, vault, instance identity, activation generation, key versions, erasure ledger, and artifact digests. A digest-only manifest is insufficient;
- wrong key, modified manifest, nonce reuse, partial write, rollback, key loss, key rotation, process death, and resource-exhaustion tests.

Restore advances a global restore generation. It invalidates copied browser sessions, ceremonies, recent-auth assertions, recovery codes, PATs, OAuth code/access/refresh/device families, and client secrets by default. External provider, service, and Authentik management credentials return `stored_unverified` or quarantined and require explicit validation or rotation. The restore applies the signed erasure ledger before authentication can activate so an old backup cannot resurrect erased personal data.

### 7.3 Durable-operation reuse rule

Access invalidation delivery, `AuthentikOperation`, and Nuvio synchronization keep distinct domain states and tables. Do not create a generic operation aggregate, worker framework, trait hierarchy, or retry service before two concrete implementations exist.

Implement the first durable operation with the existing SQLite `BEGIN IMMEDIATE`, bounded batch, transaction, clock, and receipt patterns. At the second proven use, compare exact code and extract only identical claim, lease, retry, terminal-state, restart, and bounded-sweeper mechanics into an existing application/store owner. Domain transitions, payloads, compensation, idempotency, authorization, and audit stay separate. Evolve the existing Nuvio interoperability model into one durable outbox; never add a parallel Nuvio queue.

## 8. Credential vocabulary

| Credential                     | Owner            | Purpose                            | Storage rule                                                                                              |
| ------------------------------ | ---------------- | ---------------------------------- | --------------------------------------------------------------------------------------------------------- |
| TrailBase access token         | TrailBase        | Short-lived identity proof         | Server memory during exchange; discard.                                                                   |
| TrailBase refresh token        | TrailBase        | TrailBase refresh session          | Server memory during exchange; revoke before Fasti session.                                               |
| Fasti browser session          | Fasti Access     | Browser application access         | Opaque secret; digest-only store; secure cookie.                                                          |
| Fasti PAT                      | Fasti Access     | CLI and automation                 | One-time display; digest-only; scoped, expiring, revocable.                                               |
| OAuth authorization code       | Fasti Access     | Short-lived exchange               | Safe server state; one use.                                                                               |
| OAuth access token             | Fasti Access     | Delegated client access            | Bounded lifetime and audience.                                                                            |
| OAuth refresh token            | Fasti Access     | Renew delegated access             | Opaque, digest-backed, rotating family, reuse detection.                                                  |
| OAuth client secret            | Fasti Access     | Confidential client                | One-time display; digest-only; rotate and revoke.                                                         |
| Device and user codes          | Fasti Access     | Device approval                    | Bounded, rate-limited, one use.                                                                           |
| Passkey credential             | Fasti Access     | Phishing-resistant sign-in         | Verified credential linked to `AuthSubject`.                                                              |
| TOTP secret                    | TrailBase        | TOTP verification                  | TrailBase only; Fasti stores status and audit.                                                            |
| Fasti recovery code            | Fasti Access     | Recover Fasti-owned passkey access | Required MVP. CSPRNG, digest-only, atomic one-use. Cannot reset TrailBase password or bypass disablement. |
| Provider or service credential | Credential Vault | External service access            | Opaque reference; secret in approved backend.                                                             |
| CSRF token                     | Browser boundary | Protect mutations                  | Bound to session; not a bearer credential.                                                                |

All opaque Fasti credential paths reuse the existing `SecretMaterial`, `random_secret`, `digest_secret`, constant-time comparison, zeroization, and one-time-secret UI behavior. Extend those owners for new credential kinds. Do not add per-token random, digest, comparison, or display utilities.

## 9. SessionPolicy and TokenPolicy

Timing is a product and security contract. Do not hide it in constants or copy arbitrary values.

For each value, record exact default, minimum, maximum, configuration source, owner, effect on existing credentials, revocation behavior, UI copy, API field, threat rationale, source, and tests.

PR A's dormant `SessionPolicy` contains only timings that its store enforces:

- `browser_idle_timeout`
- `browser_absolute_lifetime`
- `remembered_browser_lifetime`
- `last_seen_write_interval`

It has no `Default`. It rejects zero, subsecond, and internally inconsistent
durations before storage. PR A reads the subject lifecycle, auth and
authorization epochs, selected profile-grant ownership, grant state, and
grant-owning client state on every session authentication. It therefore has no
disabled-subject revalidation interval.

C1 must set exact production defaults, minima, maxima, configuration sources,
and change effects before it mounts session issuance. C1 also owns the separate
recent-authentication window and the implemented behavior for policy changes.
PR A fixtures use deterministic test values only. They are not production
defaults or recommendations.

`TokenPolicy`:

- Fasti OAuth access and refresh TTL
- refresh rotation and reuse response
- authorization code TTL
- device code TTL, poll interval, and slow-down interval
- passkey challenge TTL
- recovery-code policy
- PAT default and maximum TTL

`ObservedTrailBasePolicy` records upstream access, refresh, password-reset, and email-verification lifetimes as pinned facts. Fasti does not present them as Fasti configuration.

`ProviderPolicy` owns provider cache lifetime, refresh skew, rate/backoff, and stale-data behavior. `ConnectionPolicy` owns health-check interval, stale threshold, retry/backoff, pause, and offline behavior. Do not place either in token policy.

Gate `A-POLICY` proves the dormant policy invariants, whole-second storage
contract, and absence of hidden defaults. Gate `C1-POLICY` sets the exact
browser and recent-authentication values before any production route can issue
a session. Gate `E-POLICY` sets OAuth values before PR E code. Use current
source, RFC requirements, threat model, journeys, and measured operations. Mark
missing evidence `BLOCKED`.

The UI shows expiry, remembered state, recent-auth expiry, the policy that ended access, and the next action. An administrator shortening policy sees the affected session and token count before confirmation.

### 9.1 `AccessInvalidationPolicy`

Fasti Access owns one event-to-credential invalidation matrix. Local invalidation, epoch/generation changes, authorization mutation, affected-count audit, and outbox creation occur in one transaction. The next local request rejects an invalid epoch even when cleanup is pending. External revocation is fail-closed for new access, retried from a bounded durable outbox, and must meet a package-approved maximum propagation time. A missing propagation bound blocks the package.

| Event                                                 | Browser session, recent auth, ceremony                                                                                                          | Passkey and recovery                                                                                                                 | PAT, client secret                                                                                 | OAuth code, access, refresh, device                                                                         | Grant and service credential                                                                          | TrailBase session                                                               | Audit and user outcome                                                                           |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Password reset or administrator account recovery      | Increment subject auth epoch; revoke every browser session, recent-auth assertion, and open ceremony                                            | Keep passkey public credentials only after account-state proof; invalidate and regenerate Fasti recovery codes                       | Revoke every subject-owned PAT; client secrets remain only when separately owned and uncompromised | Revoke every subject authorization code, access/refresh family, and pending device grant                    | Suspend subject grants until recovery review; quarantine subject-owned service credentials            | Revoke all through a supported TrailBase API or keep recovery blocked           | Record initiator, reason, affected counts, failed external cleanup, and required security review |
| Authenticated password change                         | Rotate current browser session; revoke other browser sessions, recent auth, and open ceremonies                                                 | Retain passkeys; recovery readiness is rechecked                                                                                     | Retain PAT/client secrets unless compromise is declared                                            | Retain delegated tokens unless policy or user selects global sign-out                                       | Retain grants and service credentials                                                                 | Revoke other TrailBase refresh sessions when supported                          | Show retained and revoked counts before commit                                                   |
| Factor add, remove, or replacement                    | Increment assurance epoch; rotate current session; revoke recent auth and sessions whose assurance no longer satisfies policy                   | Revoke the removed passkey; factor removal cannot remove the last usable path; regenerate recovery material when its premise changes | No implicit PAT/client-secret change                                                               | Revoke tokens whose approved assurance claim is no longer true                                              | Retain grants; authorization still rechecks assurance                                                 | Revoke affected TrailBase sessions when the supported factor API requires it    | Name factor, last-method result, and affected access                                             |
| Fasti recovery-code use                               | Create one restricted rotated session; revoke all other browser sessions, recent auth, and open ceremonies                                      | Atomically consume the code; invalidate remaining codes until regeneration; require passkey/factor review                            | Revoke subject PATs; client secrets require separate owner review                                  | Revoke subject code/access/refresh/device families                                                          | Suspend sensitive grants until recovery review; quarantine subject service credentials                | Revoke all TrailBase refresh sessions through a supported API                   | Persistent recovery incident with affected counts and exact next steps                           |
| External identity link, unlink, disable, or collision | Revoke sessions established from the affected link; invalidate recent auth and link ceremonies                                                  | Retain only when the durable TrailBase anchor remains valid                                                                          | Retain subject PATs unless subject compromise is declared                                          | Revoke token families whose consent/session provenance used the link                                        | Retain grants for the same subject; never transfer them to another subject                            | Do not delete the durable anchor or unrelated TrailBase sessions                | Record issuer, subject, link, collision outcome, and last-method protection                      |
| Membership, role, or profile-grant change             | Increment authorization epoch; affected browser requests fail on the next check; revoke sessions when membership is suspended or removed        | Credentials remain but cannot grant removed authorization                                                                            | PATs and clients lose removed scopes/profile access; revoke credentials on owner removal           | Revoke or narrow affected token families and pending device consent; no silent scope expansion              | Change the canonical grant once; service credentials cannot bypass it                                 | TrailBase human sessions remain identity-only                                   | Preview affected sessions/tokens/grants and final-administrator result                           |
| TrailBase account disable or delete                   | Increment subject auth epoch; revoke all browser sessions, recent auth, and ceremonies                                                          | Passkey sign-in fails lifecycle check; recovery cannot bypass disablement                                                            | Revoke subject PATs and subject-owned client secrets                                               | Revoke all subject code/access/refresh/device families                                                      | Suspend memberships and quarantine subject-owned service credentials; do not delete Chronicle data    | Revoke all supported sessions and persist cleanup failure                       | Show disabled/deleted/recovery-pending anchor state and data disposition                         |
| Policy shortening                                     | Revoke or shorten every affected browser/recent/ceremony record at the declared effective time                                                  | Apply only to future/passkey ceremony or recovery policy where relevant                                                              | Expire affected PAT/client-secret policy without extending another credential                      | Expire affected code/access/refresh/device state and rotate keys only under the approved key plan           | Re-evaluate affected grants and connection health                                                     | Apply only supported upstream policy and record its effect                      | Preview affected count, effective time, and irreversibility; never resurrect expired access      |
| Token reuse or declared secret compromise             | Revoke the detected family and related recent-auth/session provenance; escalate to all subject sessions when the policy says subject compromise | Revoke the named credential/recovery set when implicated                                                                             | Revoke the named PAT, client-secret epoch, or all owner credentials under the incident playbook    | Revoke the entire refresh family, derived access tokens, codes, and device grants; advance reuse generation | Quarantine implicated service credentials and pause connection writes                                 | Revoke TrailBase sessions when its credential or proof key is implicated        | High-severity incident, containment time, evidence, exposure review, and recovery action         |
| Restore activation or clone fencing                   | Advance global restore and subject/session epochs; invalidate all copied sessions, recent auth, and ceremonies                                  | Retain passkey public keys only with current TrailBase lifecycle proof; invalidate all copied recovery codes                         | Invalidate copied PATs and client secrets                                                          | Invalidate all copied code/access/refresh/device/token families                                             | Revalidate grants; quarantine every external service credential until explicit validation or rotation | New activation generation rejects old proof tuples; former deployment is fenced | Authenticated manifest, generation, invalidated counts, quarantine state, and activation receipt |

PATs, client credentials, device credentials, provider credentials, and service credentials never satisfy human recent authentication.

## 10. Tabler-first settings and QA

```text
Settings
├── Account and security
│   ├── Sign-in methods
│   ├── Password and recovery
│   ├── TOTP
│   ├── Passkeys
│   ├── Linked identities
│   ├── External identity providers
│   │   └── Authentik
│   ├── Browser sessions
│   └── Security policy
├── Devices and clients
│   ├── Paired devices
│   ├── Registered OAuth clients
│   ├── Personal access tokens
│   └── Pending device approvals
├── Connections
│   ├── Nuvio
│   ├── Plex and Tautulli
│   ├── Jellyfin and Emby
│   ├── Webhooks
│   ├── MQTT
│   ├── Local discovery
└── Metadata
    ├── Provider preferences
    ├── Provider credentials
    ├── Provider health
    └── Cache and attribution
```

### 10.1 Canonical interaction architecture

One task has one semantic owner. The account modal is a shortcut, not a second settings application.

| Surface                          | Semantic owner                                                                                                   | First three items                                                                                       | Navigation and return                                                                                                                                                                                 |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Account modal                    | Quick sign-in when signed out; current-session summary and sign-out when signed in                               | Authentication state; one primary sign-in or sign-out action; deep link to Account and security         | Native dialog preserves the opener. A deep link closes the dialog, opens the exact Settings destination, marks the current location, and provides `Back to <origin>` when an originating task exists. |
| Settings -> Account and security | Sign-in methods, password and recovery, TOTP, passkeys, linked identities, browser sessions, and security policy | Current protection summary; required action or `No action required`; sign-in-method and recovery status | Stable route and fragment per subsection. Side navigation and mobile select expose the same label and current state. Return goes to the originating Fasti task or Settings overview.                  |
| Settings -> Devices and clients  | Paired devices, registered OAuth clients, personal access tokens, and pending approvals                          | Pending approvals; current device/client summary; credentials that need action                          | Stable route per inventory. Pairing deep links name the requester and return to it only after an explicit outcome.                                                                                    |
| Settings -> Connections          | Service connections, including Nuvio                                                                             | Connections requiring action; healthy connections; add or repair action                                 | A connection detail route owns setup, health, disconnect, and recovery. It links to but does not duplicate credentials or devices.                                                                    |
| Settings -> Metadata             | Provider preference, credential, health, cache, and attribution                                                  | Providers requiring action; active providers; cache and attribution status                              | A provider row opens one detail flow. No-auth providers say `No credential required`, not `Not configured`.                                                                                           |

The modal does not enroll factors, display one-time secrets, manage credentials, approve devices, or perform policy changes. Settings does not duplicate the modal's quick sign-in form. The shell owns navigation, current-location indication, origin capture, and return behavior.

### 10.2 User-visible state contract

Every flow defines a `UserVisibleState` design record before code:

```text
state
visible_heading
literal_status
retained_non_secret_input
primary_action_and_enabled_state
safe_state
exact_recovery_action
focus_destination
live_region_announcement
```

Secrets, recovery codes, authorization codes, device codes, tokens, and client secrets are retained only for their approved one-time display or active ceremony. They are cleared after submission, cancellation, expiry, navigation, or failure unless the protocol requires the same in-memory ceremony to continue. Persistent status replaces transient toast-only feedback.

The following matrix is mandatory. A package may mark a state `N/A` only with a source-backed reason and reviewer approval. `Initial` covers loading and empty. `Blocked` covers unavailable and offline. `Working` covers pending. `Outcome` covers success and partial success. `Invalidated` covers expired, revoked, and conflict. `Failure` covers other errors.

| Flow                                  | Initial                                                                                 | Blocked                                                                        | Working                                                                          | Outcome                                                                                           | Invalidated                                                                                            | Failure, focus, and announcement                                                                           |
| ------------------------------------- | --------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Sign-in                               | Skeleton with stable heading; empty means no established method                         | State service or method unavailable and keep the intended destination          | Disable repeat submit; announce authentication in progress                       | Show signed-in subject and return action; partial membership becomes an explicit membership state | Explain expired proof, revoked session, or identity conflict; restart only the affected step           | Keep identifier, clear password/proof, focus error summary, announce safe signed-out state and next action |
| Email verification and password reset | Show account and step without claiming email delivery                                   | State TrailBase capability or mail delivery unavailable                        | Show sent/pending state, safe retry time, and change-address path when supported | Confirm verification or password change and offer sign-in                                         | Expired or used link starts a new source-owned request                                                 | Do not reveal account existence beyond approved policy; focus heading and announce what remains unchanged  |
| TOTP enrollment                       | Show current enrollment truth and prerequisites                                         | Explain TrailBase limitation or insufficient recent authentication             | Keep the active ceremony in memory; allow cancel                                 | Confirm only after a verified code; show recovery implications                                    | Expired ceremony restarts; removed factor leaves other methods unchanged                               | Clear shared secret, focus error summary, and announce that enrollment did not complete                    |
| Passkey enrollment and use            | Show credential inventory or `No passkeys`                                              | Explain browser/platform/policy unavailability                                 | Protect the active WebAuthn ceremony from duplicate actions                      | Name the verified credential and show inventory result                                            | Expired/replayed challenge restarts; revoked credential remains listed with status when audit requires | Focus the passkey action or error summary; announce whether no credential changed                          |
| Recovery codes                        | Show readiness and remaining-count policy, not code values                              | Explain recent-auth or eligibility requirement                                 | Generate once; no duplicate request                                              | Display once with copy/download/print choices and explicit acknowledgement                        | Used, expired, or regenerated codes are invalid and never redisplayed                                  | Focus the code heading during display, then the persistent readiness result; announce one-time visibility  |
| Session inventory                     | Stable rows; empty is impossible for an active browser session and otherwise means none | Offline shows cached truth as stale and disables revoke                        | Preserve row and current-session marker during revoke                            | Persist `Revoked` or removal per audit policy; partial bulk revoke lists failures                 | Expired/revoked/conflicting refresh updates the exact row without reordering focus                     | Focus affected row/result; announce target and outcome count                                               |
| Identity linking                      | Show linked methods and the method required to continue                                 | Disable unlink of the last usable method; explain provider outage              | Bind state, nonce, issuer, and intended subject to the active ceremony           | Show linked identity and retained TrailBase anchor                                                | Collision, revoked provider proof, or stale link enters explicit recovery                              | Focus summary; announce that no account merged unless the transaction committed                            |
| Personal access tokens                | Show token metadata or `No personal access tokens`                                      | Explain policy or vault restriction                                            | Disable duplicate creation/rotation                                              | Show secret once, then only metadata; preserve scopes and expiry                                  | Expired/revoked token cannot be restored; rotation names replaced token                                | Clear secret on exit; focus one-time secret then persistent row; announce copy risk and outcome            |
| OAuth clients                         | Show registered clients or `No registered clients`                                      | Explain client-type or administrator restriction                               | Preserve typed configuration during validation                                   | Show exact redirect URIs, grants, scopes, owner, and secret-once result                           | Conflict names duplicate client/redirect; revoked client cannot issue tokens                           | Focus invalid field or summary; announce no registration on failure                                        |
| Device approval                       | Show requester, scopes, profile, expiry, and approve/deny                               | Unknown, expired, offline, or policy-blocked code cannot be approved           | One decision in flight; requesting device continues governed polling             | Persist approved or denied outcome and linked device                                              | Expired, reused, revoked, or consent-conflicting code requires a new code                              | Focus decision heading/result; announce requester and exact outcome                                        |
| Provider credentials                  | Show `No credential required`, missing, or current governed state                       | Capability, safe-transport, or vault absence explains why unavailable          | Clear secret after the credential test request; keep non-secret fields           | Separate stored status from bounded connection-test result                                        | Expired/revoked/rotated status preserves last safe cached metadata                                     | Focus first invalid field or status; announce that the old credential remains or was replaced              |
| Connections                           | Show healthy, degraded, disconnected, or `No connections`                               | Offline preserves last-known timestamp and disables unsafe actions             | Persist setup or repair step and redacted diagnostics                            | Separate configured, authenticated, healthy, and partially capable states                         | Expired/revoked/conflicting grants require explicit reconnect or reconcile                             | Focus status heading; announce retained data and exact repair action                                       |
| Authentik dry-run/apply/rollback      | Show mode, version, ownership boundary, and current inspected state                     | Unsupported version/permission/offline blocks apply but allows redacted export | Persist dry-run and per-object progress; no unrelated object can enter the plan  | List created/updated/unchanged objects and sign-in test separately                                | Drift/conflict requires a new inspection; revoked management token blocks repair                       | Focus failed object or summary; announce applied count, rollback state, and unrelated-object safety        |
| Nuvio pairing and sync                | Show pair state, profile, scopes, health, last sync, and cursor truth                   | Offline or upstream absence preserves queued state and disables false success  | Show poll/retry timing without motion dependency; preserve durable outbox        | Separate paired, authorized, delivered, synchronized, and partial outcomes                        | Expired code, revoked grant, cursor conflict, or reconciliation need has one exact recovery path       | Focus state heading; announce queued/safe data and next repair action                                      |

### 10.3 Tabler reuse and Chesterton ledger

The package plan begins with `What already exists` and `Not in scope`. It searches the current exact head before adding a component.

What already exists and must be evaluated for reuse:

- `brand/DESIGN.md` is the visual authority;
- `brand/tokens/tokens.json` owns semantic design tokens;
- the Workbench shell uses `.page`, the vertical Tabler navbar, `.page-wrapper`, and `.container-fluid`;
- the shell preserves wide list-group navigation, the constrained labelled mobile select, theme attributes, `--fasti-focus`, and token-owned radius scaling;
- responsive Settings list-group and labelled-select navigation in `packages/ui/src/runtime-settings-view.svelte`;
- native account dialog behavior in `packages/ui/src/auth-modal.svelte`;
- reusable focus restoration in `packages/ui/src/dialog-focus.ts`;
- one-time-secret focus and announcement behavior in `packages/ui/src/api-clients-panel.svelte`;
- connection empty, error, and persistent-status behavior in `packages/ui/src/connections-view.svelte`;
- safe host-problem projection in `packages/ui/src/host-problem.ts` where its contract applies.

These are package preconditions. A design that violates the shell, tokens, typography, theme, focus, radius, or breakpoint rules fails. `pnpm lint:ui` must pass without weakening its allowlist or rules.

Not in scope: a second settings shell, a custom component library, protocol vocabulary in default user journeys, speculative dashboards, decorative motion, gamification, or a card for every method, provider, session, client, or connection.

Each changed control has this Chesterton record before deletion or replacement:

| Field                       | Required evidence                                                                   |
| --------------------------- | ----------------------------------------------------------------------------------- |
| Control and current purpose | Exact file, current user outcome, and visible state                                 |
| Callers and data source     | Every host, route, store, adapter, and contract that drives it                      |
| History                     | Commit, plan, issue, screenshot, or explicit `No history found`                     |
| Tests                       | Current exact-head test or explicit missing-evidence finding                        |
| Disposition                 | Reuse, mature, move, replace, or remove, with reason                                |
| Replacement                 | Tabler component or pattern and preserved behavior                                  |
| Regression evidence         | Screenshot/state comparison, keyboard/focus result, contract test, and user outcome |

| Surface                      | Tabler-first mapping                                                                                      | Explicit preservation or replacement                                                              |
| ---------------------------- | --------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Account modal                | Native dialog plus Tabler form, alert, button, and list-group patterns                                    | Preserve opener return, Escape, focus trap, and truthful state; keep it a quick surface           |
| Account and security         | Settings navigation, alerts, list groups, responsive tables, badges, forms, and modal confirmations       | Replace unavailable-method card walls with grouped lists while retaining reasons and next actions |
| Session inventory            | Responsive table at wide widths; labelled list groups at narrow widths; badges and dropdown actions       | Preserve current marker, exact target, stable row order, and persistent revoke outcome            |
| Factors and recovery         | Step form, progress, alerts, one-time-secret/codes panel, and inventory list                              | Reuse one-time-secret focus/announcement behavior; do not create a custom wizard framework        |
| Devices, clients, and tokens | Responsive tables/list groups, scope badges, dropdown actions, modal confirmation                         | Preserve one-time secret behavior and explicit public/confidential client distinction             |
| Provider credentials         | Responsive table/list group, status badge, typed modal form, alert, and detail drawer/page only if needed | Replace repeated provider cards; retain real unavailable states and attribution                   |
| Connections                  | Responsive table/list group, persistent status alert, empty state, and detail flow                        | Replace connection card wall while preserving existing empty/error/status behavior                |
| Authentik setup              | Step form, diff table, alerts, progress, and confirmation modal                                           | No generic provisioning canvas; show only inspected Fasti-owned objects                           |
| Nuvio pairing                | Code/pairing panel, scopes list, progress, alert, and device row                                          | No animated pairing spectacle; preserve status, expiry, denial, and recovery                      |

### 10.4 Responsive, focus, and destructive-action contracts

| Viewport    | Required behavior                                                                                                                                                                                                            |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 320 CSS px  | One column; no two-dimensional page scroll; tables become labelled lists; long identifiers and errors wrap; primary action remains visible without covering content; software keyboard does not hide focused input or result |
| 375 CSS px  | Same task and order as 320; secondary actions use an accessible menu only when labels remain discoverable                                                                                                                    |
| 768 CSS px  | Settings navigation may use list group or labelled select according to measured fit; dense inventories may remain tables only when every cell and action fits at 200-percent zoom                                            |
| 1440 CSS px | Bound reading width; do not stretch forms or separate related controls; preserve the same information order and keyboard order                                                                                               |

Light, dark, forced-colors, reduced-motion, text-spacing, 200-percent zoom, and 320-pixel reflow are independent gates. State changes do not reorder a focused item, cause avoidable layout shift, or move the primary action. Long errors use headings, short cause text, safe state, and one exact next action.

Focus contract:

- Entry: move focus only for a user-opened dialog, route destination, or blocking error summary. Otherwise preserve the initiating control.
- Trap and Escape: trap only modal content; Escape cancels without committing and returns to the opener unless a platform ceremony controls focus.
- Validation failure: focus the error summary, link each error to its field, and retain permitted non-secret input.
- Async completion: keep focus stable; announce the result; move focus only to a newly exposed one-time secret or when the initiating control no longer exists.
- Cancellation or close: restore the exact opener. If it was removed, use the nearest semantic heading or owning row.
- Destructive confirmation: initial focus is on the heading or safe cancel action, never the destructive action. Name the target and affected count.
- Route return: restore the originating task and control when still valid; otherwise focus the returned-page heading and explain why.

| Action                                                | Confirmation must show                                                                                                                                                                                                                                                   | Reversibility and persistent outcome                                                                                                                                                                                                                                                                                                                     | Recovery                                                                                                                                               |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Revoke session, token, client, or device              | Exact target, current marker, scopes/profile, and affected token/session count                                                                                                                                                                                           | State whether immediate, eventual, or irreversible; persist `Revoked` and timestamp where required                                                                                                                                                                                                                                                       | Reauthenticate or register/pair/create again; never offer restore when impossible                                                                      |
| Unlink sign-in identity                               | Provider, subject label, remaining methods, and last-method protection                                                                                                                                                                                                   | Transactional and irreversible as a link; audit the result                                                                                                                                                                                                                                                                                               | Re-link only after fresh proof; blocked if it would remove the last usable method                                                                      |
| Disconnect service                                    | Service, profile/workspace, queued work, retained data, and credential disposition                                                                                                                                                                                       | Separate disconnect, credential deletion, and data deletion                                                                                                                                                                                                                                                                                              | Reconnect without silently deleting Chronicle or cached provenance                                                                                     |
| Delete credential or recovery material                | Name, owner, affected provider/client, and what will stop working                                                                                                                                                                                                        | One-time secrets cannot be recovered; replacement is a separate explicit action                                                                                                                                                                                                                                                                          | Rotate or regenerate after recent authentication                                                                                                       |
| Disable account, method, or client                    | Affected memberships, sessions, grants, and administrator-continuity result                                                                                                                                                                                              | State whether state is suspended or removed and how audit is retained                                                                                                                                                                                                                                                                                    | Approved administrator or source-owned recovery path                                                                                                   |
| Shorten session or token policy                       | New value, old value, affected current count, expiry timing, and rollout behavior                                                                                                                                                                                        | Persist policy and enumerate invalidated access                                                                                                                                                                                                                                                                                                          | Users reauthenticate; operator can set a newly approved policy, not restore expired tokens                                                             |
| Delete TrailBase account                              | Owner: TrailBase. Show the human account, Fasti links, sessions, memberships, factors, and final-administrator result                                                                                                                                                    | Require recent authentication and explicit TrailBase support. Revoke linked Fasti sessions. Do not imply that Chronicle or Fasti privacy data was deleted. Persist an audit receipt without secret or personal proof material. TrailBase rollback is limited to its proven backup and restore contract.                                                  | Recover only through a proven TrailBase/operator procedure. A new account does not silently inherit the old Fasti subject.                             |
| Erase Fasti privacy data                              | Owner: Fasti. Show the subject, workspaces, profiles, sessions, links, grants, audit-retention rule, and exact Chronicle-data disposition                                                                                                                                | Require recent authentication and final-administrator protection. Delete or anonymize only the approved Fasti-owned data classes. State retained legal, integrity, or shared-domain records. Do not delete the TrailBase account or external identity. Persist a non-sensitive erasure receipt. Rollback follows the approved backup and privacy policy. | Explain any restore limit before confirmation. Re-linking or registering later does not restore erased state unless the approved restore path says so. |
| Delete Fasti-owned Authentik objects after disconnect | Owner: Fasti's Authentik management adapter for recorded Fasti-owned object IDs only. Show application, provider, mappings, affected users and active sessions, unrelated objects, final sign-in-method result, safe state, rollback, and whether OIDC sign-in will stop | Require recent authentication, a fresh dry-run, and last-method protection. Remove only recorded Fasti-owned objects. Do not delete the Authentik user, TrailBase account, Fasti subject, or Chronicle data. Persist per-object results and rollback limits.                                                                                             | Recreate through a new approved managed-mode plan or use manual mode. Never repair by deleting unrelated Authentik objects.                            |

### 10.5 Design-framework evidence

Each changed flow owns a requirements-to-evidence worksheet. The worksheet names the exact design decision and artifact for every row below. A generic statement that a framework was considered does not pass.

| Lens        | Required decision                                                                                                 | Minimum evidence                                                                                                        |
| ----------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| AskTog      | Anticipation, target size/Fitts, latency feedback, work protection, continuity, interruption, and resumption      | Annotated flow plus interruption and delayed-response test                                                              |
| Gestalt     | Proximity, similarity, common region, continuity, closure, and figure/ground                                      | Annotated mockup at narrow and wide viewport                                                                            |
| Nielsen 1   | Show current, pending, partial, complete, and failed status                                                       | State screenshot and live-region assertion                                                                              |
| Nielsen 2   | Use account, session, device, connection, and recovery terms as users experience them                             | Copy review with protocol vocabulary removed from default flow                                                          |
| Nielsen 3   | Provide cancel, deny, back, close, revoke, and safe recovery where applicable                                     | Keyboard journey and interruption/resume evidence                                                                       |
| Nielsen 4   | Use the same labels, state badges, action order, and navigation across hosts                                      | Cross-surface comparison                                                                                                |
| Nielsen 5   | Disable impossible/destructive actions, validate before commit, and preview effects                               | Negative-control and confirmation evidence                                                                              |
| Nielsen 6   | Keep context, names, scopes, expiry, and next action visible                                                      | Five-second comprehension review                                                                                        |
| Nielsen 7   | Provide direct deep links and efficient keyboard paths without cluttering the default task                        | Expert-path keyboard timing and first-use comparison                                                                    |
| Nielsen 8   | One primary action, grouped detail, stable layout, and no card wall                                               | Annotated information hierarchy and layout-shift evidence                                                               |
| Nielsen 9   | Every error states what happened, what stayed safe, and what to do next                                           | Error catalogue screenshot plus recovery test                                                                           |
| Nielsen 10  | Put contextual help beside the decision and link operator detail separately                                       | Help-link and accessible-name inspection                                                                                |
| IxDF        | Cognitive load, progressive disclosure, motor precision, forms, interruption, security UX, and dark-mode halation | Rationale tied to current primary or established research and exact mockup decision                                     |
| WCAG 2.2 AA | Applicable success criteria, including accessible authentication                                                  | Axe plus manual keyboard, zoom, reflow, focus, contrast, target, announcement, clipboard, and password-manager evidence |
| EN 301 549  | Applicable Clauses 9, 10, 11, and 12                                                                              | Clause-to-evidence record; automation is never the sole conformance claim                                               |
| ADHD/AuDHD  | Stable layout, visible context, saved progress, short sections, closed loops, and safe deferral                   | Interrupted-task return test and cognitive walkthrough                                                                  |

Existing design-system claims are targets, not proof. Evidence binds to the exact implementation head.

### 10.6 Design artifact and copy gates

Before UI code, place generated mockups, comparison boards, and `approved.json` under `~/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-<date>/`. Do not commit generated review artifacts. Record each absolute artifact path, revision, approval, and binding constraint in this canonical plan. Repository documents keep approved textual decisions and implementation specifications only. Include Account and security, session inventory, factor and recovery setup, devices and clients, provider credentials, Connections, Authentik dry-run/apply/rollback, and Nuvio pairing/sync.

For each surface include 320, 375, 768, and 1440 CSS-pixel views; light, dark, and forced-colors; loading, empty, unavailable, error, partial, success, and destructive confirmation where applicable. Use a comparison board. If the design tool is unavailable, use an explicit text wireframe with the same states and dimensions. Record named user approval and exact artifact revision before implementation.

Copy gate:

- Define an acronym at first use in operator documentation. Keep it out of default user flows unless the user must decide it.
- Use one instruction or action per sentence. Use active voice and literal status labels.
- Separate user copy from operator documentation and review both.
- Every error says what happened, what remained safe, and what to do next.
- Replace `PKCE`, `RP`, `AS`, `DCR`, `JWKS`, and `assurance` in user copy with the concrete task or state unless advanced detail is open.
- Run a dedicated ASD-STE100 and accessibility copy review. Record exceptions with the technical term and reason.

Tabler ladder:

1. Upstream Tabler component.
2. Tabler pattern composition.
3. Fasti token-skinned Tabler element.
4. Custom Svelte only when Tabler has no equivalent and the plan records why.

Prefer compact tables, list groups, badges, alerts, dropdown actions, modals, empty states, progress, and forms. Do not use large repeated cards when a list is clearer. Do not weaken `pnpm lint:ui`.

Every changed flow completes the single requirements-to-evidence worksheet in section 10.5. Do not maintain a second framework checklist.

After plan approval and before UI code, run `/design-review` and `/impeccable polish` on the Tabler-first artifacts, resolve findings, and obtain the named user approval. After code, run Axe, Playwright, keyboard, reflow, motion, theme, forced-colors, zoom, screen-reader journeys, and `/qa`.

### 10.7 Gate 10 approved direction

The named user approved `Gate 10 A+C` on 2026-08-29. This approval binds A and C to separate purposes over one shared implementation model:

- **A is the required steady-state MVP surface.** `Settings -> Account and security` is the permanent task map for sign-in methods, recovery, sessions, devices, clients, external identities, and exact next actions.
- **C is a separate required first-run journey.** It guides initial account protection and external-identity setup, saves every confirmed step, supports `Save and leave`, resumes at the next safe task, and hands the user into A for normal management.
- A and C use one capability state, one route owner, one authorization model, and one set of application services. C is not a second settings implementation and does not duplicate persistence.
- That route owner is the existing `RuntimeSettingsView` inside the Tabler Workbench shell. Reuse its desktop Settings navigation, constrained-screen `Settings section` select, account modal, focus handling, one-time-secret handling, and governed problem projection. Mature the existing surface in place; do not create a parallel shell.
- C persists no separate wizard record. It derives its next safe task from confirmed account, session, grant, provider, and capability state. `Save and leave` means exit to A after any in-flight operation reaches a known safe boundary; it does not copy domain state into a second progress store.
- C is capability- and authorization-derived. Personal account protection appears to the signed-in person. Authentik managed setup appears only to an authorized first administrator or node operator, and every mutation rechecks authorization and recent authentication on the server.
- **B is not a third destination.** Its compact evidence tables and inspectors are reusable detail patterns opened from A for session inventories, device or client grants, and Authentik dry-run/apply/rollback evidence.
- An incomplete first run remains persistently visible in A. A can resume C at the exact unfinished step. Completing or leaving C never hides current sessions, revocation, recovery, or provider state.
- Recovery copy must not promise unconditional rollback. Compensate only for Fasti-owned changes whose remote state still matches the recorded post-apply state. If the remote object changed again, stop without overwriting it and show the exact review or repair action.

Current external artifact set:

- `/home/ryan/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-20260829/design-board.html`
- `/home/ryan/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-20260829/fasti-access.css`
- `/home/ryan/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-20260829/gate-10-design-review.md`
- `/home/ryan/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-20260829/approved.json`
- `/home/ryan/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-20260829/variant-a.png`
- `/home/ryan/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-20260829/variant-b.png`
- `/home/ryan/.gstack/projects/Scrobble-dev-Fasti/designs/fasti-access-20260829/variant-c.png`
- responsive boards at `board-320.png`, `board-375.png`, `board-768.png`, and `board-1440.png` in the same directory.
- A, C, and shared-state screenshots for light, dark, Night, and forced-colors at 320, 375, 768, and 1440 CSS pixels use the filename pattern `<surface>-<mode>-<width>.png` in the same directory.

Frozen review revision:

- `design-board.html` SHA-256: `aac81318730ce3f903263cab39e1b79937bd60c3b05ec05bc51326e8e5b34797`.
- `fasti-access.css` SHA-256: `401d4f7cb931e4114503723738f0e7a6907de05460f1fb4f2583e0ec9fc3864b`.
- Pinned Tabler CSS SHA-256: `7ef750bd10546a695d0b12767ad8048bd8f3ec5de7daefb1067f9d0daa3d1c9a`.
- Axe Core 4.10.3 returned zero WCAG-tagged violations across all 16 viewport/theme combinations. The same matrix returned zero page overflow, zero duplicate IDs, zero visible interactive targets below 44 CSS pixels, zero console or page errors, and a visible 3 CSS-pixel focus outline. Separate 200% text-size and WCAG text-spacing probes returned no A or C container overflow.

The independent review found no P0 blocker. Its design-level text-reflow, forced-colors, disclosure-control, table-row-header, conditional-scroll-focus, and active-theme findings are resolved in this frozen revision. Real Tabler offcanvas behavior, Settings select navigation, Escape/focus containment/restoration, C-to-A live announcements and heading focus, production skip link/main landmark behavior, and screen-reader journeys remain implementation evidence gates. The `/design-review`, manual `/impeccable polish`, and framework worksheet synthesis is recorded in `gate-10-design-review.md`. The exact user approval and frozen artifact hashes are recorded in `approved.json`. Automated checks do not establish WCAG 2.2 AA or EN 301 549 conformance by themselves.

## 11. Provider registry

There is no canonical registry today. The web host hard-codes 12 entries. Desktop implements only Google Books and TMDB. PR F creates one source of truth.

Requirement kinds:

```text
none
api_key
bearer_token
basic_auth
oauth2
optional_token
user_agent_only
custom_header
```

Credential states:

```text
not_required
missing
stored_unverified
valid
invalid
expired
unavailable
revoked
```

Each entry owns ID, name, media domains, typed input schema, requirement, ownership, scopes, endpoint, official docs, attribution, rate policy, connection test, credential test, expiry, refresh, and offline behavior.

| Provider       | Requirement                 | MVP state                                                                                                                 |
| -------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Open Library   | `user_agent_only`           | `not_required`; meaningful application/version/contact; obey current rate policy.                                         |
| Kitsu          | `none` for public metadata  | `not_required`; account functions are a separate Connection.                                                              |
| AniList        | `none` for public metadata  | `not_required`; Fasti is not a competing tracker. Keep public metadata use within current documented terms.               |
| MusicBrainz    | `user_agent_only`           | `not_required`; meaningful application/version/contact; obey current rate policy.                                         |
| TMDB           | `bearer_token`              | Sensitive `Authorization` header; preserve attribution and non-endorsement notice.                                        |
| TVDB v4        | `api_key` with optional PIN | Typed multi-part input; login body then bearer token; handle documented one-month session.                                |
| Google Books   | `api_key`                   | Sensitive `X-Goog-Api-Key`; never use URL `key=`.                                                                         |
| MyAnimeList v2 | `custom_header`             | `X-MAL-CLIENT-ID` application configuration. User writes use a separate OAuth Connection.                                 |
| RAWG           | `api_key`                   | `unavailable`; official transport puts the key in the URL. Wait for safe documented transport or approved proxy contract. |
| IGDB           | `oauth2`                    | Server-side Twitch client ID/secret; form-body exchange; bounded headers and expiry.                                      |
| ComicVine      | `api_key`                   | `unavailable`; documented URL-key transport conflicts with Fasti policy.                                                  |
| Podcast Index  | `custom_header`             | Typed key/secret; timestamp and signature headers; secret is not sent.                                                    |

Composite inputs still persist as one opaque `CredentialReference`. Provider secrets never enter retained Svelte state, browser storage, URLs, logs, screenshots, fixtures, normal exports, or plaintext SQLite. Clear input after every attempt. A host without safe storage shows the missing capability and exact setup action.

## 12. Authentik compatibility profile

Authentik is a named, tested, documented integration. It does not create a second identity architecture.

```text
Fasti OIDC adapter -- openidconnect
  discovery, code, PKCE, state, nonce, token validation,
  refresh, issuer + subject, RP-initiated logout URL construction

AuthentikManagementAdapter
  authentik-client: documented /api/v3 reads and CRUD
  Fasti orchestration: version and ownership gates, dry-run,
  approved provisioning, drift, repair, and rollback
```

Do not use `authentik-client` for sign-in or `openidconnect` for management. Use documented `/api/v3` only. Generated types stop at the adapter.

Manual mode: the operator configures Authentik and supplies issuer, client ID, secret, redirects, logout, scopes, and claims. Fasti validates OIDC and writes nothing. Use it for an existing Authentik installation, a restricted environment, an operator who refuses management API access, or a server version outside the managed compatibility matrix.

Managed mode: Fasti validates the server tuple, inspects state, produces a dry-run, names permissions and objects, requires approval, creates or updates only Fasti-owned objects, validates sign-in, records object IDs, detects drift, repairs and unlinks explicitly, and rolls back without touching unrelated objects.

Managed writes use a durable `AuthentikOperation` journal. For each intended object mutation, record the Authentik object ID, Fasti ownership marker, prior representation hash or source version, redacted intended representation, applied representation hash or version, operation state, compensation state, attempts, and correlation ID.

- Re-read the remote object before every mutation. Stop on any drift from the approved dry-run or the last recorded applied state.
- Never hold a Fasti SQLite transaction across an Authentik network request.
- After each remote response, persist whether the commit is confirmed, unknown, failed, or compensatable. A timeout after a possible remote commit triggers inspection, not blind retry.
- On process restart, resume inspection and either complete or compensate the journaled operation.
- Compensate only a Fasti-owned object whose current remote state still matches the exact state Fasti applied. Stop and request operator action on unrelated or concurrently edited state.
- Test concurrent edits, timeout after remote commit, reordered responses, process death at every journal state, partial apply, partial compensation, revoked management credentials, and unrelated-object protection.

Use a stable Fasti ownership marker and slug. Never identify by display name alone. The exact 2026.8.0 `ApplicationRequest` and `OAuth2ProviderRequest` models expose no generic ownership-marker field. `ScopeMappingRequest.managed` means managed by Authentik migrations; never repurpose it as a Fasti marker. A local journal, exact remote ID, and representation hash prevent blind writes but do not by themselves prove a remote ownership marker. Managed create, update, repair, rollback, and delete for an object type remain `Unavailable` until E3 selects and proves a documented remote ownership signal for that type. Never delete automatically. Disconnect does not delete objects unless the operator separately selects deletion and sees impact and rollback.

### 12.1 Version, source, and dependency contract

The tested tuple is `{Fasti version and commit, Authentik server version and exact OCI index or manifest plus architecture digest, authentik-client version, openidconnect version}`. Record it in the capability ledger, operator guide, compatibility matrix, QA receipt, and release evidence. Pin `authentik-client` and `openidconnect` through the locked workspace dependency graph and `Cargo.lock`; use no unversioned Git dependency. The E3 package plan selects the exact Authentik digest for each supported architecture before a container starts. Do not use a floating tag.

Current primary evidence identifies Authentik release commit `f3753ec20ce13ef672401a131379d1a5a2d3439b`, `authentik-client` tag commit `13c2d4f82983ff66323dc5266af4d98c28b52dd4`, crate checksum `ad417c23df7586c134dc3d4dd1a9c4a7910a15810c15f205f8f7327dbba8b70b`, and OCI index `sha256:7421753cfea67e89a6d295a1f0173ccea3866b33768c88dad90453b151cdcfd5`. These values prove candidate identity, not runtime compatibility.

The 2026.8.0 server/client tuple and `openidconnect` 4.0.1 are candidates until current socket, resource, multi-architecture, exact-tag source, Context7, and conformance gates pass. Exact server/client matching is Fasti's fail-closed policy, not an upstream cross-version compatibility promise. Context7 currently resolves `/goauthentik/client-rust` and `/ramosbugs/openidconnect-rs`; its examples are discovery evidence, not exact-version API proof. Before code, inspect the exact 2026.8.0 generated client tag and exact 4.0.1 source. Do not copy a method or field from `main`, memory, or an unversioned documentation page.

`openidconnect` 4.0.1 discovers `end_session_endpoint` and constructs an RP-initiated `LogoutRequest`. It does not implement front-channel or back-channel receivers or logout-token verification. E0 must select source-backed, pinned protocol support for those Fasti receiver paths and their JSON Web Token validation; do not hand-roll JSON Web Signature or JSON Web Token parsing.

The canonical client source is `goauthentik/client-rust`; the earlier `authentik-community` URL redirects there. The generated client's API and package versions must match the selected Authentik server version. Inject Fasti's hardened proxy-free, redirect-free client rather than accepting a default transport whose address, redirect, proxy, timeout, or body behavior is unproved.

On a tuple mismatch, generic standards-based OIDC may remain available only after conformance. Managed inspection and writes fail closed. Show `Authentik management version is not supported`, the detected server version, the supported version, and one exact remediation action. Never send a management write to an untested API version.

### 12.2 Management credential contract

Support these current documented bearer methods for Fasti management:

- an API token sent with HTTP Bearer authentication; or
- an OAuth access token carrying `goauthentik.io/api`, subject to the same Authentik authorization and minimum-permission checks.

An API token has no OAuth scope boundary. Its effective access comes from the owning Authentik user's roles and global or object permissions. An OAuth access token still requires the same Authentik authorization after the `goauthentik.io/api` scope. E3 lists a permission only after a restricted-account test proves the exact operation.

Prefer a restricted, short-lived credential. State every permission needed in the dry-run. Retain the credential only through PR C's governed `CredentialReference`; never put it in browser storage, a URL, a log, a screenshot, a normal export, a plain configuration file, a command argument, or a plain SQLite field.

Managed mode accepts either a one-operation credential or a retained credential. A one-operation credential enters through the governed operator secret input, remains only in zeroizable process memory, never enters `AuthentikOperation` or persistent storage, and is cleared after success, failure, cancellation, or timeout. After a restart, the operator must supply it again before any secret-dependent inspection, repair, or compensation continues. A retained credential uses `CredentialReference` and the governed vault. The operator chooses retention explicitly; successful provisioning never silently converts a one-operation credential into a stored credential.

When retained, support rotation, explicit revocation, last use, and last successful validation. Never display the full credential again. When removed, OIDC sign-in and public discovery validation stay available, automatic repair becomes unavailable, the UI says `Manual management`, and Fasti never reports drift as repaired.

### 12.3 Managed-resource inspection contract

Before freezing the E4 schema or writing any generated-client call, inspect and classify every current Authentik resource that Fasti might read, create, update, link, or delete:

- one application and one OAuth2/OpenID provider;
- authorization, authentication, and invalidation flows;
- redirect URIs, launch or provider setup URL, logout URIs, and logout method;
- client type, client ID, one-time client secret, and approved grant types;
- access-token and refresh-token validity, signing key, and key-rotation behavior;
- scope and property mappings for group, role, profile, email, username, and subject behavior;
- front-channel, back-channel, and RP-initiated logout;
- dynamic client registration configuration and policy.

For each resource, record exact 2026.8.0 generated model and method names, endpoint provenance, immutable and mutable fields, ownership marker, stable object identifier, required permission, secret fields, defaults, validation, read-after-write check, compensation, and delete effect. A missing public API or generated model leaves that managed action `Unavailable`; do not use Authentik tables or undocumented endpoints.

Group, role, and profile claims are expressions in scope mappings, not separate Authentik group-mapping or role-mapping resources. E3 reviews the exact expressions and claim outputs as security-sensitive code. The provider exposes one `logout_uri` and one selected `logout_method`; front-channel and back-channel are separate tested configurations, and current Authentik documentation labels them Preview. Client-secret and management-token rotation are Fasti-orchestrated sequences, not dedicated generated-client operations. Keep each action `Unavailable` until generation, cutover, revocation, rollback, permission, and secret-disposal behavior pass live proof.

### 12.4 Claim and external-identity contract

Only exact `issuer + subject` is the durable external identity. Authentik database ID, email, username, preferred username, group name, role, and profile are attributes, never identity keys.

E3 freezes required and optional claims plus explicit group, role, profile, email, and username mapping policies. Define missing-claim, changed-email, changed-username, changed-group, disabled-user, deleted-user, duplicate-account, duplicate-external-identity, and account-link behavior. A claim change never creates a second Fasti person. A group or role change never grants Fasti administrator access unless the separately approved mapping policy and Fasti authorization transaction allow it. Account linking requires recent authentication and fresh proof for both sides; it never uses email auto-linking.

### 12.5 Authentik operator experience

The canonical path is:

```text
Settings
└── Account and security
    └── External identity providers
        └── Authentik
```

Show connection mode, issuer, detected and supported versions, application, provider, client type, configured redirects and logout support, requested scopes, claim mappings, management-credential state, last credential use, last validation, last successful sign-in, drift state, and last error.

Actions are Connect existing Authentik, Configure with Authentik API, Preview changes, Apply configuration, Test sign-in, Revalidate, Repair drift, Rotate client secret, Rotate or revoke management credential, Remove management credential, Disconnect, and Remove Fasti-owned Authentik configuration. Expose only actions valid for the current state.

Destructive confirmation names every affected Fasti-owned object, affected user and active-session counts, sign-in methods affected, safe state, rollback limit, and whether OIDC sign-in will stop. Never omit an impact field. If an exact user or session count cannot be established, show `Unknown` or `Unavailable` with the exact reason and require a separately confirmed operator decision. Do not ask for internal Authentik object IDs unless a documented manual recovery needs one.

### 12.6 Dynamic client registration gate

Dynamic client registration is off by default and is not managed drift repair. Enabling it requires a separate security gate and explicit operator policy with an authorized caller allowlist, approved grant types, exact redirect rules, token lifetimes, scope mappings, rate limits, audit, revocation, and cleanup. It uses the exact Authentik DCR scope `goauthentik.io/oidc/dcr` and documented policy bindings. The generated DCR model does not supply every required caller, redirect, rate-limit, or cleanup control. Authentik does not provide RFC 7592 client-management endpoints or automatic expiry and cleanup for these registrations; keep DCR unavailable until the complete source-backed control path passes.

### 12.7 Authentik completion contract

Do not claim Authentik support until the pinned tuple, manual mode, managed mode, accurate dry-run, idempotent and ownership-safe provisioning, real Authorization Code with PKCE, token and claim validation, governed linking and role/group mapping, all configured logout modes, client-secret rotation, management-token rotation/revocation/removal, drift detection, repair, rollback, backup/restore, typed errors, Tabler UI, security, accessibility, exact-head performance, and compatibility-matrix evidence all pass. E3/E4 update every applicable contract and document named in section 17; an `N/A` requires a source-backed reason and reviewer approval.

## 13. Connection and Nuvio contracts

A connection is not a credential. It references a credential that can rotate without changing identity.

```text
connection_id, connection_kind, owner_scope, profile_id, endpoint,
credential_reference, requested_capabilities, granted_capabilities,
connection_state, health_state, created_at, updated_at,
last_checked_at, last_success_at, last_failure_at, last_error_code,
credential_expires_at, retry_policy, remote_version, remote_capabilities
```

States: `unconfigured`, `checking`, `connected`, `degraded`, `offline`, `authentication_required`, `expired`, `revoked`, `disconnected`, `failed`.

Candidate action vocabulary: Configure, Discover, Test, Connect, Reauthorize, Sync now, Pause, Resume, Rotate credential, Revoke credential, Disconnect, Delete. Each named adapter exposes only actions in its approved state machine. Do not create a universal action interface or render irrelevant disabled actions.

Adapters invoke one Fasti capability. They do not write SQLite directly, create a scope vocabulary, or keep browser-local secrets.

### 13.1 Connection-kind security profiles

The shared aggregate does not imply a shared trust profile. A kind remains `unavailable` until its profile passes source, threat, negative-control, resource, and recovery review.

| Kind                     | Required security profile                                                                                                                                                                                                                                                                                                          |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Inbound webhook          | Per-connection message authentication or signature; exact signed bytes; timestamp window; nonce/replay and idempotency store; source/client/profile binding; content type; method/path; pre-parse request, header, and body limits; constant-time verification before domain parsing; rotation and failure audit                   |
| Outbound HTTP or webhook | Approved scheme, host, and port; resolve all and authorize every address; pin the authorized addresses; validate TLS identity; disable redirects and system proxies; load and attach a credential only after destination authorization; isolate sensitive headers; bound request, response, time, retry, concurrency, and log data |
| MQTT                     | TLS and broker identity; approved authentication; client and topic allowlists; profile/owner binding; publish/subscribe direction; Quality of Service and duplicate semantics; retained-message policy; replay/idempotency; payload and queue bounds; reconnect/backoff; credential rotation and revocation                        |
| Local discovery          | Treat every advertisement as untrusted evidence. Store no credential and perform no mutation until explicit operator confirmation, endpoint re-resolution, the outbound security profile, version/capability validation, and owner/profile selection pass                                                                          |

Nuvio, Plex, Tautulli, Jellyfin, Emby, MPRIS, and each later named service instantiate one applicable profile. There is no generic HTTP adapter or UI in the MVP. A later named HTTP integration requires its own plan and cannot weaken a provider's stricter policy.

Current Fasti has real but narrow Nuvio occurrence ingress and Collections interchange. NuvioTV has no Fasti provider. Full compatibility requires upstream NuvioTV work and cannot be proven by Fasti alone.

Retain these distinctions:

- progress is not history;
- watchlist is not watched state or Collection;
- duplicate delivery is not a rewatch;
- absence, error, partial, empty, or cache miss is not deletion;
- caller-declared origin is not authoritative;
- writes require exact identity.

PR H adds Fasti OAuth/device pairing, exact scopes/profile consent, durable Nuvio outbox, timeout-after-commit recovery, idempotency, progress/completion, exact identity/time/origin, snapshots/deltas, tombstones, ordering, bounded cursors, offline replay, conflicts, reconciliation, health, recovery, and later shared catalogs/Collections/metadata projections after PR F.

Raw passwords, service-role keys, direct Nuvio database access, and binding to Nuvio SQL function names are prohibited.

## 14. Commander and plan-gate workflow

The Commander owns source precedence, the canonical plan, architecture, work allocation, integration order, exact-head evidence, GitHub state, and merge decisions.

Use read-only parallel agents for research, archaeology, threat analysis, contracts, UI, providers, Nuvio, migrations, and test design. Use isolated worktrees for parallel writes. Publish a file-ownership table before writes. Two agents do not edit the same file at the same time.

Each agent returns scope, sources, versions or SHAs, findings, evidence, files, tests, risks, blockers, confidence, and explicit `N/A` items.

Gate states:

```text
PENDING
APPROVED
REJECTED
BLOCKED
COMPLETE_WITH_EVIDENCE
```

Required sequence:

```text
Context restore and exact-head refresh
  -> /investigate
  -> /autoplan
     -> /plan-ceo-review
     -> /plan-design-review
     -> /plan-eng-review
     -> /plan-devex-review
  -> /cso and Ponytail reconciliation
  -> final plan approval
  -> Tabler-first mockups and /design-review
  -> approved PR packages
  -> @ponytail-review after each material package
  -> /review and /cso
  -> /qa, /design-review, /impeccable polish, /devex-review
  -> /ship
  -> /retro and /context-save
```

The planning reviews run sequentially. Evidence research can run in parallel. Implementation can parallelize only after dependencies, worktrees, and file ownership are explicit.

| Gate                     | Required result                                                                                       | State                                                                                                                               |
| ------------------------ | ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| 0 Context                | Exact refs, source order, current docs, research, dependency, provider, Authentik, and Nuvio evidence | `COMPLETE_WITH_EVIDENCE`                                                                                                            |
| 1 Root cause             | Correct, fake, unsafe, duplicate, stale, and missing behavior classified                              | `COMPLETE_WITH_EVIDENCE`                                                                                                            |
| 2 User scope             | Multi-PR A-H, full MVP, no compatibility layer, DDD/DRY, Tabler, Ponytail, Badass, accessibility      | `APPROVED` by current user direction                                                                                                |
| 3 Canonical architecture | Ownership, flow, policies, errors, security, migration, backup, packages                              | `COMPLETE_WITH_EVIDENCE` — all sequential plan reviews clear 2026-08-29                                                             |
| 4 CEO review             | Promise, scope, outcome, sequence, non-goals, completion metric                                       | `COMPLETE_WITH_EVIDENCE` — clear 2026-08-29                                                                                         |
| 5 Design plan            | Tabler map, flows, states, cognitive load, WCAG, EN 301 549, QA                                       | `COMPLETE_WITH_EVIDENCE` — 10/10 clear 2026-08-29                                                                                   |
| 6 Engineering            | Boundaries, schema, transactions, dependencies, failures, tests, performance, rollback                | `COMPLETE_WITH_EVIDENCE` — 9/10 clear 2026-08-29                                                                                    |
| 7 Developer experience   | Setup, debugging, compatibility, recovery, measured time to success                                   | `COMPLETE_WITH_EVIDENCE` — 8/10 plan clear 2026-08-29; implementation TTHW remains unmeasured                                       |
| 8 Security and Ponytail  | Threats, negative controls, licence, minimum complete design, no duplicate systems                    | `COMPLETE_WITH_EVIDENCE` — CSO 9/10 and Ponytail clear 2026-08-29                                                                   |
| 9 Final plan             | Review findings reconciled; unresolved items explicit                                                 | `APPROVED` — current user update 2026-08-29; Authentik correction integrated                                                        |
| 10 Design execution      | Tabler-first mockups and design review pass                                                           | `APPROVED` — named user approval `Gate 10 A+C` on 2026-08-29; A is steady state, C is first run, and B is a reusable detail pattern |
| 11 Implementation        | PR A through H independently green and merged                                                         | `IN_PROGRESS`; PR A local exact-head review and delivery remain open                                                                |

## 15. Dependency-ordered multi-PR programme

```text
A Truth reset and session foundation
└── B TrailBase runtime and account lifecycle
    └── C Fasti Access foundation, subject link, clients, devices, vault
        ├── D Passkeys and recovery
        ├── E Generic OIDC, Fasti OAuth AS, Authentik
        ├── F Metadata provider credentials
        └── G Governed connections
            └── H Full Nuvio compatibility
```

Retain A-H as programme stages. Split broad stages into independently reversible sub-PRs when their schema and owners are separable:

| Sub-PR | Scope                                                                                                                 | Dependency and rollback boundary                        |
| ------ | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| C1     | TrailBase anchor, membership, identity bootstrap, exchange, AuthCeremony, and production browser sessions             | After A and B; one identity/session schema and rollback |
| C2     | Existing-client evolution, PATs, devices, grants, scopes, and consent                                                 | After C1 capability IDs freeze; no vault schema         |
| C3     | `CredentialReference`, OS keyring, `C3-CRYPTO`, encrypted headless vault, and authenticated encrypted operator backup | After C1 ownership freeze; independent vault rollback   |
| E1     | Generic OpenID Connect relying party and logout provenance                                                            | After C1; no OAuth authorization-server state           |
| E2     | Fasti OAuth authorization server and E-HOST operations                                                                | After C2 and E0/E-HOST approval                         |
| E3     | Authentik manual compatibility and conformance                                                                        | After E1                                                |
| E4     | Authentik managed operations and durable compensation journal                                                         | After E3 and C3                                         |

Every sub-PR has its own written plan, forward migration, contracts, exact-head tests, failure injection, backup/restore effect, and rollback proof. Do not combine sub-PRs merely to reduce PR count.

Extend the existing `cargo xtask test milestone --body <BODY>` selector with Access bodies A, B, C1, C2, C3, D, E1, E2, E3, E4, F, G, and H. Reuse its existing orchestration, evidence schema, canonical receipt writers, manifest verification, and fail-closed output. Do not add a `test package` command tree or a second receipt format. Any missing prerequisite, unsupported host, planted fault, stale receipt, or failed assertion exits nonzero with one exact next action. Every package also joins `cargo xtask test pr`; deep, high-resource, soak, or multi-repository proof joins `cargo xtask test deep`.

| Package | Hermetic fixture and focused gate                                                                                                                      | CI/resource tier                                                                              |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| A       | Final dormant session schema, direct deterministic domain/application/store fixtures, production-unavailable UI with no route, migration/reset/restart | Focused plus normal PR                                                                        |
| B       | Pinned TrailBase native/OCI process, account lifecycle, wrong version/digest, stop/depot backup/restore, resource envelope                             | Focused plus normal PR; architecture matrix                                                   |
| C1      | Real TrailBase code exchange, cleanup failure, `AuthCeremony`, anchor/bootstrap/session, restart, race, clone fencing                                  | Focused plus normal PR; real browser callback test                                            |
| C2      | Deterministic clients, PATs, devices, scopes, grants, and consent with fake clock, expiry, rotation, reuse, and races                                  | Focused plus normal PR                                                                        |
| C3      | Temporary keyring and vault backends, permission failure, restart, encrypted backup/restore, and zero-plaintext scan                                   | Focused plus normal PR                                                                        |
| D       | Virtual authenticator, passkey/recovery ceremonies, loss and replay                                                                                    | Focused plus deep browser/security                                                            |
| E1      | Hermetic OpenID issuer/client with key rotation and hostile state, nonce, issuer, audience, `azp`, and logout; network denied                          | Focused plus normal PR and browser                                                            |
| E2      | Official protocol vectors, Rust and Go black-box clients, atomic store, restart, sweeper, rate limits, trusted clock, and planted faults               | Focused plus deep conformance; E0 stays a separate decision                                   |
| E3/E4   | Pinned Authentik 2026.8.0 OCI, manual/managed modes, drift, restart, timeout after commit, compensation, and unrelated-object proof                    | Separate high-resource job with at least 2 CPU and 2 GiB; never default low-hardware launcher |
| F/G     | Offline hostile HTTP and service fixtures for transport, credentials, transitions, retry, and recovery                                                 | Focused plus normal PR; credentialed live providers are opt-in                                |
| H       | Pinned Fasti and NuvioTV refs, isolated roots, deterministic clock/outbox/cursors, upstream provider path, and two-profile replay                      | Multi-repository deep job; one real event needs separate authority                            |

Fault injection is test-only, compile-time or test-harness gated, and impossible to enable on a production listener. Each planted fault records its command and the exact assertion or negative-control gate that must turn red. AniList uses public metadata without a permission gate.

Safe dependency lanes:

```text
Foundation: A -> B -> C1
Access:                    C2 -> E2
Vault:                     C3 -> F
Identity:                  E1 -> E3 -> E4
Passkeys:                  D
Connections:              G -> H
```

D starts after C1. G starts after C1 and composes Access projections without owning them. H depends on C2, E2, G, upstream Nuvio, and F for later metadata. Shared domain, application, store, migration, contract-registry, and generator changes merge sequentially. Adapter, UI, fixture, and isolated conformance work can parallelize only after schemas and capability IDs freeze.

Each PR follows: written package plan -> package review -> approval -> implementation -> exact-head QA/review -> rollback proof -> merge -> verify `dev`. A later PR does not make an earlier PR appear green.

### PR A — PR #93 truth reset and session foundation

User outcome: the final session foundation is truthful and testable, while production browser sign-in, session issuance, and inventory state `Unavailable until PR C`. PR A does not claim a usable production identity or session.

Retain:

- opaque digest-backed Fasti sessions, secure cookies, and CSRF;
- capability and authorization kernel;
- account/session inventory information architecture and truthful unavailable-state UI;
- final-administrator behavior, tests, and copy as binding PR C requirements, not as a dormant PR A production control;
- established accessibility behavior.

Remove or replace:

- fake passkey, custom TOTP, backup-code, and fabricated OIDC code, routes, DTOs, SDK methods, schema, and false success copy;
- digest-prefix session IDs, fabricated device/location, stale evidence, direct UI fetches, user-global profile selection, grant creation during selection, and arbitrary 100-year sessions;
- edits to historical v8.

Add:

- the final minimal `AuthSubject`, `FastiBrowserSession`, and exact opaque `BrowserSessionId` domain types;
- validated dormant `SessionPolicy`, idle and absolute expiry, with no hidden
  production defaults; C1 sets exact production values before activation;
- rotation at sign-in and privilege change;
- reliable bounded `last_seen_at`;
- revoke current, one, all other, and all;
- Origin and Host validation plus CSRF;
- session-local selection of an existing authorized profile grant;
- first-class capabilities, typed problems, contract and UI parity;
- deterministic fresh schema, explicit development reset, fixtures, restart, and rollback.

Remove the temporary `BrowserUser` password identity. Test the final `AuthSubject` and session model directly through deterministic domain, application, and store fixtures. Do not add a credentialed fixture listener or dormant production session route. Mark the production capabilities as owned by the later C1 body. The account modal and session inventory show one persistent unavailable state with the exact C1 dependency and operator/user next action.

The A fixtures use deterministic non-secret IDs and timestamps and no credential transport. Remove `FASTI_DEVELOPMENT_TEST_ACCOUNT` and its default from `scripts/dev.sh` in the same package so the launcher cannot point at the deleted `BrowserUser` model. C1 is the first package that mounts production browser-session routes.

Restore migrations v8 and v9 to their exact `origin/dev` definitions. Never modify a migration already present on `dev`. Add the next forward migration for retained final session tables. It must be safe for both a fresh database and each supported developer root. Do not hide a PR-only historical edit behind a schema-version number.

Convert #93 to draft and reconcile with `dev` only after final plan approval and explicit Git mutation authorization.

Gate: no production or fixture listener, unavailable UI, later-body capability ownership, forward migration/reset/rollback, direct domain/application/store fixture coverage, rotation/fixation, expiry, exact-ID collision, last use, concurrent revocation, profile isolation, Origin/Host/CSRF policy tests, SDK boundary, preserved final-administrator behavioral requirements, and accessible UI tests.

### PR B — TrailBase runtime and account lifecycle

User outcome: an operator can install, start, inspect, back up, restore, upgrade, and roll back one exact TrailBase service without SQL.

Add exact native and OCI artifacts, checksums/digests, licence notice, configuration, data root, private admin boundary, native/OCI/remote route-exposure matrix, TLS/trusted proxy, health, readiness, startup, shutdown, restart, supervision, first-start credential redaction, resource limits, upgrade, rollback, full-depot backup, and operator docs.

Prove registration, verification, reset, password, selected social, password-plus-TOTP, removal, and identity administration. Add runtime capability detection and truthful unavailable states.

Explicit limits:

- social callbacks do not prove TOTP for the current authentication;
- refresh does not rotate;
- token has no `iss`, `aud`, `kid`, or `jti`;
- `mfa` means enrolled, not current assurance;
- the shared redirect validator accepts protocol-relative values, so remote
  account and OAuth route exposure is unavailable;
- the isolated admin listener does not expose the second-factor login route,
  so administrator TOTP on that listener is unavailable;
- no passkeys or recovery codes;
- service is alpha.

PR B proves the identity service. It does not use TrailBase tokens as Fasti application sessions.

Gate: native/OCI lifecycle, wrong version/digest, account flows, social/TOTP policy, route matrix, private admin/Record API non-exposure, first-start credential redaction, outage, restart, resources, stopped-depot backup, restore mismatch, upgrade, rollback, and licence review.

### PR C — Fasti Access foundation, exchange, clients, devices, and vault

User outcome: a TrailBase-authenticated person receives a bounded Fasti session and can manage application clients, PATs, devices, and secrets without conflating them.

Add:

- `ExternalAuthLink`, `TrailBaseInstanceId`, membership lifecycle, roles, administrator continuity, auth epoch, recent authentication, and `TokenPolicy`;
- the one-use `access.identity.bootstrap` operation. The trusted CLI or packaged host proves possession of the owner-only data-root `bootstrap.secret`, descriptor-root ownership, correct permissions, and the exclusive data-root lock. A loopback HTTP caller alone is unauthorized. With no existing membership, one transaction creates the first active administrator membership for one proven TrailBase anchor. Concurrent attempts have one winner and no losing side effects. It is distinct from and never reopens the consumed first-client bootstrap endpoint;
- server-side TrailBase code exchange, instance proof, subject collision checks, refresh-session cleanup, Fasti session minting, global sign-out, and disablement handling;
- PATs, public/confidential clients, client secrets, scopes, profile grants, consent, device/client inventories, expiry, last use, rotation, revocation, and audit;
- one `CredentialReference`, OS keyring, encrypted owner-only headless vault, operator secret mount/input, rotation, revocation, encrypted operator backup, and no plaintext fallback.

Reuse PR A's `AuthSubject`, `FastiBrowserSession`, `BrowserSessionId`, and `SessionPolicy` without a second model. Link the PR A subject to TrailBase only through the approved reset/bootstrap path. Replace `BrowserUser.is_admin` with membership and role. Do not dual-run human authentication.

C1 replaces the A-only fixture journey with pinned TrailBase APIs and the real Fasti exchange. The closed-node developer bootstrap reads the owner-only data-root secret through the trusted local CLI or host, proves the descriptor-root and lock, selects and proves the TrailBase anchor, performs one transactional membership/role creation, and prints `Access ready` plus the browser session-inventory URL. It never sends the bootstrap secret to the browser or prints it. On interruption, `--status` reports whether no change occurred, the operation completed, or operator repair is required. A losing race creates no membership, role, profile, or grant and points to the winning initialized state. Test a local process that can connect to the loopback port but cannot read the data root; it must remain unauthorized.

TrailBase disable or deletion stops new sessions and moves the durable anchor to `disabled`, `deleted`, or `recovery_pending`. It does not silently detach, replace, or reuse the anchor. It never cascades into Chronicle deletion. Fasti privacy erasure is a separate explicit capability with its own authorization, preview, recovery limits, and audit.

Gate: `C1-TB-TRUST`, data-root-authorized identity bootstrap and first-admin race, invitation/approval/acceptance/suspension/removal, unaffiliated denial, identity collision/race, anchor lifecycle, signing-key rotation, restore generation and clone fencing, no email auto-link, membership/role/admin continuity, `AccessInvalidationPolicy`, stale auth epoch, TrailBase and OpenID `AuthCeremony` browser-binding/callback/link completion, cleanup failure, outage, PAT/client/device lifecycle, scope/profile isolation, secret leakage, encrypted backup/restore, and transaction-bound authorization.

### PR D — Passkeys and recovery

User outcome: a person can register, name, use, inspect, and remove a passkey and prepare and use governed recovery without a fake fallback.

Use `webauthn-rs =0.5.5`. Implement registration and authentication start/finish. Store ceremony state server-side. Expire and consume it once. Enforce global credential-ID uniqueness, exact RP ID and origin, user verification, client/authenticator data, signature, counter policy, list, name, last use, delete, and virtual-authenticator tests.

Before creating a session, verify the linked TrailBase account state. New passkey sign-in fails closed when that check cannot run. Existing Fasti sessions follow local outage policy.

Implement Fasti recovery codes as a separate Fasti Access lifecycle. They recover Fasti-owned passkey access only. They do not reset a TrailBase password or bypass disablement. Use standard CSPRNG output, digest-only storage, atomic one-time consumption, recent-auth regeneration, count without plaintext read-back, and explicit loss guidance.

Gate: positive ceremony plus wrong origin, RP ID, challenge, replay, malformed payload, duplicate credential, counter, deleted credential, disabled account, unavailable lifecycle check, recovery replay, regeneration, leakage, and device-loss recovery.

### PR E — Generic OIDC, Fasti OAuth server, and Authentik

User outcomes:

- a person can sign in through Authentik or another conformant OIDC provider without account takeover;
- Nuvio, CLI, first-party apps, and approved integrations can obtain narrow Fasti access without a password;
- an Authentik operator can connect manually or safely preview and apply managed configuration.

#### E0 dependency gate

Evaluate `oauth-as 0.9.3` against the complete Fasti profile. It is beta and is not approved by appearance. Pin source and checksum. Run source and CSO review, storage conformance, planted-fault controls, Rust/Go client interop, RFC vectors, mutation-evidence verification, concurrency/fault injection, resource, licence, and maintainer-risk review.

If it cannot safely own the protocol state machines, stop and return for a dependency decision. Do not write the missing protocol by hand. `oxide-auth` is not a drop-in fallback because its documented core is limited and requires substantial custom endpoint and storage policy.

#### Fasti as OIDC RP

Use `openidconnect =4.0.1` for discovery, Authorization Code, PKCE, state, nonce, exact issuer, JWKS, signature, audience, expiry, subject, userinfo where required, refresh, and RP-initiated logout discovery and URL construction.

Fasti adds single-use server state, hardened proxy-free/redirect-free resolved-address transport, bounded issued-at validation through the exact `IdTokenVerifier::set_issue_time_verifier_fn` hook, collision/link policy, disabled-user behavior, access-token hash verification, separately source-backed Authentik front/back-channel logout receivers and logout-token validation, and explicit rejection of unsupported JWE and unsafe multi-audience/`azp`. `openidconnect` 4.0.1 does not supply those receiver paths or a not-before validation surface. A provider profile that requires `nbf` stays `Unavailable` until E0 proves separate pinned parsing and validation support.

Default OpenID sign-in is sign-in only. Discard provider access, refresh, and ID tokens after validated session establishment. Retain an external refresh token only for a named, approved provider API capability. Store it through `CredentialReference` with expiry, rotation, revocation, backup, deletion, and scope rules.

Persist non-secret session provenance: external link ID, exact issuer, subject, optional `sid`, `auth_time`, approved `acr` and `amr`, authentication method, mapped assurance, and verification time. Index `(issuer, sid)` and `(issuer, subject)` for logout fan-out. Back-channel logout validates signature, audience, issuer, time, event, and one-use `jti`. A valid `sid` revokes the matching Fasti session family. A valid subject-only logout revokes every Fasti session established from that issuer and subject. Front-channel logout is not proof of back-channel handling.

#### Fasti as OAuth authorization server

Required profile:

- Authorization Code with mandatory S256 PKCE for public clients;
- Device Authorization with approval, denial, expiry, interval, `slow_down`, and one use;
- bounded access and refresh tokens;
- rotating refresh families and reuse detection;
- revocation, introspection or equivalent governed validation, metadata, exact redirects, public/confidential clients, approved machine credentials, administrative registration, policy-gated dynamic registration, scope/profile consent, secret rotation, inventories, and audit.
- Client Credentials Grant for governed confidential machine clients only. It issues no refresh token unless a separately reviewed profile requires one.

Reject implicit grant, Resource Owner Password Credentials, URL tokens, unbounded bearer lifetime, silent scope expansion, and public-client secrets.

Classify each referenced standard `REQUIRED`, `SUPPORTED`, `REJECTED`, or `DEFERRED WITH REASON`.

#### E-HOST implementation gate

`oauth-as` does not own Fasti's persistence or operations. After E0 pins the exact crate API and before E2 code, publish and approve an `E-HOST` map that covers every host callback and `Storage` method:

- exact existing or new SQLite table, primary key, uniqueness rule, foreign key, index, owner, and retention period;
- transaction boundary and atomic take, claim, consume, rotate, revoke, reuse-detect, and compare-and-set behavior;
- authorization code, device code, user code, access token, refresh family, consent, client, rate-limit, replay, and event-record lifecycle;
- bounded periodic expiry sweeper, batch size, restart behavior, retry limit, idempotence, backlog metric, and alarm threshold;
- per-source, per-client, per-user-code, and per-endpoint rate-limit keys and durable state for unauthenticated exhaustion paths;
- consent presentation and commit, device-form CSRF, trusted clock, event sink, listener/TLS boundary, correlation, redaction, and planted-fault behavior;
- token representation, resource-server validation, audience, revocation, introspection, key lifecycle, and rollback.

Prefer opaque digest-backed access and refresh tokens for Fasti's local resource server. Enable signed JSON Web Tokens and a JSON Web Key Set only when an approved external resource-server or client requirement proves that opaque validation is insufficient. No protocol store can grow without a bound, sweeper, and restart test.

Initial standards ledger:

| Standard/profile                                | Classification                   | Fasti use                                                                                                                            |
| ----------------------------------------------- | -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| RFC 6749 Authorization Code                     | `REQUIRED`                       | Delegated user authorization                                                                                                         |
| RFC 6749 Client Credentials                     | `REQUIRED`                       | Approved confidential machine clients only                                                                                           |
| RFC 6749 Implicit and Resource Owner Password   | `REJECTED`                       | Prohibited by current security posture                                                                                               |
| RFC 7636                                        | `REQUIRED`                       | S256 PKCE; mandatory for public clients                                                                                              |
| RFC 8628                                        | `REQUIRED`                       | Device Authorization                                                                                                                 |
| RFC 7009                                        | `REQUIRED`                       | Token revocation                                                                                                                     |
| RFC 7662                                        | `REQUIRED`                       | Governed introspection, or an explicitly equivalent internal validation contract when no external resource server needs the endpoint |
| RFC 8414                                        | `REQUIRED`                       | Server metadata derived from active routes                                                                                           |
| RFC 9207                                        | `REQUIRED`                       | Authorization response issuer and mix-up defense                                                                                     |
| RFC 9700                                        | `REQUIRED`                       | OAuth security best current practice                                                                                                 |
| RFC 7591                                        | `SUPPORTED` behind policy        | Dynamic registration is off by default                                                                                               |
| RFC 7592                                        | `SUPPORTED` only if E0 proves it | Dynamic client management; never assume Authentik DCR supplies it                                                                    |
| OIDC Core and Discovery                         | `REQUIRED` for the RP            | Generic OIDC and Authentik                                                                                                           |
| OIDC RP, front-channel, and back-channel logout | `REQUIRED` for Authentik profile | Separate configured and receiver proof                                                                                               |
| PAR, DPoP, mTLS, JAR, RAR, token exchange       | `DEFERRED WITH REASON`           | Outside the approved MVP profile unless threat or client requirements make one mandatory                                             |

#### Authentik

Implement and test section 12 as the single Authentik contract. Positive and lifecycle evidence must separately cover server-version discovery, exact tuple match, API-token authentication, OAuth API authentication with `goauthentik.io/api`, application/provider creation, idempotent repeat and safe update, dry-run accuracy, ownership-marker enforcement, redirect and logout configuration, scope-mapping CRUD, reviewed group/role/profile claim-expression outputs, client-secret and management-token rotation, management-token revocation/removal, drift, repair, rollback, OIDC discovery and code with PKCE, ID-token validation, refresh, RP/front/back-channel logout, JWKS rotation, user disable/delete, changed email, changed username, changed group, duplicate external identity, backup/restore, outage, restart, and exact-head offline behavior for existing Fasti sessions.

Negative controls must separately fail on wrong-object update, unsupported version, missing API permission, wrong issuer/audience/redirect/state/nonce/PKCE, unvalidated signing-key change, disabled-user sign-in, unapproved role or group grant, duplicate-account takeover, account-link takeover, management-secret log/browser/export leakage, stale dry-run, timeout after remote commit, and concurrent unrelated-object change.

Gate: E0 and E-HOST pass; OpenID provenance, token disposal, logout fan-out, and callback tests pass; OAuth positive, negative, atomic-storage, sweeper, rate-limit, consent/CSRF, token-lifecycle, and planted-fault conformance pass; exact Authentik tuple passes pull, resource, soak, conformance, accessibility, security, backup/restore, and performance evidence.

### PR F — Metadata provider credentials

User outcome: a person or operator sees which providers need credentials, why, their state, and the safe action without a card wall or generic Desktop instruction.

Add section 11's registry, typed input, lifecycle, validation, expiry, refresh, attribution, rate policy, offline behavior, compact Tabler table/list, and Configure, Replace, Test, Reauthorize, Remove, and View safe details. Never add plaintext read-back.

Reuse PR C vault, current data-root keyring, zeroization, sensitive headers, resolve-once/all-address authorization, proxy/redirect denial, request bounds, exact-item refetch, and atomic Record/claim creation.

Normal export excludes secrets. Explicit encrypted operator backup can include the vault.

Gate: registry completeness, all eight states, typed input, secret clearing, each backend, each provider's exact transport/policy, attribution, expiry/refresh, offline, URL/log/browser/export leakage, redirects, proxies, DNS rebinding, malformed/oversized response, timeout, and 429 behavior.

### PR G — Governed connections

User outcome: a person can configure, test, understand, pause, repair, rotate, revoke, disconnect, and delete a service connection without confusing it with a credential.

Add section 13's `Connection` aggregate only for named service integrations: Nuvio, Plex, Tautulli, Jellyfin, Emby, MPRIS, webhooks, local discovery, and MQTT. Metadata provider configuration and health remain owned by PR F and the Metadata surface. External OpenID sign-in providers remain Identity Integration configuration plus `ExternalAuthLink`. Fasti native and third-party clients remain Access-owned `ApplicationClient` and `DeviceGrant` records.

Retain real observation adapters. Replace static `IntegrationStatusDto` projection with real state. Replace the card grid with a compact Tabler list/table.

Gate: transitions, authorization, stable identity across rotation, health/retry/outage, profile ownership, adapter boundary, restart, backup/restore, offline, accessible state, and typed errors.

### PR H — Full Nuvio compatibility

User outcome: a Nuvio user can pair safely, choose profile/scopes, synchronize without duplicates or silent deletion, inspect health, recover offline work, and revoke access.

Build on C clients/vault, E OAuth/device, G connections, F metadata for later projections, current Fasti occurrence/Collections, and a separately reviewed NuvioTV provider change.

Implement section 13. Retain replay, distinct-rewatch, foreign-client, ordered-delta, projection, Collections parsing, profile isolation, API, SDK, and UI tests. Add production outbox, timeout-after-commit, two-client identity, snapshot/delta, tombstone, cursor, offline conflict, reconciliation, two-device/two-profile, migration, backup/restore, performance, security, and accessibility proof.

Gate: current pinned NuvioTV conformance and a real delivered event pass. Full compatibility cannot be claimed until upstream NuvioTV contains and tests a Fasti provider through its normal abstraction.

### 15.1 MVP capability ledger

The repository ledger must represent each row as a stable machine-readable capability with owner, PR, interim UI state, acceptance test, rollback proof, exact-head evidence, and verified `dev` evidence.

| Required capability                                                             | Owner                      | PR       | Interim state before merge                                   | Completion evidence                                                    |
| ------------------------------------------------------------------------------- | -------------------------- | -------- | ------------------------------------------------------------ | ---------------------------------------------------------------------- |
| Registration, username/email, password sign-in/change/reset, email verification | TrailBase                  | B/C      | Visible `Unavailable` with service/setup action              | Pinned-runtime positive/negative lifecycle tests                       |
| Supported social sign-in                                                        | TrailBase                  | B/C      | Visible per configured provider                              | Provider flow, collision, outage, and assurance tests                  |
| TOTP enrollment, verification, removal, password-login MFA                      | TrailBase                  | B/C      | Visible with assurance limitation                            | Current-event TOTP proof and recent-auth removal                       |
| TrailBase anchor and external identity links                                    | Fasti Access               | A/C/E    | No automatic link                                            | Unique instance/subject, link/unlink/collision/recovery                |
| Membership, roles, final administrator, media-profile grants                    | Fasti Access               | C        | Unaffiliated denial                                          | Bootstrap/invite/approval/suspend/remove and transaction authorization |
| Browser sessions, inventory, expiry, rotation, revocation, global sign-out      | Fasti Access               | A/C      | Dormant foundation in A; production `Unavailable until PR C` | Policy boundaries, fixation, callback, concurrency, auth epoch, outage |
| Recent authentication and assurance                                             | Fasti Access               | C/D/E    | Sensitive actions unavailable without proof                  | Method matrix and per-capability minimum assurance                     |
| Passkeys and passkey inventory                                                  | Fasti Access               | D        | Visible `Unavailable`                                        | Virtual-authenticator positive and negative suite                      |
| Recovery codes and recovery journey                                             | Fasti Access               | D        | Recovery guidance only                                       | One-time atomic consume, regeneration, leakage, device loss            |
| Generic OIDC and Authentik sign-in                                              | Identity Integration       | E        | Visible `Unavailable`                                        | Conformance, linking, claims, logout, disable/delete                   |
| Authentik manual and managed configuration                                      | Authentik adapter          | E        | Manual documentation only                                    | Versioned dry-run, ownership, apply, drift, repair, rollback           |
| Authorization Code and mandatory PKCE                                           | Fasti Access               | E        | No endpoint claim                                            | RFC/client interoperability and negative controls                      |
| Device Authorization Grant                                                      | Fasti Access               | E        | Visible pending approvals area                               | Approval/denial/expiry/interval/slow-down/replay                       |
| OAuth access tokens and rotating refresh families                               | Fasti Access               | E        | No token issue                                               | Issuance, expiry, rotation, reuse, race, revocation                    |
| Revocation, introspection/governed validation, server metadata                  | Fasti Access               | E        | No advertised endpoints                                      | Metadata-to-real-route, ownership, no existence oracle                 |
| Client Credentials for approved confidential machines                           | Fasti Access               | E        | Unavailable                                                  | No public clients, no refresh by default, narrow scopes/audience       |
| Administrative client registration, secrets, consent, rotation                  | Fasti Access               | C/E      | Current single-client foundation only                        | Public/confidential lifecycle, exact redirect/scope/profile consent    |
| PATs                                                                            | Fasti Access               | C        | Visible `Unavailable`                                        | One-time display, digest, expiry, scope, use, rotate, revoke           |
| Connected client/device inventories                                             | Fasti Access               | C/E      | Separate truthful empty states                               | Exact owner, use, expiry, grant, revoke                                |
| Provider credential registry and governed vault                                 | Metadata/Vault             | C/F      | Existing Google/TMDB truth only                              | Registry, backends, lifecycle, leakage, provider tests                 |
| Governed service Connections                                                    | Connection                 | G        | Static statuses labelled incomplete                          | State machine, health, rotation, outage, recovery                      |
| Nuvio pairing and full synchronization                                          | Nuvio Interoperability     | H        | Current occurrence/Collections truth only                    | Upstream provider, real event, outbox, snapshots/deltas, recovery      |
| Backup, restore, upgrade, rollback, explicit recovery                           | Each owner plus Operations | B-H      | Current Fasti backup limits stated                           | Joint manifest and mismatch matrix                                     |
| Capability, OpenAPI, AsyncAPI/NA, Schema, SDK, CLI, problems, docs              | Contracts                  | Every PR | Current drift blocks activation                              | Deliberate mutation gate and exact-head parity                         |

MVP completion means zero required ledger rows are missing, `proposed`, `reserved`, falsely active, or unsupported. A merged PR count is not a completion metric.

### 15.2 Separate inventories

The UI can group these under Access and Connections. Storage and lifecycle remain separate:

- Signed-in sessions: current marker, created/last activity/expiry, observed client/network data or `Unknown`, and revoke actions.
- Passkey credentials: user name, created/last use, safe authenticator data, backup state when available, rename, revoke.
- Registered clients and paired devices: client type, public/confidential, requested/granted scopes, profile grant, created/last use/token expiry, revoke.
- Service connections: service, endpoint, credential state, health, capabilities, profile mapping, last success/error, disconnect.

Do not fabricate device name, browser, operating system, or location.

### 15.3 Competitor capability comparison

Before PR B implementation and again at MVP close, build a dated source-backed comparison for [Ryot](https://docs.ryot.io/guides/authentication), [Cinephage](https://docs.cinephage.net/reference/database/schema-overview#authentication-better-auth), and [Yamtrack](https://github.com/FuzzyGrim/Yamtrack/wiki/Social-Authentication-in-Yamtrack).

Compare local accounts, registration policy, verification, recovery, social/OIDC, local-and-OIDC coexistence, linking, SSO-only and auto-redirect, TOTP, passkeys, sessions, recent auth, disable/delete, administrator continuity, PKCE, device pairing, PAT/API credentials, provider/service credentials, migration, backup/restore, offline behavior, accessibility, and recovery.

Map each gap to PR A-H and a ledger capability. Fasti must meet or exceed the user capability, not copy architecture or inflate feature count. Do not claim competitor behavior without current primary evidence. Do not claim superiority without exact Fasti proof.

## 16. File ownership and parallel execution

The Commander publishes exact ownership before each wave.

| Workstream                | Primary paths                                     | Rule                                                                   |
| ------------------------- | ------------------------------------------------- | ---------------------------------------------------------------------- |
| Access domain/application | `crates/fasti-domain`, `crates/fasti-application` | One semantic owner.                                                    |
| Store/migrations          | `crates/fasti-store`, fixtures                    | One migration owner; no concurrent schema edits.                       |
| API/contracts             | `crates/fasti-api`, `contracts/registry`, `xtask` | Contract source owner coordinates generation.                          |
| TrailBase package         | approved service path, packaging, operator docs   | Separate from Fasti schema work.                                       |
| Desktop/vault             | `apps/desktop/src-tauri`, secret backends         | One secret-boundary owner.                                             |
| UI                        | `packages/ui`, `apps/web`                         | One flow owner per settings area; preserve shell owner.                |
| Authentik conformance     | harness and compatibility docs                    | Isolated from generic OIDC domain code.                                |
| Providers                 | registry, adapters, fixtures                      | One registry owner; adapters parallelize after schema freeze.          |
| Nuvio                     | Fasti modules plus separate upstream worktree     | No shared cross-repository file ownership.                             |
| QA/evidence               | tests, receipts, logs                             | Read-only while writers are active unless exact fixtures are assigned. |

No agent edits the quarantined checkout. No force-push, rebase, branch rewrite, draft conversion, PR comment, or merge occurs without applicable authority and exact target checks.

## 17. Contracts and documentation

Every PR updates each applicable capability-ledger, OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD, SDK, CLI, permissions, typed problems, examples, lifecycle/conformance, user/operator guide, architecture/security guide, external-identity guide, backup/restore/migration/rollback/troubleshooting/compatibility matrix, changelog, and `AGENTS.md` surface. An `N/A` requires a source-backed reason and package-review approval.

### 17.1 Configuration and secret-input contract

Before a package adds an input, extend one canonical inventory in `docs/network-configuration.md`. Each row records the exact key or field, owning process/bounded context, source, default, required environment, precedence, restart or reload effect, redacted status display, validation, stable problem code, and deprecation rule. The package plan freezes the rows before code.

Input families are Fasti listener/public origin/data root, TrailBase executable/origin/depot/mail/social/runtime policy, browser-session policy, OpenID provider/client, Authentik server/management, vault backend, provider registry/credentials, Connections, and Nuvio client/sync. Do not copy one input into multiple owners.

- Keep the existing documented Fasti environment contract as the single Fasti non-secret runtime surface until a separately approved configuration-file need exists. TrailBase keeps its pinned official configuration boundary. `scripts/dev.sh` validates and reports both without inventing a third configuration layer.
- Headless/native/OCI secrets enter through permission-checked file descriptors or mounted secret files. Desktop secrets use the existing data-root-scoped keyring. Do not accept secrets in command arguments, URLs, ordinary `.env` files, browser storage, logs, screenshots, fixtures, or committed documentation.
- Extend the current status/diagnostic path. Do not add a second admin tool. Human and machine-readable output validates exact versions/digests, roots and permissions, ports, origins, TLS/trusted proxy, vault, migration, readiness, capability state, safe reason, and one next action. It never prints a secret.

### 17.2 Troubleshooting and correlation

Propagate one Fasti correlation ID from browser, SDK, or CLI through Fasti application and adapter boundaries. Record a redacted upstream request ID separately for TrailBase, OpenID, Authentik, provider, and Nuvio calls. Do not replace the Fasti ID or expose a secret-bearing upstream value.

`./scripts/dev.sh --status` prints this worktree's native, Workbench, TrailBase, and container log locations and the exact native or Podman/Docker command to inspect them. One canonical `docs/problems.md` file has an indexed anchor for each stable problem code with likely causes, safe state, retry safety, exact repair, correlation field, and applicable operator command. Do not create a page tree per error. The UI keeps plain default copy and exposes a labelled copyable correlation ID in details.

At minimum, induce and trace `trailbase_session_cleanup_failed`, `authentik_configuration_drift`, and `oauth_token_reuse_detected` from failure to the visible typed problem, same-ID redacted component logs, repair, and successful retry.

### 17.3 Documentation ownership

| Existing document                                                            | Required Access update                                                                                                                    |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| `README.md`                                                                  | One prepared-machine native and cached exact-digest OCI golden path and truthful status                                                   |
| `docs/dev-loop.md`                                                           | Native/Podman/Docker start, status, open, stop, scoped Access reset, focused tests, offline cache, logs, and recovery                     |
| `docs/network-configuration.md`                                              | Complete non-secret and secret-input inventory, precedence, validation, and reload behavior                                               |
| `docs/architecture/authentication.md`                                        | Final credential, identity, ceremony, session, client, vault, and Connection boundaries                                                   |
| Authentik operator, security, external-identity, and compatibility documents | Manual and managed setup, exact tested tuple, API permissions, claims, logout, drift, repair, backup/restore, removal, and support limits |
| `docs/operations/rollback-runbook.md`                                        | Forward migration, `BackupEpoch`, isolated restore, activation, fencing, and executable data rollback                                     |
| `docs/problems.md`                                                           | Single indexed typed-problem catalogue and correlation-led repair loop                                                                    |
| `tests/conformance/README.md`                                                | Hermetic package, high-resource Authentik, multi-repository Nuvio, planted-fault, and network-denied commands                             |
| `packages/sdk/README.md`                                                     | Real authenticated examples, typed problems, correlation, revocation, and no embedded secrets                                             |
| `CONTRIBUTING.md`                                                            | Package selection, prerequisites, receipts, exact-head evidence, and review gates                                                         |
| `AGENTS.md`                                                                  | Permanent source order, launcher, security, Tabler, and validation rules after the design is final                                        |
| `CHANGELOG.md`                                                               | Truth reset and each activated capability, with unavailable or breaking pre-release state stated literally                                |

Use JSON-LD only when public semantic domain data changes. Secrets, sessions, refresh tokens, provider keys, and client secrets are not linked-data entities. Record `JSON-LD: N/A — security state, no public semantic entity`.

AsyncAPI applies only to an externally visible asynchronous event. Do not invent an event for a synchronous route. Record the reason.

Each typed problem includes safe state, retry status, exact next action, correlation ID, and documentation link.

Stable problem families include:

- `identity_service_unavailable`, `trailbase_version_unsupported`, `trailbase_trust_unavailable`, `trailbase_proof_invalid`, `trailbase_session_cleanup_failed`, `auth_browser_binding_invalid`;
- `auth_subject_unaffiliated`, `auth_identity_conflict`, `auth_last_sign_in_method`, `auth_assurance_insufficient`, `recent_authentication_required`;
- `browser_session_expired`, `browser_session_revoked`, `session_policy_changed`;
- `passkey_challenge_invalid`, `passkey_origin_invalid`, `passkey_replayed`, `recovery_code_invalid`;
- `oauth_client_invalid`, `oauth_redirect_invalid`, `oauth_scope_invalid`, `oauth_consent_required`, `oauth_device_pending`, `oauth_device_slow_down`, `oauth_token_reuse_detected`;
- `credential_store_unavailable`, `credential_invalid`, `credential_expired`, `credential_quarantined`, `access_invalidation_incomplete`, `backup_manifest_invalid`, `connection_profile_unavailable`, `connection_degraded`, `connection_authentication_required`;
- `authentik_unreachable`, `authentik_version_unsupported`, `authentik_api_unauthorized`, `authentik_api_forbidden`, `authentik_configuration_invalid`, `authentik_configuration_drift`, `authentik_provisioning_failed`, `authentik_rollback_failed`, `authentik_claim_invalid`, `authentik_identity_conflict`;
- `nuvio_pairing_expired`, `nuvio_sync_conflict`, `nuvio_cursor_expired`, `nuvio_reconciliation_required`.

## 18. Security and negative controls

Threat-model credential stuffing, brute force, enumeration, session fixation/theft, CSRF, token theft/replay, stale TrailBase exchange, account-link takeover, issuer confusion, OIDC mix-up, SSRF, redirects, JWKS rotation, WebAuthn origin/RP/challenge, TOTP reset/removal, final-admin loss, cross-workspace/profile access, secret leakage, migration failure, restore mismatch, denial of service, and dependency compromise.

Negative controls include:

- fake UI success, missing/historical migration, stale evidence, and contract drift;
- session fixation, cross-user/profile access, final-admin removal, and stale auth epoch;
- PAT/client scope escalation, plaintext storage, wrong profile grant, silent expansion, public-client secret;
- provider secret in retained browser state, URL, log, screenshot, fixture, export, redirect, proxy, or unsafe DNS route;
- wrong OIDC issuer, audience, state, nonce, PKCE, redirect, discovery address, token hash, JWKS rotation, disabled user, collision;
- TrailBase copied callback in a clean browser, missing/mismatched pre-auth binding, sibling-subdomain injection, unsupported state assumption, wrong proof key/version/generation, clone, and retired key;
- TrailBase cleanup failure;
- wrong WebAuthn origin, RP ID, challenge, replay, malformed/duplicate credential, disabled linked account;
- invalid TOTP and removal without recent authentication;
- recovery replay and regeneration race;
- device replay, expiry, fast polling, denial, consent mismatch;
- refresh reuse, issuance race, revocation, introspection isolation;
- Authentik wrong object/version/permission/role, drift, rollback failure, secret leak;
- local process with loopback access but no data-root/bootstrap-secret authority;
- incomplete Access invalidation, stale auth/authorization/restore epoch, copied backup credential resurrection, modified backup manifest, wrong vault key, and external credential restored active;
- unsigned/replayed/oversized webhook, hostile discovery result, unsafe HTTP resolution/TLS/redirect/proxy, MQTT wrong broker/topic/retained replay, and accidental unnamed arbitrary-HTTP adapter exposure;
- connection expiry/revocation, invalid transition, direct-store adapter;
- Nuvio timeout after commit, duplicate/changed replay, silent deletion, expired cursor, cross-profile state.

The negative-control harness proves every gate can turn red.

### 18.1 Data disposition and incident response

Before a package stores personal, security, or audit data, set an exact retention period or source-backed event rule. `Unspecified` is `BLOCKED`.

| System                     | Purpose and owner                                                                                            | Protection and export                                                                                                                         | Delete, retention, backup, and restore rule                                                                                                                                                                                            |
| -------------------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| TrailBase                  | Human account, password/social/TOTP lifecycle; TrailBase owner                                               | Separate depot, private routes, pinned artifact, encrypted operator backup                                                                    | TrailBase account deletion does not delete Fasti. Set upstream audit/session retention from proven controls. Joint backup expiry is explicit. Restore replays Fasti erasure/invalidation records before activation.                    |
| Fasti Access and Chronicle | Authorization, sessions, audit, media state; separate bounded-context owners                                 | SQLite/data-root controls, capability authorization, credential-free portability for domain data                                              | Privacy erasure enumerates delete, anonymize, shared-record retention, legal/integrity retention, and receipt. Chronicle is never implicitly deleted by identity loss. Restore re-applies every later erasure and invalidation record. |
| Authentik                  | External identity and Fasti-owned application/provider configuration; external owner plus management adapter | Authentik owns human data. Fasti stores only link provenance, owned object IDs, redacted operation journal, and vaulted management credential | Disconnect does not delete the Authentik user. Fasti-owned object deletion is separate and state-matched. Set journal retention and remove/quarantine management credentials under policy.                                             |
| Credential vault           | Provider, service, external refresh, and management secrets; Credential Vault owner                          | `C3-CRYPTO`, least-privilege backend, no plaintext export; optional authenticated encrypted operator backup                                   | Delete or rotate by credential owner. Backups expire under operator policy. Restored external secrets are quarantined until validation or rotation.                                                                                    |
| Logs and problem evidence  | Redacted diagnosis, security audit, correlation, and recovery; operator/audit owner                          | No tokens, secrets, codes, raw sensitive payloads, or unnecessary personal data. Bound size and access. Export only redacted evidence.        | Set per-log/audit retention and deletion/anonymization law/policy. Preserve only necessary incident evidence. A restore cannot bypass deletion policy.                                                                                 |
| Joint backups              | Disaster recovery across Fasti, TrailBase, vault, and erasure ledger; operator owner                         | Authenticated encrypted manifest and artifacts, offline access control, generation and fencing                                                | Define expiry, inventory, destruction verification, restore-to-isolated target, post-backup erasure replay, credential invalidation, and activation receipt.                                                                           |

Every erasure creates a signed or authenticated data-disposition record that identifies affected systems without retaining erased content. Backup restore consumes all later disposition records before enabling authentication.

Incident playbooks share these steps: declare and correlate the incident; stop or narrow issuance; contain and revoke; rotate or advance the owning epoch/generation; preserve redacted evidence; determine exposure; apply required notification; rebuild from pinned artifacts if needed; verify recovery; and close with affected counts and exact-head receipts.

| Incident                                                             | Minimum containment and recovery                                                                                                                                                                                                  |
| -------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Browser session, recent-auth, or ceremony compromise                 | Revoke named/all subject sessions, invalidate ceremony/browser-binding state, advance subject auth epoch, inspect link and action audit, reauthenticate                                                                           |
| PAT, OAuth refresh family, device grant, or client secret compromise | Revoke the entire owning family/epoch and derived access, block reuse, inspect scopes/profile use, rotate only after review                                                                                                       |
| Provider or service credential compromise                            | Pause writes, quarantine the `CredentialReference`, revoke/rotate at the provider, inspect bounded outbound logs, retest destination and capability before resume                                                                 |
| Authentik management credential compromise                           | Remove credential, stop managed writes, inspect `AuthentikOperation`, rotate/revoke in Authentik, verify Fasti-owned and unrelated objects, resume in manual mode until cleared                                                   |
| TrailBase signing key, depot, or instance compromise                 | Stop new Fasti sessions and passkey lifecycle checks, fence the instance/generation, rotate through `C1-TB-TRUST`, revoke supported TrailBase sessions, rebuild from pinned artifact and authenticated backup, revalidate anchors |
| Vault master key or operator-backup compromise                       | Stop secret-dependent operations, rotate the key hierarchy, quarantine all external credentials, advance restore/credential generations, reissue local bearer/client/recovery credentials, inventory and destroy affected backups |
| Dependency or artifact compromise                                    | Freeze affected package, revoke exposed credentials, verify checksums/provenance, rebuild from a reviewed pinned version, run planted-fault/conformance/security gates, publish affected-version and recovery evidence            |

## 19. Migration, backup, restore, and rollback

- Restore migrations v8 and v9 to the exact definitions already on `origin/dev`. Never edit a migration present on `dev`. Add the next forward migration for retained final Access tables. Remove PR-only authentication tables through branch reconciliation, not a false historical schema version. Do not retain them for compatibility.
- Preserve unrelated Fasti data and developer roots through explicit backup/reset choices.
- Establish final fresh schema and deterministic development reset.
- PR A adds `./scripts/dev.sh --reset-access`. It validates and prints the
  exact worktree-local Fasti root, states that the TrailBase root is unavailable,
  and refuses an Access-only mutation because PR A mounts no public reset
  service. The separate `--full-dev-root` argument requires exact confirmation,
  stops only this worktree's supervised processes, refuses a non-development,
  outside-worktree, symlink-escaped, active, or ambiguous root, retains the old
  root as a recoverable backup, and rebuilds through normal forward migrations
  and public service probes. It never edits SQLite or TrailBase tables. This is
  an unreleased-MVP developer reset, not a compatibility path.
- Test fresh, each supported development root, restart, concurrency, and rollback from a copy.
- TrailBase and Fasti keep separate roots and migrations.
- A coordinated `BackupEpoch` is the consistency boundary. Reject new Fasti authentication, linking, membership, grant, credential, token, and session mutations; drain active mutations; checkpoint Fasti SQLite; stop TrailBase; then capture the full TrailBase depot and optional encrypted vault. Chronicle ingestion may continue only when its state cannot change a manifest-bound Access invariant.
- The `C3-CRYPTO`-authenticated joint manifest records the backup epoch, every artifact digest and version, Fasti data/blobs, TrailBase full depot, stable instance identity, activation generation, versioned verification-key set, subject-link integrity, erasure ledger, optional encrypted vault artifact, quiesce/drain evidence, and restore order.
- Restore starts in authentication-disabled mode. Verify every artifact, manifest authentication, TrailBase instance, activation generation, verification-key set, Fasti schema, link, erasure record, and recovery case before explicit activation. Apply later erasure/invalidation records, advance the restore generation, invalidate copied local credentials, and quarantine external credentials. A missing or partial artifact cannot mint a session. A clone or restored copy requires a new activation generation and proof that the former deployment is fenced.
- Test Fasti-only, TrailBase-only, mismatched, cloned, partial, and joint restore.
- Interrupt backup and restore at every quiesce, checkpoint, stop, capture, manifest, verification, and activation boundary. Prove restart is idempotent and no partial state can issue access.
- Normal portability excludes credentials and auth bindings. Operator backup is separate and encrypted.

Keep credential-free portability on `fasti export`, `fasti restore`, and `fasti verify`. Do not overload it with Access secrets. Add the separate proposed operator family:

```text
fasti operator-backup create  --output <backup-directory>
fasti operator-backup verify  --input <backup-directory>
fasti operator-backup restore --input <backup-directory> --target <isolated-target-root>
fasti operator-backup activate --target <isolated-target-root>
fasti operator-backup rollback --target <isolated-target-root>
```

These commands are thin high-risk adapters over the existing B3 archive, data-root lock, restore activation, recovery coordinator, verification, and receipt machinery. Extend the existing archive and activation formats with the authenticated joint manifest and encrypted vault fields. Do not build a second backup engine, lock, restore state machine, archive format, or receipt writer.

Paths are non-secret. Vault unlock material enters by an approved file descriptor, mount, or Desktop keyring, never an argument. Each command prints a digest-bound human summary and machine-readable receipt without credentials. `restore` cannot target the active roots. `activate` requires full verification, an authentication-disabled target, an incremented activation generation, and former-deployment fencing. `rollback` follows a verified previous backup generation; it cannot pretend to reverse an unrecorded mutation.

Every migration package documents and tests exact commands for fresh install, each supported development root, forward migration, restart, failed-forward recovery, restore from a copy, and old-binary rollback. A developer rehearses them on temporary roots without SQL. Update `docs/operations/rollback-runbook.md` in the first package that activates data rollback.

## 20. Performance and memory

Repository targets remain 64 MiB idle, 96 MiB normal, 160 MiB heavy, and a 192 MiB absolute process-tree ceiling. Measure whether Fasti plus TrailBase fits. If not, default activation is blocked pending product decision; TrailBase is not silently replaced.

Bind each result to exact commit, artifact, environment, command, sample count, baseline, result, and limit.

Before package code, approve its performance test contract. Define the production-shaped dataset, concurrency, cold and warm state, sample count, percentile method, p50/p95/p99 latency ceilings, CPU ceiling, file-descriptor and socket-growth ceiling, soak duration, allowed memory slope, cleanup-backlog ceiling, and explicit failure threshold. Set thresholds before observing the package result. A package with missing criteria is `BLOCKED`, not unmeasured success.

Measure Fasti/TrailBase/combined memory, startup/readiness, sign-in/exchange, session operations, TOTP, passkeys, OIDC, OAuth code/device/token/refresh/revocation/introspection, provider tests, connection health, Nuvio outbox/sync/reconciliation, cleanup, backup, restore, and package size.

Run Fasti and TrailBase inside one enforceable aggregate cgroup or equivalent process-tree resource boundary for combined measurements. Record child processes. No unbounded queue, table, cache, request, redirect, retry, log, poll, or cleanup batch. Normal Fasti requests do not call TrailBase on every request.

Authentik conformance has a separate resource and soak receipt. Authentik is not in the default runtime bundle.

## 21. Developer experience

### 21.1 One local launcher and golden path

Keep the existing `./scripts/dev.sh` family as the only local developer launcher. Do not add Compose, a second launcher, or a parallel dev environment. After C1, its default native mode supervises the pinned TrailBase process, Fasti, and the current Workbench under separate worktree-scoped roots. `--podman` and `--docker` use the same exact-digest topology. Authentik remains outside this default path.

Prepared native sequence:

```text
./scripts/dev.sh
./scripts/dev.sh --status
./scripts/dev.sh --open
./scripts/dev.sh --stop
```

Cached exact-digest OCI sequence replaces the first command with `./scripts/dev.sh --podman` or `./scripts/dev.sh --docker`. The launcher prints both exact data roots, Fasti commit/artifact, TrailBase version and checksum or digest, selected ports and public callback origin, readiness, log locations, browser URL, and one safe next action. It never prints a credential.

Readiness requires the pinned TrailBase process, its version/capability check, the Fasti durable router and migration state, the Workbench when present, and the exact callback origin. Health-only is not authentication success. A ready terminal line is `Access development environment ready: <browser-url>`. A blocked line is `Authentication unavailable: <reason>. Next: <action>`.

Golden-path success is a real TrailBase authentication, the one-use first-administrator membership bootstrap when needed, an opaque Fasti browser session, and a visible current-session row in Account and security. For a prepared machine, the TrailBase account and exact artifacts already exist. The target is at most 5 minutes and at most 3 user actions after launcher start: open the printed URL, authenticate, and confirm the local-operator bootstrap if needed. Measure first account creation and email delivery separately. Do not hide build, pull, registration, or email time inside the warm target.

Current combined Fasti plus TrailBase authentication time to first success is `UNMEASURABLE`; no executable path exists at the current exact head. The target above is a proposed package gate, not current evidence.

Write a time-to-first-success receipt with exact head, artifact/digest, environment, architecture, native or OCI mode, prepared/cold state, command, timestamps, user-action count, success assertion, failure, and receipt digest. This is a target, not a claimed result. A package does not pass until the measured receipt meets its approved target or returns for an explicit target/product decision.

`--status` supports human and machine-readable output. It reports the exact safe state and next action for an unconfigured TrailBase service, dormant PR A, missing identity bootstrap, failed refresh-session cleanup, unsupported version, vault failure, migration failure, or resource-bound violation.

### 21.2 Supported developer matrix and constrained hardware

| Host                          | Native Fasti + TrailBase                                                | OCI                                                      | Default Access development                     | Authentik conformance                              |
| ----------------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------- | ---------------------------------------------- | -------------------------------------------------- |
| Linux x86_64                  | Required exact-artifact gate                                            | Required exact-digest gate                               | Supported only after C1 evidence               | Separate high-resource job                         |
| Linux arm64                   | Required exact-artifact gate                                            | Required exact-digest gate                               | Supported only after C1 evidence               | Separate high-resource job after exact image proof |
| Other desktop hosts           | `Unavailable` until native package and restore activation prove support | `Unavailable` unless exact runtime/image evidence exists | No inferred support                            | Not a default gate                                 |
| Android or television targets | Not a developer-service host                                            | Not a default host                                       | Client conformance only after its package gate | N/A                                                |

Run Fasti and TrailBase under the existing aggregate 192 MiB process-tree boundary. If a component or the aggregate exceeds its approved threshold, the launcher names the component and bound, stops the supervised environment, and points to the receipt. It never raises the ceiling. Authentik's pinned conformance profile requires at least 2 CPU and 2 GiB and never runs in the low-hardware launcher.

### 21.3 Developer journeys and feedback

Document and test first success for local start, TrailBase bootstrap, account, sign-in, verification, reset, social, TOTP, passkey, recovery preparation, generic OpenID Connect, Authentik manual/managed setup, PAT, client, Proof Key for Code Exchange, device approval, session revocation, provider credential, service connection, Nuvio pairing/recovery, backup, restore, upgrade, rollback, and disaster recovery.

Each package plan states one prepared-machine command, prerequisites, fixture, expected pass line, expected failure line, receipt, cleanup, and recovery. Rerun `/devex-review` after implementation. Measure both a synthetic repeatable journey and a first-time maintainer walkthrough. Fasti adds no runtime telemetry for this measurement.

Deferred developer-experience work must be unnecessary for security, recovery, accessibility, or the full MVP. Do not add a hosted playground, custom developer portal, new SDK language, or second launcher.

## 22. Validation and exact-head evidence

Run the canonical `cargo xtask test pr` gate from `AGENTS.md`, including the documented Linux `PKG_CONFIG` rule where required. Run all applicable formatting, clippy, tests, locked builds, contract verification, UI formatting/lint/type/test, audit, TrailBase integration, migrations, restart, concurrency, virtual authenticator, OIDC/OAuth negative, Authentik conformance, native/OCI offline, Tauri, backup/restore, package smoke, secret scan, dependency audit, security workflow, `/review`, `/cso`, `/qa`, `/design-review`, `/impeccable polish`, `/devex-review`, and `/ship`.

Add one proposed `./scripts/dev.sh --prepare-offline` mode. Reuse the launcher's existing update/fetch functions and the existing `cargo xtask evidence` schema and verifier. It fetches and verifies both locked Rust graphs, the frozen pnpm graph, exact TrailBase native artifacts and OCI images for the supported architecture, package test fixtures, and the pinned Authentik image only when the high-resource profile is selected. It writes the existing digest-bound evidence manifest below `target/fasti-receipts/`. It does not pull Git state or fetch floating tags.

After preparation, run `cargo xtask test milestone --body <BODY> --manifest <cache-manifest>`, `cargo xtask test pr`, and applicable `cargo xtask test deep` gates with the network denied by their existing harness boundary. A missing artifact produces an unavailable result and the exact preparation action. Ordinary PR CI uses hermetic provider and service fixtures; live credentialed providers and a real Nuvio event are opt-in evidence.

Every CI failure is reproducible from the exact receipt command. Keep the documented Linuxbrew `PKG_CONFIG=/usr/bin/pkg-config` rule, both locked Cargo fetches, and frozen pnpm install in the preparation contract. Record host, architecture, toolchain, cache manifest, command, exit status, logs, receipt, and cleanup. Do not make a developer infer a hidden CI environment variable.

Do not mark an unavailable tool as passed. Do not invent scan IDs, screenshots, accessibility evidence, package evidence, performance evidence, or test results.

## 23. Completion and escalation

MVP is complete only when PRs A-H are implemented, truthful, documented, reviewed, exact-head tested, merged to `dev`, and verified on `dev`. A capability may be inactive while its PR is unmerged. It cannot be silently deferred beyond MVP.

Do not return for another premise gate. Return only when:

- a primary source contradicts the architecture;
- a selected dependency cannot safely provide its capability;
- a licence blocks the design;
- required upstream Nuvio work cannot be coordinated;
- or an irreversible destructive decision needs approval.

Controlled risks:

- TrailBase v0.33.5 does not round-trip caller state, and current evidence does not prove a public verification-key or account-status API. C1 remains blocked until the browser-binding flow and `C1-TB-TRUST` pass exact public-source and real-browser proof; otherwise return under the primary-source-conflict rule.
- `oauth-as 0.9.3` is only a candidate. PR E cannot start until E0 approves it or returns for a dependency decision.
- Authentik 2026.8.0 is only a candidate until socket/resource and multi-architecture risks pass.
- RAWG and ComicVine remain visible and unavailable until safe documented secret transport exists.
- Full Nuvio compatibility requires upstream NuvioTV provider and sync contracts.

## GSTACK REVIEW REPORT

This section remains last. It records the completed sequential CEO, design, engineering, developer-experience, CSO, and Ponytail reviews against this canonical plan.

| Review               | Runs | Status          |      Findings |
| -------------------- | ---: | --------------- | ------------: |
| CEO                  |    2 | `CLEAR`         |  6 reconciled |
| Design               |    3 | `CLEAR` — 10/10 | 12 reconciled |
| Engineering          |    2 | `CLEAR` — 9/10  | 10 reconciled |
| Developer experience |    2 | `CLEAR` — 8/10  |  8 reconciled |
| CSO                  |    2 | `CLEAR` — 9/10  |  7 reconciled |
| Ponytail             |    2 | `CLEAR`         |  7 reconciled |

Branch-bound implementation ledgers: `tasks-ceo-review-20260829-183940.jsonl`, `tasks-design-review-20260829-183940.jsonl`, `tasks-eng-review-20260829-183940.jsonl`, and `tasks-devex-review-20260829-183940.jsonl` under the Fasti gstack project directory. Engineering acceptance artifact: `ryan-pr93-live-eng-review-test-plan-20260829-183940.md`. Review records: `pr93-live-reviews.jsonl`.

VERDICT: `PLAN AND GATE 10 APPROVED`

UNRESOLVED IMPLEMENTATION GATES: C1 remains fail closed until `C1-TB-TRUST` and protocol-specific browser binding pass exact public-source and real-browser proof. C3 requires `C3-CRYPTO`. E2 requires E0 and E-HOST. E3/E4 require the exact Authentik tuple, tag-bound generated API, and section 12 conformance. These are package evidence gates; they do not reopen TrailBase selection or the approved final plan.
