# Access competitor comparison

Checked: 2026-08-30

Purpose: establish the source-backed authentication and credential baseline
required before TrailBase package B. Repeat this review at MVP close.

This is not a product-ranking exercise. Unknown means that the exact reviewed
release did not establish the capability. It does not mean that the capability
cannot exist in a later release.

## Exact releases

| Project   | Release   | Commit                                     | Primary release                                                         |
| --------- | --------- | ------------------------------------------ | ----------------------------------------------------------------------- |
| Ryot      | `v10.5.0` | `8cb503e5c86d7afe44b207850206bfa0870653bc` | [Release](https://github.com/IgnisDa/ryot/releases/tag/v10.5.0)         |
| Cinephage | `v0.16.0` | `8d475505d621a20fc74a4934646926e44d90af3a` | [Release](https://github.com/MoldyTaint/Cinephage/releases/tag/v0.16.0) |
| Yamtrack  | `v0.26.3` | `76856f9e053e7f59469d1eac0238727263e2adfd` | [Release](https://github.com/FuzzyGrim/Yamtrack/releases/tag/v0.26.3)   |

## Capability baseline

| Surface                               | Ryot `v10.5.0`                                                                                           | Cinephage `v0.16.0`                                                                       | Yamtrack `v0.26.3`                                                                                      |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Local account                         | Multi-user username and password; configurable registration                                              | One local administrator; later registration is rejected                                   | Multi-user local accounts; configurable registration                                                    |
| OIDC or social sign-in                | One OIDC issuer; local and OIDC sign-in cannot coexist for one user                                      | No exact-release OIDC or social configuration found                                       | django-allauth social and OIDC; PKCE example; account linking; social-only and automatic redirect modes |
| Verification and recovery             | Email verification not established; administrator password-change links exist                            | Email verification disabled; operator reset can revoke all sessions                       | Exact product behavior not established                                                                  |
| TOTP and recovery codes               | TOTP with hashed recovery codes                                                                          | Not established                                                                           | Not established                                                                                         |
| Browser sessions                      | Opaque cached bearer sessions; configured cookie lifetime is 90 days                                     | Database sessions; seven-day expiry; daily refresh; IP and user-agent metadata            | Django sessions; remembered by default; configured 14-day age                                           |
| Session inventory or revoke all       | User-facing inventory or revoke-all not found                                                            | User-facing inventory not found; operator password reset can revoke all                   | User-facing inventory or revoke-all not found                                                           |
| API or client credential              | Bearer API-key sessions; governed access links have expiry, use limits, revocation, and mutation control | Recoverable full-access and restricted streaming keys; rotation invalidates the prior key | One regenerable per-user integration token shared by webhook integrations                               |
| Device pairing or client registration | Not established                                                                                          | Not established                                                                           | Not established                                                                                         |
| Provider or service credential        | Operator secrets are masked; a user can read their own Sonarr secret                                     | Managed API keys and debrid tokens are encrypted; debrid public shapes are redacted       | Operator secrets use environment variables or secret files; importer OAuth tokens are encrypted         |
| Passkeys or recent-auth step-up       | Not established                                                                                          | Not established                                                                           | Not established                                                                                         |

## Source record

Ryot:

- [Authentication guide](https://github.com/IgnisDa/ryot/blob/v10.5.0/apps/docs/src/guides/authentication.md)
- [Authentication contracts](https://github.com/IgnisDa/ryot/blob/v10.5.0/crates/models/media/src/authentication.rs)
- [Session service](https://github.com/IgnisDa/ryot/blob/v10.5.0/crates/services/session/src/lib.rs)
- [Authentication operations](https://github.com/IgnisDa/ryot/blob/v10.5.0/crates/services/user/src/authentication_operations.rs)
- [TOTP implementation](https://github.com/IgnisDa/ryot/blob/v10.5.0/crates/services/user/src/two_factor_operations.rs)
- [Access-link implementation](https://github.com/IgnisDa/ryot/blob/v10.5.0/crates/services/user/src/access_link_operations.rs)
- [Configuration schema](https://github.com/IgnisDa/ryot/blob/v10.5.0/apps/docs/src/includes/backend-config-schema.yaml)

Cinephage:

- [Authentication configuration](https://github.com/MoldyTaint/Cinephage/blob/v0.16.0/src/lib/server/auth/auth.ts)
- [Session helpers](https://github.com/MoldyTaint/Cinephage/blob/v0.16.0/src/lib/server/auth/session-helpers.ts)
- [Managed API keys](https://github.com/MoldyTaint/Cinephage/blob/v0.16.0/src/lib/server/auth/api-keys.ts)
- [Administrator reset](https://github.com/MoldyTaint/Cinephage/blob/v0.16.0/scripts/reset-admin-password.js)
- [Debrid credential tests](https://github.com/MoldyTaint/Cinephage/blob/v0.16.0/src/lib/server/downloadClients/debrid-config.test.ts)

Yamtrack:

- [Social-auth guide](https://github.com/FuzzyGrim/Yamtrack/blob/v0.26.3/docs/social-auth.md)
- [Django settings](https://github.com/FuzzyGrim/Yamtrack/blob/v0.26.3/src/config/settings.py)
- [User model](https://github.com/FuzzyGrim/Yamtrack/blob/v0.26.3/src/users/models.py)
- [Integration-token tests](https://github.com/FuzzyGrim/Yamtrack/blob/v0.26.3/src/users/tests/views/test_token.py)
- [Integration handlers](https://github.com/FuzzyGrim/Yamtrack/blob/v0.26.3/src/integrations/views.py)

## Planning consequences

- Do not call Ryot access links OAuth clients or device authorization.
- Do not claim that Cinephage encrypts every integration credential.
- Do not infer Yamtrack verification or password recovery from its dependency
  on django-allauth.
- Keep Fasti session inventory, revoke-all, per-client credentials, device
  pairing, passkeys, and recent-auth step-up in the MVP plan. None was proven as
  a complete comparable surface across these three exact releases.
- Keep Fasti credential types distinct. Ryot access links, Cinephage managed
  keys, and Yamtrack integration tokens have different recovery, scope, expiry,
  storage, and URL-exposure properties.

AniList is not part of this competitor comparison. Fasti does not add a
product-competition permission gate for AniList. Provider terms, API limits,
attribution, and credential rules remain separate technical and operational
requirements.
