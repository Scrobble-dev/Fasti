# Authentication boundaries

Fasti has several credential types. They are not interchangeable. The access
bounded context owns client credentials and authorization. Presentation code
can request a host capability, but it does not invent a sign-in protocol.

## Implemented credentials

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

## Preserved sign-in methods

The Workbench keeps the planned methods visible so future work does not erase
the approved interface. Only **API Credential** accepts input now. The other
tabs state the missing host contract and do not generate placeholder secrets.

| Method          | Required owner and completion evidence                                                                                                                                                                                                                                                                     |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Passkey         | Access-owned browser-session capability; registered WebAuthn relying party; server-issued registration and authentication challenges; origin binding; credential lifecycle and recovery; accessible-authentication, replay, cross-origin, and device-loss tests.                                           |
| OIDC / SSO      | Access-owned browser-session capability; governed issuer and static client registration; state, nonce, and PKCE validation; consent and canonical Fasti scopes; refresh and revocation policy; redirect, issuer-confusion, and logout tests. Dynamic client registration remains off.                      |
| NuvioTV Device  | Access-owned device-authorization capability; server-issued device and user codes; separate browser approval; bounded expiry; polling interval and backoff; client scopes and revocation; denial, expiry, replay, and polling tests.                                                                       |
| Master Password | Access-owned browser-session capability; password hashing and upgrade policy; rate limits; CSRF protection; recent-authentication and recovery policy; breached-password and lockout disposition; accessible authentication without cognitive tests. A raw password must never be sent to the Records API. |

These are owned B4 access and presentation TODOs. A method becomes active only
when its backend capability, host adapter, typed problem recovery, generated
client surface, threat review, and end-to-end tests land together. A Svelte-only
implementation is a regression.

## Identity issuer evaluation

[ADR-0005](adr-0005-framework-and-auth-adoption.md) controls framework and
identity-provider adoption.

TrailBase is a candidate identity issuer. It is not an accepted replacement for
the implemented local credentials yet. A TrailBase adapter may authenticate a
human subject and mint short-lived access tokens. Fasti still owns workspaces,
profiles, roles, client grants, capability scopes, object authorization, audit,
and Chronicle state.

The durable identity key is `issuer + subject`. Fasti must not store a
TrailBase database row ID as its own user identity. Fasti must not read or write
TrailBase tables directly. TrailBase tokens do not authorize a Fasti operation
until the access application service checks the current local grants.

Do not remove the current local account path during the evaluation. The
TrailBase spike must prove migration, rollback, package operation, network-denied
local access, token-expiry behavior, backup/restore boundaries, and memory limits
before any default changes. Do not claim passkeys, multi-factor authentication,
or TV device authorization from TrailBase unless the selected release documents
and passes those flows.

## Contract disposition

- Production bearer operations and their scopes remain owned by the capability
  registry and generated OpenAPI documents.
- The authenticated receipt stream remains owned by AsyncAPI and requires its
  documented `receipt_read` bearer scope.
- Credentials and browser sessions are security state, not linked-data domain
  entities. JSON-LD is not applicable unless a future public domain vocabulary
  genuinely needs a non-secret authorization concept.
- Tauri administrator and provider-secret commands are local IPC. They do not
  create an undocumented HTTP or AsyncAPI surface.
- ADR-0005 and the TrailBase evaluation change no current route, event, schema,
  SDK method, CLI command, or permission. OpenAPI and AsyncAPI remain unchanged.

## Compatibility reference

The boundary was checked against the local `origin/vendor/floppy-pr-791`
snapshot at `a50c9d98`. Its useful invariant is the separation between browser
sessions, scoped integration tokens, first-party headless sessions, and OIDC
delegated tokens. Fasti reuses that boundary, not Floppy's Django/allauth
implementation. Fasti keeps one access-domain scope vocabulary and its own
local-first storage and host adapters.
