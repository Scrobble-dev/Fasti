# B7: Nuvio Integration Design

## Current status

This document describes the target design and the application-level conformance model. Production pairing, Nuvio client code, durable client storage, network transport, daemon ingest routes, and shared catalog publication do not exist. The checked-in `NuvioOutbox` is process-local and non-durable.

## 1. Constitutional Boundary

**Fasti records. Players play.**

Fasti does not decode, transcode, manage streams, or function as a media player. A future Nuvio integration must use public, authenticated application capabilities.

```text
┌─────────────────────────────────────────────────────────┐
│                      Nuvio Player                       │
│  ┌─────────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │ Playback Engine │  │ Local Session│  │   Outbox   │  │
│  └────────┬────────┘  └──────┬───────┘  └─────┬──────┘  │
└───────────┼──────────────────┼────────────────┼─────────┘
            │                  │                │ (Process-local, non-durable)
            │ (Playback        ▼                │
            │  Unblocked) ┌─────────────────────┴─────────┐
            └────────────►│ Canonical Observation Accept  │
                          └─────────────┬─────────────────┘
                                        ▼
                          ┌───────────────────────────────┐
                          │         Fasti Daemon          │
                          │   (Chronicle & Verification)  │
                          └───────────────────────────────┘
```

---

## 2. Invariants

1. **Playback Independence:**
   Playback within Nuvio must never depend on Fasti availability or network connectivity. If the Fasti daemon is unreachable, crashing, or returning errors, Nuvio continues playback seamlessly without UI interruption or lag, buffering observations into its client-side outbox.

2. **No Direct Storage or Schema Coupling:**
   A production adapter must not access Fasti SQLite files directly or share internal SQLite handles. It must use canonical application ports and governed transport contracts.

3. **Deterministic Operation Derivation & Replay Safety:**
   Every observation maps to a deterministic operation ID:
   - Progress Heartbeats: `nuvio:session:<session_id>:beat:<sequence_number>`
   - Session Completion: `nuvio:session:<session_id>:complete`
   
   Network retries, out-of-order queue draining, and reconnection bursts replay existing receipts with zero new operations and zero false rewatches.

4. **External Identifiers as Evidence:**
   Nuvio media identifiers (e.g. TMDB, IMDb, Kitsu, SIMKL) are treated as directional evidence claims attached to the observation, never as canonical Fasti record identities.

---

## 3. Programme Lanes

The target Nuvio integration is partitioned into three strictly sequenced lanes. Current code models parts of each lane for conformance only:

- **B7a — Observation ingress and pairing:** application models cover progress, completion, bounded FIFO retry, and typed problems. Pairing, persistence, and transport are absent.
- **B7b — Two-way watched-state synchronization:** `NuvioStateSyncEngine` models ordered deltas and self-origin suppression. Change-feed transport and reconciliation UI are absent.
- **B7c — Shared catalogs and media metadata:** `NuvioCatalogProjectionStore` models local projection and filtering. Publication and provider resolution are absent.

---

## 4. B7a Conformance Flow

### Heartbeat Progression
The conformance model can build a periodic observation command:
```rust
let mut session = NuvioPlaybackSession::new("session-42", Grain::Film, "Title", clues, 7200);
let cmd = session.tick_heartbeat(access, 600, observed_at);
outbox.dispatch_or_buffer(&port, cmd);
```

### Outbox Buffering & Drain
The process-local `NuvioOutbox` queues generated commands in FIFO order. `outbox.drain(&port)` exercises a caller-supplied application port. It does not contact a daemon or survive a process restart.
