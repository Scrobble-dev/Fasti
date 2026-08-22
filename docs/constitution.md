# Fasti Constitution

This document controls implementation when a scaffold, example, issue, or dependency suggests a different product.

## Product boundary

**Fasti records. Players play.**

Fasti receives observations, preserves evidence, reconciles identity, records occurrences, and exposes governed history. It does not decode, transcode, stream, or play media.

A Fasti Record has a stable local identifier. TMDB, TVDB, IMDb, MAL, AniList, Kitsu, AniDB, ISBNs, MusicBrainz, Steam, GOG, podcast GUIDs, and future identifiers are typed claims attached to that Record. Changing a preferred metadata provider must not change the Record ID or move history, progress, ratings, notes, tags, lists, evidence, or review state.

Unresolved, partially resolved, conflicted, known-absent, and blocked states are valid data. Fasti must not guess to make a screen look complete.

## Architecture

Domain-driven design is mandatory. Domain policy lives in named bounded contexts. Application capabilities coordinate domain behavior through ports. Storage, HTTP, CLI, SDK, provider, UI, and packaging code are adapters and must not own domain meaning.

DRY is mandatory at the semantic level: an invariant, capability, public type, error, permission, or lifecycle rule has one owner. Generated surfaces consume that owner. Incidental repetition may remain when removing it would couple bounded contexts or hide intent.

Provider adapters translate evidence into the neutral observation contract. They cannot create a second identity model, write directly to persistence, or make a provider canonical.

## Evidence and change

Original observations and opaque evidence are immutable. Interpretations, corrections, merges, splits, and tombstones are append-only audited operations. A successful receipt means the promised durability boundary was reached; no adapter may return optimistic success.

Every capability must be idempotent where retry is possible and must preserve enough state to resume without hidden client work. No SDK or UI may introduce an ungoverned retry or offline mutation queue.

## Contract spine

OpenAPI 3.1 through Utoipa-bound real handlers, AsyncAPI 3.x, JSON Schema 2020-12, JSON-LD 1.1, OKF, generated SDKs, CLI behavior, examples, permissions, errors, and knowledge links are release gates. A capability is incomplete when any required surface is missing or disagrees.

B0 records the location and future ownership of these surfaces without pretending generation exists. B1 makes the contract spine executable and adds mutation tests that prove drift fails.

## Local operation and distribution

Core workflows must function with the network denied or fail with a local typed problem and safe next action. The same runtime must support native execution, an OCI wrapper, and later packaged distribution without making one delivery medium the domain architecture.

Public binary, package, image, attestation, and GitHub Release publication remain disabled until B8 readiness and an explicit release action.

## Performance and accessibility

Measured targets are 64 MiB idle, 96 MiB normal, 160 MiB heavy, and a 192 MiB absolute process-tree ceiling. Raspberry Pi 5 is the champion profile; a J4125-class x86 machine is the second mandatory profile. Claims require artifact-bound receipts from the named profile.

Every user-facing flow must follow Gestalt grouping, Nielsen usability heuristics, 44 px minimum targets, visible focus, reduced motion, stable list position, persistent critical state, and resumable review. ADHD/AuDHD needs are acceptance criteria, not a later polish pass. The interface should make the user capable and confident through clear actions, immediate truthful feedback, and recoverable mistakes; it must not explain away unclear interaction design.

## Quality gates

Every body of work requires QA evidence. Rendered UI or UX changes also require design-review evidence. A body is not complete because its unit tests pass; its promised native, OCI, contract, network-denied, recovery, performance, accessibility, and documentation evidence must also pass where applicable.

No later body may be used to make an earlier body appear green.
