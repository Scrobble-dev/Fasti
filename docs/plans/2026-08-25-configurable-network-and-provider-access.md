# Configurable network and provider access

**Status:** Accepted implementation plan

**Date:** 2026-08-25

**Branch:** `codex/configurable-network-providers`

**PR dependencies:** repair and land PR #51 before replacing or reducing PR #44; PR #52 is not a dependency.

## Outcome

Fasti must let an operator configure how a client reaches a node without changing how the node binds. The same Settings surface must show the effective endpoint, its source, loopback aliases, port, trust state, and outbound provider policy. Provider credentials must be write-only and must drive real, scoped Discover results. No control may report success before a real host command succeeds.

The first complete slice supports native `fastid`, Docker or Podman, the Tauri desktop host, and a Tauri Android build. TLS remains owned by the deployment edge or the operating system trust store. Fasti does not mint a private CA, store CA private keys, disable certificate validation, or expose unauthenticated remote capability routes.

## Scope decision

The work exceeds eight files because it crosses four delivery boundaries and repairs a false UI surface. The user accepted the complete option. The implementation remains split into reviewable vertical slices:

1. repair the PR #51 governance baseline;
2. replace the false Settings and Discover behavior on the PR #44 source branch;
3. add endpoint, port, trust, and policy configuration;
4. add one real provider path before broadening provider coverage;
5. prove native, container, desktop, and Android configuration paths.

The plan does not introduce a generic policy language, a Fasti TLS server, a certificate authority, a plugin runtime, or remote multi-tenant access.

## Root cause

PR #44 introduced a static presentation prototype, not a provider regression:

- `fasti-workbench.svelte` imports provider state and Discover records from `mock-data.ts`.
- saving a provider key only changes an in-memory Svelte array;
- typing in Discover filters six sample records and displays an online state without a request;
- `fastid` mounts health and loopback setup only;
- the sole Google Books manifest is explicitly illustrative and has no parser or executor;
- listener bind, advertised URL, client endpoint, development port, container host port, and Tauri endpoint have no shared configuration contract.

The fix must remove false state at the presentation boundary and add one real end-to-end path. Adding more sample data or another callback would preserve the root cause.

## Domain and ownership model

Use the existing dependency direction. Keep deployment settings out of media identity rules.

```text
Operator inputs / build environment / saved client preference
                           |
                           v
          Connectivity configuration composition
          - deny > managed > saved > default
          - bind address != client endpoint
          - localhost, 127.0.0.1 and ::1 are aliases only
            inside the same network namespace
                           |
             +-------------+-------------+
             |                           |
             v                           v
      Runtime listener             Client connection
      FASTI_LISTEN                  FASTI_API_URL / saved URL
      native/container             web/Tauri/Android
             |                           |
             +-------------+-------------+
                           v
             Read-only effective settings view
             value + source + managed/locked state

Provider declaration maximum
             ∩
Operator allow rules
             -
Operator deny rules (deny wins)
             |
             v
Application provider capability decision
             |
             +--> secret adapter: configured/source only to UI
             +--> HTTP adapter: scheme/host/IP/redirect/method/limits
             +--> neutral search candidates with provenance
```

Ownership:

- `fasti-application` owns provider capability decisions and neutral search results.
- Provider adapters own remote request and response translation, not identity or storage.
- `fasti-api` owns HTTP admission and truthful exposure.
- Desktop owns OS keyring access and native HTTP execution.
- `packages/sdk` owns client endpoint validation and transport configuration.
- `packages/ui` renders host-provided state and invokes host-provided commands. It stores no secret and infers no verification state. In Tauri, endpoint tests and provider HTTP use native IPC commands; the webview does not fetch configurable origins directly.
- Container and build files project deployment configuration. They do not redefine it.

## Configuration contract

Keep bind and connection settings separate.

| Setting | Purpose | Default | Allowed source |
|---|---|---|---|
| `FASTI_LISTEN` | Daemon bind socket | `127.0.0.1:8420` | runtime environment |
| `FASTI_PUBLIC_URL` | Operator-advertised node URL | unset | runtime environment |
| `FASTI_API_URL` | Client endpoint | `http://127.0.0.1:8420` | managed build/env, saved client setting |
| `FASTI_WEB_PORT` | Vite development listener | `5173` | development environment or script option |
| `FASTI_PORT` | Container host port | `8420` | Compose/Podman environment |

Rules:

- accept only absolute `http:` or `https:` origins;
- reject credentials, query strings, fragments, and non-root paths;
- preserve a valid `.internal` hostname;
- normalize only syntax, not DNS identity;
- display `localhost`, `127.0.0.1`, and `[::1]` as loopback alternatives when applicable;
- explain that Android and containers use a different network namespace;
- never silently replace a custom hostname with loopback;
- report the value source as `default`, `saved`, `environment`, or `build`;
- managed values are visible and read-only in Settings.

## CA and TLS boundary

Fasti uses normal platform certificate validation.

- Native and desktop clients use the operating system trust store.
- Android uses its network security configuration and a documented, explicit private-CA trust path when required.
- Container deployments terminate TLS at a reverse proxy. The documented local example uses Caddy `tls internal`.
- Settings shows whether the endpoint is HTTP, public HTTPS, or private-CA HTTPS and links to the relevant trust steps.
- A verification action must perform a real health request and show the certificate or connection failure without offering an insecure bypass.
- No CA private key enters Fasti, the browser, logs, screenshots, fixtures, or repository files.

## Central policy composition

Use one small data model for configuration and one deterministic composer. Do not build a generic policy engine.

Policy dimensions:

- provider IDs;
- declared provider capabilities such as `metadata.search`;
- outbound schemes and hosts;
- resolved network classes;
- HTTP methods;
- request timeout, response-byte, pagination, redirect, and retry limits.

Composition:

```text
effective provider access
  = provider declaration maximum
    intersect operator/build allow rules
    minus every matching deny rule
```

A configured allow cannot widen a provider declaration. A deny always wins. The application checks provider and capability. The HTTP adapter checks the actual destination, DNS answers, redirect hops, method, and bounds. The secret adapter checks credential source and redaction. The Settings view may preview the composed result, but it does not enforce policy.

The initial outbound default denies loopback, private, link-local, multicast, unspecified, and cloud metadata-service destinations for provider HTTP. An explicitly supported local provider requires a future threat-modelled declaration; a broad user allow does not bypass this default.

## Provider and Discover slice

Start with the smallest honest vertical slice:

1. one provider declaration that has reviewed terms and stable API behavior;
2. one credential-backed adapter to prove write-only key storage and replacement;
3. one public/no-key adapter only if it reuses the same neutral result contract;
4. explicit Search submission, not search-on-every-keystroke;
5. bounded results with provider name and provenance;
6. persistent loading, empty, missing-key, denied-policy, offline, partial-failure, and retry states;
7. no result may create, merge, or move a Fasti Record.

Tauri is the first trusted execution host. It stores app-entered secrets in the OS keyring and performs provider requests outside the webview. Headless and container keys come from environment variables or mounted secret files and remain read-only in Settings. The ordinary browser build must not persist provider secrets or claim provider search until an authenticated daemon route exists.

The ordinary browser build renders provider secret entry as disabled with a direct explanation. It may show managed, non-secret provider status supplied by its host, but it cannot inspect or persist credential bytes.

## User journey and developer experience

**Primary persona:** a privacy-focused self-hoster or platform operator who runs Fasti locally, in a container, or as an installed client.

**Target time to first healthy connection:** 2–5 minutes.

**Magical moment:** one copied command starts the node; Settings shows the exact effective endpoint and source; Test connection returns a real health response; a configured provider returns a real neutral search result.

Golden path:

```text
1. FASTI_DATA_ROOT=... FASTI_LISTEN=127.0.0.1:8420 cargo run --locked -p fastid
   -> prints the effective listener and public URL without secrets
2. Open Settings > Connection
   -> shows http://127.0.0.1:8420, localhost alternative, source: default
3. Select Test connection
   -> persistent success with server version, or an actionable typed failure
```

Container path:

```text
FASTI_PORT=8420 podman compose up
curl --fail http://127.0.0.1:8420/api/v1/health
```

Private local domain path:

```text
fasti.internal -> reverse proxy -> fastid:8420
client trusts the proxy CA through the operating system
Settings endpoint: https://fasti.internal
```

### DX review scorecard

| Pass | Initial | Required 10/10 condition |
|---|---:|---|
| Getting started | 4 | Three-step native and container quick starts with exact output and no hidden port assumptions |
| API/CLI/SDK | 5 | One origin validator, explicit value source, typed failures, no raw-HTTP escape for supported operations |
| Errors and debugging | 3 | Every connection/provider failure states what happened, why, the rejected value, and the next action |
| Documentation | 4 | One task-led guide for native, container, desktop, `.internal`, CA trust, and Android namespace differences |
| Upgrade and migration | 7 | Existing defaults remain valid; new config is additive; invalid prior saved endpoints fail with recovery |
| Environment and tooling | 5 | Script, Vite, OCI, Tauri, and Android build inputs use documented names and focused checks |
| Community and ecosystem | 6 | Provider contribution recipe uses neutral contracts and does not promise unsupported compatibility |
| Measurement | 2 | QA records time-to-health, command count, failed-step clarity, and environment tested |

All eight passes target 10/10 in this plan. Telemetry is not added to the product. QA receipts provide local DX evidence without phone-home behavior.

## UI and accessibility acceptance

The existing PR #44 Settings screen is not a safe base without repair.

- Make the sidebar participate in desktop layout and use a drawer/collapsed pattern on narrow screens.
- Add `min-width: 0`; stack Settings navigation above content on narrow screens; pass 320 CSS px reflow without two-dimensional scrolling.
- Use one `main` landmark.
- Use `aria-current` for the current Settings page and native radio semantics for exclusive choices.
- Keep every interactive target at least 44 by 44 CSS px.
- Use write-only secret inputs. Never return or render secret bytes or masked placeholders as values.
- Show status near the initiating control and keep failure text until the next attempt.
- Remove fake token creation, notification success, unsupported connector URLs, and no-op import controls from the active interface.
- Use one theme editor. Restore canonical status colors and only offer contrast-safe foreground/background pairs.
- Use literal labels: Connection, Provider access, Test connection, Saved, Managed, Not configured, Denied by policy.

## Failure modes

| Failure | Required behavior |
|---|---|
| Invalid URL or port | Reject before save; keep prior valid value; identify the invalid field |
| `.internal` does not resolve | Keep the saved hostname; report DNS failure; show hosts/DNS next step |
| Private CA is untrusted | Fail certificate validation; show platform trust guidance; no bypass |
| `localhost` from Android/container targets itself | Explain network namespace and request the reachable host or gateway name |
| Managed environment conflicts with saved value | Managed value wins and appears locked with its source |
| Provider key missing or replaced | Next request uses the current key; UI never sees prior bytes |
| Provider allow widens declaration | Reject the widening rule |
| Provider or host denied | Deny before secret lookup or request; show which policy source denied it |
| DNS rebinding or redirect changes destination | Re-evaluate the resolved address and every hop; stop on a denied destination |
| Provider timeout, 429, oversized or malformed response | Bound work; return typed partial failure; preserve local Chronicle state |
| UI closes or navigates during request | Abort or ignore stale completion; preserve the last committed state |
| Remote daemon bind | Keep capability routes closed until the access threat model is implemented |

## Test coverage plan

```text
CODE PATHS                                      USER FLOWS
[+] endpoint origin parser                     [+] Settings > Connection
  |- valid http/https + .internal [UNIT]          |- default loopback visible [COMPONENT]
  |- loopback alias classification [UNIT]         |- save custom origin [COMPONENT]
  |- credentials/path/query/fragment [UNIT]       |- managed value is locked [COMPONENT]
  `- invalid/overflowing port [UNIT]               `- real health test success/failure [E2E]

[+] policy composer                             [+] Settings > Provider access
  |- declaration intersect allow [UNIT]           |- write-only key save/remove [E2E]
  |- deny wins at every dimension [UNIT]           |- configured/source only [COMPONENT]
  |- safe default network classes [UNIT]           |- denied rule explanation [COMPONENT]
  `- deterministic effective view [UNIT]           `- no fake verified state [REGRESSION]

[+] provider request adapter                    [+] Discover
  |- secret lookup/redaction [INTEGRATION]         |- explicit submit [COMPONENT]
  |- DNS and redirect recheck [INTEGRATION]         |- live neutral results [E2E]
  |- method/timeout/bytes/pages/retries [UNIT]      |- empty/offline/429/partial [E2E]
  `- response to neutral candidates [UNIT]          `- typing alone never says online [REGRESSION]

[+] delivery configuration                      [+] Packaging
  |- fastid listener/public URL [RUST]              |- native custom port health [SMOKE]
  |- Vite/API endpoint/port [NODE]                  |- Docker + Podman host port [SMOKE]
  |- Tauri build/runtime config [RUST]              |- desktop custom endpoint [E2E]
  `- Android config merge/entrypoint [BUILD]        `- APK build and endpoint input [BUILD]
```

Coverage target is every new branch plus the two false-state regressions. External provider success tests use a bounded fake server in CI; one optional live smoke may run only with an operator-supplied key and must not gate deterministic CI.

## Contract disposition

- OpenAPI: update only if an HTTP route is activated. Do not document a Tauri IPC command as HTTP.
- AsyncAPI: `N/A`; this slice adds no event transport.
- JSON Schema: add a versioned operator configuration schema if a durable JSON config file is introduced.
- JSON-LD: `N/A`; connection and secret administration are not linked-data domain state.
- SDK: expose the existing origin normalizer and add typed connection-test configuration if the browser/SDK consumes it.
- CLI: print effective non-secret listener/public URL state and support validation without changing data.
- Knowledge/docs: add native, container, desktop, Android, `.internal`, CA, and provider contribution guidance.

## Performance and memory

- Parse static provider declarations once per process.
- Keep result and response byte limits explicit; do not materialize unbounded pages.
- Use one bounded request per selected provider and a small fixed concurrency limit only after two providers exist.
- Do not perform provider I/O inside a SQLite transaction.
- Abort stale searches when practical; never spawn one unbounded task per result.
- Measure desktop/web bundle deltas, daemon idle RSS, request latency, and Android artifact size. Existing 64/96/160/192 MiB targets remain unchanged.

## What already exists

- `FASTI_LISTEN` parsing and the loopback/default listener.
- `ListenerConfiguration` for staged local application behavior.
- SDK `normalizeBaseUrl` validation, currently private.
- Desktop OS keyring setup-secret pattern.
- Provider-neutral metadata claims and deterministic resolution.
- A generated contract spine and typed problem conventions.
- Docker/Podman smoke scripts and Tauri project structure.
- Fasti design tokens, focus styles, skip link, and reduced-motion handling.

Reuse these owners. Do not duplicate origin validation, keyring handling, provider identity, or typed error meaning in Svelte.

## Not in scope

- app-owned TLS termination or CA issuance;
- certificate-validation bypasses;
- public unauthenticated provider/search routes;
- hosted or multi-tenant access;
- automatic DNS or hosts-file mutation;
- mDNS, MQTT, Home Assistant, WebTransport, or plugin execution;
- bulk provider catalogue or compatibility claims;
- automatic identity merges or Chronicle mutations from search;
- Cloudflare Worker conversion or a fake one-click button;
- PikaPods publication before remote access and release gates;
- PR #52 release workflow remediation in the feature slice.

Cloudflare and PikaPods remain explicit compatibility constraints. PikaPods needs one HTTPS port and correct proxy headers. A Cloudflare Deploy button would need a separate Worker or Tunnel adapter because the native daemon is not a Worker project.

## Parallelization

| Step | Modules | Depends on |
|---|---|---|
| A | governance/workflows/scripts | none |
| B | application/contracts/provider adapters | A baseline |
| C | SDK/UI/web | B contracts |
| D | desktop/mobile | B contracts |
| E | OCI/dev scripts/docs | B configuration names |
| F | QA/design/performance/contracts | C, D, E |

Lane A runs first. After its clean baseline, lanes C, D, and E may run in parallel around the application contract from lane B. UI and desktop both consume host command types, so their shared interface lands before parallel work. Final QA runs after all lanes merge.

## Implementation Tasks

- [ ] **T1 (P1, human: ~4h / CC: ~30min)** — governance — repair PR #51 and record exact-head gate evidence.
  - Surfaced by: PR audit — current canonical gate failure, active review findings, and unsigned commits.
  - Files: governance workflows, benchmark fixture, `scripts/`, governing docs.
  - Verify: `PKG_CONFIG=/usr/bin/pkg-config cargo xtask test pr` and exact-head GitHub checks.
- [ ] **T2 (P1, human: ~6h / CC: ~45min)** — configuration — implement validated bind, public URL, client endpoint, source, alias, and port composition.
  - Surfaced by: Architecture review — hardcoded, conflated delivery settings.
  - Files: application/contracts, daemon, SDK, web configuration.
  - Verify: focused Rust and TypeScript parser/precedence tests.
- [ ] **T3 (P1, human: ~8h / CC: ~60min)** — provider policy — implement declaration-limited, deny-wins composition and bounded destination checks.
  - Surfaced by: Architecture and security review — provider/network/capability controls have no owner.
  - Files: application/provider adapters, contracts, tests.
  - Verify: policy truth table, network class, redirect, timeout, and byte-limit tests.
- [ ] **T4 (P1, human: ~8h / CC: ~60min)** — provider search — implement one real credential-backed provider and neutral Discover results.
  - Surfaced by: Root cause — sample data is presented as online search.
  - Files: application/provider adapter, desktop host commands, UI Discover.
  - Verify: fake-server integration, key replacement, missing-key, 429, malformed, and partial-result tests.
- [ ] **T5 (P1, human: ~8h / CC: ~60min)** — Settings — replace fake controls with real endpoint, trust, policy, and write-only credential flows.
  - Surfaced by: Code quality and UX review — local-only callbacks and false success states.
  - Files: UI Settings/workbench, host adapters, component tests.
  - Verify: UI typecheck, accessibility tests, no-secret DOM assertions, false-state regressions.
- [ ] **T6 (P1, human: ~6h / CC: ~45min)** — responsive shell — fix sidebar, landmarks, selection semantics, focus, target size, and contrast.
  - Surfaced by: UX review — desktop overlay and 390 px reflow failure.
  - Files: UI shell, Settings, design tokens.
  - Verify: keyboard and screen-reader audit; screenshots at 320, 390, 768, and 1440 CSS px.
- [ ] **T7 (P1, human: ~8h / CC: ~60min)** — delivery — make ports/endpoints configurable in native, Docker/Podman, Tauri, and Android build inputs.
  - Surfaced by: Distribution review — hardcoded ports and absent mobile entrypoint/config proof.
  - Files: container, scripts, web, desktop/mobile configuration.
  - Verify: native/container smoke, Tauri desktop build, real APK build or an explicit failing gate.
- [ ] **T8 (P2, human: ~6h / CC: ~45min)** — documentation and contracts — publish truthful task-led setup, CA, policy, provider, and deployment guidance.
  - Surfaced by: Documentation review — no discoverable custom-domain/private-CA path.
  - Files: README, AGENTS, SECURITY, roadmap, contract/knowledge surfaces.
  - Verify: doc links, contract verification, repository truth, STE100 copy review.
- [ ] **T9 (P1, human: ~8h / CC: ~60min)** — final evidence — run canonical QA, Impeccable, design, DX, performance, and outside-voice reviews.
  - Surfaced by: Definition of Done and PR audit — stale or absent exact-head evidence.
  - Files: evidence artifacts and review reports.
  - Verify: `cargo xtask test pr`, focused QA report, screenshots, review dashboard, GitHub checks.

## Completion gates

- PR #51 is green or the feature remains explicitly stacked and non-mergeable.
- PR #44 is replaced or reduced; its fake controls are not carried forward.
- Focused tests and the canonical PR gate pass from a clean tree.
- Contract generation is deterministic and all non-applicable surfaces have reasons.
- Native, Docker, Podman, Tauri desktop, and Android port/endpoint paths have real artifacts or explicit failed gates.
- Settings passes keyboard, screen reader, reflow, focus, contrast, reduced motion, and state-continuity checks.
- No secret appears in logs, URLs, DOM return state, fixtures, screenshots, or generated docs.
- CodeRabbit and Codacy findings on the submitted exact head are fixed or explicitly dispositioned; no merge occurs without user authorization.

## Review summary

- Step 0: scope accepted as the complete option; work split into bounded slices.
- Architecture: 6 issues found and folded into ownership, sequencing, TLS, trust, policy, and remote-access gates.
- Code quality: 5 issues found and folded into reuse, truthful UI, write-only secrets, and removal of no-op controls.
- Tests: 29 planned branches and user flows; deterministic fake-provider E2E coverage required.
- Performance: 4 issues folded into bounded parsing, I/O, concurrency, and measurement.
- DX: 31/80 initial; target 80/80 through one golden path, typed recovery, task-led docs, and local evidence.
- Failure modes: 11 critical cases specified.
- Outside voice: Claude and Antigravity ran. Both required Tauri-native IPC for configurable network calls, a disabled browser secret surface, explicit PR #51 sequencing, and reuse of the SDK origin validator. Their recommendations to defer operator allow/deny and Android were not accepted because both are explicit user requirements; the implementation keeps the policy model small and treats Android build proof as a delivery gate.
- Parallelization: 6 lanes; 3 may run in parallel after the shared contract lands.
- Unresolved decisions: 0.

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope and strategy | 0 | not run | User supplied the scope and selected the complete option |
| Codex Review | `/codex review` | Independent second opinion | 0 | not run | Not required at plan stage |
| Eng Review | `/plan-eng-review` | Architecture and tests | 1 | clear | 26 issues and gaps folded; 0 unresolved |
| Design Review | `/plan-design-review` | UI and UX gaps | 1 | issues folded | 15 verified PR #44 findings define acceptance criteria |
| DX Review | `/plan-devex-review` | Developer experience gaps | 1 | clear | 31/80 initial to 80/80 planned; TTHW target 2-5 minutes |

**CROSS-MODEL:** Parallel repository, provider, UX, Claude, and Antigravity reviews agreed that PR #44 is sample-only, configurable desktop network calls belong behind Tauri IPC, browser clients cannot store provider secrets, and PR #51 is the first landing dependency.

**VERDICT:** ENG + DX PLAN CLEARED — implement as stacked slices after the PR #51 prerequisite.

NO UNRESOLVED DECISIONS
