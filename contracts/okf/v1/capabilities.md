---
type: Fasti Capability Catalogue
title: Governed capabilities
description: Finalized capability identifiers published through the shared contract catalogue.
resource: https://fasti.scrobble.dev/ns/knowledge/v1/capabilities
tags: [fasti, contracts, capabilities]
status: draft
identifiers:
  - system.health
  - integration.status
  - system.capabilities.discover
  - node.initialize
  - client.enroll
  - profile.select
  - credential.rotate
  - credential.revoke
  - listener.configure
  - observation.accept
  - receipt.replay
  - receipt.stream
  - identity.record.create
  - identity.identifier.attach
  - identity.record.list
  - identity.namespace.register
  - provider.list
  - provider.credential.configure
  - provider.credential.test
  - provider.health.read
authorization_postures:
  - unauthenticated
  - bootstrap_only
  - scoped
authorization_assignments:
  system.health: unauthenticated
  integration.status: unauthenticated
  system.capabilities.discover: scoped
  node.initialize: bootstrap_only
  client.enroll: scoped
  profile.select: scoped
  credential.rotate: scoped
  credential.revoke: scoped
  listener.configure: scoped
  observation.accept: scoped
  receipt.replay: scoped
  receipt.stream: scoped
  identity.record.create: scoped
  identity.identifier.attach: scoped
  identity.record.list: scoped
  identity.namespace.register: scoped
  provider.list: scoped
  provider.credential.configure: scoped
  provider.credential.test: scoped
  provider.health.read: scoped
sources:
  - id: fasti-capability-registry
    resource: ../../registry/v1/capabilities.yaml
    title: Fasti capability registry v1
---

# Governed capabilities

The [capability registry](../../registry/v1/capabilities.yaml) is the semantic
source for this catalogue.[^fasti-capability-registry] A finalized contract is
not automatically an implemented runtime. Read each entry together with the
[lifecycle](lifecycle.md) catalogue before presenting it as available.

| Capability                      | Bounded context             | Authorization     | Runtime disposition                          |
| ------------------------------- | --------------------------- | ----------------- | -------------------------------------------- |
| `system.health`                 | `system.operations`         | `unauthenticated` | Implemented                                  |
| `integration.status`            | `observation.ingress`       | `unauthenticated` | Implemented                                  |
| `system.capabilities.discover`  | `system.contracts`          | `scoped`          | Fixture only                                 |
| `node.initialize`               | `node.administration`       | `bootstrap_only`  | Fixture only; durable behavior belongs to B2 |
| `client.enroll`                 | `client.enrollment`         | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `profile.select`                | `profile.preferences`       | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `credential.rotate`             | `credential.administration` | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `credential.revoke`             | `credential.administration` | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `listener.configure`            | `observation.ingress`       | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `observation.accept`            | `observation.ingress`       | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `receipt.replay`                | `observation.receipts`      | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `receipt.stream`                | `observation.receipts`      | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `identity.record.create`        | `identity.records`          | `scoped`          | Implemented                                  |
| `identity.identifier.attach`    | `identity.identifiers`      | `scoped`          | Implemented                                  |
| `identity.record.list`          | `identity.records`          | `scoped`          | Implemented                                  |
| `identity.namespace.register`   | `identity.identifiers`      | `scoped`          | Implemented                                  |
| `provider.list`                 | `connections.providers`     | `scoped`          | Implemented in M1                            |
| `provider.credential.configure` | `connections.providers`     | `scoped`          | Implemented in M1                            |
| `provider.credential.test`      | `connections.providers`     | `scoped`          | Implemented in M1                            |
| `provider.health.read`          | `connections.providers`     | `scoped`          | Implemented in M1                            |

Only `scoped` capabilities use identifiers from the [scope catalogue](scopes.md).
`unauthenticated` capabilities require no authorization facts, while
`bootstrap_only` capabilities require the explicit fresh-node state rather than
a scope grant. Failures use the shared [problem catalogue](problems.md).

[^fasti-capability-registry]: Fasti capability registry v1
