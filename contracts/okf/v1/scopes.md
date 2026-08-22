---
type: Fasti Authorization Scope Catalogue
title: B1 authorization scopes
description: Scope identifiers attached to finalized B1 capabilities.
resource: https://fasti.scrobble.dev/ns/knowledge/v1/scopes
tags: [fasti, b1, contracts, authorization]
status: draft
identifiers:
  - capability_read
  - client_enroll
  - credential_manage
  - listener_configure
  - node_initialize
  - observation_accept
  - profile_select
  - receipt_read
sources:
  - id: fasti-capability-registry
    resource: ../../registry/v1/capabilities.yaml
    title: Fasti capability registry v1
---

# B1 authorization scopes

Scopes are capability-specific grants, not roles and not proof that a runtime
exists. The authorization decision also evaluates workspace, profile, client,
credential, grant, and capability context; possession of a scope string alone
does not authorize a request.

| Scope                | Capability use                                               |
| -------------------- | ------------------------------------------------------------ |
| `capability_read`    | Discover the governed capability contract.                   |
| `client_enroll`      | Enroll the first client through the governed bootstrap flow. |
| `credential_manage`  | Rotate or revoke a credential.                               |
| `listener_configure` | Configure the observation listener.                          |
| `node_initialize`    | Initialize an empty node.                                    |
| `observation_accept` | Submit an observation for governed acceptance.               |
| `profile_select`     | Select a profile explicitly.                                 |
| `receipt_read`       | Replay an authorized receipt.                                |

The exact capability-to-scope bindings live in the
[capability registry](../../registry/v1/capabilities.yaml). Read their runtime
meaning through the [lifecycle catalogue](lifecycle.md).
