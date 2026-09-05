# TrailBase upstream hardening plan

Status: `PLANNED — NO UPSTREAM ISSUE, PATCH, PR, OR ACCEPTANCE EVIDENCE YET`

Date: 2026-08-30

## 1. Boundary and decision

This is a separate upstream contribution programme for TrailBase. It is not a
Fasti C1 implementation plan.

Fasti C1 continues with the approved D3-C direct backchannel:

```text
TrailBase /token -> TrailBase /status -> TrailBase /logout
                  -> discard TrailBase tokens
                  -> issue an opaque Fasti session
```

None of the work below blocks C1. C1 can stop only if later exact source or
runtime evidence proves that this direct exchange is unsafe.

This programme must not:

- copy, patch, vendor, or build TrailBase source inside Fasti;
- make Fasti read TrailBase tables;
- make Fasti depend on an unmerged TrailBase branch;
- distribute a modified TrailBase artifact under the existing licence review;
- combine independent concerns into one upstream pull request; or
- claim maintainer agreement, test success, compatibility, or release support
  before evidence exists.

The smallest upstream patch that meets each accepted contract wins. Reuse the
existing TrailBase auth, session database, migration, generated-binding, and
client owners. Do not add a new auth framework, token service, cache, queue, or
Fasti-specific extension.

TrailBase's authentication domain owns these invariants. Keep one owner for
code consumption, one owner for refresh transitions, one shared account
eligibility predicate, and one key lifecycle. Protocol handlers and clients
project those rules; they must not duplicate or redefine them. This is the DRY
and domain-driven boundary for every package.

## 2. Evidence baseline

### 2.1 Exact release under review

Fasti pins the exact, unmodified TrailBase `v0.33.5` release at commit
`b4c85d5152d4e5f472e0b5da5303f7c938e3a083`. The local release lock records
native archive, executable, OCI graph, and licence digests. The existing
licence review covers only that exact, unmodified release running as a separate
process.

Sources:

- [Fasti release lock](../../third_party/trailbase/release.json)
- [Fasti licence review](../../third_party/trailbase/LICENSE-REVIEW.md)
- [TrailBase v0.33.5 source](https://github.com/trailbaseio/trailbase/tree/b4c85d5152d4e5f472e0b5da5303f7c938e3a083)
- [TrailBase tagged licence](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/LICENSE)
- [TrailBase security policy](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/SECURITY.md)
- [TrailBase contribution guidance](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/README.md#contributing)

A read-only check on 2026-08-30 resolved upstream `main` to
`e8fd53a798ada706cf95ebfba47b6220f2fc7a5f`. The inspected auth-code, refresh,
user, and key behavior remained materially unchanged there. That observation
does not reserve an upstream design or prove future applicability. Rebase and
repeat the source audit immediately before every issue or patch.

### 2.2 Verified gaps in v0.33.5

| Concern                     | Exact source evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                            | Observed limit                                                                                                                                                                                                                    |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Authorization-code exchange | [`auth/api/token.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/auth/api/token.rs#L42-L100) and [session schema](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/migrations/session/V1__initial.sql#L21-L33)                                                                                                                                                     | The handler selects a valid code, then mints a refresh session. It does not consume the code.                                                                                                                                     |
| Refresh                     | [`auth/tokens.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/auth/tokens.rs#L168-L270) and [`auth/api/refresh.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/auth/api/refresh.rs#L10-L49)                                                                                                                                                              | Refresh looks up and returns the same opaque refresh token while minting only a new auth token. There is no rotation, family, or reuse response.                                                                                  |
| Account lifecycle           | [`auth/user.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/auth/user.rs#L13-L34), [`admin/user/update_user.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/admin/user/update_user.rs#L20-L138), and [`auth/tokens.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/auth/tokens.rs#L207-L270) | The user model and update API expose no disabled or suspended state. Refresh checks existence and email verification, not an account lifecycle state.                                                                             |
| Claims, metadata, and keys  | [`auth/jwt.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/auth/jwt.rs#L39-L105) and [`admin/jwt.rs`](https://github.com/trailbaseio/trailbase/blob/b4c85d5152d4e5f472e0b5da5303f7c938e3a083/crates/core/src/admin/jwt.rs#L1-L15)                                                                                                                                                                                    | Auth tokens have `sub`, `iat`, and `exp`, but no issuer, audience, or key ID. One PEM public-key endpoint exists on the administrator surface. No public discovery, JSON Web Key Set, overlap, or retirement contract is present. |

Relevant standards:

- [OAuth 2.0 authorization-code requirements, RFC 6749 section 4.1.2](https://www.rfc-editor.org/rfc/rfc6749.html#section-4.1.2)
- [OAuth authorization-code security, RFC 6749 section 10.5](https://www.rfc-editor.org/rfc/rfc6749.html#section-10.5)
- [OAuth 2.0 Security Best Current Practice refresh protection, RFC 9700 section 4.14.2](https://www.rfc-editor.org/rfc/rfc9700.html#section-4.14.2)
- [OpenID Provider metadata](https://openid.net/specs/openid-connect-discovery-1_0.html#ProviderMetadata)
- [OAuth authorization-server metadata, RFC 8414](https://www.rfc-editor.org/rfc/rfc8414.html)
- [JSON Web Key Set, RFC 7517 section 5](https://www.rfc-editor.org/rfc/rfc7517.html#section-5)

These sources define review inputs. They do not prove that TrailBase accepts
the proposed design or that any package is implemented.

## 3. Contribution and licence gate

Before any code is written:

1. Recheck upstream `main`, open issues, discussions, pull requests, release
   notes, contribution guidance, and security policy.
2. Send replay-capable authorization-code findings through
   `security@trailbase.io`, as required by TrailBase's security policy. Do not
   publish exploit details before the maintainers choose a disclosure path.
3. For each non-security feature, discuss the bounded contract with the
   maintainers before implementation. TrailBase asks contributors to align on
   work beyond bug fixes. GitHub issue creation was restricted when this plan
   was written, so do not assume that Fasti can open an issue directly.
4. Confirm any then-current contributor agreement, sign-off, provenance, test,
   formatting, and generated-file requirements. None is assumed here.
5. Record the exact upstream base commit and the maintainer-approved scope for
   that package.

An upstream pull request publishes its patch in the upstream OSL-3.0 project;
it does not authorize Fasti to ship a modified build. Fasti may consume a fix
only after it is in an upstream release that Fasti separately pins and reviews.

Any Fasti-maintained patch, fork, preview binary, OCI image, or externally
deployed modified build requires, before build or distribution:

- a new written OSL-3.0 review;
- retained licence and notices;
- convenient public access to corresponding source;
- prominent modification and version attribution;
- reproducible build instructions and exact artifact digests;
- an owned update and security-response commitment; and
- tested activation and rollback.

The default is not to create or distribute such an artifact.

## 4. Delivery sequence

Each package has its own maintainer conversation, issue or private report,
branch, pull request, test evidence, compatibility note, and release decision.
Closing or rejecting one package does not close the others.

```text
U1 atomic code consumption
  -> U2 refresh rotation and reuse detection
  -> U3 account disable or suspend lifecycle
  -> U4 standards metadata and key rotation (consumer-gated)
```

Only U1 has priority over the other work. The ordering limits review scope; it
does not create code dependencies unless upstream maintainers request them.
Do not stack pull requests on unmerged branches. Start each accepted patch from
the then-current upstream default branch.

## 5. U1 — Atomic authorization-code consumption

### 5.1 Goal

Make an authorization code usable at most once, including concurrent exchange
attempts, without changing the external successful response shape.

### 5.2 Smallest acceptable design

Keep the existing `_authorization_code` table, unique index, PKCE validation,
token handler, and session store. Use one session-database transaction to claim
or delete the matching unexpired code and create the resulting refresh session.
A single conditional `DELETE ... RETURNING` is the preferred starting point if
TrailBase's supported database abstraction and SQLite/Postgres matrix can prove
it. Otherwise use the smallest transactionally equivalent existing primitive.

The implementation must preserve these invariants:

- a wrong PKCE verifier does not consume the code;
- an expired code cannot mint tokens;
- exactly one concurrent valid exchange can create a refresh session;
- the losing exchange returns the existing non-secret invalid-grant-style
  error surface selected by maintainers;
- a database failure cannot both preserve the code and leave a committed
  refresh session; and
- codes created by password, TOTP, and social callback paths share the same
  consumption owner.

Do not add a replay cache, distributed lock, new service, or new table. The
session database is already the consistency boundary.

### 5.3 Tests required before acceptance

- Existing password, PKCE, TOTP, and social authorization-code tests stay
  green.
- One successful exchange followed by a sequential replay produces one token
  response and one refresh-session row.
- Two synchronized exchanges of the same code and verifier produce exactly one
  success and one failure, with one refresh-session row.
- A wrong-verifier attempt leaves a later correct exchange usable until normal
  expiry; expired and malformed codes never mint tokens.
- An injected or controlled session-insert failure proves transaction rollback.
- SQLite and experimental Postgres coverage follows the upstream support
  policy at the implementation commit; unsupported backends are reported, not
  claimed.

No test is recorded as passed in this plan.

### 5.4 Compatibility and rollback

This is an intentional behavior correction: clients that replay a code lose
that behavior. The successful response and first exchange remain compatible.
The patch should need no schema migration.

Rollback is a normal binary rollback only if the accepted patch changes no
persistent schema or data contract. Prove that condition on the final diff.
Otherwise require a pre-change backup and restore procedure.

### 5.5 Independent sequence

1. Private security report with exact affected commits, bounded reproduction,
   concurrent test, and proposed invariant.
2. Maintainer acknowledgement and disclosure instruction.
3. One minimal upstream PR only after authorization.
4. Maintainer CI and review.
5. Upstream release and advisory or release note as maintainers decide.
6. Separate Fasti release qualification; never change the C1 pin implicitly.

## 6. U2 — Refresh rotation and reuse detection

### 6.1 Goal

Replace reusable refresh credentials with one-use rotation and a defined token
family response, consistent with the accepted TrailBase client model and RFC
9700 section 4.14.2.

### 6.2 Contract gate before code

Maintainers must first approve:

- whether every client is treated as public or whether sender-constrained
  tokens exist;
- the family identifier and retained spent-token evidence needed for reuse
  detection;
- family absolute expiry versus per-token expiry;
- the response when a spent token is presented;
- whether detected reuse revokes the whole active family;
- cookie refresh behavior; and
- the release strategy for official clients that currently retain the old
  refresh token.

Do not invent these as implementation facts. Record the accepted answers in
the upstream issue or discussion.

### 6.3 Smallest likely patch surface

Reuse `_session`, `reauth_with_refresh_token`, `/refresh`, `/status`, `/logout`,
generated bindings, and official client token stores. Add only the persistence
needed to identify an active token, its family, and spent-token reuse. Rotate
and invalidate in one session-database transaction. Return the new refresh
token from refresh-capable responses and make each official client replace the
old token atomically.

Do not add a general token ledger, event bus, background revoker, or cache.
Hash-at-rest is a separate decision unless maintainers include it in the same
accepted security contract; do not expand U2 silently.

### 6.4 Tests required before acceptance

- A valid refresh returns a new auth token and new refresh token; the old token
  cannot refresh again.
- Sequential reuse triggers the accepted family response and does not leave an
  undisclosed active descendant.
- Concurrent use of one refresh token has one deterministic winner; the loser
  follows the accepted reuse policy.
- Expired, revoked, logged-out, deleted-user, and malformed tokens stay denied.
- `/status`, header tokens, cookies, `/logout`, anonymous users, CLI minting,
  and every official client use the new token consistently.
- Restart, migration, backup, restore, and configured TTL behavior are proven.
- Generated API and client bindings match the final response schema.
- Logs, metrics, errors, and fixtures contain no refresh-token value.

### 6.5 Compatibility and rollback

Rotation is client-visible. An old client that ignores the returned replacement
will fail on its next refresh. Therefore the accepted upstream change must ship
server behavior, generated types, official client updates, release notes, and a
version-compatibility matrix together. Do not hide this behind a floating or
permanent compatibility mode.

Any session-schema migration needs forward-upgrade proof. Unless upstream
proves in-place downgrade, rollback means stop the new binary and restore a
pre-upgrade full-depot backup before starting the old binary.

### 6.6 Independent sequence

1. Maintainer discussion with the contract questions and v0.33.5 source links.
2. Maintainer-owned or maintainer-authorized issue.
3. One server-and-official-client PR because partial rotation is unsafe.
4. Separate documentation and release evidence if maintainers require it.
5. Upstream release.
6. Separate Fasti qualification and pin update after U1 qualification remains
   green.

## 7. U3 — Account disable or suspend lifecycle

### 7.1 Goal

Provide one explicit reversible inactive state for a human account and enforce
its documented effect on login, refresh, sessions, and protected access.

### 7.2 Contract gate before code

Start with one `disabled` state. Do not design a multi-state identity workflow,
reason taxonomy, scheduled suspension system, or policy engine unless an
upstream use case proves it.

Maintainers must decide and document:

- who can disable and re-enable normal users and administrators;
- whether disabling immediately revokes every refresh session;
- whether already-issued short-lived auth tokens are rejected immediately or
  remain valid for a bounded documented interval;
- what public login and refresh errors reveal;
- whether self-service deletion and email verification interact with the
  state; and
- what administrator-continuity guard applies.

### 7.3 Smallest likely patch surface

Reuse the existing user row, administrator user API, CLI-only administrator
boundary, session deletion helper, login paths, and refresh lookup. A
default-active column plus focused checks is preferable to a new lifecycle
aggregate. One shared predicate must own eligibility so password, TOTP, social,
anonymous, status, and refresh paths cannot drift.

If immediate auth-token rejection is accepted, implement it once in the shared
authenticated-user extraction boundary. Do not scatter route-local checks.

### 7.4 Tests required before acceptance

- Password, TOTP, social, authorization-code, status, refresh, and any
  anonymous-account path that resolves the same disabled subject follow the
  accepted denial contract.
- Disable revokes sessions atomically if that contract is selected.
- Existing auth tokens follow the exact immediate or bounded rule chosen by
  maintainers.
- Re-enable restores only the allowed sign-in path; it does not resurrect old
  refresh sessions.
- Non-admin and administrator control boundaries are denied as specified.
- Last-administrator and concurrent disable/demotion cases preserve the
  accepted continuity rule.
- Migration, restart, backup, restore, and old-data defaults are proven.
- Public errors do not create an account-enumeration regression.

### 7.5 Compatibility and rollback

Existing accounts must migrate to active. The new field and API are additive,
but runtime enforcement changes behavior for disabled accounts. Generated
admin types and documentation must ship with the server change.

Treat an irreversible or old-binary-incompatible schema change like U2:
rollback through a pre-upgrade full-depot restore. Do not assert that an old
binary tolerates the new schema until a test proves it.

### 7.6 Independent sequence

1. Maintainer discussion defining the one-state lifecycle and propagation
   rule.
2. Maintainer-owned or maintainer-authorized issue.
3. One domain, API, enforcement, migration, and generated-client PR.
4. Upstream release.
5. Separate Fasti conformance addition and pin decision.

## 8. U4 — Standards metadata, claims, and key rotation

### 8.1 Entry gate

Do not implement U4 until a named consumer needs offline validation or
TrailBase chooses to act as an OpenID Provider. Fasti C1 is not that consumer:
it validates through direct `/status`, revokes through `/logout`, and discards
TrailBase tokens.

Before code, maintainers must choose the intended protocol profile. Publishing
an OpenID discovery document implies more than exposing a public key. Do not
claim OpenID Provider conformance from a metadata-shaped JSON response.

### 8.2 U4a — Claims and metadata contract

The maintainer-approved contract must define a stable issuer, intended
audience, endpoint URLs, supported signing algorithms, and the exact public
metadata surface. If the profile is OpenID Connect, validate every required
provider-metadata field and behavior against OpenID Connect Discovery. If it is
only OAuth authorization-server metadata, use the applicable OAuth metadata
specification instead.

The smallest patch should reuse `JwtHelper`, route construction, generated
OpenAPI ownership, and configuration. Add only claims the server can validate
and maintain. Every token type needs an explicit inclusion or exclusion rule;
do not add fields only to the auth-token struct and leave reset or pending
tokens ambiguous.

Required tests before acceptance:

- exact issuer and audience positive and negative validation;
- wrong-origin, proxy, path-prefix, and configuration cases;
- old-token/new-runtime and new-token/old-runtime compatibility where claimed;
- discovery or metadata schema and endpoint consistency;
- no private key, admin credential, or internal address disclosure; and
- generated OpenAPI, bindings, examples, and documentation drift checks.

### 8.3 U4b — JSON Web Key Set and rotation

U4b follows an accepted U4a contract. Convert the public verification material
to a standards-valid JSON Web Key Set, assign stable key IDs, and make token
headers select a key. Keep the private key private. The existing administrator
PEM endpoint is not a rotation API and must not become the public contract by
accident.

Maintainers must define generation, activation, overlap, retirement, backup,
restore, and compromise recovery before implementation. The minimal persistent
model is one active signing key plus only the retired verification keys needed
for the longest still-valid token. Do not build a general key-management
service or remote KMS adapter without a proven upstream requirement.

Required tests before acceptance:

- tokens issued before rotation validate throughout the declared overlap;
- new tokens contain the active key ID and validate with the published set;
- unknown, retired, malformed, wrong-algorithm, wrong-issuer, and wrong-audience
  tokens fail;
- restart, backup, restore, and concurrent rotation preserve one active signer;
- retirement cannot precede the maximum accepted token lifetime;
- public metadata and key responses have an explicit cache policy; and
- no private key enters responses, logs, fixtures, or source control.

### 8.4 Compatibility and rollback

Claims and key rotation change the external token contract. The upstream PR
must publish a compatibility matrix for tokens and runtimes across the change.
New claim validation must account for already-issued tokens or deliberately
invalidate them with a documented sign-in requirement.

Key-schema or file-layout changes require a pre-change backup. Rollback must
retain every key needed to validate tokens that the restored runtime still
accepts. A successful unit test is not backup-and-restore evidence.

### 8.5 Independent sequence

1. Named-consumer evidence and maintainer protocol decision.
2. One maintainer-owned or maintainer-authorized metadata/claims issue.
3. U4a PR and release, with conformance evidence appropriate to the chosen
   protocol.
4. Separate key-lifecycle design discussion and issue.
5. U4b PR and release.
6. Separate Fasti evaluation. C1 remains on direct backchannel unless a later
   approved Fasti package has a real offline-validation need.

## 9. Cross-package quality gates

Every upstream patch must include, at the exact proposed commit:

- the maintainer-approved contract and exact base commit;
- the smallest focused regression test that fails on the old behavior;
- the relevant existing TrailBase auth and client suites;
- generated-file and formatting checks required by upstream;
- database migration, restart, backup, restore, and rollback evidence when the
  patch changes persistence;
- a concurrency test for any one-use credential transition;
- bounded input, error, log, and secret handling;
- no material unbounded memory, retry, row-growth, or background-task owner;
- documentation and release-note changes for behavior or wire changes; and
- an explicit list of untested backends or clients.

Performance evidence is proportional to the patch. U1 must prove no unbounded
retry or row growth. U2 must bound spent-token retention and indexed lookups.
U3 must avoid route-local N+1 lifecycle queries. U4 must bound retained keys and
metadata size. Do not claim performance improvement without measurement.

UI work is not planned. If upstream later adds administrator controls, use its
existing component system and require keyboard, focus, error, contrast, reflow,
and screen-reader evidence. Fasti's Gate 10 A+C design remains separate.

## 10. Fasti intake after an upstream release

An upstream merge is not a Fasti dependency update. For each adopted release:

1. Pin the exact tag, commit, native archives, executables, OCI manifests, and
   licence digest.
2. Update the written licence review for the new exact unmodified release.
3. Review upstream diffs from the last Fasti pin, including migrations and
   release notes.
4. Run TrailBase conformance, backup/restore, adjacent upgrade/rollback,
   resource, and C1 direct-backchannel tests on both supported architectures.
5. Record exact success and failure evidence. Do not inherit upstream CI claims
   as Fasti runtime evidence.
6. Deliver the pin update in its own reviewable Fasti pull request.

Until all six steps pass, Fasti remains on its current exact pin and C1 keeps
its D3-C controls.

## 11. Current ledger

| Package                    | Upstream alignment                   | Patch       | Tests   | Release | Fasti adoption |
| -------------------------- | ------------------------------------ | ----------- | ------- | ------- | -------------- |
| U1 atomic code consumption | Not started; private report required | Not written | Not run | None    | Not applicable |
| U2 refresh rotation/reuse  | Not started                          | Not written | Not run | None    | Not applicable |
| U3 account disable/suspend | Not started                          | Not written | Not run | None    | Not applicable |
| U4 metadata/key rotation   | Consumer gate not met                | Not written | Not run | None    | Not applicable |

No row in this ledger is acceptance evidence. Update it only from exact
upstream and Fasti artifacts.
