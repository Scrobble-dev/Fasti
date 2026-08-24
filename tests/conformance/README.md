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
