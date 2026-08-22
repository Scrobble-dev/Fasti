---
type: Fasti Contract Lifecycle
title: Contract and runtime lifecycle
description: Meanings for body ownership, contract state, and runtime availability.
resource: https://fasti.scrobble.dev/ns/knowledge/v1/lifecycle
tags: [fasti, b1, contracts, lifecycle]
status: draft
contract_states: [finalized, reserved]
runtime_availabilities: [fixture_only, guarded, implemented, later_body]
body_ids: [b0, b1, b2, b3]
sources:
  - id: fasti-capability-registry
    resource: ../../registry/v1/capabilities.yaml
    title: Fasti capability registry v1
---

# Contract and runtime lifecycle

Contract state and runtime availability are independent. A stable identifier
can be reserved without freezing its later DTO, and a finalized B1 contract can
remain fixture-only until B2 supplies durable implementation evidence.

## Contract state

| Value       | Meaning                                                                  |
| ----------- | ------------------------------------------------------------------------ |
| `finalized` | The current body's public contract is governed and drift-checked.        |
| `reserved`  | The identifier is held for a later body; its public shape is not frozen. |

## Runtime availability

| Value          | Meaning                                                                             |
| -------------- | ----------------------------------------------------------------------------------- |
| `implemented`  | The named runtime behavior has executable evidence in its owning body.              |
| `fixture_only` | A non-production conformance adapter exercises semantics without durability claims. |
| `guarded`      | The command exists only to fail explicitly without mutation until its owning body.  |
| `later_body`   | Runtime behavior and executable acceptance evidence belong to a later body.         |

## Body ownership

`b0` is the truthful baseline, `b1` the executable contract spine, `b2` the
local durable kernel, and `b3` correction and portability. Later product bodies
are intentionally absent from the current capability registry. Consult
[capabilities](capabilities.md), [problems](problems.md), and [scopes](scopes.md)
as one linked contract catalogue.
