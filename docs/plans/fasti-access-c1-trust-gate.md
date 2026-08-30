# Fasti Access C1 implementation gate

Status: `C1_0_COMPLETE_C1_1_IN_PROGRESS`

Recorded: 2026-08-30

Planning base commit: `4546459105c8c762886b32cdbd580be3e039736c`

Planning base tree: `6ccfa5d96064b51f3dcd80dfb95f00cd60ce5a55`

Implementation base commit: `45fb31d906bec3f3ad2e5bf95edf2938c7c942b8`

Implementation base tree: `8263411e2ba193c4b88acdcafd89f0c48905b004`

Owner: Commander / Mothership

## 1. Result

The Fasti Access programme is active. TrailBase remains the selected private,
local human account platform. Framework selection, Gate 10, and the complete
MVP scope remain closed.

C1 uses the approved direct backchannel trust profile:

1. Fasti creates and owns a one-use browser authentication ceremony.
2. TrailBase authenticates the person.
3. Fasti exchanges the returned code directly with the exact supervised
   TrailBase origin.
4. Fasti calls TrailBase status with both returned tokens to recheck the
   current subject, refresh session, and locally accepted account-email state.
5. Fasti resolves the confirmed TrailBase instance and subject to one stable
   `AuthSubject`.
6. Fasti checks local subject state, workspace membership, role, profile grant,
   authentication epoch, authorization epoch, and activation generation.
7. Fasti calls TrailBase logout to revoke that refresh session.
8. Fasti discards every TrailBase token.
9. Fasti creates one opaque Fasti browser session.

Fasti never accepts a TrailBase token supplied by a browser as application
authorization. C1 does not perform offline TrailBase token validation. It does
not need a JSON Web Key Set, signing-key cache, or key-rotation subsystem.

## 2. Approved review decisions

| Decision                    | Approved option | Binding result                                                                                                                 |
| --------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| D1: delivery shape          | A               | Deliver complete C1 in dependency-ordered, reviewable slices. Do not cut approved scope.                                       |
| D2: interrupted exchange    | A               | Fail closed, issue no Fasti session, retain only non-secret needs-attention evidence, and require a fresh sign-in.             |
| D3: TrailBase trust profile | C               | Ship direct-backchannel C1. Keep useful TrailBase hardening as separate governed upstream packages. Do not block C1 on a fork. |

Gate 10 remains:

- A is the permanent Account and security destination.
- C is a separate, resumable first-run journey.
- B is the shared evidence and detail pattern, not another destination.

### 2.1 Frozen C1.1 domain decisions

The dependency-ordered engineering review freezes these values before schema
or route work:

- `TrailBaseInstanceId` is the executable `tbi_` identifier. A TrailBase
  subject is the exact canonical 16-byte value decoded from the pinned
  release's URL-safe Base64 `sub`; it is not an email or a second Fasti ID.
- A membership has a stable `MembershipId`. `Removed` is terminal and its row
  stays unchanged. Re-invite creates a new `Invited`/`Member` aggregate with a
  new ID, while a partial unique index permits at most one non-removed
  membership for one subject and workspace. Re-invite restores no role,
  approval, profile grant, or access.
- Administrator continuity rejects only a positive-to-zero viable-
  administrator transition. Membership mutations and subject lifecycle
  mutations use the same continuity decision and advance the existing subject
  epochs through one shared owner.
- Subject lifecycle transitions are exact: `active` may become `disabled`,
  `recovery_pending`, or `deleted`; `disabled` may become `active`,
  `recovery_pending`, or `deleted`; `recovery_pending` may become `active`,
  `disabled`, or `deleted`; and `deleted` is terminal. Direct deletion is
  required when TrailBase reports deletion without a prior local disablement.
  Every real transition advances the authentication epoch, revokes every Fasti
  browser session, and records the exact resulting lifecycle.
- Administrator membership and subject-lifecycle mutations require a current
  browser session, matching CSRF and request-boundary proof, an unexpired
  recent-authentication record, and transaction-local administrator authority.
  The administrator command can disable, begin recovery, or reactivate; it
  cannot set `deleted`. Only the C1.2 TrailBase status/deletion evidence owner
  may apply that terminal state.
  Invitation acceptance does not reuse that administrator command: C1.2 binds
  it to the proved TrailBase subject and claimed one-use ceremony before any
  ordinary Fasti browser session exists.
- TrailBase activation is `inactive`, `active`, or `blocked`. Blockers are
  `release_mismatch`, `physical_root_identity_mismatch`, and
  `declared_restore`. Initial verification changes generation `0` to `1`.
  Active-to-blocked increments once per blocking episode. Repeated observation
  is idempotent. C1 can clear only `release_mismatch` after exact release and
  root re-verification. A root mismatch or declared restore may replace
  `release_mismatch`, but a non-recoverable blocker is never downgraded. Root
  mismatch and declared restore remain blocked until C3. Generation overflow
  fails closed. Process outage is not persisted as a blocker. Focused tests
  cover initial activation, idempotence, blocker precedence, repair, proof
  invalidation, outage, and overflow.
- `OperationId` owns the ceremony identity. Purposes are `sign_in`,
  `recent_authentication`, and `first_administrator_bootstrap`. Return targets
  are `application_home`, `account_security`, and `first_run`, permitted only
  in that order as fixed one-to-one pairs. No arbitrary path, URL, query,
  fragment, encoded target, or wizard step is stored.
- C1 authentication methods are only `trailbase_password` and
  `trailbase_social`; both are single factor. Storage rejects
  `trailbase_password_totp`. The exact typed unavailable reason is
  `trailbase_password_totp_continuity_unavailable`.
- Access audit evidence is retained for 90 days with a global 10,000-row hard
  ceiling per data root. Each insert atomically prunes older rows, inserts the
  event, then prunes deterministic overflow by `(occurred_at, audit_event_id)`.
  No timer, configuration key, archive stream, per-workspace quota, secret, or
  vendor token is added.
- Terminal authentication ceremonies are replay tombstones retained for
  exactly 24 hours after `terminal_at`, with a 10,000-row hard ceiling per data
  root. Startup atomically converts pending rows to
  `failed/verifier_lost_on_restart` and claimed rows to
  `cleanup_uncertain/exchange_outcome_uncertain`, then prunes terminal rows at
  the exact boundary. Normal ceremony mutations expire pending rows, never
  sweep claimed rows by expiry, and prune terminal rows. A new start rejects
  with `capacity_exceeded` before a row, verifier, cookie, or redirect when the
  ceiling remains full. No timer, background worker, or configuration key is
  added; terminal audit evidence follows the separate 90-day rule.
- Active cancellation has its own terminal `cancelled` state. A pending
  cancellation wins durably before its process-memory verifier is removed;
  claimed ceremonies cannot be cancelled because exchange outcome may already
  be uncertain. Cancellation is never represented as expiry, failure, or row
  deletion.

## 3. Exact TrailBase evidence

Authority: TrailBase `v0.33.5`, tag commit
`b4c85d5152d4e5f472e0b5da5303f7c938e3a083`.

| Surface                           | Exact tagged-source result                                                                                                                                                                                              | C1 disposition                                                                                                                                                                                            |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Release identity                  | Fasti pins the version, tag commit, native digests, OCI digests, licence digest, and executable version.                                                                                                                | Reuse the existing release lock and sole launcher.                                                                                                                                                        |
| Authorization-code exchange       | `/api/auth/v1/token` requires the code and matching Proof Key for Code Exchange verifier, checks expiry, reloads the current user, and requires TrailBase-local accepted account-email state.                           | Call only from the server-held ceremony. Never accept browser tokens.                                                                                                                                     |
| Current status                    | `/api/auth/v1/status` with both the access token and `Refresh-Token` checks the access proof, refresh-session row, expiry, current user, and TrailBase-local accepted account-email state, then returns current tokens. | Require all response fields. Treat a 200 response with null fields as failure. Do not describe this as independent mailbox-ownership proof.                                                               |
| Refresh-session cleanup           | POST `/api/auth/v1/logout` deletes the named refresh session and returns 200 even when it is already absent.                                                                                                            | Require 200 before Fasti session creation. A timeout, redirect, or other response fails closed.                                                                                                           |
| Subject                           | The current subject is in the status-returned authentication token payload.                                                                                                                                             | Decode only the bounded status-returned payload to read the current subject and authentication metadata. The trusted fact is the direct response from the pinned process, not an offline signature check. |
| Password plus TOTP                | The password-to-MFA transition loses the original redirect, response type, Proof Key for Code Exchange challenge, and method in the exact pinned release.                                                               | Keep password-plus-TOTP authorization-code sign-in unavailable in C1 until an official source-backed TrailBase release preserves and verifies the complete ceremony.                                      |
| Social sign-in with enrolled TOTP | Social callbacks do not prove a TrailBase TOTP challenge occurred.                                                                                                                                                      | Never describe social sign-in as TOTP-verified. Do not use enrollment as proof of recent multi-factor authentication.                                                                                     |
| Authorization-code consumption    | TrailBase reads but does not delete the authorization code. The same code and verifier can mint more refresh sessions until expiry.                                                                                     | Fasti atomically consumes its own ceremony before exchange. This contains replay through Fasti but does not claim the upstream code is globally single use.                                               |
| Account state                     | Status proves the current user still exists with a non-null email in TrailBase's locally accepted account state. TrailBase has no disabled or suspended account field.                                                  | Fasti enforces its own `AuthSubject` and membership lifecycle. Do not claim independent mailbox ownership or TrailBase account suspension support.                                                        |
| Refresh rotation                  | TrailBase does not rotate refresh tokens.                                                                                                                                                                               | C1 revokes and discards the refresh token during sign-in. Fasti never uses it as a durable application credential.                                                                                        |
| Token claims and keys             | Tokens have no issuer, audience, key identifier, token identifier, or not-before claim. There is no supported key overlap or retirement API.                                                                            | These facts block offline TrailBase token acceptance. C1 does not accept tokens offline, so they do not block the direct backchannel.                                                                     |

Primary source:

- [login and MFA flow](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/login.rs)
- [authorization-code exchange](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/token.rs)
- [authentication status](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/status.rs)
- [refresh-session logout](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/logout.rs)
- [token and refresh checks](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/tokens.rs)
- [token payload and key loading](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/jwt.rs)
- [built-in authentication UI](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/auth-ui/src/lib.rs)
- [tagged OSL-3.0 licence](https://github.com/trailbaseio/trailbase/blob/v0.33.5/LICENSE)

The existing written licence review approves only the exact unmodified
TrailBase release as a separate process. Any maintained patch or fork needs a
new written review before source modification or distribution.

## 4. Trust and data flow

```text
Browser
  |
  | start sign-in
  v
Fasti AuthCeremony start
  |-- random PKCE verifier in bounded zeroizing process memory
  |-- digest-only browser binding
  |-- exact TrailBaseInstanceId + activation generation
  |-- exact callback path + allowlisted return target
  |-- expiry + one active claimant
  |
  | redirect with PKCE challenge
  v
Pinned, supervised TrailBase v0.33.5
  |
  | password or social proof; TOTP assurance unavailable
  | short-lived authorization code
  v
Fasti callback
  |-- atomically claim the ceremony before network exchange
  |-- POST /token to the fixed numeric-loopback origin
  |-- GET /status with returned access + refresh tokens
  |-- require a current accepted non-anonymous subject
  |-- resolve (TrailBaseInstanceId, subject) -> AuthSubject
  |-- check activation, subject, membership, role, grant, and epochs
  |-- POST /logout with the refresh token
  |-- require exact success; discard every TrailBase token
  v
Existing Fasti browser-session owner
  |-- random opaque session and CSRF secrets
  |-- digest-only persistence
  |-- Secure, HttpOnly, SameSite cookie
  v
Authenticated Fasti application access
```

Inline code comments should contain this diagram only at the multi-step callback
orchestration owner. Domain aggregates should document their transition table,
not repeat the network flow.

## 5. Required trust controls

### 5.1 Ceremony

The ceremony is durable and one use. Store:

- ceremony identifier;
- purpose and protocol;
- TrailBase instance identifier;
- activation generation;
- digest of the browser binding;
- exact callback path;
- allowlisted return target;
- created and expiry times;
- claim, terminal, and failure state;
- non-secret correlation and audit data.

Keep the Proof Key for Code Exchange verifier only in bounded zeroizing process
memory, keyed by the ceremony identifier. Allow at most 64 live entries per
process. Capacity exhaustion returns the existing typed `capacity_exceeded`
safe state before a row, cookie, or redirect exists. Never persist the verifier.
Never put it, a TrailBase token, subject, workspace, grant, or return URL in
browser-controlled state. Before callback traffic after process restart, pending rows become failed
because their verifier is lost and claimed rows become cleanup-uncertain because
exchange outcome is unknowable. Neither is retried. C resumes at its last
confirmed step and requires fresh sign-in. The browser receives only one
host-only, Secure, HttpOnly, SameSite, narrow-path binding cookie.

The callback path has no query or fragment before TrailBase appends the code.
The callback atomically claims the ceremony before the remote exchange. A
replay, second tab, or concurrent callback cannot perform a second exchange.

### 5.2 Backchannel transport

Use one concrete TrailBase client. Do not add a generic identity-provider
framework.

Keep the client private in `fasti-api`, beside the shared Access callback and
router facade used by daemon and packaged-host composition. Reuse only the
neutral `pinned_client` and `bounded_body` mechanics from
`fasti-provider-runtime`. Do not move TrailBase identity semantics into the
metadata-provider runtime or SQLite store, and do not add a one-client runtime
crate.

The client must use:

- the exact supervised numeric-loopback origin;
- fixed code-owned paths;
- proxy-disabled and redirect-disabled HTTP behavior;
- connect and total timeouts;
- bounded request and response bodies;
- bounded concurrency;
- strict content type and response shape checks;
- redacted errors and audit data.

No browser input may select a host, port, scheme, path, header name, or redirect.
The same-user local process boundary remains part of the approved operator
threat model. A future hostile-same-user threat requires an authenticated local
transport decision; it is not repaired by JSON Web Keys.

### 5.3 Status payload

Read subject and authentication metadata only from the token returned by the
successful status response. Apply strict byte and JSON limits. Require:

- authentication token type;
- non-empty canonical subject;
- sensible issue and expiry times;
- a non-null account email in TrailBase's current locally accepted state.

Ignore TrailBase administrator status for Fasti authorization. Treat the TOTP
field as enrollment metadata, not proof that the current social sign-in used
TOTP. Never auto-link by email.

### 5.4 Cleanup and token discard

TrailBase logout must succeed before Fasti creates a browser session. Zeroize
or discard all TrailBase access, refresh, and CSRF values immediately after
cleanup. Never store them in the browser, database, logs, audit rows, error
messages, or generated contracts.

The D2 recovery policy is final:

- interrupted or uncertain exchange fails closed;
- no Fasti session is created;
- the ceremony becomes terminal;
- only non-secret needs-attention evidence remains;
- the person must start a fresh sign-in.

Do not promise transparent retry. A crash after TrailBase token creation and
before logout can leave an unreachable TrailBase refresh-session row until its
normal expiry. C1 does not persist the secret merely to clean that row later.

## 6. Production failure modes

| Failure                                                                                                     | Required behavior                                                                                                                           | User-visible recovery                                                                | Verification                                            |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------- |
| Expired, missing, copied, or wrong browser binding                                                          | Reject before exchange and clear the narrow cookie.                                                                                         | Start sign-in again.                                                                 | Unit, SQLite integration, and browser negative control. |
| Callback replay or two-tab race                                                                             | Exactly one claimant may call TrailBase. Every loser fails closed.                                                                          | Continue the winning tab or start again.                                             | Concurrent SQLite test and browser two-tab test.        |
| TrailBase timeout or malformed response                                                                     | Create no Fasti session. Store no vendor secret.                                                                                            | State that sign-in could not be confirmed and offer retry.                           | Scripted adapter integration test.                      |
| Status returns 200 with null fields                                                                         | Treat as authentication failure.                                                                                                            | Start sign-in again.                                                                 | Adapter negative test.                                  |
| Wrong content type, oversized JSON, malformed token, wrong token type, missing subject, or implausible time | Fail closed with a typed redacted problem.                                                                                                  | Start sign-in again; operator detail contains no secret.                             | Adapter mutation tests.                                 |
| Logout timeout, redirect, or non-200 response                                                               | Create no Fasti session. Mark cleanup uncertain.                                                                                            | Start sign-in again.                                                                 | Orchestration integration test.                         |
| Process crash after claim or exchange                                                                       | Keep no recoverable token. On restart mark the claimed ceremony cleanup-uncertain and create no Fasti session.                              | Start sign-in again.                                                                 | Crash-window state-machine test and restart test.       |
| Process restart while ceremony is pending                                                                   | Lose the process-memory verifier, mark the pending attempt failed without a remote call, and keep confirmed first-run progress.             | Resume first run and start sign-in again.                                            | Restart and no-exchange negative control.               |
| Fasti session insert succeeds but response is lost                                                          | Do not claim the digest-only secret can be replayed. The next attempt uses a new ceremony.                                                  | Start sign-in again.                                                                 | Response-loss integration test.                         |
| TrailBase outage after a Fasti session exists                                                               | Existing Fasti sessions remain local until their own expiry or revocation. New sign-in and recent authentication fail closed.               | Existing access continues; new proof states TrailBase is unavailable.                | Restart/outage end-to-end test.                         |
| TrailBase restore or activation changes                                                                     | Reject old ceremonies and proof tuples. Reject identity mismatches and every declared restore; C1 does not claim universal clone detection. | Keep Access unavailable until the C3 authenticated restore/source-fence flow exists. | File-copy, declared-restore, and clone-limit controls.  |
| Sign-in for an account enrolled in TOTP                                                                     | Never report TOTP as satisfied from the v0.33.5 enrollment claim.                                                                           | Do not issue higher-assurance proof; state the exact unavailable reason.             | Real browser and provenance tests.                      |

No failure in this table may be silent. Each has a typed safe problem and exact
recovery action. Durable Access audit evidence begins only after a valid
server-created ceremony passes browser-binding lookup; invalid callback noise
is not persisted.

## 7. Existing owners and minimum additions

DRY and Domain-Driven Design are mandatory. Extend existing semantic owners.
Do not create parallel session, grant, workspace, profile, or runtime models.

| Bounded context           | Existing owner to reuse                                                                                           | Minimum C1 addition                                                                                                                                             |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Human application subject | `crates/fasti-domain/src/access.rs`                                                                               | Extend subject lifecycle tests and add stable external anchor behavior without email linking.                                                                   |
| Browser session           | `crates/fasti-domain/src/access.rs`; `crates/fasti-application/src/browser_auth.rs`                               | Reuse `FastiBrowserSession`, `SessionPolicy`, request boundary, commands, and `BrowserSessionPort`. Do not add a second session aggregate.                      |
| Session persistence       | `crates/fasti-store/src/browser_auth.rs`                                                                          | Reuse digest-only secrets, grant checks, expiry, rotation, inventory, and revocation. Replace the administrator-count placeholder through the membership owner. |
| Local operator authority  | `crates/fasti-store/src/access.rs`; `crates/fasti-store/src/kernel.rs`; local API                                 | Reuse `bootstrap.secret`, descriptor-root checks, data-root identity, and exclusive lock. Human first-administrator bootstrap remains a distinct operation.     |
| TrailBase runtime         | `third_party/trailbase/release.json`; `scripts/trailbase_runtime.py`; `scripts/dev.sh`                            | Reuse the release lock and sole launcher. Add no process or supervisor.                                                                                         |
| HTTP hardening            | Private `fasti-api` TrailBase client using existing proxy-free, redirect-free, address-pinned transport mechanics | Reuse only neutral mechanics. Do not make Access depend on provider policy types or expose the concrete client as a public framework.                           |
| Permanent A UI            | `packages/ui/src/runtime-settings-view.svelte`                                                                    | Drive active Account and security state from one generated Access projection.                                                                                   |
| Resumable C UI            | Same route owner and application projection                                                                       | Derive the next safe task from confirmed server state. Store no parallel wizard authority.                                                                      |
| B detail pattern          | Existing status, problem, modal, disclosure, and focus helpers                                                    | Reuse inside A and C. Do not mount a third destination.                                                                                                         |

Minimum new domain owners:

- TrailBase installation and activation;
- stable TrailBase external anchor;
- workspace membership with one current role;
- `AuthCeremony`;
- authentication provenance and recent-authentication expiry;
- bounded Access audit evidence.

Avoid a role catalogue, key registry, identity-provider factory, recent-auth
service, bootstrap-state table, or recovery-envelope framework until a proven
requirement needs one.

## 8. Persistence and transaction contract

Metadata M2 owns schema migration v13. PR #117 is merged at the exact
implementation base above. Its shared-file handoff releases migration v14 to
C1. Published v12 and final v13 remain immutable.

One forward migration adds only:

- singleton TrailBase installation and activation state;
- unique `(trailbase_instance_id, subject)` anchors;
- workspace memberships with lifecycle and current role;
- durable authentication ceremonies;
- bounded Access audit evidence;
- authentication provenance and recent-authentication expiry on the existing
  session owner where practical.

Archive v4 remains frozen at 29 workspace streams and does not export the
node-local Access tables. When v14 becomes current, restore must continue to
accept a genuine archive-v4/schema-v13 manifest only with the exact frozen
schema digest
`sha256:e470f2e8ae2972aa05fecd5b39642b79ef739de89eda204c37bf1d3e48f892c3`.
Archive v3/schema-v12 acceptance and its digest remain unchanged. Restoring a
workspace archive creates no human authority; Access stays unavailable until
the authorized local bootstrap succeeds.

Migration v14 owns exactly these node-local structures:

1. singleton TrailBase installation and activation;
2. immutable TrailBase subject anchors;
3. versioned workspace membership aggregates;
4. durable authentication ceremonies;
5. a one-to-one authentication-provenance companion for the existing browser
   session owner; and
6. append-only bounded Access audit events.

Do not add workspace-revision triggers or archive entities for these tables.

The migration and application invariants require:

- one stable anchor per TrailBase instance and subject;
- no email auto-link;
- one active first-administrator winner;
- workspace-scoped memberships and roles;
- no membership transition creates a profile grant;
- only authorization-viable administrators count for continuity;
- reject only a positive-to-zero viable-administrator transition;
- role or membership access changes advance the authorization epoch;
- subject credential/lifecycle changes advance the authentication epoch;
- C1 rejects detected physical-root identity mismatches and every declared
  restore, invalidating affected sessions, ceremonies, and recent proof. It does
  not claim universal image-clone detection; authenticated copied-root
  activation and external source fencing belong to C3;
- every mutation checks authorization in the same database transaction.

First-administrator bootstrap atomically creates the anchor, `AuthSubject`,
active workspace membership, and administrator role. It requires the correct
bootstrap secret, descriptor-root authority, and exclusive data-root lock.
Loopback access alone is never authority. A concurrent loser has zero side
effects.

## 9. Frozen session policy

Use these product constants in one application owner. Do not create new
configuration keys in C1.

| Value                                | C1 constant | Enforcement                                            |
| ------------------------------------ | ----------- | ------------------------------------------------------ |
| Browser idle timeout                 | 30 minutes  | Server-enforced on every check.                        |
| Browser absolute lifetime            | 8 hours     | Never extended by activity.                            |
| Remembered browser absolute lifetime | 30 days     | Idle timeout still applies.                            |
| Last-seen write interval             | 60 seconds  | Bounds write amplification.                            |
| Recent-authentication window         | 10 minutes  | Derived only from a proven fresh authentication event. |

A more restrictive future policy takes effect on the next authentication
check. A less restrictive policy never extends an existing credential without
successful reauthentication and session rotation.

## 10. Dependency-ordered delivery slices

### C1.0 — Contract correction and handoff

- Apply D1, D2, and D3-C to this plan and the canonical programme.
- Keep the C1 branch local until the false-blocker documents are corrected.
- Wait for Metadata M2 to merge.
- Fetch current `dev`; record exact commit and tree.
- Confirm M2 schema v13, C1 migration v14, and shared-file ownership handoff.

Gate: reviewed plan, clean plan commit, exact M2 handoff, no shared-file overlap.

### C1.1 — Identity and authorization core

- Freeze the reviewed domain vocabulary and tests before writing migration
  v14.
- Add the one forward migration.
- Add activation, anchor, membership/role, ceremony, provenance, recent-auth,
  and audit domain transitions.
- Replace `viable_administrator_count` with the authorization-viable membership
  query.
- Implement first-administrator bootstrap and final-administrator continuity.

Gate: domain and SQLite tests prove every transition, collision, race,
transaction rollback, epoch, and restore invariant.

### C1.2 — TrailBase exchange and session issuance

- Add one concrete TrailBase backchannel client.
- Add the server-held ceremony start and callback orchestration.
- Enforce status recheck, cleanup-before-session, token discard, and D2
  fail-closed recovery.
- Call the existing browser-session owner only after all proof and local
  authorization checks pass.

Gate: scripted positive, mutation, timeout, replay, crash, response-loss, and
outage tests pass. No vendor secret crosses the adapter boundary.

### C1.3 — Authenticated Access operations and contracts

- Mount the existing browser-session inventory, end, revoke, rotate, and
  profile-selection capabilities.
- Add only indispensable identity, membership, bootstrap, activation, and
  recent-authentication operations.
- Apply Origin, Host, CSRF, subject, membership, role, profile grant, and both
  epochs at application and transaction boundaries.
- Generate OpenAPI, JSON Schema, problems, capability registry, OKF, and SDK.
- Record AsyncAPI and JSON-LD as not applicable unless a real asynchronous or
  linked-data surface exists.

Gate: generated drift is zero; secrets do not appear in schemas, examples, SDK
types, logs, or browser responses.

### C1.4 — Gate 10 A+C implementation

- Use Tabler first.
- Drive A and C from one Access application projection.
- Keep B as the evidence/detail pattern.
- Preserve visible C2, D, and E controls with exact unavailable reasons until
  their own packages activate them.
- Apply Kathy Sierra's user-success lens: show the next safe action, reduce
  memory burden, make progress visible, and preserve recovery.

Gate: real browser behavior passes keyboard, focus, screen-reader, 320/375/768/
1440 reflow, 200% zoom, text spacing, forced colors, reduced motion, Axe, WCAG
2.2 Level AA, EN 301 549, AskTog, Gestalt, all Nielsen heuristics, relevant
IxDF guidance, and ADHD/AuDHD scannability review. Run Impeccable and the live
developer-experience review.

### C1.5 — Closure and delivery

- Add `cargo xtask test milestone --body C1` through the existing milestone
  command and receipt format.
- Run focused tests, the C1 milestone, canonical PR gate, security review,
  Ponytail review, QA, design review, Impeccable, developer-experience review,
  exact-head CI, native/OCI evidence, rollback, restart, backup/restore, and
  context save.
- Push one C1 branch and open one draft C1 PR. Do not create duplicates.
- Merge only a green exact head to `dev`. Verify merged `dev` separately.

Gate: exact-head and merged-`dev` evidence prove every C1 requirement. Do not
promote to `release`.

## 11. Worktree parallelization

| Step                            | Modules                            | Depends on                                   |
| ------------------------------- | ---------------------------------- | -------------------------------------------- |
| Domain transitions and tests    | `fasti-domain`                     | M2 handoff for final schema names only       |
| Migration and SQLite adapter    | `fasti-store`                      | M2 merge; frozen domain transitions          |
| Backchannel and orchestration   | Access application/adapter modules | Frozen domain commands and ceremony contract |
| Contracts and generated clients | contracts, generators, SDK         | Migration and operations frozen              |
| A+C UI                          | UI and host                        | Generated Access projection frozen           |
| QA and delivery                 | tests, xtask, docs                 | All implementation lanes merged              |

Execution:

```text
Lane A: domain transitions -> store integration
Lane B: direct TrailBase adapter fixtures -> orchestration tests
                         \   /
                          merge
                            |
Lane C: contracts -> generated SDK -> host composition
                            |
Lane D: A+C UI -> browser QA -> developer-experience review
                            |
Lane E: milestone/security/performance/rollback -> PR -> merged-dev proof
```

Lane A and the fixture-only part of Lane B may run in parallel after the M2
handoff. Shared application, store, registry, generator, host, and UI owners
merge sequentially. No two writers may edit one module at the same time.

## 12. Test coverage plan

Existing baseline at planning head `c3c9d52460a8fe7e075206e3c5681ab6df65558d`:

- 2 `fasti-domain` Access tests pass;
- 2 `fasti-application` browser policy/boundary tests pass;
- 8 SQLite-backed browser-session tests pass;
- 2 Gate 10 A+C unavailable-state Playwright tests pass.

```text
CODE PATHS                                      USER FLOWS
[PARTIAL] AuthSubject + session state           [GAP] First administrator
  |-- active/disabled/deleted + auth epoch         |-- authorized local bootstrap
  |-- [GAP] recovery pending                       |-- wrong secret/no lock denied
  |-- [GAP] authorization epoch                    `-- concurrent one winner
  `-- [GAP] every session terminal state

[GAP] Membership + role lifecycle               [GAP] Sign in with TrailBase
  |-- invite/approve/accept                        |-- start ceremony
  |-- suspend/remove/role change                   |-- password/social proof; TOTP unavailable
  |-- cross-workspace denial                       |-- callback without Fasti cookie
  `-- final viable administrator                   |-- status + cleanup + Fasti session
                                                   `-- fresh-login recovery on failure
[GAP] AuthCeremony
  |-- expiry and browser binding                 [GAP] Manage Account and security
  |-- one claimant/replay/two tabs                 |-- A permanent inventory/actions
  |-- hostile return target                        |-- C resumes next safe setup task
  `-- terminal failure/restart                      |-- B opens shared evidence detail
                                                   `-- operator-only actions authorized
[GAP] Direct TrailBase orchestration
  |-- token -> status -> local authorization     [GAP] Restore/clone boundary
  |-- logout -> discard -> Fasti session           |-- declared restore stays disabled
  |-- malformed/timeout/cleanup failure            |-- identity mismatch fails closed
  `-- crash and response-loss recovery              `-- full source fence belongs to C3
```

Required tests:

1. Extend `crates/fasti-domain/src/access.rs` unit tests for every subject,
   membership, role, ceremony, provenance, and session-state branch.
2. Extend SQLite integration tests for bootstrap authority, one-winner races,
   anchor collision, membership lifecycle, admin continuity, epochs, ceremony
   replay, cleanup order, restart, restore, and activation fencing.
3. Add an application integration test with a scripted identity backend. Prove
   `exchange -> status -> local authorization -> logout -> session`, and prove
   every error creates zero sessions and zero cookies.
4. Add direct TrailBase adapter mutation tests for null, malformed, oversized,
   redirected, slow, wrong-type, missing-subject, and implausible-time responses.
5. Add `tests/e2e/access-c1.spec.ts` for the real callback, missing Strict Fasti
   cookie, valid narrow cookie, copied callback, sibling cookie, wrong path,
   wrong origin, replay, two tabs, lost response, outage, and recovery.
6. Add the A+C fixture matrix for loading, empty, unavailable, working,
   duplicate submit, partial membership, expired/revoked/conflict, resumable C,
   Save and leave, completion, B detail, authorization, focus, and live status.
7. Retain the existing unavailable-state browser tests as regression controls.
8. Add the C1 milestone runner only after M2 hands off `xtask`.

Final commands:

```text
cargo test -p fasti-domain access::tests
cargo test -p fasti-application browser_auth::tests
cargo test -p fasti-application --test c1_auth_flow
cargo test -p fasti-store browser_auth::tests
pnpm exec playwright test tests/e2e/access-c1.spec.ts --project=chrome
cargo xtask test milestone --body C1
PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr
pnpm test:ui
```

## 13. Performance and memory contract

- No new daemon or process.
- One bounded TrailBase client and one exchange per claimed ceremony.
- No retry loop repeats a remote exchange.
- Bound all network bodies, timeouts, concurrency, and JSON decoding.
- Keep TrailBase tokens in short-lived memory only and discard them promptly.
- Keep browser session and ceremony lookups indexed by digests and identifiers.
- Keep the viable-administrator continuity query transaction-local and indexed
  by workspace, lifecycle, and role.
- Preserve the existing 60-second last-seen write bound.
- Bound session, membership, ceremony, and audit inventories. Use stable
  ordering and cursor pagination when the current contract already provides it;
  do not add speculative pagination frameworks.
- Do not add authentication caches. Current subject, membership, grant, client,
  and epoch state must be checked at the application and transaction boundary.
- Run the canonical x86_64 and arm64 resource envelopes on the exact final head.

No material N+1 query, unbounded memory owner, or cache requirement is accepted
in this plan.

## 14. Documentation and contract surfaces

Update in the same implementation programme:

- this C1 gate and the canonical authentication programme;
- `AGENTS.md`, authentication architecture, security guidance, capability
  ledger, operator runbook, and developer loop;
- OpenAPI, JSON Schema, typed problems, registry, OKF, and generated SDK;
- AsyncAPI only if C1 creates a real external asynchronous API;
- JSON-LD only if C1 creates a useful visible linked-data surface;
- exact source, version, licence, artifact, test, environment, limits, rollback,
  and merged-head evidence.

Public and operator copy uses concise active language. Do not expose protocol
abbreviations where the person only needs a task, state, reason, and next
action.

## 15. Separate TrailBase hardening programme

D3-C preserves useful upstream improvements without making them C1 blockers:

1. Atomic authorization-code consumption with concurrent double-exchange proof.
2. Refresh-token rotation and reuse detection with a defined token-family
   response.
3. Explicit account disablement and enforcement.
4. Standards metadata, issuer/audience claims, JSON Web Keys, and key rotation
   only when a proven consumer needs offline validation or provider behavior.

Prefer bounded upstream issues and pull requests. Use collaborative language
and link exact conformance tests. A Fasti-maintained patch or fork requires a
new written OSL-3.0 review, public corresponding source, retained notices,
prominent modification attribution, reproducible builds, pinned artifacts,
rollback, and long-term update ownership before implementation or distribution.

Do not wait for an upstream merge to complete C1 unless a later exact source
finding proves the direct backchannel unsafe.

## 16. NOT in C1 scope

- Personal access tokens, OAuth clients, device grants, consent, and full scope
  issuance: C2 owns them.
- Credential vault and encrypted operator backup: C3 owns them.
- Passkeys and recovery codes: D owns them.
- Generic OpenID Connect and named Authentik integration: E owns them.
- Provider credentials and service connections: F and G own them.
- Full Nuvio pairing and synchronization: H owns them.
- Offline TrailBase token validation, JSON Web Key caching, and key retirement:
  no C1 consumer exists.
- TrailBase fork maintenance: separate upstream hardening, subject to a new
  written licence review.
- Loco or another application framework: Fasti already has its application
  framework; Loco remains a developer-experience reference.
- A compatibility layer for PR #93 authentication: Fasti has not launched and
  the prior simulated behavior has no supported users.

These packages remain required for the complete MVP. They are outside this
C1 slice, not removed from the programme.

## 17. What already exists

- Exact pinned TrailBase native and OCI artifacts, licence review, release
  verifier, launcher, backup/restore, runtime lock, and conformance smoke.
- `AuthSubject`, lifecycle, authentication and authorization epochs.
- `FastiBrowserSession`, policy validation, opaque secrets, digest-only storage,
  CSRF proof, inventory, exact/other/all revocation, rotation, and profile
  selection.
- Transaction-bound workspace, grant, client, and selected-profile checks.
- Owner-only bootstrap secret, descriptor-root authority, data-root identity,
  and exclusive lock.
- Reserved C1 browser-session capability identities.
- Tabler-first A and C shells plus B-compatible modal, problem, disclosure,
  focus, and status patterns.
- Unavailable-state browser regression tests that prevent fake controls from
  returning.

C1 reuses these owners. It does not rebuild them.

## 18. Implementation tasks

- [x] **T1 (P1, human: ~1 day / Codex: ~2 hours)** — Contract — Apply D3-C to the canonical programme and freeze the C1 route, policy, migration, and ownership contract.
  - Surfaced by: architecture review; the former plan incorrectly required offline TrailBase token validation.
  - Files: C1 plan, canonical authentication plan, decision and context records.
  - Verify: document source links, exact hashes, review report, and clean diff.
- [ ] **T2 (P1, human: ~3 days / Codex: ~1 day)** — Access domain and store — Implement the frozen activation, anchors, versioned memberships/roles, ceremonies, provenance, recent proof, audit, first-admin bootstrap, and continuity.
  - Surfaced by: code-quality and test review; the current viable-administrator count is a placeholder and no membership aggregate exists.
  - Files: Access domain/application/store modules and migration v14 after M2.
  - Verify: focused unit and SQLite integration tests, restart, migration, rollback, restore, and race proof.
- [ ] **T3 (P1, human: ~3 days / Codex: ~1 day)** — TrailBase adapter — Implement the fixed-origin exchange, status recheck, cleanup, token discard, and D2 recovery policy.
  - Surfaced by: architecture and security review; no production callback orchestration exists.
  - Files: one concrete Access adapter, orchestration owner, focused fixtures/tests.
  - Verify: scripted positive/negative matrix and no-secret scan.
- [ ] **T4 (P1, human: ~2 days / Codex: ~6 hours)** — Access API and contracts — Mount authorized operations and generate every applicable contract and SDK surface.
  - Surfaced by: code-quality review; C1 capabilities are reserved but not mounted.
  - Files: API, capability registry, generators, SDK, host after M2 handoff.
  - Verify: generated drift, mutation tests, authorization negatives, no-secret schemas.
- [ ] **T5 (P1, human: ~3 days / Codex: ~1 day)** — Gate 10 A+C — Implement permanent A, resumable C, and shared B evidence from one projection.
  - Surfaced by: UI test review; only truthful unavailable shells exist today.
  - Files: existing runtime settings owner, host, types, and `access-c1.spec.ts` after M2 handoff.
  - Verify: full browser, accessibility, cognitive-accessibility, and Impeccable evidence.
- [ ] **T6 (P1, human: ~2 days / Codex: ~6 hours)** — Closure — Add the C1 milestone, run all delivery gates, open one PR, merge the green exact head, and verify merged `dev`.
  - Surfaced by: test and delivery review; the milestone runner has no C1 body yet.
  - Files: existing xtask/test/evidence/documentation owners after M2 handoff.
  - Verify: C1 milestone, canonical PR gate, native/OCI envelopes, reviews, rollback, exact-head and merged-head receipts.

## 19. Safe next action

1. Preserve published migration v12, final migration v13, archive v3, and
   archive v4.
2. Complete the frozen C1.1 domain transitions and focused tests.
3. Implement and test the single append-only v14 Access migration, including
   archive-v4/schema-v13 compatibility.
4. Complete C1.1 persistence work before mounting C1 routes.
5. Keep one writer for shared schema, registry, generator, host, and Workbench
   files.
6. Do not request another premise gate.

## GSTACK REVIEW REPORT

| Review        | Trigger               | Why                             | Runs | Status          | Findings                                                                                                                                              |
| ------------- | --------------------- | ------------------------------- | ---- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| CEO Review    | `/plan-ceo-review`    | Scope & strategy                | 0    | —               | Gate 10 and canonical programme approvals are existing source decisions, not a new run.                                                               |
| Codex Review  | `/codex review`       | Independent 2nd opinion         | 0    | —               | Nested Codex review was not run. Two read-only subagents and optional AGY independently challenged D3-C.                                              |
| Eng Review    | `/plan-eng-review`    | Architecture & tests (required) | 2    | CLEAR           | C1 trust profile plus activation, membership, ceremony, audit-retention, archive-compatibility, and TOTP decisions are frozen.                       |
| Design Review | `/plan-design-review` | UI/UX gaps                      | 0    | —               | Existing approved Gate 10 A+C review and artifact hashes remain binding. Runtime design evidence stays in C1.4.                                       |
| DX Review     | `/plan-devex-review`  | Developer experience gaps       | 0    | PENDING RUNTIME | The live review runs after C1 has an executable path.                                                                                                 |

**CROSS-MODEL:** Two read-only subagents and AGY agree that direct backchannel C1 plus separate upstream hardening is the correct bounded-context design. The strongest shared objection is the upstream authorization-code and crash window, covered by one-use Fasti ceremonies and D2 fail-closed recovery.

**VERDICT:** C1.0 COMPLETE — exact M2 handoff verified; implement C1.1 on v14.

NO UNRESOLVED DECISIONS
