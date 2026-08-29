use crate::{access::V8_NODE_OWNER_SCOPE_BACKFILL, kernel::scope_storage_key};
use fasti_domain::{Grain, MAX_EXTERNAL_IDENTIFIER_BYTES};
use rusqlite::{Connection, Result, Transaction, TransactionBehavior};
use std::fmt::Write as _;

pub(crate) const SCHEMA_VERSION: i64 = 9;

pub(crate) fn migrate(connection: &Connection) -> Result<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 0 {
        migrate_v1(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 1 {
        migrate_v2(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 2 {
        migrate_v3(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 3 {
        migrate_v4(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 4 {
        migrate_v5(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 5 {
        migrate_v6(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 6 {
        migrate_v7(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 7 {
        migrate_v8(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 8 {
        migrate_v9(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == SCHEMA_VERSION {
        let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
        repair_legacy_provider_coordinates_v1(&transaction)?;
        transaction.commit()?;
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

struct RevisionSource {
    table: &'static str,
    new_workspace: &'static str,
    old_workspace: &'static str,
}

const REVISION_SOURCES: &[RevisionSource] = &[
    RevisionSource {
        table: "workspaces",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "profiles",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "clients",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "records",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "external_identifiers",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "evidence",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "observations",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "observation_clues",
        new_workspace:
            "(SELECT workspace_id FROM observations WHERE observation_id = NEW.observation_id)",
        old_workspace:
            "(SELECT workspace_id FROM observations WHERE observation_id = OLD.observation_id)",
    },
    RevisionSource {
        table: "occurrences",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "interpretations",
        new_workspace:
            "(SELECT workspace_id FROM observations WHERE observation_id = NEW.observation_id)",
        old_workspace:
            "(SELECT workspace_id FROM observations WHERE observation_id = OLD.observation_id)",
    },
    RevisionSource {
        table: "review_items",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "review_candidates",
        new_workspace:
            "(SELECT workspace_id FROM review_items WHERE review_item_id = NEW.review_item_id)",
        old_workspace:
            "(SELECT workspace_id FROM review_items WHERE review_item_id = OLD.review_item_id)",
    },
    RevisionSource {
        table: "corrections",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "receipts",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
    RevisionSource {
        table: "operations",
        new_workspace: "NEW.workspace_id",
        old_workspace: "OLD.workspace_id",
    },
];

fn migrate_v3(connection: &Connection) -> Result<()> {
    let mut sql = String::from(
        r#"
        BEGIN IMMEDIATE;

        -- This table intentionally has no workspace foreign key. A workspace
        -- deletion must advance, not erase, its last durable revision.
        CREATE TABLE workspace_revisions (
            workspace_id TEXT PRIMARY KEY,
            revision INTEGER NOT NULL
                CHECK (revision >= 0 AND typeof(revision) = 'integer')
        ) STRICT;

        INSERT INTO workspace_revisions(workspace_id, revision)
        SELECT workspace_id, 0 FROM workspaces;
        "#,
    );

    for source in REVISION_SOURCES {
        append_revision_triggers(&mut sql, source);
    }

    sql.push_str(
        r#"
        PRAGMA user_version = 3;
        COMMIT;
        "#,
    );
    connection.execute_batch(&sql)
}

fn append_revision_triggers(sql: &mut String, source: &RevisionSource) {
    write!(
        sql,
        r#"
            CREATE TRIGGER workspace_revision_{table}_insert
            AFTER INSERT ON {table}
            BEGIN
                INSERT INTO workspace_revisions(workspace_id, revision)
                VALUES ({new_workspace}, 1)
                ON CONFLICT(workspace_id) DO UPDATE
                    SET revision = workspace_revisions.revision + 1;
            END;

            CREATE TRIGGER workspace_revision_{table}_update
            AFTER UPDATE ON {table}
            BEGIN
                INSERT INTO workspace_revisions(workspace_id, revision)
                VALUES ({old_workspace}, 1)
                ON CONFLICT(workspace_id) DO UPDATE
                    SET revision = workspace_revisions.revision + 1;

                INSERT INTO workspace_revisions(workspace_id, revision)
                SELECT {new_workspace}, 1
                WHERE {new_workspace} IS NOT {old_workspace}
                ON CONFLICT(workspace_id) DO UPDATE
                    SET revision = workspace_revisions.revision + 1;
            END;

            CREATE TRIGGER workspace_revision_{table}_delete
            AFTER DELETE ON {table}
            BEGIN
                INSERT INTO workspace_revisions(workspace_id, revision)
                SELECT {old_workspace}, 1
                WHERE {old_workspace} IS NOT NULL
                ON CONFLICT(workspace_id) DO UPDATE
                    SET revision = workspace_revisions.revision + 1;
            END;
            "#,
        table = source.table,
        new_workspace = source.new_workspace,
        old_workspace = source.old_workspace,
    )
    .expect("writing migration SQL to a String cannot fail");
}

fn migrate_v4(connection: &Connection) -> Result<()> {
    let mut sql = String::from(
        r#"
        BEGIN IMMEDIATE;

        CREATE TABLE namespace_definitions (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            namespace TEXT NOT NULL,
            label TEXT NOT NULL,
            supported_grains TEXT NOT NULL,
            id_pattern TEXT NOT NULL,
            normalization TEXT NOT NULL,
            licence_posture TEXT NOT NULL CHECK (
                licence_posture IN (
                    'open', 'identifiers_only', 'indirect_only', 'excluded', 'unknown'
                )
            ),
            created_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, namespace)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER external_identifier_namespace_insert_guard
        BEFORE INSERT ON external_identifiers
        WHEN NOT EXISTS (
            SELECT 1 FROM namespace_definitions
            WHERE workspace_id = NEW.workspace_id
              AND namespace = NEW.namespace
              AND instr(',' || supported_grains || ',', ',' || NEW.grain || ',') > 0
        )
        BEGIN
            SELECT RAISE(ABORT, 'external identifier namespace is not registered for this grain');
        END;

        CREATE TRIGGER external_identifier_namespace_update_guard
        BEFORE UPDATE OF workspace_id, namespace, grain ON external_identifiers
        WHEN NOT EXISTS (
            SELECT 1 FROM namespace_definitions
            WHERE workspace_id = NEW.workspace_id
              AND namespace = NEW.namespace
              AND instr(',' || supported_grains || ',', ',' || NEW.grain || ',') > 0
        )
        BEGIN
            SELECT RAISE(ABORT, 'external identifier namespace is not registered for this grain');
        END;

        CREATE TRIGGER namespace_definition_delete_guard
        BEFORE DELETE ON namespace_definitions
        WHEN EXISTS (
            SELECT 1 FROM external_identifiers
            WHERE workspace_id = OLD.workspace_id AND namespace = OLD.namespace
        )
        BEGIN
            SELECT RAISE(ABORT, 'namespace definition is referenced by external identifiers');
        END;

        CREATE TRIGGER namespace_definition_update_guard
        BEFORE UPDATE OF workspace_id, namespace, supported_grains ON namespace_definitions
        WHEN EXISTS (
            SELECT 1 FROM external_identifiers
            WHERE workspace_id = OLD.workspace_id
              AND namespace = OLD.namespace
              AND (
                  NEW.workspace_id IS NOT OLD.workspace_id
                  OR NEW.namespace IS NOT OLD.namespace
                  OR instr(',' || NEW.supported_grains || ',', ',' || grain || ',') = 0
              )
        )
        BEGIN
            SELECT RAISE(ABORT, 'namespace definition update would orphan external identifiers');
        END;
        "#,
    );
    append_revision_triggers(
        &mut sql,
        &RevisionSource {
            table: "namespace_definitions",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
    );
    sql.push_str(
        r#"
        PRAGMA user_version = 4;
        COMMIT;
        "#,
    );
    connection.execute_batch(&sql)
}

fn migrate_v5(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        r#"
        BEGIN IMMEDIATE;

        ALTER TABLE node_state ADD COLUMN recovery_restore_attempt_id TEXT;

        PRAGMA user_version = 5;
        COMMIT;
        "#,
    )
}

fn migrate_v6(connection: &Connection) -> Result<()> {
    let mut sql = String::from(
        r#"
        BEGIN IMMEDIATE;

        -- Every claim a provider ever supplied for one Record field, kept as
        -- history rather than overwritten in place: metadata.rs's resolver
        -- picks the winning tier from the full set, and a stale or expired
        -- claim is evidence the resolver itself weighs, not a row to discard.
        CREATE TABLE metadata_field_claims (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            field_key TEXT NOT NULL,
            source TEXT NOT NULL,
            value TEXT NOT NULL,
            locale TEXT,
            fetched_at TEXT NOT NULL,
            expires_at TEXT,
            created_at TEXT NOT NULL,
            PRIMARY KEY (record_id, field_key, source, fetched_at)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_field_claims_record_field_idx
            ON metadata_field_claims(workspace_id, record_id, field_key);

        -- A user-owned value for one field. Single row per (record, field):
        -- an override is not versioned history, it is the current profile
        -- decision, and a later override simply replaces it.
        CREATE TABLE metadata_field_overrides (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            field_key TEXT NOT NULL,
            value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (record_id, field_key)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_field_overrides_record_field_idx
            ON metadata_field_overrides(workspace_id, record_id, field_key);

        CREATE TRIGGER metadata_field_claims_scope_insert
        BEFORE INSERT ON metadata_field_claims
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata field claim crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_field_claims_scope_update
        BEFORE UPDATE ON metadata_field_claims
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata field claim crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_field_overrides_scope_insert
        BEFORE INSERT ON metadata_field_overrides
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata field override crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_field_overrides_scope_update
        BEFORE UPDATE ON metadata_field_overrides
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata field override crosses a workspace boundary');
        END;
        "#,
    );
    for source in [
        RevisionSource {
            table: "metadata_field_claims",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
        RevisionSource {
            table: "metadata_field_overrides",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
    ] {
        append_revision_triggers(&mut sql, &source);
    }
    sql.push_str(
        r#"
        PRAGMA user_version = 6;
        COMMIT;
        "#,
    );
    connection.execute_batch(&sql)
}

fn migrate_v7(connection: &Connection) -> Result<()> {
    let mut sql = String::from(
        r#"
        BEGIN IMMEDIATE;

        CREATE TABLE profile_record_tracking_dispositions (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            disposition TEXT NOT NULL CHECK (disposition IN ('watching', 'on_hold', 'dropped')),
            updated_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, profile_id, record_id)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX profile_record_tracking_dispositions_profile_idx
            ON profile_record_tracking_dispositions(workspace_id, profile_id, record_id);

        CREATE TRIGGER profile_record_tracking_dispositions_scope_insert
        BEFORE INSERT ON profile_record_tracking_dispositions
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'profile tracking disposition crosses a workspace boundary');
        END;

        CREATE TRIGGER profile_record_tracking_dispositions_scope_update
        BEFORE UPDATE ON profile_record_tracking_dispositions
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'profile tracking disposition crosses a workspace boundary');
        END;
        "#,
    );
    append_revision_triggers(
        &mut sql,
        &RevisionSource {
            table: "profile_record_tracking_dispositions",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
    );
    sql.push_str(
        r#"
        PRAGMA user_version = 7;
        COMMIT;
        "#,
    );
    connection.execute_batch(&sql)
}

fn migrate_v8(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        CREATE TABLE browser_users (
            user_id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            client_id TEXT NOT NULL REFERENCES clients(client_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            is_admin INTEGER NOT NULL CHECK (is_admin IN (0, 1)),
            is_test_account INTEGER NOT NULL CHECK (is_test_account IN (0, 1)),
            active INTEGER NOT NULL CHECK (active IN (0, 1)),
            failed_login_count INTEGER NOT NULL DEFAULT 0 CHECK (failed_login_count >= 0),
            locked_until TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        ) STRICT;

        CREATE TRIGGER browser_users_scope_insert
        BEFORE INSERT ON browser_users
        WHEN NOT EXISTS (
            SELECT 1
            FROM clients c
            JOIN profiles p ON p.profile_id = NEW.profile_id
            WHERE c.client_id = NEW.client_id
              AND c.workspace_id = p.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'browser user crosses a workspace boundary');
        END;

        CREATE TRIGGER browser_users_scope_update
        BEFORE UPDATE ON browser_users
        WHEN NOT EXISTS (
            SELECT 1
            FROM clients c
            JOIN profiles p ON p.profile_id = NEW.profile_id
            WHERE c.client_id = NEW.client_id
              AND c.workspace_id = p.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'browser user crosses a workspace boundary');
        END;

        CREATE TABLE browser_sessions (
            session_digest TEXT PRIMARY KEY,
            csrf_digest TEXT NOT NULL,
            user_id TEXT NOT NULL REFERENCES browser_users(user_id) ON DELETE CASCADE,
            expires_at TEXT NOT NULL,
            created_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX browser_sessions_user_idx
            ON browser_sessions(user_id, expires_at);

        -- This marker intentionally survives deletion or renaming of the
        -- development account so startup never recreates a deleted user.
        CREATE TABLE browser_auth_bootstrap (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            seeded_at TEXT NOT NULL
        ) STRICT;

        "#,
    )?;
    for scope in V8_NODE_OWNER_SCOPE_BACKFILL {
        transaction.execute(
            r#"
            INSERT OR IGNORE INTO grant_scopes(grant_id, scope_key)
            SELECT pg.grant_id, ?1
            FROM profile_grants pg
            JOIN node_state ns
              ON ns.singleton = 1
             AND ns.client_id = pg.client_id
             AND ns.profile_id = pg.profile_id
            WHERE pg.status = 'active'
            "#,
            [scope_storage_key(*scope)],
        )?;
    }
    transaction.pragma_update(None, "user_version", 8)?;
    transaction.commit()
}

fn migrate_v9(connection: &Connection) -> Result<()> {
    let mut sql = String::from(
        r#"
        BEGIN IMMEDIATE;

        CREATE TABLE profile_nuvio_collections (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            document_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, profile_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER profile_nuvio_collections_scope_insert
        BEFORE INSERT ON profile_nuvio_collections
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'Nuvio Collections document crosses a workspace boundary');
        END;

        CREATE TRIGGER profile_nuvio_collections_scope_update
        BEFORE UPDATE ON profile_nuvio_collections
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'Nuvio Collections document crosses a workspace boundary');
        END;
        "#,
    );
    append_revision_triggers(
        &mut sql,
        &RevisionSource {
            table: "profile_nuvio_collections",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
    );
    sql.push_str(
        r#"
        PRAGMA user_version = 9;
        COMMIT;
        "#,
    );
    connection.execute_batch(&sql)
}

fn migration_conflict(message: &str) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
        Some(message.to_owned()),
    )
}

// This immutable snapshot keeps the schema-neutral v9 data repair
// reproducible if the live provider policy changes later.
#[derive(Clone, Copy)]
struct ProviderCoordinateRepairV1 {
    legacy_namespace: &'static str,
    legacy_label: &'static str,
    legacy_grains: &'static str,
    legacy_pattern: &'static str,
    legacy_grain: Grain,
    namespace: &'static str,
    label: &'static str,
    grain: Grain,
    value_kind: RepairIdentifierValueKindV1,
}

#[derive(Clone, Copy)]
enum RepairIdentifierValueKindV1 {
    PositiveDecimal,
    AsciiToken,
}

impl RepairIdentifierValueKindV1 {
    const fn pattern(self) -> &'static str {
        match self {
            Self::PositiveDecimal => "^[1-9][0-9]*$",
            Self::AsciiToken => "[A-Za-z0-9_-]+",
        }
    }

    fn accepts(self, value: &str) -> bool {
        value == value.trim()
            && !value.is_empty()
            && value.len() <= MAX_EXTERNAL_IDENTIFIER_BYTES
            && match self {
                Self::PositiveDecimal => {
                    let mut bytes = value.bytes();
                    bytes.next().is_some_and(|byte| matches!(byte, b'1'..=b'9'))
                        && bytes.all(|byte| byte.is_ascii_digit())
                }
                Self::AsciiToken => value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
            }
    }
}

const GOOGLE_BOOKS_PROVIDER_ID: &str = "google-books";
const TMDB_PROVIDER_ID: &str = "tmdb";
const GOOGLE_BOOKS_REPAIR_V1: ProviderCoordinateRepairV1 = ProviderCoordinateRepairV1 {
    legacy_namespace: GOOGLE_BOOKS_PROVIDER_ID,
    legacy_label: GOOGLE_BOOKS_PROVIDER_ID,
    legacy_grains: "chapter",
    legacy_pattern: ".+",
    legacy_grain: Grain::Chapter,
    namespace: "googlebooks.volume",
    label: "Google Books Volume",
    grain: Grain::Edition,
    value_kind: RepairIdentifierValueKindV1::AsciiToken,
};
const TMDB_MOVIE_REPAIR_V1: ProviderCoordinateRepairV1 = ProviderCoordinateRepairV1 {
    legacy_namespace: TMDB_PROVIDER_ID,
    legacy_label: "The Movie Database (TMDB)",
    legacy_grains: "film,series",
    legacy_pattern: "[0-9]+",
    legacy_grain: Grain::Film,
    namespace: "tmdb.movie",
    label: "TMDB Movie",
    grain: Grain::Film,
    value_kind: RepairIdentifierValueKindV1::PositiveDecimal,
};
const TMDB_SHOW_REPAIR_V1: ProviderCoordinateRepairV1 = ProviderCoordinateRepairV1 {
    legacy_namespace: TMDB_PROVIDER_ID,
    legacy_label: "The Movie Database (TMDB)",
    legacy_grains: "film,series",
    legacy_pattern: "[0-9]+",
    legacy_grain: Grain::Series,
    namespace: "tmdb.tv",
    label: "TMDB TV",
    grain: Grain::Series,
    value_kind: RepairIdentifierValueKindV1::PositiveDecimal,
};
const PROVIDER_COORDINATE_REPAIRS_V1: [ProviderCoordinateRepairV1; 3] = [
    GOOGLE_BOOKS_REPAIR_V1,
    TMDB_MOVIE_REPAIR_V1,
    TMDB_SHOW_REPAIR_V1,
];

fn install_canonical_provider_namespace(
    transaction: &Transaction<'_>,
    mapping: ProviderCoordinateRepairV1,
) -> Result<()> {
    transaction.execute(
        r#"
        INSERT OR IGNORE INTO namespace_definitions(
            workspace_id, namespace, label, supported_grains, id_pattern,
            normalization, licence_posture, created_at
        )
        SELECT workspace_id, ?1, ?2, ?3, ?4, ?5, ?6, created_at
        FROM namespace_definitions
        WHERE namespace = ?7
          AND label = ?8
          AND supported_grains = ?9
          AND id_pattern = ?10
          AND normalization = 'identity'
          AND licence_posture = 'identifiers_only'
        "#,
        rusqlite::params![
            mapping.namespace,
            mapping.label,
            mapping.grain.as_str(),
            mapping.value_kind.pattern(),
            "identity",
            "identifiers_only",
            mapping.legacy_namespace,
            mapping.legacy_label,
            mapping.legacy_grains,
            mapping.legacy_pattern,
        ],
    )?;
    let incompatible: bool = transaction.query_row(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM namespace_definitions legacy
            LEFT JOIN namespace_definitions canonical
              ON canonical.workspace_id = legacy.workspace_id
             AND canonical.namespace = ?1
             AND canonical.label = ?2
             AND canonical.supported_grains = ?3
             AND canonical.id_pattern = ?4
             AND canonical.normalization = ?5
             AND canonical.licence_posture = ?6
            WHERE legacy.namespace = ?7
              AND legacy.label = ?8
              AND legacy.supported_grains = ?9
              AND legacy.id_pattern = ?10
              AND legacy.normalization = 'identity'
              AND legacy.licence_posture = 'identifiers_only'
              AND canonical.workspace_id IS NULL
        )
        "#,
        rusqlite::params![
            mapping.namespace,
            mapping.label,
            mapping.grain.as_str(),
            mapping.value_kind.pattern(),
            "identity",
            "identifiers_only",
            mapping.legacy_namespace,
            mapping.legacy_label,
            mapping.legacy_grains,
            mapping.legacy_pattern,
        ],
        |row| row.get(0),
    )?;
    if incompatible {
        return Err(migration_conflict(
            "canonical provider namespace conflicts with an existing definition",
        ));
    }
    Ok(())
}

fn validate_legacy_provider_values(
    transaction: &Transaction<'_>,
    mapping: ProviderCoordinateRepairV1,
) -> Result<()> {
    let mut statement = transaction.prepare(
        r#"
        SELECT identifier.value
        FROM external_identifiers identifier
        JOIN namespace_definitions definition
          ON definition.workspace_id = identifier.workspace_id
         AND definition.namespace = identifier.namespace
         AND definition.label = ?3
         AND definition.supported_grains = ?4
         AND definition.id_pattern = ?5
         AND definition.normalization = 'identity'
         AND definition.licence_posture = 'identifiers_only'
        WHERE identifier.namespace = ?1 AND identifier.grain = ?2
        "#,
    )?;
    let values = statement.query_map(
        rusqlite::params![
            mapping.legacy_namespace,
            mapping.legacy_grain.as_str(),
            mapping.legacy_label,
            mapping.legacy_grains,
            mapping.legacy_pattern,
        ],
        |row| row.get::<_, String>(0),
    )?;
    for value in values {
        if !mapping.value_kind.accepts(&value?) {
            return Err(migration_conflict(
                "legacy provider identifier does not satisfy the canonical value pattern",
            ));
        }
    }
    Ok(())
}

pub(crate) fn repair_legacy_provider_coordinates_v1(transaction: &Transaction<'_>) -> Result<()> {
    let books = GOOGLE_BOOKS_REPAIR_V1;

    for mapping in PROVIDER_COORDINATE_REPAIRS_V1 {
        install_canonical_provider_namespace(transaction, mapping)?;
    }

    let mixed_google_identity: bool = transaction.query_row(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM external_identifiers legacy
            JOIN namespace_definitions definition
              ON definition.workspace_id = legacy.workspace_id
             AND definition.namespace = legacy.namespace
             AND definition.label = 'google-books'
             AND definition.supported_grains = 'chapter'
             AND definition.id_pattern = '.+'
             AND definition.normalization = 'identity'
             AND definition.licence_posture = 'identifiers_only'
            WHERE legacy.namespace = 'google-books'
              AND legacy.grain = 'chapter'
              AND EXISTS (
                  SELECT 1 FROM external_identifiers other
                  WHERE other.record_id = legacy.record_id
                    AND (other.namespace <> 'google-books' OR other.grain <> 'chapter')
              )
        )
        "#,
        [],
        |row| row.get(0),
    )?;
    if mixed_google_identity {
        return Err(migration_conflict(
            "legacy Google Books record has mixed identity grains",
        ));
    }

    for mapping in PROVIDER_COORDINATE_REPAIRS_V1 {
        let mismatched_record_grain: bool = transaction.query_row(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM external_identifiers legacy
                JOIN records record ON record.record_id = legacy.record_id
                JOIN namespace_definitions definition
                  ON definition.workspace_id = legacy.workspace_id
                 AND definition.namespace = legacy.namespace
                 AND definition.label = ?3
                 AND definition.supported_grains = ?4
                 AND definition.id_pattern = ?5
                 AND definition.normalization = 'identity'
                 AND definition.licence_posture = 'identifiers_only'
                WHERE legacy.namespace = ?1
                  AND legacy.grain = ?2
                  AND record.grain <> legacy.grain
            )
            "#,
            rusqlite::params![
                mapping.legacy_namespace,
                mapping.legacy_grain.as_str(),
                mapping.legacy_label,
                mapping.legacy_grains,
                mapping.legacy_pattern,
            ],
            |row| row.get(0),
        )?;
        if mismatched_record_grain {
            return Err(migration_conflict(
                "legacy provider record and identifier grains disagree",
            ));
        }

        validate_legacy_provider_values(transaction, mapping)?;
        let conflict: bool = transaction.query_row(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM external_identifiers legacy
                JOIN namespace_definitions definition
                  ON definition.workspace_id = legacy.workspace_id
                 AND definition.namespace = legacy.namespace
                 AND definition.label = ?5
                 AND definition.supported_grains = ?6
                 AND definition.id_pattern = ?7
                 AND definition.normalization = 'identity'
                 AND definition.licence_posture = 'identifiers_only'
                JOIN external_identifiers canonical
                  ON canonical.workspace_id = legacy.workspace_id
                 AND canonical.namespace = ?1
                 AND canonical.grain = ?2
                 AND canonical.value = legacy.value
                WHERE legacy.namespace = ?3 AND legacy.grain = ?4
            )
            "#,
            rusqlite::params![
                mapping.namespace,
                mapping.grain.as_str(),
                mapping.legacy_namespace,
                mapping.legacy_grain.as_str(),
                mapping.legacy_label,
                mapping.legacy_grains,
                mapping.legacy_pattern,
            ],
            |row| row.get(0),
        )?;
        if conflict {
            return Err(migration_conflict(
                "legacy provider identifier conflicts with a canonical coordinate",
            ));
        }

        let claim_conflict: bool = transaction.query_row(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM metadata_field_claims legacy_claim
                JOIN external_identifiers legacy_identifier
                  ON legacy_identifier.record_id = legacy_claim.record_id
                 AND legacy_identifier.workspace_id = legacy_claim.workspace_id
                 AND legacy_identifier.namespace = ?1
                 AND legacy_identifier.grain = ?2
                JOIN namespace_definitions definition
                  ON definition.workspace_id = legacy_identifier.workspace_id
                 AND definition.namespace = legacy_identifier.namespace
                 AND definition.label = ?4
                 AND definition.supported_grains = ?5
                 AND definition.id_pattern = ?6
                 AND definition.normalization = 'identity'
                 AND definition.licence_posture = 'identifiers_only'
                JOIN metadata_field_claims canonical_claim
                  ON canonical_claim.record_id = legacy_claim.record_id
                 AND canonical_claim.field_key = legacy_claim.field_key
                 AND canonical_claim.source = ?3
                 AND canonical_claim.fetched_at = legacy_claim.fetched_at
                WHERE legacy_claim.source = ?1
            )
            "#,
            rusqlite::params![
                mapping.legacy_namespace,
                mapping.legacy_grain.as_str(),
                mapping.namespace,
                mapping.legacy_label,
                mapping.legacy_grains,
                mapping.legacy_pattern,
            ],
            |row| row.get(0),
        )?;
        if claim_conflict {
            return Err(migration_conflict(
                "legacy provider metadata conflicts with a canonical claim",
            ));
        }
    }

    transaction.execute(
        r#"
        UPDATE records SET grain = ?1
        WHERE grain = ?2 AND record_id IN (
            SELECT identifier.record_id
            FROM external_identifiers identifier
            JOIN namespace_definitions definition
              ON definition.workspace_id = identifier.workspace_id
             AND definition.namespace = identifier.namespace
             AND definition.label = 'google-books'
             AND definition.supported_grains = 'chapter'
             AND definition.id_pattern = '.+'
             AND definition.normalization = 'identity'
             AND definition.licence_posture = 'identifiers_only'
            WHERE identifier.namespace = ?3 AND identifier.grain = ?2
        )
        "#,
        rusqlite::params![
            books.grain.as_str(),
            Grain::Chapter.as_str(),
            GOOGLE_BOOKS_PROVIDER_ID,
        ],
    )?;
    for mapping in PROVIDER_COORDINATE_REPAIRS_V1 {
        transaction.execute(
            r#"
            UPDATE metadata_field_claims SET source = ?1
            WHERE source = ?2 AND record_id IN (
                SELECT identifier.record_id
                FROM external_identifiers identifier
                JOIN namespace_definitions definition
                  ON definition.workspace_id = identifier.workspace_id
                 AND definition.namespace = identifier.namespace
                 AND definition.label = ?4
                 AND definition.supported_grains = ?5
                 AND definition.id_pattern = ?6
                 AND definition.normalization = 'identity'
                 AND definition.licence_posture = 'identifiers_only'
                WHERE identifier.namespace = ?2 AND identifier.grain = ?3
            )
            "#,
            rusqlite::params![
                mapping.namespace,
                mapping.legacy_namespace,
                mapping.legacy_grain.as_str(),
                mapping.legacy_label,
                mapping.legacy_grains,
                mapping.legacy_pattern,
            ],
        )?;
        transaction.execute(
            r#"
            UPDATE external_identifiers SET namespace = ?1, grain = ?2
            WHERE namespace = ?3 AND grain = ?4
              AND EXISTS (
                  SELECT 1 FROM namespace_definitions definition
                  WHERE definition.workspace_id = external_identifiers.workspace_id
                    AND definition.namespace = external_identifiers.namespace
                    AND definition.label = ?5
                    AND definition.supported_grains = ?6
                    AND definition.id_pattern = ?7
                    AND definition.normalization = 'identity'
                    AND definition.licence_posture = 'identifiers_only'
              )
            "#,
            rusqlite::params![
                mapping.namespace,
                mapping.grain.as_str(),
                mapping.legacy_namespace,
                mapping.legacy_grain.as_str(),
                mapping.legacy_label,
                mapping.legacy_grains,
                mapping.legacy_pattern,
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn workspace_revision(connection: &Connection, workspace_id: &str) -> Result<i64> {
    connection.query_row(
        "SELECT revision FROM workspace_revisions WHERE workspace_id = ?1",
        [workspace_id],
        |row| row.get(0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::FULL_ADMIN_SCOPES;
    use rusqlite::params;
    use std::fs;

    const WORKSPACE: &str = "wsp_revision_test";
    const CREATED_AT: &str = "2026-08-24T00:00:00.000000Z";

    fn migrated_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate(&connection).expect("migrate database");
        connection
    }

    fn version_nine_connection() -> Connection {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_v1(&connection).expect("version one");
        migrate_v2(&connection).expect("version two");
        migrate_v3(&connection).expect("version three");
        migrate_v4(&connection).expect("version four");
        migrate_v5(&connection).expect("version five");
        migrate_v6(&connection).expect("version six");
        migrate_v7(&connection).expect("version seven");
        migrate_v8(&connection).expect("version eight");
        migrate_v9(&connection).expect("version nine");
        connection
    }

    fn seed_legacy_provider_records(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("workspace");
        for (namespace, label, grains, pattern) in [
            ("google-books", "google-books", "chapter", ".+"),
            ("tmdb", "The Movie Database (TMDB)", "film,series", "[0-9]+"),
        ] {
            connection
                .execute(
                    "INSERT INTO namespace_definitions(workspace_id, namespace, label, supported_grains, id_pattern, normalization, licence_posture, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'identity', 'identifiers_only', ?6)",
                    params![WORKSPACE, namespace, label, grains, pattern, CREATED_AT],
                )
                .expect("legacy provider namespace");
        }
        for (record, grain, identifier, namespace, value) in [
            ("rec_book", "chapter", "xid_book", "google-books", "book-1"),
            ("rec_movie", "film", "xid_movie", "tmdb", "42"),
            ("rec_show", "series", "xid_show", "tmdb", "42"),
        ] {
            connection
                .execute(
                    "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, ?3, 'active', ?4)",
                    params![record, WORKSPACE, grain, CREATED_AT],
                )
                .expect("legacy provider record");
            connection
                .execute(
                    "INSERT INTO external_identifiers(external_identifier_id, workspace_id, record_id, namespace, grain, value, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![identifier, WORKSPACE, record, namespace, grain, value, CREATED_AT],
                )
                .expect("legacy provider identifier");
            connection
                .execute(
                    "INSERT INTO metadata_field_claims(workspace_id, record_id, field_key, source, value, fetched_at, created_at) VALUES (?1, ?2, 'core.title', ?3, ?4, ?5, ?5)",
                    params![WORKSPACE, record, namespace, record, CREATED_AT],
                )
                .expect("legacy provider claim");
        }
    }

    fn seed_review_graph(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("workspace");
        for namespace in ["imdb", "tmdb"] {
            connection
                .execute(
                    "INSERT INTO namespace_definitions(workspace_id, namespace, label, supported_grains, id_pattern, normalization, licence_posture, created_at) VALUES (?1, ?2, ?2, 'film', '.+', 'identity', 'unknown', ?3)",
                    params![WORKSPACE, namespace, CREATED_AT],
                )
                .expect("namespace definition");
        }
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES ('prf_revision_test', ?1, ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("profile");
        connection
            .execute(
                "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES ('cli_revision_test', ?1, 'active', 1, ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("client");
        connection
            .execute(
                "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES ('rec_revision_test', ?1, 'film', 'active', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("record");
        connection
            .execute(
                "INSERT INTO evidence(evidence_id, workspace_id, digest, size_bytes, relative_path, created_at) VALUES ('evd_revision_test', ?1, ?2, 0, ?3, ?4)",
                params![
                    WORKSPACE,
                    format!("sha256:{}", "0".repeat(64)),
                    format!("payloads/sha256/00/{}", "0".repeat(64)),
                    CREATED_AT
                ],
            )
            .expect("evidence");
        connection
            .execute(
                r#"
                INSERT INTO observations(
                    observation_id, workspace_id, profile_id, source_client_id,
                    evidence_id, observed_at_json, received_at, created_at
                ) VALUES (
                    'obs_revision_test', ?1, 'prf_revision_test', 'cli_revision_test',
                    'evd_revision_test', '{}', ?2, ?2
                )
                "#,
                params![WORKSPACE, CREATED_AT],
            )
            .expect("observation");
        connection
            .execute(
                "INSERT INTO observation_clues(observation_id, ordinal, namespace, grain, value) VALUES ('obs_revision_test', 0, 'imdb', 'film', 'tt0000001')",
                [],
            )
            .expect("observation clue");
        connection
            .execute(
                "INSERT INTO occurrences(occurrence_id, workspace_id, profile_id, observation_id, created_at) VALUES ('occ_revision_test', ?1, 'prf_revision_test', 'obs_revision_test', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("occurrence");
        connection
            .execute(
                "INSERT INTO interpretations(interpretation_id, observation_id, occurrence_id, state, created_at) VALUES ('int_revision_test', 'obs_revision_test', 'occ_revision_test', 'unresolved', ?1)",
                [CREATED_AT],
            )
            .expect("interpretation");
        connection
            .execute(
                "INSERT INTO review_items(review_item_id, workspace_id, profile_id, observation_id, current_interpretation_id, status, created_at, updated_at) VALUES ('rev_revision_test', ?1, 'prf_revision_test', 'obs_revision_test', 'int_revision_test', 'open', ?2, ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("review item");
        connection
            .execute(
                "INSERT INTO review_candidates(review_item_id, record_id) VALUES ('rev_revision_test', 'rec_revision_test')",
                [],
            )
            .expect("review candidate");
    }

    #[test]
    fn fresh_database_reaches_current_schema() {
        let connection = migrated_connection();
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
        let namespace_definitions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'namespace_definitions'",
                [],
                |row| row.get(0),
            )
            .expect("find namespace definitions table");
        assert_eq!(namespace_definitions, 1);
        let trigger_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'workspace_revision_%'",
                [],
                |row| row.get(0),
            )
            .expect("count revision triggers");
        // +3 for namespace_definitions (migrate_v4), +6 for the two metadata
        // field tables (migrate_v6), +3 for profile tracking disposition
        // (migrate_v7), and +3 for profile Nuvio Collections (migrate_v9),
        // none of which are in the
        // original REVISION_SOURCES list built for the v3 schema snapshot.
        assert_eq!(
            trigger_count,
            (REVISION_SOURCES.len() * 3 + 3 + 6 + 3 + 3) as i64
        );
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
            .expect("read current version");
        assert_eq!(after, SCHEMA_VERSION);
    }

    #[test]
    fn provider_coordinate_repair_v1_preserves_schema_and_existing_records() {
        let connection = version_nine_connection();
        seed_legacy_provider_records(&connection);

        migrate(&connection).expect("repair legacy provider coordinates");
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, 9);

        let mut statement = connection
            .prepare(
                "SELECT record_id, namespace, grain, value FROM external_identifiers ORDER BY record_id",
            )
            .expect("identifier query");
        let identifiers = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .expect("identifiers")
            .collect::<Result<Vec<_>>>()
            .expect("identifier rows");
        assert_eq!(
            identifiers,
            vec![
                (
                    "rec_book".to_owned(),
                    "googlebooks.volume".to_owned(),
                    "edition".to_owned(),
                    "book-1".to_owned(),
                ),
                (
                    "rec_movie".to_owned(),
                    "tmdb.movie".to_owned(),
                    "film".to_owned(),
                    "42".to_owned(),
                ),
                (
                    "rec_show".to_owned(),
                    "tmdb.tv".to_owned(),
                    "series".to_owned(),
                    "42".to_owned(),
                ),
            ]
        );
        let book_grain: String = connection
            .query_row(
                "SELECT grain FROM records WHERE record_id = 'rec_book'",
                [],
                |row| row.get(0),
            )
            .expect("book grain");
        assert_eq!(book_grain, "edition");
        let sources = connection
            .prepare("SELECT source FROM metadata_field_claims ORDER BY record_id")
            .expect("claim query")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("claims")
            .collect::<Result<Vec<_>>>()
            .expect("claim rows");
        assert_eq!(sources, ["googlebooks.volume", "tmdb.movie", "tmdb.tv"]);
        let canonical_definitions = connection
            .prepare(
                "SELECT namespace, label, supported_grains, id_pattern FROM namespace_definitions WHERE namespace IN ('googlebooks.volume', 'tmdb.movie', 'tmdb.tv') ORDER BY namespace",
            )
            .expect("canonical namespace query")
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .expect("canonical definitions")
            .collect::<Result<Vec<_>>>()
            .expect("canonical definition rows");
        assert_eq!(
            canonical_definitions,
            [
                (
                    "googlebooks.volume".to_owned(),
                    "Google Books Volume".to_owned(),
                    "edition".to_owned(),
                    "[A-Za-z0-9_-]+".to_owned(),
                ),
                (
                    "tmdb.movie".to_owned(),
                    "TMDB Movie".to_owned(),
                    "film".to_owned(),
                    "^[1-9][0-9]*$".to_owned(),
                ),
                (
                    "tmdb.tv".to_owned(),
                    "TMDB TV".to_owned(),
                    "series".to_owned(),
                    "^[1-9][0-9]*$".to_owned(),
                ),
            ]
        );
        let legacy_definitions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM namespace_definitions WHERE namespace IN ('google-books', 'tmdb')",
                [],
                |row| row.get(0),
            )
            .expect("legacy definitions");
        assert_eq!(
            legacy_definitions, 2,
            "legacy import definitions remain available"
        );
        let changes = connection.total_changes();
        migrate(&connection).expect("repeat provider repair");
        assert_eq!(connection.total_changes(), changes);
    }

    #[test]
    fn provider_coordinate_repair_v1_rolls_back_on_canonical_coordinate_collision() {
        let connection = version_nine_connection();
        seed_legacy_provider_records(&connection);
        connection
            .execute(
                "INSERT INTO namespace_definitions(workspace_id, namespace, label, supported_grains, id_pattern, normalization, licence_posture, created_at) VALUES (?1, 'tmdb.movie', 'TMDB Movie', 'film', '^[1-9][0-9]*$', 'identity', 'identifiers_only', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("canonical namespace");
        connection
            .execute(
                "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES ('rec_existing', ?1, 'film', 'active', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("canonical record");
        connection
            .execute(
                "INSERT INTO external_identifiers(external_identifier_id, workspace_id, record_id, namespace, grain, value, created_at) VALUES ('xid_existing', ?1, 'rec_existing', 'tmdb.movie', 'film', '42', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("canonical identifier");

        let error = migrate(&connection).expect_err("coordinate conflict must fail closed");
        assert!(error
            .to_string()
            .contains("conflicts with a canonical coordinate"));
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, 9);
        let legacy: String = connection
            .query_row(
                "SELECT namespace FROM external_identifiers WHERE external_identifier_id = 'xid_movie'",
                [],
                |row| row.get(0),
            )
            .expect("legacy identifier preserved");
        assert_eq!(legacy, "tmdb");
    }

    #[test]
    fn provider_coordinate_repair_v1_rolls_back_on_incompatible_namespace() {
        let connection = version_nine_connection();
        seed_legacy_provider_records(&connection);
        connection
            .execute(
                "INSERT INTO namespace_definitions(workspace_id, namespace, label, supported_grains, id_pattern, normalization, licence_posture, created_at) VALUES (?1, 'tmdb.movie', 'Wrong TMDB label', 'film', '^[1-9][0-9]*$', 'identity', 'identifiers_only', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("incompatible canonical namespace");

        let error = migrate(&connection).expect_err("namespace conflict must fail closed");
        assert!(error
            .to_string()
            .contains("conflicts with an existing definition"));
        let state: (i64, String, i64) = connection
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), (SELECT namespace FROM external_identifiers WHERE external_identifier_id = 'xid_movie'), (SELECT COUNT(*) FROM namespace_definitions WHERE namespace IN ('googlebooks.volume', 'tmdb.movie', 'tmdb.tv'))",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("rollback state");
        assert_eq!(state, (9, "tmdb".to_owned(), 1));
    }

    #[test]
    fn provider_coordinate_repair_v1_rolls_back_on_mixed_google_books_identity() {
        let connection = version_nine_connection();
        seed_legacy_provider_records(&connection);
        connection
            .execute(
                "INSERT INTO namespace_definitions(workspace_id, namespace, label, supported_grains, id_pattern, normalization, licence_posture, created_at) VALUES (?1, 'other-book-id', 'Other book ID', 'chapter', '.+', 'identity', 'identifiers_only', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("other book namespace");
        connection
            .execute(
                "INSERT INTO external_identifiers(external_identifier_id, workspace_id, record_id, namespace, grain, value, created_at) VALUES ('xid_book_other', ?1, 'rec_book', 'other-book-id', 'chapter', 'other-1', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("mixed Google Books identity");

        let error = migrate(&connection).expect_err("mixed identity must fail closed");
        assert!(error.to_string().contains("mixed identity grains"));
        let state: (i64, String, String, i64) = connection
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), record.grain, identifier.namespace, (SELECT COUNT(*) FROM namespace_definitions WHERE namespace IN ('googlebooks.volume', 'tmdb.movie', 'tmdb.tv')) FROM records record JOIN external_identifiers identifier ON identifier.record_id = record.record_id WHERE identifier.external_identifier_id = 'xid_book'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("rollback state");
        assert_eq!(
            state,
            (9, "chapter".to_owned(), "google-books".to_owned(), 0)
        );
    }

    #[test]
    fn provider_coordinate_repair_v1_rolls_back_on_metadata_claim_collision() {
        let connection = version_nine_connection();
        seed_legacy_provider_records(&connection);
        connection
            .execute(
                "INSERT INTO metadata_field_claims(workspace_id, record_id, field_key, source, value, fetched_at, created_at) VALUES (?1, 'rec_movie', 'core.title', 'tmdb.movie', 'Existing title', ?2, ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("canonical metadata claim");

        let error = migrate(&connection).expect_err("claim conflict must fail closed");
        assert!(error
            .to_string()
            .contains("metadata conflicts with a canonical claim"));
        let state: (i64, String, i64, i64) = connection
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), (SELECT namespace FROM external_identifiers WHERE external_identifier_id = 'xid_movie'), (SELECT COUNT(*) FROM metadata_field_claims WHERE record_id = 'rec_movie'), (SELECT COUNT(*) FROM namespace_definitions WHERE namespace IN ('googlebooks.volume', 'tmdb.movie', 'tmdb.tv'))",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("rollback state");
        assert_eq!(state, (9, "tmdb".to_owned(), 2, 0));
    }

    #[test]
    fn provider_coordinate_repair_v1_rejects_malformed_values_atomically() {
        for malformed in ["not-a-number", "0", "00042", " 42 "] {
            let connection = version_nine_connection();
            seed_legacy_provider_records(&connection);
            connection
                .execute(
                    "UPDATE external_identifiers SET value = ?1 WHERE external_identifier_id = 'xid_movie'",
                    [malformed],
                )
                .expect("simulate malformed legacy TMDB value");

            let error = migrate(&connection).expect_err("malformed legacy value must fail closed");
            assert!(error
                .to_string()
                .contains("does not satisfy the canonical value pattern"));
            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("schema version");
            assert_eq!(version, 9);
            let legacy: (String, String) = connection
                .query_row(
                    "SELECT namespace, value FROM external_identifiers WHERE external_identifier_id = 'xid_movie'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("legacy identifier preserved");
            assert_eq!(legacy, ("tmdb".to_owned(), malformed.to_owned()));
            let canonical_definitions: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM namespace_definitions WHERE namespace IN ('googlebooks.volume', 'tmdb.movie', 'tmdb.tv')",
                    [],
                    |row| row.get(0),
                )
                .expect("canonical definitions rolled back");
            assert_eq!(canonical_definitions, 0);
        }
    }

    #[test]
    fn provider_coordinate_repair_v1_rolls_back_on_record_identifier_grain_mismatch() {
        let connection = version_nine_connection();
        seed_legacy_provider_records(&connection);
        connection
            .execute(
                "UPDATE records SET grain = 'series' WHERE record_id = 'rec_movie'",
                [],
            )
            .expect("simulate damaged legacy record grain");

        let error = migrate(&connection).expect_err("grain mismatch must fail closed");
        assert!(error
            .to_string()
            .contains("record and identifier grains disagree"));
        let state: (i64, String, String, String) = connection
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), record.grain, identifier.namespace, identifier.grain FROM records record JOIN external_identifiers identifier ON identifier.record_id = record.record_id WHERE record.record_id = 'rec_movie'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("legacy state preserved");
        assert_eq!(
            state,
            (9, "series".to_owned(), "tmdb".to_owned(), "film".to_owned())
        );
        let canonical_definitions: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM namespace_definitions WHERE namespace IN ('googlebooks.volume', 'tmdb.movie', 'tmdb.tv')",
                [],
                |row| row.get(0),
            )
            .expect("canonical definitions rolled back");
        assert_eq!(canonical_definitions, 0);
    }

    #[test]
    fn version_six_upgrade_backfills_the_full_node_owner_scope_set() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_v1(&connection).expect("version one");
        migrate_v2(&connection).expect("version two");
        migrate_v3(&connection).expect("version three");
        migrate_v4(&connection).expect("version four");
        migrate_v5(&connection).expect("version five");
        migrate_v6(&connection).expect("version six");
        seed_version_one_rows(&connection);
        connection
            .execute_batch(
                r#"
                INSERT INTO profile_grants(
                    grant_id, workspace_id, profile_id, client_id, status, created_at
                ) VALUES (
                    'grt_seed', 'wsp_seed', 'prf_seed', 'cli_seed', 'active',
                    '2026-08-24T00:00:05Z'
                );
                INSERT INTO node_state(
                    singleton, initialized, workspace_id, profile_id, client_id, created_at
                ) VALUES (
                    1, 1, 'wsp_seed', 'prf_seed', 'cli_seed',
                    '2026-08-24T00:00:06Z'
                );
                "#,
            )
            .expect("seed enrolled version-six node");
        for scope in FULL_ADMIN_SCOPES
            .iter()
            .filter(|scope| !V8_NODE_OWNER_SCOPE_BACKFILL.contains(scope))
        {
            connection
                .execute(
                    "INSERT INTO grant_scopes(grant_id, scope_key) VALUES ('grt_seed', ?1)",
                    [scope_storage_key(*scope)],
                )
                .expect("seed version-six scope");
        }

        migrate(&connection).expect("upgrade version-six database");

        let scopes: Vec<String> = connection
            .prepare(
                "SELECT scope_key FROM grant_scopes WHERE grant_id = 'grt_seed' ORDER BY scope_key",
            )
            .expect("prepare scope query")
            .query_map([], |row| row.get(0))
            .expect("query scopes")
            .collect::<Result<_, _>>()
            .expect("collect scopes");
        let mut expected: Vec<_> = FULL_ADMIN_SCOPES
            .iter()
            .map(|scope| scope_storage_key(*scope).to_owned())
            .collect();
        expected.sort_unstable();
        assert_eq!(scopes, expected);
    }

    #[test]
    fn browser_users_cannot_cross_workspace_client_and_profile_ownership() {
        let connection = migrated_connection();
        connection
            .execute_batch(
                r#"
                INSERT INTO workspaces(workspace_id, created_at)
                    VALUES ('wsp_browser_a', '2026-08-24T00:00:00Z');
                INSERT INTO workspaces(workspace_id, created_at)
                    VALUES ('wsp_browser_b', '2026-08-24T00:00:00Z');
                INSERT INTO profiles(profile_id, workspace_id, created_at)
                    VALUES ('prf_browser_a', 'wsp_browser_a', '2026-08-24T00:00:00Z');
                INSERT INTO profiles(profile_id, workspace_id, created_at)
                    VALUES ('prf_browser_b', 'wsp_browser_b', '2026-08-24T00:00:00Z');
                INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at)
                    VALUES ('cli_browser_a', 'wsp_browser_a', 'active', 1, '2026-08-24T00:00:00Z');
                "#,
            )
            .expect("seed browser ownership graph");
        let insert_user = |user_id: &str, profile_id: &str| {
            connection.execute(
                r#"
                INSERT INTO browser_users(
                    user_id, username, password_hash, client_id, profile_id,
                    is_admin, is_test_account, active, created_at, updated_at
                ) VALUES (?1, ?1, 'hash', 'cli_browser_a', ?2, 1, 1, 1, ?3, ?3)
                "#,
                params![user_id, profile_id, CREATED_AT],
            )
        };

        assert!(insert_user("usr_cross", "prf_browser_b").is_err());
        assert_eq!(
            insert_user("usr_same", "prf_browser_a").expect("same workspace"),
            1
        );
        assert!(connection
            .execute(
                "UPDATE browser_users SET profile_id = 'prf_browser_b' WHERE user_id = 'usr_same'",
                [],
            )
            .is_err());
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
        connection
            .execute_batch(
                r#"
                INSERT INTO records(record_id, workspace_id, grain, status, created_at)
                    VALUES ('rec_seed', 'wsp_seed', 'film', 'active',
                            '2026-08-24T00:00:05Z');
                INSERT INTO external_identifiers(
                    external_identifier_id, workspace_id, record_id, namespace,
                    grain, value, created_at
                ) VALUES (
                    'xid_seed', 'wsp_seed', 'rec_seed', 'imdb', 'film',
                    'tt0000001', '2026-08-24T00:00:06Z'
                );
                "#,
            )
            .expect("seed version one identifier");

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
            ("records", "record_id", "rec_seed"),
            ("external_identifiers", "external_identifier_id", "xid_seed"),
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
        assert!(
            connection
                .execute(
                    "UPDATE external_identifiers SET grain = 'film' WHERE external_identifier_id = 'xid_seed'",
                    [],
                )
                .is_err(),
            "a migrated identifier needs an explicit namespace registration before identity updates"
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

    #[test]
    fn version_four_upgrade_preserves_existing_schema_and_adds_later_tables() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_v1(&connection).expect("create version one database");
        migrate_v2(&connection).expect("upgrade to version two");
        migrate_v3(&connection).expect("upgrade to version three");
        migrate_v4(&connection).expect("upgrade to version four");
        connection
            .execute(
                "INSERT INTO node_state(singleton, initialized, created_at) VALUES (1, 0, ?1)",
                [CREATED_AT],
            )
            .expect("seed pre-recovery node state");

        let tables_before: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .expect("prepare table inventory")
            .query_map([], |row| row.get(0))
            .expect("query table inventory")
            .collect::<Result<_, _>>()
            .expect("collect table inventory");
        let columns_before: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('node_state') ORDER BY cid")
            .expect("prepare node-state columns")
            .query_map([], |row| row.get(0))
            .expect("query node-state columns")
            .collect::<Result<_, _>>()
            .expect("collect node-state columns");

        migrate(&connection).expect("upgrade version four database");

        let tables_after: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name")
            .expect("prepare upgraded table inventory")
            .query_map([], |row| row.get(0))
            .expect("query upgraded table inventory")
            .collect::<Result<_, _>>()
            .expect("collect upgraded table inventory");
        let columns_after: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('node_state') ORDER BY cid")
            .expect("prepare upgraded node-state columns")
            .query_map([], |row| row.get(0))
            .expect("query upgraded node-state columns")
            .collect::<Result<_, _>>()
            .expect("collect upgraded node-state columns");
        // Beyond v4, this connection also picks up migrate_v5's node_state
        // column, migrate_v6's two metadata tables, and migrate_v7's profile
        // tracking table, migrate_v8's browser authentication tables, and
        // migrate_v9's Nuvio Collections table -- all additive, so every v4
        // table and column remains.
        let expected_tables_after: Vec<String> = tables_before
            .iter()
            .cloned()
            .chain([
                "metadata_field_claims".to_owned(),
                "metadata_field_overrides".to_owned(),
                "profile_record_tracking_dispositions".to_owned(),
                "profile_nuvio_collections".to_owned(),
                "browser_auth_bootstrap".to_owned(),
                "browser_sessions".to_owned(),
                "browser_users".to_owned(),
            ])
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(tables_after, expected_tables_after);
        assert_eq!(
            columns_after,
            columns_before
                .into_iter()
                .chain(["recovery_restore_attempt_id".to_owned()])
                .collect::<Vec<_>>()
        );
        let marker: Option<String> = connection
            .query_row(
                "SELECT recovery_restore_attempt_id FROM node_state WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .expect("read nullable recovery marker");
        assert_eq!(marker, None);
    }

    #[test]
    fn database_rejects_undeclared_namespace_attachment_and_orphaning() {
        let connection = migrated_connection();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("workspace");
        connection
            .execute(
                "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES ('rec_namespace_guard', ?1, 'film', 'active', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("record");
        let insert = || {
            connection.execute(
                "INSERT INTO external_identifiers(external_identifier_id, workspace_id, record_id, namespace, grain, value, created_at) VALUES ('xid_namespace_guard', ?1, 'rec_namespace_guard', 'imdb', 'film', 'tt0000001', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
        };
        assert!(insert().is_err(), "undeclared namespace must fail");
        connection
            .execute(
                "INSERT INTO namespace_definitions(workspace_id, namespace, label, supported_grains, id_pattern, normalization, licence_posture, created_at) VALUES (?1, 'imdb', 'IMDb', 'series', '.+', 'identity', 'unknown', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("wrong-grain definition");
        assert!(insert().is_err(), "undeclared grain must fail");
        connection
            .execute(
                "UPDATE namespace_definitions SET supported_grains = 'film' WHERE workspace_id = ?1 AND namespace = 'imdb'",
                [WORKSPACE],
            )
            .expect("declare film grain");
        insert().expect("declared namespace and grain");
        assert!(connection
            .execute(
                "DELETE FROM namespace_definitions WHERE workspace_id = ?1 AND namespace = 'imdb'",
                [WORKSPACE],
            )
            .is_err());
        assert!(connection
            .execute(
                "UPDATE namespace_definitions SET supported_grains = 'series' WHERE workspace_id = ?1 AND namespace = 'imdb'",
                [WORKSPACE],
            )
            .is_err());
    }

    #[test]
    fn revision_tracks_identity_review_nested_and_equal_count_changes() {
        let connection = migrated_connection();
        seed_review_graph(&connection);
        let seeded = workspace_revision(&connection, WORKSPACE).expect("seed revision");

        connection
            .execute(
                "INSERT INTO external_identifiers(external_identifier_id, workspace_id, record_id, namespace, grain, value, created_at) VALUES ('xid_revision_test', ?1, 'rec_revision_test', 'imdb', 'film', 'tt0000001', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("identity insert");
        assert_eq!(
            workspace_revision(&connection, WORKSPACE).expect("identity revision"),
            seeded + 1
        );

        connection
            .execute(
                "UPDATE review_items SET status = 'deferred', updated_at = ?1 WHERE review_item_id = 'rev_revision_test'",
                [CREATED_AT],
            )
            .expect("review update");
        connection
            .execute(
                "UPDATE review_candidates SET record_id = record_id WHERE review_item_id = 'rev_revision_test'",
                [],
            )
            .expect("nested review-candidate update");
        assert_eq!(
            workspace_revision(&connection, WORKSPACE).expect("review revision"),
            seeded + 3
        );

        let before_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM external_identifiers", [], |row| {
                row.get(0)
            })
            .expect("identity count");
        connection
            .execute(
                "INSERT INTO external_identifiers(external_identifier_id, workspace_id, record_id, namespace, grain, value, created_at) VALUES ('xid_revision_replacement', ?1, 'rec_revision_test', 'tmdb', 'film', '1', ?2)",
                params![WORKSPACE, CREATED_AT],
            )
            .expect("replacement identity insert");
        connection
            .execute(
                "DELETE FROM external_identifiers WHERE external_identifier_id = 'xid_revision_test'",
                [],
            )
            .expect("original identity delete");
        let after_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM external_identifiers", [], |row| {
                row.get(0)
            })
            .expect("replacement identity count");
        assert_eq!(after_count, before_count);
        assert_eq!(
            workspace_revision(&connection, WORKSPACE).expect("replacement revision"),
            seeded + 5
        );
    }

    #[test]
    fn direct_nested_delete_tracks_the_parent_workspace() {
        let connection = migrated_connection();
        seed_review_graph(&connection);
        let before = workspace_revision(&connection, WORKSPACE).expect("revision before delete");

        connection
            .execute(
                "DELETE FROM review_candidates WHERE review_item_id = 'rev_revision_test'",
                [],
            )
            .expect("direct review candidate delete");

        assert_eq!(
            workspace_revision(&connection, WORKSPACE).expect("revision after delete"),
            before + 1
        );
    }

    #[test]
    fn deleting_review_item_cascades_candidates_and_advances_revision() {
        let connection = migrated_connection();
        seed_review_graph(&connection);
        let before = workspace_revision(&connection, WORKSPACE).expect("revision before cascade");

        connection
            .execute(
                "DELETE FROM review_items WHERE review_item_id = 'rev_revision_test'",
                [],
            )
            .expect("review item delete");

        let candidates: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM review_candidates WHERE review_item_id = 'rev_revision_test'",
                [],
                |row| row.get(0),
            )
            .expect("count review candidates");
        assert_eq!(candidates, 0);
        assert_eq!(
            workspace_revision(&connection, WORKSPACE).expect("revision after cascade"),
            before + 1
        );
    }

    #[test]
    fn deleting_observation_cascades_clues_and_advances_revision() {
        let connection = migrated_connection();
        seed_review_graph(&connection);
        connection
            .execute(
                "DELETE FROM review_items WHERE review_item_id = 'rev_revision_test'",
                [],
            )
            .expect("review item delete");
        connection
            .execute(
                "DELETE FROM interpretations WHERE interpretation_id = 'int_revision_test'",
                [],
            )
            .expect("interpretation delete");
        connection
            .execute(
                "DELETE FROM occurrences WHERE occurrence_id = 'occ_revision_test'",
                [],
            )
            .expect("occurrence delete");
        let before = workspace_revision(&connection, WORKSPACE).expect("revision before cascade");

        connection
            .execute(
                "DELETE FROM observations WHERE observation_id = 'obs_revision_test'",
                [],
            )
            .expect("observation delete");

        let clues: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM observation_clues WHERE observation_id = 'obs_revision_test'",
                [],
                |row| row.get(0),
            )
            .expect("count observation clues");
        assert_eq!(clues, 0);
        assert_eq!(
            workspace_revision(&connection, WORKSPACE).expect("revision after cascade"),
            before + 1
        );
    }

    #[test]
    fn revision_is_durable_in_a_frozen_database_copy() {
        let root = tempfile::tempdir().expect("temporary root");
        let source_path = root.path().join("source.sqlite3");
        let snapshot_path = root.path().join("snapshot.sqlite3");
        {
            let connection = Connection::open(&source_path).expect("source database");
            connection
                .pragma_update(None, "foreign_keys", "ON")
                .expect("enable foreign keys");
            migrate(&connection).expect("migrate source");
            connection
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![WORKSPACE, CREATED_AT],
                )
                .expect("workspace");
            assert_eq!(
                workspace_revision(&connection, WORKSPACE).expect("source revision"),
                1
            );
        }

        fs::copy(&source_path, &snapshot_path).expect("copy closed database snapshot");
        let snapshot = Connection::open(&snapshot_path).expect("snapshot database");
        assert_eq!(
            workspace_revision(&snapshot, WORKSPACE).expect("snapshot revision"),
            1
        );
    }
}
