---
type: Fasti Capability Catalogue
title: B1 capabilities
description: Finalized capability identifiers owned by the B1 contract spine.
resource: https://fasti.scrobble.dev/ns/knowledge/v1/capabilities
tags: [fasti, b1, contracts, capabilities]
status: draft
identifiers:
  - system.health
  - system.capabilities.discover
  - node.initialize
  - client.enroll
  - profile.select
  - credential.rotate
  - credential.revoke
  - listener.configure
  - observation.accept
  - receipt.replay
sources:
  - id: fasti-capability-registry
    resource: ../../registry/v1/capabilities.yaml
    title: Fasti capability registry v1
---

# B1 capabilities

The [capability registry](../../registry/v1/capabilities.yaml) is the semantic
source for this catalogue.[^fasti-capability-registry] A finalized contract is
not automatically an implemented runtime. Read each entry together with the
[lifecycle](lifecycle.md) catalogue before presenting it as available.

| Capability                     | Bounded context             | B1 runtime disposition                       |
| ------------------------------ | --------------------------- | -------------------------------------------- |
| `system.health`                | `system.operations`         | Implemented                                  |
| `system.capabilities.discover` | `system.contracts`          | Fixture only                                 |
| `node.initialize`              | `node.administration`       | Fixture only; durable behavior belongs to B2 |
| `client.enroll`                | `client.enrollment`         | Fixture only; durable behavior belongs to B2 |
| `profile.select`               | `profile.preferences`       | Fixture only; durable behavior belongs to B2 |
| `credential.rotate`            | `credential.administration` | Fixture only; durable behavior belongs to B2 |
| `credential.revoke`            | `credential.administration` | Fixture only; durable behavior belongs to B2 |
| `listener.configure`           | `observation.ingress`       | Fixture only; durable behavior belongs to B2 |
| `observation.accept`           | `observation.ingress`       | Fixture only; durable behavior belongs to B2 |
| `receipt.replay`               | `observation.receipts`      | Fixture only; durable behavior belongs to B2 |

Capability authorization uses the identifiers in the [scope catalogue](scopes.md),
and failures use the shared [problem catalogue](problems.md).

[^fasti-capability-registry]: Fasti capability registry v1
