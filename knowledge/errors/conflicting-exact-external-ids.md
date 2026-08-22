# Error: `conflicting_exact_external_ids`

**Status:** Governed design draft; not implemented

**Target body:** B1 problem catalogue and B2 identity behavior

**Provisional semantic:** conflicting exact external identifiers must not be merged automatically.

---

## Intended condition

This future problem applies when an observation or import row supplies two or more exact external identifiers that resolve to incompatible candidate Records. B1 must assign the canonical typed problem code and transport mapping before any handler or SDK can expose it.

## Required safe state

- Do not guess or combine candidate Records.
- Do not move occurrences, progress, ratings, notes, tags, or list membership.
- Preserve the supplied evidence and create resumable review state once B2 implements that capability.

No current B0 route, HTTP status, review screen, or manual-merge operation is associated with this draft.
