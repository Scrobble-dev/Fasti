# Fasti TypeScript SDK

`@fasti/sdk` is the generated-contract TypeScript client for Fasti's HTTP and receipt SSE surfaces. It is private to this source tree in B1: it is not published, versioned for external installation, or evidence of a supported Fasti release.

The generated public types, operation bindings, problem parsers, and capability identifiers derive from the governed contract registry. Do not edit `src/generated.ts` or redefine those meanings in this package.

## Reach first value locally

Install the locked workspace dependencies and build the SDK:

```bash
pnpm install --frozen-lockfile
pnpm --filter @fasti/sdk build
```

Start the production daemon in health-only mode from the repository root:

```bash
cargo run --locked -p fastid
```

In another terminal, call the health route:

```bash
node --input-type=module <<'EOF'
import { FastiClient } from "./packages/sdk/dist/transport.js";

const client = new FastiClient({ baseUrl: "http://127.0.0.1:8420" });
console.log(await client.health());
EOF
```

Expected output:

```text
{ status: 'healthy', version: '0.1.0' }
```

Stop the daemon with `Ctrl-C`. This health call does not prove any B1 fixture route or B2 runtime behavior.

## Durable local setup

Set `FASTI_DATA_ROOT` and keep the listener on loopback to mount durable node initialization and first-client enrollment. The generated client exposes these operations as `initializeDurableNode` and `enrollDurableFirstClient`. Both mutations run once and never retry. Fasti provides no development browser account or local password path.

The initialization proof and bearer credential exist only in JSON bodies. A trusted local host shell must store them in permission-restricted credential storage. Do not print them or put them in URLs, command arguments, logs, `localStorage`, or `sessionStorage`.

Integration clients use separately revocable scoped bearer credentials. Do not
copy a bearer secret into browser storage. The generated production parsers
cover the active observation, identity-record, profile-state, and C1 Access
DTOs. C1 adds `startTrailBaseSignIn`, continuation read/complete/cancel,
`readAccessProjection`, current-session read/end, session inventory and scoped
revocation, rotation, and profile selection. The browser handles the
`HttpOnly`, `Secure`, and `SameSite` cookies. The TrailBase callback is browser
navigation and has no SDK method.

These C1 methods exist only on the exact requested-and-bound
`127.0.0.1:8420` durable listener. Exchange and new session issuance require a
verified installation receipt and persisted active TrailBase activation.
Fallback, alternate-loopback, generic, integration, wildcard or container
forwarding, and remote routers omit C1. Review, exact-head, merge, and
merged-tree evidence remain pending. Packaged Tauri authentication, platform
WebView behavior, and packaged assistive-technology evidence are deferred to
`C1-TAURI-AUTH` and are not claimed.

## Governed metadata projections

The M2 client surface uses only generated production contracts:

- `readMetadataProjection(recordId, { offline })` reads the active profile's selected fields, rating claims, provenance, attribution, and cache state. `offline` defaults to `false`; a caller must opt in explicitly.
- `configureMetadataProjection(request)` updates the server-owned profile policy and reports how many affected cache entries Fasti invalidated. The SDK does not persist a browser copy.
- `refreshMetadataClaims(request)` appends governed provider claims and returns their projection, attribution, and cache evidence. The transport verifies that the response record and provider match the request.

These authenticated operations require the generated metadata scopes. Provider credentials, raw provider responses, and cache secrets are not part of their DTOs.

## Exercise the B1 contract

The focused client test builds and starts the loopback-only Rust fixture on an ephemeral port, executes the generated SDK against it, and stops it:

```bash
pnpm --filter @fasti/sdk build
node --test tests/js/sdk-client.test.mjs
```

Fixture successes always declare `fixture_only` availability and `none` durability. Production runtime operations remain distinct from that fixture. Run the full governed gate before treating a contract change as complete:

```bash
cargo xtask contract verify --locked
```

The receipt from that command is software evidence only. The B1 milestone separately requires the exact-head aggregate evidence package, including Tauri and same-workflow-attempt x86_64/aarch64 low-hardware envelope receipts.

## Error and retry behavior

- RFC 9457 responses become `FastiProblemError` with the governed problem body.
- Network, timeout, cancellation, transport, protocol, and contract-parse failures have distinct error classes.
- Safe reads may retry under the bounded policy. Observation submission retries only with the stable body-owned `operation_id`.
- The receipt stream resumes from the last successfully handled cursor and enforces bounded line, event, and cursor sizes.
- Credentials are resolved per request and are never sent to the unauthenticated health operation.

See [`contracts/README.md`](../../contracts/README.md) for ownership and [`docs/capability-ledger.md`](../../docs/capability-ledger.md) for runtime truth.
