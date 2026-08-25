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

| Capability group                                                                                                                                     | Runtime truth                                                                                                   |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `system.health`                                                                                                                                      | Implemented by the production daemon and its health-only OpenAPI document                                       |
| B1 capability discovery, node/client setup, profile/credential/listener administration, observation acceptance, receipt replay, and `receipt.stream` | Executable only in the feature-gated loopback conformance fixture; state is bounded, in-memory, and nondurable  |
| Identity records, identifiers, and review                                                                                                            | Implemented behind internal B2 ports for review; no production runtime exists                                   |
| Corrections and portability                                                                                                                          | Implemented behind internal B3 ports for review; export, restore, and verify remain explicit nonzero CLI guards |
| Browser QA harness                                                                                                                                  | Local, private `system.health` evidence tooling; not a product UI or capability activation                      |
| Product web, desktop, provider, packaging, and release behavior                                                                                      | Later bodies; absent now                                                                                        |

The fixture separates contract proof from availability claims. Its finite routes are generated into a dedicated conformance OpenAPI document. `receipt.stream` is governed as an AsyncAPI 3.x SSE operation. Successful fixture responses identify `fixture_only` availability and `none` durability; problem-only routes cannot imply a false success. The production `fastid` router mounts none of them and returns `404` for those paths.

Required surfaces are generated or validated from the registry across domain/application ownership, OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, semantic examples, CLI, typed TypeScript HTTP/SSE SDK, knowledge, and package smoke. Reasoned `N/A` is explicit—for example, operational health has no event stream and access administration is not linked-data domain state.

The browser harness consumes the generated `system.health` SDK parser and adds no API shape, domain state, retry queue, persistence, or public binding. Its local Vite proxy is not an endpoint-configuration authority. The registry therefore keeps the product UI surface not applicable through B3.

`cargo xtask contract verify --locked` is the deterministic software gate. Its success receipt does not close B1. The mandatory headless QA and developer-experience gates also pass on this branch. Closure requires the exact-head aggregate manifest, including the governed Tauri package and same-workflow-attempt x86_64/aarch64 low-hardware envelope packages from one exact `dev` push. Until `cargo xtask test milestone --body B1` passes, B1 remains in progress and B2 is not authorized.

`cargo xtask test milestone --body B8b` is the fail-closed gate for public release readiness (checksums, SBOM, provenance, final security review, and release notes — see [B8b release readiness](architecture/b8b-release-readiness.md)). It stays unsatisfiable today: it requires a passing B8a manifest as a prerequisite, and B8a's own evidence formalization is not implemented; and it requires a `Pass` design review, which cannot be legitimately claimed until B4 (the UI) ships. Preparing this evidence does not activate release behavior — the table above still holds until B8's explicit, out-of-band publishing action.

Reserved identifiers do not authorize early request shapes, success behavior, persistence, installation, release, UI, or playback. Fasti records; players play.
