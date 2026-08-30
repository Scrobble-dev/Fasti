---
type: Fasti Authorization Scope Catalogue
title: Governed authorization scopes
description: Scope identifiers attached to finalized catalogue capabilities.
resource: https://fasti.scrobble.dev/ns/knowledge/v1/scopes
tags: [fasti, contracts, authorization]
status: draft
identifiers:
  - capability_read
  - client_enroll
  - credential_manage
  - identity_read
  - identity_write
  - listener_configure
  - observation_accept
  - profile_select
  - provider_credential_manage
  - provider_read
  - receipt_read
sources:
  - id: fasti-capability-registry
    resource: ../../registry/v1/capabilities.yaml
    title: Fasti capability registry v1
---

# Governed authorization scopes

Scopes apply only to capabilities whose authorization posture is `scoped`.
They are capability-specific grants, not roles and not proof that a runtime
exists. The authorization decision also evaluates workspace, profile, client,
credential, grant, and capability context; possession of a scope string alone
does not authorize a request. `system.health` is `unauthenticated`, and
`node.initialize` is `bootstrap_only` with no scope grant.

| Scope                        | Capability use                                                 |
| ---------------------------- | -------------------------------------------------------------- |
| `capability_read`            | Discover the governed capability contract.                     |
| `client_enroll`              | Enroll the first client through the governed bootstrap flow.   |
| `credential_manage`          | Rotate or revoke a credential.                                 |
| `identity_read`              | List records visible to the authenticated workspace.           |
| `identity_write`             | Create records, attach identifier claims, register namespaces. |
| `listener_configure`         | Configure the observation listener.                            |
| `observation_accept`         | Submit an observation for governed acceptance.                 |
| `profile_select`             | Select a profile explicitly.                                   |
| `provider_credential_manage` | Store, replace, remove, or test a provider credential.         |
| `provider_read`              | Read provider capabilities and health.                         |
| `receipt_read`               | Replay receipts or subscribe to their authorized event stream. |

The exact capability-to-scope bindings live in the
[capability registry](../../registry/v1/capabilities.yaml). Read their runtime
meaning through the [lifecycle catalogue](lifecycle.md).
