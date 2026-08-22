# Contract Ownership

The authored registry at [`registry/v1/capabilities.yaml`](registry/v1/capabilities.yaml) is the authoritative machine ledger for capability identifiers, bounded-context ownership, body ownership, lifecycle, scopes, problems, examples, UAT relationships, and required or reasoned-not-applicable surfaces. Generated files and prose must follow it; they must not become parallel sources of domain meaning.

## Ownership map

| Location                              | Owner and role                                                                               |
| ------------------------------------- | -------------------------------------------------------------------------------------------- |
| `registry/v1/capabilities.yaml`       | Authored capability and surface ownership                                                    |
| `../crates/fasti-domain/`             | Domain identifiers, time values, vocabulary, and invariants                                  |
| `../crates/fasti-application/`        | Use cases, authorization, ports, capability ownership, and typed problems                    |
| `../crates/fasti-contracts/`          | Shared public Rust DTOs and generated capability identifiers                                 |
| `../crates/fasti-api/`                | Real Utoipa operations: production health plus a separately feature-gated conformance router |
| `asyncapi/v1/transport.yaml`          | Authored `receipt.stream` SSE channel and message flow                                       |
| `jsonld/v1/`                          | Authored JSON-LD 1.1 context and vocabulary                                                  |
| `okf/v1/`                             | Authored operational knowledge for capabilities, scopes, and problems                        |
| `examples/v1/`                        | Registry-owned semantic examples, validated against their contract surfaces                  |
| `generated/v1/`                       | Deterministic projections: production and conformance OpenAPI, registry, and JSON Schemas    |
| `../packages/sdk/`                    | Generated typed TypeScript HTTP/SSE client and parsers                                       |
| `../crates/fasti-cli/`                | Local `capability list/show` projection of the generated public registry                     |
| `../tests/conformance/uat-matrix.csv` | Acceptance inventory and explicit body relationships                                         |

The production OpenAPI document contains only `GET /api/v1/health`, matching the route mounted by `fastid`. The separate B1 conformance OpenAPI and router are compiled only with `conformance-fixture`; its server binds only to IPv4 loopback, keeps bounded state in memory, and declares fixture-only, no-durability success. The SSE receipt channel belongs to AsyncAPI rather than being smuggled into the finite OpenAPI document. None of these fixture surfaces authorize B2 runtime behavior.

JSON Schema uses draft 2020-12. JSON-LD and OKF remain separate governed surfaces: operational access or administration can be reasoned `N/A` for linked data while still requiring recovery knowledge. The SDK, CLI discovery, examples, and documentation consume the same registry instead of re-declaring capability meaning.

Run `cargo xtask contract generate` to regenerate checked-in projections. Run `cargo xtask contract verify --locked` to prove deterministic bytes, checked-in drift, semantic examples, standards validation, Rust/TypeScript parity, package truth, and other B1 software gates. The [local TypeScript SDK guide](../packages/sdk/README.md) provides a copy-paste health check and focused black-box contract test. A verifier receipt proves only the software contract spine. B1 remains open until named physical Raspberry Pi 5 and J4125 RAM evidence exists. B2 is not authorized.

Provider seeds and manifest examples remain future adapter inputs, not working integrations. There is no supported install, release, persistence kernel, web interface, desktop package, or player.

See the [human capability guide](../docs/capability-ledger.md) for a concise interpretation of the registry.
