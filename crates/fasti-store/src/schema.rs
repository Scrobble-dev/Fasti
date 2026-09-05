use crate::{
    access::{V11_NODE_OWNER_SCOPE_BACKFILL, V8_NODE_OWNER_SCOPE_BACKFILL},
    kernel::scope_storage_key,
};
use fasti_application::ScopeKey;
use fasti_domain::{Grain, MetadataClaimId, MAX_EXTERNAL_IDENTIFIER_BYTES};
use rusqlite::{Connection, Result, Transaction, TransactionBehavior};
use std::fmt::Write as _;

pub(crate) const SCHEMA_VERSION: i64 = 16;

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
    if version == 9 {
        migrate_v10(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 10 {
        migrate_v11(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 11 {
        migrate_v12(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 12 {
        migrate_v13(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 13 {
        migrate_v14(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 14 {
        migrate_v15(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == 15 {
        migrate_v16(connection)?;
    }

    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version == SCHEMA_VERSION {
        let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
        repair_legacy_provider_coordinates_v1(&transaction)?;
        transaction.commit()?;
    }
    Ok(())
}

fn migrate_v16(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    transaction.execute_batch(r#"
        CREATE INDEX metadata_claim_provenance_recent_idx
            ON metadata_claim_provenance(workspace_id, record_id, field_key, fetched_at DESC, source DESC);
        CREATE TABLE local_search_grams (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_partition TEXT NOT NULL,
            gram TEXT NOT NULL CHECK (length(gram) BETWEEN 1 AND 3),
            record_id TEXT NOT NULL REFERENCES records(record_id) ON DELETE CASCADE,
            PRIMARY KEY (workspace_id, profile_partition, gram, record_id)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX local_search_grams_record_idx
            ON local_search_grams(workspace_id, profile_partition, record_id, gram);
        CREATE TRIGGER local_search_grams_scope_insert BEFORE INSERT ON local_search_grams
        WHEN NOT EXISTS (SELECT 1 FROM records WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id)
          OR (NEW.profile_partition <> '' AND NOT EXISTS (SELECT 1 FROM profiles WHERE profile_id = NEW.profile_partition AND workspace_id = NEW.workspace_id))
        BEGIN SELECT RAISE(ABORT, 'invalid local search scope'); END;
        CREATE TRIGGER local_search_grams_no_update BEFORE UPDATE ON local_search_grams
        BEGIN SELECT RAISE(ABORT, 'rebuild local search postings instead of updating scope'); END;
        ALTER TABLE provider_capability_states ADD COLUMN authority_version INTEGER NOT NULL DEFAULT 1 CHECK (authority_version >= 1);
        UPDATE provider_capability_states SET authority_version = capability_version;
        CREATE TRIGGER provider_search_authority_changed AFTER UPDATE ON provider_capability_states
        WHEN OLD.credential_reference IS NOT NEW.credential_reference
          OR OLD.credential_requirement IS NOT NEW.credential_requirement
          OR OLD.credential_status IS NOT NEW.credential_status
          OR OLD.configuration_digest IS NOT NEW.configuration_digest
          OR (OLD.capability_status IS NOT NEW.capability_status AND (
              OLD.capability_status NOT IN ('available', 'degraded')
              OR NEW.capability_status NOT IN ('available', 'degraded')))
          OR (OLD.capability_version IS NOT NEW.capability_version
              AND OLD.capability_status IS NEW.capability_status
              AND OLD.health_status IS NEW.health_status
              AND OLD.health_checked_at IS NEW.health_checked_at
              AND OLD.health_problem_code IS NEW.health_problem_code
              AND OLD.credential_test_status IS NEW.credential_test_status
              AND OLD.credential_test_checked_at IS NEW.credential_test_checked_at
              AND OLD.credential_test_problem_code IS NEW.credential_test_problem_code)
        BEGIN
            UPDATE provider_capability_states SET authority_version = OLD.authority_version + 1
            WHERE workspace_id = NEW.workspace_id AND provider_id = NEW.provider_id AND capability_id = NEW.capability_id;
        END;
        CREATE TABLE search_pages (
            sequence INTEGER PRIMARY KEY AUTOINCREMENT,
            context_json TEXT NOT NULL CHECK (
                length(CAST(context_json AS BLOB)) <= 2048
                AND json_valid(context_json) AND json_type(context_json) = 'object'
            ),
            partition_json TEXT NOT NULL CHECK (
                length(CAST(partition_json AS BLOB)) <= 4096
                AND json_valid(partition_json) AND json_type(partition_json) = 'object'
            ),
            partition_digest TEXT NOT NULL CHECK (
                length(partition_digest) = 71 AND substr(partition_digest, 1, 7) = 'sha256:'
                AND substr(partition_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            actor_client_id TEXT NOT NULL REFERENCES clients(client_id),
            actor_subject_id TEXT REFERENCES auth_subjects(auth_subject_id),
            grant_id TEXT NOT NULL REFERENCES profile_grants(grant_id),
            provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
            upstream_page INTEGER NOT NULL CHECK (upstream_page BETWEEN 1 AND 4294967295),
            next_page INTEGER CHECK (next_page > upstream_page AND next_page <= 4294967295),
            candidate_count INTEGER NOT NULL CHECK (candidate_count BETWEEN 0 AND 100),
            candidate_bytes INTEGER NOT NULL CHECK (candidate_bytes BETWEEN 0 AND 6553600),
            response_digest TEXT NOT NULL CHECK (
                length(response_digest) = 71 AND substr(response_digest, 1, 7) = 'sha256:'
                AND substr(response_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            created_at TEXT NOT NULL,
            fresh_until TEXT NOT NULL CHECK (fresh_until >= created_at),
            stale_until TEXT NOT NULL CHECK (stale_until >= fresh_until),
            expires_at TEXT NOT NULL CHECK (expires_at >= stale_until)
        ) STRICT;
        CREATE INDEX search_pages_lookup_idx ON search_pages(partition_digest, sequence DESC);
        CREATE INDEX search_pages_expiry_idx ON search_pages(expires_at, sequence);
        CREATE TABLE search_candidate_receipts (
            candidate_receipt_id TEXT PRIMARY KEY NOT NULL,
            page_sequence INTEGER NOT NULL REFERENCES search_pages(sequence) ON DELETE CASCADE,
            ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 99),
            kind TEXT NOT NULL,
            provider_record_id TEXT NOT NULL,
            candidate_json TEXT NOT NULL CHECK (
                length(CAST(candidate_json AS BLOB)) <= 65536
                AND json_valid(candidate_json) AND json_type(candidate_json) = 'object'
            ),
            UNIQUE (page_sequence, ordinal),
            UNIQUE (page_sequence, kind, provider_record_id),
            CHECK (json_extract(candidate_json, '$.kind') IS kind),
            CHECK (json_extract(candidate_json, '$.provider_id') IS provider_record_id)
        ) STRICT;
        CREATE TRIGGER search_pages_scope_insert BEFORE INSERT ON search_pages
        WHEN NOT EXISTS (
            SELECT 1 FROM profile_grants g
            JOIN profiles p ON p.profile_id = g.profile_id AND p.workspace_id = g.workspace_id
            JOIN clients c ON c.client_id = g.client_id AND c.workspace_id = g.workspace_id
            WHERE g.grant_id = NEW.grant_id AND g.workspace_id = NEW.workspace_id
              AND g.profile_id = NEW.profile_id AND g.client_id = NEW.actor_client_id
              AND (NEW.actor_subject_id IS NULL OR EXISTS (
                SELECT 1 FROM auth_subject_profile_grants s
                WHERE s.auth_subject_id = NEW.actor_subject_id AND s.profile_grant_id = g.grant_id
              ))
        ) OR json_extract(NEW.partition_json, '$.workspace_id') IS NOT NEW.workspace_id
          OR json_extract(NEW.partition_json, '$.profile_id') IS NOT NEW.profile_id
          OR json_extract(NEW.partition_json, '$.actor_client_id') IS NOT NEW.actor_client_id
          OR json_extract(NEW.partition_json, '$.actor_subject_id') IS NOT NEW.actor_subject_id
          OR json_extract(NEW.partition_json, '$.grant_id') IS NOT NEW.grant_id
          OR json_extract(NEW.context_json, '$.provider') IS NOT NEW.provider_id
          OR json_extract(NEW.context_json, '$.page') IS NOT NEW.upstream_page
        BEGIN SELECT RAISE(ABORT, 'invalid search page scope'); END;
        CREATE TRIGGER search_candidates_parent_insert BEFORE INSERT ON search_candidate_receipts
        WHEN NOT EXISTS (
            SELECT 1 FROM search_pages p WHERE p.sequence = NEW.page_sequence
              AND NEW.ordinal < p.candidate_count
              AND json_extract(NEW.candidate_json, '$.provider') IS p.provider_id
        )
        BEGIN SELECT RAISE(ABORT, 'invalid search candidate parent'); END;
        CREATE TRIGGER search_pages_immutable_update BEFORE UPDATE ON search_pages
        BEGIN SELECT RAISE(ABORT, 'search pages are immutable'); END;
        CREATE TRIGGER search_candidates_immutable_update BEFORE UPDATE ON search_candidate_receipts
        BEGIN SELECT RAISE(ABORT, 'search candidate receipts are immutable'); END;
        CREATE TABLE search_action_receipts (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            operation_id TEXT NOT NULL,
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            actor_client_id TEXT NOT NULL REFERENCES clients(client_id),
            actor_subject_id TEXT CHECK (actor_subject_id IS NULL OR (
                length(actor_subject_id) = 36 AND substr(actor_subject_id, 1, 4) = 'sub_'
                AND substr(actor_subject_id, 5) NOT GLOB '*[^0-9a-f]*'
                AND substr(actor_subject_id, 17, 1) = '7' AND substr(actor_subject_id, 21, 1) GLOB '[89ab]'
            )),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            semantic_digest TEXT NOT NULL CHECK (
                length(semantic_digest) = 71 AND substr(semantic_digest, 1, 7) = 'sha256:'
                AND substr(semantic_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            receipt_json TEXT NOT NULL CHECK (
                length(CAST(receipt_json AS BLOB)) <= 16384
                AND json_valid(receipt_json) AND json_type(receipt_json) = 'object'
            ),
            PRIMARY KEY (workspace_id, operation_id)
        ) STRICT, WITHOUT ROWID;
        CREATE TRIGGER search_actions_scope_insert BEFORE INSERT ON search_action_receipts
        WHEN NOT EXISTS (SELECT 1 FROM profiles WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id)
          OR NOT EXISTS (SELECT 1 FROM clients WHERE client_id = NEW.actor_client_id AND workspace_id = NEW.workspace_id)
          OR NOT EXISTS (SELECT 1 FROM records WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id)
          OR json_extract(NEW.receipt_json, '$.workspace_id') IS NOT NEW.workspace_id
          OR json_extract(NEW.receipt_json, '$.operation_id') IS NOT NEW.operation_id
          OR json_extract(NEW.receipt_json, '$.profile_id') IS NOT NEW.profile_id
          OR json_extract(NEW.receipt_json, '$.actor_client_id') IS NOT NEW.actor_client_id
          OR json_extract(NEW.receipt_json, '$.actor_subject_id') IS NOT NEW.actor_subject_id
          OR json_extract(NEW.receipt_json, '$.record_id') IS NOT NEW.record_id
        BEGIN SELECT RAISE(ABORT, 'invalid search action scope'); END;
        CREATE TRIGGER search_actions_immutable_update BEFORE UPDATE ON search_action_receipts
        BEGIN SELECT RAISE(ABORT, 'search action receipts are immutable'); END;
        CREATE TRIGGER search_actions_immutable_delete BEFORE DELETE ON search_action_receipts
        BEGIN SELECT RAISE(ABORT, 'search action receipts are immutable'); END;
    "#)?;
    let mut revision_sql = String::new();
    append_revision_triggers(
        &mut revision_sql,
        &RevisionSource {
            table: "search_action_receipts",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
    );
    transaction.execute_batch(&revision_sql)?;
    crate::local_search::rebuild(&transaction)?;
    // C1's first human administrator links this same node-owner grant. Do not
    // expand delegated grants or provisional enrollment/recovery authority.
    transaction.execute(
        "INSERT OR IGNORE INTO grant_scopes(grant_id, scope_key)
         SELECT pg.grant_id, ?1 FROM node_state ns
         JOIN profile_grants pg ON pg.workspace_id = ns.workspace_id
           AND pg.profile_id = ns.profile_id AND pg.client_id = ns.client_id
         JOIN clients c ON c.client_id = pg.client_id AND c.workspace_id = pg.workspace_id
         WHERE ns.singleton = 1 AND ns.initialized = 1
           AND ns.initialization_consumed_at IS NOT NULL
           AND ns.recovery_restore_attempt_id IS NULL
           AND pg.status = 'active' AND c.status = 'active'",
        [scope_storage_key(ScopeKey::MetadataSearch)],
    )?;
    transaction.pragma_update(None, "user_version", 16)?;
    transaction.commit()
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

fn migrate_v10(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        -- Remove the PR-only simulated identity and factor state. These tables
        -- never represented a supported user population or compatibility
        -- boundary. IF EXISTS also repairs developer roots that ran the
        -- previously edited v8 migration.
        DROP TABLE IF EXISTS auth_ephemeral_challenges;
        DROP TABLE IF EXISTS oidc_provider_configs;
        DROP TABLE IF EXISTS user_backup_codes;
        DROP TABLE IF EXISTS user_passkeys;
        DROP TABLE IF EXISTS user_totp;
        DROP TABLE IF EXISTS browser_sessions;
        DROP TABLE IF EXISTS browser_auth_bootstrap;
        DROP TABLE IF EXISTS browser_users;

        CREATE TABLE auth_subjects (
            auth_subject_id TEXT PRIMARY KEY,
            lifecycle TEXT NOT NULL
                CHECK (lifecycle IN ('active', 'disabled', 'deleted', 'recovery_pending')),
            auth_epoch INTEGER NOT NULL CHECK (auth_epoch >= 0),
            authorization_epoch INTEGER NOT NULL CHECK (authorization_epoch >= 0),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        ) STRICT;

        CREATE TABLE auth_subject_profile_grants (
            auth_subject_id TEXT NOT NULL
                REFERENCES auth_subjects(auth_subject_id) ON DELETE CASCADE,
            profile_grant_id TEXT NOT NULL
                REFERENCES profile_grants(grant_id) ON DELETE CASCADE,
            PRIMARY KEY (auth_subject_id, profile_grant_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE fasti_browser_sessions (
            browser_session_id TEXT PRIMARY KEY,
            session_digest TEXT NOT NULL UNIQUE,
            csrf_digest TEXT NOT NULL,
            auth_subject_id TEXT NOT NULL
                REFERENCES auth_subjects(auth_subject_id) ON DELETE CASCADE,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            selected_profile_grant_id TEXT NOT NULL
                REFERENCES profile_grants(grant_id),
            created_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL,
            idle_expires_at TEXT NOT NULL,
            absolute_expires_at TEXT NOT NULL,
            idle_timeout_seconds INTEGER NOT NULL CHECK (idle_timeout_seconds > 0),
            last_seen_write_interval_seconds INTEGER NOT NULL
                CHECK (last_seen_write_interval_seconds > 0),
            revoked_at TEXT,
            auth_epoch INTEGER NOT NULL CHECK (auth_epoch >= 0),
            authorization_epoch INTEGER NOT NULL CHECK (authorization_epoch >= 0),
            rotation_generation INTEGER NOT NULL CHECK (rotation_generation >= 0),
            CHECK (last_seen_at >= created_at),
            CHECK (idle_expires_at <= absolute_expires_at)
        ) STRICT;
        CREATE INDEX fasti_browser_sessions_subject_idx
            ON fasti_browser_sessions(auth_subject_id, revoked_at, absolute_expires_at);

        CREATE TABLE fasti_browser_session_grants (
            browser_session_id TEXT NOT NULL
                REFERENCES fasti_browser_sessions(browser_session_id) ON DELETE CASCADE,
            profile_grant_id TEXT NOT NULL REFERENCES profile_grants(grant_id),
            PRIMARY KEY (browser_session_id, profile_grant_id)
        ) STRICT, WITHOUT ROWID;
        "#,
    )?;
    transaction.pragma_update(None, "user_version", 10)?;
    transaction.commit()
}

fn migrate_v11(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        CREATE TABLE provider_capability_states (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            provider_id TEXT NOT NULL
                CHECK (
                    length(provider_id) BETWEEN 1 AND 128
                    AND provider_id NOT GLOB '*[^a-z0-9._:/-]*'
                ),
            capability_id TEXT NOT NULL
                CHECK (
                    length(capability_id) BETWEEN 1 AND 128
                    AND capability_id NOT GLOB '*[^a-z0-9._:/-]*'
                ),
            capability_status TEXT NOT NULL
                CHECK (capability_status IN (
                    'available', 'degraded', 'unavailable', 'disabled'
                )),
            capability_version INTEGER NOT NULL CHECK (capability_version >= 1),
            credential_requirement TEXT NOT NULL
                CHECK (credential_requirement IN (
                    'none', 'optional_api_key', 'api_key', 'bearer_token',
                    'basic_auth', 'oauth2', 'user_agent_only', 'custom_header',
                    'operator_secret_mount'
                )),
            credential_reference TEXT
                CHECK (
                    credential_reference IS NULL OR (
                        length(credential_reference) BETWEEN 1 AND 253
                        AND credential_reference NOT GLOB '*[^a-z0-9._:/-]*'
                    )
                ),
            credential_status TEXT NOT NULL
                CHECK (credential_status IN (
                    'not_required', 'optional', 'missing', 'stored_unverified',
                    'valid', 'invalid', 'expired', 'unavailable', 'revoked'
                )),
            configuration_digest TEXT NOT NULL
                CHECK (
                    length(configuration_digest) = 64
                    AND configuration_digest NOT GLOB '*[^0-9a-f]*'
                ),
            health_status TEXT NOT NULL
                CHECK (health_status IN ('never_run', 'passed', 'failed', 'unavailable')),
            health_checked_at TEXT,
            health_problem_code TEXT,
            credential_test_status TEXT NOT NULL
                CHECK (credential_test_status IN ('never_run', 'passed', 'failed', 'unavailable')),
            credential_test_checked_at TEXT,
            credential_test_problem_code TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, provider_id, capability_id),
            CHECK (
                (credential_requirement IN ('none', 'user_agent_only')
                    AND credential_reference IS NULL
                    AND credential_status = 'not_required')
                OR
                (credential_requirement = 'optional_api_key'
                    AND credential_reference IS NULL
                    AND credential_status = 'optional')
                OR
                (credential_requirement NOT IN ('none', 'user_agent_only', 'optional_api_key')
                    AND credential_reference IS NULL
                    AND credential_status = 'missing')
                OR
                (credential_requirement NOT IN ('none', 'user_agent_only')
                    AND credential_reference IS NOT NULL
                    AND credential_status IN (
                        'stored_unverified', 'valid', 'invalid', 'expired',
                        'unavailable', 'revoked'
                    ))
            ),
            CHECK (
                (health_status = 'never_run'
                    AND health_checked_at IS NULL
                    AND health_problem_code IS NULL)
                OR
                (health_status = 'passed'
                    AND health_checked_at IS NOT NULL
                    AND health_problem_code IS NULL)
                OR
                (health_status IN ('failed', 'unavailable')
                    AND health_checked_at IS NOT NULL)
            ),
            CHECK (
                (credential_test_status = 'never_run'
                    AND credential_test_checked_at IS NULL
                    AND credential_test_problem_code IS NULL)
                OR
                (credential_test_status = 'passed'
                    AND credential_test_checked_at IS NOT NULL
                    AND credential_test_problem_code IS NULL)
                OR
                (credential_test_status IN ('failed', 'unavailable')
                    AND credential_test_checked_at IS NOT NULL)
            )
        ) STRICT, WITHOUT ROWID;
        "#,
    )?;
    let mut revision_sql = String::new();
    append_revision_triggers(
        &mut revision_sql,
        &RevisionSource {
            table: "provider_capability_states",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
    );
    transaction.execute_batch(&revision_sql)?;
    for scope in V11_NODE_OWNER_SCOPE_BACKFILL {
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
    transaction.pragma_update(None, "user_version", 11)?;
    transaction.commit()
}

fn migrate_v12(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;

    // Repair legacy provider coordinates before companion rows take a foreign
    // key to the frozen v6 claim identity.
    repair_legacy_provider_coordinates_v1(&transaction)?;
    transaction.execute_batch(
        r#"
        CREATE TABLE metadata_claims (
            claim_id TEXT PRIMARY KEY
                CHECK (
                    length(claim_id) = 36
                    AND substr(claim_id, 1, 4) = 'mcl_'
                    AND substr(claim_id, 5) NOT GLOB '*[^0-9a-f]*'
                ),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            claim_kind TEXT NOT NULL CHECK (claim_kind IN ('field', 'rating')),
            created_at TEXT NOT NULL
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_claims_record_idx
            ON metadata_claims(workspace_id, record_id, claim_kind, claim_id);

        CREATE TRIGGER metadata_claims_scope_insert
        BEFORE INSERT ON metadata_claims
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_claims_scope_update
        BEFORE UPDATE ON metadata_claims
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_claims_immutable_update
        BEFORE UPDATE ON metadata_claims
        BEGIN
            SELECT RAISE(ABORT, 'metadata claims are immutable');
        END;

        CREATE TRIGGER metadata_claims_immutable_delete
        BEFORE DELETE ON metadata_claims
        BEGIN
            SELECT RAISE(ABORT, 'metadata claims are immutable');
        END;

        CREATE TRIGGER metadata_field_claims_immutable_update
        BEFORE UPDATE ON metadata_field_claims
        WHEN EXISTS (
            SELECT 1 FROM metadata_claim_provenance provenance
            WHERE provenance.record_id = OLD.record_id
              AND provenance.field_key = OLD.field_key
              AND provenance.source = OLD.source
              AND provenance.fetched_at = OLD.fetched_at
        ) OR NOT (
            (
                (OLD.source = 'google-books' AND NEW.source = 'googlebooks.volume')
                OR (OLD.source = 'tmdb' AND NEW.source IN ('tmdb.movie', 'tmdb.tv'))
            )
            AND NEW.workspace_id IS OLD.workspace_id
            AND NEW.record_id IS OLD.record_id
            AND NEW.field_key IS OLD.field_key
            AND NEW.value IS OLD.value
            AND NEW.locale IS OLD.locale
            AND NEW.fetched_at IS OLD.fetched_at
            AND NEW.expires_at IS OLD.expires_at
            AND NEW.created_at IS OLD.created_at
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata field claims are immutable');
        END;

        CREATE TRIGGER metadata_field_claims_immutable_delete
        BEFORE DELETE ON metadata_field_claims
        BEGIN
            SELECT RAISE(ABORT, 'metadata field claims are immutable');
        END;

        CREATE TABLE metadata_claim_provenance (
            claim_id TEXT PRIMARY KEY REFERENCES metadata_claims(claim_id),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL,
            field_key TEXT NOT NULL,
            source TEXT NOT NULL,
            fetched_at TEXT NOT NULL,
            provider_id TEXT CHECK (
                provider_id IS NULL OR length(provider_id) BETWEEN 1 AND 128
            ),
            source_record_id TEXT CHECK (
                source_record_id IS NULL OR length(source_record_id) BETWEEN 1 AND 512
            ),
            region TEXT CHECK (region IS NULL OR length(region) BETWEEN 2 AND 8),
            source_version TEXT CHECK (
                source_version IS NULL OR length(source_version) BETWEEN 1 AND 128
            ),
            evidence_digest TEXT CHECK (
                evidence_digest IS NULL OR (
                    length(evidence_digest) = 71
                    AND substr(evidence_digest, 1, 7) = 'sha256:'
                    AND substr(evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'
                )
            ),
            classification TEXT NOT NULL DEFAULT 'internal'
                CHECK (classification IN ('public', 'internal', 'confidential', 'restricted')),
            terms_revision TEXT CHECK (
                terms_revision IS NULL OR length(terms_revision) BETWEEN 1 AND 128
            ),
            provenance_state TEXT NOT NULL
                CHECK (provenance_state IN ('complete', 'legacy_incomplete')),
            initial_status TEXT NOT NULL CHECK (initial_status IN (
                'fresh', 'stale', 'invalid', 'revoked', 'superseded', 'unavailable'
            )),
            created_at TEXT NOT NULL,
            UNIQUE(record_id, field_key, source, fetched_at),
            FOREIGN KEY(record_id, field_key, source, fetched_at)
                REFERENCES metadata_field_claims(record_id, field_key, source, fetched_at)
                ON UPDATE CASCADE ON DELETE RESTRICT,
            CHECK (
                (provenance_state = 'complete'
                    AND provider_id IS NOT NULL
                    AND source_record_id IS NOT NULL
                    AND evidence_digest IS NOT NULL)
                OR
                (provenance_state = 'legacy_incomplete'
                    AND provider_id IS NULL
                    AND source_record_id IS NULL
                    AND evidence_digest IS NULL)
            )
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_claim_provenance_record_field_idx
            ON metadata_claim_provenance(workspace_id, record_id, field_key, fetched_at DESC);

        CREATE TRIGGER metadata_claim_provenance_scope_insert
        BEFORE INSERT ON metadata_claim_provenance
        WHEN NOT EXISTS (
            SELECT 1 FROM metadata_claims registered
            WHERE registered.claim_id = NEW.claim_id
              AND registered.workspace_id = NEW.workspace_id
              AND registered.record_id = NEW.record_id
              AND registered.claim_kind = 'field'
        ) OR NOT EXISTS (
            SELECT 1 FROM metadata_field_claims claim
            WHERE claim.workspace_id = NEW.workspace_id
              AND claim.record_id = NEW.record_id
              AND claim.field_key = NEW.field_key
              AND claim.source = NEW.source
              AND claim.fetched_at = NEW.fetched_at
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim provenance crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_claim_provenance_immutable_update
        BEFORE UPDATE ON metadata_claim_provenance
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim provenance is immutable');
        END;

        CREATE TRIGGER metadata_claim_provenance_immutable_delete
        BEFORE DELETE ON metadata_claim_provenance
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim provenance is immutable');
        END;

        CREATE TRIGGER metadata_claim_provenance_scope_update
        BEFORE UPDATE ON metadata_claim_provenance
        WHEN NOT EXISTS (
            SELECT 1 FROM metadata_claims registered
            WHERE registered.claim_id = NEW.claim_id
              AND registered.workspace_id = NEW.workspace_id
              AND registered.record_id = NEW.record_id
              AND registered.claim_kind = 'field'
        ) OR NOT EXISTS (
            SELECT 1 FROM metadata_field_claims claim
            WHERE claim.workspace_id = NEW.workspace_id
              AND claim.record_id = NEW.record_id
              AND claim.field_key = NEW.field_key
              AND claim.source = NEW.source
              AND claim.fetched_at = NEW.fetched_at
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim provenance crosses a workspace boundary');
        END;

        CREATE TABLE metadata_rating_claims (
            claim_id TEXT PRIMARY KEY REFERENCES metadata_claims(claim_id),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            value_millis INTEGER NOT NULL CHECK (value_millis BETWEEN 0 AND 1000000),
            scale_minimum_millis INTEGER NOT NULL
                CHECK (scale_minimum_millis BETWEEN 0 AND 999999),
            scale_maximum_millis INTEGER NOT NULL
                CHECK (scale_maximum_millis BETWEEN 1 AND 1000000),
            provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
            source TEXT NOT NULL,
            source_record_id TEXT NOT NULL CHECK (length(source_record_id) BETWEEN 1 AND 512),
            locale TEXT CHECK (locale IS NULL OR length(locale) BETWEEN 2 AND 16),
            region TEXT CHECK (region IS NULL OR length(region) BETWEEN 2 AND 8),
            source_version TEXT CHECK (
                source_version IS NULL OR length(source_version) BETWEEN 1 AND 128
            ),
            evidence_digest TEXT NOT NULL CHECK (
                length(evidence_digest) = 71
                AND substr(evidence_digest, 1, 7) = 'sha256:'
                AND substr(evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            classification TEXT NOT NULL DEFAULT 'internal'
                CHECK (classification IN ('public', 'internal', 'confidential', 'restricted')),
            terms_revision TEXT CHECK (
                terms_revision IS NULL OR length(terms_revision) BETWEEN 1 AND 128
            ),
            fetched_at TEXT NOT NULL,
            expires_at TEXT,
            initial_status TEXT NOT NULL CHECK (initial_status IN (
                'fresh', 'stale', 'invalid', 'revoked', 'superseded', 'unavailable'
            )),
            created_at TEXT NOT NULL,
            CHECK (scale_minimum_millis < scale_maximum_millis),
            CHECK (value_millis BETWEEN scale_minimum_millis AND scale_maximum_millis),
            UNIQUE(record_id, provider_id, source_record_id, fetched_at)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_rating_claims_record_idx
            ON metadata_rating_claims(workspace_id, record_id, fetched_at DESC, claim_id);

        CREATE TRIGGER metadata_rating_claims_scope_insert
        BEFORE INSERT ON metadata_rating_claims
        WHEN NOT EXISTS (
            SELECT 1 FROM metadata_claims registered
            WHERE registered.claim_id = NEW.claim_id
              AND registered.workspace_id = NEW.workspace_id
              AND registered.record_id = NEW.record_id
              AND registered.claim_kind = 'rating'
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata rating claim crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_rating_claims_scope_update
        BEFORE UPDATE ON metadata_rating_claims
        WHEN NOT EXISTS (
            SELECT 1 FROM metadata_claims registered
            WHERE registered.claim_id = NEW.claim_id
              AND registered.workspace_id = NEW.workspace_id
              AND registered.record_id = NEW.record_id
              AND registered.claim_kind = 'rating'
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata rating claim crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_rating_claims_immutable_update
        BEFORE UPDATE ON metadata_rating_claims
        BEGIN
            SELECT RAISE(ABORT, 'metadata rating claims are immutable');
        END;

        CREATE TRIGGER metadata_rating_claims_immutable_delete
        BEFORE DELETE ON metadata_rating_claims
        BEGIN
            SELECT RAISE(ABORT, 'metadata rating claims are immutable');
        END;

        CREATE TABLE metadata_claim_lifecycle_events (
            claim_id TEXT NOT NULL REFERENCES metadata_claims(claim_id),
            sequence INTEGER NOT NULL CHECK (sequence >= 1),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            previous_status TEXT NOT NULL CHECK (previous_status IN (
                'fresh', 'stale', 'invalid', 'revoked', 'superseded', 'unavailable'
            )),
            status TEXT NOT NULL CHECK (status IN (
                'fresh', 'stale', 'invalid', 'revoked', 'superseded', 'unavailable'
            )),
            occurred_at TEXT NOT NULL,
            evidence_digest TEXT CHECK (
                evidence_digest IS NULL OR (
                    length(evidence_digest) = 71
                    AND substr(evidence_digest, 1, 7) = 'sha256:'
                    AND substr(evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'
                )
            ),
            PRIMARY KEY (claim_id, sequence)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_claim_lifecycle_workspace_idx
            ON metadata_claim_lifecycle_events(workspace_id, claim_id, sequence DESC);

        CREATE TRIGGER metadata_claim_lifecycle_scope_insert
        BEFORE INSERT ON metadata_claim_lifecycle_events
        WHEN NOT EXISTS (
            SELECT 1 FROM metadata_claims registered
            WHERE registered.claim_id = NEW.claim_id
              AND registered.workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim lifecycle crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_claim_lifecycle_append_only_update
        BEFORE UPDATE ON metadata_claim_lifecycle_events
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim lifecycle is append-only');
        END;

        CREATE TRIGGER metadata_claim_lifecycle_append_only_delete
        BEFORE DELETE ON metadata_claim_lifecycle_events
        BEGIN
            SELECT RAISE(ABORT, 'metadata claim lifecycle is append-only');
        END;

        CREATE TABLE metadata_projection_policies (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            preferred_provider_id TEXT CHECK (
                preferred_provider_id IS NULL OR length(preferred_provider_id) BETWEEN 1 AND 128
            ),
            preferred_locale TEXT CHECK (
                preferred_locale IS NULL OR length(preferred_locale) BETWEEN 2 AND 16
            ),
            original_locale TEXT CHECK (
                original_locale IS NULL OR length(original_locale) BETWEEN 2 AND 16
            ),
            region TEXT CHECK (region IS NULL OR length(region) BETWEEN 2 AND 8),
            enabled_field_groups TEXT NOT NULL DEFAULT '[]'
                CHECK (length(enabled_field_groups) BETWEEN 2 AND 1024),
            allow_english_fallback INTEGER NOT NULL
                CHECK (allow_english_fallback IN (0, 1)),
            last_known_good_policy TEXT NOT NULL CHECK (
                last_known_good_policy IN ('allow', 'deny')
            ),
            updated_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, profile_id)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_projection_policies_profile_idx
            ON metadata_projection_policies(workspace_id, profile_id);

        CREATE TRIGGER metadata_projection_policies_scope_insert
        BEFORE INSERT ON metadata_projection_policies
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata projection policy crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_projection_policies_scope_update
        BEFORE UPDATE ON metadata_projection_policies
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata projection policy crosses a workspace boundary');
        END;

        CREATE TABLE metadata_profile_field_overrides (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            field_key TEXT NOT NULL,
            value TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            origin TEXT NOT NULL CHECK (origin IN ('user', 'legacy_migration')),
            PRIMARY KEY (workspace_id, profile_id, record_id, field_key)
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_profile_field_overrides_record_idx
            ON metadata_profile_field_overrides(workspace_id, profile_id, record_id, field_key);

        CREATE TRIGGER metadata_profile_field_overrides_scope_insert
        BEFORE INSERT ON metadata_profile_field_overrides
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'profile metadata override crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_profile_field_overrides_scope_update
        BEFORE UPDATE ON metadata_profile_field_overrides
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'profile metadata override crosses a workspace boundary');
        END;

        -- The v6 table remains byte-for-byte intact for archive-v2. These
        -- rows record whether its owner was unambiguous; they never copy an
        -- override to multiple profiles or choose one arbitrarily.
        CREATE TABLE metadata_legacy_override_ownership (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL,
            field_key TEXT NOT NULL,
            owner_profile_id TEXT REFERENCES profiles(profile_id),
            state TEXT NOT NULL CHECK (state IN ('migrated', 'review_required')),
            review_reason TEXT CHECK (
                review_reason IS NULL OR review_reason IN ('zero_profiles', 'multiple_profiles')
            ),
            recorded_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, record_id, field_key),
            FOREIGN KEY(record_id, field_key)
                REFERENCES metadata_field_overrides(record_id, field_key) ON DELETE RESTRICT,
            CHECK (
                (state = 'migrated' AND owner_profile_id IS NOT NULL AND review_reason IS NULL)
                OR
                (state = 'review_required' AND owner_profile_id IS NULL AND review_reason IS NOT NULL)
            )
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE metadata_override_migration_receipts (
            receipt_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL,
            field_key TEXT NOT NULL,
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            source_created_at TEXT NOT NULL,
            migrated_at TEXT NOT NULL,
            UNIQUE(workspace_id, record_id, field_key),
            FOREIGN KEY(workspace_id, profile_id, record_id, field_key)
                REFERENCES metadata_profile_field_overrides(
                    workspace_id, profile_id, record_id, field_key
                ) ON DELETE RESTRICT
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE metadata_projections (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            field_key TEXT NOT NULL,
            resolution_tier TEXT NOT NULL CHECK (resolution_tier IN (
                'user_override', 'preferred_provider_claim',
                'fallback_provider_claim', 'last_known_good', 'empty'
            )),
            value TEXT,
            claim_id TEXT REFERENCES metadata_claims(claim_id),
            is_stale INTEGER NOT NULL CHECK (is_stale IN (0, 1)),
            projected_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, profile_id, record_id, field_key),
            CHECK (
                (resolution_tier = 'empty' AND value IS NULL AND claim_id IS NULL)
                OR
                (resolution_tier = 'user_override' AND value IS NOT NULL AND claim_id IS NULL)
                OR
                (resolution_tier NOT IN ('empty', 'user_override')
                    AND value IS NOT NULL AND claim_id IS NOT NULL)
            )
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_projections_profile_idx
            ON metadata_projections(workspace_id, profile_id, record_id, field_key);

        CREATE TRIGGER metadata_projections_scope_insert
        BEFORE INSERT ON metadata_projections
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        ) OR (NEW.claim_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM metadata_claims
            WHERE claim_id = NEW.claim_id
              AND record_id = NEW.record_id
              AND workspace_id = NEW.workspace_id
        ))
        BEGIN
            SELECT RAISE(ABORT, 'metadata projection crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_projections_scope_update
        BEFORE UPDATE ON metadata_projections
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        ) OR (NEW.claim_id IS NOT NULL AND NOT EXISTS (
            SELECT 1 FROM metadata_claims
            WHERE claim_id = NEW.claim_id
              AND record_id = NEW.record_id
              AND workspace_id = NEW.workspace_id
        ))
        BEGIN
            SELECT RAISE(ABORT, 'metadata projection crosses a workspace boundary');
        END;

        CREATE TABLE metadata_attributions (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
            attribution_text TEXT NOT NULL CHECK (length(attribution_text) BETWEEN 1 AND 256),
            documentation_url TEXT NOT NULL CHECK (
                length(documentation_url) BETWEEN 9 AND 2048
                AND substr(documentation_url, 1, 8) = 'https://'
            ),
            updated_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, provider_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TABLE metadata_cache_entries (
            cache_key TEXT PRIMARY KEY CHECK (
                length(cache_key) = 64 AND cache_key NOT GLOB '*[^0-9a-f]*'
            ),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            provider_id TEXT NOT NULL CHECK (length(provider_id) BETWEEN 1 AND 128),
            settings_fingerprint TEXT NOT NULL CHECK (
                length(settings_fingerprint) = 71
                AND substr(settings_fingerprint, 1, 7) = 'sha256:'
                AND substr(settings_fingerprint, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            configuration_digest TEXT NOT NULL CHECK (
                length(configuration_digest) = 71
                AND substr(configuration_digest, 1, 7) = 'sha256:'
                AND substr(configuration_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            credential_reference_version INTEGER
                CHECK (credential_reference_version IS NULL OR credential_reference_version >= 1),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            route TEXT NOT NULL CHECK (length(route) BETWEEN 1 AND 512),
            grain TEXT NOT NULL CHECK (length(grain) BETWEEN 1 AND 32),
            identifier_namespace TEXT NOT NULL
                CHECK (length(identifier_namespace) BETWEEN 1 AND 128),
            identifier_value TEXT NOT NULL
                CHECK (length(identifier_value) BETWEEN 1 AND 512),
            locale TEXT CHECK (locale IS NULL OR length(locale) BETWEEN 2 AND 16),
            region TEXT CHECK (region IS NULL OR length(region) BETWEEN 2 AND 8),
            field_group TEXT NOT NULL CHECK (length(field_group) BETWEEN 1 AND 64),
            schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
            purpose TEXT NOT NULL CHECK (length(purpose) BETWEEN 1 AND 64),
            terms_revision TEXT NOT NULL CHECK (length(terms_revision) BETWEEN 1 AND 128),
            classification TEXT NOT NULL DEFAULT 'internal'
                CHECK (classification IN ('public', 'internal', 'confidential', 'restricted')),
            invalidation_reason TEXT CHECK (invalidation_reason IS NULL OR invalidation_reason IN (
                'provider_configuration_changed', 'credential_rotated',
                'projection_policy_changed', 'terms_changed', 'explicit_retraction'
            )),
            invalidated_at TEXT,
            fresh_until TEXT NOT NULL,
            stale_while_refreshing_until TEXT NOT NULL,
            stale_on_error_until TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK (fresh_until >= created_at),
            CHECK (stale_while_refreshing_until >= fresh_until),
            CHECK (stale_on_error_until >= stale_while_refreshing_until),
            CHECK (
                (invalidation_reason IS NULL AND invalidated_at IS NULL)
                OR (invalidation_reason IS NOT NULL AND invalidated_at IS NOT NULL)
            ),
            UNIQUE (
                workspace_id, provider_id, settings_fingerprint, configuration_digest,
                credential_reference_version, record_id, route, grain,
                identifier_namespace, identifier_value, locale, region,
                field_group, schema_version, purpose, terms_revision,
                classification
            )
        ) STRICT, WITHOUT ROWID;
        CREATE INDEX metadata_cache_entries_expiry_idx
            ON metadata_cache_entries(workspace_id, provider_id, stale_on_error_until, cache_key);

        CREATE TABLE metadata_cache_claims (
            cache_key TEXT NOT NULL REFERENCES metadata_cache_entries(cache_key) ON DELETE CASCADE,
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 255),
            claim_id TEXT NOT NULL REFERENCES metadata_claims(claim_id),
            PRIMARY KEY (cache_key, ordinal),
            UNIQUE(cache_key, claim_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER metadata_cache_claims_scope_insert
        BEFORE INSERT ON metadata_cache_claims
        WHEN NOT EXISTS (
            SELECT 1 FROM metadata_cache_entries cache
            WHERE cache.cache_key = NEW.cache_key
              AND cache.workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM metadata_claims claim
            JOIN metadata_cache_entries cache
              ON cache.cache_key = NEW.cache_key
            WHERE claim.claim_id = NEW.claim_id
              AND claim.workspace_id = NEW.workspace_id
              AND claim.record_id = cache.record_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata cache claim crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_cache_claims_scope_update
        BEFORE UPDATE ON metadata_cache_claims
        BEGIN
            SELECT RAISE(ABORT, 'metadata cache claim references are immutable');
        END;

        CREATE TRIGGER metadata_cache_entries_scope_insert
        BEFORE INSERT ON metadata_cache_entries
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata cache entry crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_cache_entries_scope_update
        BEFORE UPDATE ON metadata_cache_entries
        WHEN NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata cache entry crosses a workspace boundary');
        END;
        "#,
    )?;

    migrate_imported_legacy_metadata_v12(&transaction)?;

    let mut revision_sql = String::new();
    // Disposable projections and cache partitions are intentionally absent:
    // their authoritative inputs are revisioned and portable, while these
    // derived rows are rebuilt after restore.
    for table in [
        "metadata_claims",
        "metadata_claim_provenance",
        "metadata_rating_claims",
        "metadata_claim_lifecycle_events",
        "metadata_projection_policies",
        "metadata_profile_field_overrides",
        "metadata_legacy_override_ownership",
        "metadata_override_migration_receipts",
        "metadata_attributions",
    ] {
        append_revision_triggers(
            &mut revision_sql,
            &RevisionSource {
                table,
                new_workspace: "NEW.workspace_id",
                old_workspace: "OLD.workspace_id",
            },
        );
    }
    transaction.execute_batch(&revision_sql)?;
    for scope in [
        ScopeKey::MetadataClaimRefresh,
        ScopeKey::MetadataProjectionRead,
        ScopeKey::MetadataProjectionConfigure,
    ] {
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
            [scope_storage_key(scope)],
        )?;
    }
    transaction.pragma_update(None, "user_version", 12)?;
    transaction.commit()
}

fn migrate_v13(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        CREATE TABLE metadata_refresh_receipts (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            client_id TEXT NOT NULL REFERENCES clients(client_id),
            operation_id TEXT NOT NULL CHECK (
                length(operation_id) = 35
                AND substr(operation_id, 1, 3) = 'op_'
                AND substr(operation_id, 4) NOT GLOB '*[^0-9a-f]*'
                AND substr(operation_id, 16, 1) = '7'
                AND substr(operation_id, 20, 1) GLOB '[89ab]'
            ),
            semantic_digest TEXT NOT NULL CHECK (
                length(semantic_digest) = 71
                AND substr(semantic_digest, 1, 7) = 'sha256:'
                AND substr(semantic_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            provider_id TEXT NOT NULL CHECK (
                length(provider_id) BETWEEN 1 AND 128
                AND provider_id = trim(provider_id)
                AND provider_id = lower(provider_id)
                AND provider_id NOT GLOB '*[^a-z0-9._:/-]*'
            ),
            response_json TEXT NOT NULL CHECK (
                length(response_json) BETWEEN 2 AND 1048576
                AND json_valid(response_json)
            ),
            created_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, client_id, operation_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER metadata_refresh_receipts_scope_insert
        BEFORE INSERT ON metadata_refresh_receipts
        WHEN NOT EXISTS (
            SELECT 1 FROM clients
            WHERE client_id = NEW.client_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM profiles
            WHERE profile_id = NEW.profile_id AND workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM records
            WHERE record_id = NEW.record_id AND workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'metadata refresh receipt crosses a workspace boundary');
        END;

        CREATE TRIGGER metadata_refresh_receipts_immutable_update
        BEFORE UPDATE ON metadata_refresh_receipts
        BEGIN
            SELECT RAISE(ABORT, 'metadata refresh receipts are immutable');
        END;

        CREATE TRIGGER metadata_refresh_receipts_immutable_delete
        BEFORE DELETE ON metadata_refresh_receipts
        BEGIN
            SELECT RAISE(ABORT, 'metadata refresh receipts are immutable');
        END;
        "#,
    )?;
    let mut revision_sql = String::new();
    append_revision_triggers(
        &mut revision_sql,
        &RevisionSource {
            table: "metadata_refresh_receipts",
            new_workspace: "NEW.workspace_id",
            old_workspace: "OLD.workspace_id",
        },
    );
    transaction.execute_batch(&revision_sql)?;
    transaction.pragma_update(None, "user_version", 13)?;
    transaction.commit()
}

fn migrate_v14(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        CREATE TABLE trailbase_installation (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            trailbase_instance_id TEXT NOT NULL UNIQUE CHECK (
                length(trailbase_instance_id) = 36
                AND substr(trailbase_instance_id, 1, 4) = 'tbi_'
                AND substr(trailbase_instance_id, 5) NOT GLOB '*[^0-9a-f]*'
                AND substr(trailbase_instance_id, 17, 1) = '7'
                AND substr(trailbase_instance_id, 21, 1) GLOB '[89ab]'
            ),
            physical_root_identity TEXT NOT NULL CHECK (
                length(physical_root_identity) = 71
                AND substr(physical_root_identity, 1, 7) = 'sha256:'
                AND substr(physical_root_identity, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            release_lock_identity TEXT CHECK (
                release_lock_identity IS NULL OR (
                    length(release_lock_identity) = 71
                    AND substr(release_lock_identity, 1, 7) = 'sha256:'
                    AND substr(release_lock_identity, 8) NOT GLOB '*[^0-9a-f]*'
                )
            ),
            activation_state TEXT NOT NULL
                CHECK (activation_state IN ('inactive', 'active', 'blocked')),
            activation_blocker TEXT CHECK (activation_blocker IN (
                'release_mismatch', 'physical_root_identity_mismatch', 'declared_restore'
            )),
            activation_generation INTEGER NOT NULL CHECK (activation_generation >= 0),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK (updated_at >= created_at),
            CHECK (
                (activation_state = 'inactive'
                    AND activation_blocker IS NULL
                    AND activation_generation = 0)
                OR
                (activation_state = 'active'
                    AND activation_blocker IS NULL
                    AND release_lock_identity IS NOT NULL
                    AND activation_generation >= 1)
                OR
                (activation_state = 'blocked' AND activation_blocker IS NOT NULL)
            )
        ) STRICT;

        CREATE TABLE trailbase_auth_anchors (
            trailbase_instance_id TEXT NOT NULL
                REFERENCES trailbase_installation(trailbase_instance_id),
            trailbase_subject BLOB NOT NULL CHECK (length(trailbase_subject) = 16),
            auth_subject_id TEXT NOT NULL UNIQUE
                REFERENCES auth_subjects(auth_subject_id),
            linked_at TEXT NOT NULL,
            PRIMARY KEY (trailbase_instance_id, trailbase_subject)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER trailbase_auth_anchors_immutable_update
        BEFORE UPDATE ON trailbase_auth_anchors
        BEGIN
            SELECT RAISE(ABORT, 'TrailBase authentication anchors are immutable');
        END;

        CREATE TRIGGER trailbase_auth_anchors_immutable_delete
        BEFORE DELETE ON trailbase_auth_anchors
        BEGIN
            SELECT RAISE(ABORT, 'TrailBase authentication anchors are immutable');
        END;

        CREATE TABLE workspace_memberships (
            membership_id TEXT PRIMARY KEY CHECK (
                length(membership_id) = 36
                AND substr(membership_id, 1, 4) = 'mem_'
                AND substr(membership_id, 5) NOT GLOB '*[^0-9a-f]*'
                AND substr(membership_id, 17, 1) = '7'
                AND substr(membership_id, 21, 1) GLOB '[89ab]'
            ),
            auth_subject_id TEXT NOT NULL REFERENCES auth_subjects(auth_subject_id),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            lifecycle TEXT NOT NULL CHECK (lifecycle IN (
                'invited', 'pending_approval', 'active', 'suspended', 'removed'
            )),
            role TEXT NOT NULL CHECK (role IN ('member', 'administrator')),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK (updated_at >= created_at),
            CHECK (
                lifecycle NOT IN ('invited', 'pending_approval') OR role = 'member'
            )
        ) STRICT;
        CREATE UNIQUE INDEX workspace_memberships_current_idx
            ON workspace_memberships(auth_subject_id, workspace_id)
            WHERE lifecycle <> 'removed';
        CREATE INDEX workspace_memberships_authorization_idx
            ON workspace_memberships(workspace_id, lifecycle, role, auth_subject_id);

        CREATE TRIGGER workspace_memberships_removed_immutable_update
        BEFORE UPDATE ON workspace_memberships
        WHEN OLD.lifecycle = 'removed'
        BEGIN
            SELECT RAISE(ABORT, 'removed workspace memberships are immutable');
        END;

        CREATE TRIGGER workspace_memberships_removed_immutable_delete
        BEFORE DELETE ON workspace_memberships
        WHEN OLD.lifecycle = 'removed'
        BEGIN
            SELECT RAISE(ABORT, 'removed workspace memberships are immutable');
        END;

        CREATE TABLE auth_ceremonies (
            operation_id TEXT PRIMARY KEY CHECK (
                length(operation_id) = 35
                AND substr(operation_id, 1, 3) = 'op_'
                AND substr(operation_id, 4) NOT GLOB '*[^0-9a-f]*'
                AND substr(operation_id, 16, 1) = '7'
                AND substr(operation_id, 20, 1) GLOB '[89ab]'
            ),
            purpose TEXT NOT NULL CHECK (purpose IN (
                'sign_in', 'recent_authentication', 'first_administrator_bootstrap'
            )),
            protocol TEXT NOT NULL
                CHECK (protocol = 'trailbase_authorization_code_pkce'),
            trailbase_instance_id TEXT NOT NULL
                REFERENCES trailbase_installation(trailbase_instance_id),
            activation_generation INTEGER NOT NULL CHECK (activation_generation >= 1),
            browser_binding_digest TEXT NOT NULL CHECK (
                length(browser_binding_digest) = 71
                AND substr(browser_binding_digest, 1, 7) = 'sha256:'
                AND substr(browser_binding_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            workspace_id TEXT CHECK (
                workspace_id IS NULL OR (
                    length(workspace_id) = 36
                    AND substr(workspace_id, 1, 4) = 'wsp_'
                    AND substr(workspace_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(workspace_id, 17, 1) = '7'
                    AND substr(workspace_id, 21, 1) GLOB '[89ab]'
                )
            ),
            selected_profile_grant_id TEXT CHECK (
                selected_profile_grant_id IS NULL OR (
                    length(selected_profile_grant_id) = 36
                    AND substr(selected_profile_grant_id, 1, 4) = 'grt_'
                    AND substr(selected_profile_grant_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(selected_profile_grant_id, 17, 1) = '7'
                    AND substr(selected_profile_grant_id, 21, 1) GLOB '[89ab]'
                )
            ),
            bound_browser_session_id TEXT CHECK (
                bound_browser_session_id IS NULL OR (
                    length(bound_browser_session_id) = 36
                    AND substr(bound_browser_session_id, 1, 4) = 'ses_'
                    AND substr(bound_browser_session_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(bound_browser_session_id, 17, 1) = '7'
                    AND substr(bound_browser_session_id, 21, 1) GLOB '[89ab]'
                )
            ),
            invited_membership_id TEXT CHECK (
                invited_membership_id IS NULL OR (
                    length(invited_membership_id) = 36
                    AND substr(invited_membership_id, 1, 4) = 'mem_'
                    AND substr(invited_membership_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(invited_membership_id, 17, 1) = '7'
                    AND substr(invited_membership_id, 21, 1) GLOB '[89ab]'
                )
            ),
            remembered INTEGER NOT NULL CHECK (remembered IN (0, 1)),
            confirmed_auth_subject_id TEXT
                REFERENCES auth_subjects(auth_subject_id),
            authentication_method TEXT CHECK (
                authentication_method IS NULL OR authentication_method IN (
                    'trailbase_password', 'trailbase_social'
                )
            ),
            authentication_verified_at TEXT,
            confirmed_auth_epoch INTEGER CHECK (
                confirmed_auth_epoch IS NULL OR confirmed_auth_epoch >= 0
            ),
            confirmed_authorization_epoch INTEGER CHECK (
                confirmed_authorization_epoch IS NULL
                OR confirmed_authorization_epoch >= 0
            ),
            callback_path TEXT NOT NULL CHECK (
                length(callback_path) BETWEEN 1 AND 128
                AND substr(callback_path, 1, 1) = '/'
                AND substr(callback_path, 1, 2) <> '//'
                AND callback_path NOT GLOB '*[^A-Za-z0-9/_.-]*'
            ),
            return_target TEXT NOT NULL CHECK (return_target IN (
                'application_home', 'account_security', 'first_run'
            )),
            correlation_id TEXT NOT NULL CHECK (
                length(correlation_id) = 36
                AND substr(correlation_id, 1, 4) = 'req_'
                AND substr(correlation_id, 5) NOT GLOB '*[^0-9a-f]*'
                AND substr(correlation_id, 17, 1) = '7'
                AND substr(correlation_id, 21, 1) GLOB '[89ab]'
            ),
            state TEXT NOT NULL CHECK (state IN (
                'pending', 'claimed', 'selection_required', 'completed',
                'cancelled', 'failed', 'cleanup_uncertain', 'expired'
            )),
            failure TEXT CHECK (failure IN (
                'verifier_lost_on_restart', 'exchange_outcome_uncertain',
                'exchange_failed', 'status_rejected', 'logout_uncertain',
                'local_authorization_denied', 'local_persistence_failed',
                'trust_unavailable'
            )),
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            claimed_at TEXT,
            terminal_at TEXT,
            CHECK (expires_at > created_at),
            CHECK (claimed_at IS NULL OR (claimed_at >= created_at AND claimed_at < expires_at)),
            CHECK (terminal_at IS NULL OR terminal_at >= COALESCE(claimed_at, created_at)),
            CHECK (
                (workspace_id IS NULL
                    AND selected_profile_grant_id IS NULL
                    AND bound_browser_session_id IS NULL
                    AND invited_membership_id IS NULL)
                OR
                (workspace_id IS NOT NULL AND selected_profile_grant_id IS NOT NULL)
            ),
            CHECK (
                (confirmed_auth_subject_id IS NULL
                    AND authentication_method IS NULL
                    AND authentication_verified_at IS NULL
                    AND confirmed_auth_epoch IS NULL
                    AND confirmed_authorization_epoch IS NULL)
                OR
                (confirmed_auth_subject_id IS NOT NULL
                    AND authentication_method IS NOT NULL
                    AND authentication_verified_at IS NOT NULL
                    AND confirmed_auth_epoch IS NOT NULL
                    AND confirmed_authorization_epoch IS NOT NULL)
            ),
            CHECK (
                authentication_verified_at IS NULL OR (
                    claimed_at IS NOT NULL
                    AND authentication_verified_at >= claimed_at
                    AND authentication_verified_at < expires_at
                )
            ),
            CHECK (
                (purpose = 'sign_in' AND return_target = 'application_home')
                OR (purpose = 'recent_authentication' AND return_target = 'account_security')
                OR (purpose = 'first_administrator_bootstrap' AND return_target = 'first_run')
            ),
            CHECK (
                (purpose = 'sign_in'
                    AND bound_browser_session_id IS NULL
                    AND (
                        (state IN (
                            'pending', 'claimed', 'failed', 'cleanup_uncertain'
                        )
                            AND workspace_id IS NULL
                            AND confirmed_auth_subject_id IS NULL)
                        OR
                        (state IN ('cancelled', 'expired')
                            AND workspace_id IS NULL
                            AND (
                                (claimed_at IS NULL
                                    AND confirmed_auth_subject_id IS NULL)
                                OR
                                (claimed_at IS NOT NULL
                                    AND confirmed_auth_subject_id IS NOT NULL)
                            ))
                        OR
                        (state = 'selection_required'
                            AND workspace_id IS NULL
                            AND confirmed_auth_subject_id IS NOT NULL)
                        OR
                        (state = 'completed'
                            AND workspace_id IS NOT NULL
                            AND confirmed_auth_subject_id IS NOT NULL)
                    ))
                OR (purpose = 'recent_authentication'
                    AND workspace_id IS NOT NULL
                    AND bound_browser_session_id IS NOT NULL
                    AND invited_membership_id IS NULL
                    AND remembered = 0
                    AND confirmed_auth_subject_id IS NULL
                    AND state <> 'selection_required')
                OR (purpose = 'first_administrator_bootstrap'
                    AND workspace_id IS NOT NULL
                    AND bound_browser_session_id IS NULL
                    AND invited_membership_id IS NULL
                    AND remembered = 0
                    AND confirmed_auth_subject_id IS NULL
                    AND state <> 'selection_required')
            ),
            CHECK (
                (state = 'pending' AND failure IS NULL
                    AND claimed_at IS NULL AND terminal_at IS NULL)
                OR
                (state = 'claimed' AND failure IS NULL
                    AND claimed_at IS NOT NULL AND terminal_at IS NULL)
                OR
                (state = 'selection_required' AND failure IS NULL
                    AND claimed_at IS NOT NULL AND terminal_at IS NULL)
                OR
                (state = 'completed' AND failure IS NULL
                    AND claimed_at IS NOT NULL AND terminal_at IS NOT NULL
                    AND terminal_at < expires_at)
                OR
                (state = 'cancelled' AND failure IS NULL
                    AND terminal_at IS NOT NULL
                    AND terminal_at < expires_at)
                OR
                (state = 'failed' AND terminal_at IS NOT NULL AND (
                    (claimed_at IS NULL AND failure = 'verifier_lost_on_restart')
                    OR
                    (claimed_at IS NOT NULL AND failure IN (
                        'exchange_failed', 'status_rejected', 'local_authorization_denied',
                        'local_persistence_failed', 'trust_unavailable'
                    ))
                ))
                OR
                (state = 'cleanup_uncertain' AND claimed_at IS NOT NULL
                    AND terminal_at IS NOT NULL
                    AND failure IN ('exchange_outcome_uncertain', 'logout_uncertain'))
                OR
                (state = 'expired' AND failure IS NULL
                    AND terminal_at IS NOT NULL AND terminal_at >= expires_at)
            )
        ) STRICT;
        CREATE INDEX auth_ceremonies_state_expiry_idx
            ON auth_ceremonies(state, expires_at, operation_id);
        CREATE UNIQUE INDEX auth_ceremonies_browser_binding_idx
            ON auth_ceremonies(browser_binding_digest);
        CREATE INDEX auth_ceremonies_terminal_idx
            ON auth_ceremonies(terminal_at, operation_id)
            WHERE terminal_at IS NOT NULL;
        CREATE UNIQUE INDEX auth_ceremonies_active_bootstrap_idx
            ON auth_ceremonies(purpose)
            WHERE purpose = 'first_administrator_bootstrap'
                AND state IN ('pending', 'claimed');

        CREATE TABLE fasti_browser_session_authentication (
            browser_session_id TEXT PRIMARY KEY
                REFERENCES fasti_browser_sessions(browser_session_id) ON DELETE CASCADE,
            trailbase_instance_id TEXT NOT NULL
                REFERENCES trailbase_installation(trailbase_instance_id),
            activation_generation INTEGER NOT NULL CHECK (activation_generation >= 1),
            method TEXT NOT NULL CHECK (method IN (
                'trailbase_password', 'trailbase_social'
            )),
            verified_at TEXT NOT NULL,
            recent_authentication_expires_at TEXT,
            CHECK (
                recent_authentication_expires_at IS NULL
                OR recent_authentication_expires_at > verified_at
            )
        ) STRICT;
        CREATE INDEX fasti_browser_session_authentication_generation_idx
            ON fasti_browser_session_authentication(
                trailbase_instance_id, activation_generation, browser_session_id
            );

        CREATE TABLE access_audit_events (
            audit_event_id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_kind TEXT NOT NULL CHECK (event_kind IN (
                'trailbase_activated', 'trailbase_blocked', 'anchor_linked',
                'first_administrator_bootstrapped', 'subject_disabled',
                'subject_deleted', 'subject_recovery_pending', 'subject_reactivated',
                'membership_invited', 'membership_approval_requested',
                'membership_invitation_accepted', 'membership_approved',
                'membership_suspended', 'membership_resumed', 'membership_removed',
                'membership_promoted', 'membership_demoted',
                'ceremony_claimed', 'ceremony_selection_required', 'ceremony_completed',
                'ceremony_cancelled', 'ceremony_expired',
                'ceremony_cleanup_uncertain', 'ceremony_failed',
                'browser_session_issued', 'browser_session_revoked'
            )),
            trailbase_instance_id TEXT CHECK (
                trailbase_instance_id IS NULL OR (
                    length(trailbase_instance_id) = 36
                    AND substr(trailbase_instance_id, 1, 4) = 'tbi_'
                    AND substr(trailbase_instance_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(trailbase_instance_id, 17, 1) = '7'
                    AND substr(trailbase_instance_id, 21, 1) GLOB '[89ab]'
                )
            ),
            auth_subject_id TEXT CHECK (
                auth_subject_id IS NULL OR (
                    length(auth_subject_id) = 36
                    AND substr(auth_subject_id, 1, 4) = 'sub_'
                    AND substr(auth_subject_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(auth_subject_id, 17, 1) = '7'
                    AND substr(auth_subject_id, 21, 1) GLOB '[89ab]'
                )
            ),
            actor_auth_subject_id TEXT CHECK (
                actor_auth_subject_id IS NULL OR (
                    length(actor_auth_subject_id) = 36
                    AND substr(actor_auth_subject_id, 1, 4) = 'sub_'
                    AND substr(actor_auth_subject_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(actor_auth_subject_id, 17, 1) = '7'
                    AND substr(actor_auth_subject_id, 21, 1) GLOB '[89ab]'
                )
            ),
            workspace_id TEXT CHECK (
                workspace_id IS NULL OR (
                    length(workspace_id) = 36
                    AND substr(workspace_id, 1, 4) = 'wsp_'
                    AND substr(workspace_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(workspace_id, 17, 1) = '7'
                    AND substr(workspace_id, 21, 1) GLOB '[89ab]'
                )
            ),
            membership_id TEXT CHECK (
                membership_id IS NULL OR (
                    length(membership_id) = 36
                    AND substr(membership_id, 1, 4) = 'mem_'
                    AND substr(membership_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(membership_id, 17, 1) = '7'
                    AND substr(membership_id, 21, 1) GLOB '[89ab]'
                )
            ),
            operation_id TEXT CHECK (
                operation_id IS NULL OR (
                    length(operation_id) = 35
                    AND substr(operation_id, 1, 3) = 'op_'
                    AND substr(operation_id, 4) NOT GLOB '*[^0-9a-f]*'
                    AND substr(operation_id, 16, 1) = '7'
                    AND substr(operation_id, 20, 1) GLOB '[89ab]'
                )
            ),
            browser_session_id TEXT CHECK (
                browser_session_id IS NULL OR (
                    length(browser_session_id) = 36
                    AND substr(browser_session_id, 1, 4) = 'ses_'
                    AND substr(browser_session_id, 5) NOT GLOB '*[^0-9a-f]*'
                    AND substr(browser_session_id, 17, 1) = '7'
                    AND substr(browser_session_id, 21, 1) GLOB '[89ab]'
                )
            ),
            correlation_id TEXT NOT NULL CHECK (
                length(correlation_id) = 36
                AND substr(correlation_id, 1, 4) = 'req_'
                AND substr(correlation_id, 5) NOT GLOB '*[^0-9a-f]*'
                AND substr(correlation_id, 17, 1) = '7'
                AND substr(correlation_id, 21, 1) GLOB '[89ab]'
            ),
            occurred_at TEXT NOT NULL,
            CHECK (
                (event_kind IN ('trailbase_activated', 'trailbase_blocked')
                    AND trailbase_instance_id IS NOT NULL)
                OR
                (event_kind = 'anchor_linked'
                    AND trailbase_instance_id IS NOT NULL
                    AND auth_subject_id IS NOT NULL)
                OR
                (event_kind = 'first_administrator_bootstrapped'
                    AND trailbase_instance_id IS NOT NULL
                    AND auth_subject_id IS NOT NULL
                    AND workspace_id IS NOT NULL
                    AND membership_id IS NOT NULL
                    AND operation_id IS NOT NULL)
                OR
                (event_kind IN (
                    'subject_disabled', 'subject_deleted',
                    'subject_recovery_pending', 'subject_reactivated'
                )
                    AND auth_subject_id IS NOT NULL
                    AND actor_auth_subject_id IS NOT NULL)
                OR
                (event_kind IN (
                        'membership_invited', 'membership_approval_requested',
                        'membership_invitation_accepted', 'membership_approved',
                        'membership_suspended', 'membership_resumed',
                        'membership_removed', 'membership_promoted',
                        'membership_demoted'
                    )
                    AND auth_subject_id IS NOT NULL
                    AND actor_auth_subject_id IS NOT NULL
                    AND workspace_id IS NOT NULL
                    AND membership_id IS NOT NULL)
                OR
                (event_kind IN (
                        'ceremony_claimed', 'ceremony_selection_required', 'ceremony_completed',
                        'ceremony_cancelled', 'ceremony_expired',
                        'ceremony_cleanup_uncertain', 'ceremony_failed'
                    )
                    AND trailbase_instance_id IS NOT NULL
                    AND operation_id IS NOT NULL)
                OR
                (event_kind = 'browser_session_issued'
                    AND trailbase_instance_id IS NOT NULL
                    AND auth_subject_id IS NOT NULL
                    AND workspace_id IS NOT NULL
                    AND operation_id IS NOT NULL
                    AND browser_session_id IS NOT NULL)
                OR
                (event_kind = 'browser_session_revoked'
                    AND trailbase_instance_id IS NOT NULL
                    AND auth_subject_id IS NOT NULL
                    AND actor_auth_subject_id IS NOT NULL
                    AND workspace_id IS NOT NULL
                    AND browser_session_id IS NOT NULL)
            )
        ) STRICT;
        CREATE INDEX access_audit_events_retention_idx
            ON access_audit_events(occurred_at, audit_event_id);

        CREATE TRIGGER access_audit_events_immutable_update
        BEFORE UPDATE ON access_audit_events
        BEGIN
            SELECT RAISE(ABORT, 'Access audit events are immutable');
        END;
        "#,
    )?;
    transaction.pragma_update(None, "user_version", 14)?;
    transaction.commit()
}

fn migrate_v15(connection: &Connection) -> Result<()> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        r#"
        CREATE INDEX records_workspace_record_idx
            ON records(workspace_id, record_id);

        CREATE TABLE identity_assertions (
            assertion_id TEXT PRIMARY KEY CHECK (
                length(assertion_id) = 36
                AND substr(assertion_id, 1, 4) = 'asr_'
                AND substr(assertion_id, 5) NOT GLOB '*[^0-9a-f]*'
                AND substr(assertion_id, 17, 1) = '7'
                AND substr(assertion_id, 21, 1) GLOB '[89ab]'
            ),
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            record_id TEXT NOT NULL REFERENCES records(record_id),
            source_external_identifier_id TEXT NOT NULL
                REFERENCES external_identifiers(external_identifier_id),
            target_namespace TEXT NOT NULL CHECK (
                length(target_namespace) BETWEEN 2 AND 64
                AND target_namespace = lower(target_namespace)
                AND substr(target_namespace, 1, 1) GLOB '[a-z]'
                AND target_namespace NOT GLOB '*[^a-z0-9._-]*'
            ),
            target_grain TEXT NOT NULL CHECK (length(target_grain) BETWEEN 1 AND 32),
            target_value TEXT NOT NULL CHECK (length(target_value) BETWEEN 1 AND 256),
            relation TEXT NOT NULL CHECK (relation IN (
                'exact', 'subset_of', 'superset_of', 'overlaps',
                'alternate_cut_of', 'related', 'not_same_as'
            )),
            coverage_json TEXT NOT NULL CHECK (
                length(coverage_json) BETWEEN 2 AND 262144
                AND json_valid(coverage_json)
                AND json_type(coverage_json) = 'array'
                AND json_array_length(coverage_json) <= 64
            ),
            episode_links_json TEXT NOT NULL CHECK (
                length(episode_links_json) BETWEEN 2 AND 262144
                AND json_valid(episode_links_json)
                AND json_type(episode_links_json) = 'array'
                AND json_array_length(episode_links_json) <= 64
            ),
            evidence_class TEXT NOT NULL CHECK (evidence_class IN (
                'asserted', 'verified', 'corroborated', 'inferred',
                'candidate', 'disputed'
            )),
            evidence_json TEXT NOT NULL CHECK (
                length(evidence_json) BETWEEN 2 AND 131072
                AND json_valid(evidence_json)
                AND json_type(evidence_json) = 'array'
                AND json_array_length(evidence_json) BETWEEN 1 AND 16
            ),
            id_source TEXT NOT NULL CHECK (length(id_source) BETWEEN 3 AND 256),
            source_version TEXT CHECK (
                source_version IS NULL OR length(source_version) BETWEEN 1 AND 256
            ),
            authority TEXT CHECK (
                authority IS NULL OR length(authority) BETWEEN 1 AND 256
            ),
            reasoning TEXT CHECK (
                reasoning IS NULL OR length(reasoning) BETWEEN 1 AND 4096
            ),
            initial_status TEXT NOT NULL CHECK (initial_status IN (
                'candidate', 'accepted', 'disputed', 'rejected', 'revoked'
            )),
            created_at TEXT NOT NULL,
            UNIQUE (
                workspace_id, record_id, source_external_identifier_id,
                target_namespace, target_grain, target_value, relation
            )
        ) STRICT;
        CREATE INDEX identity_assertions_record_idx
            ON identity_assertions(workspace_id, record_id, assertion_id);

        CREATE TRIGGER identity_assertions_scope_insert
        BEFORE INSERT ON identity_assertions
        WHEN NOT EXISTS (
            SELECT 1 FROM external_identifiers identifier
            WHERE identifier.external_identifier_id = NEW.source_external_identifier_id
              AND identifier.workspace_id = NEW.workspace_id
              AND identifier.record_id = NEW.record_id
        ) OR NOT EXISTS (
            SELECT 1 FROM namespace_definitions definition
            WHERE definition.workspace_id = NEW.workspace_id
              AND definition.namespace = NEW.target_namespace
              AND instr(
                    ',' || definition.supported_grains || ',',
                    ',' || NEW.target_grain || ','
                  ) > 0
        )
        BEGIN
            SELECT RAISE(ABORT, 'identity assertion crosses its Record or namespace boundary');
        END;

        CREATE TRIGGER identity_assertions_immutable_update
        BEFORE UPDATE ON identity_assertions
        BEGIN
            SELECT RAISE(ABORT, 'identity assertions are immutable');
        END;

        CREATE TRIGGER identity_assertions_immutable_delete
        BEFORE DELETE ON identity_assertions
        BEGIN
            SELECT RAISE(ABORT, 'identity assertions are immutable');
        END;

        CREATE TRIGGER identity_assertion_namespace_delete_guard
        BEFORE DELETE ON namespace_definitions
        WHEN EXISTS (
            SELECT 1 FROM identity_assertions assertion
            WHERE assertion.workspace_id = OLD.workspace_id
              AND assertion.target_namespace = OLD.namespace
        )
        BEGIN
            SELECT RAISE(ABORT, 'namespace is referenced by an identity assertion');
        END;

        CREATE TABLE identity_assertion_lifecycle_events (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            assertion_id TEXT NOT NULL REFERENCES identity_assertions(assertion_id),
            sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 4294967295),
            previous_status TEXT NOT NULL CHECK (previous_status IN (
                'candidate', 'accepted', 'disputed', 'rejected', 'revoked'
            )),
            status TEXT NOT NULL CHECK (status IN (
                'candidate', 'accepted', 'disputed', 'rejected', 'revoked'
            )),
            reviewer_client_id TEXT NOT NULL REFERENCES clients(client_id),
            occurred_at TEXT NOT NULL,
            evidence_digest TEXT CHECK (
                evidence_digest IS NULL OR (
                    length(evidence_digest) = 71
                    AND substr(evidence_digest, 1, 7) = 'sha256:'
                    AND substr(evidence_digest, 8) NOT GLOB '*[^0-9a-f]*'
                )
            ),
            PRIMARY KEY (assertion_id, sequence)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER identity_assertion_lifecycle_scope_insert
        BEFORE INSERT ON identity_assertion_lifecycle_events
        WHEN NOT EXISTS (
            SELECT 1 FROM identity_assertions assertion
            WHERE assertion.assertion_id = NEW.assertion_id
              AND assertion.workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM clients client
            WHERE client.client_id = NEW.reviewer_client_id
              AND client.workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'identity assertion lifecycle crosses a workspace boundary');
        END;

        CREATE TRIGGER identity_assertion_lifecycle_immutable_update
        BEFORE UPDATE ON identity_assertion_lifecycle_events
        BEGIN
            SELECT RAISE(ABORT, 'identity assertion lifecycle events are immutable');
        END;

        CREATE TRIGGER identity_assertion_lifecycle_immutable_delete
        BEFORE DELETE ON identity_assertion_lifecycle_events
        BEGIN
            SELECT RAISE(ABORT, 'identity assertion lifecycle events are immutable');
        END;

        CREATE TABLE profile_anime_grouping_policies (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            preference TEXT NOT NULL CHECK (preference IN (
                'group_by_tv_work', 'keep_mal_releases_separate',
                'keep_kitsu_releases_separate', 'automatic'
            )),
            revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
            updated_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, profile_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER profile_anime_grouping_policy_scope_insert
        BEFORE INSERT ON profile_anime_grouping_policies
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles profile
            WHERE profile.profile_id = NEW.profile_id
              AND profile.workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'anime grouping profile policy crosses a workspace boundary');
        END;

        CREATE TRIGGER profile_anime_grouping_policy_scope_update
        BEFORE UPDATE ON profile_anime_grouping_policies
        WHEN NEW.workspace_id <> OLD.workspace_id OR NEW.profile_id <> OLD.profile_id
        BEGIN
            SELECT RAISE(ABORT, 'anime grouping profile policy ownership is immutable');
        END;

        CREATE TABLE client_anime_grouping_policies (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            client_id TEXT NOT NULL REFERENCES clients(client_id),
            preference TEXT CHECK (preference IS NULL OR preference IN (
                'group_by_tv_work', 'keep_mal_releases_separate',
                'keep_kitsu_releases_separate', 'automatic'
            )),
            revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
            updated_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, profile_id, client_id)
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER client_anime_grouping_policy_scope_insert
        BEFORE INSERT ON client_anime_grouping_policies
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles profile
            WHERE profile.profile_id = NEW.profile_id
              AND profile.workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM clients client
            WHERE client.client_id = NEW.client_id
              AND client.workspace_id = NEW.workspace_id
        )
        BEGIN
            SELECT RAISE(ABORT, 'anime grouping client policy crosses a workspace boundary');
        END;

        CREATE TRIGGER client_anime_grouping_policy_scope_update
        BEFORE UPDATE ON client_anime_grouping_policies
        WHEN NEW.workspace_id <> OLD.workspace_id
          OR NEW.profile_id <> OLD.profile_id
          OR NEW.client_id <> OLD.client_id
        BEGIN
            SELECT RAISE(ABORT, 'anime grouping client policy ownership is immutable');
        END;

        CREATE TABLE anime_grouping_policy_receipts (
            workspace_id TEXT NOT NULL REFERENCES workspaces(workspace_id),
            profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
            actor_client_id TEXT NOT NULL REFERENCES clients(client_id),
            scope_kind TEXT NOT NULL CHECK (scope_kind IN ('profile', 'client')),
            scope_client_id TEXT REFERENCES clients(client_id),
            operation_id TEXT NOT NULL CHECK (
                length(operation_id) = 35
                AND substr(operation_id, 1, 3) = 'op_'
                AND substr(operation_id, 4) NOT GLOB '*[^0-9a-f]*'
                AND substr(operation_id, 16, 1) = '7'
                AND substr(operation_id, 20, 1) GLOB '[89ab]'
            ),
            semantic_digest TEXT NOT NULL CHECK (
                length(semantic_digest) = 71
                AND substr(semantic_digest, 1, 7) = 'sha256:'
                AND substr(semantic_digest, 8) NOT GLOB '*[^0-9a-f]*'
            ),
            change_kind TEXT NOT NULL CHECK (
                change_kind IN ('set', 'inherit_profile', 'rollback')
            ),
            requested_preference TEXT CHECK (
                requested_preference IS NULL OR requested_preference IN (
                    'group_by_tv_work', 'keep_mal_releases_separate',
                    'keep_kitsu_releases_separate', 'automatic'
                )
            ),
            rollback_operation_id TEXT,
            previous_preference TEXT NOT NULL CHECK (previous_preference IN (
                'group_by_tv_work', 'keep_mal_releases_separate',
                'keep_kitsu_releases_separate', 'automatic'
            )),
            previous_source TEXT NOT NULL CHECK (
                previous_source IN ('profile_default', 'client_override')
            ),
            result_preference TEXT NOT NULL CHECK (result_preference IN (
                'group_by_tv_work', 'keep_mal_releases_separate',
                'keep_kitsu_releases_separate', 'automatic'
            )),
            result_source TEXT NOT NULL CHECK (
                result_source IN ('profile_default', 'client_override')
            ),
            result_revision INTEGER NOT NULL CHECK (
                result_revision BETWEEN 1 AND 9007199254740991
            ),
            affected_records INTEGER NOT NULL CHECK (affected_records >= 0),
            unresolved_routes INTEGER NOT NULL CHECK (unresolved_routes >= 0),
            possible_season_regroupings INTEGER NOT NULL CHECK (
                possible_season_regroupings BETWEEN 0 AND affected_records
            ),
            created_at TEXT NOT NULL,
            PRIMARY KEY (workspace_id, actor_client_id, operation_id),
            UNIQUE (workspace_id, operation_id),
            CHECK (
                (scope_kind = 'profile' AND scope_client_id IS NULL)
                OR (scope_kind = 'client' AND scope_client_id IS NOT NULL)
            ),
            CHECK (
                (change_kind = 'set'
                    AND requested_preference IS NOT NULL
                    AND rollback_operation_id IS NULL)
                OR (change_kind = 'inherit_profile'
                    AND requested_preference IS NULL
                    AND rollback_operation_id IS NULL
                    AND scope_kind = 'client')
                OR (change_kind = 'rollback'
                    AND requested_preference IS NULL
                    AND rollback_operation_id IS NOT NULL
                    AND rollback_operation_id <> operation_id)
            )
        ) STRICT, WITHOUT ROWID;

        CREATE TRIGGER anime_grouping_policy_receipt_scope_insert
        BEFORE INSERT ON anime_grouping_policy_receipts
        WHEN NOT EXISTS (
            SELECT 1 FROM profiles profile
            WHERE profile.profile_id = NEW.profile_id
              AND profile.workspace_id = NEW.workspace_id
        ) OR NOT EXISTS (
            SELECT 1 FROM clients client
            WHERE client.client_id = NEW.actor_client_id
              AND client.workspace_id = NEW.workspace_id
        ) OR (
            NEW.scope_client_id IS NOT NULL AND NOT EXISTS (
                SELECT 1 FROM clients client
                WHERE client.client_id = NEW.scope_client_id
                  AND client.workspace_id = NEW.workspace_id
            )
        )
        BEGIN
            SELECT RAISE(ABORT, 'anime grouping policy receipt crosses a workspace boundary');
        END;

        CREATE TRIGGER anime_grouping_policy_receipts_immutable_update
        BEFORE UPDATE ON anime_grouping_policy_receipts
        BEGIN
            SELECT RAISE(ABORT, 'anime grouping policy receipts are immutable');
        END;

        CREATE TRIGGER anime_grouping_policy_receipts_immutable_delete
        BEFORE DELETE ON anime_grouping_policy_receipts
        BEGIN
            SELECT RAISE(ABORT, 'anime grouping policy receipts are immutable');
        END;
        "#,
    )?;
    let mut revision_sql = String::new();
    for table in [
        "identity_assertions",
        "identity_assertion_lifecycle_events",
        "profile_anime_grouping_policies",
        "client_anime_grouping_policies",
        "anime_grouping_policy_receipts",
    ] {
        append_revision_triggers(
            &mut revision_sql,
            &RevisionSource {
                table,
                new_workspace: "NEW.workspace_id",
                old_workspace: "OLD.workspace_id",
            },
        );
    }
    transaction.execute_batch(&revision_sql)?;
    transaction.pragma_update(None, "user_version", 15)?;
    transaction.commit()
}

/// Materialize v12 companions for rows restored from archive-v2 after the
/// archive's frozen legacy tables have been imported and coordinate-repaired.
/// Every insert is retry-safe and the archive-owned rows remain untouched.
pub(crate) fn migrate_imported_legacy_metadata_v12(connection: &Connection) -> Result<()> {
    let legacy_claims = {
        let mut statement = connection.prepare(
            r#"
            SELECT legacy.workspace_id, legacy.record_id, legacy.field_key,
                   legacy.source, legacy.fetched_at, legacy.created_at
            FROM metadata_field_claims legacy
            LEFT JOIN metadata_claim_provenance provenance
              ON provenance.record_id = legacy.record_id
             AND provenance.field_key = legacy.field_key
             AND provenance.source = legacy.source
             AND provenance.fetched_at = legacy.fetched_at
            WHERE provenance.claim_id IS NULL
            ORDER BY legacy.workspace_id, legacy.record_id, legacy.field_key,
                     legacy.source, legacy.fetched_at
            "#,
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>>>()?;
        rows
    };
    for (workspace_id, record_id, field_key, source, fetched_at, created_at) in legacy_claims {
        let claim_id = MetadataClaimId::new_v7().to_string();
        connection.execute(
            r#"
            INSERT INTO metadata_claims(
                claim_id, workspace_id, record_id, claim_kind, created_at
            ) VALUES (?1, ?2, ?3, 'field', ?4)
            "#,
            rusqlite::params![claim_id, workspace_id, record_id, created_at],
        )?;
        connection.execute(
            r#"
            INSERT INTO metadata_claim_provenance(
                claim_id, workspace_id, record_id, field_key, source, fetched_at,
                provider_id, classification, provenance_state, initial_status, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, NULL, 'internal',
                'legacy_incomplete', 'fresh', ?7
            )
            "#,
            rusqlite::params![
                claim_id,
                workspace_id,
                record_id,
                field_key,
                source,
                fetched_at,
                created_at
            ],
        )?;
    }
    connection.execute_batch(
        r#"
        INSERT OR IGNORE INTO metadata_legacy_override_ownership(
            workspace_id, record_id, field_key, owner_profile_id, state,
            review_reason, recorded_at
        )
        SELECT legacy.workspace_id, legacy.record_id, legacy.field_key,
               CASE WHEN (
                   SELECT COUNT(*) FROM profiles profile
                   WHERE profile.workspace_id = legacy.workspace_id
               ) = 1 THEN (
                   SELECT profile_id FROM profiles profile
                   WHERE profile.workspace_id = legacy.workspace_id
                   ORDER BY profile_id LIMIT 1
               ) ELSE NULL END,
               CASE WHEN (
                   SELECT COUNT(*) FROM profiles profile
                   WHERE profile.workspace_id = legacy.workspace_id
               ) = 1 THEN 'migrated' ELSE 'review_required' END,
               CASE WHEN (
                   SELECT COUNT(*) FROM profiles profile
                   WHERE profile.workspace_id = legacy.workspace_id
               ) = 0 THEN 'zero_profiles'
               WHEN (
                   SELECT COUNT(*) FROM profiles profile
                   WHERE profile.workspace_id = legacy.workspace_id
               ) > 1 THEN 'multiple_profiles'
               ELSE NULL END,
               legacy.created_at
        FROM metadata_field_overrides legacy;

        INSERT OR IGNORE INTO metadata_profile_field_overrides(
            workspace_id, profile_id, record_id, field_key, value,
            created_at, updated_at, origin
        )
        SELECT legacy.workspace_id, ownership.owner_profile_id, legacy.record_id,
               legacy.field_key, legacy.value, legacy.created_at,
               legacy.created_at, 'legacy_migration'
        FROM metadata_field_overrides legacy
        JOIN metadata_legacy_override_ownership ownership
          ON ownership.workspace_id = legacy.workspace_id
         AND ownership.record_id = legacy.record_id
         AND ownership.field_key = legacy.field_key
        WHERE ownership.state = 'migrated';

        INSERT OR IGNORE INTO metadata_override_migration_receipts(
            receipt_id, workspace_id, record_id, field_key, profile_id,
            source_created_at, migrated_at
        )
        SELECT 'legacy_override:' || hex(
                   legacy.workspace_id || char(0) || legacy.record_id || char(0) || legacy.field_key
               ),
               legacy.workspace_id, legacy.record_id, legacy.field_key,
               ownership.owner_profile_id, legacy.created_at, legacy.created_at
        FROM metadata_field_overrides legacy
        JOIN metadata_legacy_override_ownership ownership
          ON ownership.workspace_id = legacy.workspace_id
         AND ownership.record_id = legacy.record_id
         AND ownership.field_key = legacy.field_key
        WHERE ownership.state = 'migrated';
        "#,
    )
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

fn suspend_metadata_coordinate_guards(transaction: &Transaction<'_>) -> Result<Vec<String>> {
    let mut statement = transaction.prepare(
        r#"
        SELECT sql FROM sqlite_schema
        WHERE type = 'trigger'
          AND name IN (
            'metadata_field_claims_immutable_update',
            'metadata_claim_provenance_immutable_update'
          )
        ORDER BY name
        "#,
    )?;
    let definitions = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;
    drop(statement);
    transaction.execute_batch(
        r#"
        DROP TRIGGER IF EXISTS metadata_field_claims_immutable_update;
        DROP TRIGGER IF EXISTS metadata_claim_provenance_immutable_update;
        "#,
    )?;
    Ok(definitions)
}

fn restore_metadata_coordinate_guards(
    transaction: &Transaction<'_>,
    definitions: &[String],
) -> Result<()> {
    for definition in definitions {
        transaction.execute_batch(definition)?;
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

    let metadata_coordinate_guards = suspend_metadata_coordinate_guards(transaction)?;
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
    restore_metadata_coordinate_guards(transaction, &metadata_coordinate_guards)?;
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
    include!("search_scope_migration_tests.rs");
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
        migrate_to_version_nine(&connection);
        connection
    }

    fn migrate_to_version_nine(connection: &Connection) {
        migrate_v1(connection).expect("version one");
        migrate_v2(connection).expect("version two");
        migrate_v3(connection).expect("version three");
        migrate_v4(connection).expect("version four");
        migrate_v5(connection).expect("version five");
        migrate_v6(connection).expect("version six");
        migrate_v7(connection).expect("version seven");
        migrate_v8(connection).expect("version eight");
        migrate_v9(connection).expect("version nine");
    }

    fn migrate_to_version_ten(connection: &Connection) {
        migrate_to_version_nine(connection);
        migrate_v10(connection).expect("version ten");
    }

    fn migrate_to_version_eleven(connection: &Connection) {
        migrate_to_version_ten(connection);
        migrate_v11(connection).expect("version eleven");
    }

    fn migrate_to_version_twelve(connection: &Connection) {
        migrate_to_version_eleven(connection);
        migrate_v12(connection).expect("version twelve");
    }

    fn migrate_to_version_thirteen(connection: &Connection) {
        migrate_to_version_twelve(connection);
        migrate_v13(connection).expect("version thirteen");
    }

    fn migrate_to_version_fourteen(connection: &Connection) {
        migrate_to_version_thirteen(connection);
        migrate_v14(connection).expect("version fourteen");
    }

    #[test]
    fn published_v15_schema_fingerprint_is_stable() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        migrate_to_version_fourteen(&connection);
        migrate_v15(&connection).unwrap();
        let fingerprint = crate::portability::schema_fingerprint(
            &connection,
            fasti_domain::RequestCorrelationId::new_v7(),
        )
        .unwrap();
        assert_eq!(fingerprint.migration_version(), 15);
        assert_eq!(
            fingerprint.digest().as_str(),
            "sha256:36720ca62ef606e52f960e71cb40452323269f14e4a4af984e2fe875279a155e"
        );
    }

    #[test]
    fn v16_failure_rolls_back_and_same_connection_retry_preserves_v15() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        migrate_to_version_fourteen(&connection);
        migrate_v15(&connection).unwrap();
        connection
            .execute(
                "CREATE TABLE search_candidate_receipts(conflicting_fixture TEXT)",
                [],
            )
            .unwrap();
        assert!(migrate_v16(&connection).is_err());
        assert!(connection.is_autocommit());
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            15
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('search_pages', 'local_search_grams', 'metadata_claim_provenance_recent_idx')",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        connection
            .execute("DROP TABLE search_candidate_receipts", [])
            .unwrap();
        migrate_v16(&connection).unwrap();
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            16
        );
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'workspace_revision_%' AND tbl_name IN ('search_pages', 'search_candidate_receipts')", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }

    #[test]
    fn v16_backfills_local_search_from_published_title_and_private_override_rows() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        migrate_to_version_fourteen(&connection);
        migrate_v15(&connection).unwrap();
        let workspace = fasti_domain::WorkspaceId::new_v7().to_string();
        let profile = fasti_domain::ProfileId::new_v7().to_string();
        let record = fasti_domain::RecordId::new_v7().to_string();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id,created_at) VALUES (?1,?2)",
                params![workspace, CREATED_AT],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO profiles(profile_id,workspace_id,created_at) VALUES (?1,?2,?3)",
                params![profile, workspace, CREATED_AT],
            )
            .unwrap();
        connection.execute("INSERT INTO records(record_id,workspace_id,grain,status,created_at) VALUES (?1,?2,'film','active',?3)", params![record,workspace,CREATED_AT]).unwrap();
        // Historical SQL fixture uses only the published v15 schema.
        connection.execute("INSERT INTO metadata_field_claims(workspace_id,record_id,field_key,source,value,fetched_at,created_at) VALUES (?1,?2,'core.title','tmdb','Árbol 東京',?3,?3)", params![workspace,record,CREATED_AT]).unwrap();
        connection.execute("INSERT INTO metadata_profile_field_overrides(workspace_id,profile_id,record_id,field_key,value,created_at,updated_at,origin) VALUES (?1,?2,?3,'core.title','Private title',?4,?4,'user')", params![workspace,profile,record,CREATED_AT]).unwrap();
        migrate_v16(&connection).unwrap();
        for (partition, gram, expected) in [
            ("", "árb", 1),
            ("", "東京", 1),
            (profile.as_str(), "pri", 1),
            ("", "pri", 0),
        ] {
            let count: i64 = connection.query_row("SELECT COUNT(*) FROM local_search_grams WHERE workspace_id=?1 AND profile_partition=?2 AND gram=?3 AND record_id=?4", params![workspace,partition,gram,record], |r| r.get(0)).unwrap();
            assert_eq!(count, expected);
        }
        let original: String = connection
            .query_row(
                "SELECT value FROM metadata_field_claims WHERE record_id=?1",
                [record],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(original, "Árbol 東京");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn historical_v15_archive_v5_restores_into_v16() {
        use crate::archive::{ArchiveLimits, ArchiveWriter};
        use crate::kernel::LockedDataRoot;
        use crate::portability::{schema_fingerprint, stream_archive_entity};
        use crate::restore_activation::RESTORE_STAGING_DIRECTORY;
        use crate::restore_import::{stage_workspace_archive_pass_two, RestoreImportError};
        use fasti_application::{
            CancellationSignal, PortabilityLimits, WorkspaceExportEntity, WorkspaceManifest,
            WORKSPACE_ARCHIVE_CONTRACT_VERSION,
        };
        use fasti_contracts::CanonicalWorkspaceManifestProjection;
        use fasti_domain::{
            ClientId, ProfileId, RecordId, RequestCorrelationId, RestoreAttemptId, Sha256Digest,
            WorkspaceId,
        };
        use std::{io::Cursor, num::NonZeroU64};

        // Produce real v15 streams; do not relabel a current-schema archive.
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        migrate_to_version_fourteen(&connection);
        migrate_v15(&connection).unwrap();
        let correlation_id = RequestCorrelationId::new_v7();
        let fingerprint = schema_fingerprint(&connection, correlation_id).unwrap();
        assert_eq!(fingerprint.migration_version(), 15);
        assert_eq!(
            fingerprint.digest().as_str(),
            "sha256:36720ca62ef606e52f960e71cb40452323269f14e4a4af984e2fe875279a155e"
        );

        let workspace = WorkspaceId::new_v7();
        let profile = ProfileId::new_v7();
        let client = ClientId::new_v7();
        let record = RecordId::new_v7();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![workspace.to_string(), CREATED_AT],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                params![profile.to_string(), workspace.to_string(), CREATED_AT],
            )
            .unwrap();
        connection.execute(
            "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at)
             VALUES (?1, ?2, 'active', 1, ?3)",
            params![client.to_string(), workspace.to_string(), CREATED_AT],
        ).unwrap();
        connection
            .execute(
                "INSERT INTO records(record_id, workspace_id, grain, status, created_at)
             VALUES (?1, ?2, 'film', 'active', ?3)",
                params![record.to_string(), workspace.to_string(), CREATED_AT],
            )
            .unwrap();

        let nonzero = |value| NonZeroU64::new(value).unwrap();
        let limits = PortabilityLimits {
            max_snapshot_bytes: nonzero(32 * 1024 * 1024),
            max_wal_growth_bytes: nonzero(8 * 1024 * 1024),
            max_archive_bytes: nonzero(64 * 1024 * 1024),
            max_uncompressed_bytes: nonzero(32 * 1024 * 1024),
            max_entry_bytes: nonzero(8 * 1024 * 1024),
            max_entries: nonzero(64),
            max_rows_per_stream: nonzero(1024),
            max_path_bytes: nonzero(100),
            max_path_depth: nonzero(8),
            max_decompression_ratio: nonzero(1024),
            scratch_ceiling_bytes: nonzero(64 * 1024 * 1024),
            cleanup_reserve_bytes: nonzero(1024 * 1024),
            backup_step_pages: nonzero(64),
            backup_step_millis: nonzero(1000),
        };
        let revision =
            u64::try_from(workspace_revision(&connection, &workspace.to_string()).unwrap())
                .unwrap();
        for hostile in [false, true] {
            let archive_limits =
                ArchiveLimits::new(64 * 1024 * 1024, 128, 16 * 1024 * 1024, 64 * 1024 * 1024)
                    .unwrap();
            let mut writer = ArchiveWriter::new(Vec::new(), archive_limits).unwrap();
            let mut descriptors = Vec::new();
            let entities = WorkspaceExportEntity::for_format(5).unwrap();
            assert_eq!(entities.len(), 34);
            for &entity in entities {
                let mut bytes = Vec::new();
                let descriptor = stream_archive_entity(
                    &connection,
                    workspace,
                    entity,
                    limits,
                    &mut bytes,
                    &mut || Ok(()),
                    correlation_id,
                )
                .unwrap();
                writer
                    .append(
                        &format!("{}.ndjson", entity.as_str()),
                        bytes.len() as u64,
                        Cursor::new(bytes),
                    )
                    .unwrap();
                descriptors.push(descriptor);
            }
            let manifest = WorkspaceManifest::try_new_for_format(
                5,
                workspace,
                revision,
                WORKSPACE_ARCHIVE_CONTRACT_VERSION.to_owned(),
                fingerprint.migration_version(),
                if hostile {
                    Sha256Digest::from_bytes(&[0xff; 32])
                } else {
                    fingerprint.digest().clone()
                },
                descriptors,
                Vec::new(),
            )
            .unwrap();
            let projection =
                CanonicalWorkspaceManifestProjection::try_from_application(manifest).unwrap();
            let manifest_bytes = projection.canonical_json_bytes();
            writer
                .append(
                    "manifest.json",
                    manifest_bytes.len() as u64,
                    Cursor::new(manifest_bytes),
                )
                .unwrap();
            let root = tempfile::tempdir().unwrap();
            let lock = LockedDataRoot::acquire(root.path()).unwrap();
            let attempt = RestoreAttemptId::new_v7();
            let result = stage_workspace_archive_pass_two(
                &lock,
                &mut Cursor::new(writer.finish().unwrap()),
                attempt,
                correlation_id,
                limits,
                &CancellationSignal::new(),
            );
            if hostile {
                assert!(matches!(result, Err(RestoreImportError::SchemaMismatch)));
            } else {
                let staged = result.expect("published v15 archive restores");
                let restored = Connection::open_with_flags(
                    staged.database_path(),
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )
                .unwrap();
                assert_eq!(
                    restored
                        .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                        .unwrap(),
                    SCHEMA_VERSION
                );
                let restored_record: (String, String, String) = restored
                    .query_row(
                        "SELECT workspace_id, grain, status FROM records WHERE record_id = ?1",
                        [record.to_string()],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    )
                    .unwrap();
                assert_eq!(
                    restored_record,
                    (
                        workspace.to_string(),
                        "film".to_owned(),
                        "active".to_owned()
                    )
                );
                for table in ["workspaces", "profiles", "clients", "records"] {
                    assert_eq!(
                        restored
                            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                                .get::<_, i64>(0))
                            .unwrap(),
                        1,
                        "{table}"
                    );
                }
                for table in [
                    "search_pages",
                    "search_candidate_receipts",
                    "node_state",
                    "credentials",
                    "profile_grants",
                    "grant_scopes",
                    "auth_subjects",
                    "fasti_browser_sessions",
                ] {
                    assert_eq!(
                        restored
                            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                                .get::<_, i64>(0))
                            .unwrap(),
                        0,
                        "{table}"
                    );
                }
                drop(restored);
                staged.cleanup().unwrap();
            }
            assert!(!root
                .path()
                .join(RESTORE_STAGING_DIRECTORY)
                .join(attempt.to_string())
                .exists());
        }
    }

    fn seed_legacy_override_root(connection: &Connection, profile_count: usize) {
        connection
            .execute_batch(
                r#"
                INSERT INTO workspaces(workspace_id, created_at)
                    VALUES ('wsp_legacy_override', '2026-08-24T00:00:00Z');
                INSERT INTO records(record_id, workspace_id, grain, status, created_at)
                    VALUES (
                        'rec_legacy_override', 'wsp_legacy_override', 'film', 'active',
                        '2026-08-24T00:00:01Z'
                    );
                INSERT INTO metadata_field_overrides(
                    workspace_id, record_id, field_key, value, created_at
                ) VALUES (
                    'wsp_legacy_override', 'rec_legacy_override', 'core.title',
                    'The retained title', '2026-08-24T00:00:02Z'
                );
                "#,
            )
            .expect("seed legacy override");
        for index in 0..profile_count {
            connection
                .execute(
                    "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, 'wsp_legacy_override', ?2)",
                    params![format!("prf_legacy_override_{index}"), CREATED_AT],
                )
                .expect("seed profile");
        }
    }

    fn seed_version_nine_browser_state(connection: &Connection) {
        connection
            .execute_batch(
                r#"
                INSERT INTO workspaces(workspace_id, created_at)
                    VALUES ('wsp_v9', '2026-08-24T00:00:00Z');
                INSERT INTO profiles(profile_id, workspace_id, created_at)
                    VALUES ('prf_v9', 'wsp_v9', '2026-08-24T00:00:01Z');
                INSERT INTO clients(client_id, workspace_id, status,
                                    current_credential_epoch, created_at)
                    VALUES ('cli_v9', 'wsp_v9', 'active', 1,
                            '2026-08-24T00:00:02Z');
                INSERT INTO browser_users(
                    user_id, username, password_hash, client_id, profile_id,
                    is_admin, is_test_account, active, failed_login_count,
                    created_at, updated_at
                ) VALUES (
                    'usr_v9', 'developer', 'removed-pr-only-digest', 'cli_v9',
                    'prf_v9', 1, 1, 1, 0, '2026-08-24T00:00:03Z',
                    '2026-08-24T00:00:03Z'
                );
                INSERT INTO browser_sessions(
                    session_digest, csrf_digest, user_id, expires_at,
                    created_at, last_seen_at
                ) VALUES (
                    'session-v9', 'csrf-v9', 'usr_v9',
                    '2026-08-24T01:00:00Z', '2026-08-24T00:00:04Z',
                    '2026-08-24T00:00:04Z'
                );
                INSERT INTO records(record_id, workspace_id, grain, status, created_at)
                    VALUES ('rec_v9', 'wsp_v9', 'film', 'active',
                            '2026-08-24T00:00:05Z');
                "#,
            )
            .expect("seed populated version-nine developer root");
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
        // (migrate_v7), +3 for profile Nuvio Collections (migrate_v9), and
        // +3 for provider capability state (migrate_v11), +27 for the nine
        // authoritative metadata tables (migrate_v12), and +3 for immutable
        // metadata refresh receipts (migrate_v13), plus +15 for the five M3
        // identity and anime-policy tables (migrate_v15), +3 for durable
        // Search action receipts (migrate_v16). Disposable
        // projection and cache tables do not advance the workspace revision,
        // none of which are in the
        // original REVISION_SOURCES list built for the v3 schema snapshot.
        assert_eq!(
            trigger_count,
            (REVISION_SOURCES.len() * 3 + 3 + 6 + 3 + 3 + 3 + 27 + 3 + 15 + 3) as i64
        );
    }

    #[test]
    fn version_twelve_upgrades_through_append_only_refresh_receipts() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_twelve(&connection);

        let before: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read v12");
        let receipts_before: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'metadata_refresh_receipts'",
                [],
                |row| row.get(0),
            )
            .expect("inspect v12 tables");
        assert_eq!((before, receipts_before), (12, 0));

        migrate(&connection).expect("upgrade v12 to v13");

        let after: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read v13");
        let columns = connection
            .prepare("PRAGMA table_info(metadata_refresh_receipts)")
            .expect("receipt columns")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query receipt columns")
            .collect::<Result<Vec<_>>>()
            .expect("collect receipt columns");
        let triggers: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND tbl_name = 'metadata_refresh_receipts'",
                [],
                |row| row.get(0),
            )
            .expect("count receipt triggers");
        assert_eq!(after, SCHEMA_VERSION);
        assert_eq!(
            columns,
            [
                "workspace_id",
                "profile_id",
                "client_id",
                "operation_id",
                "semantic_digest",
                "record_id",
                "provider_id",
                "response_json",
                "created_at",
            ]
        );
        assert_eq!(triggers, 6);
    }

    #[test]
    fn version_thirteen_upgrades_to_node_local_access_without_archive_triggers() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_thirteen(&connection);

        let before: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read v13");
        assert_eq!(before, 13);
        for table in [
            "trailbase_installation",
            "trailbase_auth_anchors",
            "workspace_memberships",
            "auth_ceremonies",
            "fasti_browser_session_authentication",
            "access_audit_events",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("inspect v13 tables");
            assert_eq!(count, 0, "{table} must not exist in published v13");
        }

        migrate_v14(&connection).expect("upgrade v13 to v14");

        let after: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read v14");
        assert_eq!(after, 14);
        for table in [
            "trailbase_installation",
            "trailbase_auth_anchors",
            "workspace_memberships",
            "auth_ceremonies",
            "fasti_browser_session_authentication",
            "access_audit_events",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("inspect v14 tables");
            assert_eq!(count, 1, "{table}");
        }
        let revision_triggers: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'trigger' AND name LIKE 'workspace_revision_%' AND tbl_name IN ('trailbase_installation', 'trailbase_auth_anchors', 'workspace_memberships', 'auth_ceremonies', 'fasti_browser_session_authentication', 'access_audit_events')",
                [],
                |row| row.get(0),
            )
            .expect("count Access revision triggers");
        assert_eq!(revision_triggers, 0);
        let active_bootstrap_index: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'index' AND name = 'auth_ceremonies_active_bootstrap_idx'",
                [],
                |row| row.get(0),
            )
            .expect("count active bootstrap index");
        assert_eq!(active_bootstrap_index, 1);
    }

    #[test]
    fn version_fourteen_upgrades_to_append_only_identity_and_anime_policy_state() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_fourteen(&connection);

        let before: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read v14");
        assert_eq!(before, 14);
        for table in [
            "identity_assertions",
            "identity_assertion_lifecycle_events",
            "profile_anime_grouping_policies",
            "client_anime_grouping_policies",
            "anime_grouping_policy_receipts",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("inspect v14 tables");
            assert_eq!(count, 0, "{table} must not exist in published v14");
        }

        migrate_v15(&connection).expect("upgrade v14 to v15");

        let after: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read v15");
        assert_eq!(after, 15);
        for table in [
            "identity_assertions",
            "identity_assertion_lifecycle_events",
            "profile_anime_grouping_policies",
            "client_anime_grouping_policies",
            "anime_grouping_policy_receipts",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("inspect v15 tables");
            assert_eq!(count, 1, "{table}");
        }
        let revision_triggers: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'trigger' AND name LIKE 'workspace_revision_%' AND tbl_name IN ('identity_assertions', 'identity_assertion_lifecycle_events', 'profile_anime_grouping_policies', 'client_anime_grouping_policies', 'anime_grouping_policy_receipts')",
                [],
                |row| row.get(0),
            )
            .expect("count M3 revision triggers");
        assert_eq!(revision_triggers, 15);
        let records_index: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'index' AND name = 'records_workspace_record_idx'",
                [],
                |row| row.get(0),
            )
            .expect("count Record keyset index");
        assert_eq!(records_index, 1);
        let operation_uniqueness: i64 = connection
            .query_row(
                r#"
                SELECT COUNT(*)
                FROM pragma_index_list('anime_grouping_policy_receipts') AS index_list
                WHERE index_list."unique" = 1
                  AND (
                    SELECT group_concat(index_info.name, ',')
                    FROM pragma_index_info(index_list.name) AS index_info
                  ) = 'workspace_id,operation_id'
                "#,
                [],
                |row| row.get(0),
            )
            .expect("inspect operation uniqueness");
        assert_eq!(operation_uniqueness, 1);
    }

    #[test]
    fn version_fourteen_constraints_preserve_membership_ceremony_and_audit_invariants() {
        let connection = migrated_connection();
        let workspace_id = fasti_domain::WorkspaceId::new_v7().to_string();
        let subject_id = fasti_domain::AuthSubjectId::new_v7().to_string();
        let instance_id = fasti_domain::TrailBaseInstanceId::new_v7().to_string();
        let membership_id = fasti_domain::MembershipId::new_v7().to_string();
        let next_membership_id = fasti_domain::MembershipId::new_v7().to_string();
        let grant_id = fasti_domain::ProfileGrantId::new_v7().to_string();
        let operation_id = fasti_domain::OperationId::new_v7().to_string();
        let correlation_id = fasti_domain::RequestCorrelationId::new_v7().to_string();
        let root_digest = format!("sha256:{}", "11".repeat(32));
        let release_lock_digest = format!("sha256:{}", "22".repeat(32));
        let binding_digest = format!("sha256:{}", "22".repeat(32));
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![workspace_id, CREATED_AT],
            )
            .expect("workspace");
        connection
            .execute(
                "INSERT INTO auth_subjects(auth_subject_id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at) VALUES (?1, 'active', 0, 0, ?2, ?2)",
                params![subject_id, CREATED_AT],
            )
            .expect("subject");
        connection
            .execute(
                "INSERT INTO trailbase_installation(singleton, trailbase_instance_id, physical_root_identity, release_lock_identity, activation_state, activation_blocker, activation_generation, created_at, updated_at) VALUES (1, ?1, ?2, NULL, 'inactive', NULL, 0, ?3, ?3)",
                params![instance_id, root_digest, CREATED_AT],
            )
            .expect("inactive installation may omit release lock identity");
        connection
            .execute("DELETE FROM trailbase_installation", [])
            .expect("replace inactive constraint fixture");
        connection
            .execute(
                "INSERT INTO trailbase_installation(singleton, trailbase_instance_id, physical_root_identity, release_lock_identity, activation_state, activation_blocker, activation_generation, created_at, updated_at) VALUES (1, ?1, ?2, NULL, 'blocked', 'declared_restore', 0, ?3, ?3)",
                params![instance_id, root_digest, CREATED_AT],
            )
            .expect("blocked installation may omit release lock identity");
        connection
            .execute("DELETE FROM trailbase_installation", [])
            .expect("replace blocked constraint fixture");
        assert!(connection
            .execute(
                "INSERT INTO trailbase_installation(singleton, trailbase_instance_id, physical_root_identity, activation_state, activation_blocker, activation_generation, created_at, updated_at) VALUES (1, ?1, ?2, 'active', NULL, 1, ?3, ?3)",
                params![instance_id, root_digest, CREATED_AT],
            )
            .is_err());
        connection
            .execute(
                "INSERT INTO trailbase_installation(singleton, trailbase_instance_id, physical_root_identity, release_lock_identity, activation_state, activation_blocker, activation_generation, created_at, updated_at) VALUES (1, ?1, ?2, ?3, 'active', NULL, 1, ?4, ?4)",
                params![instance_id, root_digest, release_lock_digest, CREATED_AT],
            )
            .expect("installation");
        connection
            .execute(
                "INSERT INTO trailbase_auth_anchors(trailbase_instance_id, trailbase_subject, auth_subject_id, linked_at) VALUES (?1, ?2, ?3, ?4)",
                params![instance_id, [7_u8; 16].as_slice(), subject_id, CREATED_AT],
            )
            .expect("anchor");
        assert!(connection
            .execute(
                "UPDATE trailbase_auth_anchors SET linked_at = ?1",
                ["2026-08-24T00:00:01.000000Z"],
            )
            .is_err());
        assert!(connection
            .execute("DELETE FROM trailbase_auth_anchors", [])
            .is_err());

        assert!(connection
            .execute(
                "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'invited', 'administrator', ?4, ?4)",
                params![membership_id, subject_id, workspace_id, CREATED_AT],
            )
            .is_err());
        connection
            .execute(
                "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'invited', 'member', ?4, ?4)",
                params![membership_id, subject_id, workspace_id, CREATED_AT],
            )
            .expect("invited membership");
        assert!(connection
            .execute(
                "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'member', ?4, ?4)",
                params![next_membership_id, subject_id, workspace_id, CREATED_AT],
            )
            .is_err());
        connection
            .execute(
                "UPDATE workspace_memberships SET lifecycle = 'removed', updated_at = '2026-08-24T00:00:01.000000Z' WHERE membership_id = ?1",
                [membership_id.as_str()],
            )
            .expect("remove membership");
        connection
            .execute(
                "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'invited', 'member', ?4, ?4)",
                params![next_membership_id, subject_id, workspace_id, CREATED_AT],
            )
            .expect("reinvite with new identity");
        assert!(connection
            .execute(
                "UPDATE workspace_memberships SET role = 'administrator' WHERE membership_id = ?1",
                [membership_id.as_str()],
            )
            .is_err());

        assert!(connection
            .execute(
                "INSERT INTO auth_ceremonies(operation_id, purpose, protocol, trailbase_instance_id, activation_generation, browser_binding_digest, workspace_id, selected_profile_grant_id, bound_browser_session_id, invited_membership_id, remembered, callback_path, return_target, correlation_id, state, failure, created_at, expires_at, claimed_at, terminal_at) VALUES (?1, 'sign_in', 'trailbase_authorization_code_pkce', ?2, 1, ?3, NULL, NULL, NULL, NULL, 0, '/auth/trailbase/callback', 'first_run', ?4, 'pending', NULL, ?5, '2026-08-24T00:05:00.000000Z', NULL, NULL)",
                params![operation_id, instance_id, binding_digest, correlation_id, CREATED_AT],
            )
            .is_err());
        connection
            .execute(
                "INSERT INTO auth_ceremonies(operation_id, purpose, protocol, trailbase_instance_id, activation_generation, browser_binding_digest, workspace_id, selected_profile_grant_id, bound_browser_session_id, invited_membership_id, remembered, callback_path, return_target, correlation_id, state, failure, created_at, expires_at, claimed_at, terminal_at) VALUES (?1, 'sign_in', 'trailbase_authorization_code_pkce', ?2, 1, ?3, NULL, NULL, NULL, NULL, 0, '/auth/trailbase/callback', 'application_home', ?4, 'pending', NULL, ?5, '2026-08-24T00:05:00.000000Z', NULL, NULL)",
                params![operation_id, instance_id, binding_digest, correlation_id, CREATED_AT],
            )
            .expect("valid pending ceremony");
        let browser_session_id = fasti_domain::BrowserSessionId::new_v7().to_string();
        assert!(connection
            .execute(
                "UPDATE auth_ceremonies SET bound_browser_session_id = ?1 WHERE operation_id = ?2",
                params![browser_session_id, operation_id],
            )
            .is_err());
        connection
            .execute(
                "UPDATE auth_ceremonies SET purpose = 'recent_authentication', return_target = 'account_security', workspace_id = ?1, selected_profile_grant_id = ?2, bound_browser_session_id = ?3, remembered = 0 WHERE operation_id = ?4",
                params![workspace_id, grant_id, browser_session_id, operation_id],
            )
            .expect("valid reserved recent-auth association");
        assert!(connection
            .execute(
                "UPDATE auth_ceremonies SET invited_membership_id = ?1 WHERE operation_id = ?2",
                params![next_membership_id, operation_id],
            )
            .is_err());
        connection
            .execute(
                "UPDATE auth_ceremonies SET purpose = 'sign_in', return_target = 'application_home', workspace_id = NULL, selected_profile_grant_id = NULL, bound_browser_session_id = NULL WHERE operation_id = ?1",
                [operation_id.as_str()],
            )
            .expect("restore sign-in association");
        assert!(connection
            .execute(
                "INSERT INTO auth_ceremonies(operation_id, purpose, protocol, trailbase_instance_id, activation_generation, browser_binding_digest, workspace_id, selected_profile_grant_id, bound_browser_session_id, invited_membership_id, remembered, callback_path, return_target, correlation_id, state, failure, created_at, expires_at, claimed_at, terminal_at) VALUES (?1, 'sign_in', 'trailbase_authorization_code_pkce', ?2, 1, ?3, NULL, NULL, NULL, NULL, 0, '/auth/trailbase/callback', 'application_home', ?4, 'pending', NULL, ?5, '2026-08-24T00:05:00.000000Z', NULL, NULL)",
                params![
                    fasti_domain::OperationId::new_v7().to_string(),
                    instance_id,
                    binding_digest,
                    correlation_id,
                    CREATED_AT,
                ],
            )
            .is_err());
        let lookup_plan: String = connection
            .query_row(
                "EXPLAIN QUERY PLAN SELECT operation_id FROM auth_ceremonies WHERE browser_binding_digest = ?1",
                [binding_digest.as_str()],
                |row| row.get(3),
            )
            .expect("browser binding lookup plan");
        assert!(lookup_plan.contains("auth_ceremonies_browser_binding_idx"));
        assert!(connection
            .execute(
                "UPDATE auth_ceremonies SET state = 'completed' WHERE operation_id = ?1",
                [operation_id.as_str()],
            )
            .is_err());
        connection
            .execute(
                "UPDATE auth_ceremonies SET state = 'cancelled', terminal_at = '2026-08-24T00:00:01.000000Z' WHERE operation_id = ?1",
                [operation_id.as_str()],
            )
            .expect("cancel pending ceremony");
        let claimed_operation_id = fasti_domain::OperationId::new_v7().to_string();
        let claimed_binding_digest = format!("sha256:{}", "33".repeat(32));
        connection
            .execute(
                "INSERT INTO auth_ceremonies(operation_id, purpose, protocol, trailbase_instance_id, activation_generation, browser_binding_digest, workspace_id, selected_profile_grant_id, bound_browser_session_id, invited_membership_id, remembered, callback_path, return_target, correlation_id, state, failure, created_at, expires_at, claimed_at, terminal_at) VALUES (?1, 'sign_in', 'trailbase_authorization_code_pkce', ?2, 1, ?3, NULL, NULL, NULL, NULL, 0, '/auth/trailbase/callback', 'application_home', ?4, 'claimed', NULL, ?5, '2026-08-24T00:05:00.000000Z', '2026-08-24T00:00:01.000000Z', NULL)",
                params![
                    claimed_operation_id,
                    instance_id,
                    claimed_binding_digest,
                    correlation_id,
                    CREATED_AT,
                ],
            )
            .expect("claimed ceremony");
        assert!(connection
            .execute(
                "UPDATE auth_ceremonies SET state = 'cancelled', terminal_at = '2026-08-24T00:00:02.000000Z' WHERE operation_id = ?1",
                [claimed_operation_id.as_str()],
            )
            .is_err());

        connection
            .execute(
                "INSERT INTO access_audit_events(event_kind, trailbase_instance_id, auth_subject_id, actor_auth_subject_id, workspace_id, membership_id, operation_id, browser_session_id, correlation_id, occurred_at) VALUES ('membership_invited', ?1, ?2, ?2, ?3, ?4, NULL, NULL, ?5, ?6)",
                params![instance_id, subject_id, workspace_id, next_membership_id, correlation_id, CREATED_AT],
            )
            .expect("audit event");
        assert!(connection
            .execute(
                "UPDATE access_audit_events SET event_kind = 'membership_demoted'",
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO access_audit_events(event_kind, correlation_id, occurred_at) VALUES ('membership_invited', ?1, ?2)",
                params![correlation_id, CREATED_AT],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO access_audit_events(event_kind, workspace_id, correlation_id, occurred_at) VALUES ('membership_invited', 'wsp_00000000000040008000000000000000', ?1, ?2)",
                params![correlation_id, CREATED_AT],
            )
            .is_err());
        connection
            .execute("DELETE FROM access_audit_events", [])
            .expect("retention pruning may delete audit events");
    }

    #[test]
    fn version_fourteen_migration_conflict_is_atomic_and_retryable() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_thirteen(&connection);
        connection
            .execute_batch("CREATE TABLE trailbase_installation (collision INTEGER) STRICT;")
            .expect("simulate schema collision");

        assert!(migrate(&connection).is_err());
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read version after rollback");
        let partial_tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name IN ('trailbase_auth_anchors', 'workspace_memberships', 'auth_ceremonies', 'fasti_browser_session_authentication', 'access_audit_events')",
                [],
                |row| row.get(0),
            )
            .expect("count rolled-back tables");
        assert_eq!((version, partial_tables), (13, 0));

        connection
            .execute_batch("DROP TABLE trailbase_installation;")
            .expect("remove test collision");
        migrate_v14(&connection).expect("retry v14 migration");
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .expect("read retried version"),
            14
        );
    }

    #[test]
    fn metadata_override_migration_retains_zero_profile_rows_for_review() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_eleven(&connection);
        seed_legacy_override_root(&connection, 0);

        migrate(&connection).expect("upgrade metadata schema");

        let ownership: (String, Option<String>, String) = connection
            .query_row(
                "SELECT state, owner_profile_id, review_reason FROM metadata_legacy_override_ownership",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("migration review row");
        assert_eq!(
            ownership,
            (
                "review_required".to_owned(),
                None,
                "zero_profiles".to_owned()
            )
        );
        assert_eq!(legacy_override_counts(&connection), (1, 0, 0));
    }

    #[test]
    fn metadata_override_migration_moves_one_unambiguous_owner_once() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_eleven(&connection);
        seed_legacy_override_root(&connection, 1);

        migrate(&connection).expect("upgrade metadata schema");
        migrate(&connection).expect("retry is a no-op");

        let ownership: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT state, owner_profile_id, review_reason FROM metadata_legacy_override_ownership",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("migration ownership");
        assert_eq!(
            ownership,
            (
                "migrated".to_owned(),
                Some("prf_legacy_override_0".to_owned()),
                None
            )
        );
        assert_eq!(legacy_override_counts(&connection), (1, 1, 1));
        let migrated: (String, String, String) = connection
            .query_row(
                "SELECT value, created_at, origin FROM metadata_profile_field_overrides",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("migrated override");
        assert_eq!(
            migrated,
            (
                "The retained title".to_owned(),
                "2026-08-24T00:00:02Z".to_owned(),
                "legacy_migration".to_owned()
            )
        );
    }

    #[test]
    fn metadata_override_migration_never_chooses_between_profiles() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_eleven(&connection);
        seed_legacy_override_root(&connection, 2);

        migrate(&connection).expect("upgrade metadata schema");

        let ownership: (String, Option<String>, String) = connection
            .query_row(
                "SELECT state, owner_profile_id, review_reason FROM metadata_legacy_override_ownership",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("migration review row");
        assert_eq!(
            ownership,
            (
                "review_required".to_owned(),
                None,
                "multiple_profiles".to_owned()
            )
        );
        assert_eq!(legacy_override_counts(&connection), (1, 0, 0));
    }

    fn legacy_override_counts(connection: &Connection) -> (i64, i64, i64) {
        connection
            .query_row(
                r#"
                SELECT
                    (SELECT COUNT(*) FROM metadata_field_overrides),
                    (SELECT COUNT(*) FROM metadata_profile_field_overrides),
                    (SELECT COUNT(*) FROM metadata_override_migration_receipts)
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("metadata override counts")
    }

    #[test]
    fn provider_state_migration_is_failure_atomic() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_ten(&connection);
        connection
            .execute_batch("CREATE TABLE provider_capability_states (incompatible TEXT) STRICT;")
            .expect("install incompatible table");

        migrate(&connection).expect_err("incompatible provider table must fail migration");

        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, 10);
        let columns: Vec<String> = connection
            .prepare("PRAGMA table_info(provider_capability_states)")
            .expect("table info")
            .query_map([], |row| row.get(1))
            .expect("column rows")
            .collect::<Result<_>>()
            .expect("columns");
        assert_eq!(columns, ["incompatible"]);
        let triggers: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name LIKE 'workspace_revision_provider_capability_states_%'",
                [],
                |row| row.get(0),
            )
            .expect("provider trigger count");
        assert_eq!(triggers, 0);
    }

    #[test]
    fn provider_state_migration_backfills_node_owner_provider_scopes() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_ten(&connection);
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
            .expect("seed enrolled version-ten node");

        migrate(&connection).expect("upgrade version-ten database");

        for scope in V11_NODE_OWNER_SCOPE_BACKFILL {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM grant_scopes WHERE grant_id = 'grt_seed' AND scope_key = ?1",
                    [scope_storage_key(*scope)],
                    |row| row.get(0),
                )
                .expect("query backfilled provider scope");
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn metadata_migration_backfills_node_owner_metadata_scopes() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_to_version_eleven(&connection);
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
            .expect("seed enrolled version-eleven node");

        migrate(&connection).expect("upgrade version-eleven database");

        for scope in [
            ScopeKey::MetadataClaimRefresh,
            ScopeKey::MetadataProjectionRead,
            ScopeKey::MetadataProjectionConfigure,
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM grant_scopes WHERE grant_id = 'grt_seed' AND scope_key = ?1",
                    [scope_storage_key(scope)],
                    |row| row.get(0),
                )
                .expect("query backfilled metadata scope");
            assert_eq!(count, 1);
        }
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
        assert_eq!(version, SCHEMA_VERSION);

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
        assert_eq!(version, 11, "the failed v12 transaction must retain v11");
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
        assert_eq!(state, (11, "tmdb".to_owned(), 1));
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
            (11, "chapter".to_owned(), "google-books".to_owned(), 0)
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
        assert_eq!(state, (11, "tmdb".to_owned(), 2, 0));
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
            assert_eq!(version, 11, "the failed v12 transaction must retain v11");
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
            (
                11,
                "series".to_owned(),
                "tmdb".to_owned(),
                "film".to_owned()
            )
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
    fn version_ten_replaces_browser_user_and_rejects_pr_only_factor_tables() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .expect("enable foreign keys");
        migrate_v1(&connection).expect("v1");
        migrate_v2(&connection).expect("v2");
        migrate_v3(&connection).expect("v3");
        migrate_v4(&connection).expect("v4");
        migrate_v5(&connection).expect("v5");
        migrate_v6(&connection).expect("v6");
        migrate_v7(&connection).expect("v7");
        migrate_v8(&connection).expect("v8");
        migrate_v9(&connection).expect("v9");
        connection
            .execute_batch(
                r#"
                CREATE TABLE user_passkeys (passkey_id TEXT PRIMARY KEY) STRICT;
                CREATE TABLE user_totp (user_id TEXT PRIMARY KEY) STRICT;
                CREATE TABLE user_backup_codes (code_hash TEXT PRIMARY KEY) STRICT;
                CREATE TABLE oidc_provider_configs (workspace_id TEXT PRIMARY KEY) STRICT;
                CREATE TABLE auth_ephemeral_challenges (challenge_id TEXT PRIMARY KEY) STRICT;
                "#,
            )
            .expect("simulate developer root created by the edited v8");

        migrate(&connection).expect("forward migration");

        for removed in [
            "browser_users",
            "browser_sessions",
            "browser_auth_bootstrap",
            "user_passkeys",
            "user_totp",
            "user_backup_codes",
            "oidc_provider_configs",
            "auth_ephemeral_challenges",
        ] {
            let exists: i64 = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
                    [removed],
                    |row| row.get(0),
                )
                .expect("table inventory");
            assert_eq!(exists, 0, "{removed} survived the truth-reset migration");
        }
        for retained in [
            "auth_subjects",
            "auth_subject_profile_grants",
            "fasti_browser_sessions",
            "fasti_browser_session_grants",
        ] {
            let exists: i64 = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1)",
                    [retained],
                    |row| row.get(0),
                )
                .expect("table inventory");
            assert_eq!(exists, 1, "{retained} is missing");
        }
    }

    #[test]
    fn version_ten_failed_forward_is_atomic_and_retryable() {
        let connection = version_nine_connection();
        seed_version_nine_browser_state(&connection);
        connection
            .execute_batch("CREATE TABLE auth_subjects (auth_subject_id TEXT PRIMARY KEY) STRICT;")
            .expect("inject a forward-migration conflict");

        migrate(&connection).expect_err("conflicting forward migration must fail");
        let state: (i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), (SELECT COUNT(*) FROM browser_users), (SELECT COUNT(*) FROM browser_sessions)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("failed-forward state");
        assert_eq!(state, (9, 1, 1), "v10 failure must roll back every drop");

        connection
            .execute("DROP TABLE auth_subjects", [])
            .expect("remove injected conflict");
        migrate(&connection).expect("retry forward migration");
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn version_ten_restart_and_old_binary_rollback_use_a_closed_copy() {
        let root = tempfile::tempdir().expect("temporary migration rehearsal");
        let version_nine = root.path().join("fasti-v9.sqlite3");
        let backup = root.path().join("fasti-v9.backup.sqlite3");
        let rollback = root.path().join("fasti-v9.rollback.sqlite3");
        {
            let connection = Connection::open(&version_nine).expect("version-nine database");
            connection
                .pragma_update(None, "foreign_keys", "ON")
                .expect("enable foreign keys");
            migrate_to_version_nine(&connection);
            seed_version_nine_browser_state(&connection);
        }
        fs::copy(&version_nine, &backup).expect("closed pre-migration backup");

        {
            let connection = Connection::open(&version_nine).expect("forward database");
            connection
                .pragma_update(None, "foreign_keys", "ON")
                .expect("enable foreign keys");
            migrate(&connection).expect("forward migration");
            let state: (i64, i64, i64) = connection
                .query_row(
                    "SELECT (SELECT user_version FROM pragma_user_version), (SELECT COUNT(*) FROM records WHERE record_id = 'rec_v9'), (SELECT COUNT(*) FROM auth_subjects)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("forward state");
            assert_eq!(state, (SCHEMA_VERSION, 1, 0));
        }
        {
            let connection = Connection::open(&version_nine).expect("restart database");
            migrate(&connection).expect("restart migration is idempotent");
            let version: i64 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("restart schema version");
            assert_eq!(version, SCHEMA_VERSION);
        }

        fs::copy(&backup, &rollback).expect("restore old-binary rollback copy");
        let connection = Connection::open(&rollback).expect("rollback database");
        let state: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT user_version FROM pragma_user_version), (SELECT COUNT(*) FROM browser_users WHERE user_id = 'usr_v9'), (SELECT COUNT(*) FROM browser_sessions WHERE user_id = 'usr_v9'), (SELECT COUNT(*) FROM records WHERE record_id = 'rec_v9')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("restored version-nine state");
        assert_eq!(state, (9, 1, 1, 1));
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
        // tracking table, migrate_v9's Nuvio Collections table, v10's final
        // dormant Access tables, and v11's provider capability state. V10
        // deliberately replaces the unsupported browser-user tables
        // introduced by v8.
        let expected_tables_after: Vec<String> = tables_before
            .iter()
            .cloned()
            .chain([
                "metadata_field_claims".to_owned(),
                "metadata_field_overrides".to_owned(),
                "profile_record_tracking_dispositions".to_owned(),
                "profile_nuvio_collections".to_owned(),
                "auth_subject_profile_grants".to_owned(),
                "auth_subjects".to_owned(),
                "fasti_browser_session_grants".to_owned(),
                "fasti_browser_sessions".to_owned(),
                "provider_capability_states".to_owned(),
                "metadata_claims".to_owned(),
                "metadata_claim_provenance".to_owned(),
                "metadata_rating_claims".to_owned(),
                "metadata_claim_lifecycle_events".to_owned(),
                "metadata_projection_policies".to_owned(),
                "metadata_profile_field_overrides".to_owned(),
                "metadata_legacy_override_ownership".to_owned(),
                "metadata_override_migration_receipts".to_owned(),
                "metadata_projections".to_owned(),
                "metadata_attributions".to_owned(),
                "metadata_cache_entries".to_owned(),
                "metadata_cache_claims".to_owned(),
                "metadata_refresh_receipts".to_owned(),
                "local_search_grams".to_owned(),
                "search_pages".to_owned(),
                "search_candidate_receipts".to_owned(),
                "search_action_receipts".to_owned(),
                "trailbase_installation".to_owned(),
                "trailbase_auth_anchors".to_owned(),
                "workspace_memberships".to_owned(),
                "auth_ceremonies".to_owned(),
                "fasti_browser_session_authentication".to_owned(),
                "access_audit_events".to_owned(),
                "identity_assertions".to_owned(),
                "identity_assertion_lifecycle_events".to_owned(),
                "profile_anime_grouping_policies".to_owned(),
                "client_anime_grouping_policies".to_owned(),
                "anime_grouping_policy_receipts".to_owned(),
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
    fn derived_metadata_tables_do_not_advance_the_authoritative_revision() {
        let connection = migrated_connection();
        for table in [
            "metadata_projections",
            "metadata_cache_entries",
            "metadata_cache_claims",
        ] {
            let triggers: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'trigger' AND name LIKE ?1",
                    [format!("workspace_revision_{table}_%")],
                    |row| row.get(0),
                )
                .expect("count derived revision triggers");
            assert_eq!(triggers, 0, "{table} is disposable derived state");
        }
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
