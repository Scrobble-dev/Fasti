# Fasti Architecture Overview

Fasti is architected around one core conviction: **your media history is a durable personal asset, not application state.**

---

## 1. Domain Separation

```
┌────────────────────────────────────────────────────────┐
│                   Scrobble.dev                         │
│   (Open Vocabulary · Activity Profiles · Schemas)      │
└───────────────────────────┬────────────────────────────┘
                            │ Normative semantics
                            ▼
┌────────────────────────────────────────────────────────┐
│                   Fasti Monorepo                       │
│                                                        │
│   ┌────────────────────────────────────────────────┐   │
│   │ fasti-core (UUIDv7, timestamps, actor, device) │   │
│   └───────────────────────┬────────────────────────┘   │
│                           │                            │
│   ┌───────────────────────▼────────────────────────┐   │
│   │ fasti-activity (Ledger envelope & idempotency) │   │
│   └───────────────────────┬────────────────────────┘   │
│                           │                            │
│   ┌───────────────────────▼────────────────────────┐   │
│   │ fasti-store (SQLite persistence in WAL mode)   │   │
│   └───────────────────────┬────────────────────────┘   │
│                           │                            │
│   ┌───────────────────────▼────────────────────────┐   │
│   │ fasti-projections (Deterministic views)        │   │
│   └────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────┘
```

---

## 2. The 3 Timestamp Semantics
Fasti records 3 distinct timestamps for every event:
1. `occurred_at`: When the activity occurred in the source media system.
2. `observed_at`: When the Fasti client or connector observed the play.
3. `received_at`: When the Fasti storage node accepted and committed the ledger transaction.

These timestamps are accompanied by a strictly monotonic `device_seq` per device, enabling sequence gap detection across offline periods.
