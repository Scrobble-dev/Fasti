---
type: Fasti Problem Catalogue
title: B1 problem codes
description: Shared problem-code identifiers referenced by finalized B1 capabilities.
resource: https://fasti.scrobble.dev/ns/knowledge/v1/problems
tags: [fasti, b1, contracts, problems]
status: draft
identifiers:
  - capability_unavailable
  - contract_drift
  - forbidden
  - idempotency_conflict
  - invalid_identifier
  - invalid_observation
  - invalid_time
  - receipt_not_found
  - validation_failed
sources:
  - id: fasti-capability-registry
    resource: ../../registry/v1/capabilities.yaml
    title: Fasti capability registry v1
---

# B1 problem codes

These are the stable problem names referenced by finalized B1 capabilities in
the [registry](../../registry/v1/capabilities.yaml).[^fasti-capability-registry]
The shared RFC 9457 representation carries a capability ID, safe state,
retryability, ordered next actions, and a correlation ID. Individual codes do
not claim a later-body failure path is executable.

| Code                     | Contract meaning                                                 |
| ------------------------ | ---------------------------------------------------------------- |
| `capability_unavailable` | The requested capability is owned by another runtime body.       |
| `contract_drift`         | A public surface disagrees with the governed contract source.    |
| `forbidden`              | The request context is not authorized for the capability.        |
| `idempotency_conflict`   | An operation identifier was reused with different semantics.     |
| `invalid_identifier`     | A typed identifier is malformed or used in the wrong role.       |
| `invalid_observation`    | An observation violates the governed input contract.             |
| `invalid_time`           | A claimed time is malformed or violates its role-specific rules. |
| `receipt_not_found`      | No visible receipt matches the requested identifier and context. |
| `validation_failed`      | One or more public fields fail contract validation.              |

See [capabilities](capabilities.md) for which codes each capability declares and
[lifecycle](lifecycle.md) before treating a declared problem as a production
runtime path.

[^fasti-capability-registry]: Fasti capability registry v1
