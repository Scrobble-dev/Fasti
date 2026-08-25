# ADR-0004: Register namespace definitions before identity attachment

- Status: Accepted for the B2 internal kernel and B3 portability work
- Date: 2026-08-24
- Public contract: Unchanged; no route, CLI command, SDK method, or provider lifecycle is activated

## Context

`ExternalIdentifierClaim` validated only the spelling of a namespace key. The
SQLite identity path then attached any syntactically valid key to a Record. The
database had no durable namespace definition to export. This allowed different
clients to give the same key different grain or comparison meaning.

The active repository seed defines the B2 minimum: namespace, label, supported
grain, identifier pattern, normalization declaration, and licence posture. The
accepted B0-B3 plan reserves `NamespaceDefinition` in the Identity context and
requires B3 to export namespaces. The supplied planning v1 seed also describes
owner, identifier type, case, uniqueness, deep-link, acquisition, and lifecycle
fields. Those richer fields need provider and migration policy. B6 owns that
work. B2 and archive format v1 do not claim it.

## Decision

`fasti-domain::NamespaceDefinition` is the one runtime value for the active B2
fields. Its constructor and checked deserializer validate the key, required
text, non-empty grain set, and licence posture. Pattern and normalization remain
declarations. B2 does not execute them.

The application Identity port registers a definition under the authenticated
workspace. `CapabilityKey::AttachIdentifier` binds registration to
`ScopeKey::IdentityWrite` (`identity_write`). Registration is idempotent for the
same definition. It rejects a different definition for the same workspace key.

SQLite stores one definition row per workspace and key. Identifier attachment
requires both the declared key and the claim grain. A database trigger applies
the same fail-closed rule to direct inserts and relevant updates. Observation
clues remain original evidence and can retain an undeclared key; only attaching
that key to canonical Record identity requires registration.

B3 exports these rows in the deterministic `namespaces` stream before external
identifiers. Credentials and active authorization bindings remain excluded.

## Consequences

- A test, seed, review resolution, or correction that attaches an identifier
  must register its test namespace explicitly.
- A migrated database with existing external identifiers must register matching
  definitions before verify or export succeeds. The migration invents no
  namespace data.
- A namespace key cannot gain a second comparison space through an attachment.
- B3 can restore definitions before external identifiers.
- B6 must define executable pattern, normalization, acquisition, and lifecycle
  conformance before those behaviors are advertised.
- Public contract activation remains a later coordinated change across the
  registry, OpenAPI, AsyncAPI, Schema, JSON-LD, SDK, CLI, examples, and docs.

## Verification

Focused tests cover checked deserialization, undeclared keys, undeclared grains,
idempotent registration, conflicting registration, revision tracking, and the
deterministic B3 namespace stream.
