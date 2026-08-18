# Integration authentication boundary

This document is the durable authentication contract for Floppy integrations.
It describes which credential type owns each use case and how new integration
work must reuse the existing account and authorization surfaces.

## Current platform

Floppy already uses `django-allauth` for browser accounts, account sessions,
and social-account authentication. The REST integration API uses a separate
`IntegrationToken` credential for named, scoped third-party access.

These two credentials have different jobs and must not be collapsed into one
opaque token type:

| Credential | Owner | Intended caller | Authority |
| --- | --- | --- | --- |
| Browser session | django-allauth / Django session | Floppy web UI | Signed-in account session |
| Integration token | Floppy `IntegrationToken` | Nuvio, Kodi, Scrob, other third-party clients | Explicit Floppy scopes only |
| Headless session token | django-allauth Headless, later | First-party packaged/mobile Floppy clients | Signed-in account session |
| OIDC access/refresh token | django-allauth IdP, later | Paired Nuvio or another delegated client | Registered-client scopes |

A session token is not a replacement for an integration token. A third-party
client must not gain account-wide authority merely because it authenticated a
user successfully.

## Release 1 contract

Release 1 keeps the named scoped integration token as the supported third-party
credential. This preserves the current Floppy API and gives integrations:

- a human-readable credential name;
- an explicit client identifier;
- least-privilege scopes;
- optional expiry;
- immediate revocation;
- one-time secret display;
- digest-only secret storage;
- a stable opaque client origin;
- client-bound cursors;
- client-bound idempotency and replay semantics.

All integration-token API endpoints must fail closed when the route has no
scope declaration.

The canonical scope vocabulary lives in `integrations/scopes.py`. Do not define
Nuvio-only, Stremio-only, or UI-only copies of the same permissions.

## Browser security

Account and administrator login must use the existing allauth login boundary.
Django admin has its own login view by default, so Floppy explicitly wraps that
view with allauth's `secure_admin_login` helper. This keeps browser login rate
limits and future MFA/account policy consistent.

Credential creation and revocation remain browser-session-only account actions.
They must return `Cache-Control: private, no-store` and never expose token
digests. A raw integration secret is returned once when it is created.

Before Release 1 is marked stable, token creation/revocation must also use the
same recent-authentication policy as other sensitive account changes. The UI
must preserve a clear recovery path when reauthentication is required rather
than converting that flow into an opaque API failure.

## First-party packaged clients

A packaged/mobile Floppy client should use django-allauth Headless rather than
invent another login-token implementation. Allauth provides `X-Session-Token`
and a Django REST framework authentication class for this purpose.

This is a first-party session credential. It does not inherit the scoped
third-party integration-token contract.

When Headless is enabled:

1. install the `headless` extra;
2. add `allauth.headless`;
3. expose the supported allauth headless routes;
4. allow only the client types Floppy actually supports;
5. keep Floppy integration scopes separate from first-party session authority;
6. link the allauth OpenAPI specification from Floppy API documentation instead
   of copying allauth endpoint schemas by hand.

Core tracking writes must continue to work without a network connection to an
external identity provider after a local session is established.

## Nuvio pairing and Release 2

Release 2 should use django-allauth's OpenID Connect provider for the supported
server-to-client pairing path once the dependency and configuration review is
complete. The allauth IdP supports the device authorization grant, which maps
well to TV and packaged-client setup:

1. Nuvio discovers the Floppy issuer and capabilities.
2. Nuvio requests a device authorization.
3. The user approves the connection in a normal Floppy/allauth browser flow.
4. Nuvio receives only the scopes approved for that registered client.
5. The client uses refresh/revocation behavior owned by the allauth IdP.

Floppy must not give Nuvio a database password, service-role key, raw Floppy
password, or all-purpose account token.

The existing `IntegrationToken` remains a PAT fallback for integrations that do
not implement OIDC. Do not create a second Floppy OAuth token model.

Dynamic client registration stays disabled by default. If Floppy enables it in
the future, initial-access authorization and strict registration validation are
required.

## Shared scope vocabulary

The same capability names must drive:

- `IntegrationToken` validation;
- OpenAPI security documentation;
- AsyncAPI channel requirements;
- integration capability discovery;
- future OIDC client scope configuration;
- Nuvio conformance fixtures;
- user-facing permission descriptions.

Current tracking scopes are resource-oriented. Keep them independent from
provider names so a future client can reuse the same contract.

## Identity and matching

Authentication does not change media-identity semantics. Every authoritative
tracking write still requires exact supported external identity or a verified
translation through the existing shared resolver.

Do not use account identity, client identity, title matching, or a provider name
as a substitute for exact media identity.

## Security invariants

- A credential never grants more authority than its declared model permits.
- Integration tokens fail closed on undeclared routes.
- Browser/account management uses session CSRF protection.
- Raw secrets do not enter logs, metrics, cache keys, screenshots, or exports.
- Token revocation is authoritative on the next request.
- Client origin comes from the authenticated credential, not request payload.
- Cursors and replay receipts are scoped to the authenticated integration
  client.
- All authentication and authorization failures use stable public errors and do
  not expose database or provider details.
- Login and reauthentication flows use allauth rate limits.
- Direct Django-admin login does not bypass the allauth login controls.
- Offline tracking state does not depend on allauth, Redis, Celery, Docker, or
  an external provider being reachable after authentication has completed.

## Contract documentation

When authentication behavior changes, review all of these surfaces in the same
work package:

- `src/api/contracts/openapi.yaml`;
- AsyncAPI channel/security metadata;
- `/api/v1/integrations/capabilities/`;
- the API/MCP wiki;
- token-management UI help text;
- this document;
- migration and compatibility notes when credential persistence changes.

Do not claim a credential flow is supported until its OpenAPI/AsyncAPI contract,
conformance fixtures, revocation behavior, and UI recovery path are verified.

## Upstream references

- django-allauth documentation: https://docs.allauth.org/en/latest/
- django-allauth Headless session tokens:
  https://docs.allauth.org/en/latest/headless/token-strategies/session-tokens.html
- django-allauth OpenID Connect provider:
  https://docs.allauth.org/en/latest/idp/openid-connect/index.html
- Nuvio public integration request:
  https://github.com/NuvioMedia/NuvioTV/issues/2484
- Nuvio Floppy integration request:
  https://github.com/NuvioMedia/NuvioTV/issues/2935

Refs #532 and #636.
