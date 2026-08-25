# B7: Nuvio Integration Architecture

## 1. Constitutional Boundary

**Fasti records. Players play.**

Fasti does not decode, transcode, manage streams, or function as a media player. Nuvio is an external player and media environment that integrates with Fasti strictly through public, authenticated application capabilities.

```text
┌─────────────────────────────────────────────────────────┐
│                      Nuvio Player                       │
│  ┌─────────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │ Playback Engine │  │ Local Session│  │   Outbox   │  │
│  └────────┬────────┘  └──────┬───────┘  └─────┬──────┘  │
└───────────┼──────────────────┼────────────────┼─────────┘
            │                  │                │ (Durable queue)
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
   Nuvio does not access Fasti SQLite files directly or share internal SQLite handles. All interaction occurs through canonical application ports and HTTP/SSE/gRPC capability contracts.

3. **Deterministic Operation Derivation & Replay Safety:**
   Every observation maps to a deterministic operation ID:
   - Progress Heartbeats: `nuvio:session:<session_id>:beat:<sequence_number>`
   - Session Completion: `nuvio:session:<session_id>:complete`
   
   Network retries, out-of-order queue draining, and reconnection bursts replay existing receipts with zero new operations and zero false rewatches.

4. **External Identifiers as Evidence:**
   Nuvio media identifiers (e.g. TMDB, IMDb, Kitsu, SIMKL) are treated as directional evidence claims attached to the observation, never as canonical Fasti record identities.

---

## 3. Programme Lanes

The Nuvio integration is partitioned into three strictly sequenced lanes:

- **B7a — Observation Ingress & Pairing (Current):**
  - Device pairing and client enrollment with workspace/profile attribution.
  - Periodic progress heartbeats and completion events.
  - Durable client-side outbox with exponential backoff and replay deduplication.
  - Transparent error visibility with RFC 9457 structured problems.
- **B7b — Two-Way Watched State Synchronization (Later):**
  - Watched-state snapshots and ordered change feeds.
  - Cursor-based delta synchronization and loop prevention.
  - Reconciliation workbench diagnostics.
- **B7c — Shared Catalogs & Media Metadata (Later):**
  - Collection projections and Stremio/Nuvio catalog publication.
  - Normalized metadata claims and provider resolution.

---

## 4. B7a Operational Flow

### Heartbeat Progression
During playback, Nuvio dispatches an observation command every 5–10 minutes:
```rust
let mut session = NuvioPlaybackSession::new("session-42", Grain::Film, "Title", clues, 7200);
let cmd = session.tick_heartbeat(access, 600, observed_at);
outbox.dispatch_or_buffer(&port, cmd);
```

### Outbox Buffering & Drain
When offline, all generated commands are queued in FIFO order in `NuvioOutbox`. Upon link re-establishment, `outbox.drain(&port)` drains all entries against the active daemon. Fasti commits fresh entries and deduplicates replayed entries idempotently.
