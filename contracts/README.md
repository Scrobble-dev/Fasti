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
| `portability/v3/`–`portability/v6/`    | Frozen claim, refresh, identity-routing and Search-action archive formats retained for restore                      |
| `portability/v7/`                     | Current staged format; adds nullable response-policy evidence to the existing claim registry, with 35 streams       |
| `generated/v1/`                       | Deterministic projections: production and conformance OpenAPI, registry, and JSON Schemas                            |
| `../packages/sdk/`                    | Generated typed TypeScript HTTP/SSE client and parsers                                                               |
| `../crates/fasti-cli/`                | Local `capability list/show` projection of the generated public registry                                             |
| `../tests/conformance/uat-matrix.csv` | Acceptance inventory and explicit body relationships                                                                 |

The production OpenAPI document contains health, loopback bootstrap, observation, identity-record, profile tracking, profile catalog-configuration, and finite C1 browser-authentication and session operations. The Nuvio custom Collections request is the exact bare-array interchange shape; responses wrap the current optional profile document. The application applies the full bounded compatibility rules before persistence. OpenAPI declares bootstrap, credential bearer, browser-session cookie, CSRF, and continuation-cookie security where each operation requires them. The TrailBase callback is browser navigation and has no SDK method. First-administrator bootstrap is a trusted local Unix CLI operation and has no HTTP or SDK operation. Windows first-administrator setup remains deferred with packaged-host authentication. `fastid` mounts bootstrap only on loopback with an explicit `FASTI_DATA_ROOT`; the authenticated bearer subset can also mount behind the explicit trusted HTTPS proxy boundary. C1 routes mount only on the exact requested-and-bound `127.0.0.1:8420` durable listener. Fallback, alternate-loopback, generic local, integration, wildcard or container forwarding, and remote routers omit them. The separate B1 conformance OpenAPI and router are compiled only with `conformance-fixture`; its server binds only to IPv4 loopback, keeps bounded state in memory, and declares fixture-only, no-durability success. The SSE receipt channel belongs to AsyncAPI rather than being added to the finite OpenAPI document. Browser authentication and profile catalog configuration are finite request/response state, so their registry-owned AsyncAPI surfaces are explicitly not applicable.

JSON Schema uses draft 2020-12. JSON-LD and OKF remain separate governed surfaces: operational access or administration can be reasoned `N/A` for linked data while still requiring recovery knowledge. The SDK, CLI discovery, examples, and documentation consume the same registry instead of re-declaring capability meaning.

Run `cargo xtask contract generate` to regenerate checked-in projections. Run `cargo xtask contract verify --locked` to prove deterministic bytes, checked-in drift, semantic examples, standards validation, Rust/TypeScript parity, package truth, and other B1 software gates. The [local TypeScript SDK guide](../packages/sdk/README.md) provides a copy-paste health check and focused black-box contract test. A verifier receipt proves only the software contract spine. B1 remains open until the current aggregate milestone manifest, including Tauri and both low-hardware envelope architectures, passes. B2 is not authorized.

Provider seeds and manifest examples remain future adapter inputs, not working integrations. Durable observation, identity-record, profile tracking, and fixed-direct-listener C1 paths are production HTTP operations in local source. C1 exchange and new session issuance require a verified installation receipt and persisted active TrailBase activation. Identity review and portability remain staged. The pre-production Workbench consumes the generated contract without owning a second API shape. The implemented Access projection UI binding does not claim package smoke. There is no supported install, release, deployed web application, desktop package, or player. C1 ordinary-browser delivery merged in [PR #119](https://github.com/Scrobble-dev/Fasti/pull/119), with exact evidence in the [canonical checkpoint](../docs/plans/trailbase-authentication-remediation.md#24-c1-delivery-and-c2-foundation-checkpoint). Packaged WebView, cross-platform, and packaged assistive-technology proof remain deferred to `C1-TAURI-AUTH`; supported-package and public-release evidence remain separate.

Archive v7 accompanies append-only schema v17. It carries bounded canonical
`response_policy_json` in `metadata_claims`, for both field and rating claim
owners. NULL preserves unknown historical policy; it does not grant upstream
reuse permission. The v1–v6 files remain unchanged. Restore uses the original
version's row shape for both decoding and exact post-import stream verification.
Its existing bounded preflight rejects malformed/no-store policy and crossed
v6/v7 claim shapes before staging unchanged verified archive bytes. The same
strict decoder runs again during import; between-pass source changes remain a
separate digest-checked restore failure, not proof of immutable input.

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
