# Fasti TypeScript SDK

`@fasti/sdk` is the generated-contract TypeScript client for Fasti's HTTP and receipt SSE surfaces. It is private to this source tree in B1: it is not published, versioned for external installation, or evidence of a supported Fasti release.

The generated public types, operation bindings, problem parsers, and capability identifiers derive from the governed contract registry. Do not edit `src/generated.ts` or redefine those meanings in this package.

## Reach first value locally

Install the locked workspace dependencies and build the SDK:

```bash
pnpm install --frozen-lockfile
pnpm --filter @fasti/sdk build
```

Start the health-only production daemon from the repository root:

```bash
cargo run --locked -p fastid
```

In another terminal, call the only production API currently mounted:

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

## Exercise the B1 contract

The focused client test builds and starts the loopback-only Rust fixture on an ephemeral port, executes the generated SDK against it, and stops it:

```bash
pnpm --filter @fasti/sdk build
node --test tests/js/sdk-client.test.mjs
```

Fixture successes always declare `fixture_only` availability and `none` durability. The production `fastid` process does not mount those routes. Run the full governed gate before treating a contract change as complete:

```bash
cargo xtask contract verify --locked
```

The receipt from that command is software evidence only. Physical Raspberry Pi 5 and J4125 measurements remain separate B1 evidence.

## Error and retry behavior

- RFC 9457 responses become `FastiProblemError` with the governed problem body.
- Network, timeout, cancellation, transport, protocol, and contract-parse failures have distinct error classes.
- Safe reads may retry under the bounded policy. Observation submission retries only with the stable body-owned `operation_id`.
- The receipt stream resumes from the last successfully handled cursor and enforces bounded line, event, and cursor sizes.
- Credentials are resolved per request and are never sent to the unauthenticated health operation.

See [`contracts/README.md`](../../contracts/README.md) for ownership and [`docs/capability-ledger.md`](../../docs/capability-ledger.md) for runtime truth.
