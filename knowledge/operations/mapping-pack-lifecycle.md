# Operation: Mapping Pack Lifecycle

Mapping packs supply directional identity assertions (e.g. anime cour episode offsets, season crosswalks) from curated upstreams.

---

## Lifecycle Steps

1. **Governed Download:** Retrieve pack archive and cryptographic digest.
2. **Signature & Schema Verification:** Validate manifest schema version and publisher signature.
3. **Licence & Lineage Check:** Verify source lineage is documented and permissible.
4. **Disposable Index Compilation:** Build local SQLite lookup indexes without touching canonical user state.
5. **Conflict Preview:** Generate a diff of affected records and detect any conflicts with existing history.
6. **Atomic Activation:** Swap active mapping index pointer atomically.
7. **Retain Rollback:** Preserve the previous active version for immediate one-click rollback if anomalies arise.
