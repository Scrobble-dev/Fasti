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

| Capability group                                                                 | Runtime truth                                                                                                   |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `system.health`                                                                  | Implemented by the production daemon and its production OpenAPI document                                        |
| `system.node.initialize` and `access.client.enroll`                              | Durable production routes with `FASTI_DATA_ROOT` and direct loopback or an explicit loopback-only port forward |
| `observation.accept`                                                             | Durable production HTTP route on the same local exposure (`POST /api/v1/observations`), bearer-authenticated    |
| Other B1 administration, receipt, and `receipt.stream` capabilities              | Executable only in the feature-gated loopback conformance fixture; state is bounded, in-memory, and nondurable  |
| Identity records, identifiers, and namespaces                                    | Durable production HTTP routes on the same local exposure (`/api/v1/records`, `/api/v1/records/identifiers`, `/api/v1/namespaces`), bearer-authenticated, covered by `cargo xtask contract verify` |
| Identity review (inspect, defer, resume, resolve)                                | Implemented behind internal B2 ports for review; no production route exists                                     |
| Corrections and portability                                                      | Implemented behind internal B3 ports for review; export, restore, and verify remain explicit nonzero CLI guards |
| Browser Workbench                                                                | Local, private B4 review surface; `/` renders the media Workbench and `/status` keeps the separate health diagnostic |
| Trusted desktop network settings and Google Books/TMDB search                    | Local Tauri IPC review body; no public HTTP route, event, record write, or browser provider execution           |
| Product packaging and release behavior                                           | Later bodies; absent now                                                                                        |

The fixture separates contract proof from availability claims. Its finite routes are generated into a dedicated conformance OpenAPI document. `receipt.stream` is governed as an AsyncAPI 3.x SSE operation. Successful fixture responses identify `fixture_only` availability and `none` durability; problem-only routes cannot imply a false success. The production router mounts health, durable node setup (initialize/enroll), observation acceptance, and the identity records/identifiers/namespaces routes above. All other fixture paths return `404` in production.

Required surfaces are generated or validated from the registry across domain/application ownership, OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, semantic examples, CLI, typed TypeScript HTTP/SSE SDK, knowledge, and package smoke. Reasoned `N/A` is explicit—for example, operational health has no event stream and access administration is not linked-data domain state.

The browser Workbench consumes generated SDK parsers and adds no API shape,
domain state, retry queue, persistence, or public binding. Its local Vite proxy
is not an endpoint-configuration authority. The Workbench is review code, not a
published web product or B4 completion claim.

## Workbench presentation truth

| Affordance                                                                                          | Current truth                                                                                                                 | Owner before activation                                                                                                                                  |
| --------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Product root and media navigation                                                                   | `/` renders the preserved Tabler-based Workbench. Empty and error states come from the host; no mock catalogue is loaded.     | B4 presentation evidence and B8 packaging remain open.                                                                                                   |
| Local service status                                                                                | `/status` consumes the generated `system.health` parser.                                                                      | `system.health`; no new capability required.                                                                                                             |
| Browser record list                                                                                 | `GET /api/v1/records` uses a real `identity_read` bearer held only in tab memory. Reload clears it.                           | Existing `identity.record.list` registry/OpenAPI/SDK surface.                                                                                            |
| Tauri record list                                                                                   | Trusted host invokes the authenticated local records query without an HTTP round trip.                                        | Existing access and identity application ports.                                                                                                          |
| Activity, watch-state, watchlist, collection, rating, review, note, tag, artwork, and episode edits | Approved controls remain visible but disabled when the host has no matching command. The UI does not mutate local mock state. | B4 must land each domain/application capability, registry disposition, applicable OpenAPI/SDK or IPC adapter, typed recovery, and E2E evidence together. |
| API credential connection                                                                           | Active in the browser for record reads; active API-client administration stays in the trusted packaged host.                  | [Authentication boundaries](architecture/authentication.md).                                                                                             |
| Passkey, OIDC/SSO, NuvioTV device, and master-password tabs                                         | Preserved with precise unavailable states. They submit no placeholder challenge, code, or password.                           | The owned completion gates are in [authentication boundaries](architecture/authentication.md).                                                           |
| Provider/display preferences and custom type/field editors                                          | Preserved with saved values visible, but disabled. No active search, Records, or node-schema path consumes them yet.          | Add the owned domain contract and host adapter before enabling each control.                                                                             |
| Settings composition                                                                                | `RuntimeSettingsView` is the single product/export path. The older duplicate is parked as migration reference.                | Delete the parked source only after its distinct design intent is classified or migrated; never restore it as a parallel settings surface.               |

The trusted Tauri host can persist non-secret network preferences, test a
configured Fasti service, store Google Books and TMDB credentials in the
platform credential store, and return bounded neutral search candidates. These
review commands are not daemon HTTP routes and do not activate public provider
or media-record capabilities. OpenAPI, AsyncAPI, JSON Schema, and JSON-LD remain
not applicable to this local IPC body.

`cargo xtask contract verify --locked` is the deterministic software gate. Its success receipt does not close B1. The mandatory headless QA and developer-experience gates also pass on this branch. Closure requires the exact-head aggregate manifest, including the governed Tauri package and same-workflow-attempt x86_64/aarch64 low-hardware envelope packages from one exact `dev` push. Until `cargo xtask test milestone --body B1` passes, B1 remains in progress and B2 is not authorized.

`cargo xtask test milestone --body B8b` is the fail-closed gate for public release readiness (checksums, SBOM, provenance, final security review, and release notes — see [B8b release readiness](architecture/b8b-release-readiness.md)). It stays unsatisfiable today: it requires a passing B8a manifest as a prerequisite, and B8a's own evidence formalization is not implemented; and it requires a `Pass` design review, which cannot be legitimately claimed until B4 (the UI) ships. Preparing this evidence does not activate release behavior — the table above still holds until B8's explicit, out-of-band publishing action.

Reserved identifiers do not authorize early request shapes, success behavior, persistence, installation, release, UI, or playback. Fasti records; players play.
