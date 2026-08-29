# Conformance and UAT ownership

Fasti keeps two acceptance matrices. They answer different questions and use separate ID namespaces so one trace ID never resolves to two different cases.

| File                                                       | Namespace | Rows | Scope                                                                                  |
| ---------------------------------------------------------- | --------- | ---- | -------------------------------------------------------------------------------------- |
| [`uat-matrix.csv`](uat-matrix.csv)                         | `ID-###`  | 80   | Product-wide acceptance across identity, resolution, sync, offline, API, and knowledge |
| [`identity-uat-matrix.v1.csv`](identity-uat-matrix.v1.csv) | `IDF-###` | 126  | Identity-first acceptance derived from the identity-first greenfield plan              |

A row in either file describes required behavior. It does not claim that the behavior is implemented.

## Evidence rule

No UAT row passes from prose, syntax parsing, or a mocked UI. Evidence must identify the source commit, artifact, command, environment, and result. Hardware claims also identify the physical profile.

## Body ownership

B0 owns repository-truth, guarded-command, workflow-policy, draft-syntax, native-build, and OCI-smoke evidence. B1 imports each UAT ID into the capability registry and marks it `direct`, `split`, or `deferred` with one owning body and reason. B2 and B3 bind implemented rows to executable API, CLI, SDK, persistence, recovery, and network-denied evidence.

## Why both matrices

`uat-matrix.csv` is the release gate. [`uat-ownership.v1.json`](uat-ownership.v1.json) assigns every `ID-###` case a body and a status, and [`contracts/registry/v1/capabilities.yaml`](../../contracts/registry/v1/capabilities.yaml) traces B1 capabilities to specific `ID-###` cases. Those traces are load-bearing: `scripts/validate-okf-uat.mjs` asserts that the registry trace set and the B1-owned ownership set are identical.

`identity-uat-matrix.v1.csv` is deeper on identity but narrower on product. It has no counterpart for the capability-discovery case (`ID-065`) or the API exact-ID-conflict case (`ID-064`), both of which currently carry B1 registry traces. It therefore extends the product matrix; it does not replace it.

The identity matrix was published with its own `ID-###` numbering. Row _n_ of the source file is `IDF-nnn` here. The renumbering is mechanical and order-preserving.

## Schemas

`uat-matrix.csv`:

```
ID,Area,Scenario,Expected result,Release gate,Priority,Test type
```

`identity-uat-matrix.v1.csv`:

```
test_id,category,phase,persona,risk,precondition,action,expected_result,automation,source_basis
```

The identity matrix carries a `phase` column (`M0`-`M6`) describing identity-programme milestones. These are not the `B0`-`B8` implementation bodies used by `uat-ownership.v1.json`. Mapping `M` phases onto `B` bodies is an open governance decision. Until it is made, the identity matrix is validated for shape and completeness but does not gate a body.

## Validation

Both files are checked by:

```bash
node scripts/validate-okf-uat.mjs
```

The check enforces header, row count, complete ordered ID set, controlled vocabularies for category, phase, risk, and automation class, and non-empty persona, precondition, action, expected result, and source basis on every identity row.

The `conformance.yml` workflow validates draft schema shape and YAML syntax. Full OpenAPI, AsyncAPI, JSON-LD, OKF, generated SDK, and semantic mutation gates run from B1.

## B6 neutral source conformance

Lives in [`crates/fasti-application/tests/b6_client_conformance.rs`](../../crates/fasti-application/tests/b6_client_conformance.rs), not in this directory, because it is an executable suite rather than a matrix.

### What it proves, and why it is not what B6 sounds like

The public acceptance contract carries an opaque evidence digest, not source structure:

```rust
// crates/fasti-contracts/src/conformance.rs
pub struct AcceptObservationRequest {
    pub operation_id: String,
    pub occurred_at: Option<OccurredTimeDto>,
    pub observed_at: ObservedTimeDto,
    pub evidence: EvidenceReferenceDto,
}
```

There is no `media`, `source`, or `progress` field. A batch importer and a live player send byte-identical request shapes, so the contract is source-neutral **by construction**. Pushing several source payloads through it proves nothing: every payload takes one code path.

What can diverge between client shapes is behavior. The suite models four archetypes with different operational patterns and asserts they share one outcome table:

| Archetype           | Operational pattern                                                          |
| ------------------- | ---------------------------------------------------------------------------- |
| `batch_importer`    | Historical backfill, `SourceClaim` time, deterministic operation ids         |
| `live_player`       | High-frequency heartbeats, `DeviceObserved` time, reconnect burst redelivery |
| `browser_extension` | Ephemeral, no durable device identity, cross-tab duplicate submission        |
| `polling_sync`      | Periodic diff pulls with overlapping windows, `Inferred` time                |

Outcome table, identical for all four:

| Submission                     | Result                                                        |
| ------------------------------ | ------------------------------------------------------------- |
| First submit                   | `Committed`                                                   |
| Resubmit, same semantic digest | `Replayed`, receipt value equal                               |
| Resubmit, changed digest       | `IdempotencyConflict`, nothing mutated                        |
| Foreign access context         | Denied as `Forbidden`; operation and receipt counts stay zero |

A vendor-specific branch anywhere in the acceptance path makes exactly one archetype diverge and fails `every_archetype_shares_one_outcome_table`.

Receipt equality here is **semantic**, not byte-level. The assertion is `PartialEq` on the in-memory application receipt; it does not serialize `AcceptObservationResponse`, so it does not prove byte-identical transport output. Proving that needs an API-level test, which does not exist yet.

### The property that matters most

`derive_operation_id` models how a client turns a source key into an operation id. A client that mints a random id per attempt duplicates its entire backfill after a crash; one that derives the id from the source row survives a restart because the server recognises the replay. Fasti cannot tell the two apart from a single request, so the client-side contract has to be tested. `batch_importer_survives_a_restart_without_duplicating_its_backfill` covers it.

### Deliberately not covered

- **Outbox draining.** No outbox exists in `crates/`. Offline behaviour is covered only as far as the contract reaches: a delayed or reordered resubmission still deduplicates.
- **Payload byte handling.** Already covered by the evidence upload tests in `crates/fasti-store/src/evidence.rs`.

### Running it

```bash
cargo test -p fasti-application --features conformance-fixture --test b6_client_conformance
```

The suite is verified by mutation, not only by passing. Breaking determinism in `derive_operation_id` fails three tests; injecting a vendor-specific branch into the command builder fails exactly one.

## Access B TrailBase conformance

On a prepared native Linux machine, run:

```bash
cargo xtask test milestone --body B
```

The existing milestone receipt owns this package evidence. The gate verifies
the exact release lock and mutation sentinels, sole launcher, native and OCI
lifecycle, combined 192 MiB boundary, public-account lifecycle, local OIDC with
PKCE, TOTP, restart, full-depot recovery, and the test-only `v0.33.4` to
`v0.33.5` adjacent upgrade and old-backup rollback fixture.

The gate records source limitations as limitations. It does not turn them into
success claims. Remote account and OAuth exposure remains unavailable because
`v0.33.5` accepts protocol-relative redirects. The isolated administrator
listener also lacks its second-factor login route. Run the same gate on native
x86_64 and arm64 before claiming two-architecture execution.
