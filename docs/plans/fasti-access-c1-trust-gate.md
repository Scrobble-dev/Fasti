# Fasti Access C1 implementation gate

Status: `BLOCKED_PRIMARY_SOURCE_CONFLICT`

Recorded: 2026-08-30

Base commit: `4546459105c8c762886b32cdbd580be3e039736c`

Base tree: `6ccfa5d96064b51f3dcd80dfb95f00cd60ce5a55`

Owner: Commander / Mothership

## 1. Result

C1 production implementation must not start.

TrailBase remains the selected human account platform. This gate does not
reopen framework selection. The blocker is narrower: TrailBase `v0.33.5` does
not provide the public trust and account-state surfaces required by the
approved `C1-TB-TRUST` contract.

The approved contract requires all of these before C1 code:

- a documented public method to establish the verification key;
- a documented public key-rotation, overlap, and retirement method;
- a documented public account-state check;
- an owner-authorized `TrailBaseInstanceId` and activation generation;
- proof-key version and fingerprint provenance;
- restore-generation and former-deployment fencing evidence.

Exact tagged source proves that the first three requirements are absent. The
commander prompt names a primary-source contradiction and an unavailable
required dependency capability as stop conditions.

## 2. Exact TrailBase evidence

Authority: TrailBase `v0.33.5`, tag commit
`b4c85d5152d4e5f472e0b5da5303f7c938e3a083`.

| Required proof | Exact source result | Gate state |
| --- | --- | --- |
| Public verification-key discovery | The public OpenAPI has no JSON Web Key Set, public-key, user-introspection, or session-list route. The only key route is admin `GET /api/_admin/public_key`. | `BLOCKED` |
| Public-key authorization | The admin key route requires current administrator authentication, a database recheck, and matching CSRF proof. It is not the approved public trust bootstrap. | `BLOCKED` |
| Key rotation | One Ed25519 PEM pair is loaded or generated from the TrailBase data root when files are missing. The token header has no key ID. No supported overlap, version, rotation, or retirement API exists. | `BLOCKED` |
| Token provenance | Claims include subject, issue and expiry times, token type, administrator flag, MFA enrollment flag, provider, email, username, and CSRF value. They do not include issuer, audience, token ID, not-before time, or key ID. | `BLOCKED` for the approved hermetic validation contract |
| Account state | `GET /api/auth/v1/status` can recheck the current user and email-verification state only when the caller presents a refresh token. Without a refresh token it re-encodes the valid access token and cannot check session liveness. There is no arbitrary public account lifecycle lookup or disabled/suspended field. | `BLOCKED` |
| Direct code exchange | The token exchange reloads the user and rejects an unverified email before it returns tokens. This is a source-authenticated exists-and-verified snapshot, not the required lifecycle or key-rotation API. | `PARTIAL`; insufficient for `C1-TB-TRUST` |
| Authorization-code replay | The token endpoint reads a valid authorization code and challenge but does not consume the code. The same code and verifier can create more refresh sessions until the five-minute expiry. | `LIMIT`; Fasti would still require its own atomic one-use ceremony |
| Password plus TOTP callback | The initial password step redirects to MFA with only the MFA token. The built-in MFA page does not preserve the original redirect URI, response type, or Proof Key for Code Exchange challenge. | `BLOCKED` for the approved built-in password-plus-TOTP callback flow |

Primary source:

- [admin public-key route](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/admin/jwt.rs)
- [admin access enforcement](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/server/mod.rs)
- [JWT claims and key loading](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/jwt.rs)
- [authentication status](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/status.rs)
- [token exchange](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/token.rs)
- [token creation and refresh checks](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/tokens.rs)

Context7 was used for discovery. Its TrailBase material tracks `main` and does
not establish these `v0.33.5` capabilities. Exact tagged source controls.

### 2.1 Upstream revalidation

Rechecked: 2026-08-30

- GitHub still reports `v0.33.5` as the latest non-draft, non-prerelease
  TrailBase release.
- `refs/tags/v0.33.5` still resolves to
  `b4c85d5152d4e5f472e0b5da5303f7c938e3a083`.
- `refs/heads/main` resolves to
  `e8fd53a798ada706cf95ebfba47b6220f2fc7a5f`, dated 2026-08-27. Targeted
  source searches on that revision find only the existing administrator
  `public_key` owner and no JSON Web Key Set, introspection, key-rotation, or
  disabled-user implementation.
- Targeted upstream issue and pull-request searches found no published work
  item for JSON Web Key Set, key rotation, account disablement, or token
  introspection.

This check does not replace exact tagged-source review when a new release
appears. A future continuation must compare the new tag, public OpenAPI, and
source against every `C1-TB-TRUST` requirement before it changes this gate.

## 3. Browser ceremony evidence

TrailBase can carry the redirect URI, response type, and Proof Key for Code
Exchange challenge from its login form. External social authentication uses a
TrailBase-owned, signed, five-minute `oauth_state` cookie. A successful Proof
Key for Code Exchange response redirects with `code` only. TrailBase does not
round-trip a Fasti state value. Its callback builder also assumes the Fasti
callback URI has no existing query or fragment.

A future C1 implementation must therefore reuse the approved Fasti design:

1. Fasti creates one high-entropy browser binding and stores only its digest.
2. Fasti stores the Proof Key for Code Exchange verifier on the server.
3. The browser receives one host-only, Secure, HttpOnly, narrow-path,
   one-use pre-authentication cookie.
4. The callback succeeds without the Strict Fasti session cookie only after
   the browser binding and durable ceremony are atomically consumed.
5. Fasti exchanges the code directly with the exact configured TrailBase
   instance.
6. Fasti revokes the returned TrailBase refresh session before it creates a
   Fasti session.
7. A retry returns the stored terminal result. It does not repeat the remote
   exchange.

This browser design remains `PROPOSED`. It still requires the approved real
browser negative-control suite. Browser proof cannot repair the missing
`C1-TB-TRUST` dependency.

## 4. Existing owners that C1 must reuse

DRY and Domain-Driven Design are mandatory. C1 must extend these owners and
must not create parallel models.

| Bounded context | Existing owner | C1 rule |
| --- | --- | --- |
| Human application subject | `crates/fasti-domain/src/access.rs` | Reuse `AuthSubject` and its lifecycle and epochs. |
| Browser session | `crates/fasti-domain/src/access.rs` and `crates/fasti-application/src/browser_auth.rs` | Reuse `FastiBrowserSession`, `BrowserSessionId`, `SessionPolicy`, request-boundary policy, commands, and `BrowserSessionPort`. |
| Session persistence | `crates/fasti-store/src/browser_auth.rs` | Reuse digest-only secrets, CSRF proof, grant checks, expiry, rotation, and bounded last-seen writes. |
| Local operator authority | `crates/fasti-store/src/access.rs` and `crates/fasti-store/src/kernel.rs` | Reuse the owner-only descriptor-root and exclusive data-root lock. Do not reopen first-client bootstrap. |
| Capability truth | `contracts/registry/v1/capabilities.yaml` | Activate only the existing C1 browser-session capability IDs after the runtime is real. |
| TrailBase runtime | `third_party/trailbase/release.json`, `scripts/dev.sh`, and `scripts/trailbase_runtime.py` | Reuse the exact pinned service and sole launcher. Do not add another supervisor. |
| Permanent Account and security UI | `packages/ui/src/runtime-settings-view.svelte` | A remains the permanent destination. |
| First-run Access UI | `packages/ui/src/runtime-settings-view.svelte` | C remains a separate resumable journey derived from confirmed capability state. |
| Shared detail evidence | Existing status, problem, modal, and focus helpers | B is a pattern inside A and C. It is not a third destination. |

The first administrator operation remains the distinct one-use
`access.identity.bootstrap` capability. A trusted CLI or packaged host must
prove possession of `<data_root>/bootstrap.secret`, correct descriptor-root
ownership and permissions, and the exclusive data-root lock. Loopback access
alone is never authority. One transaction creates one anchor, active
membership, and administrator role. A concurrent loser has no side effects.

## 5. C1 package boundary

C1 owns only:

- TrailBase instance and activation-generation provenance;
- a stable external TrailBase anchor for one `AuthSubject`;
- membership lifecycle, roles, and administrator continuity;
- one-use identity bootstrap;
- `AuthCeremony` and browser binding;
- authentication provenance and recent-authentication state;
- server-side code exchange and refresh-session cleanup;
- production Fasti browser-session issuance and inventory;
- global sign-out and authentication-epoch invalidation;
- restore-generation and clone fencing;
- typed problems, audit, contracts, generated SDK, operator status, and A+C
  UI for those capabilities.

C1 does not own:

- personal access tokens, OAuth clients, device grants, consent, or scopes
  beyond the existing browser-session profile-grant boundary; those are C2;
- the credential vault or encrypted operator backup; those are C3;
- passkeys or recovery codes; those are D;
- generic OpenID Connect or Authentik; those are E;
- Metadata M2, provider credentials, Connections, or Nuvio state.

## 6. Dependency and file-ownership gate

The active Metadata M2 worktree is
`/home/ryan/code/fasti-nuvio-metadata-programme-m2`. It has an uncommitted
forward migration and owns the shared schema, capability registry, generators,
API composition, SDK, host composition, and Workbench surfaces that C1 needs.

C1 must not reserve migration version 12. Exact `dev` is schema version 11.
Metadata M2 currently implements migration version 12. If M2 merges first,
C1 uses the next version after the merged schema. If M2 does not merge, C1
freezes its migration only after M2 is explicitly stopped or handed off.

No C1 implementation writer may edit shared files until both conditions hold:

- Metadata M2 is merged and C1 is rebased on the new `dev`, or Metadata M2 is
  explicitly stopped and handed off;
- `C1-TB-TRUST` changes from `BLOCKED` to `VERIFIED` through supported pinned
  primary-source evidence.

## 7. Session policy gate

The approved domain model deliberately has no default. Fixture values are not
production values.

The C1 implementation contract must freeze the following before route mount:

| Value | Proposed product value | Evidence and remaining decision |
| --- | --- | --- |
| Browser idle timeout | 30 minutes | Upper end of the OWASP low-risk range. Must remain server-enforced. |
| Browser absolute lifetime | 8 hours | Upper end of OWASP's office-day example and below the NIST AAL2 24-hour recommendation. |
| Remembered browser lifetime | 30 days | Product choice. It changes the absolute bound only; the idle timeout still applies. |
| Last-seen write interval | 60 seconds | Product choice that bounds write amplification without client-side authority. |
| Recent-authentication window | 10 minutes | Product choice for sensitive Access changes. It never derives from factor enrollment alone. |

Sources:

- [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
- [NIST SP 800-63B reauthentication guidance](https://pages.nist.gov/800-63-4/sp800-63b/aal/)

Minimums, maximums, configuration ownership, exact configuration keys, and
change effects remain `UNRESOLVED`. This document does not invent those keys.
The gate must freeze them against the post-M2 configuration owner before code.
More restrictive policy takes effect on the next authentication check. A less
restrictive policy does not extend an existing credential without successful
reauthentication and rotation.

## 8. Planned implementation sequence after the blocker clears

1. Rebase an isolated C1 worktree on the current exact `dev` after Metadata M2
   disposition.
2. Update this plan with the supported trust API, exact key and lifecycle
   behavior, frozen policies, capability IDs, route contracts, migration
   number, rollback, and file ownership.
3. Add one forward migration for instance provenance, external anchors,
   memberships and roles, identity-bootstrap state, ceremonies, recent-auth
   provenance, and required invalidation state. Reuse PR A session tables.
4. Add domain transitions and application commands before adapters.
5. Add one TrailBase adapter through documented public APIs only.
6. Implement transactional first-administrator bootstrap, anchor collision and
   race handling, membership authorization, cleanup-before-session issuance,
   epoch invalidation, and clone fencing.
7. Mount the existing C1 capability IDs and add only the new identity and
   membership capabilities that the frozen contract requires.
8. Generate OpenAPI, JSON Schema, OKF, problem, and SDK surfaces. Record
   AsyncAPI and JSON-LD as not applicable for private finite authentication
   state unless a real external asynchronous surface exists.
9. Implement Tabler-first A and C states from one application projection. Keep
   downstream C2, D, and E controls visible and truthfully unavailable.
10. Extend the existing milestone runner with body C1 and the existing receipt
    format. Do not create a second test command tree.

## 9. Mandatory verification after the blocker clears

- exact public trust, account-state, key-rotation, restore-generation, and
  clone-fencing proof;
- first-administrator one-winner race and loopback-without-data-root denial;
- membership invitation, approval, acceptance, suspension, removal,
  unaffiliated denial, and final-administrator continuity;
- subject and external-anchor collision and no email auto-link;
- ceremony expiry, one-use consumption, replay, two-tab race, hostile return
  target, wrong origin/path, sibling-cookie injection, and retry;
- real browser callback with the Strict Fasti cookie absent and the valid
  narrow pre-authentication cookie present;
- refresh cleanup failure creates no Fasti session;
- existing valid Fasti sessions survive a TrailBase outage until local expiry
  or revocation, while new authentication and recent-auth fail closed;
- session fixation, rotation, idle and absolute expiry, policy change, CSRF,
  Origin, Host, grant, profile, auth-epoch, and authorization-epoch checks;
- restart, migration failure and retry, rollback, restore, and concurrent
  revocation;
- generated-contract mutation, SDK no-secret behavior, and zero drift;
- Tabler-first A+C UI with keyboard, focus, Axe, reflow, zoom, forced colors,
  reduced motion, WCAG 2.2 AA, EN 301 549, AskTog, Gestalt, Nielsen, IxDF, and
  ADHD/AuDHD scannability evidence;
- focused Rust and JavaScript tests, `cargo xtask test milestone --body C1`,
  canonical PR gate, security review, Ponytail review, exact-head evidence,
  merged-`dev` verification, and context save.

## 10. Safe next action

Obtain one supported pinned TrailBase capability that satisfies the approved
trust boundary. Acceptable evidence is an official documented public API and
source-backed lifecycle for verification-key establishment, key rotation, and
account-state checks. An upstream TrailBase release may add it.

Operator-provisioned private-key reads, direct depot access, undocumented
internal endpoints, a custom Fasti authentication platform, another identity
framework, or trust on first network use are not approved substitutes.

After supported evidence exists, refresh Metadata M2 state, rebase on current
`dev`, update this gate, and start the smallest complete C1 slice.
