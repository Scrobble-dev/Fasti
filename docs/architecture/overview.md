# Fasti Architecture Overview

Fasti is an identity-first local system of record for media activity. It is not a media player.

## Current B0-B4 review spine

```text
fasti-domain
    ^
    └── fasti-application ──> use cases, ports, authorization, typed problems
              ^
              ├── fasti-contracts ──> shared public DTOs
              ├── fasti-api ──> local bootstrap plus authenticated local/remote durable routers
              │                 └── feature-gated loopback conformance router
              ├── fasti CLI ──> capability list/show; guarded B3 commands
              ├── fasti-store ──> staged B2 kernel and B3 portability adapters
              └── generated TypeScript HTTP/SSE SDK
                        └── apps/web pre-production Workbench
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

`fasti-store` contains the B2 local kernel and staged B3 correction/portability adapters. Production `fastid` opens one SQLite kernel only when the operator supplies `FASTI_DATA_ROOT`. Direct loopback or an explicitly declared loopback-only container port forward mounts bootstrap, observation, identity-record, and profile-state routes. A non-loopback bind mounts the authenticated durable subset only after explicit trusted-proxy and HTTPS-public-origin configuration; bootstrap routes stay absent. PR A contains dormant browser-session domain and store foundations only. It mounts no human-authentication or browser-session route; C1 owns activation after TrailBase identity, membership, request-boundary, and cookie gates pass. Missing data-root configuration remains health-only. B1’s separate conformance server is compile-time feature-gated, binds only to IPv4 loopback, holds bounded data in memory, and labels every success as fixture-only with no durability. Identity review, export, restore, and recovery remain outside the supported production surface.

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

The native daemon and CLI are the current executable product shapes. OCI wraps the same binaries and does not add a web build or hidden static fallback. The native daemon mounts bootstrap plus authenticated durable routes when an explicit data root is present. The loopback-only container launcher also requires `FASTI_EXTERNAL_BIND_IP` to declare the outer loopback-only port forward. The non-loopback daemon mounts only the authenticated subset behind explicit trusted HTTPS proxy configuration. `apps/web` is the private, unpackaged pre-production Workbench over generated contracts. `apps/desktop` remains a trusted-host review candidate, not a supported package. There is no supported installation, release, player, or deployed web application.

B1 cannot close on software checks alone. Its milestone manifest binds contract, QA, Tauri, raw-gate, and retained performance artifacts. The two performance receipts must declare one exact `dev` push and workflow attempt, cover x86_64 and aarch64, complete the 600-second warm-up and 900-second route-less idle window, and bind the kernel-applied 192 MiB, one-vCPU, zero-swap envelope. The verifier recomputes memory, CPU, architecture, and applicable artifact-size results. Optional Pi 5 and J4125 specifications remain useful comparison targets but do not gate the milestone. B4 matures the existing Workbench without promoting incomplete capabilities or removing established interaction paths. B8 owns supported packages, signing, non-Linux restore activation, formal TV support, and public releases.

See [the constitution](../constitution.md), [capability ledger](../capability-ledger.md), and [contract ownership](../../contracts/README.md).
