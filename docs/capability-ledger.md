# Capability Ledger

The authoritative ledger is the versioned machine-readable [`contracts/registry/v1/capabilities.yaml`](../contracts/registry/v1/capabilities.yaml). It owns stable capability IDs, bounded contexts, contract and runtime bodies, lifecycle, scopes, problems, examples, UAT relationships, and every required or reasoned-not-applicable surface. This page explains that registry; it does not duplicate or override it.

Inspect the generated public projection with:

```bash
fasti capability list
fasti capability show receipt.stream
fasti capability show observation.accept --output json
```

Those commands report contract state. They do not activate later-body runtime behavior.

## Current runtime truth

| Capability group                                                                                                                                     | Runtime truth                                                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `system.health`                                                                                                                                      | Implemented by the production daemon and its health-only OpenAPI document                                      |
| B1 capability discovery, node/client setup, profile/credential/listener administration, observation acceptance, receipt replay, and `receipt.stream` | Executable only in the feature-gated loopback conformance fixture; state is bounded, in-memory, and nondurable |
| Identity records, identifiers, and review                                                                                                            | Reserved for B2; no production runtime exists                                                                  |
| Corrections and portability                                                                                                                          | Reserved for B3; export, restore, and verify remain explicit nonzero CLI guards                                |
| Web, desktop, provider, packaging, and release behavior                                                                                              | Later bodies; absent now                                                                                       |

The fixture separates contract proof from availability claims. Its finite routes are generated into a dedicated conformance OpenAPI document. `receipt.stream` is governed as an AsyncAPI 3.x SSE operation. Successful fixture responses identify `fixture_only` availability and `none` durability; problem-only routes cannot imply a false success. The production `fastid` router mounts none of them and returns `404` for those paths.

Required surfaces are generated or validated from the registry across domain/application ownership, OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, semantic examples, CLI, typed TypeScript HTTP/SSE SDK, knowledge, and package smoke. Reasoned `N/A` is explicit—for example, operational health has no event stream and access administration is not linked-data domain state.

`cargo xtask contract verify --locked` is the deterministic software gate. Its success receipt does not close B1. Closure also requires named physical Raspberry Pi 5 and J4125 RAM evidence and mandatory QA/developer-experience receipts. Until those exist, B1 remains in progress and B2 is not authorized.

Reserved identifiers do not authorize early request shapes, success behavior, persistence, installation, release, UI, or playback. Fasti records; players play.
