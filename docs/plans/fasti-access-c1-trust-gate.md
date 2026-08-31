# Fasti Access C1 implementation gate

Status: `C1_SOURCE_COMPLETE_DELIVERY_GATES_PENDING_TAURI_DEFERRED`

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

Credential custody is also fixed. Each TrailBase installation has its own
administrator credential, held in an operator-selected password manager or
equivalent private installation record. Each person has a distinct TrailBase
account. Fasti never copies, stores, logs, receipts, or browser-persists the
administrator password. It does not share that password across installations
or add a Fasti secret store for it.

The installation credential is operator-scoped, not a shared human sign-in.
The operator retrieves it by installation ID and Account URL when TrailBase
administration requires it. Each person enters only that person's TrailBase
password in TrailBase's authentication UI. Tests use generated disposable
installation and human credentials and keep their values out of output and
evidence receipts.

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
  `disabled`, or `deleted`; and `deleted` is terminal. Direct deletion remains
  a domain transition for a future explicit, supported deletion-evidence
  owner; it is never inferred from a failed TrailBase exchange or status call.
  Every real transition advances the authentication epoch, revokes every Fasti
  browser session, and records the exact resulting lifecycle.
- Administrator membership and subject-lifecycle mutations require a current
  browser session, matching CSRF and request-boundary proof, an unexpired
  recent-authentication record, and transaction-local administrator authority.
  The administrator command can disable, begin recovery, or reactivate; it
  cannot set `deleted`. C1.2 cannot apply that terminal state because pinned
  TrailBase `v0.33.5` exposes no result that distinguishes deletion from an
  invalid proof, missing refresh session, or outage.
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

### 2.2 Frozen C1.2 implementation decisions

C1.2 is a private exchange and orchestration slice. It does not publish the
browser routes or generated contracts owned by C1.3.

- Add one private `fasti-api` module, `trailbase`, containing the concrete
  TrailBase client, wire DTOs, bounded Proof Key for Code Exchange verifier
  vault, and callback orchestration. Do not add a provider trait, factory,
  identity runtime crate, or second launcher. Reuse only
  `fasti_provider_runtime::pinned_client` and `bounded_body`.
- The production TrailBase backchannel origin is the existing supervised
  `http://127.0.0.1:4000`. Code owns the three paths
  `/api/auth/v1/token`, `/api/auth/v1/status`, and
  `/api/auth/v1/logout`. Only a test-only constructor may inject another
  numeric-loopback address. Browser input cannot select any origin, address,
  scheme, path, or header.
- The Fasti callback path is
  `/api/access/v1/trailbase/callback`, and its only C1 browser origin is
  `http://127.0.0.1:8420`. The trusted host provisions the exact absolute URI
  `http://127.0.0.1:8420/api/access/v1/trailbase/callback` in TrailBase
  `auth.redirect_uri_allowlist`. Authentication readiness rejects a different
  port, unprovisioned URI, relative redirect, protocol-relative redirect, or
  any non-loopback origin. Existing non-authentication port-fallback behavior
  is unchanged. Remote browser authentication remains explicitly unavailable
  until a separate reviewed browser-facing proxy/origin work package exists;
  `FASTI_PUBLIC_URL` does not activate TrailBase browser authentication in C1.
- Keep at most 64 `OperationId -> Zeroizing<verifier>` entries behind one
  process-local mutex. Reserve capacity before generating a verifier, durable
  row, cookie, or redirect. Insert memory before the durable row and remove it
  on durable failure. A callback claims the durable row before it takes the
  verifier. Cancellation commits first and removes memory second. Restart
  recovery never makes a remote call.
- Extend the unpublished v14 ceremony with only the non-secret associations
  required to finish its purpose: optional bound browser session, optional
  exact invited membership, workspace, selected profile grant, and
  remembered-browser choice. Database and domain checks require:
  - sign-in: workspace and selected grant, no bound session, and at most one
    invited membership already bound to the permanent TrailBase anchor;
  - recent authentication: one bound session and its existing workspace and
    selected grant, no invited membership, and no C1 production start because
    pinned TrailBase does not prove a fresh credential challenge;
  - first-administrator bootstrap: workspace and an existing active selected
    grant chosen by the trusted operator path, no bound session or invited
    membership, and `remembered = false`.
    The browser may request a selection, but the durable ceremony stores the
    server-normalized identifiers and the final transaction proves the subject
    owns the grant. TrailBase proof never selects or grants a profile.
- First-administrator start proves the existing owner-only `bootstrap.secret`,
  descriptor root, permissions, and exclusive data-root lock before the
  ceremony becomes usable. After successful TrailBase cleanup, one final
  transaction creates the subject, permanent anchor, active administrator
  membership, explicit subject-to-existing-grant assignment, provenance,
  browser session, audit evidence, and completed ceremony. Nothing is created
  before cleanup. A losing race has no side effects.
- Ordinary sign-in resolves only `(TrailBaseInstanceId, TrailBaseSubject)` and
  loads every currently active grant for that subject and ceremony workspace.
  It requires the ceremony-selected grant among them. It never links by email
  and never selects the first or only row implicitly. When the ceremony binds
  an invitation, the final transaction requires that exact membership to be
  `invited`, owned by the resolved subject, and in the ceremony workspace;
  it applies `AcceptInvitation`, advances the subject authorization epoch,
  records the exact membership audit, rechecks the selected grant and client,
  then issues provenance and the session atomically. An unknown TrailBase
  subject is never linked by email and cannot use an invitation in C1.2.
- Recent authentication remains visible but unavailable in C1.2. Exact
  TrailBase `v0.33.5` can reuse an existing TrailBase or social-provider
  session and exposes neither a forced fresh challenge nor trusted
  `auth_time`. C1.2 therefore never starts or finalizes a production
  `recent_authentication` ceremony and never stamps callback time as recent
  credential proof. The durable purpose and association remain reserved for
  a later source-backed work package.
- Do not hold a SQLite transaction across network input/output. After status,
  perform a non-mutating local preauthorization. Attempt TrailBase logout
  exactly once after every successful exchange, including status or local
  authorization failure. After exact logout success and token destruction,
  repeat every mutable check in one `BEGIN IMMEDIATE` final transaction.
- The private wire contract is exact for TrailBase `v0.33.5`: token exchange
  accepts a 48-character authorization code and server-held verifier; status
  uses `Authorization: Bearer` plus `Refresh-Token`; logout receives only the
  refresh token. Token and status JSON must contain exactly the documented
  fields. Status rejects null fields, refresh mismatch, or CSRF mismatch.
  Logout requires status 200. Redirects are disabled.
- Decode only the bounded status-returned token payload. Require authentication
  `type = 1`, a URL-safe Base64 subject of exactly 16 bytes, a non-empty email,
  provider `0` for `trailbase_password` or one of `1`, `2`, `9` through `17`
  for `trailbase_social`, and sensible current `iat`/`exp`. Ignore `admin`.
  Treat `mfa` only as enrollment metadata and never raise assurance from it.
- Transport limits are fixed: four concurrent exchanges; two-second connect
  timeout; five-second total timeout per request; 8 KiB request JSON; 16 KiB
  response body; 8 KiB compact token; 4 KiB decoded payload; 60 seconds future
  clock skew; and at most a two-hour status-token lifetime. These are Access
  limits, not inherited provider policy.
- Vendor authorization code, verifier, access token, refresh token, and CSRF
  values implement no `Debug`, `Clone`, or serialization, are zeroized or
  dropped at the narrowest boundary, and never enter SQLite, browser state,
  contracts, logs, audit records, problems, or receipts. Only the confirmed
  non-secret subject, method, instance, generation, and verification time cross
  into application/store commands.

C1.2 fails closed with the existing exact ceremony failures:

| Boundary                                                                                        | Durable result                                                                                                                                                                                                           |
| ----------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Binding, code, claim, replay, or missing verifier before exchange                               | No remote call; the applicable one-use or validation denial remains authoritative.                                                                                                                                       |
| Definite exchange rejection before a successful token response                                  | `failed/exchange_failed`.                                                                                                                                                                                                |
| Exchange timeout, response loss, or malformed successful response with uncertain token creation | `cleanup_uncertain/exchange_outcome_uncertain`.                                                                                                                                                                          |
| Status or local authorization fails and logout succeeds                                         | `failed/status_rejected` or `failed/local_authorization_denied`.                                                                                                                                                         |
| Logout is not an exact 200 after tokens exist                                                   | `cleanup_uncertain/logout_uncertain`.                                                                                                                                                                                    |
| Logout succeeds but the final local authorization or trust recheck fails                        | No session; the store records the exact `failed/local_authorization_denied` or `failed/trust_unavailable` evidence.                                                                                                      |
| Logout succeeds but the selection transition has a localized persistence failure                | No session; one subsequent writable fallback transaction records `failed/local_persistence_failed`. A continuing or ambiguous database failure returns local-state failure and does not claim durable terminal evidence. |
| Pending or claimed ceremony is recovered after restart                                          | `failed/verifier_lost_on_restart` or `cleanup_uncertain/exchange_outcome_uncertain`; never retry.                                                                                                                        |
| A committed session response is lost                                                            | Do not reproduce its digest-only secret; start a fresh ceremony.                                                                                                                                                         |

C1.2 exits only when a private scripted loopback fixture proves the exact
methods, paths, headers, bodies, content types, bounds, redirect refusal,
timeout behavior, token parsing, provider classification, cleanup-on-failure,
and exactly-once logout; and domain/store tests prove capacity compensation,
claim/cancel and two-tab races, activation-generation rechecks, anchor and
membership/grant/client denials, invitation binding and atomic acceptance,
stale epochs, recent-auth unavailability,
post-cleanup bootstrap, final-transaction atomicity, restart windows, response
loss, and existing-session behavior during a later TrailBase outage.

### 2.3 C1.2 completion record

C1.2 is frozen at commit
`ce278d667a10ccc531f8bf5edd0969f44eeb52f3`, tree
`5757c73e6907023420fb7e1b82d07a36cfbec0f1`. Nothing from this checkpoint was
pushed or merged.

- The concrete private adapter performs only the approved fixed-origin
  `/token` to `/status` to local authorization to `/logout` sequence. It drops
  every vendor token before final Fasti session issuance.
- The adapter owns the active installation ID and activation generation. A
  browser cannot choose either value or any TrailBase origin, address, path,
  or header.
- The process-memory verifier vault reserves before secret generation, holds
  at most 64 zeroizing verifiers, and gives a claimed ceremony one exchange
  winner.
- Sign-in and first-administrator completion repeat mutable authorization in
  one final immediate transaction. Audit, provenance, invitation acceptance,
  subject and membership bootstrap, and session issuance cannot partially
  commit.
- Active workspace membership is rechecked during session creation and every
  browser-session authentication. The former first-administrator bypass was
  deleted.
- Exact C1.2 focused results are 11 TrailBase adapter/orchestration tests, 22
  human-access store tests, 10 browser-session store tests, and 20 Access
  domain tests. The full affected-package run passed 45 API, 75 application,
  78 domain, 23 provider-runtime, 263 store, and three store-security tests,
  plus documentation tests.
- Strict clippy passed for every affected Rust package. Formatting, diff
  checks, and a redacted Gitleaks scan passed.
- The independent lifecycle/diff reviewer and the additive AGY review returned
  clear with no actionable P0 or P1. The required Ponytail review returned
  `Lean already. Ship.`
- The signed-in shell temp directory crosses the `.cache` symlink. SQLite's
  deliberate no-follow flag rejects snapshot destinations there. The untouched
  snapshot suite failed for that environmental reason and passed 7/7 with
  `TMPDIR=/tmp`; the complete affected-package gate then passed with the same
  real temp root. No snapshot code changed.

### 2.4 Frozen C1.3 public contract decisions

C1.3 is split into four written, sequential gates. One writer owns shared
registry, generator, API, SDK, host, and Workbench files. Read-only agents may
review the active gate and prepare the next one.

#### C1.3a — authorization, operations, and wire contract

- A Fasti browser session authorizes the existing first-party application
  routes that already promise credential-or-browser-session access. It never
  becomes a client credential. Add one distinct
  `BrowserSessionAccessContext`; keep the existing credential context intact,
  and use a two-variant application access context only on these ten
  capabilities: `AcceptObservation`, `CreateRecord`, `AttachIdentifier`,
  `ListRecords`, `RegisterNamespace`, `ListTrackingDispositions`,
  `SetTrackingDisposition`, `GetNuvioCollections`,
  `ReplaceNuvioCollections`, and `ClearNuvioCollections`.
- Add `AuthorizationKind::ScopedOrBrowserSession` only for that fixed
  allowlist. The credential branch keeps every current credential, grant,
  epoch, and scope check. The browser branch carries no credential identity or
  browser-presented scopes. The store atomically reloads the selected grant's
  current scopes and requires the capability's existing scope set; human
  sign-in never grants a client scope by itself. It attributes domain work to
  an explicit actor client: the active client that owns the selected profile
  grant. It then rechecks the browser-session row, current subject, active
  membership, selected grant and profile, active grant owner, both subject
  epochs, and required scopes before data access or mutation. The mixed request
  type exposes no client, credential, or actor identity before that
  transactional authorization. The result preserves actor kind as
  `AuthorizedActor::Credential { presented_client_id, credential_id }` or
  `AuthorizedActor::BrowserSession { auth_subject_id, browser_session_id,
grant_owner_client_id }`. Only persistence that truly records source
  attribution may call the explicitly named `attribution_client_id()`
  projection; audit and provenance retain the actor variant. Reject a request
  that supplies both a bearer credential and a session cookie.
- Browser-session management routes continue to use the existing
  `BrowserSessionPort`, `AuthenticatedBrowserSession`, and mutation-command
  boundaries. Mutations repeat session, CSRF, subject, membership, grant,
  client, authentication-epoch, and authorization-epoch checks in the same
  transaction as the mutation.
- Activate the nine existing browser-session capabilities without renaming
  them. Add only `access.projection.read` for the server-derived Account and
  security projection and `access.identity.bootstrap` for the local-operator
  first-administrator start. The bootstrap capability has no public HTTP or
  SDK method.
- Keep membership lifecycle, role, subject lifecycle, activation repair, and
  recent-authentication mutation controls visible but unavailable. Their
  application/store owners remain intact. Do not mount a route that can never
  satisfy recent authentication in C1.
- Add the canonical non-enumerating C1 problem families
  `identity_service_unavailable`, `trailbase_version_unsupported`,
  `trailbase_trust_unavailable`, `trailbase_proof_invalid`,
  `trailbase_session_cleanup_failed`, `auth_browser_binding_invalid`,
  `auth_subject_unaffiliated`, `auth_continuation_persistence_failed`,
  `auth_identity_conflict`,
  `auth_last_sign_in_method`, `auth_assurance_insufficient`, and
  `recent_authentication_required`. Reuse existing browser-session, capacity,
  forbidden, validation, integrity, and storage problems. No problem detail
  identifies which authorization predicate failed.
- Freeze their governed problem contracts before registry generation:

  | Problem code                           | HTTP | Safe state             | Retry                    | Exact default next action                                                               |
  | -------------------------------------- | ---: | ---------------------- | ------------------------ | --------------------------------------------------------------------------------------- |
  | `identity_service_unavailable`         |  503 | `prior_state_retained` | `retry_safe`             | `retry_identity_service` — Check TrailBase health and retry the same safe operation     |
  | `trailbase_version_unsupported`        |  503 | `no_mutation`          | `retry_after_correction` | `install_supported_trailbase` — Install the pinned supported TrailBase release          |
  | `trailbase_trust_unavailable`          |  503 | `no_mutation`          | `retry_after_correction` | `repair_trailbase_activation` — Repair the pinned TrailBase activation before retrying  |
  | `trailbase_proof_invalid`              |  401 | `prior_state_retained` | `retry_after_correction` | `restart_sign_in` — Start a new sign-in ceremony                                        |
  | `trailbase_session_cleanup_failed`     |  502 | `prior_state_retained` | `retry_after_correction` | `inspect_trailbase_cleanup` — Check TrailBase health, then start a new sign-in ceremony |
  | `auth_browser_binding_invalid`         |  401 | `no_mutation`          | `retry_after_correction` | `restart_sign_in` — Start a new sign-in ceremony                                        |
  | `auth_subject_unaffiliated`            |  403 | `prior_state_retained` | `retry_after_correction` | `request_workspace_membership` — Ask a workspace administrator for access               |
  | `auth_continuation_persistence_failed` |  503 | `prior_state_retained` | `retry_after_correction` | `restart_sign_in` — Start a new sign-in ceremony                                        |
  | `auth_identity_conflict`               |  409 | `prior_state_retained` | `not_retryable`          | `inspect_identity_link` — Inspect the existing identity link before making changes      |
  | `auth_last_sign_in_method`             |  409 | `prior_state_retained` | `retry_after_correction` | `add_sign_in_method` — Add another verified sign-in method before removal               |
  | `auth_assurance_insufficient`          |  403 | `no_mutation`          | `retry_after_correction` | `use_required_assurance` — Use a sign-in method that meets the required assurance level |
  | `recent_authentication_required`       |  403 | `no_mutation`          | `retry_after_correction` | `authenticate_again` — Authenticate again before this sensitive action                  |

- `auth_last_sign_in_method`, `auth_assurance_insufficient`, and
  `recent_authentication_required` keep these frozen descriptors but remain
  reserved outside the generated callable-capability problem catalog until
  their unavailable controls gain an implementing capability. Do not attach an
  impossible error to an unrelated active route merely to satisfy catalog
  coverage.
- Correct the existing `register_namespace_definition` authorization key from
  `AttachIdentifier` to `RegisterNamespace` before enabling the mixed-auth
  matrix. This is a pre-existing root-cause defect, not a reason to retain the
  wrong capability policy.
- Freeze these HTTP operations and paths:

  | Operation ID                        | Method and path                                               | Capability                       |
  | ----------------------------------- | ------------------------------------------------------------- | -------------------------------- |
  | `start_trailbase_sign_in`           | `POST /api/access/v1/trailbase/sign-in`                       | `browser.session.create`         |
  | `complete_trailbase_authentication` | `GET /api/access/v1/trailbase/callback`                       | `browser.session.create`         |
  | `read_access_projection`            | `GET /api/access/v1/projection`                               | `access.projection.read`         |
  | `read_browser_session`              | `GET /api/access/v1/browser-session`                          | `browser.session.read`           |
  | `end_browser_session`               | `DELETE /api/access/v1/browser-session`                       | `browser.session.end`            |
  | `list_browser_sessions`             | `GET /api/access/v1/browser-sessions`                         | `browser.sessions.list`          |
  | `revoke_browser_session`            | `DELETE /api/access/v1/browser-sessions/{browser_session_id}` | `browser.session.revoke`         |
  | `revoke_other_browser_sessions`     | `DELETE /api/access/v1/browser-sessions/others`               | `browser.sessions.revoke_others` |
  | `revoke_all_browser_sessions`       | `DELETE /api/access/v1/browser-sessions`                      | `browser.sessions.revoke_all`    |
  | `rotate_browser_session`            | `POST /api/access/v1/browser-session/rotation`                | `browser.session.rotate`         |
  | `select_browser_session_profile`    | `PUT /api/access/v1/browser-session/profile`                  | `browser.session.profile.select` |

- Sign-in start accepts only `workspace_id`, `profile_grant_id`, `remembered`,
  and an optional exact `invited_membership_id`. It accepts no origin,
  callback, return URL, TrailBase instance, provider, or vendor token. It
  returns the fixed authorization URL, ceremony ID, and expiry after the
  durable row and process-memory verifier both exist.
- The callback accepts only the exact 48-character `code` query and the narrow
  binding cookie. It never depends on the Strict Fasti session cookie. Success
  redirects to the ceremony's fixed `application_home` or `first_run` target.
  Failure redirects to the matching fixed target with only a server-generated
  correlation ID and a fixed failed-state marker. The UI treats that query as
  a hint and loads server evidence; it never treats query text as proof.

#### C1.3b — cookie, CSRF, and host boundary

- The opaque session cookie is `__Host-fasti_session`: `Secure`, `HttpOnly`,
  `SameSite=Strict`, `Path=/`, and no `Domain`. Its maximum age never exceeds
  the session's absolute expiry.
- The CSRF cookie is `__Host-fasti_csrf`: `Secure`, `SameSite=Strict`,
  `Path=/`, and no `Domain`. It is readable only so the first-party SDK can
  copy it to the exact `X-CSRF-Token` header. The store also verifies its
  digest against the current session; equality of cookie and header alone is
  insufficient.
- The one-use callback cookie is
  `__Secure-fasti_auth_binding`: `Secure`, `HttpOnly`, `SameSite=Lax`, exact
  `Domain=127.0.0.1`, and `Path=/api/access/v1/trailbase/callback`. Clear it
  with the same attributes on success and every failure. The locked Linux Wry
  adapter requires the explicit IP domain; Windows and macOS remain separate
  platform gates. The session and CSRF cookies remain `__Host-` cookies with
  no `Domain`.
- Authenticated reads require exact `Host: 127.0.0.1:8420`. Browser mutations
  require exact `Origin: http://127.0.0.1:8420`, exact Host, the Strict session
  cookie, and CSRF cookie/header proof. Sign-in start requires exact Origin and
  Host. Callback requires exact Host but no Origin or Strict session cookie.
- Mount browser authentication only on direct `127.0.0.1:8420`. Port fallback,
  wildcard/remote listeners, `FASTI_PUBLIC_URL`, and a proxy do not activate
  C1 authentication. The remote router keeps every C1 route absent.
- Session rotation and profile selection rotate both session and CSRF values.
  Ending or revoking the current session clears both cookies. No browser
  credential is written to local storage, session storage, a URL, a log, an
  example, or a generated schema.
- Every Access response, including redirects and problems, sends
  `Cache-Control: private, no-store`.

#### C1.3c — projection and generated contracts

- Add one Access application/store projection. It returns only current,
  authorized, non-secret state: subject lifecycle; workspace membership and
  role; current session and bounded session inventory; available and selected
  profile grants; policy and expiry; recent-authentication availability and
  expiry; TrailBase activation state and generation; first-run steps; and
  bounded evidence/correlation identifiers and times.
- The projection is the only source for Gate 10 A and C. It uses the shared B
  evidence vocabulary: `unavailable`, `needs_attention`, `failed_safely`, and
  `verified`, plus loading and empty states. It never reports passkeys,
  recovery, devices, Authentik management, or other later packages as
  verified.
- Update the authored registry and capability/problem enums first. Then update
  authored DTO and OpenAPI sources and the generator operation table. Regenerate
  OpenAPI 3.1, JSON Schema, problem and capability catalogs, SDK types and
  methods, and OKF from those owners. Do not edit generated output manually.
- The navigation callback is documented in OpenAPI but has no SDK method.
  AsyncAPI remains not applicable because C1 adds finite request/response
  operations and no event channel. JSON-LD remains not applicable because
  identity and session state are private security state.

#### C1.3d — trusted composition and deferred platform gate

- The trusted local Unix CLI owns the active C1 first-administrator bootstrap. Run
  `fasti access bootstrap-administrator` while `fastid` is stopped. The CLI
  holds the exclusive data-root lock, proves the owner-only `bootstrap.secret`,
  requires completed first-client enrollment, verifies the exact active
  TrailBase installation receipt, and keeps the PKCE verifier and callback
  binding in one Rust process. It accepts no password, token, bootstrap secret,
  subject, or binding argument.
- The CLI prints only the TrailBase authorization URL and expiry. TrailBase
  receives each person's password. The operator pastes the exact fixed callback
  URL through bounded, non-echoed terminal input. Fasti then performs the same
  `/token` -> `/status` -> `/logout` exchange and one transactional anchor,
  subject, administrator membership, profile-grant, provenance, audit, and
  ceremony completion. The transaction immediately revokes its evidence-only
  Fasti session, returns no cookie or session secret, and leaves ordinary
  browser sign-in as the next action.
- The CLI, standalone `fastid`, and packaged host are mutually exclusive owners
  of one data root. The CLI has no router or fabricated listener. `fastid` owns
  the exact `127.0.0.1:8420` router for ordinary browser sign-in. A normal
  sign-in ceremony cannot become bootstrap, and no loopback HTTP caller can
  invoke the operator operation.
- The locked Tauri 2.11.5/Wry 0.55.1 native cookie source is preserved, but its
  runtime proof is no longer an immediate C1 exit gate. Linux, Windows, and
  macOS WebView callback transport, first-administrator authentication, and
  cookie behavior belong to `C1-TAURI-AUTH`. A platform that rejects the
  Secure cookie remains unavailable; do not add a cookie dependency, custom
  cookie store, or weaker cookie.
- C1 exits only after operation authorization matrices, transaction rollback,
  cookie/path/origin/two-tab/replay tests, generated drift, SDK parsing,
  no-secret scans, local/remote router negatives, trusted-host source tests,
  restart, exact TrailBase outage behavior, and ordinary-browser evidence
  pass. Packaged-host runtime and WebView proof remain required only before a
  packaged desktop authentication claim.
- Windows first-administrator setup is deferred with packaged authentication;
  no Windows CLI support is claimed until protected console input and its
  exact platform evidence exist.

### 2.5 C1.3a-c completion record

This historical checkpoint is superseded by the current local checkpoint in
section 2.9.

C1.3a-c is frozen at commit
`5682073bdf54d3589b7815f3d5814ff52ab6c390`, tree
`a764469e23b2c6deb28fd676e1e3dee39cbc3a59`. Nothing from this checkpoint was
pushed or merged.

- The exact direct `127.0.0.1:8420` listener mounts Access and browser-session
  authorization only after the stored TrailBase installation is active. Port
  fallback, alternate IPv4 or IPv6 loopback, wildcard/container forwarding,
  generic local, integration, and remote routers keep those routes absent.
- The nine browser-session capabilities and `access.projection.read` are
  active. `access.identity.bootstrap` remains inactive until C1.3d owns the
  one packaged-host runtime and first-administrator command.
- OpenAPI 3.1, generated JSON Schema, capability and problem catalogs, OKF,
  callable SDK methods, cookie and CSRF security declarations, and exact
  operation problem subsets share the authored registry and application
  capability owners. The navigation callback has no SDK method.
- Rust and JavaScript validators walk every schema reachable from an Access
  request or response and reject the complete frozen secret-property set.
  Staged-diff Gitleaks found no secret.
- The exact clean commit passed `cargo xtask contract verify --locked`,
  including deterministic generation, generated drift, formatting, workspace
  type checks, 110 JavaScript mutation/SDK tests, strict workspace clippy,
  complete workspace tests, conformance tests, builds, and package policy.
- Independent route, generator, error-mapping, and next-slice reviewers found
  and closed the active-installation mount gate, operation-specific problems,
  callable-SDK proof, and transitive no-secret gate. The optional AGY outside
  review could not inspect the repository because its headless command
  permission was denied; no AGY pass is claimed.
- C1.3d remains open. It must share one TrailBase orchestrator and verifier
  vault between packaged-host bootstrap start and the callback, reload the
  owner-only bootstrap secret only after a bootstrap ceremony is claimed, own
  the fixed listener without fallback, and resolve packaged `/first-run`
  serving before C1.4 begins.

### 2.6 C1.3d code completion record

This historical checkpoint is superseded by the current local checkpoint in
section 2.9.

C1.3d production code is frozen locally at commit
`55cfd0809f6dd147587b37bd640c7c4b495f0ad5`, tree
`6743f38590623654d215788a6afc18640e9fecc4`. Nothing from this checkpoint was
pushed or merged.

- The packaged desktop host opens the exact fixed `127.0.0.1:8420` listener,
  embeds the existing Access router, serves packaged Workbench assets without
  masking `/api` or non-navigation methods, and navigates the main WebView to
  that origin. It does not fall back to another port.
- The native first-administrator command, callback, kernel, TrailBase client,
  and process-memory PKCE verifier use one shared runtime. Only that command
  reads `bootstrap.secret`; ordinary sign-in does not. A native cookie failure
  durably cancels the unclaimed ceremony before removing its verifier.
- Unpublished migration v14 permits only one pending or claimed
  first-administrator bootstrap. A second start fails before the host can
  replace the first callback-path cookie. The first callback remains usable.
- Desktop exit uses Tauri `run_return`, signals Axum graceful shutdown, and
  waits up to 20 seconds. This covers the three sequential five-second
  TrailBase request ceilings plus margin while keeping shutdown bounded.
- Desktop and Android capabilities preserve every existing trusted-host
  command. Desktop grants only the main window at the exact loopback origin;
  Android keeps the packaged local WebView and reports first-administrator
  bootstrap unavailable because its locked WebView cookie setter is a no-op.
- The exact clean commit passed all 27 governed contract gates. Receipt:
  `target/fasti-receipts/b1-contract-verification.json`, SHA-256
  `69f51b1ffa7783f3b6da80609418b3dec6ec80029fbf62544468408737cc6716`.
  Focused results also include 54 `fasti-api` tests, 43 packaged desktop tests,
  34 schema tests, deterministic generated output, and staged no-secret scans.

Code completion is not platform conformance. Real Linux, Windows, and macOS
WebViews must still prove the native-set Secure callback cookie, exact-path
delivery, two-start recovery, and one exchange before `C1-TAURI-AUTH` closes.
They do not block immediate C1 delivery. The local
fixed port is currently owned by an unrelated `pasta.avx2` process, PID
`1329362`; no shared process was stopped and no fallback was introduced.

### 2.7 Bounded C1.4 continuation gate

Two source-backed gaps must be corrected before ordinary signed-out sign-in or
failure evidence becomes active in C1.4. This is not a new premise gate.

1. Ordinary sign-in cannot require a signed-out browser to supply workspace or
   profile-grant identifiers. TrailBase identity must be confirmed first. Fasti
   must then expose only bounded choices owned by that confirmed subject,
   require an explicit selection even when one choice exists, and recheck the
   chosen membership, grant, client, subject epochs, and activation generation
   in the final session-creation transaction. No unauthenticated enumeration,
   implicit first choice, browser-stored identifier, or bootstrap authority is
   permitted.
2. A failed callback has no Fasti session and therefore cannot read the
   authenticated Access projection. Attributable post-claim failures need one
   browser-binding-protected continuation read that returns only durable safe
   evidence. Pre-claim callback noise remains a generic failure with a
   correlation reference and no invented evidence.

The minimum design reuses the existing ceremony row and high-entropy browser
binding. It adds no provisional Fasti session, second identity system,
continuation table, polling loop, or browser state store. The exact typed
states, authorization vocabulary, DTOs, expiry, candidate bound, revision
digest, cookie path, and transaction races must be frozen in one bounded
engineering and developer-experience review before production code changes.
The D3-C order remains exact:

```text
TrailBase /token -> /status -> local subject check -> /logout -> discard
-> explicit Fasti selection continuation -> final authorization transaction
-> opaque Fasti session
```

The C1.4 UI may be authored and fixture-tested while packaged platform proof
is pending. Ordinary-browser activation requires this continuation gate and
ordinary-browser evidence. Packaged desktop activation separately requires
the `C1-TAURI-AUTH` WebView gate.

### 2.8 Frozen identity-first continuation contract

The bounded engineering, test, developer-experience, and additive AGY reviews
converged on the following minimum. These decisions amend the unpublished v14
design only. They add no provisional session, second ceremony table, browser
state store, provider abstraction, or polling loop.

- Ordinary `POST /api/access/v1/trailbase/sign-in` accepts only
  `{ "remembered": boolean }`. It rejects workspace, profile-grant,
  membership, subject, ceremony, callback, origin, and provider identifiers.
  The response contains the authorization URL and expiry only; the browser
  binding cookie is the sole ceremony authority.
- `AuthCeremony` stores an optional final selection and the remembered choice
  separately. Ordinary sign-in starts without a selection. Bootstrap and the
  reserved recent-authentication purpose keep their existing required,
  server-normalized selections and `remembered = false`.
- Add exactly one non-terminal domain state, `SelectionRequired`. Only an
  already claimed ordinary sign-in may enter it, and only after TrailBase
  logout succeeds and the refresh/access proof material has been destroyed.
  It survives process restart because no remote cleanup or PKCE verifier
  remains. `Pending` and `Claimed` keep their existing fail-closed restart
  behavior.
- Persist on the existing ceremony row only the non-secret confirmation needed
  after logout: anchored `AuthSubjectId`, authentication method and verified
  time, subject authentication epoch, and subject authorization epoch. The
  activation generation remains the existing ceremony field. No TrailBase
  token, authorization code, verifier, email, username, or provider payload is
  persisted.
- Selection-required ceremonies retain the original ten-minute ceremony
  expiry. Maintenance expires both `Pending` and `SelectionRequired` exactly
  at `expires_at`. Expiry, completion, and cancellation remain terminal and
  retained by the existing 24-hour bounded evidence policy.
- The callback returns a typed bootstrap-completed, selection-required, or
  attributable-failure outcome. Bootstrap still creates its session directly.
  Ordinary sign-in never creates a Fasti session in the callback.
- On selection-required or attributable post-claim failure, the response
  clears the callback cookie and writes the same high-entropy binding value to
  host-only `__Secure-fasti_auth_continuation` with exact
  `Path=/api/access/v1/trailbase/continuation`, `Secure`, `HttpOnly`,
  `SameSite=Strict`, and `Max-Age` no longer than the ceremony's remaining
  lifetime. Pre-claim noise clears the callback cookie and receives no
  continuation cookie. Completion, cancellation, and expired evidence clear
  the continuation cookie.
- Add one resource path, `/api/access/v1/trailbase/continuation`, under the
  existing `browser.session.create` capability:
  - `GET` reads a binding-protected `SelectionRequired` projection or returns
    one governed `application/problem+json` terminal result;
  - `POST` accepts only an opaque zero-based choice ordinal and the exact
    candidate revision, never retries automatically, and returns `204` plus
    the opaque Fasti session and CSRF cookies on success;
  - `DELETE` cancels or dismisses the bound continuation, returns `204`, and
    clears the cookie. It is the existing user-control/start-over path, not a
    second workflow.
    `GET` requires the exact Host and exactly one well-formed continuation
    cookie. `POST` and `DELETE` additionally require the exact Origin. None
    accepts bearer authorization, browser-session CSRF, operation IDs, or
    correlation IDs as authority.
- A selection projection contains only `expires_at`, `remembered`, one
  canonical SHA-256 revision, and at most 64 choices. Each choice exposes an
  opaque ordinal plus safe presentation facts: deterministic workspace and
  profile ordinals, workspace/profile creation times, membership state, and
  role. It emits no workspace, profile, grant, membership, client, subject, or
  TrailBase identifier. More than 64 choices fails closed; it is never
  truncated.
- Core storage currently has no authoritative workspace or profile display
  name. C1 does not invent one or expose identifiers as labels. The C1 UI uses
  the truthful creation time, role, membership state, and deterministic
  ordinal. Adding editable names is a separate domain decision; it must not be
  smuggled into authentication. Runtime UX evidence must record this known
  recognition-over-recall ceiling instead of claiming it is solved.
- The candidate revision is computed over the deterministic, complete
  candidate tuples plus the stored subject epochs and activation generation.
  `POST` uses one `BEGIN IMMEDIATE` transaction to reload the bound ceremony,
  require `SelectionRequired`, require it unexpired, compare stored and current
  subject epochs, require the same active TrailBase generation, recompute the
  complete candidate set and revision, resolve the ordinal, and recheck the
  membership or exact invitation, grant, client, and subject lifecycle. It then
  accepts an invitation when applicable, stores the internal selection,
  inserts authentication provenance and audit evidence, creates exactly one
  opaque Fasti session, and CAS-transitions the ceremony to `Completed` before
  commit. A stale revision, losing tab, invalid ordinal, or injected write
  failure creates no session and changes no authorization state.
- Add governed `auth_selection_changed` evidence for a stale revision:
  HTTP 409, prior state retained, retry after correction, next action
  `review_sign_in_choices`. Preserve exact existing unaffiliated, identity,
  cleanup-uncertain, expiry, and binding evidence where source state proves it;
  do not collapse a known condition into invented success or generic copy.
  Pre-claim failures remain generic and non-attributable.
- Workbench and the generated SDK use one same-origin client and exact request
  DTOs. The callback remains browser navigation and has no SDK method. The SDK
  exposes read, complete, and cancel continuation methods, never retries the
  completion POST, and keeps returned data only in volatile component state.
  No new Access host abstraction, provider, or wizard store is permitted.

The minimum proof is one table-driven dependency-race store test, one complete
TrailBase-to-continuation-to-session test, one table-driven attributable failure
test, focused domain/state and cookie boundary tests, generated-contract drift,
and the existing exact C1 suites. Explicit selection remains required even for
one candidate.

Theme and accessibility ownership stays in the existing Workbench:

- light: `data-bs-theme=light`, `data-fasti-theme=light`;
- dark: `data-bs-theme=dark`, `data-fasti-theme=dark`;
- night: `data-bs-theme=dark`, `data-fasti-theme=night`;
- forced colors is an environment, never a saved fourth theme.

Reuse `ThemeSettings`, `fasti-theme-settings`, current root attributes, tokens,
and Tabler CSS. Do not add a theme provider, wizard store, UI framework, or
custom control where Tabler or a native element already works. Test the A/C/B
golden path across three themes and four viewports; cover every canonical state
once in a representative combination; and run forced-colors, reduced-motion,
200% zoom, text-spacing, keyboard, focus, and screen-reader checks per surface.
Static Gate 10 evidence is design evidence only, not WCAG or EN conformance.

### 2.9 Current local implementation checkpoint

The current local source checkpoint is commit
`dde753d77aa5022cdcffc7b284f2abf78c94af79`, tree
`a771a0f7d3f658398f55252e0d9aeabf0a4784ec`. The branch is one documentation-
only commit behind `origin/dev`. It is not pushed, merged, released, or
deployed.

- Commit `2825d1c625f6b0e360fcea6725cfe28319c3070f` implements the bounded
  identity-first continuation and the final C1.3 route and contract behavior.
- Commit `85b2e8036b5935f2326a6d95e371452a01040db2` implements Gate 10 A and C
  from one Access projection. B remains an in-context evidence pattern.
- Commit `7092943a61650497af5933e618141e746251c0fd` binds activation to the exact
  verified TrailBase release, artifact, and physical root. The native and
  Desktop launchers auto-start only an already initialized TrailBase root. They
  never initialize it.
- Historical commit `0873e227027ff713a5fb40671cf40088e625a018` added
  `cargo xtask test milestone --body C1`. It writes the gate-suite receipt at
  `target/fasti-receipts/access-c1.json`. It does not write or accept a C1
  closure manifest. Section 2.10 and the current runner supersede that receipt's
  delivery scope.
- Commit `0b14bd69c8d00bcc25fd8d63119e2e79106f10ec` binds the implemented Access
  projection to the A+C UI evidence owner without claiming package smoke.
- Commit `dde753d77aa5022cdcffc7b284f2abf78c94af79` preserves exact installation
  identity across the pinned adjacent-version upgrade and rollback fixtures,
  keeps the OCI runtime nonce stable, and makes the launcher self-test hermetic.

Only the exact requested-and-bound `127.0.0.1:8420` durable listener mounts the
C1 route set. It mounts those routes even when no TrailBase root is available,
so the UI can report the exact unavailable state. Exchange and new Fasti
session issuance require both a verified installation receipt and persisted
active activation. Port fallback, alternate loopback, generic local,
integration, wildcard or container forwarding, and remote routers omit the C1
route set.

Source implementation is not C1 delivery. The in-scope milestone receipt,
final reviews, exact-head CI, pull request, merge, and merged-tree evidence
remain pending. Full WCAG 2.2 Level AA and EN 301 549 conformance is not
claimed.

### 2.10 Deferred packaged Tauri authentication

Work package `C1-TAURI-AUTH` owns packaged-WebView authentication after C1
merges. It is not part of the immediate C1 delivery boundary.

The preserved Linux failure harness is
`scripts/smoke-desktop-access-webdriver.py`. With WebKitGTK 2.52.3 it reached
the exact pinned TrailBase sign-in flow, then failed before callback exchange
because the packaged WebView did not retain the required Secure callback-
binding cookie on either `http://127.0.0.1` or `http://localhost`. Both native
cookie insertion and an HTTP `Set-Cookie` probe failed. The harness uses
generated disposable credentials, redacts values, and writes no password or
vendor token to its evidence.

This evidence does not authorize local HTTPS, certificate generation, a
weaker cookie, JavaScript credential storage, or a separate native session
system. C1 keeps the ordinary-browser Secure-cookie contract and direct
TrailBase backchannel unchanged. No packaged desktop authentication support is
claimed. The follow-up must choose and verify its transport independently,
retain the existing Fasti authorization and transaction boundary, and prove
Linux, Windows, macOS, platform accessibility, restart, revocation, and
no-secret behavior before activation.

## 3. Exact TrailBase evidence

Authority: TrailBase `v0.33.5`, tag commit
`b4c85d5152d4e5f472e0b5da5303f7c938e3a083`.

| Surface                           | Exact tagged-source result                                                                                                                                                                                              | C1 disposition                                                                                                                                                                                                |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Release identity                  | Fasti pins the version, tag commit, native digests, OCI digests, licence digest, executable version, and the official same-release Auth UI WASM archive and component digests.                                          | Reuse the existing release lock and sole launcher. Install only the verified Auth UI component into the private TrailBase root before start.                                                                  |
| Human authentication UI           | TrailBase v0.33.5 ships its default Auth UI as the separate release asset `trailbase_v0.33.5_wasm_auth_ui.zip`; the server alone returns 404 for `/_/auth/login`.                                                       | Use the official component. Do not build a Fasti login replacement. A missing, unsafe, mismatched, or mutated component keeps sign-in `trailbase_trust_unavailable`; no runtime floating download is allowed. |
| Authorization-code exchange       | `/api/auth/v1/token` requires the code and matching Proof Key for Code Exchange verifier, checks expiry, reloads the current user, and requires TrailBase-local accepted account-email state.                           | Call only from the server-held ceremony. Never accept browser tokens.                                                                                                                                         |
| Current status                    | `/api/auth/v1/status` with both the access token and `Refresh-Token` checks the access proof, refresh-session row, expiry, current user, and TrailBase-local accepted account-email state, then returns current tokens. | Require all response fields. Treat a 200 response with null fields as failure. Do not describe this as independent mailbox-ownership proof.                                                                   |
| Refresh-session cleanup           | POST `/api/auth/v1/logout` deletes the named refresh session and returns 200 even when it is already absent.                                                                                                            | Require 200 before Fasti session creation. A timeout, redirect, or other response fails closed.                                                                                                               |
| Subject                           | The current subject is in the status-returned authentication token payload.                                                                                                                                             | Decode only the bounded status-returned payload to read the current subject and authentication metadata. The trusted fact is the direct response from the pinned process, not an offline signature check.     |
| Password plus TOTP                | The password-to-MFA transition loses the original redirect, response type, Proof Key for Code Exchange challenge, and method in the exact pinned release.                                                               | Keep password-plus-TOTP authorization-code sign-in unavailable in C1 until an official source-backed TrailBase release preserves and verifies the complete ceremony.                                          |
| Social sign-in with enrolled TOTP | Social callbacks do not prove a TrailBase TOTP challenge occurred.                                                                                                                                                      | Never describe social sign-in as TOTP-verified. Do not use enrollment as proof of recent multi-factor authentication.                                                                                         |
| Authorization-code consumption    | TrailBase reads but does not delete the authorization code. The same code and verifier can mint more refresh sessions until expiry.                                                                                     | Fasti atomically consumes its own ceremony before exchange. This contains replay through Fasti but does not claim the upstream code is globally single use.                                                   |
| Account state                     | Status proves the current user still exists with a non-null email in TrailBase's locally accepted account state. TrailBase has no disabled or suspended account field.                                                  | Fasti enforces its own `AuthSubject` and membership lifecycle. Do not claim independent mailbox ownership or TrailBase account suspension support.                                                            |
| Refresh rotation                  | TrailBase does not rotate refresh tokens.                                                                                                                                                                               | C1 revokes and discards the refresh token during sign-in. Fasti never uses it as a durable application credential.                                                                                            |
| Token claims and keys             | Tokens have no issuer, audience, key identifier, token identifier, or not-before claim. There is no supported key overlap or retirement API.                                                                            | These facts block offline TrailBase token acceptance. C1 does not accept tokens offline, so they do not block the direct backchannel.                                                                         |

Primary source:

- [login and MFA flow](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/login.rs)
- [authorization-code exchange](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/token.rs)
- [authentication status](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/status.rs)
- [refresh-session logout](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/api/logout.rs)
- [token and refresh checks](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/tokens.rs)
- [token payload and key loading](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/core/src/auth/jwt.rs)
- [separate authentication UI component](https://github.com/trailbaseio/trailbase/blob/v0.33.5/crates/auth-ui/README.md)
- [official component installation](https://github.com/trailbaseio/trailbase/blob/v0.33.5/docs/src/content/docs/getting-started/install.mdx)
- [v0.33.5 release assets](https://github.com/trailbaseio/trailbase/releases/tag/v0.33.5)
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
- authentication provenance with a separate optional recent-authentication
  expiry;
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
- authentication provenance on the existing session owner and a nullable
  recent-authentication expiry. Ordinary C1.2 password and social sign-in
  persists provenance with no recent assertion. Authorization fails closed
  when that assertion is absent. A future source-backed fresh-challenge owner
  may set the expiry without replacing the provenance row.

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

The one-use browser authentication ceremony expires after 10 minutes. This
fixed C1 lifetime is owned beside the session policy in the application layer;
it is not an operator configuration key and does not extend on activity.

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
5. Verify the exact Auth UI archive and extracted component size and digests.
   Reject unsafe archive members, alternate layouts, missing files, version
   drift, and mutation. Prove a prepared root serves `/_/auth/login` without a
   runtime network fetch; absence or mismatch must fail closed.
6. Add `tests/e2e/access-c1.spec.ts` for the real callback, missing Strict Fasti
   cookie, valid narrow cookie, copied callback, sibling cookie, wrong path,
   wrong origin, replay, two tabs, lost response, outage, and recovery.
7. Add the A+C fixture matrix for loading, empty, unavailable, working,
   duplicate submit, partial membership, expired/revoked/conflict, resumable C,
   Save and leave, completion, B detail, authorization, focus, and live status.
8. Retain the existing unavailable-state browser tests as regression controls.
9. Add the C1 milestone runner only after M2 hands off `xtask`.

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
- [x] **T2 (P1, human: ~3 days / Codex: ~1 day)** — Access domain and store — Implement the frozen activation, anchors, versioned memberships/roles, ceremonies, provenance, recent proof, audit, first-admin bootstrap, and continuity.
  - Surfaced by: code-quality and test review; the current viable-administrator count is a placeholder and no membership aggregate exists.
  - Files: Access domain/application/store modules and migration v14 after M2.
  - Verify: focused unit and SQLite integration tests, restart, migration, rollback, restore, and race proof.
- [x] **T3 (P1, human: ~3 days / Codex: ~1 day)** — TrailBase adapter — Implement the fixed-origin exchange, status recheck, cleanup, token discard, and D2 recovery policy.
  - Surfaced by: architecture and security review; no production callback orchestration exists.
  - Files: one concrete Access adapter, orchestration owner, focused fixtures/tests.
  - Verify: scripted positive/negative matrix and no-secret scan.
- [x] **T4 (P1, human: ~2 days / Codex: ~6 hours)** — Access API and contracts — Mount authorized operations and generate every applicable contract and SDK surface.
  - Surfaced by: code-quality review; C1 capabilities are reserved but not mounted.
  - Files: API, capability registry, generators, SDK, host after M2 handoff.
  - Verify: generated drift, mutation tests, authorization negatives, and no-secret schemas. The local source implementation is complete; C1 delivery evidence remains pending.
- [x] **T5 (P1, human: ~3 days / Codex: ~1 day)** — Gate 10 A+C source — Implement permanent A, resumable C, and shared B evidence from one projection.
  - Surfaced by: UI test review; the earlier truthful unavailable shells required one implemented projection and separate A and C purposes.
  - Files: existing runtime settings owner, host, types, and `access-c1.spec.ts` after M2 handoff.
  - Verify: focused browser, theme, reflow, automated Axe, Tabler-policy, and Impeccable source checks. Ordinary-browser manual acceptance stays in T6; packaged acceptance belongs to `C1-TAURI-AUTH`.
- [ ] **T6 (P1, human: ~2 days / Codex: ~6 hours)** — Delivery — Run the C1 milestone, open one PR, merge the green exact head, and verify merged `dev`.
  - Surfaced by: test and delivery review; the in-scope runner exists, but its exact-head receipt and delivery evidence are pending.
  - Files: existing xtask/test/evidence/documentation owners after M2 handoff.
  - Verify: C1 milestone, canonical PR gate, native/OCI envelopes, browser A+C accessibility automation, and the real ordinary-browser TrailBase-to-Fasti session receipt bound to the exact clean commit and tree. Review rollback plus exact-head and merged-head receipts. Packaged Tauri authentication is separately owned by `C1-TAURI-AUTH`.

## 19. Safe next action

1. Preserve published migrations v12 and v13, archive v3 and v4, and the
   unpublished C1 migration v14.
2. Keep the local source checkpoint at
   `dde753d77aa5022cdcffc7b284f2abf78c94af79` while C1.5 reviews the combined
   result.
3. Run the C1 gate suite. Treat its receipt as the in-scope delivery receipt,
   not proof of packaged Tauri authentication.
4. Complete security, design, developer-experience, browser accessibility,
   exact-head, and merged-tree delivery evidence.
5. Reconcile the one documentation-only `origin/dev` commit without changing
   C1 behavior. Do not claim a merge or allocate v15 before merged-tree proof.
6. Keep one writer for shared source surfaces while later-slice agents remain
   read-only.
7. AGY may add an outside challenge. It never replaces the subagent review,
   written gates, tests, or delivery evidence.
8. Do not request another premise gate.

## GSTACK REVIEW REPORT

| Review        | Trigger               | Why                             | Runs | Status                                | Findings                                                                                                                                                                                                                                     |
| ------------- | --------------------- | ------------------------------- | ---- | ------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CEO Review    | `/plan-ceo-review`    | Scope & strategy                | 0    | —                                     | Gate 10 and canonical programme approvals are existing source decisions, not a new run.                                                                                                                                                      |
| Codex Review  | `/codex review`       | Independent 2nd opinion         | 0    | CLEAR BY EQUIVALENT READ-ONLY REVIEWS | Independent C1.3d review found and closed shared-runtime, Android-capability, two-start, PKCE-capacity, and shutdown-drain defects. Three continuation reviews then converged on the existing-row design. Platform evidence remains pending. |
| Eng Review    | `/plan-eng-review`    | Architecture & tests (required) | 3    | CLEAR                                 | The identity-first state machine, persisted confirmation, expiry/restart rules, cookie rotation, bounded candidates, final transaction, and lean test matrix are frozen in section 2.8.                                                      |
| Design Review | `/plan-design-review` | UI/UX gaps                      | 0    | —                                     | Existing approved Gate 10 A+C review and artifact hashes remain binding. Runtime design evidence stays in C1.4.                                                                                                                              |
| DX Review     | `/plan-devex-review`  | Developer experience gaps       | 1    | CONTRACT CLEAR; LIVE AUDIT DEFERRED   | The request/response, SDK, cookie, retry, Workbench host, copy, and no-display-name limitation are frozen. The skill's required live product audit runs against the ordinary-browser C1 surface.                                             |

**OUTSIDE REVIEW:** Read-only subagents support direct backchannel C1 plus
separate upstream hardening and identified the callback, association,
selection, and finalization gaps frozen above. After the user supplied the
signed-in zsh environment, AGY reviewed the exact C1.2 diff and returned clear.
For C1.3, the independent contract reviewer found and closed required-scope,
single-runtime, actor-provenance, problem-contract, native-cookie, and
RegisterNamespace policy gaps. Additive AGY challenged the corrected contract;
it did not replace the subagent review or an evidence gate.

**VERDICT:** C1 SOURCE IMPLEMENTATION COMPLETE LOCALLY; C1 DELIVERY GATES
PENDING; PACKAGED TAURI AUTHENTICATION DEFERRED. Do not claim packaged desktop
authentication, accessibility conformance, pull request, merge, release, or
deployment evidence before its exact gate passes.

NO UNRESOLVED DECISIONS
