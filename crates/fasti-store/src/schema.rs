use rusqlite::{Connection, Result};

pub(crate) const SCHEMA_VERSION: i64 = 2;

pub(crate) fn migrate(connection: &Connection) -> Result<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 0 {
        migrate_v1(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 1 {
        migrate_v2(connection)?;
    }
    Ok(())
}

fn migrate_v1(connection: &Connection) -> Result<()> {
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

fn migrate_v2(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
        BEGIN IMMEDIATE;

        CREATE TABLE corrections (
            correction_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            observation_id TEXT NOT NULL REFERENCES observations(observation_id),
            prior_interpretation_id TEXT NOT NULL UNIQUE REFERENCES interpretations(interpretation_id),
            replacement_interpretation_id TEXT NOT NULL UNIQUE REFERENCES interpretations(interpretation_id),
            actor_client_id TEXT NOT NULL REFERENCES clients(client_id),
            record_id TEXT REFERENCES records(record_id),
            reason TEXT NOT NULL CHECK (length(reason) > 0 AND length(reason) <= 1024),
            created_at TEXT NOT NULL
        ) STRICT;
        CREATE INDEX corrections_scope_observation_idx
            ON corrections(workspace_id, profile_id, observation_id, created_at, correction_id);

        PRAGMA user_version = 2;
        COMMIT;
        "#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_database_reaches_current_schema() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate(&connection).expect("migrate fresh database");
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user version");
        assert_eq!(version, SCHEMA_VERSION);
        let corrections: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'corrections'",
                [],
                |row| row.get(0),
            )
            .expect("find corrections table");
        assert_eq!(corrections, 1);
    }

    #[test]
    fn version_one_database_upgrades_without_replaying_base_schema() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_v1(&connection).expect("create version one database");
        let before: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read version one");
        assert_eq!(before, 1);

        migrate(&connection).expect("upgrade version one database");
        let after: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read version two");
        assert_eq!(after, SCHEMA_VERSION);
    }

    /// Seed the minimum row set that a real version-one database would hold.
    ///
    /// The upgrade test above proves the version number moves. It inserts no
    /// rows, so a migration that dropped or rewrote user data would still pass
    /// it. These rows make that failure observable.
    fn seed_version_one_rows(connection: &Connection) {
        connection
            .execute_batch(
                r#"
                INSERT INTO workspaces(workspace_id, created_at)
                    VALUES ('wsp_seed', '2026-08-24T00:00:00Z');
                INSERT INTO profiles(profile_id, workspace_id, created_at)
                    VALUES ('prf_seed', 'wsp_seed', '2026-08-24T00:00:01Z');
                INSERT INTO clients(client_id, workspace_id, status,
                                    current_credential_epoch, created_at)
                    VALUES ('cli_seed', 'wsp_seed', 'active', 1, '2026-08-24T00:00:02Z');
                INSERT INTO evidence(evidence_id, workspace_id, digest, size_bytes,
                                     relative_path, created_at)
                    VALUES ('evd_seed', 'wsp_seed', 'sha256:aa', 2,
                            'payloads/sha256/aa/aa', '2026-08-24T00:00:03Z');
                INSERT INTO observations(observation_id, workspace_id, profile_id,
                                         source_client_id, evidence_id, occurred_at_json,
                                         observed_at_json, received_at, created_at)
                    VALUES ('obs_seed', 'wsp_seed', 'prf_seed', 'cli_seed', 'evd_seed',
                            NULL, '{"claim":"seed"}', '2026-08-24T00:00:04Z',
                            '2026-08-24T00:00:04Z');
                "#,
            )
            .expect("seed version one rows");
    }

    #[test]
    fn upgrading_a_populated_version_one_database_preserves_every_row() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_v1(&connection).expect("create version one database");
        seed_version_one_rows(&connection);

        migrate(&connection).expect("upgrade populated database");

        // Every seeded row must still be readable, and readable unchanged. A
        // migration that recreated a table and copied rows badly would show up
        // here rather than in production.
        for (table, key_column, key) in [
            ("workspaces", "workspace_id", "wsp_seed"),
            ("profiles", "profile_id", "prf_seed"),
            ("clients", "client_id", "cli_seed"),
            ("evidence", "evidence_id", "evd_seed"),
            ("observations", "observation_id", "obs_seed"),
        ] {
            let found: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {key_column} = ?1"),
                    [key],
                    |row| row.get(0),
                )
                .unwrap_or_else(|error| panic!("read {table} after upgrade: {error}"));
            assert_eq!(found, 1, "{table} lost {key} during the upgrade");
        }

        let observed: String = connection
            .query_row(
                "SELECT observed_at_json FROM observations WHERE observation_id = 'obs_seed'",
                [],
                |row| row.get(0),
            )
            .expect("read observation payload after upgrade");
        assert_eq!(
            observed, r#"{"claim":"seed"}"#,
            "observation payload was rewritten during the upgrade"
        );

        // Foreign keys must still resolve after the schema changed underneath
        // them. A silent constraint break is worse than a loud migration error.
        let violations: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .expect("run foreign key check");
        assert_eq!(violations, 0, "upgrade left dangling foreign keys");
    }

    #[test]
    fn migrating_twice_is_a_no_op() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate(&connection).expect("first migration");
        seed_version_one_rows(&connection);

        // Re-running must not replay any step. Replaying migrate_v1 would fail
        // on CREATE TABLE, and replaying migrate_v2 would drop the seeded rows.
        migrate(&connection).expect("second migration is a no-op");

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user version");
        assert_eq!(version, SCHEMA_VERSION);
        let workspaces: i64 = connection
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .expect("count workspaces");
        assert_eq!(workspaces, 1, "re-running migrate disturbed existing rows");
    }

    #[test]
    fn a_database_from_a_newer_fasti_is_left_untouched() {
        // Downgrade path. If a user runs a newer Fasti and then an older one,
        // the older binary must not touch the file. Silently operating on a
        // schema it does not understand is the corruption case; `migrate` must
        // leave the version alone so `SqliteKernel::open` can reject it.
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate(&connection).expect("reach current schema");
        seed_version_one_rows(&connection);
        connection
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .expect("simulate a newer schema");

        migrate(&connection).expect("migrate must not error on a newer schema");

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read user version");
        assert_eq!(
            version,
            SCHEMA_VERSION + 1,
            "an older binary must not rewrite a newer schema version"
        );
        let workspaces: i64 = connection
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .expect("count workspaces");
        assert_eq!(
            workspaces, 1,
            "a newer database lost rows to an older binary"
        );
    }
}
