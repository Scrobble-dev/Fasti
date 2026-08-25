# Fasti External Harness Context Save: 2026-08-25

> Status correction, verified 2026-08-25: this dated context save originally overstated B6 and B7. Current source contains application-level payload converters and in-memory conformance models. Production `fastid` has no Plex or Jellyfin webhook routes, MPRIS D-Bus observer, mDNS discovery, Nuvio pairing endpoint, Nuvio transport, or callers for these converters. Use the master handoff, capability ledger, current source, and live checks for current status.

**Date:** 2026-08-25T08:10:00Z  
**Branch:** `dev`  
**Milestone Disposition:**
- B0 (Ledger & Truth): Complete
- B1 (Hardware Envelopes & Verifiable Receipts): Complete (PR #36 merged into `dev` at `6e18c18a`)
- B2 (Namespace Governance): Branch `codex/b2-namespace-definition` verified against `dev`
- B3 (Portability Archive v1 & Restore): Complete (PR #35 merged into `dev` at `fd6bdd60`)
- B4 (UI / Fasti Workbench): In progress by ChatGPT
- B5 (Metadata Resolution & Claims): Single-pass $O(1)$ space optimization implemented in PR #39
- B6 (Multi-Client Conformance & Ingest): source-neutral archetype fixtures and Plex, Jellyfin/Emby, and Linux MPRIS payload converters exist; production transports are absent
- B7 (Nuvio Deep Integration): application models cover observation, replay, state-delta, and catalog behavior; production pairing, persistence, transport, and Nuvio client work are absent
- B8 (Packaging & Platform Readiness): Clean compilation and zero-network reproducible contract verification

---

## Key Files & Capabilities

- `crates/fasti-application/src/nuvio.rs`: `NuvioPlaybackSession`, `NuvioOutbox`, `NuvioWatchedState`, `NuvioChangeDelta`, `NuvioStateSyncEngine`, `NuvioCatalogDescriptor`, `NuvioCatalogItem`, `NuvioCatalogProjectionStore`.
- `crates/fasti-application/src/ingest.rs`: `PlexWebhookPayload`, `JellyfinWebhookPayload`, `MprisMediaEvent`.
- `crates/fasti-application/tests/b7_nuvio_observations.rs`: 14 feature-gated conformance tests.
- `crates/fasti-application/tests/b6_ingest_webhooks.rs`: 16 feature-gated converter and conformance tests.

Run these suites explicitly:

```bash
cargo test --locked -p fasti-application --features conformance-fixture --test b6_ingest_webhooks
cargo test --locked -p fasti-application --features conformance-fixture --test b7_nuvio_observations
```
- `docs/architecture/nuvio-integration.md`: Architectural specification and invariant documentation.
