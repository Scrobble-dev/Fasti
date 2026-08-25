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
- destination-owned filesystem-capacity preflight before snapshot or data-root
  mutation, using a conservative uncompressed output bound;
- one cloneable standard-library cancellation signal on export and restore
  requests, polled at bounded work boundaries;
- ordered stream descriptors and evidence-blob descriptors;
- a numeric workspace-revision watermark, full-workspace manifest, and a
  wire-neutral export completion summary containing workspace identity and
  revision plus manifest/archive digests and archive bytes;
- an owned restore request, archive reader, and complete-only success outcome.

`RestoreWorkspace` has the distinct non-delegable `local_operator`
authorization disposition and no credential scope. Before dispatch, an adapter
must prove owner-only authority over the local data root and hold its exclusive
lock. The ordinary request-credential `AccessSnapshot` evaluator always denies
this disposition; credentials, grants, and scopes cannot bypass the local
proof. Recovery bootstrap prepare and complete are phases of this same restore
capability, not separately delegable capabilities.

Restore rejection is a typed portability failure. It is never an `Ok` outcome.
This keeps `PortabilityResult` unambiguous: success means that clean activation
completed.

The manifest digest covers RFC 8785 canonical JSON bytes of the `manifest`
body only. The digest is outside that body, so the checksum is not recursive.
Rust `serde_json_canonicalizer` and JavaScript `canonicalize` 2.1.0 compute the
same RFC 8785/JCS bytes. Mutation tests reject a stale digest. The checked-in
JSON Schema and example are an internal staged contract for store implementation
and compatibility tests. Archive v1 freezes 16 streams. The `namespaces` stream
comes after `records` and before `external_identifiers`, so definitions restore
before identifier claims. This archive compatibility decision does not activate
the public export or restore capabilities.

`fasti-contracts` owns the application-manifest to DTO projection, the JCS body
checksum, and the canonical complete `manifest.json` bytes. The store adapter
does not rebuild the wire mapping. The outbound projection is the only type
that associates the DTO, application manifest, digest, and canonical bytes; it
does not expose a consuming parts API. Hostile inbound conversion verifies the
JCS body digest and privately constructs a contract-owned verified manifest.
The application layer has no wire-checksum constructor or canonical serializer.
Archive-entry admission counts the mandatory final `manifest.json` as well as
every stream and blob entry. The wire `migration_version` is bounded to the
application `u32` maximum, `4294967295`.

Restore receives one opened seekable archive. It completes the format, path,
header, final-manifest, checksum, reference, and limit preflight before any
destination mutation. It then rewinds the same opened source for the restore
pass. It does not reopen a path after validation and does not spool the complete
archive into memory.

The private pass-two importer creates one fresh owner-only staging attempt only
after pass one succeeds. It reuses the sole archive visitor and the same opened
source, decodes bounded typed canonical NDJSON for all 16 frozen streams, and
copies each evidence blob through descriptor-relative handles with a second
size and digest check. It creates and migrates the staged SQLite database only
through the shared schema helper. One immediate transaction defers foreign-key
checks until all ordered streams are present, then verifies SQL counts,
workspace and profile ownership, namespace bindings, identity and correction
relations, receipt and operation semantics, SQLite integrity, the schema
fingerprint, and a canonical re-export descriptor for every stored stream.
Only after those checks does it set the exact manifest revision and commit.

The staged database contains portable client shells but no `node_state`,
credentials, grants, scopes, or initialization proof. Staging cleanup follows
retained directory descriptors, reports cleanup failure, and never writes a
COMPLETE marker or renames the attempt to `current`; the activation coordinator
owns those later steps. Runtime, CLI, HTTP, SDK, and capability-registry
activation remain separate gated slices.

Operating-system paths, archive headers, lock handles, compression objects,
temporary filenames, staging directories, synchronization calls, and atomic
activation primitives stay in store and platform adapters.

The existing `capacity_exceeded`, `integrity_failed`, and
`storage_unavailable` problems are reused. B3 adds the missing typed meanings:
`data_root_locked`, `export_canceled`, `operation_canceled`,
`stopped_node_export_required`, and `unsupported_platform`. Resource or
admission cancellation of online export can direct the caller to stopped-node
export. Restore lock contention directs the caller to stop the daemon through
`data_root_locked`. Stopped-node verification maps the same shared-lock failure
to `data_root_locked` for `VerifyWorkspace` without panicking. Unsafe paths,
durability-configuration mismatches, schema mismatch, corrupt SQLite, and
non-transient open failures map to non-retryable `integrity_failed`; only
explicitly transient I/O and SQLite lock/interruption failures map to
retry-safe `storage_unavailable`. Recovery after activation uses
`recovery_bootstrap_pending`, the safe state
`restored_data_active_bootstrap_pending`, and the next action
`retry_recovery_bootstrap`. These problems are staged in the application
capability policy. They do not enter the authored public registry until B3
activation.

Both online and stopped-node export require the Linux anchored `openat2`
evidence-read contract. An adapter on another platform must fail before reading
or disclosing evidence with `unsupported_platform`; it must not weaken the
time-of-check/time-of-use boundary.

Caller cancellation is explicit request state. Either export mode aborts and
removes its partial destination before it returns mode-neutral
`export_canceled`. Restore rejects or
removes staging before it returns `operation_canceled`. Cancellation cannot
return a partial success.

Portability ports return a typed failure receipt after the destination leaves
the caller's control. Every receipt constructor takes its concrete request,
validates the problem capability, correlation ID, and operation-specific
problem set, then derives identity from that request. Online and stopped-node
export retain mode, workspace, and correlation. Clean restore retains
correlation and restore attempt. Recovery prepare also retains workspace and
the explicit selected profile; completion additionally retains the concrete
recovery client. The adapter discards incomplete bytes before it returns an
export or restore receipt when cleanup succeeds. Export receipts distinguish
successful discard, indeterminate partial cleanup, and the fail-closed case
where a complete archive was linked but parent-directory durability could not
be confirmed.

Stopped-node export is a distinct request mode. It carries the same
`ExportWorkspaceQuery` and `RequestAccessContext` as online export, derives the
workspace from that access context, and applies the same grant rules against
the stopped database while holding the shared data-root lock. There is no
second caller-supplied workspace identifier.

Verified clean activation leaves both `node_state` and the authorization tables
empty. Recovery bootstrap must then name the restored workspace and one existing
profile explicitly. Prepare runs in one immediate transaction: it creates a
distinct fresh node-local client, a one-time proof digest, and one provisional
grant with only `client_enroll`, and records the restore attempt in the newly
inserted `node_state`. Explicit replacement can replace only that exact pending,
unconsumed provisional state. It never chooses a profile implicitly or reuses
imported client authentication, credentials, grants, scopes, or node state.

Completion receives the one-time proof and an already-owned final credential
from the caller. One immediate transaction revokes the proof credential, stores
only the supplied credential digest, expands the provisional grant to the shared
current administrator scopes, and consumes the proof. A retry with the same
proof and final credential returns the original non-secret access metadata. A
different pair fails with `bootstrap_closed`; no failure-attempt counter or
post-commit secret generation exists.

The store migration adds only nullable `node_state.recovery_restore_attempt_id`.
Strict pass-two import uses the same opened archive as pass one, admits disk
capacity before staging, imports all 16 typed streams and checked blobs, and
re-exports the staged database to prove exact descriptor equality. It leaves
`node_state` and authorization tables empty. Cancellation and every pre-rename
failure remove the phased staging attempt and synchronize its parent.

The private Linux activation composition writes create-new `received`,
`staging`, `verified`, and `activating` sentinels, then moves one canonical
digest-bound marker with the verified staging directory by no-replace rename.
It publishes `complete` through a synchronized pending file and no-replace
rename after the data-root parent sync, then synchronizes `current` and the data
root before success. The staged owner is consumed and disarmed at activation,
so its cleanup cannot remove the renamed `current`. Startup refuses non-empty
pre-rename `staging`. A new stopped-node restore validates its incoming archive,
then rejects and removes exactly one interrupted attempt before retrying. Startup
completes the one valid post-rename/pre-complete crash state and opens SQLite
through the retained data-root descriptor rather than the replaceable configured
path.

Private recovery composition verifies the full COMPLETE marker before and
after opening the activated database, then calls the prepare or completion
transaction. `SqliteKernel` implements only live archive export. A distinct
stopped-node adapter implements stopped export, clean restore, and recovery;
wrong-mode calls abort their destination and return typed failures without
re-resolving a live kernel path. There is deliberately no CLI, HTTP, SDK, or
public capability activation. Ordinary initialization retains its existing
closed result for a restored workspace until runtime dispatch supports a
distinct recovery-required classification.

The store predecessor for final archive assembly keeps the frozen entity SQL
and row codec in one place. It can stream exactly one plain NDJSON entity from
a caller-owned, read-only snapshot connection in 256-row keyset pages. The
caller supplies the cancellation and authorization monitor, and the helper
checks the configured per-stream row and byte ceilings before it returns the
row count, byte length, and SHA-256 descriptor. The same pager and codec remain
behind the transitional staged exporter.

The snapshot schema fingerprint uses the numeric `user_version` and the
ordered, non-SQLite-owned `sqlite_schema` definitions from the actual opened
database. It does not hash Rust migration source or physical root-page
allocation. Evidence inventory reads the same snapshot, orders entries by
`EvidenceId`, and rejects non-canonical IDs, digests, sizes, paths, or duplicate
blob digests before later archive code copies any bytes. These primitives do
not create the tar stream, publish a destination, or activate a public export
surface.

The internal archive assembler now combines those primitives without activating
runtime or public discovery. Online export preflights the destination for the
conservative uncompressed archive plus cleanup reserve, and separately admits
scratch space for one bounded snapshot plus one stream file while reserving the
configured WAL-growth allowance on the shared filesystem. Stopped-node export
owns the same exclusive `LockedDataRoot`, skips the unnecessary snapshot, and
admits one stream file plus cleanup reserve without crediting compression. Both
modes use the same ordered entity and blob encoder, canonical final manifest,
bounded destination writer, authorization checks, and owner-safe stale scratch
sweep. The next exporter reclaims crash-stale plaintext attempts before
admission, and success performs fallible cleanup before publication. Online
export releases the live snapshot connection before archive generation. Both
modes reauthorize at bounded disclosure points and once more after final flush,
open evidence from the locked data-root descriptor on Linux, validate the same
opened inode while copying, and keep the destination under an abort guard until
consuming completion succeeds. The orchestration seams remain crate-private
behind the two ownership-specific store port implementations. The production
Linux filesystem destination keeps its bounded partial archive in an unnamed
owner-only `O_TMPFILE`, verifies the complete archive digest, links it to the
caller-selected final name without replacement, and synchronizes the retained
parent directory. A process crash before linking leaves no named partial; a
crash after linking exposes only the complete digest-verified archive. A
post-link directory-sync failure does not attempt a check-then-unlink rollback:
the failure receipt reports indeterminate publication durability so the caller
can inspect the selected path without risking deletion of a concurrent
replacement.

## Consequences

- Store orchestration can implement one bounded contract without importing
  filesystem mechanics into the domain.
- Export cannot begin source mutation until its destination confirms space for
  all stream bytes, referenced blob bytes, container and final-manifest
  overhead, and the applicable reserve. Admission does not credit compression.
- Restore can preflight and consume one opened inode without a TOCTOU reopen.
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
outcomes, stopped-node authorization context, prepare/complete failure-operation
identity, caller-owned recovery completion, and staged problem ownership.
Contract tests deserialize the checked-in example,
compare its bounds with the generated Schemars surface, reject values outside
the DTO type range, and prove the canonical outbound projection round-trips.
Store regressions prove the v5 migration adds only the nullable recovery marker;
prepare rejects wrong workspace/profile and imported authorization without
mutation; replacement removes only a pending provisional; completion is atomic,
concurrent, idempotent for the same caller-owned pair, closed for a different
pair, and never persists plaintext secrets. Pass-two regressions exercise a
reachable fixture across all 16 frozen streams plus one evidence blob, verify
zero node-local authorization state and no activation marker, and reject typed,
ordering, cross-workspace, and missing-reference mutations while removing the
failed attempt. A real subprocess matrix sends `SIGKILL` at every restore phase,
including rejection, database/blob sync, marker publication, activation rename,
and completion sync boundaries. It proves lock release, no pre-rename `current`,
validated retry after interrupted staging, digest-proven post-rename recovery,
and fail-closed partial completion. Recovery tests prove the source credential
is not restored, the new caller-owned credential authenticates, and the
recovered workspace verifies. Lock regressions prove
stopped-node restore and offline verify return `data_root_locked` while a live
kernel owns the data root. Run the focused crash gate with:

```bash
cargo test -p fasti-store archive::tests::filesystem_destination_sigkill_matrix --locked -- --exact
cargo test -p fasti-store restore_import::tests::full_restore_sigkill_matrix --locked -- --exact
```

The governed contract verifier must show that public restore activation remains
absent. Generated metadata records its governed `local_operator` authorization
with no scopes.

## Related records

- [Capability problem partitions](adr-0002-capability-problem-partitions.md)
- [Namespace definition registration](adr-0004-namespace-definition-registration.md)
- [B3 implementation target](../handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md#12-b3-implementation-target)
- [Contract ownership guide](../../contracts/README.md)
