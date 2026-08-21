# Error: `conflicting_exact_external_ids`

**Error Code:** `identity_conflict.conflicting_exact_external_ids`  
**HTTP Status:** `409 Conflict`

---

## Explanation
This error occurs when an observation or import row supplies two or more exact external identifiers (for example, a TMDB TV ID and a TVDB Series ID) that currently resolve to different, separate Fasti records.

## Safe State Guarantee
* **Zero Merging:** Fasti does not guess or automatically combine the two candidate records.
* **Zero History Movement:** No Chronicle events or user progress states are modified.

## Remediation Steps
1. Inspect the two candidate records in **Review Matches** (`/settings/identity/review`).
2. Remove or correct the conflicting external identifier on the source system, or submit an explicit manual merge with preview if the records truly represent the same media item.
