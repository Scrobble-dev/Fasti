# ADR 0001: Append-Only Activity Ledger & Deterministic Projections

* **Status:** Accepted
* **Deciders:** Fasti Core Team
* **Date:** 2026-08-21

---

## Context
Traditional media trackers use mutable database tables (e.g. `watch_history` with `watched: boolean` and `last_updated_at`). This causes permanent loss of prior plays, collapses replay dates, destroys observer provenance, and makes offline conflict resolution unpredictable.

## Decision
1. We implement an **append-only immutable event ledger** in SQLite (`activity_ledger`).
2. Current progress, library status, and continue queues are implemented as **deterministic materialised projections**.
3. Corrections and deletions are modeled as new events referencing older event IDs (`correction_of`, `tombstone_of`).

## Consequences
* **Positive:** Complete provenance preservation, replayability, lossless export/restore, and trivial replica synchronization.
* **Tradeoff:** Storage footprint grows monotonically with activity events. Mitigated by compact SQLite indexing and optional compacted projection snapshots.
