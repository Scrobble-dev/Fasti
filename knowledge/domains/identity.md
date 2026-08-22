# Knowledge: Identity Domain

**Status:** Governed design draft; not implemented

**Domain:** `identity`

**Target body:** B1 contract spine and B2 local kernel

**Purpose:** Preserve the accepted identity rules without implying that storage, errors, or routes exist in B0.

---

## Required future invariants

1. **Stable Local Fasti ID:** A Fasti Record must receive an opaque local `record_id`. External provider identifiers remain claims attached to that Record.
2. **Grain Separation:** Identifier evidence must be evaluated within its declared grain, such as series, release, edition, season, episode, book, or chapter.
3. **Exact-ID Consistency:** Conflicting exact external identifiers must not trigger an automatic merge. B1 owns the shared typed problem name; B2 owns the transactional behavior.
4. **Directional Assertions:** Cross-provider mappings must remain directional, versioned, range-aware, and provenance-bearing. B1 owns the public types before storage is implemented.

The [capability ledger](../../docs/capability-ledger.md) is authoritative for current implementation status.
