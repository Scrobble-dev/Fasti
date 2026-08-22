# Conformance and UAT Ownership

`uat-matrix.csv` is the acceptance inventory supplied to the identity-first plan. A row describes required behavior; it does not claim that the behavior is implemented.

B0 owns repository-truth, guarded-command, workflow-policy, draft-syntax, native-build, and OCI-smoke evidence. B1 imports each UAT ID into the capability registry and marks it `direct`, `split`, or `deferred` with one owning body and reason. B2 and B3 bind implemented rows to executable API, CLI, SDK, persistence, recovery, and network-denied evidence.

No UAT row passes from prose, syntax parsing, or a mocked UI. Evidence must identify the source commit, artifact, command, environment, and result. Hardware claims also identify the physical profile.

The current `conformance.yml` workflow validates draft schema shape and YAML syntax only. Full OpenAPI, AsyncAPI, JSON-LD, OKF, generated SDK, and semantic mutation gates begin in B1.
