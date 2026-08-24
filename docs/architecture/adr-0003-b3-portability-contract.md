# ADR-0003: Fix the B3 workspace portability contract before store orchestration

- Status: Accepted for B3 contract staging
- Date: 2026-08-24

## Context

B3 must prove deterministic full-workspace export, stopped-node clean restore,
and equality without publishing credentials or active authorization bindings.
The store needs a stable inward contract before it owns SQLite snapshots,
archive framing, data-root locks, filesystem staging, and Linux activation.

The current staged NDJSON writer is useful as one deterministic entity-stream
component. It is not a complete `.fasti` archive: it has no archive manifest,
blob inventory, destination publication lifecycle, or restore boundary.

## Decision

The B3 domain vocabulary has one value for each initial portability policy:

- export scope is the full workspace;
- archive profile is `zstd-l3-w22` (level 3 with a 4 MiB maximum window);
- restore is clean-only;
- restored authorization requires a fresh bootstrap;
- restore moves through received, staging, verified, activating, and complete,
  or reaches the rejected terminal state.

The application layer owns the use-case inputs and outcomes. It defines:

- an owned export request and consuming destination completion or abort;
- explicit non-zero ceilings for snapshots, WAL growth, archives, entries,
  paths, decompression, scratch space, cleanup reserve, and backup steps;
- ordered stream descriptors and evidence-blob descriptors;
- a numeric workspace-revision watermark, full-workspace manifest, and a
  separate manifest digest;
- an owned restore request, archive reader, and terminal outcome.

The manifest digest covers RFC 8785 canonical JSON bytes of the `manifest`
body only. The digest is outside that body, so the checksum is not recursive.
Rust `serde_json_canonicalizer` and JavaScript `canonicalize` 2.1.0 compute the
same RFC 8785/JCS bytes. Mutation tests reject a stale digest. The checked-in
JSON Schema and example are an internal draft for store implementation and
compatibility tests. The stream inventory and final count remain unfrozen until
namespace ownership is resolved; this draft invents no namespace stream.

Operating-system paths, archive headers, lock handles, compression objects,
temporary filenames, staging directories, synchronization calls, and atomic
activation primitives stay in store and platform adapters.

The existing `capacity_exceeded`, `integrity_failed`, and
`storage_unavailable` problems are reused. B3 adds the missing typed meanings:
`data_root_locked`, `export_canceled`, `operation_canceled`,
`stopped_node_export_required`, and `unsupported_platform`. Resource or
admission cancellation of online export can direct the caller to stopped-node
export. Restore lock contention directs the caller to stop the daemon through
`data_root_locked`. These problems are staged in the application capability
policy. They do not enter the authored public registry until B3 activation.

Portability ports return a typed failure receipt outside the partial archive
destination. Online export uses the request correlation ID. Clean restore also
uses the restore-attempt ID. The adapter discards incomplete bytes before it
returns this receipt.

After clean activation, recovery bootstrap must name the restored workspace and
one existing profile explicitly. It creates a fresh node-local client and
one-time proof for the normal fresh-credential exchange. It never chooses a
profile implicitly or reuses imported credentials, grants, scopes, or node
state.

## Consequences

- Store orchestration can implement one bounded contract without importing
  filesystem mechanics into the domain.
- Callers cannot reuse a destination after completion or abort.
- Restore cannot select an in-place or credential-preserving policy.
- A multi-profile restore cannot select a recovery profile implicitly.
- A platform adapter must reject unsupported activation before mutation.
- The staged NDJSON port remains temporarily for the existing store adapter;
  store orchestration must compose or replace it behind the complete archive
  port rather than expose it as the user archive.
- This decision activates no runtime, CLI, HTTP, SDK, or discovery surface.

## Verification

Domain tests fix the archive policy and restore state machine. Application
tests fix manifest order, bounded metadata, request policies, terminal restore
outcomes, and staged problem ownership. Contract tests deserialize the
checked-in example, compare its important bounds with the generated Schemars
surface, and reject unknown fields. The governed contract verifier must show
that the public registry and generated public artifacts remain unchanged.

## Related records

- [Capability problem partitions](adr-0002-capability-problem-partitions.md)
- [B3 implementation target](../handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md#12-b3-implementation-target)
- [Contract ownership guide](../../contracts/README.md)
