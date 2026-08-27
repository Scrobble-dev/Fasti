# Contract Ownership

The authored registry at [`registry/v1/capabilities.yaml`](registry/v1/capabilities.yaml) is the authoritative machine ledger for capability identifiers, bounded-context ownership, body ownership, lifecycle, scopes, problems, examples, UAT relationships, and required or reasoned-not-applicable surfaces. Generated files and prose must follow it; they must not become parallel sources of domain meaning.

## Ownership map

| Location                              | Owner and role                                                                                                       |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `registry/v1/capabilities.yaml`       | Authored capability and surface ownership                                                                            |
| `../crates/fasti-domain/`             | Domain identifiers, time values, vocabulary, and invariants                                                          |
| `../crates/fasti-application/`        | Use cases, authorization, ports, capability ownership, and typed problems                                            |
| `../crates/fasti-contracts/`          | Shared public Rust DTOs and generated capability identifiers                                                         |
| `../crates/fasti-api/`                | Real Utoipa operations: production bootstrap/authenticated routes plus a separately feature-gated conformance router |
| `asyncapi/v1/transport.yaml`          | Authored `receipt.stream` SSE channel and message flow                                                               |
| `jsonld/v1/`                          | Authored JSON-LD 1.1 context and vocabulary                                                                          |
| `okf/v1/`                             | Authored operational knowledge for capabilities, scopes, and problems                                                |
| `examples/v1/`                        | Registry-owned semantic examples, validated against their contract surfaces                                          |
| `portability/v1/`                     | Frozen internal archive-v1 schema and example; retained for restore compatibility                                    |
| `portability/v2/`                     | Generated archive-v2 schema and example; adds metadata and profile tracking state                                    |
| `generated/v1/`                       | Deterministic projections: production and conformance OpenAPI, registry, and JSON Schemas                            |
| `../packages/sdk/`                    | Generated typed TypeScript HTTP/SSE client and parsers                                                               |
| `../crates/fasti-cli/`                | Local `capability list/show` projection of the generated public registry                                             |
| `../tests/conformance/uat-matrix.csv` | Acceptance inventory and explicit body relationships                                                                 |

The production OpenAPI document contains health, loopback bootstrap, browser account, observation, identity-record, and profile-state operations. It declares the opaque bearer and browser-session cookie schemes and the allowed alternative for each authenticated data route. `fastid` mounts bootstrap only on loopback with an explicit `FASTI_DATA_ROOT`; the authenticated subset can also mount behind the explicit trusted HTTPS proxy boundary. The separate B1 conformance OpenAPI and router are compiled only with `conformance-fixture`; its server binds only to IPv4 loopback, keeps bounded state in memory, and declares fixture-only, no-durability success. The SSE receipt channel belongs to AsyncAPI rather than being added to the finite OpenAPI document. Browser authentication is finite request/response state, so its registry-owned AsyncAPI surface is explicitly not applicable.

JSON Schema uses draft 2020-12. JSON-LD and OKF remain separate governed surfaces: operational access or administration can be reasoned `N/A` for linked data while still requiring recovery knowledge. The SDK, CLI discovery, examples, and documentation consume the same registry instead of re-declaring capability meaning.

Run `cargo xtask contract generate` to regenerate checked-in projections. Run `cargo xtask contract verify --locked` to prove deterministic bytes, checked-in drift, semantic examples, standards validation, Rust/TypeScript parity, package truth, and other B1 software gates. The [local TypeScript SDK guide](../packages/sdk/README.md) provides a copy-paste health check and focused black-box contract test. A verifier receipt proves only the software contract spine. B1 remains open until the current aggregate milestone manifest, including Tauri and both low-hardware envelope architectures, passes. B2 is not authorized.

Provider seeds and manifest examples remain future adapter inputs, not working integrations. Durable observation, browser authentication, identity-record, and profile tracking paths are production HTTP operations; identity review and portability remain staged. The pre-production Workbench consumes the generated contract without owning a second API shape. There is no supported install, release, deployed web application, desktop package, or player.

The [archive-v1 schema](portability/v1/workspace-manifest.schema.json) and
[example](portability/v1/workspace-manifest.example.json) keep the original 16
streams frozen for restore compatibility. The generated
[archive-v2 schema](portability/v2/workspace-manifest.schema.json) and
[example](portability/v2/workspace-manifest.example.json) retain that exact
prefix and append metadata claims, metadata overrides, and profile tracking
dispositions. The Rust contract owns both strict hostile-input
conversion and the application-to-wire RFC 8785/JCS projection. Freezing the
archive format does not add a public capability, registry entry, route, SDK
method, or CLI operation. The outbound projection owns the checked DTO,
canonical `manifest.json` bytes, application manifest, and digest as one opaque
unit; it has no consuming parts API. Hostile inbound conversion returns a
contract-owned verified manifest whose construction is private. Store adapters
must not rebuild or independently pair wire values. Restore success is
complete-only. Rejection and post-activation recovery-bootstrap pending states
remain typed staged failures. Restore uses the non-delegable `local_operator`
authorization disposition with no credential scope. Recovery bootstrap prepare
and complete remain phases of that restore capability; they do not create a
second capability. This metadata does not add a route, SDK method, or CLI
operation.

See the [human capability guide](../docs/capability-ledger.md) for a concise interpretation of the registry.
