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

| Capability group                                                            | Runtime truth                                                                                                                 |
| --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `system.health`                                                             | Implemented by the production daemon and its production OpenAPI document                                                      |
| `system.node.initialize` and `access.client.enroll`                         | Durable production routes on loopback when `FASTI_DATA_ROOT` is set; also covered by the nondurable fixture                   |
| Browser session and browser-user administration                             | Durable local and explicitly trusted-proxy remote routes; sessions remain separate from scoped bearer integration credentials |
| `observation.accept`                                                        | Durable production HTTP route (`POST /api/v1/observations`), authorized by a scoped bearer credential or browser session      |
| Other B1 administration, receipt, and `receipt.stream` capabilities         | Executable only in the feature-gated loopback conformance fixture; state is bounded, in-memory, and nondurable                |
| Identity records, identifiers, namespaces, and profile tracking disposition | Durable local and authenticated remote HTTP routes, covered by `cargo xtask contract verify`                                  |
| Identity review (inspect, defer, resume, resolve)                           | Implemented behind internal B2 ports for review; no production route exists                                                   |
| Corrections and portability                                                 | Implemented behind internal B3 ports for review; export, restore, and verify remain explicit nonzero CLI guards               |
| Browser Workbench                                                           | Pre-production browser session and implemented data surfaces; not a supported installation or release                         |
| Trusted desktop network settings and Google Books search                    | Local Tauri IPC review body; no public HTTP route, event, record write, or browser provider execution                         |
| Product packaging and release behavior                                      | Later bodies; absent now                                                                                                      |

The fixture separates contract proof from availability claims. Its finite routes are generated into a dedicated conformance OpenAPI document. `receipt.stream` is governed as an AsyncAPI 3.x SSE operation. Successful fixture responses identify `fixture_only` availability and `none` durability; problem-only routes cannot imply a false success. The production router mounts health, durable node setup (initialize/enroll), observation acceptance, and the identity records/identifiers/namespaces routes above. All other fixture paths return `404` in production.

Required surfaces are generated or validated from the registry across domain/application ownership, OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, semantic examples, CLI, typed TypeScript HTTP/SSE SDK, knowledge, and package smoke. Reasoned `N/A` is explicit—for example, operational health has no event stream and access administration is not linked-data domain state.

The browser Workbench consumes generated production DTO parsers and browser-session cookies. Its global search uses loaded records and valid navigation commands. Its shared, configurable record-action registry mutates only active host capabilities; unsupported completion, progress, watchlist, collection, review, and tag operations stay disabled. Profile tracking disposition uses the governed profile-state route on web and the same application port on Desktop. Integration clients continue to use separately revocable scoped bearer credentials. The local Vite proxy is QA tooling, not an endpoint-configuration authority. Browser account request/response operations are OpenAPI-owned; they do not add an AsyncAPI event channel or JSON-LD domain entity.

The trusted Tauri host can persist non-secret network preferences, test a
configured Fasti service, store a Google Books key in the platform credential
store, and return bounded neutral search candidates. These review commands are
not daemon HTTP routes and do not activate public provider or media-record
capabilities. OpenAPI, AsyncAPI, JSON Schema, and JSON-LD remain not applicable
to this local IPC body.

`cargo xtask contract verify --locked` is the deterministic software gate. Its success receipt does not close B1. The mandatory headless QA and developer-experience gates also pass on this branch. Closure requires the exact-head aggregate manifest, including the governed Tauri package and same-workflow-attempt x86_64/aarch64 low-hardware envelope packages from one exact `dev` push. Until `cargo xtask test milestone --body B1` passes, B1 remains in progress and B2 is not authorized.

`cargo xtask test milestone --body B8b` is the fail-closed gate for public release readiness (checksums, SBOM, provenance, final security review, and release notes — see [B8b release readiness](architecture/b8b-release-readiness.md)). It stays unsatisfiable today: it requires a passing B8a manifest as a prerequisite, and B8a's own evidence formalization is not implemented; and it requires a `Pass` design review, which cannot be legitimately claimed until B4 (the UI) ships. Preparing this evidence does not activate release behavior — the table above still holds until B8's explicit, out-of-band publishing action.

Reserved identifiers do not authorize early request shapes, success behavior, persistence, installation, release, UI, or playback. Fasti records; players play.
