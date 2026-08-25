# ADR 0001: Append-Only Activity Ledger & Deterministic Projections

* **Status:** Accepted as a target decision; implementation begins in B2
* **Deciders:** Fasti Core Team
* **Date:** 2026-08-21

---

## Context
Traditional media trackers use mutable database tables (e.g. `watch_history` with `watched: boolean` and `last_updated_at`). This causes permanent loss of prior plays, collapses replay dates, destroys observer provenance, and makes offline conflict resolution unpredictable.

## Decision
1. The local kernel will persist immutable observations, occurrences, evidence references, and append-only interpretations in SQLite.
2. Current progress, library status, and presentation lists will be deterministic derived views whose meaning belongs to the domain rather than a provider or UI.
3. Corrections and deletions are modeled as new events referencing older event IDs (`correction_of`, `tombstone_of`).

## Consequences
* **Positive:** The model can preserve provenance, replay interpretation, and make correction auditable without rewriting original evidence.
* **Tradeoff:** Storage grows with observations and evidence. B2 and B3 must prove bounded indexing, export, restore, and low-resource behavior before those benefits are claimed.
* **Constraint:** This decision does not authorize a generic event-sourcing layer or a provider-keyed projection crate. B1 defines the bounded contexts first.
