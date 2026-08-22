# Contract Ownership

The current files are governed drafts, not proof of a working public contract.

| Location | B0 role | Future owner |
|---|---|---|
| `identity/identity-contract-seed.yaml` | Provider-neutral identity input preserved for reconciliation | B1 `fasti-contracts` authored semantic source |
| `addons/manifests/*.provider.yaml` | Example manifest shape; not a working provider adapter | B5 declarative enrichment and B6 conformance |
| `../packages/schemas/schemas/` | JSON Schema draft validated against examples | B1 deterministic generator from shared DTOs |
| `../fixtures/activity/` | Governed example observations | B1/B2 conformance corpus |
| `../tests/conformance/uat-matrix.csv` | Acceptance inventory and later-body ownership | Versioned capability registry plus executable tests |

B1 adds explicit authored locations for transport-only AsyncAPI behavior and JSON-LD vocabularies/contexts. Real Utoipa-bound handlers and shared public Rust DTOs own OpenAPI operations and data shapes. Generated OpenAPI, assembled AsyncAPI, JSON Schema, JSON-LD, OKF, examples, errors, permissions, and TypeScript SDK artifacts must not become parallel sources of meaning.

See the [capability ledger](../docs/capability-ledger.md) for current surface status.
