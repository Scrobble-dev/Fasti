use rusqlite::{Connection, Result};

pub(crate) const SCHEMA_VERSION: i64 = 1;

pub(crate) fn migrate(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
        BEGIN IMMEDIATE;

        CREATE TABLE IF NOT EXISTS node_state (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            initialized INTEGER NOT NULL CHECK (initialized IN (0, 1)),
            workspace_id TEXT,
            profile_id TEXT,
            client_id TEXT,
            initialization_digest TEXT,
            initialization_expires_at TEXT,
            initialization_consumed_at TEXT,
            created_at TEXT NOT NULL
        ) STRICT;

        CREATE TABLE IF NOT EXISTS workspaces (
            workspace_id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL
        ) STRICT;

        CREATE TABLE IF NOT EXISTS profiles (
            profile_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            created_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX IF NOT EXISTS profiles_workspace_idx
            ON profiles(workspace_id, profile_id);

        CREATE TABLE IF NOT EXISTS clients (
            client_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
            current_credential_epoch INTEGER NOT NULL CHECK (current_credential_epoch >= 0),
            created_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX IF NOT EXISTS clients_workspace_idx
            ON clients(workspace_id, client_id);

        CREATE TABLE IF NOT EXISTS credentials (
            credential_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            client_id TEXT NOT NULL REFERENCES clients(client_id),
            digest TEXT NOT NULL UNIQUE,
            epoch INTEGER NOT NULL CHECK (epoch >= 1),
            status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
            created_at TEXT NOT NULL,
            revoked_at TEXT
        ) STRICT;
        CREATE INDEX IF NOT EXISTS credentials_client_idx
            ON credentials(client_id, status, epoch);

        CREATE TABLE IF NOT EXISTS profile_grants (
            grant_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            client_id TEXT NOT NULL REFERENCES clients(client_id),
            status TEXT NOT NULL CHECK (status IN ('active', 'revoked')),
            created_at TEXT NOT NULL,
            revoked_at TEXT,
            UNIQUE(workspace_id, profile_id, client_id)
        ) STRICT;

        CREATE TABLE IF NOT EXISTS grant_scopes (
            grant_id TEXT NOT NULL REFERENCES profile_grants(grant_id) ON DELETE CASCADE,
            scope_key TEXT NOT NULL,
            PRIMARY KEY (grant_id, scope_key)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS records (
            record_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            grain TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status = 'active'),
            created_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX IF NOT EXISTS records_workspace_grain_idx
            ON records(workspace_id, grain, record_id);

        CREATE TABLE IF NOT EXISTS external_identifiers (
            external_identifier_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            namespace TEXT NOT NULL,
            grain TEXT NOT NULL,
            value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(workspace_id, namespace, grain, value)
        ) STRICT;
        CREATE INDEX IF NOT EXISTS external_identifiers_record_idx
            ON external_identifiers(workspace_id, record_id);

        CREATE TABLE IF NOT EXISTS evidence (
            evidence_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            digest TEXT NOT NULL,
            size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
            relative_path TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(workspace_id, digest)
        ) STRICT;

        CREATE TABLE IF NOT EXISTS observations (
            observation_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            source_client_id TEXT NOT NULL REFERENCES clients(client_id),
            evidence_id TEXT NOT NULL REFERENCES evidence(evidence_id),
            occurred_at_json TEXT,
            observed_at_json TEXT NOT NULL,
            received_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX IF NOT EXISTS observations_profile_idx
            ON observations(workspace_id, profile_id, observation_id);

        CREATE TABLE IF NOT EXISTS observation_clues (
            observation_id TEXT NOT NULL REFERENCES observations(observation_id) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
            namespace TEXT NOT NULL,
            grain TEXT NOT NULL,
            value TEXT NOT NULL,
            PRIMARY KEY (observation_id, ordinal)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS occurrences (
            occurrence_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            observation_id TEXT NOT NULL UNIQUE REFERENCES observations(observation_id),
            record_id TEXT REFERENCES records(record_id),
            occurred_at_json TEXT,
            created_at TEXT NOT NULL
        ) STRICT;

        CREATE TABLE IF NOT EXISTS interpretations (
            interpretation_id TEXT PRIMARY KEY,
            observation_id TEXT NOT NULL REFERENCES observations(observation_id),
            occurrence_id TEXT NOT NULL REFERENCES occurrences(occurrence_id),
            prior_interpretation_id TEXT REFERENCES interpretations(interpretation_id),
            record_id TEXT REFERENCES records(record_id),
            state TEXT NOT NULL CHECK (state IN ('unresolved', 'resolved', 'conflicted')),
            created_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX IF NOT EXISTS interpretations_observation_idx
            ON interpretations(observation_id, created_at, interpretation_id);

        CREATE TABLE IF NOT EXISTS review_items (
            review_item_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            observation_id TEXT NOT NULL UNIQUE REFERENCES observations(observation_id),
            current_interpretation_id TEXT NOT NULL REFERENCES interpretations(interpretation_id),
            status TEXT NOT NULL CHECK (status IN ('open', 'deferred', 'resolved')),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX IF NOT EXISTS review_items_profile_status_idx
            ON review_items(workspace_id, profile_id, status, review_item_id);

        CREATE TABLE IF NOT EXISTS review_candidates (
            review_item_id TEXT NOT NULL REFERENCES review_items(review_item_id) ON DELETE CASCADE,
            record_id TEXT NOT NULL REFERENCES records(record_id),
            PRIMARY KEY (review_item_id, record_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS receipts (
            sequence INTEGER PRIMARY KEY AUTOINCREMENT,
            receipt_id TEXT NOT NULL UNIQUE,
            operation_id TEXT NOT NULL,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            client_id TEXT NOT NULL REFERENCES clients(client_id),
            capability_key TEXT NOT NULL,
            observation_id TEXT NOT NULL REFERENCES observations(observation_id),
            occurrence_id TEXT REFERENCES occurrences(occurrence_id),
            interpretation_id TEXT REFERENCES interpretations(interpretation_id),
            record_id TEXT REFERENCES records(record_id),
            review_item_id TEXT REFERENCES review_items(review_item_id),
            evidence_id TEXT NOT NULL REFERENCES evidence(evidence_id),
            payload_digest TEXT NOT NULL,
            resolution TEXT NOT NULL CHECK (resolution IN ('unresolved', 'resolved', 'conflicted')),
            received_at TEXT NOT NULL,
            committed_at TEXT NOT NULL,
            created_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX IF NOT EXISTS receipts_scope_sequence_idx
            ON receipts(workspace_id, profile_id, client_id, sequence);

        CREATE TABLE IF NOT EXISTS operations (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            client_id TEXT NOT NULL REFERENCES clients(client_id),
            operation_id TEXT NOT NULL,
            capability_key TEXT NOT NULL,
            semantic_digest TEXT NOT NULL,
            receipt_id TEXT NOT NULL UNIQUE REFERENCES receipts(receipt_id),
            created_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, client_id, operation_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE IF NOT EXISTS listener_configuration (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            listen TEXT NOT NULL,
            remote_enabled INTEGER NOT NULL CHECK (remote_enabled IN (0, 1)),
            updated_at TEXT NOT NULL
        ) STRICT;

        PRAGMA user_version = 1;
        COMMIT;
        "#,
    )?;
    Ok(())
}
