# Authentication boundaries

Fasti has several credential types. They are not interchangeable. The access
bounded context owns client credentials and authorization. Presentation code
can request a host capability, but it does not invent a sign-in protocol.

## Active non-human credentials

| Credential                             | Owner and purpose                                                                                   | Storage and disclosure                                                                                                                                                                        | Current UI                                                                                                                                     |
| -------------------------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| Node initialization proof              | Access bootstrap. Authorizes first-client enrollment once.                                          | Returned in one JSON response. Never put it in a URL, log, browser store, or provider setting.                                                                                                | Trusted setup flow only. It is not a sign-in method.                                                                                           |
| Packaged-host administrator credential | Access administration for the opened Fasti data root.                                               | The Tauri host keeps the 32-byte secret in the platform credential store under a data-root-scoped account. The webview does not receive it.                                                   | Resumes trusted setup and authorizes local IPC commands.                                                                                       |
| Scoped API client credential           | One independently revocable client and profile grant. The server stores only its digest and scopes. | Plaintext is returned once when the trusted host creates the client. A credential is 32 bytes, represented as 64 hexadecimal characters.                                                      | The API-client panel can create an `observation_accept` client. The browser Workbench can use a separately created `identity_read` credential. |
| Browser record credential              | A browser-tab connection to the local Records API. It is not an account session.                    | Held only in the `createWebHost` closure. Sent in the `Authorization: Bearer` header. Reload and **Clear browser credential** remove it. Legacy local-storage values are deleted and ignored. | Implemented for `GET /api/v1/records`.                                                                                                         |
| Provider credential                    | Outbound adapter secret, such as a Google Books key. It never grants Fasti API access.              | Stored by the packaged host in a data-root-scoped platform credential-store account. Never place it in a provider URL or browser storage.                                                     | Trusted settings only.                                                                                                                         |

The current API-client model has scopes, profile ownership, one-time plaintext
disclosure, digest-only persistence, listing, and revocation. It does not yet
have a user-defined name or expiry. Add those only as an access-domain contract
change with migration, OpenAPI or IPC projection, SDK updates, and lifecycle
tests. Do not add presentation-only fields.

None of these credentials authenticates a person or satisfies recent human
authentication.

## Fasti browser sessions

PR A defined the Fasti `AuthSubject`, `FastiBrowserSession`,
`BrowserSessionId`, and `SessionPolicy` ownership. C1 now implements the Access
operations and generated contracts in local source.

The model uses opaque random session secrets, digest-only storage,
exact opaque public identifiers, idle and absolute expiry, rotation, bounded
activity updates, Origin and Host validation, strict cross-site request
forgery protection, and session-local selection of an existing authorized
profile grant.

Only the exact requested-and-bound `127.0.0.1:8420` durable listener mounts the
C1 route set. It does so even without a TrailBase root, which lets the
Workbench report the exact unavailable state. Code exchange and new session
issuance require a verified installation receipt and persisted active
activation. Port fallback, alternate loopback, generic local, integration,
container-forwarded, wildcard, and remote routers omit C1 routes.

This source state is not a supported release. C1 review, exact-head CI, merge,
and merged-tree evidence remain pending. Packaged Tauri authentication,
cross-platform WebView behavior, and packaged assistive-technology proof are
deferred to `C1-TAURI-AUTH`; no packaged desktop authentication support is
claimed.

## Required sign-in methods

The Workbench keeps the planned methods visible so future work does not erase
the approved interface. Each unavailable control states its owning package and
next action. It does not generate or store placeholder security state.

| Method                                                                                    | Owner and activation gate                                                                                                                                                          |
| ----------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| TrailBase password, registration, verification, reset, and supported social sign-in | TrailBase in B, then Fasti exchange in C1. Fasti does not store the human password.                                                                                               |
| TrailBase password plus TOTP                                                        | Unavailable in C1 because v0.33.5 loses the original PKCE ceremony. Fasti does not store the TOTP secret or simulate the missing transition.                                      |
| Fasti browser session                                                               | Fasti Access. C1 implements fixed-origin sign-in, issuance, inventory, rotation, profile selection, and revocation in local source. Package and delivery evidence remains pending. |
| Passkey and Fasti recovery code                                                     | Fasti Access in D, linked to `AuthSubject` with a verified WebAuthn ceremony and active TrailBase-anchor check.                                                                    |
| Generic OpenID Connect and Authentik sign-in                                        | Identity Integration in E1/E3. Require discovery, state, nonce, S256 Proof Key for Code Exchange, exact issuer and subject, token validation, linking policy, and logout evidence. |
| Fasti OAuth and Nuvio or command-line device approval                               | Fasti Access in E2. Require approved clients, scopes, profile consent, bounded codes, polling, rotation, revocation, and replay tests.                                             |

These methods are approved MVP work. A method becomes active only
when its backend capability, host adapter, typed problem recovery, generated
client surface, threat review, and end-to-end tests land together. A Svelte-only
implementation is a regression.

## Selected identity architecture

[ADR-0005](adr-0005-framework-and-auth-adoption.md) records the earlier
framework evaluation. Its optional TrailBase and django-allauth conclusions
are superseded by the approved
[TrailBase authentication programme](../plans/trailbase-authentication-remediation.md).

TrailBase is the selected private, local human-account platform. It runs as a
separate, pinned, unmodified process with a separate data root. Fasti uses only
documented public TrailBase APIs. TrailBase owns only proven human-account
functions. Fasti owns `AuthSubject`, browser sessions, workspaces, memberships,
roles, profiles, grants, scopes, clients, devices, passkeys, recovery codes,
authorization, audit, and Chronicle state.

Every TrailBase installation has its own administrator credential. The
operator keeps it in a selected password manager or equivalent private
installation record. Fasti never copies, stores, logs, receipts, or browser-
persists that password. Every person uses a distinct TrailBase account. The
administrator credential is not a shared human account and does not justify a
new Fasti secret store.

C1 uses a server-only direct backchannel to the exact supervised TrailBase
origin. Fasti exchanges the one-use ceremony's authorization code at
`/api/auth/v1/token`, rechecks the returned proof and refresh session at
`/api/auth/v1/status`, and revokes that refresh session at
`/api/auth/v1/logout`. It discards every TrailBase token before it creates an
opaque Fasti browser session. Fasti does not accept a browser-supplied
TrailBase token, validate TrailBase tokens offline, or treat TrailBase
administrator and TOTP-enrollment claims as Fasti authorization or proof of
multi-factor use.

TrailBase hardening that is useful beyond C1 is tracked in the separate
[upstream hardening plan](../plans/trailbase-upstream-hardening.md). That work
does not authorize a Fasti-maintained fork and does not block the direct C1
profile.

Fasti assigns one stable `TrailBaseInstanceId` to the installation and links it
to the proven TrailBase subject. Fasti must not use a TrailBase database row ID
as its identity, read or write TrailBase tables, or use TrailBase Record APIs
for Fasti data. TrailBase proof does not authorize a Fasti operation until the
application service checks current subject, membership, role, profile grant,
scope, and epoch state.

The superseded local `BrowserUser`, password, development account, custom TOTP,
backup-code, WebAuthn-shaped, and fabricated OpenID Connect paths are not a
compatibility surface. Keep useful user controls visible as truthful
unavailable states until their approved owner passes its package gate.

## Contract disposition

- Production bearer operations and their scopes remain owned by the capability
  registry and generated OpenAPI documents.
- The authenticated receipt stream remains owned by AsyncAPI and requires its
  documented `receipt_read` bearer scope.
- Credentials and browser sessions are security state, not linked-data domain
  entities. `JSON-LD: N/A — security state, no public semantic entity`.
- PR A adds no externally visible asynchronous authentication event.
  `AsyncAPI: N/A — synchronous dormant state with no event channel`.
- PR A adds no public command-line authentication operation.
  `Public CLI: N/A — direct deterministic fixtures only; activation belongs to C1`.
- PR B exposes TrailBase operator lifecycle only through `scripts/dev.sh trailbase`.
  The exact vendor OpenAPI is digest-bound in conformance evidence. C1 owns
  Fasti's separate exchange contract and does not copy vendor routes.
- PR B adds no Fasti asynchronous authentication event or public linked-data
  entity. `AsyncAPI: N/A — synchronous vendor account lifecycle` and
  `JSON-LD: N/A — private human credential and session state`.
- Tauri administrator and provider-secret commands are local IPC. They do not
  create an undocumented HTTP or AsyncAPI surface.
- C1 generates the finite browser-authentication and session OpenAPI, JSON
  Schema, typed problem, capability, and TypeScript SDK surfaces. Its callback
  is browser navigation and has no SDK method. AsyncAPI and JSON-LD remain not
  applicable. First-administrator bootstrap remains packaged-host IPC rather
  than HTTP.

## Compatibility reference

The boundary was checked against the historical local
`origin/vendor/floppy-pr-791` snapshot at `a50c9d98`. Its useful invariant is
the separation between browser sessions, scoped integration tokens,
first-party headless sessions, and OpenID Connect delegated tokens. The former
Django/allauth choice is `SUPERSEDED`; it is behavioral context, not an
implementation dependency or compatibility requirement. Fasti keeps one
Access scope vocabulary and its own local-first authorization, storage, and
host adapters.
