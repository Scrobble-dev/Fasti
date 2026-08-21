//! Fasti SQLite Storage Engine.

use fasti_activity::{ActivityEvent, EventReceipt, ReceiptStatus};
use rusqlite::{params, Connection};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// SQLite-backed store for the Fasti immutable event ledger, identity graph, and projections.
pub struct EventStore {
    conn: Connection,
}

impl EventStore {
    /// Opens or creates a SQLite event store at the specified path with WAL mode enabled.
    pub fn open<P: AsRef<Path>>(path: P) -> std::result::Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        
        let store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    /// Initializes in-memory SQLite store for testing.
    pub fn in_memory() -> std::result::Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&self) -> std::result::Result<(), StoreError> {
        self.conn.execute_batch(
            r#"
            -- 1. Canonical Fasti Records
            CREATE TABLE IF NOT EXISTS records (
                record_id TEXT PRIMARY KEY,
                workspace_id TEXT NOT NULL,
                record_type TEXT NOT NULL,
                grain TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_records_workspace ON records(workspace_id, record_type);

            -- 2. Registered Namespace Definitions
            CREATE TABLE IF NOT EXISTS namespace_definitions (
                namespace TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                grain TEXT NOT NULL,
                id_pattern TEXT NOT NULL,
                normalization TEXT NOT NULL,
                licence_posture TEXT NOT NULL
            );

            -- 3. External Identifiers attached to Records
            CREATE TABLE IF NOT EXISTS external_identifiers (
                external_identifier_id TEXT PRIMARY KEY,
                record_id TEXT NOT NULL REFERENCES records(record_id),
                namespace TEXT NOT NULL,
                value TEXT NOT NULL,
                grain TEXT NOT NULL,
                status TEXT NOT NULL,
                observed_via TEXT NOT NULL,
                first_seen_at TEXT NOT NULL,
                last_verified_at TEXT
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_xid_unique ON external_identifiers(namespace, value, grain);
            CREATE INDEX IF NOT EXISTS idx_xid_record ON external_identifiers(record_id);

            -- 4. Identity Assertions (Mappings)
            CREATE TABLE IF NOT EXISTS identity_assertions (
                assertion_id TEXT PRIMARY KEY,
                from_xid TEXT NOT NULL,
                to_xid TEXT NOT NULL,
                relation TEXT NOT NULL,
                direction TEXT NOT NULL,
                evidence_json TEXT NOT NULL,
                status TEXT NOT NULL,
                policy_version TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_assertions_from ON identity_assertions(from_xid);
            CREATE INDEX IF NOT EXISTS idx_assertions_to ON identity_assertions(to_xid);

            -- 5. Immutable Activity Ledger
            CREATE TABLE IF NOT EXISTS activity_ledger (
                event_id TEXT PRIMARY KEY,
                schema_version TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                device_seq INTEGER NOT NULL,
                kind TEXT NOT NULL,
                media_source TEXT NOT NULL,
                media_id TEXT NOT NULL,
                record_id TEXT REFERENCES records(record_id),
                occurred_at TEXT NOT NULL,
                observed_at TEXT NOT NULL,
                received_at TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                correction_of TEXT,
                tombstone_of TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_activity_occurred_at ON activity_ledger(occurred_at);
            CREATE INDEX IF NOT EXISTS idx_activity_media ON activity_ledger(media_source, media_id);
            CREATE INDEX IF NOT EXISTS idx_activity_device_seq ON activity_ledger(device_id, device_seq);
            "#,
        )?;
        Ok(())
    }

    /// Appends an activity event idempotently to the ledger.
    pub fn append(&self, event: &ActivityEvent) -> std::result::Result<EventReceipt, StoreError> {
        let payload = serde_json::to_string(event)?;
        let mut stmt = self.conn.prepare_cached(
            r#"
            INSERT OR IGNORE INTO activity_ledger (
                event_id, schema_version, actor_id, device_id, device_seq,
                kind, media_source, media_id, record_id, occurred_at, observed_at,
                received_at, payload_json, correction_of, tombstone_of
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
        )?;

        let rows_affected = stmt.execute(params![
            event.event_id.0.to_string(),
            event.schema_version,
            event.actor_id,
            event.device_id,
            event.device_seq,
            event.kind,
            event.media.source,
            event.media.id,
            event.record_id.as_ref().map(|r| r.0.as_str()),
            event.timestamps.occurred_at.to_rfc3339(),
            event.timestamps.observed_at.to_rfc3339(),
            event.timestamps.received_at.to_rfc3339(),
            payload,
            event.correction_of.map(|id| id.0.to_string()),
            event.tombstone_of.map(|id| id.0.to_string()),
        ])?;

        let status = if rows_affected > 0 {
            if event.correction_of.is_some() {
                ReceiptStatus::CorrectionAccepted
            } else {
                ReceiptStatus::Committed
            }
        } else {
            ReceiptStatus::DuplicateIgnored
        };

        Ok(EventReceipt {
            event_id: event.event_id,
            received_at: event.timestamps.received_at,
            status,
        })
    }
}
