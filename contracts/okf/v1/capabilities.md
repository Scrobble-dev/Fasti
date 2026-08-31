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
  - access.identity.bootstrap
  - access.projection.read
  - browser.session.create
  - browser.session.end
  - browser.session.profile.select
  - browser.session.read
  - browser.session.revoke
  - browser.session.rotate
  - browser.sessions.list
  - browser.sessions.revoke_all
  - browser.sessions.revoke_others
  - identity.record.create
  - identity.identifier.attach
  - identity.record.list
  - identity.namespace.register
  - profile.nuvio_collections.clear
  - profile.nuvio_collections.get
  - profile.nuvio_collections.replace
  - profile.record.tracking_disposition.list
  - profile.record.tracking_disposition.set
  - provider.list
  - provider.credential.configure
  - provider.credential.test
  - provider.health.read
  - metadata.claim.refresh
  - metadata.projection.read
  - metadata.projection.configure
authorization_postures:
  - unauthenticated
  - bootstrap_only
  - browser_session
  - local_operator
  - scoped
  - scoped_or_browser_session
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
  observation.accept: scoped_or_browser_session
  receipt.replay: scoped
  receipt.stream: scoped
  access.identity.bootstrap: local_operator
  access.projection.read: browser_session
  browser.session.create: unauthenticated
  browser.session.end: browser_session
  browser.session.profile.select: browser_session
  browser.session.read: browser_session
  browser.session.revoke: browser_session
  browser.session.rotate: browser_session
  browser.sessions.list: browser_session
  browser.sessions.revoke_all: browser_session
  browser.sessions.revoke_others: browser_session
  identity.record.create: scoped_or_browser_session
  identity.identifier.attach: scoped_or_browser_session
  identity.record.list: scoped_or_browser_session
  identity.namespace.register: scoped_or_browser_session
  profile.nuvio_collections.clear: scoped_or_browser_session
  profile.nuvio_collections.get: scoped_or_browser_session
  profile.nuvio_collections.replace: scoped_or_browser_session
  profile.record.tracking_disposition.list: scoped_or_browser_session
  profile.record.tracking_disposition.set: scoped_or_browser_session
  provider.list: scoped
  provider.credential.configure: scoped
  provider.credential.test: scoped
  provider.health.read: scoped
  metadata.claim.refresh: scoped
  metadata.projection.read: scoped
  metadata.projection.configure: scoped
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
| `observation.accept`            | `observation.ingress`       | `scoped_or_browser_session` | Implemented                         |
| `receipt.replay`                | `observation.receipts`      | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `receipt.stream`                | `observation.receipts`      | `scoped`          | Fixture only; durable behavior belongs to B2 |
| `access.identity.bootstrap`     | `access.identity`           | `local_operator`  | Implemented                                  |
| `access.projection.read`        | `access.projection`         | `browser_session` | Implemented                                  |
| `browser.session.create`        | `browser.authentication`    | `unauthenticated` | Implemented                                  |
| `browser.session.end`           | `browser.authentication`    | `browser_session` | Implemented                                  |
| `browser.session.profile.select` | `browser.authentication`   | `browser_session` | Implemented                                  |
| `browser.session.read`          | `browser.authentication`    | `browser_session` | Implemented                                  |
| `browser.session.revoke`        | `browser.authentication`    | `browser_session` | Implemented                                  |
| `browser.session.rotate`        | `browser.authentication`    | `browser_session` | Implemented                                  |
| `browser.sessions.list`         | `browser.authentication`    | `browser_session` | Implemented                                  |
| `browser.sessions.revoke_all`   | `browser.authentication`    | `browser_session` | Implemented                                  |
| `browser.sessions.revoke_others` | `browser.authentication`   | `browser_session` | Implemented                                  |
| `identity.record.create`        | `identity.records`          | `scoped_or_browser_session` | Implemented                         |
| `identity.identifier.attach`    | `identity.identifiers`      | `scoped_or_browser_session` | Implemented                         |
| `identity.record.list`          | `identity.records`          | `scoped_or_browser_session` | Implemented                         |
| `identity.namespace.register`   | `identity.identifiers`      | `scoped_or_browser_session` | Implemented                         |
| `profile.nuvio_collections.clear` | `profile.catalog_configuration` | `scoped_or_browser_session` | Implemented               |
| `profile.nuvio_collections.get` | `profile.catalog_configuration` | `scoped_or_browser_session` | Implemented               |
| `profile.nuvio_collections.replace` | `profile.catalog_configuration` | `scoped_or_browser_session` | Implemented           |
| `profile.record.tracking_disposition.list` | `profile.tracking` | `scoped_or_browser_session` | Implemented                 |
| `profile.record.tracking_disposition.set` | `profile.tracking` | `scoped_or_browser_session` | Implemented                  |
| `provider.list`                 | `connections.providers`     | `scoped`          | Implemented in M1                            |
| `provider.credential.configure` | `connections.providers`     | `scoped`          | Implemented in M1                            |
| `provider.credential.test`      | `connections.providers`     | `scoped`          | Implemented in M1                            |
| `provider.health.read`          | `connections.providers`     | `scoped`          | Implemented in M1                            |
| `metadata.claim.refresh`        | `metadata.claims`           | `scoped`          | Implemented in M2                            |
| `metadata.projection.read`      | `metadata.projection`       | `scoped`          | Implemented in M2                            |
| `metadata.projection.configure` | `metadata.projection`       | `scoped`          | Implemented in M2                            |

`scoped` and `scoped_or_browser_session` capabilities use identifiers from the
[scope catalogue](scopes.md). `browser_session` requires an active opaque Fasti
session. `local_operator` stays inside the packaged-host trust boundary.
`unauthenticated` requires no prior authorization facts, while `bootstrap_only`
requires the explicit fresh-node state rather than a scope grant. Failures use
the shared [problem catalogue](problems.md).

[^fasti-capability-registry]: Fasti capability registry v1
