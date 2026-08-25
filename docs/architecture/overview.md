# Fasti Architecture Overview

Fasti is an identity-first local system of record for media activity. It is not a media player.

## Current B0-B3 review spine

```text
fasti-domain
    ^
    └── fasti-application ──> use cases, ports, authorization, typed problems
              ^
              ├── fasti-contracts ──> shared public DTOs
              ├── fasti-api ──> production health router
              │                 └── feature-gated loopback conformance router
              ├── fasti CLI ──> capability list/show; guarded B3 commands
              ├── fasti-store ──> staged B2 kernel and B3 portability adapters
              └── generated TypeScript HTTP/SSE SDK
                        └── apps/web local health QA harness
                                  ├── packages/ui presentation
                                  └── packages/tokens design projection

authored capability registry
    ├── production + conformance OpenAPI 3.1
    ├── AsyncAPI 3.x receipt.stream
    ├── JSON Schema 2020-12
    ├── JSON-LD 1.1 + OKF + semantic examples
    └── deterministic verification receipt
```

Dependencies point inward. Domain meaning is owned once and projected outward; HTTP, CLI, SDK, provider, storage, and later presentation types cannot become domain primitives. The retired `fasti-core`, `fasti-activity`, and `fasti-auth` scaffolds are not compatibility layers. Their raw IDs, collapsed activity envelope, caller-controlled server times, and token claims were not proven domain primitives.

`fasti-store` contains the staged B2 local kernel and B3 correction/portability adapters. Production `fastid` still mounts only `GET /api/v1/health`, and its generated OpenAPI document contains only that route. B1’s separate conformance server is compile-time feature-gated, binds only to IPv4 loopback, holds bounded data in memory, and labels every success as fixture-only with no durability. Neither path activates durable observation acceptance, identity resolution, export, restore, or recovery through a supported production surface.

## Target bounded contexts

B1 establishes three ownership layers:

1. `fasti-domain` owns Record identity, observations, evidence, occurrences, interpretations, review state, corrections, access policy, and invariants.
2. `fasti-application` owns versioned capabilities and ports. It coordinates domain work without importing storage, HTTP, CLI, provider, or UI types.
3. `fasti-contracts` owns shared public DTOs. Real Utoipa-bound handlers own OpenAPI operations; authored transport behavior owns the AsyncAPI `receipt.stream` channel; authored vocabularies and contexts own JSON-LD meaning; OKF owns operational recovery knowledge.

The authored capability registry is the authoritative machine ledger. Deterministic generation projects it into the two OpenAPI documents, public registry, JSON Schemas, typed TypeScript HTTP/SSE SDK, and generated capability identifiers. Semantic example validation, standards validators, mutation tests, Rust/TypeScript tests, package checks, and repository-truth checks converge in `cargo xtask contract verify --locked`. A verifier receipt is software evidence only, not a B1 completion or performance receipt.

## Durability sequence

The staged B2 kernel uses this success sequence:

```text
authorize current grant and reserve limits
  -> stream and hash evidence under the governed data root
  -> re-authorize
  -> durably promote evidence
  -> run one bounded SQLite writer transaction
  -> sync required files and directories
  -> publish a replayable durable receipt
```

Any failure before the durability boundary returns a typed problem and cannot report committed success.

## Distribution

The native daemon and CLI are the current executable product shapes. OCI wraps the same binaries and does not add a web build or hidden static fallback. `apps/web` is a local, private, unpackaged QA harness over the generated `system.health` binding. It activates no product capability or presentation milestone. There is no supported installation, release, player, production-mounted persistence kernel, product web application, or desktop package.

B1 cannot close on software checks alone. Its milestone manifest binds contract, QA, Tauri, raw-gate, and retained performance artifacts. The two performance receipts must declare one exact `dev` push and workflow attempt, cover x86_64 and aarch64, complete the 600-second warm-up and 900-second route-less idle window, and bind the kernel-applied 192 MiB, one-vCPU, zero-swap envelope. The verifier recomputes memory, CPU, architecture, and applicable artifact-size results. Optional Pi 5 and J4125 specifications remain useful comparison targets but do not gate the milestone. B4 adds the product browser presentation only after B0-B3 prove the headless kernel; the earlier health harness gathers interface-quality evidence without changing that sequence. B8 owns supported packages, signing, non-Linux restore activation, formal TV support, and public releases.

See [the constitution](../constitution.md), [capability ledger](../capability-ledger.md), and [contract ownership](../../contracts/README.md).
