# AGENTS.md

## Start here

Before planning or implementation, read:

1. [`docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md`](docs/handoffs/FASTI_MASTER_INTEGRATOR_HANDOFF.md)
2. [`README.md`](README.md)
3. [`docs/constitution.md`](docs/constitution.md)
4. [`docs/definition-of-done.md`](docs/definition-of-done.md)
5. [`ROADMAP.md`](ROADMAP.md)
6. [`docs/capability-ledger.md`](docs/capability-ledger.md)
7. [`contracts/README.md`](contracts/README.md)
8. [`SECURITY.md`](SECURITY.md)

The master handoff is the onboarding map. It does not replace the detailed engineering, test, security, and design documents.

## Product boundary

Fasti records. Players play.

Do not add playback, transcoding, decoding, stream selection, or player claims.

Provider identifiers are evidence, not canonical identity.

## Architecture rules

- Domain rules own meaning.
- Application services own capabilities and authorization.
- Contracts project the same meaning into APIs, events, schemas, SDKs, and docs.
- Adapters must not redefine business rules.
- Reuse existing ownership before creating new abstractions.
- Keep provider integrations modular.

## Contract changes

Any capability change must consider:

- OpenAPI 3.1;
- AsyncAPI 3.x;
- JSON Schema 2020-12;
- JSON-LD;
- SDK;
- CLI;
- permissions;
- typed problems;
- examples;
- documentation.

Generated files are outputs, not sources of truth.

## Offline, security, and performance

- Local operation must work without external services.
- Fail closed on missing authorization, stale state, missing evidence, or unsafe input.
- Keep secrets out of logs, URLs, fixtures, and documentation.
- Bound memory, files, requests, archives, and retries.
- Validate recovery and interruption paths.

Performance targets remain:

- 64 MiB idle;
- 96 MiB normal operation;
- 160 MiB heavy operation;
- 192 MiB absolute ceiling.

Physical Pi 5 and J4125 evidence cannot be replaced by hosted runners.

## QA

Run applicable checks:

```bash
cargo xtask test pr
```

Also run focused checks for changed surfaces. Add regression tests for fixed defects.

UI changes require design review. Headless changes should state when visual evidence is not applicable.

## Review and handoff

Preserve issue and PR history. Do not edit another contributor's original body.

Document:

- user impact;
- contract impact;
- offline impact;
- security impact;
- performance evidence;
- accessibility impact;
- tests;
- rollback.
