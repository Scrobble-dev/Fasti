# Fasti Access C2 implementation gate

Status: `FOUNDATION_MERGED_C2_RUNTIME_INTEGRATION_OPEN`

Delivery sequencing amendment (2026-09-05): the completed pure source may
land through the separate [foundation delivery gate](fasti-access-c2-foundation.md).
That PR does not close this plan, complete C2.1, activate runtime operations,
allocate a migration, or release M4's shared ownership. The gate pins the
source and requires fresh exact-head review and canonical regression checks.
Earlier test counts and reviewed migration numbers below are historical
evidence, not final foundation-head results or current migration allocations.

Recorded: 2026-08-31

Planning base commit: `171f1953e1ea552e044ea7e6e027353746d89156`

Planning base tree: `8d7ca18af7e2d6033e87f0c3e14f741fa53ae526`

Verified implementation base (2026-09-04): merged M3 `dev`
`df09101028a988a92f4546313c5eed6dd20d238a`, tree
`5552947a30b82497c7fa279a6932fe7877ed612b`; schema v15, archive v5.

Migration allocation (current coordination, 2026-09-05): M4 retains migration
v17 and archive v7. Neither is released to C2. C2 must use the next released
migration after metadata's verified handoff. References to v16 below are the
original reviewed migration proposal, not a current allocation. Reconcile all
migration examples and historical archive fingerprints before C2 storage work.

Owner: Commander / Mothership

## Current completion boundary — 2026-09-05

The pure domain/application foundation merged in [PR #125](https://github.com/Scrobble-dev/Fasti/pull/125)
at `62e10d2e9bd738ed5da425c008eb839f89cdbea5`. Its tree exactly matches the
reviewed head `90374622ec5bad52beabf9405835bba51b56dda5`:
`d6fcea1563b673f83cb4cabe1ef50d1c6dc5c087`. The foundation modules and C2
integration-test files remain unchanged through observed merged dev
`3d775bf7af2dd52fffafeaba24ceea22da1cfcc1`. This is source/merge readback,
not a fresh test execution or proof of complete C2 runtime support.

Independent completion audit found no additional foundation implementation
gap. PAT actors, C2 capability classification, transactional inventory,
PAT/consent persistence, caller cutover, contracts and the A+C UI remain
explicit C2.1–C2.7 work. The foundation merge does not satisfy those gates.
Do not rerun foundation implementation or infer a new migration allocation.
Before shared edits, obtain M4's exact merged commit/tree, migration allocation,
archive disposition and ownership release. Prepare source-backed transaction
and contract checks independently; do not invent a temporary authority model.

This checkpoint supersedes earlier current-state and pending-delivery wording
below. Historical test results, migration proposals and failed gates remain
preserved. C3 qualification does not complete C2; packaged Tauri authentication
remains deferred. No shared production file changes in this reconciliation.

### Parallel pure-test follow-up — 2026-09-05

Independent PAT/consent review identified two missing direct boundary checks,
not product defects: unused-token revocation exactly at creation (and one
nanosecond before), and replacement exactly at the predecessor's last-use time.
The bounded writer owns only `crates/fasti-domain/tests/c2_personal_tokens.rs`.
Reuse existing public methods and fixture helpers; preserve all model fields
except the expected terminal transition. Add no production method, mock store,
dependency or runtime authority. Run the focused PAT/consent domain and
application suites, formatting and strict Clippy; independently review the
test-only diff before committing it. M4 ownership and C2 runtime gates remain
unchanged. No failing-before-fix claim follows from new tests of existing code.

Completed locally: both tests passed on first execution. The focused domain
and application suites passed 33 tests with zero failed, ignored or filtered
cases. Workspace formatting and strict all-target/all-feature domain and
application Clippy passed. Independent source review found no concrete defect
in the two-test delta. These checks do not implement persistence or runtime
authority; those gates and the shared-file handoff remain open.

Historical foundation checkpoint (2026-09-05): implementation commit `5eb3def0` passes 319
all-feature domain/application tests and strict all-target Clippy. Independent
reconciliation confirms that sections 5.4 and 9 cover all 22 merged-M3 scopes
and its four new capabilities. M4's reserved `metadata_search` stays denied.
No usable C2 inventory store can be added against v15 alone: client ownership
and issuance fields, the real capability/port, and persistence must land
together after the shared-file handoff. No temporary authority model is added.

## 1. Result

C2 evolves the existing Fasti Access spine into governed application-client,
personal-access-token, profile-grant, scope, consent, and device-inventory
owners. It does not create a second client, grant, scope, or credential model.

M3 has merged. C2 continues the isolated domain slice while metadata retains
v16 and the shared persistence, registry/generator, API, SDK, host, Workbench,
and portability surfaces. The commander owns the new Access credential domain
leaf and its domain re-export. Preserve M3's canonical `ClientId` for connection
policies, immutable receipts, and published archive v5/v15 compatibility.

The immediate C2.1a slice implements shared bounded credential names and valid
client classifications, with pure tests. It adds no IDs, schema, dependencies,
routes, or authorization path. Subsequent C2.1 work must finish lifecycle,
policy, consent, and actor contracts before storage integration. This partial
slice is not C2 completion. The existing C2 security and delivery gates remain
binding, and packaged Tauri authentication remains deferred.

C2.1a verification on 2026-09-04: all 90 `fasti-domain` tests passed,
including three new tests covering name boundaries, control characters, and
all classification pairs. Independent read-only review found no concrete
defect in this slice. Persistence enforcement and runtime authorization have
not been activated or verified by these pure tests.

The next isolated slice implements `TokenPolicy` in the application Access
leaf, following `SessionPolicy`'s explicit construction pattern. It enforces
whole-day bounds, explicit client expiry, the 30-day PAT default, and checked
expiry arithmetic. All 126 application unit tests and the M3 routing
integration test passed with default features; strict clippy passed for both
domain and application crates. Independent review found no concrete defect
in the policy slice. Authentication must still enforce that a credential is
expired at its exact expiry instant when the lifecycle/store path is wired.

Next C2.1 domain gate: implement `ApplicationClient` with the existing
`ClientId`, workspace, optional historical human owner/name, immutable
classification, current credential epoch, creation time, and terminal lifecycle.
New C2 registrations require a person and name and start at epoch 1. Rotation
rejects revoked, node, and ownerless clients and must detect SQLite-integer
epoch overflow before mutation. Revocation is terminal and preserves the final
epoch. Focused tests must exercise historical ownerless shells, every rejected
rotation, overflow without mutation, and repeated revocation. The eventual
store transaction still owns authorization, consent, all credential/grant
writes, audit, and administrator continuity; this model does not authorize them.

The parallel application-leaf slice adds the specified `fasti_pat_` parser and
domain-separated SHA-256 digest using `SecretMaterial`, `Sha256Digest`, and
the installed SHA-256/zeroization dependencies. The secret wrapper must not
implement Debug, Clone, or serialization. Test exact prefix/length/lowercase
bounds, all invalid ASCII suffix bytes, Unicode inputs, round-trip encoding,
and an independently computed fixed digest. No route accepts this secret until
the approved transaction-level PAT validation and actor matrix are integrated.

This client-lifecycle/PAT-secret slice is implemented and independently reviewed.
Verification: 94 domain unit tests, 128 application unit tests, and the one
default-feature M3 application integration test pass with the locked offline
dependency graph. Focused Access coverage is 7 domain and 5 application tests.
All-target domain/application Clippy with warnings denied, workspace formatting,
and whitespace checks pass. The independent reviewer found no concrete defect
in either slice; Ponytail review found no new dependency or duplicate owner to
remove. These results prove the pure slice only, not C2 runtime authorization,
store integration, UI, or package delivery. No shared M4 surface changed.

Next C2.1 gate: model registered-client credential identity, digest, positive
epoch, explicit creation/expiry, and terminal revocation in the same domain
leaf. Preserve historical/node credentials with no expiry; only confidential
legacy credentials require review. New human issuance requires the existing
explicit application `TokenPolicy` expiry check. Domain validity checks must
reject future creation, the exact expiry instant, revoked clients/credentials,
wrong client identity, and stale epochs. Credential/grant rotation stays one
future store transaction. Reuse `CredentialId` and `Sha256Digest`; the adapter
must retain the existing bare-hex SHA-256 storage spelling, not rehash it or
store PATs in `credentials`. Tests must prove monotonic, idempotent revocation
and historical credential preservation. Shared integration remains with M4.

Registered-client credential slice verification (2026-09-04): all 97 domain
unit tests, 128 application unit tests, and the default-feature M3 integration
test pass, including ten focused Access domain tests. All-target Clippy with
warnings denied, workspace formatting, and whitespace checks pass. Independent
review found no concrete correctness/security defect. Ponytail review retains
one aggregate, existing IDs/digest types, one shared issuance guard, and derived
expiry/revocation; no new dependency or persistence state is needed. Store
mapping must check persisted status against `revoked_at`; inventory must label
legacy confidential credentials `review_required` without invalidating them.

Additive secret-owner review found two outstanding C2 security prerequisites:
`SecretMaterial::Drop` uses an ordinary fill that an optimized Rust 1.96
reproduction removed, and SHA-256's secret-containing block buffer does not
enable the installed crate's `zeroize` feature. The shared owner has been
asked for isolated fixes: volatile zeroization through the existing zeroize
dependency and the existing SHA-256 zeroize feature. Do not create another
secret owner. Verify optimized retained erasure and the resolved feature in
addition to functional tests. PAT transport strings already use `Zeroizing`;
this does not prove erasure of raw bytes, hasher buffers, caller-owned inputs,
or serialized responses. C2 security delivery remains open until these
shared-owner changes and downstream exposure checks are verified.

Shared fixes are now imported: ID source `f46e5de8` as local `e1bfcdec`, and
secret-erasure source `fee9059d` as local `b2503cdf`. The latter keeps the
existing secret owner, uses volatile zeroization in Drop, and enables SHA-256's
installed `zeroize` feature with only two lockfile dependency edges. Four
focused secret tests pass and the resolved feature tree includes `zeroize`.
The supplied optimized owner IR contains retained byte-wise volatile zero
stores. Context7 had no matching feature documentation; pinned sha2 0.11.0
Cargo/source and the compile-time `Sha256: ZeroizeOnDrop` assertion establish
the feature contract. This closes the two owner-level findings, not the
downstream plaintext-exposure audit or C2 security delivery.

Next C2.1 PAT domain gate: implement the frozen PAT identity and immutable
workspace/subject/profile-grant binding, digest, name, captured subject epochs
and TrailBase installation/generation, plus expiry, last-use, and terminal
revocation/replacement. Reuse the current subject and installation models.
Issuance requires their active current states; application policy and the
transaction still own recent authentication, membership, grants, and exact
`ScopeKey` intersection. Domain code must not invent scope strings or import
application types. Test exact expiry, every identity/epoch/generation mismatch,
blocked/reactivated installations, non-active subjects, monotonic time, and
replacement that cannot revive an old token. A new replacement keeps the same
workspace/owner/grant, has a different ID and digest, and is newly created at
the rotation instant. Expired but unrevoked tokens may be replaced after fresh
authorization; revoked tokens are terminal. Import the shared ID commit before
compiling and committing this slice; no schema or runtime path is activated.

PAT domain slice verification (2026-09-04): 99 domain unit tests, six
independent public-API PAT test groups, 129 application unit tests and the
default-feature M3 integration test pass. Strict all-target domain/application
Clippy, workspace formatting and whitespace checks pass. A separate source
review found no concrete defect. The tests cover expiry, time/epoch ceilings,
identity and generation mismatch, installation reactivation, monotonic use and
revocation, fresh-authority replacement of an expired predecessor, and rejected
replacement without partial mutation. The shared concrete-ID JSON validation
fix `8657a912` is imported as `ca1efa80`; deserialization now uses each type's
existing `FromStr` and its all-prefix matrix passes. No plaintext is stored in
the PAT domain model. These are domain/integration-leaf results, not proof of
the still-unimplemented PAT operation transaction, API, UI or package gates.

Consent domain slice verification (2026-09-04): 99 domain unit tests, five
independent consent test groups, six PAT test groups, 129 application unit
tests and the default-feature M3 integration test pass (240 total). Strict
all-target domain/application Clippy, workspace formatting and whitespace
checks pass. Independent source review found no concrete defect; Ponytail
review found no unnecessary owner or abstraction. Consent successors preserve
their original binding and prior revision, reject invalid sequences and
backward time, and cannot restore withdrawn authority. These pure checks do
not prove current-revision CAS, persisted history, profile ownership, exact
scope approval or atomic grant/audit writes; those remain transaction gates.

Browser administration request slice verification (2026-09-04): all six
focused application Access tests and strict all-target application Clippy pass.
The new envelope retains the existing browser command without copying secrets
or adding identity claims. Compile-time checks reject generic actor/credential
conversions, Debug, Clone, Default and serialization. Independent correctness
and Ponytail reviews found no concrete finding. This request type does not
establish recent authentication or activate a route.

Registration input and scope evidence slice verification (2026-09-04): 130
application unit tests, five independent registration test groups, the
default-feature M3 integration test, 99 domain unit tests, five consent groups
and six PAT groups pass (246 total). Strict all-target domain/application
Clippy, workspace formatting and whitespace checks pass. Independent source
and Ponytail reviews found no concrete defect or needless abstraction. Fixed
scope-digest vectors were checked with Node's SHA-256 implementation, including
empty withdrawal evidence and non-lexical canonical order. Expiry regression
tests preserve in-range fractional timestamps and reject one-nanosecond bound
violations without rounding. No new dependency, public route, generated
contract, shared persistence edit or runtime authorization was added.

PAT command slice verification (2026-09-04): six new independent public-API
test groups pass; the complete default-feature domain/application run is 252
tests. Strict all-target Clippy, workspace formatting and whitespace checks
pass. Separate source and Ponytail reviews found no concrete finding. Tests
cover retained browser evidence, exact targets, empty scopes, deferred default
expiry, unchanged explicit expiry, execution-time minimum crossings, custom
policy and date overflow. The private transaction still must verify current
authority, preserved scopes and the replacement/revocation race; these command
constructors do not implement those checks or make PATs usable at runtime.

Registered-client lifecycle request slice verification (2026-09-04): four new
independent test groups pass, bringing the default-feature domain/application
run to 256 tests. Strict all-target Clippy, workspace formatting and whitespace
checks pass. Independent source and Ponytail reviews found no concrete finding.
Tests preserve exact client/consent targets and both epoch boundaries, reject
empty scopes and invalid expiry, retain custom policy and browser evidence,
and enforce prohibited secret traits/conversions. This is request-contract
evidence, not proof of the still-pending compare-and-swap or revocation transaction.

Next C2.1 inventory-input gate: reuse `BrowserSessionQuery` and
`ValidatedBrowserReadBoundary` for read-only browser evidence. One generic
`AccessInventoryPage<Id>` owns the bounded limit and optional timestamp/typed-ID
pair for client, PAT, consent and device inventories; one query envelope owns
that page and the browser request. Default 32, reject zero or above 100, and
reject either half of a cursor. Normalize timestamps to UTC microseconds with
the installed Chrono 0.4.45 `SubsecRound::trunc_subsecs(6)`, matching the store's
RFC 3339 microsecond formatting without collapsing leap seconds. Context7's
current-source result was checked against pinned `src/round.rs`; its generated
trait synopsis uses different names and is not API authority.

Verify exact bounds, paired inputs, offset/pre-epoch/leap-second precision,
retained browser evidence and prohibited secret traits/conversions. These
types neither authenticate requests nor authorize inventory. Ownership-first
SQL filtering, descending keyset ordering, bounded `limit + 1` retrieval and
generated API/SDK/UI integration remain shared-owner delivery gates. No CSRF
or recent-auth mutation envelope is required merely to inspect inventory.

Inventory-input verification (2026-09-04): five independent test groups pass;
the default-feature domain/application run now has 261 passing tests. Strict
all-target Clippy, formatting and whitespace checks pass. Independent source
and Ponytail reviews found no actionable finding. Cursor tests include every
accepted limit, both incomplete pairs, endpoint-specific IDs, pre-epoch and
offset inputs, leap-second preservation and date extremes. Runtime inventory,
SQL ownership filtering and API/SDK projections remain unimplemented here.

Next C2.1 consent-command gate: `GrantAccessConsentCommand` takes the existing
browser administration request, the expected current `AccessConsentRevisionId`
and a nonempty exact scope set. `RevokeAccessConsentCommand` takes that request
and the expected revision. Derive client, owner, workspace and profile/grant
from the persisted revision, not independent caller identities or a newly
selected browser profile. This request shape is an implementation freeze;
the unpublished POST route did not previously prescribe one.

First consent remains atomic with client registration or legacy secret
rotation. Do not introduce a second standalone first-grant path. Empty grant
sets direct the caller to revocation. The future transaction compares the
target against the current revision, rejects stale requests without changing
newer consent, permits current-revoked revocation as a no-op, and rejects
revoked-to-granted transitions. Test exact target/scope/browser retention,
empty-set rejection and prohibited secret/actor traits. These input owners do
not implement recent authentication, CAS or grant/audit persistence.

Consent-command verification (2026-09-05): two independent test groups pass;
all 263 default-feature domain/application tests, strict all-target Clippy,
formatting and whitespace checks pass. Independent source and Ponytail reviews
found no actionable finding. Exact revision and canonical scopes are retained;
empty-grant errors direct callers to revoke. Secret traits and generic actor
conversions remain prohibited. This proves the input slice only; shared-store
integration and complete C2 delivery gates remain open.

All-feature follow-through at `75f8ea25b9744dee30f0fb33ed377b5d24fefbca`:
`cargo test -p fasti-domain -p fasti-application --all-features --locked
--offline` passes 312 tests, including the existing feature-gated conformance
fixtures. All-target, all-feature Clippy with warnings denied also passes.
This broadens source regression evidence; fixture results do not activate a
production capability or substitute for the full C2 PR/runtime gates.

TrailBase remains the human-account platform. C2 does not reopen framework
selection, change the C1 direct backchannel trust profile, accept TrailBase
tokens as Fasti authorization, weaken Secure cookies, or claim packaged Tauri
authentication.

### Next integration boundary (2026-09-05)

M4 retains shared ownership and its unfrozen v16 migration. A store leaf alone
cannot supply full C2 inventory from v15: client ownership/classification and
credential expiry are absent, as are PAT/consent tables. Profile-grant access
does not prove creator ownership. The smallest real client-inventory slice
requires the allocated migration, exact capability/registry projection, real
application port and a new store leaf together. Do not add an interim legacy
inventory port or label `RotateCredential` as a C2 inventory capability.

The next disjoint input/output gate adds two concrete issuance results shared
by creation and rotation. `IssuedAccessClientCredential` retains the existing
client, registered credential, consent revision, canonical scope set and
`SecretMaterial`; `IssuedPersonalAccessToken` retains the existing PAT, scope
set and PAT secret. Validate structural identity/epoch/consent/digest matches,
nonempty scopes, explicit client expiry, and fresh unrevoked secret state.
An older still-current consent may accompany a rotated client secret; it must
not precede client creation or follow credential creation. Independent source
review identified the missing lower bound for rehydrated consent, so both
inclusive boundaries join the regression gate. Do not copy domain fields into
another authoritative model.

Results expose borrowed metadata and consume themselves to transfer secret
ownership. They have no Debug, Clone, Default, serialization or generic actor
conversion. Reuse the installed hash/zeroization owners and the unchanged
client SHA-256 algorithm. The future store constructs the result before commit,
commits mutation and audit, and returns it only after commit succeeds. These
structural checks are not proof of persistence, current authority, single network
delivery or erasure of serialized responses. The API still owns response-only
secret fields and `private, no-store`; lost plaintext responses are never replayed.

Issuance-result verification (2026-09-05): seven independent public-API test
groups pass, including exact model/secret transfer, both digest domains,
binding/state mismatches, empty scopes and consent-time bounds. All 319
domain/application tests with all features and strict all-target/all-feature
Clippy pass; formatting and whitespace checks pass. Independent review's
consent-time finding is fixed and rechecked; Ponytail review found no additional
complexity finding. No route, transaction, migration, secret persistence or
packaged desktop behavior was activated by these results.

## 2. User outcomes

After C2 and its required recent-authentication dependency are active, a person
can:

- inspect registered Fasti clients, their type, purpose, owner, grants, scopes,
  last use, expiry, and state;
- register a confidential integration client without creating a password or
  external-service credential;
- see a confidential client secret once, rotate it, and revoke it;
- create a profile-scoped personal access token, see it once, rotate it, and
  revoke it;
- review and revoke exact profile-and-scope consent;
- inspect and revoke connected device clients without confusing inventory with
  OAuth Device Authorization.

The permanent owner is Gate 10 A, `Settings -> Account and security -> Devices
and clients`. Gate 10 C may link to the same owner during first-run setup. Gate
10 B supplies the reusable evidence/detail pattern and never becomes another
destination.

Until a source-backed recent-authentication method is available, sensitive
create, rotate, grant, consent, and revoke controls remain visible with the
exact `recent_authentication_required` reason and next action. C2 does not use
session age, callback time, a TrailBase enrollment flag, or a second password
prompt as fake recent authentication.

C2 therefore ships its production inventory read-only until that dependency is
active. Tests inject recent-auth state only through direct deterministic store
fixtures. No fixture capability, development route, magic header, or dormant
production bypass is added.

## 3. Scope boundary

### 3.1 In C2

- evolve `clients` as the `ApplicationClient` persistence owner;
- evolve `credentials` for registered confidential-client secret epochs only;
- keep `profile_grants`, `grant_scopes`, and `ScopeKey` canonical;
- add a subject-owned `PersonalAccessToken` lifecycle and digest-only table;
- add durable consent evidence that commits with the exact grant/scope change;
- project clients whose purpose is `device` into a connected-device inventory;
- add exact client/PAT/consent audit events and targets;
- publish fixed-origin browser Access routes, OpenAPI, generated contracts,
  generated SDK methods, host bindings, and Tabler-first UI;
- add deterministic migration, restore, concurrency, security, API, UI, and
  exact-head evidence.

### 3.2 Not in C2

- OAuth authorization codes, access tokens, refresh families, introspection,
  revocation endpoints, or RFC 8628 device/user code polling: E2 owns these;
- generic OpenID Connect or Authentik: E1, E3, and E4 own these;
- passkeys and recovery codes: D owns these after its RP/origin, account
  selection, and TrailBase lifecycle blockers are resolved;
- provider/service credentials, keyring, encrypted vault, or encrypted backup:
  C3 and F own these;
- Nuvio pairing and synchronization: H owns these after C2, E2, G, F, and the
  required upstream work;
- packaged Tauri authentication: `C1-TAURI-AUTH` remains deferred;
- AsyncAPI without a real event transport;
- JSON-LD for private credential or security state.

## 4. Existing owners and required evolution

| Concern | Existing owner | C2 rule |
| --- | --- | --- |
| Client identity and lifecycle | `clients` | Evolve this table. Do not add an OAuth-client or device-client table. |
| Client secret epochs | `credentials` | Keep digest-only registered-client secrets here. Never store PATs or provider secrets here. |
| Profile authorization | `profile_grants` | Consent creates or changes this canonical grant in the same transaction. |
| Scope mapping | `grant_scopes` and `ScopeKey` | Add a generated delegability classification. Do not add adapter-local scope strings. |
| Secret material | `SecretMaterial`, `random_secret`, `digest_secret`, constant-time comparison | Reuse these owners. Add only a PAT type prefix at the transport boundary. |
| Browser authorization | C1 browser session, CSRF, membership, subject/profile grants, auth and authorization epochs | Every browser mutation rechecks these facts in its transaction. |
| Recent authentication | C1 `RecentAuthentication` | Required for sensitive mutations. Do not infer it from session freshness. |
| Audit | v14 `access_audit_events` | Rebuild forward in v16 only if required to extend checked event kinds/targets. Preserve all C1 rows. |
| UI | C1 Account and Security, API-client panel, truthful PAT/device unavailable states | Mature and compose these. Do not add a second Settings destination. |

## 5. Ubiquitous language and invariants

### 5.1 ApplicationClient

`ApplicationClient` is one Fasti application actor registered inside one
workspace. It is not a person, TrailBase account, media profile, governed
`Connection`, external OpenID provider, metadata provider, or service
credential.

It owns two independent classifications:

- authentication type: `first_party` or `confidential` in C2;
- purpose: `node`, `cli`, `device`, or `integration`.

Confidential clients may have one active credential epoch. `first_party/node`
preserves the current node bootstrap credential and cannot be browser-created
or converted. Status remains `active` or terminal `revoked`; revocation cannot
be undone. E2 adds public clients together with exact redirect validation,
Authorization Code with Proof Key for Code Exchange, and the protocol state
that makes a secretless client usable. C2 adds no dead public-client or redirect
schema.

Migration classification is deterministic:

- `node_state.client_id` becomes `first_party/node`;
- every other pre-v16 client becomes `confidential/integration` because its
  existing usable authority is an opaque client credential;
- no timestamp, display name, or row order is used to infer type.

Client display names are bounded user-visible labels, not identifiers. A
nullable creating `AuthSubjectId` records human ownership where one exists.
System/bootstrap clients remain operator-owned and have no invented person.
Client and PAT names share one semantic `AccessCredentialName`: trim leading
and trailing whitespace, then require 1 to 128 UTF-8 bytes and reject NUL,
control characters, carriage return, line feed, and Unicode bidirectional
formatting controls. The bound matches Fasti's existing 128-byte namespace-label
convention without coupling the bounded contexts. Names are neither normalized
nor unique because they are display labels, not identifiers.

### 5.2 Registered-client credential

Registered-client secrets stay in `credentials`. The database stores only the
digest. One successful response shows the plaintext once. Rotation creates a
new epoch and revokes the old epoch in one transaction. Revocation also revokes
the affected grant/client authority under the approved command semantics.

C2 does not add automatic secret expiry to the existing node credential. A
human-created confidential client receives an explicit expiry no later than
365 days after creation. Existing non-node credentials retain their current
state but appear as `review_required` until rotated into the C2 policy.
Rotating a legacy subject-owned client requires explicit current consent and
creates its first immutable consent revision in the same transaction. C2 does
not create or rotate ownerless/system clients; administrators can inventory and
revoke those existing operator resources without inventing human consent.

Next isolated client lifecycle command gate (2026-09-04): rotation carries the
browser administration request, typed client target, expected current credential
epoch, expected consent revision, exact displayed scope set and explicit
expiry. The expected epoch must admit a next SQLite integer epoch; it is not
an independently asserted actor epoch. Compare it to the current client epoch
inside the transaction, following the existing credential-rotation CAS pattern.
Without this precondition, two queued rotations can both succeed and silently
invalidate the first response. `None` for the expected consent revision means
expect no current revision for a legacy client, not permission to skip CAS.

Rotation preserves the current grant/profile and requires the submitted
nonempty scopes to equal the current scope set and satisfy current human and
audience authority. Existing consent must match the supplied revision; a legacy
client's first consent is created atomically with rotation. Scope changes use
the separate consent operation. The transaction permits only human-owned CLI
and integration clients here; device-purpose clients remain inventory/revoke
only in C2. Client revocation carries only the browser request and typed target,
revokes all current client authority and remains terminal/idempotent. Do not
conflate client revocation with the old single-credential request. Test intent
retention, epoch ceilings, empty scopes, policy expiry, distinct typed targets
and absence of secret traits; actual CAS/race/consent checks remain store gates.

### 5.3 PersonalAccessToken

The C2 implementation freezes `PersonalAccessTokenId` with resource prefix
`pat_` and executable ID lifecycle. This is a UUIDv7 resource identifier,
not the `fasti_pat_` bearer secret. The existing domain ID macro is the sole
registry; no parallel parser or identifier registry is added.

A `PersonalAccessToken` is subject-owned Fasti authority for CLI or automation.
It is not a client secret and does not live in `credentials`.

It contains:

- a typed PAT ID;
- workspace, owning `AuthSubjectId`, selected profile grant, and exact scopes;
- a domain-separated digest of 32 random bytes;
- name, created time, expiry, optional last-use time, state, replacement link,
  and revoked time;
- subject authentication and authorization epochs, TrailBase installation ID,
  and activation generation captured at issuance.

The transport renders one secret as `fasti_pat_<64 lowercase hex characters>`.
Storage keeps `SHA-256("fasti-pat-v1:" || raw_32_bytes)`. Registered-client
secrets retain the current `SHA-256(raw)` contract because their unrecoverable
digests cannot be rehashed during v16. PATs are already isolated by their fixed
transport prefix and separate table, so C2 adds no client digest-scheme column
or dual lookup. Parsing strips the PAT prefix, reuses `SecretMaterial`, and never
logs the supplied value. Missing, malformed, unknown, revoked, expired,
stale-epoch, and wrong-scope tokens return the same authentication outcome.
Authorization requires the current TrailBase installation ID and activation
generation to equal the pair captured by the PAT. Rotation captures the current
pair; reactivation at a later generation never revives an older PAT.

`pat_scopes` is the issuance-time upper bound, not another authorization owner.
Runtime authorization uses the intersection of the PAT's stored scopes and the
owning subject's current active profile-grant scopes. Grant narrowing therefore
takes effect on the next request without rewriting or expanding the token.

Rotation is a one-way replacement. It returns a new secret once and revokes the
old token at the same commit. A lost success response does not permit replay of
the plaintext. The durable row and audit event make the outcome inspectable;
the user must rotate again after recent authentication.

Next isolated C2.1 PAT command gate (2026-09-04): create accepts the existing
browser-only administration request, bounded name, nonempty canonical scope
set and optional absolute expiry. Rotate accepts that request, one typed PAT
target and optional expiry; revoke accepts only the request and typed target.
No caller supplies a subject, workspace, profile grant, epoch or installation
identity. Create derives current authority; rotate preserves the predecessor's
name, grant binding and issued scope upper bound while rechecking current
authorization and capturing current epochs/activation. Rotation does not
accept a replacement scope set or rename. A revoked predecessor stays terminal.
The whole preserved scope set must still be allowed by the current grant and
current PAT audience/actor policy. If it is no longer allowed, rotation rejects
without changes and directs the person to create a new token with narrower
scopes. Do not silently intersect the issuance set or expand current authority.

Keep optional expiry as request intent. Validate against request time for early
feedback, then call the same `TokenPolicy::pat_expiry` at transaction execution:
an omitted expiry starts the configured default from actual issuance time,
whereas an explicit expiry is never extended or rounded. Do not copy a default
computed at request receipt into persisted issuance. Add no port until its real
store implementation can be connected. Tests must prove both expiry paths,
empty-scope rejection, exact typed targets, retained browser evidence and no
Debug/Clone/serialization or generic-actor conversion. These constructors do
not authorize operations, mint secrets, write audit or activate HTTP methods.

### 5.4 ProfileGrant, scope, and consent

The C2 implementation freezes `AccessConsentRevisionId` with prefix `cnr_`
and executable ID lifecycle. Each ID names one immutable consent revision;
there is no second stable consent ID. The route parameter `consent_id` names
that revision. Its grant/client/profile binding identifies the existing
authorization owner. Mutation must compare the supplied revision to the
current revision inside the authorized transaction. A stale revision returns
a conflict and changes nothing; it must not revoke or overwrite newer consent.
Revoking a current granted revision appends a revoked revision and updates its
grant/scopes atomically. Revoking the current already-revoked revision is a
no-op, not another audit/revision append. The UI refreshes after a conflict.
These new assignments were frozen during implementation, not inferred as
previously published contracts. Metadata's shared-registry writer integrates
the two disjoint ID entries; C2 stays read-only on that file until handoff.

C2 consent transition freeze: first grant creates revision 1; narrowing or
expansion appends a granted successor; withdrawal appends a revoked successor;
revoked-to-revoked is an application no-op. A revoked consent chain does not
re-grant its old client/profile authority in C2. The safe next action is fresh
client registration and consent, not reactivation of old credentials. There is
no existing production re-grant operation to preserve. Later protocol work must
explicitly define fresh credential issuance/invalidation before adding such a
transition. This is a new implementation decision, not a claim about old SQL
constraints or test-only reactivation fixtures.

The next domain slice owns immutable consent revision identity, workspace,
client, owner, profile/grant binding, monotonic sequence/prior-revision link and
time. Its granted decision carries only the scope-set digest; revoked means
an empty set. Exact `ScopeKey` sets, delegability, canonical digest calculation,
current-revision compare-and-swap, grant writes and audit remain with the
application/store transaction. No second grant or scope owner is introduced.
Test initial-grant restrictions, immutable successors, every binding mismatch,
checked sequence overflow, self-links, time reversal and terminal withdrawal.

`ProfileGrant` remains the only client-to-profile authorization owner.
`grant_scopes` remains the only mapping of a grant to the generated
`ScopeKey` vocabulary.

Consent is durable evidence that one current subject approved one exact client,
profile grant, and scope set. It is immutable by overwrite: narrowing or
expansion creates a new consent revision and changes the canonical grant in
the same `BEGIN IMMEDIATE` transaction. Expansion is never inferred from a
client request and never exceeds both:

1. the approving subject's current authorized scopes; and
2. the generated delegability policy for that client type and purpose.

The bootstrap-only `client_enroll`, listener configuration, credential
administration, identity bootstrap, membership/role administration, provider
credential administration, workspace restore, and any later explicitly
human-only scope are non-delegable. The authored scope registry owns this
classification; adapters do not maintain allow lists.

Freeze every current `ScopeKey` by issuance audience. Capability-level accepted
actors remain a separate, stricter gate. `Yes` means the audience can request
the scope subject to the current grant; it does not make every operation using
that scope accept that actor.

| Exact current scope key | Confidential CLI | Confidential integration | PAT | C2 device issuance |
| --- | --- | --- | --- | --- |
| `client_enroll` | No | No | No | N/A |
| `profile_select` | No | No | No | N/A |
| `credential_manage` | No | No | No | N/A |
| `listener_configure` | No | No | No | N/A |
| `provider_credential_manage` | No | No | No | N/A |
| `capability_read` | Yes | Yes | No | N/A |
| `observation_accept` | Yes | Yes | No | N/A |
| `receipt_read` | Yes | Yes | No | N/A |
| `identity_write` | Yes | Yes | Yes | N/A |
| `identity_read` | Yes | Yes | Yes | N/A |
| `profile_state_read` | Yes | Yes | Yes | N/A |
| `profile_state_write` | Yes | Yes | Yes | N/A |
| `provider_read` | Yes | Yes | Yes | N/A |
| `metadata_claim_refresh` | Yes | Yes | No | N/A |
| `metadata_projection_read` | Yes | Yes | Yes | N/A |
| `metadata_projection_configure` | Yes | Yes | Yes | N/A |
| `review_read` | Yes | Yes | No | N/A |
| `review_write` | Yes | Yes | No | N/A |
| `correction_read` | Yes | Yes | No | N/A |
| `correction_write` | Yes | Yes | No | N/A |
| `workspace_export` | Yes | Yes | No | N/A |
| `workspace_verify` | Yes | Yes | No | N/A |

Device scope issuance is `not_applicable` in C2 because this package only lists
and revokes existing device-purpose clients. E2 owns device creation, approval,
and exact scope issuance. New scopes default to `No` for every audience. One
exhaustive generated test fails when any `ScopeKey` lacks an explicit value for
every C2 audience. PAT issuance additionally rejects every scope that is not
used by the exact C2 PAT actor set in Section 9.

C2 preserves the current fail-closed rule that one presented client credential
cannot ambiguously select between multiple active profile grants. A client
requiring another profile receives a distinct client/grant until E2 adds an
explicit token-time profile selection contract.

### 5.5 Devices

A connected device is an `ApplicationClient` whose purpose is `device`, plus
its current grant, consent, and last-use projection. C2 does not add a second
device identity table merely for inventory.

C2 can list and revoke a real device client. It does not expose a pairing code,
polling endpoint, device token, or simulated connection. Creation remains
unavailable until E2 can finish the OAuth Device Authorization transaction.
This is a truthful protocol dependency, not removal of the device surface.

## 6. TokenPolicy

C2 activates only the PAT and human-created confidential-client portions of
`TokenPolicy`. It does not scaffold E2 OAuth timings.

| Value | C2 contract |
| --- | --- |
| PAT minimum lifetime | 1 whole day |
| PAT default lifetime | 30 whole days |
| PAT maximum lifetime | 365 whole days |
| Confidential client-secret minimum | 1 whole day |
| Confidential client-secret maximum | 365 whole days |
| Confidential client-secret default | None; creation requires an explicit expiry |
| Non-expiring human-created credentials | Rejected |
| Clock | Explicit command time, UTC, exact boundary is expired |
| Configuration source | Explicit fixed C2 product policy constructed by the trusted host; no environment key or hidden `Default` |

Policy durations use whole days. An explicit absolute expiry may contain a
fractional-day interval; validate it against the exact inclusive duration
bounds without rounding. Validate again at trusted transaction execution time,
not only the earlier request timestamp. Section 7 records the source-backed
integration correction and its regression gate.

GitHub's current fine-grained PAT creation contract defaults to 30 days and
accepts 1 to 366 days. GitLab's current PAT guidance requires expiry and uses a
365-day normal maximum; its CLI defaults to 30 days. C2 uses the common 30-day
default and the stricter 365-day maximum:

- <https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens>
- <https://docs.gitlab.com/user/profile/personal_access_tokens/>
- <https://docs.gitlab.com/cli/token/create/>

`TokenPolicy` has no `Default`. It rejects zero, sub-day, over-maximum, and
inconsistent values. A future policy change requires a written gate and a
forward migration. It never extends existing credentials. A shortening
migration sets affected expiry to the earlier permitted instant and records
the affected count; it never resurrects an expired or revoked record.

## 7. Authorization and transaction boundary

Every browser mutation requires the fixed C1 browser Access runtime, exact
Host and Origin, Secure session and CSRF cookies, matching CSRF header, active
subject, active membership, current role, selected profile grant, current auth
and authorization epochs, and unexpired recent authentication.

C2 mutation commands accept only a dedicated `AccessAdministrationRequest`
that owns the existing `BrowserSessionMutationCommand`. This is browser request
evidence, not completed authentication. It has no generic access-context
conversion, independent caller-supplied subject, or plaintext serialization,
Debug or Clone implementation. Commands do not accept `RequestAccessContext`,
`ApplicationAccessContext`, a client credential, PAT, or the packaged host's
stored setup credential. The operation's store transaction constructs its
private verified administration proof only after all current-state checks pass.
Public `AuthenticatedBrowserSession` or `RecentAuthentication` value constructors
cannot substitute for those checks. With pinned TrailBase `v0.33.5`,
production cannot construct the recent-auth portion, so mutation routes remain
visible but unavailable and the packaged Tauri host remains inventory-only.
Direct store fixtures may construct deterministic proofs for tests; no runtime
fixture or bypass is compiled into a production target.

Source-backed C2.1 clarification: the existing browser mutation command keeps
session/CSRF secrets, correlation ID, request time and the validated Host/Origin
marker. The API already compares the two CSRF values and validates exact
Host/Origin; raw headers are not duplicated in domain/storage models. Reuse the
existing store browser and recent-auth helpers, reload authority in the same
operation transaction, and validate time at execution after queue delays.
An earlier request timestamp must not extend recent authentication. The next
application leaf slice adds only the browser-only envelope and compile-time
negative conversion/secret-trait tests. No new verification port or adapter
bypass is needed to make it compile. Shared-store integration supplies the
private transaction-local proof, trusted execution clock and current-state tests.

Next isolated application gate: `RegisterAccessClientCommand` owns that
browser request, a bounded credential name, a CLI/integration classification,
an exact requested scope set and explicit expiry. It accepts no caller-supplied
owner, workspace or profile-grant identity; derive them from current verified
browser state. Reuse `ApplicationClientClassification::for_registration` and
`TokenPolicy`; reject malformed input now and recheck expiry against trusted
transaction time before issuance. Preserve the existing machine credential
command/port until its callers can change atomically with shared integration.

One `AccessScopeSet` value canonicalizes typed `ScopeKey` inputs, rejects
duplicates and inputs larger than `ScopeKey::ALL`, and computes the specified
digest without a second string vocabulary. Empty sets remain representable for
revocation audit evidence; registration rejects them. The current canonical
order is explicitly `ScopeKey::ALL` order, the existing application vocabulary
checked by registry parity, not the lexical sort used by old store issuance.
There is not yet a generated global scope-order list. Shared integration must
project and verify this same order and audience classifications before use.
Tests must prove permutation-invariant digests, the empty digest, exact bounds,
duplicate rejection, rejected node/device registration, explicit expiry and
absence of secret/actor conversion traits. Structural scope validity never
implies delegability or authorization; those checks stay with the authored
policy and current-grant transaction. No shared registry file changes here.

Expiry integration correction (2026-09-04): independent source review found
that requiring an absolute expiry minus server time to be an exact whole-day
multiple rejects ordinary client timestamps and even a one-nanosecond queue
delay. Whole-day granularity applies to configured policy durations only.
Absolute client/PAT expiry must remain unchanged and fall within the inclusive
configured duration bounds at both request and transaction validation. Reuse
the shared `validate_expiry` helper; add no rounding, extension, tolerance or
clock abstraction. Preserve the omitted PAT default. Test an in-range fractional
duration and queue delay, plus one-nanosecond violations at both bounds. An
expiry that genuinely falls below the minimum before execution is rejected;
the user must select a later expiry. No runtime caller currently uses the new
policy, so this correction changes no issued credentials or active route.

Authority is frozen per resource and enforced again inside every transaction:

| Operation | Member | Administrator |
| --- | --- | --- |
| List clients, PATs, consents, devices | Own resources only | All workspace resources |
| Register client | Own subject-owned client | Own subject-owned client |
| Create PAT or grant consent | Own subject only | Own subject only |
| Rotate client secret or PAT | Own resource only | Own resource only; never another person's secret |
| Revoke client, PAT, consent, or device | Own resource | Any workspace resource |

Ownerless/system clients are administrator-only for inventory and revocation;
C2 does not create or rotate them. Ownership and workspace predicates are
applied before pagination ordering and `LIMIT`; filtering a page after retrieval
is forbidden.

Each create, rotate, revoke, grant, scope, or consent command:

1. parses and bounds input before storage;
2. starts one SQLite `BEGIN IMMEDIATE` transaction;
3. authenticates and authorizes the current browser session inside it;
4. loads current client, subject, membership, profile grant, and scope facts;
5. applies domain transition and secret generation as narrowly as possible;
6. writes the mutation and immutable audit evidence in the same transaction;
7. commits once;
8. returns plaintext only for the one successful secret response.

No transaction spans network input/output. No bearer credential satisfies
human recent authentication. Audit failure rolls back the mutation. Concurrent
mutations have one winner and deterministic terminal state.

The v16 audit vocabulary is exact:

| Event kind | Typed target evidence |
| --- | --- |
| `application_client_registered` | client ID, resulting credential epoch, consent revision ID, scope-set digest |
| `application_client_secret_rotated` | client ID, resulting credential epoch, current consent revision ID, scope-set digest |
| `application_client_revoked` | client ID, final credential epoch, current consent revision ID, scope-set digest |
| `personal_access_token_created` | PAT ID, scope-set digest |
| `personal_access_token_rotated` | replaced PAT ID, replacement PAT ID, scope-set digest |
| `personal_access_token_revoked` | PAT ID, scope-set digest |
| `access_consent_granted` | client ID, immutable consent revision ID, scope-set digest |
| `access_consent_revoked` | client ID, immutable revocation revision ID, empty scope-set digest |
| `connected_device_revoked` | device-purpose client ID, final credential epoch, current consent revision ID, scope-set digest |

Every row also requires the actor subject, workspace, correlation ID, and
occurrence time. A target owner subject is required except when an administrator
revokes a pre-v16 ownerless/system client or device shell. Those legacy rows
also omit consent revision evidence because C2 must not invent either a human
owner or consent. They retain the client ID, final credential epoch, and current
grant scope-set digest. An archive-restored shell has no grant or scopes, so its
administrator-revocation event uses the canonical empty-set digest. Exactly one
typed primary target is set; the PAT rotation replacement is its only permitted
secondary target. The
scope-set digest is SHA-256 over the canonical UTF-8 `ScopeKey` strings in
generated registry order separated by one newline; the empty set has the digest
of empty bytes. This is evidence, not an authorization owner. The event-specific
database checks reject missing, excess, or mismatched identifiers.

PAT authentication extends the existing `ApplicationAccessContext` with a
typed PAT actor. It does not synthesize a client, credential, or profile grant.
Digest matching yields only the typed PAT ID. The protected operation's own
transaction reloads and validates the PAT state, owning subject, membership,
role, selected grant, authentication and authorization epochs, expiry, authored
accepted actor, effective scope intersection, and current TrailBase activation
state/generation before using domain state. Revocation, grant narrowing,
membership change, or installation blocking therefore cannot race between
authentication and the mutation/read. The installation must be `active`;
physical-root mismatch, declared restore, release mismatch, or any other
blocked state rejects the PAT. Existing client-credential and browser-session
paths remain unchanged.

```text
Browser administration
  -> fixed C1 Access router
  -> cookie + CSRF + Host/Origin
  -> BEGIN IMMEDIATE
  -> subject + membership + selected grant + epochs
  -> recent authentication
  -> client/PAT/consent transition + audit
  -> commit
  -> one-time secret response when applicable

PAT application request
  -> data/integration route that explicitly accepts PAT actors
  -> parse fasti_pat_ + SecretMaterial
  -> digest lookup yields PAT ID only
  -> operation BEGIN IMMEDIATE
  -> reload PAT + active TrailBase installation + generation
  -> active subject + membership + role + selected grant + epochs
  -> PAT expiry + accepted actor + stored scopes intersect current grant scopes
  -> capability authorization inside the same transaction
  -> bounded conditional last-use update
```

```text
active client/PAT --rotate--> replaced/revoked --X--> active
       |                    \
       +--revoke/expire-----> terminal

consent revision N --narrow/expand/revoke--> consent revision N+1
       |                                      |
       +------ same transaction --------------+
                         |
                         v
                 canonical grant/scopes
```

Inline comments should keep only the non-obvious PAT authorization intersection
and audit-table rebuild sequence. The plan owns the broader diagrams; adapters
must not repeat them.

## 8. Migration v16 and restore

M3 is merged with `SCHEMA_VERSION = 15`; M4 now owns v16. Do not write or
reserve a C2 migration until M4's exact merged handoff allocates the next
version. Reconcile the historical v16 proposal below against that source;
preserve every published migration byte-for-byte.

The v16 migration must:

- add safe, deterministic `ApplicationClient` classification and metadata to
  `clients`, using defaults/backfill that keep every existing insert caller
  valid until authored callers are updated;
- enforce the 128-byte name bound with
  `length(CAST(name AS BLOB)) BETWEEN 1 AND 128`; do not use SQLite character
  length for a UTF-8 byte contract;
- preserve client status and credential epoch;
- defer public-client and redirect metadata to E2; C2 adds no unused OAuth
  redirect column or table;
- add PAT, PAT-scope, and consent-revision tables with same-workspace/profile/
  subject/client constraints;
- extend `credentials` with human-created client-secret expiry without changing
  the node credential or existing `SHA-256(raw)` digest contract;
- rebuild `access_audit_events` forward if its checked vocabulary or target
  columns change, copy every existing row, and recreate the retention index and
  immutable-UPDATE trigger; preserve retention deletion, do not add a DELETE or
  workspace-revision trigger, preserve the `sqlite_sequence` value, and prove the
  next inserted audit ID is strictly greater than the pre-migration maximum;
- update `restore_import.rs` and every direct client insert with deterministic
  C2 classification values rather than relying on column order;
- prove fresh v16 and exact v15-to-v16 schemas are identical;
- prove interrupted migration rollback, restart, and old-binary failure on a
  copied database.

Archive v4 is frozen. C2 must not add secrets, live grants, PATs, consent or
device authority to it. Restored client shells keep the archived lifecycle but
are non-authoritative: credential epoch `0`, no credential, grant, scope, PAT,
consent, subject, or session authority. C2 does not invent an `inactive` client
lifecycle. Copied PATs and client secrets fail after restore. M3 has created
archive v5; C2 must preserve its manifest, streams, DTOs, and exporter byte-for-byte.
When v16 becomes current, restore must retain an explicit historical accepted
case for archive format v5 at migration v15 using M3's exact handed-off schema
fingerprint. Any C2 archive evolution requires its own append-only version and
review. C3 owns the later authenticated encrypted backup and restore-generation
invalidation.

## 9. Public contract and routes

Add authored capabilities before generated outputs. The C2 registry surface
profile key is `c2_access_credentials`. Freeze the registry's singular noun and
operation grammar:

- `access.client.list`
- `access.client.register`
- `access.client.secret.rotate`
- `access.client.revoke`
- `access.personal_token.list`
- `access.personal_token.create`
- `access.personal_token.rotate`
- `access.personal_token.revoke`
- `access.consent.list`
- `access.consent.grant`
- `access.consent.revoke`
- `access.device.list`
- `access.device.revoke`

Do not keep aliases or a compatibility layer for the unpublished plural names.
Client and PAT create/rotate success DTOs expose the one-time plaintext only as
a response-only `secret` field marked `readOnly: true`. The schema-specific
validator exception is limited to those four success DTOs; the global Access
secret-field scan remains strict.

Freeze the C2 administration routes and generated methods:

| Method and route | Operation ID | SDK method | Retry |
| --- | --- | --- | --- |
| `GET /api/access/v1/clients` | `list_access_clients` | `listAccessClients` | safe |
| `POST /api/access/v1/clients` | `register_access_client` | `registerAccessClient` | never |
| `POST /api/access/v1/clients/{client_id}/secret-rotations` | `rotate_access_client_secret` | `rotateAccessClientSecret` | never |
| `DELETE /api/access/v1/clients/{client_id}` | `revoke_access_client` | `revokeAccessClient` | never |
| `GET /api/access/v1/personal-access-tokens` | `list_personal_access_tokens` | `listPersonalAccessTokens` | safe |
| `POST /api/access/v1/personal-access-tokens` | `create_personal_access_token` | `createPersonalAccessToken` | never |
| `POST /api/access/v1/personal-access-tokens/{token_id}/rotations` | `rotate_personal_access_token` | `rotatePersonalAccessToken` | never |
| `DELETE /api/access/v1/personal-access-tokens/{token_id}` | `revoke_personal_access_token` | `revokePersonalAccessToken` | never |
| `GET /api/access/v1/consents` | `list_access_consents` | `listAccessConsents` | safe |
| `POST /api/access/v1/consents` | `grant_access_consent` | `grantAccessConsent` | never |
| `DELETE /api/access/v1/consents/{consent_id}` | `revoke_access_consent` | `revokeAccessConsent` | never |
| `GET /api/access/v1/devices` | `list_connected_devices` | `listConnectedDevices` | safe |
| `DELETE /api/access/v1/devices/{client_id}` | `revoke_connected_device` | `revokeConnectedDevice` | never |

All C2 routes mount only with the fixed C1 browser Access router. Integration,
alternate-loopback, generic, wildcard, container-forwarded, and remote routers
do not gain C2 browser-administration endpoints. Existing machine credential
routes remain separate.

PAT bearer authentication is accepted only by data/integration operations whose
authored capability explicitly allows a PAT actor. Every browser administration
route under `/api/access/` rejects PAT and client bearer credentials before
handler execution. Read-only inventory uses the existing C1 validated browser
read boundary and rechecks the cookie session, current membership and resource
ownership inside the transaction. Mutations additionally use the C1 mutation
boundary, including CSRF and Origin checks, and require recent authentication
for sensitive operations. Inventory reads do not require the mutation/recent-auth
envelope; this preserves the read-only delivery disposition in section 2 and
the existing `AccessInventoryQuery` contract. No browser trust check is bypassed.

Add one governed `AcceptedActorKind` set with exactly
`registered_client_credential`, `browser_session`, and
`personal_access_token`. Generate exact equality into application policy, the
registry, capability DTOs, SDK operation metadata, OpenAPI
`x-fasti-accepted-actors`, security alternatives, and OKF. Keep the existing
coarse `AuthorizationKind`; do not add combinatorial authorization strings or
infer PAT acceptance from scope delegability. Webhooks remain registered-client
only.

The initial PAT actor set is exact and limited to current data operations:

- `identity.record.create`, `identity.identifier.attach`,
  `identity.record.list`, and `identity.namespace.register`;
- `profile.record.tracking_disposition.list` and
  `profile.record.tracking_disposition.set`;
- `profile.nuvio_collections.get`, `profile.nuvio_collections.replace`, and
  `profile.nuvio_collections.clear`;
- `provider.list` and `provider.health.read`;
- `metadata.projection.read` and `metadata.projection.configure`;
- `identity.route.resolve`;
- `profile.anime_grouping_policy.read` and
  `profile.anime_grouping_policy.preview`, for profile scope only.

M3 reconciliation (read-only preflight against merged `df091010`): M3 adds
no scope keys. Its operation-level PAT contract is:

| M3 operation | Existing scope | C2 PAT disposition |
| --- | --- | --- |
| `identity.route.resolve` | `identity_read` | Accept after transaction-level PAT integration. |
| `profile.anime_grouping_policy.read` | `profile_state_read` | Accept profile scope; reject client-override scope. |
| `profile.anime_grouping_policy.preview` | `profile_state_read` | Accept profile scope; reject client-override scope. |
| `profile.anime_grouping_policy.apply` | `profile_state_write` | Reject: the immutable receipt requires real client attribution. |

The source owners are the authored capability registry and
`fasti-store/src/identity_routing.rs`: `authorize_policy_scope` requires an
override's client to match the actor's attribution client, while apply binds
receipt replay and storage to that client. PATs must never synthesize a client
or borrow one from their grant. Preserve existing browser and registered-client
behavior. Add positive profile read/preview/resolve tests, negative client-scope
and apply tests, and revocation/grant-narrowing transaction races. M3's deferred
read transactions do not replace C2's transaction-bound validation and last-use
contract. These are future integration requirements, not active PAT routes.

Observation/webhook, receipt, credential administration, provider-credential,
workspace portability, Access administration, `metadata.claim.refresh`,
fixture-only, guarded, and later-body operations reject PATs. Metadata refresh
remains client-only because M2 receipts are keyed by
`(workspace_id, client_id, operation_id)` and C2 PATs do not synthesize a client.
A later body must explicitly review and author any new PAT actor; a shared scope
is never enough.

Every C2 inventory uses one keyset page contract. Default page size is 32,
maximum is 100, and 0 or more than 100 is rejected rather than clamped. Order is
`(created_at DESC, id DESC)`. Request cursors are paired typed
`after_created_at` and `after_id` fields, both present or both absent; times are
RFC 3339 normalized to UTC microseconds and IDs use the endpoint's typed ID.
Responses return the same typed pair as nullable `next_cursor` plus `truncated`.
The predicate is
`created_at < after_created_at OR (created_at = after_created_at AND id < after_id)`.
The store applies workspace, kind, and member/administrator ownership before
that predicate and before `LIMIT`, reads `page_size + 1` rows, and returns the
last visible row as `next_cursor` when truncated. Do not add a cursor table,
signing key, base64 codec, or client-selected unbounded limit.

OpenAPI 3.1 and generated SDK operations are required. JSON Schema is provided
through the production OpenAPI operation schemas. AsyncAPI and JSON-LD are
`not_applicable` with exact reasons because C2 has no event transport and
private credential state is not linked data. Problems use the shared typed
problem registry and never reveal whether a supplied secret exists.

## 10. UI and accessibility contract

Use upstream Tabler first:

- Settings list-group/select navigation for `Devices and clients`;
- Tabler cards for Clients, Personal access tokens, and Connected devices;
- responsive Tabler tables or list groups for inventory;
- Tabler forms and validation for name, type, profile, scopes, and expiry;
- Tabler modal for destructive confirmation;
- existing Fasti one-time-secret pattern matured into the permanent Access
  owner rather than duplicated.

The one-time-secret state cannot be dismissed by backdrop click or `Escape`.
The Tabler-styled native dialog masks the secret by default, provides Copy and
Reveal controls with `aria-pressed`, announces copy through a persistent polite
live region, traps and restores focus, and requires `I have saved this secret`
before the final close action. It also offers `Revoke and close` when the person
cannot preserve the secret. Clipboard denial retains a manual reveal path
without discarding the secret. Every secret-bearing success response reuses the
Access `Cache-Control: private, no-store` response helper. A
`recent_authentication_required` response preserves only non-secret form state
in memory and never stores it in the URL or browser storage.

The UI must preserve user work, show system status, use recognition over
recall, name exact scope/profile effects, and provide one next action. Secret
plaintext is released from application state on completion, navigation,
cancellation, expiry, failure, component destruction, or acknowledged close.
JavaScript cleanup is best-effort state release, not a memory-zeroization claim.
The value never enters browser storage,
history state, URL, screenshots, logs, analytics, problems, projections, or
retained state. A lost success response exposes only a durable rotate/revoke
recovery action and never replays plaintext.

QA covers AskTog principles, Gestalt grouping, all ten Nielsen heuristics,
relevant IxDF cognitive-load and motor-precision guidance, WCAG 2.2 Level AA,
and an EN 301 549 clause-to-evidence record. Automated checks do not by
themselves justify a conformance claim. Required widths are 320, 375, 768, and
1440 CSS pixels in light, dark, night, and forced-colors modes, with keyboard,
focus, text-spacing, zoom, reduced-motion, live-announcement, and error-recovery
checks.

## 11. Dependency-ordered implementation slices

### C2.0 - Written gate and review

- validate exact M3 handoff, base commit/tree, v15, archive disposition, and
  client-versus-connection vocabulary;
- run engineering and developer-experience plan review without reopening Gates
  0 through 10;
- freeze capability IDs, delegability, policy, migration, UI states, tests, and
  rollback.

### C2.1 - Domain and application freeze

- add the bounded `access_credentials` domain module and typed IDs, then evolve
  `ApplicationClient`, registered-client credential, PAT, consent,
  `TokenPolicy`, delegated-scope, and typed actor models additively;
- add `CapabilityBody::C2`, problems, authored scopes/delegability, commands,
  queries, outcomes, and ports;
- keep plaintext-bearing outcomes non-`Debug`, non-`Clone`, and outside
  serialization;
- do not delete the legacy client-credential port or export the new port from
  `LocalKernel` before the store and all callers can compile together;
- keep all domain decisions free of SQLite, Axum, Tauri, or Svelte types.

### C2.2 - Append-only v16 and store

- implement deterministic backfill, constraints, PAT/consent persistence, audit
  evolution, and explicit command times;
- implement the new administration port in the same store owner before caller
  cutover;
- update every pre-v16 client insert caller;
- prove transaction races, response loss, expiry, and restore behavior.

### C2.3 - Client, grant, and consent operations

- register/list clients, issue/rotate/revoke confidential secrets, and apply
  exact consent/grant/scope transitions;
- atomically migrate the existing Tauri/host callers from
  `CreateScopedClientCredentialCommand` and
  `ClientCredentialAdministrationPort` to the C2 owner, then delete the old
  exported commands and trait in the same compiling slice;
- keep packaged Tauri mutation inventory-only or truthfully unavailable because
  it cannot manufacture recent-authentication evidence;
- reject public-client registration until E2 owns the required redirect and
  protocol state, and preserve ambiguous-grant fail-closed behavior.

### C2.4 - PAT lifecycle

- create/list/authenticate/rotate/revoke with one-time display, fake time,
  expiry, last use, epochs, exact scopes, and audit;
- reuse existing secret and authorization owners.

### C2.5 - Device inventory foundation

- list/revoke device-purpose clients and project their grant/consent state;
- keep pair/approve/poll/token controls visibly unavailable until E2.

### C2.6 - Contracts, API, SDK, and host

- author registry and problem changes, then regenerate once;
- mount fixed-origin routes and generated SDK methods;
- prove non-C2 routers remain closed.

### C2.7 - Gate 10 A+C UI and closure

- implement the permanent A destination and C link using B evidence patterns;
- run focused, contract, UI, security, migration, canonical PR, exact-head,
  rollback, and diff-review gates;
- merge one coherent PR to `dev`, verify the merged tree, allocate the next
  migration, and release shared ownership.

## 12. Worktree and parallelization rule

One integration writer owns schema, domain/application exports, capabilities,
problems, registry, generators, API, SDK, host, Workbench, and generated files.
Subagents remain additive:

- reviewer: active-slice transaction, race, security, and diff review;
- next-slice preparer: tests and failure matrix without shared production edits;
- contract/UX reviewer: API, generated projection, Tabler, copy, and
  accessibility preflight;
- AGY may provide an independent outside review in addition to these gates and
  never replaces them.

After one slice freezes, the commander integrates the next. Parallel writers
may edit only disjoint new files with explicit ownership. Nobody hand-edits a
generated artifact or reverts another worktree's changes.

Production failure matrix:

| Code path | Real failure | Required handling | User outcome | Proof |
| --- | --- | --- | --- | --- |
| Client/PAT create | Recent auth absent or expires between render and submit | Recheck in the mutation transaction; write nothing | Preserve non-secret form state and show one re-auth next action | Store + API + E2E |
| One-time secret response | Connection closes after commit | Never retain/replay plaintext; inventory exposes safe rotate/revoke action | `Needs attention`; no invisible credential | API planted fault + E2E |
| PAT request | Copied root or declared restore blocks activation | Reject before scope authorization | One non-enumerating 401; local data unchanged | Store + API regression |
| PAT request | Grant narrowed after issuance | Intersect stored PAT scopes with current grant scopes | Removed operation returns forbidden immediately | Store + API race |
| Secret rotation | Two tabs rotate/revoke concurrently | One transaction wins; every replaced secret is terminal | Refresh inventory and name the winning state | Store concurrency + E2E |
| Consent mutation | Audit insert or constraint fails | Roll back grant and consent together | Exact retry action; no silent scope change | Store planted fault |
| Migration | Process stops during v15-to-v16 rebuild | SQLite transaction rolls back; old schema remains intact | Restart can retry migration | Migration interruption test |
| Inventory | More rows than the bounded projection | Stable pagination/truncation, no unbounded allocation | `View all` or next page; no missing-state claim | Store + API + UI |
| Last-use write | Concurrent requests reach the 60-second boundary | Conditional update; losers continue after current-state recheck | Request succeeds without `SQLITE_BUSY` | 100-request store test |
| Generated contract | Authored registry and runtime drift | Generator/parity gate fails closed | Package does not merge | Registry + OpenAPI + SDK tests |

## 13. Test and negative-control gate

### Policy and domain

- zero, sub-day, over-maximum, inconsistent, and exact-boundary lifetimes;
- names at empty, whitespace-only, 127/128/129-byte, multibyte-boundary,
  control-character, Unicode bidirectional-formatting, trim, and persistence
  cases;
- unsupported public-client registration and first-party node conversion
  rejection;
- terminal revocation and no resurrection;
- explicit fake clock; no new clock framework.

### Authorization and scope

- empty, duplicate, unknown, non-delegable, and expanded scopes;
- cross-workspace/profile, revoked grant, disabled/deleted subject, removed
  membership, stale auth epoch, and stale authorization epoch;
- missing/expired recent authentication;
- ambiguous active grants remain denied;
- PAT issuance above the subject's active grant is denied;
- later grant narrowing immediately removes the narrowed PAT authority;
- members cannot list, rotate, or revoke another subject's resources;
  administrators can list/revoke but cannot rotate another subject's secret;
  ownerless clients are administrator-only and visibility filters precede
  pagination;
- PAT issuance rejects every scope outside the exact PAT actor set, including
  `metadata_claim_refresh`, and every new scope defaults denied;
- PAT digest resolution racing revocation, grant narrowing, membership removal,
  epoch change, or TrailBase blocking is rejected by the operation transaction;
- every `ScopeKey::ALL` entry is classified exactly once for CLI, integration,
  PAT, and C2 device disposition; a future unclassified scope fails compilation
  and tests.

### Secrets and concurrency

- no plaintext in database, WAL, logs, problems, examples, SDK fixtures,
  screenshots, browser state, or evidence bundles;
- one-time display cannot replay;
- rotate/revoke/consent races have one winner;
- audit failure rolls back mutation;
- response loss exposes no second copy and leaves an inspectable safe state;
- malformed/unknown/revoked/expired PATs are non-enumerating;
- legacy and newly rotated client secrets continue to authenticate through the
  single existing `SHA-256(raw)` contract; PAT digests never authenticate as
  client credentials;
- `fasti_pat_` prefix fuzz covers 63/65 hex characters, uppercase hex, wrong
  credential prefixes, and invalid ASCII without panic or distinguishing
  response;
- blocked TrailBase activation or copied-root mismatch rejects every PAT;
- 100 concurrent uses after the last-use boundary do not surface `SQLITE_BUSY`.

### Migration and restore

- fresh v16 equals exact v15-to-v16;
- all old client insert callers remain valid;
- deterministic node/legacy classification;
- C1 audit rows survive byte-for-byte;
- copied secrets and PATs do not activate;
- forbidden secret/grant archive streams are rejected.

### API and UI

- fixed Host, Origin, CSRF, cookie, body-size, media-type, and unknown-field
  enforcement;
- remote/integration route matrices remain unchanged;
- OpenAPI, generated registry, generated SDK, and runtime parity;
- actor-set parity across application, registry, OpenAPI security and
  `x-fasti-accepted-actors`, generated SDK metadata, and OKF;
- every secret-bearing success response is `private, no-store`; `Revoke and
  close`, clipboard denial, focus restoration, acknowledgement, and best-effort
  state release preserve a safe recovery path without a zeroization claim;
- list limits omitted/1/32/100/0/101, paired cursor validation, equal-time ID
  ties, 32/33/100/101 rows, concurrent newer insertion, and no duplicate or gap;
- focus, keyboard, forced colors, zoom, text spacing, reduced motion, and
  320/375/768/1440 reflow;
- Account Security has one Devices and clients owner; Connections does not
  duplicate it;
- E2 protocol controls stay truthfully unavailable.

### Package and delivery

- add `cargo xtask test milestone --body C2` using the existing receipt format;
- join `cargo xtask test pr`;
- run security/licence/SBOM checks and exact final diff review;
- verify exact PR head and merged `dev` tree;
- report packaged Tauri authentication separately and do not claim it.

Planned coverage map:

```text
CODE PATHS                                      USER FLOWS
[+] client administration                      [+] Devices and clients
  +-- register confidential client [UNIT+E2E]    +-- inventory/empty/truncated [E2E]
  +-- rotate/revoke/race [UNIT+STORE+E2E]         +-- recent-auth locked state [E2E]
  +-- unsupported public client [UNIT+API]        +-- one-time secret save [E2E]
[+] PAT authentication                         [+] PAT lifecycle
  +-- parse/digest/non-enumeration [UNIT]         +-- create/use/rotate/revoke [E2E]
  +-- activation/subject/epoch [STORE+API]        +-- lost response recovery [E2E]
  +-- scope intersection/narrowing [STORE+API]    +-- scope explanation [E2E]
  +-- bounded last-use concurrency [STORE]      [+] Consent and devices
[+] v15 -> v16 migration                          +-- exact consent change [E2E]
  +-- backfill/audit sequence [MIGRATION]          +-- device revoke [E2E]
  +-- restore non-resurrection [MIGRATION+API]     +-- pairing unavailable [E2E]

Every branch above is a required C2 test. No LLM prompt or evaluation surface
changes in this package.
```

## 14. Performance and memory

- keep list responses bounded and paginated by stable `(created_at, id)` order;
- bound names, scopes, clients, PATs, consent revisions, request bodies, and
  audit rows;
- update last-use at most once per 60 seconds with one conditional update
  (`last_used_at IS NULL OR last_used_at < boundary`) rather than on every
  request; concurrent losers continue without another write after re-reading
  current state;
- reuse prepared statements, SQLite indexes, and current digest primitives;
- add no background worker, queue, cache, trait hierarchy, clock framework, or
  generic credential service in C2;
- preserve the 64 MiB idle, 96 MiB normal, 160 MiB heavy, and 192 MiB absolute
  process-tree ceilings.

## 15. Documentation and rollback

Update the canonical authentication plan, authentication architecture,
capability ledger, OpenAPI/SDK docs, Account and Security user guide,
administrator recovery guide, migration notes, backup/restore disposition,
AGENTS.md when an invariant changes, and the implementation ledger.

Rollback is package-scoped:

- stop the new binary before using an old binary against v16;
- restore a copied pre-v16 database for binary rollback;
- revoke or expire newly issued credentials before rollback;
- never down-migrate or rewrite v15/v16 in place;
- keep secrets outside ordinary archive/rollback artifacts;
- record exact lost functionality and next action.

## 16. Developer perspective and DX contract

### 16.1 Primary persona

| Field | C2 decision |
| --- | --- |
| Developer | A self-hosting Fasti operator or integration developer |
| Starting point | Fasti and TrailBase are installed; the person has an active browser Access session |
| Goal | Create one narrowly scoped credential for a CLI, automation, Nuvio installation, or integration and prove one real authorized request |
| Tolerance | Fewer than five minutes, at most three user steps, and no database editing or hand-authored JSON |
| Expected control | Exact profile, scopes, expiry, owner, one-time secret handling, revocation, and typed recovery |
| Context | Permanent Account and Security A, with C linking to the same owner; never a second Connections-only workflow |

The user has already selected the complete path: resolve every observed
confusion point without weakening the trust boundary or expanding C2 into E2.
This is `DX POLISH`, not a new product or framework decision.

### 16.2 Empathy narrative

> I have Fasti and TrailBase running, and I want a small script or Nuvio
> installation to read one profile. The README Quick start helps me start
> `fastid`, but the current status text says the browser Workbench is
> pre-production and that later token controls are unavailable. The Nuvio guide
> then tells me to create an API client through Connections in a trusted
> packaged host, while the same repository says packaged Tauri authentication
> is deferred. I cannot tell which surface owns the credential or whether the
> browser can create it. I also do not want to learn the internal differences
> between clients, grants, credentials, PATs, and scopes before I make one safe
> request. I need Fasti to ask for a name, profile, scopes, and expiry, explain
> the effect in plain language, then show the secret once. I want a generated
> SDK example and a terminal-safe cURL example that do not put the secret in
> shell history or a process argument. The first request must show a real result,
> even if the result is an empty Records list. If it fails, I need one stable
> problem code, a correlation ID, and one exact next action. Later, I need the
> same Account and Security page to show last use and let me revoke the exact
> credential without guessing which device or integration it belongs to.

### 16.3 Benchmark and time to hello world

These are documented flow comparisons, not measured external timings.

| Tool | Documented credential choices | C2 lesson | Source |
| --- | --- | --- | --- |
| GitHub fine-grained PAT | Resource owner, repository access, permissions, expiry, one-time token display | Put authority and expiry before creation; never replay plaintext | [GitHub PAT documentation](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens) |
| GitLab PAT | Name, expiry, scopes, one-time token display; CLI supports explicit scopes and expiry | Keep the safe path explicit and scriptable without creating a second policy | [GitLab PAT documentation](https://docs.gitlab.com/user/profile/personal_access_tokens/), [GitLab CLI token documentation](https://docs.gitlab.com/cli/token/create/) |
| Fasti before C2 | Existing machine credential path plus a disabled PAT surface; Nuvio guide names a packaged-host workflow | Current production C2 time is blocked because recent authentication cannot yet be proven | Repository README and `docs/integrations/nuvio.md` at the planning base |
| Fasti C2 target | One permanent A owner, one generated contract, one secret-safe first-success example | Competitive target: under five minutes after recent authentication is available | This gate |

The magical moment is one successful, real `identity.record.list` response
using a credential whose profile, scopes, expiry, and revocation owner the user
just saw. An empty `records` array is success because it proves authentication,
authorization, transport, generated parsing, and the selected profile without
creating fake data.

Current production TTHW is `blocked`, not estimated. C2 may not claim the target
until a source-backed recent-authentication provider is active and the complete
flow is timed on the exact delivered build. The post-dependency target is less
than five minutes and three user steps:

1. Open **Account and security -> Devices and clients**, choose **Create personal
   access token**, then select one profile, the minimum scopes, and an expiry.
2. Copy the one-time `fasti_pat_...` value, acknowledge that it was saved, and
   load it from protected terminal input. Never paste it into source, a URL, a
   command argument, shell history, browser storage, or a screenshot.
3. Run the generated `FastiClient.listRecords()` example. Expected output is a
   parsed Records response, including `{"records":[]}` for an empty profile.

The generated SDK example must use the existing credential-provider callback,
not a hard-coded string:

```ts
const client = new FastiClient({
  baseUrl: "http://127.0.0.1:8420",
  credential: () => process.env.FASTI_PAT ?? "",
});

const result = await client.listRecords();
console.log(JSON.stringify(result));
```

The paired zsh instructions read the secret without echo and export it only for
the child process. A cURL alternative reads its configuration from protected
standard input so the bearer is not present in shell history or the process
argument list. The final route, operation ID, and output example are generated
from the frozen registry; documentation tests fail if they drift.

### 16.4 Developer journey

| Stage | Current evidence | C2 requirement | Verification |
| --- | --- | --- | --- |
| Discover | README names scoped bearer clients and later PAT controls | Link Account and Security, SDK, OpenAPI, and Nuvio guidance to one credential owner | Link and generated-reference checks |
| Install | README Quick start starts `fastid`; TrailBase has a separate runbook | Do not add another service, CLI, certificate, or packaged-host prerequisite | Clean install and warm-start evidence |
| Hello world | Nuvio guide says the browser cannot create credentials and points to packaged host | Supply the three-step secret-safe `listRecords` path only after recent auth is real | Timed first-success E2E and docs test |
| Real use | Existing clients and scopes authorize durable routes | Show exact profile, scopes, expiry, last use, state, and revocation; keep inventories bounded and paginated | Store, API, SDK, and E2E tests |
| Debug | Existing typed problems can identify authorization failures | Every C2 problem contains a stable code, correlation ID, one next action, and no secret-existence signal | Contract and negative-response tests |
| Upgrade | Append-only migrations and generated contracts already exist | Document v15-to-v16 behavior, legacy `review_required`, frozen archive disposition, and rollback | Migration, restore, and old-binary gates |

### 16.5 First-time roleplay and confusion closure

```text
FIRST-TIME DEVELOPER REPORT
===========================
Persona: self-hosting Fasti operator or integration developer
Attempting: create a scoped token and list Records

T+0:00  Reads README Quick start and confirms fastid is healthy.
T+0:30  Opens the Workbench and sees later token controls marked unavailable.
T+1:00  Reads docs/integrations/nuvio.md; it points to Connections in a packaged host.
T+2:00  Finds packaged Tauri authentication is deferred and cannot identify a valid owner.
T+3:00  Stops without a credential rather than using an undocumented database or fixture path.
```

C2 closes every item: A becomes the sole owner; C links to A; Connections links
instead of duplicating; recent-authentication absence remains an honest lock;
generated examples use the same authored capability; and the first real result
is visible without fake records. No hosted playground is added because Fasti is
private and local-first. The exact local flow is the safe delivery vehicle.

### 16.6 Eight-pass DX scorecard

Scores describe the plan before and after this review. They do not claim that
unimplemented runtime behavior works.

| Pass | Initial | Planned | Requirement that closes the gap |
| --- | ---: | ---: | --- |
| Getting Started | 2/10 | 10/10 | One permanent owner, three steps, expected output, secret-safe terminal setup, and measured sub-five-minute target after recent auth activates |
| API/CLI/SDK | 6/10 | 10/10 | Generated operations, existing callback credential provider, stable pagination, typed actors, and no raw-HTTP-only edge case |
| Error Messages and Debugging | 6/10 | 10/10 | Stable problem code, correlation ID, cause-safe detail, one next action, and non-enumerating authentication failures |
| Documentation and Learning | 5/10 | 10/10 | Quick start, Nuvio guide, OpenAPI, SDK, Account and Security guide, and generated examples share one source-backed flow |
| Upgrade and Migration | 7/10 | 10/10 | Append-only v16, deterministic legacy classification, `review_required`, archive disposition, rollback, and exact migration notes |
| Developer Environment and Tooling | 7/10 | 10/10 | Reuse the existing daemon, generator, SDK, PR gate, fake time, and local test harness; add no setup dependency |
| Community and Ecosystem | 4/10 | 9/10 | Publish exact supported/unavailable status and one stable integration contract; 10 waits for real upstream consumers and feedback |
| Measurement and Feedback | 2/10 | 9/10 | Exact-head docs/E2E tests, timed TTHW evidence, typed failure coverage, and implementation `/devex-review`; 10 waits for repeated real-user measurements |

Planned weighted score: `9.75/10`. The remaining points require delivered and
repeated external evidence, so C2 must not round the score up or call the runtime
best-in-class before the implementation review.

### 16.7 DX implementation constraints

- Do not add a hosted playground, second CLI, second SDK, token-specific scope
  vocabulary, custom UI system, or packaged-Tauri dependency.
- Preserve stable pagination and bounded/truncated metadata in every inventory.
- Keep unsupported and recent-auth locked controls visible with reason, safe
  state, and one next action.
- Bind code examples to generated operation IDs, schemas, problem codes, and
  route paths in tests.
- Document offline/read-only state and the exact recent-authentication
  dependency. Never turn a test fixture into a production shortcut.
- Run `/devex-review` after implementation and measure the exact three-step
  flow. A plan score cannot satisfy the delivery gate.

## 17. Current blockers and next action

M3 is merged and this branch is rebased onto its exact commit/tree in the
header. Metadata M4 confirmed ownership of v16 and shared integration files on
2026-09-04. C2 owns `access_credentials.rs` and its Access module/re-export
lines; M4 may add its separate search module/re-export in its own worktree.

Continue C2.1 pure semantics and focused review. Before shared integration,
obtain M4's merged commit/tree and next migration allocation, rebase, and
reconcile historical archive acceptance. Explicitly classify the four M3
capabilities for PAT actors before changing authorization. Preserve M3's
routes, SDK methods, browser mutation transport, and Preferences controls.
Then complete C2.1 through C2.7 in dependency order.

D preflight remains separate. `webauthn-rs =0.5.5` is dependency-approved,
but D production code is blocked by the IP-literal RP/origin, safe account
selection, and arbitrary TrailBase lifecycle-proof contracts. Do not use D to
bypass shared-file ownership or begin speculative passkey integration.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
| --- | --- | --- | ---: | --- | --- |
| CEO Review | `/plan-ceo-review` | Scope and strategy | prior Gate 0-10 | CLEAR | Gate 10 A+C and full programme scope remain binding |
| Codex Review | `/codex review` | Independent second opinion | 1 AGY equivalent | INFORMATIONAL | 10 challenges; verified security and scope points folded into engineering review |
| Eng Review | `/plan-eng-review` | Architecture and tests | 1 + 4 additive preflights | CLEAR | 8 issues plus domain, store, contract, and security maps folded; 0 critical gaps, 0 unresolved decisions |
| Security Challenge | read-only subagent | Authorization and migration challenge | 1 | CLEAR AFTER CORRECTION | 6 P1 and 4 P2 findings resolved in the frozen contract |
| Design Review | `/plan-design-review` | UI/UX gaps | prior Gate 10 | CLEAR FOR PLAN | Approved A+C with B evidence pattern; runtime proof remains implementation work |
| DX Review | `/plan-devex-review` | Developer experience gaps | 1 | CLEAR FOR PLAN | 8 passes; plan 4.875 -> 9.75, production TTHW blocked -> target under 5m, 0 unresolved |

### Engineering review

Review base: `171f1953e1ea552e044ea7e6e027353746d89156`

Scope decision: proceed with the complete C2 contract. The package crosses more
than eight files because Fasti requires domain, persistence, authored contracts,
generated SDK, host, UI, and evidence parity. A credential-only shortcut would
leave duplicate owners and false capability claims. The plan reduces complexity
through existing owners, one integration writer, sequential shared-file slices,
and no new framework/service/artifact.

#### Architecture

- `[P1] (confidence: 10/10) docs/plans/fasti-access-c2.md:372` - PAT
  authorization must include current TrailBase installation activation and
  generation. The plan now rejects blocked/restored roots before scope
  authorization. Prior learning applied:
  `fasti_data_root_identity_is_mismatch_detector`; C1's physical identity is a
  mismatch detector, while complete authenticated source fencing remains C3.
- `[P1] (confidence: 10/10) docs/plans/fasti-access-c2.md:507` - PAT bearer
  authentication must never satisfy browser Access administration. The plan now
  requires authored per-capability PAT acceptance and rejects bearer credentials
  on every `/api/access/` administration route.
- `[P2] (confidence: 10/10) docs/plans/fasti-access-c2.md:57` - C1 cannot issue
  recent-authentication evidence with pinned TrailBase `v0.33.5`. C2 therefore
  ships production inventory read-only until a real provider activates; tests
  use direct store fixtures and add no runtime bypass.
- `[P2] (confidence: 9/10) docs/plans/fasti-access-c2.md:130` - public clients
  without redirects and OAuth protocol state are unusable scaffolding. Public
  registration and redirect validation now stay together in E2.

Architecture result: `PASS_WITH_M3_SEQUENCE_BLOCKER`.

#### Code quality

- `[P1] (confidence: 10/10) crates/fasti-store/src/client_credentials.rs:33` -
  the existing `ClientCredentialAdministrationPort` directly inserts clients,
  credentials, grants, and scopes. C2 must route its callers through the one C2
  administration owner so classification, policy, recent auth, and audit cannot
  be bypassed.
- `[P2] (confidence: 10/10) docs/plans/fasti-access-c2.md:179` - PAT digests are
  domain-separated while unrecoverable existing client digests keep their
  current single scheme; no speculative dual lookup or migration is added.
- `[P2] (confidence: 10/10) docs/plans/fasti-access-c2.md:187` - PAT scopes are
  an issuance upper bound and runtime authorization intersects them with current
  grant scopes. No second scope owner is introduced.
- `[P2] (confidence: 9/10) docs/plans/fasti-access-c2.md:443` - the v16 audit
  rebuild preserves rows, triggers, indexes, and monotonic sequence state; every
  restore/direct insert uses explicit classification columns.

Code-quality result: `PASS_AFTER_M3_HANDOFF`.

#### Tests

The required branch and user-flow coverage is mapped in Section 13 and in:

`~/.gstack/projects/Scrobble-dev-Fasti/ryan-codex-fasti-access-c2-eng-review-test-plan-20260901-000352.md`

The matrix covers policy boundaries, secret non-disclosure, activation fencing,
dynamic scope narrowing, prefix fuzzing, response loss, races, migration,
restore non-resurrection, route exposure, generated parity, and Tabler/A11y
states. No prompt or LLM eval surface changes.

Test result: `PLAN_COMPLETE_IMPLEMENTATION_NOT_STARTED`.

#### Performance

- `[P2] (confidence: 9/10) docs/plans/fasti-access-c2.md:818` - PAT last use is
  updated at most once per 60 seconds with a conditional write and a 100-request
  contention regression. The request path adds no cache or worker.
- Inventories remain bounded and stable-order paginated. The implementation must
  reuse current Access projection limits or freeze a smaller explicit limit
  before schema/API code. No unbounded list is permitted.
- No new binary, container, runtime service, or distribution channel is added.
  Existing native/OCI/host delivery gates remain authoritative.

Performance result: `PASS_FOR_IMPLEMENTATION`.

### Developer experience review

Primary persona: a self-hosting operator or integration developer with an
active Fasti and TrailBase installation who needs one narrowly scoped
credential and one real authorized response.

Mode: `DX POLISH`. The user already chose the complete path and authorized the
commander to resolve implementation details without another architecture gate.
The review therefore closes all observed confusion in the plan while keeping
recent authentication, packaged Tauri, public OAuth clients, and device flow at
their proven dependency boundaries.

Current experience result: `4.875/10`. README Quick start can start the daemon,
but current token controls are unavailable and the Nuvio guide names a packaged
host that cannot yet authenticate. A new operator cannot complete a truthful
credential flow.

Planned experience result: `9.75/10`. Section 16 now provides the persona,
first-person narrative, official GitHub/GitLab comparison, magical moment,
three-step secret-safe flow, journey map, timed first-run roleplay, eight-pass
scorecard, generated-SDK callback example, typed recovery contract, docs drift
tests, and implementation boomerang. The final quarter-point needs repeated
real-user measurements after delivery and cannot be claimed by a plan.

TTHW result: `blocked -> <5m`. The target begins only after a real recent-auth
provider is active. Inventory remains truthful and read-only before then. The
first-success operation is `identity.record.list`; an empty Records response is
valid proof and creates no fake data.

DX result: `PASS_FOR_PLAN_IMPLEMENTATION_REQUIRES_DEVEX_REVIEW`.

### Additive C2 preflight

Four read-only subagent reviews prepared later slices without touching M3's
shared production files:

- C2.1 mapped one `access_credentials` domain module, typed IDs and actors,
  `TokenPolicy`, audience-specific scope delegability, explicit time, and an atomic
  legacy-port cutover after the new store owner exists;
- C2.2 mapped all 45 current client/credential/grant/scope inserts, M3's inbound
  client foreign keys, append-only client-column evolution, exact audit sequence
  preservation, restore non-authority, frozen archive v4, and the v16 gate;
- C2.6/C2.7 mapped singular capability grammar, thirteen fixed routes and SDK
  methods, explicit accepted actors, PAT-enabled data operations, one generated
  actor policy, keyset pagination, one A projection, Tabler components,
  one-time-secret safety, accessibility tests, and documentation owners.
- the independent security challenge froze per-resource authority, operation-
  transaction PAT revalidation, the client-digest migration boundary, PAT
  audience denial for client-bound metadata refresh, recent-auth proof input,
  exact audit evidence, ownership-first pagination, and secret-response recovery.

These reviews did not run as production writers. M3 owned those surfaces at
the review checkpoint. M4 is now the sole current schema, registry, generator,
API, SDK, host, and Workbench writer; the foundation gate grants no activation.

### What already exists

- canonical `clients`, `credentials`, `profile_grants`, and `grant_scopes`;
- shared secret generation, digest, zeroization, and constant-time comparison;
- client credential create/list/revoke transactions that C2 must evolve;
- C1 browser session, CSRF, Host/Origin, membership, epoch, activation, recent
  authentication, audit, and Account and Security projections;
- registry/generator/OpenAPI/SDK/host/Workbench pipelines;
- Tabler-first API-client and one-time-secret patterns;
- migration, restore, archive, exact-head, and milestone receipt machinery.

### NOT in scope

Section 3.2 is binding. In particular, C2 does not add public OAuth clients,
redirects, protocol tokens, RFC 8628 endpoints, passkeys, vaults, Authentik,
Nuvio pairing, packaged Tauri auth, AsyncAPI without transport, or linked data
for credential state.

### Parallelization

| Step | Modules | Depends on |
| --- | --- | --- |
| C2.0 plan/review | docs/plans | M3 facts, no shared writes |
| C2.1 foundation | isolated domain/application and exact shared prerequisites | M3 merged; bounded metadata reconciliation |
| Remaining C2.1 integration | application, registry, scopes, capabilities, ports | M4 exact merge/handoff |
| C2.2 storage | store, migration, restore | C2.1 |
| C2.3-C2.5 operations | application, store | C2.2 |
| C2.6 public surface | registry, contracts, API, SDK, host | C2.3-C2.5 |
| C2.7 UI/delivery | UI, E2E, docs, xtask | C2.6 |

Lane A is the commander integration sequence C2.1 through C2.7. Lane B reviews
the active slice. Lane C prepares the next slice's tests. Lane D performs
contract/UX preflight. Only B-D run in parallel, and they do not write Lane A's
shared files.

### Implementation Tasks

- [ ] **T1 (P1, human: ~4h / CC: ~30m)** - M4 handoff - verify its exact
  merged schema/archive disposition, client/connection vocabulary, ancestry,
  next migration allocation, and shared-file release before C2 integration.
  M3's merged v15/archive v5 remains the historical base, not a current wait.
- [ ] **T2 (P1, human: ~1d / CC: ~2h)** - Access domain - freeze one
  `ApplicationClient`, PAT, consent, scope-delegability, actor, and policy model;
  converge legacy credential-administration callers.
- [ ] **T3 (P1, human: ~2d / CC: ~4h)** - Store - add append-only v16,
  deterministic backfill, activation-fenced PAT authentication, audit rebuild,
  explicit restore inserts, and race/response-loss behavior.
- [ ] **T4 (P1, human: ~1d / CC: ~2h)** - Operations - implement complete
  confidential-client, PAT, consent, and device-inventory transactions with
  recent-auth enforcement and exact scope intersection.
- [ ] **T5 (P1, human: ~1d / CC: ~2h)** - Contracts - author C2 capability and
  problem changes, generate OpenAPI/SDK once, mount fixed routes, and prove PAT
  denial on Access administration.
- [ ] **T6 (P2, human: ~1d / CC: ~2h)** - Tabler UI - mature the permanent A
  destination, C link, one-time-secret protection, read-only recent-auth state,
  evidence pattern, and accessibility states.
- [ ] **T7 (P1, human: ~1d / CC: ~2h)** - Verification - run focused,
  migration, security, contract, SDK, UI, milestone C2, canonical PR, rollback,
  exact-head review, and merged-tree gates.
- [ ] **T8 (P2, human: ~4h / CC: ~1h)** - DX closure - update README, Nuvio,
  OpenAPI/SDK, and Account and Security docs from one generated flow; bind
  examples to authored contracts; time the three-step path when recent auth is
  active; run implementation `/devex-review` without treating plan scores as
  runtime evidence.

**VERDICT:** CEO, Gate 10 design, engineering, and developer-experience reviews
are recorded for the original plan. C2.1 pure semantics are in progress;
shared integration waits on M4's handoff and the post-M3 actor/archive
reconciliation. Runtime, accessibility, and TTHW claims remain unverified until C2 is
implemented and the exact delivery gates pass.

NO UNRESOLVED DECISIONS
