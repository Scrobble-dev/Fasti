# Fasti Architecture Overview

Fasti is an identity-first local system of record for media activity. It is not a media player.

## Current B0 shape

```text
fastid ──> fasti-api ──> GET /api/v1/health

fasti CLI ──> explicit nonzero guards for export, restore, and verify

governed drafts ──> identity seed, provider-manifest example,
                    activity fixtures, schema, and UAT matrix
```

The retained `fasti-core`, `fasti-activity`, `fasti-store`, and `fasti-auth` crates are scaffolds awaiting B1 reconciliation. Their presence is not the final boundary and does not prove durable observation acceptance, access control, identity resolution, export, restore, or replication.

## Target bounded contexts

B1 establishes three ownership layers:

1. `fasti-domain` owns Record identity, observations, evidence, occurrences, interpretations, review state, corrections, access policy, and invariants.
2. `fasti-application` owns versioned capabilities and ports. It coordinates domain work without importing storage, HTTP, CLI, provider, or UI types.
3. `fasti-contracts` owns shared public DTOs and deterministic generated surfaces. Real Utoipa-bound handlers own OpenAPI operations; authored transport behavior owns AsyncAPI channels; authored vocabularies and contexts own JSON-LD meaning.

Adapters implement application ports for SQLite and files, HTTP/SSE, CLI, generated SDKs, providers, and later UI. Dependencies point inward. Provider and presentation concepts cannot enter domain or application crates.

## Durability sequence

The local kernel does not exist until B2. Its planned success sequence is:

```text
authorize current grant and reserve limits
  -> stream and hash evidence under the governed data root
  -> re-authorize
  -> durably promote evidence
  -> run one bounded SQLite writer transaction
  -> sync required files and directories
  -> publish a replayable durable receipt
```

Any failure before the durability boundary returns a typed problem and cannot report committed success.

## Distribution

The native daemon and CLI are the runtime. OCI wraps the same binaries and does not add a web build or hidden static fallback. B4 adds a local browser presentation only after B0-B3 prove the headless kernel. B8 owns supported packages, signing, non-Linux restore activation, formal TV support, and public releases.

See [the constitution](../constitution.md), [capability ledger](../capability-ledger.md), and [contract ownership](../../contracts/README.md).
