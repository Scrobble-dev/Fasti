# Knowledge: Identity Domain

**Domain:** `identity`  
**Purpose:** Stable record identity, grain hierarchy, external identifier management, and typed directional assertions.

---

## Core Invariants

1. **Stable Local Fasti ID:** A Fasti record is identified by an opaque UUID (`record_id`, e.g. `rec_01K...`). External provider identifiers are claims attached to this record.
2. **Grain Separation:** Identifiers are evaluated strictly within their declared grain (e.g. `series` vs `release` vs `edition` vs `episode`).
3. **Exact-ID Consistency:** All supplied exact IDs must resolve to one compatible identity group. If supplied exact IDs conflict across namespaces (e.g. TMDB vs TVDB point to different records), Fasti rejects the merge with `conflicting_exact_external_ids`.
4. **Directional Assertions:** Mappings between external namespaces are stored as versioned `IdentityAssertion` records with explicit coverage ranges, ordering spaces, and evidence provenance.
