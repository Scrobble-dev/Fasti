# Fasti Master Integrator Handoff

## Purpose

This document is the entry point for a new integrator joining Fasti without previous conversation context.

Read this before changing code.

The goal is to preserve the decisions already made, avoid repeating previous mistakes, and continue from evidence rather than assumptions.

---

# Product Boundary

## Fasti records. Players play.

Fasti is a local-first Chronicle and identity system.

Fasti does not:

- decode media;
- stream media;
- select playback sources;
- become a Kodi, Plex, Stremio, or Nuvio player;
- make a provider ID the permanent identity of a work.

External systems submit observations.

---

# Source of Truth Order

When documents disagree, use this order:

1. Current PR and repository state.
2. Approved design documents.
3. Approved engineering execution plans.
4. Approved test plans and QA evidence.
5. Historical research.

Historical research is context, not implementation authority.

---

# Current Programme State

| Body | Status |
| --- | --- |
| B0 truth reset | Complete |
| B1 contract foundation | Software complete; physical evidence remains open |
| B2 local kernel | Implemented behind review boundaries; evidence completion required |
| B3 correction and portability | Next major implementation gate |
| B4 product experience | Future |
| B5 metadata projections | Future |
| B6 neutral conformance | Future |
| B7 Nuvio readiness | Future |
| B8 release hardening | Future |

Do not claim a milestone complete without evidence.

---

# Core Rules

## Identity

Fasti owns its own stable IDs.

External identifiers are evidence:

```
Fasti Entity
├── IMDb
├── AniList
├── MAL
├── TMDB
├── TVDB
└── other namespaces
```

Never:

- use provider IDs as primary identity;
- silently merge because IDs look similar;
- rewrite history because metadata changed.

---

## Chronicle

Keep separate:

- observation;
- occurrence;
- interpretation;
- correction.

Original evidence remains preserved.

A better interpretation does not rewrite what happened.

---

## Integrations

Integrations adapt to Fasti.

Do not import another project's internal model into Fasti.

Correct:

```
Provider
   |
   v
Adapter
   |
   v
Fasti capability
```

Incorrect:

```
Provider database
   |
   v
Fasti core
```

---

# Architecture Boundaries

Expected dependency direction:

```
Domain
  -> Application
      -> Contracts
          -> Adapters
              -> SQLite/API/CLI/UI
```

Domain code should not depend on:

- Axum;
- SQLite;
- CLI frameworks;
- UI frameworks;
- provider APIs.

---

# Current PR Context

Primary PR:

Scrobble-dev/Fasti#14

Current focus:

- B0-B2 hardening;
- contracts;
- security boundaries;
- local kernel.

Remaining B2 evidence:

- crash recovery;
- physical Raspberry Pi 5 evidence;
- J4125 evidence;
- full durability proof.

---

# Next Engineering Order

## B2 Completion

Finish:

- receipt durability;
- idempotency proof;
- restart recovery;
- SQLite durability evidence;
- authorization regression tests.

Then:

## B3

Implement:

- correction chains;
- export;
- restore;
- equality verification;
- offline restore.

Then:

## B4-B8

```
B4 Product experience
      |
B5 Metadata projections
      |
B6 Neutral conformance
      |
B7 Nuvio readiness
      |
B8 Release hardening
```

---

# Nuvio Roadmap

Nuvio is not the first foundation layer.

Sequence:

```
B6
Neutral client conformance

B7a
Pairing + observation submission

B7b
Progress/watchlist/watched reconciliation

B7c
Catalogues, collections, metadata projections
```

Do not start with:

- direct Nuvio database access;
- provider-specific identity shortcuts;
- playback ownership.

---

# Required Quality Gates

Every implementation must consider:

- QA evidence;
- security review;
- OpenAPI/AsyncAPI/JSON-LD impact;
- documentation updates;
- offline operation;
- packaged distribution;
- performance impact;
- accessibility;
- cognitive load.

Required practices:

- run `/qa`;
- update docs;
- add regression tests;
- explain commits clearly;
- preserve issue and upstream relationships.

---

# Do Not Repeat Previous Mistakes

## Do not build a player

Fasti records. Players play.

## Do not create provider lock-in

Providers are evidence sources, not truth.

## Do not add APIs before semantics exist

Contracts define APIs.

## Do not treat absence as deletion

Missing data, timeout, or provider failure is not removal.

## Do not create duplicate abstractions

Reuse existing capability ownership.

---

# First 48 Hours For A New Integrator

1. Read this document.
2. Read the B0-B3 engineering plan.
3. Read the B0-B3 test plan.
4. Review PR #14.
5. Run existing checks.
6. Identify the owning capability before changing code.

Do not begin with:

- Nuvio integration;
- player support;
- MQTT;
- plugin systems;
- metadata federation.

First prove the Chronicle foundation.

---

# Required Handoff Artifacts

Maintain:

- implementation status;
- QA reports;
- security findings;
- evidence manifests;
- rollback notes;
- architecture decisions;
- contract changes;
- screenshots for UI changes.

A future integrator should be able to continue from the repository alone.

---

# Final Goal

A user can:

- keep their media history locally;
- understand it;
- repair it;
- move it;
- recover it;

without surrendering ownership of their record to one provider.
