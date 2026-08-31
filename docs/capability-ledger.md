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

| Capability group                                                                                             | Runtime truth                                                                                                                                                             |
| ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `system.health`                                                                                              | Implemented by the production daemon and its production OpenAPI document                                                                                                  |
| `system.node.initialize` and `access.client.enroll`                                                          | Durable production routes with `FASTI_DATA_ROOT` and direct loopback or an explicit loopback-only port forward; also covered by the nondurable fixture                    |
| C1 browser authentication and sessions                                                                       | Implemented in local source only on the exact requested-and-bound `127.0.0.1:8420` durable listener; exchange and new issuance require verified active TrailBase installation evidence |
| TrailBase account lifecycle                                                                                  | Exact v0.33.5 separate service; native/OCI operations and prepared-machine account, social OIDC/PKCE, TOTP, deletion, backup, and restore conformance pass. C1 uses a direct local exchange and discards vendor tokens. |
| `observation.accept`                                                                                         | Durable production HTTP route (`POST /api/v1/observations`), authorized by a scoped bearer client credential                                                              |
| Other B1 administration, receipt, and `receipt.stream` capabilities                                          | Executable only in the feature-gated loopback conformance fixture; state is bounded, in-memory, and nondurable                                                            |
| Identity records, identifiers, namespaces, profile tracking disposition, and Nuvio Collections configuration | Durable local and authenticated remote HTTP routes, covered by `cargo xtask contract verify`                                                                              |
| Identity review (inspect, defer, resume, resolve)                                                            | Implemented behind internal B2 ports for review; no production route exists                                                                                               |
| Corrections and portability                                                                                  | Implemented behind internal B3 ports for review; export, restore, and verify remain explicit nonzero CLI guards                                                           |
| Browser Workbench                                                                                            | Pre-production data surfaces plus Gate 10 A permanent Account and security and C separate resumable first-run views from one Access projection; B is in-context evidence |
| M1 provider registry, credentials, and health                                                                | Durable scoped HTTP/SDK/UI status and write-only credential management; shared governed Google Books/TMDB runtime; ten additional providers remain explicitly unavailable |
| Trusted desktop provider metadata                                                                            | Shared governed search/read runtime, bounded local artwork, and atomic Google Books/TMDB claim writes; Desktop credentials use the platform credential store              |
| Product packaging and release behavior                                                                       | Later bodies; absent now                                                                                                                                                  |

The fixture separates contract proof from availability claims. Its finite routes are generated into a dedicated conformance OpenAPI document. `receipt.stream` is governed as an AsyncAPI 3.x SSE operation. Successful fixture responses identify `fixture_only` availability and `none` durability; problem-only routes cannot imply a false success. The production router mounts health, durable node setup (initialize/enroll), observation acceptance, and the identity records/identifiers/namespaces routes above. Only the exact requested-and-bound `127.0.0.1:8420` durable composition also mounts C1. Fallback, alternate-loopback, generic local, integration, wildcard or container forwarding, and remote compositions do not. All other fixture paths return `404` in production.

Required surfaces are generated or validated from the registry across domain/application ownership, OpenAPI 3.1, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, semantic examples, CLI, typed TypeScript HTTP/SSE SDK, knowledge, and package smoke. Reasoned `N/A` is explicit—for example, operational health has no event stream and access administration is not linked-data domain state.

The browser Workbench consumes generated production DTO parsers for active data capabilities. Its C1 source uses one generated Access projection for A and C. It never reads TrailBase tokens. Its global search uses loaded records and valid navigation commands. Its shared, configurable record-action registry mutates only active host capabilities; unsupported completion, progress, watchlist, collection membership, review, and tag operations stay disabled. Profile tracking disposition and Nuvio custom Collections configuration use governed profile-state routes on web and the same application ports on Desktop. Collections import accepts the NuvioTV bare-array wire shape, normalizes it under fixed bounds, and stores one document per workspace/profile. It never fetches imported URLs. Integration clients continue to use separately revocable scoped bearer credentials. The local Vite proxy is QA tooling, not an endpoint-configuration authority.

PR A records these authentication contract dispositions:

| Surface                    | Disposition                                                                                                                                 |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Production OpenAPI and SDK | No human-account or browser-session route. C1 owns activation and the public contract.                                                      |
| AsyncAPI                   | `N/A — PR A exposes no externally visible asynchronous authentication event.`                                                               |
| JSON-LD                    | `N/A — subjects, sessions, credentials, and tokens are security state, not public semantic entities.`                                       |
| Public CLI                 | `N/A — PR A uses direct deterministic domain, application, and store fixtures; C1 owns trusted identity bootstrap and activation commands.` |

Reserved or dormant authentication identifiers do not authorize a route,
session, fixture listener, success response, or UI success state.

C1 adds finite OpenAPI, JSON Schema, problem, capability, and TypeScript SDK
surfaces for sign-in, continuation, Access projection, and browser-session
management. Its callback is browser navigation and has no SDK method.
First-administrator bootstrap is packaged-host IPC. AsyncAPI and JSON-LD remain
not applicable. The implemented projection UI binding does not claim package
smoke.

PR B records these TrailBase contract dispositions:

| Surface               | Disposition                                                                                                                                      |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Vendor OpenAPI        | The exact runtime `openapi print` output is captured by SHA-256 in the account-conformance receipt. Fasti does not copy or rename vendor routes. |
| Fasti OpenAPI and SDK | No TrailBase exchange or Fasti browser-session route. C1 owns the adapter contract.                                                              |
| AsyncAPI              | `N/A — the PR B operator and account lifecycle has no Fasti asynchronous event surface.`                                                         |
| JSON-LD               | `N/A — human credentials, sessions, TOTP state, and provider links are private security state.`                                                  |
| CLI                   | `scripts/dev.sh trailbase` is the sole operator entry point. It does not issue a Fasti credential.                                               |
| Remote account routes | `Unavailable — TrailBase v0.33.5 accepts protocol-relative redirects. Keep the account and OAuth listener on loopback.`                          |
| Upgrade and rollback  | Exact test-only v0.33.4 to v0.33.5 adjacent artifact replacement and old full-depot rollback; no schema migration is claimed.                    |

The trusted Tauri host can persist non-secret network preferences, test a
configured Fasti service, store Google Books and TMDB credentials in the
platform credential store, and return bounded neutral search candidates. The
durable daemon exposes scoped provider inventory, write-only credential
configuration/removal/test, and health routes through the same provider runtime.
It refetches the exact selected item, downloads provider artwork through a
separate governed request into a bounded owner-only cache, and serves only a
narrowly scoped local Tauri asset to the Desktop UI. The application then
atomically creates a Record with its identifier and claims or appends refreshed
claims to an existing Record. These mutations are not daemon HTTP routes. The
additive Record read projection is generated into OpenAPI and the TypeScript
SDK; AsyncAPI and JSON-LD remain not applicable to this local IPC body.

`cargo xtask contract verify --locked` is the deterministic software gate. Its success receipt does not close B1, PR A, or C1. B1 closure requires evidence bound to the exact reviewed head, including the applicable headless, developer-experience, Tauri, and same-workflow-attempt x86_64/aarch64 low-hardware packages. Until `cargo xtask test milestone --body B1` passes, B1 remains in progress and B2 is not authorized. `cargo xtask test milestone --body C1` writes the in-scope C1 delivery receipt. It excludes packaged Tauri authentication, cross-platform WebView behavior, and packaged assistive-technology proof; those remain visible under `C1-TAURI-AUTH` and are not claimed. C1 review, exact-head, merge, and merged-tree evidence remain pending.

`cargo xtask test milestone --body B8b` is the fail-closed gate for public release readiness (checksums, SBOM, provenance, final security review, and release notes — see [B8b release readiness](architecture/b8b-release-readiness.md)). It stays unsatisfiable today: it requires a passing B8a manifest as a prerequisite, and B8a's own evidence formalization is not implemented; and it requires a `Pass` design review, which cannot be legitimately claimed until B4 (the UI) ships. Preparing this evidence does not activate release behavior — the table above still holds until B8's explicit, out-of-band publishing action.

Reserved identifiers do not authorize early request shapes, success behavior, persistence, installation, release, UI, or playback. Fasti records; players play.
