mod search_scope_migration_tests {
    use super::*;

    fn enrolled_v15() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        migrate_to_version_fourteen(&connection);
        migrate_v15(&connection).unwrap();
        let workspace = fasti_domain::WorkspaceId::new_v7().to_string();
        let profile = fasti_domain::ProfileId::new_v7().to_string();
        let client = fasti_domain::ClientId::new_v7().to_string();
        let grant = fasti_domain::ProfileGrantId::new_v7().to_string();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![workspace, CREATED_AT],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                params![profile, workspace, CREATED_AT],
            )
            .unwrap();
        connection.execute("INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 7, ?3)", params![client, workspace, CREATED_AT]).unwrap();
        connection.execute("INSERT INTO profile_grants(grant_id, workspace_id, profile_id, client_id, status, created_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5)", params![grant, workspace, profile, client, CREATED_AT]).unwrap();
        connection
            .execute(
                "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, 'identity_read')",
                [&grant],
            )
            .unwrap();
        connection.execute("INSERT INTO node_state(singleton, initialized, workspace_id, profile_id, client_id, initialization_consumed_at, created_at) VALUES (1, 1, ?1, ?2, ?3, ?4, ?4)", params![workspace, profile, client, CREATED_AT]).unwrap();
        connection
    }

    fn owner_grant(connection: &Connection) -> String {
        connection.query_row("SELECT pg.grant_id FROM profile_grants pg JOIN node_state ns ON pg.workspace_id = ns.workspace_id AND pg.profile_id = ns.profile_id AND pg.client_id = ns.client_id WHERE ns.singleton = 1", [], |row| row.get(0)).unwrap()
    }

    fn search_grants(connection: &Connection) -> Vec<String> {
        connection.prepare("SELECT grant_id FROM grant_scopes WHERE scope_key = 'metadata_search' ORDER BY grant_id").unwrap().query_map([], |row| row.get(0)).unwrap().collect::<Result<Vec<_>>>().unwrap()
    }

    #[test]
    fn v16_search_scope_backfills_only_enrolled_node_owner() {
        let connection = enrolled_v15();
        let owner = owner_grant(&connection);
        // Delegated client, same profile; original client, different profile.
        connection.execute_batch("INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) SELECT 'cli_delegate', workspace_id, 'active', 1, created_at FROM node_state; INSERT INTO profile_grants(grant_id, workspace_id, profile_id, client_id, status, created_at) SELECT 'grt_delegate', workspace_id, profile_id, 'cli_delegate', 'active', created_at FROM node_state; INSERT INTO profiles(profile_id, workspace_id, created_at) SELECT 'prf_other', workspace_id, created_at FROM node_state; INSERT INTO profile_grants(grant_id, workspace_id, profile_id, client_id, status, created_at) SELECT 'grt_other_profile', workspace_id, 'prf_other', client_id, 'active', created_at FROM node_state; INSERT INTO grant_scopes(grant_id, scope_key) VALUES ('grt_delegate', 'identity_read'), ('grt_other_profile', 'identity_read');").unwrap();
        assert!(search_grants(&connection).is_empty());
        migrate_v16(&connection).unwrap();
        assert_eq!(search_grants(&connection), [owner]);
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM grant_scopes WHERE scope_key = 'identity_read'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            3
        );
    }

    #[test]
    fn v16_search_scope_preserves_c1_subject_link_and_authorization_epochs() {
        let connection = enrolled_v15();
        let owner = owner_grant(&connection);
        let subject = fasti_domain::AuthSubjectId::new_v7().to_string();
        let membership = fasti_domain::MembershipId::new_v7().to_string();
        // Historical C1 rows: bootstrap links the existing owner grant, rather
        // than creating a separate human-owned grant or scope set.
        connection.execute("INSERT INTO auth_subjects(auth_subject_id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at) VALUES (?1, 'active', 3, 5, ?2, ?2)", params![subject, CREATED_AT]).unwrap();
        connection.execute("INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)", params![subject, owner]).unwrap();
        connection.execute("INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) SELECT ?1, ?2, workspace_id, 'active', 'administrator', ?3, ?3 FROM node_state", params![membership, subject, CREATED_AT]).unwrap();
        migrate_v16(&connection).unwrap();
        assert_eq!(search_grants(&connection), std::slice::from_ref(&owner));
        let actual: (String, i64, i64, String, String, i64) = connection.query_row("SELECT link.profile_grant_id, subject.auth_epoch, subject.authorization_epoch, membership.lifecycle, membership.role, client.current_credential_epoch FROM auth_subjects subject JOIN auth_subject_profile_grants link ON link.auth_subject_id = subject.auth_subject_id JOIN workspace_memberships membership ON membership.auth_subject_id = subject.auth_subject_id JOIN profile_grants pg ON pg.grant_id = link.profile_grant_id JOIN clients client ON client.client_id = pg.client_id WHERE subject.auth_subject_id = ?1", [&subject], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))).unwrap();
        assert_eq!(
            actual,
            (owner, 3, 5, "active".into(), "administrator".into(), 7)
        );
    }

    #[test]
    fn v16_search_scope_does_not_activate_revoked_or_provisional_owners() {
        for (name, mutation) in [
            (
                "revoked grant",
                "UPDATE profile_grants SET status = 'revoked'",
            ),
            ("revoked client", "UPDATE clients SET status = 'revoked'"),
            ("unenrolled", "UPDATE node_state SET initialized = 0"),
            (
                "provisional enrollment",
                "UPDATE node_state SET initialization_consumed_at = NULL",
            ),
            (
                "pending recovery",
                "UPDATE node_state SET recovery_restore_attempt_id = 'rst_pending_fixture'",
            ),
        ] {
            let connection = enrolled_v15();
            connection.execute_batch(mutation).unwrap();
            migrate_v16(&connection).unwrap();
            assert!(search_grants(&connection).is_empty(), "{name}");
            assert_eq!(
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM grant_scopes WHERE scope_key = 'identity_read'",
                        [],
                        |row| row.get::<_, i64>(0)
                    )
                    .unwrap(),
                1,
                "{name}"
            );
        }
    }

    #[test]
    fn v16_search_scope_preserves_existing_scope_without_readding_removed_scope_on_open() {
        let connection = enrolled_v15();
        let owner = owner_grant(&connection);
        connection
            .execute(
                "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, 'metadata_search')",
                [&owner],
            )
            .unwrap();
        migrate_v16(&connection).unwrap();
        migrate(&connection).unwrap();
        assert_eq!(search_grants(&connection), [owner]);
        connection
            .execute(
                "DELETE FROM grant_scopes WHERE scope_key = 'metadata_search'",
                [],
            )
            .unwrap();
        migrate(&connection).unwrap();
        assert!(
            search_grants(&connection).is_empty(),
            "ordinary reopen must not regrant a removed permission"
        );
    }

    #[test]
    fn v16_search_scope_failure_rolls_back_schema_and_grant_then_retries() {
        let connection = enrolled_v15();
        let owner = owner_grant(&connection);
        connection.execute_batch("CREATE TRIGGER reject_search_scope_fixture AFTER INSERT ON grant_scopes WHEN NEW.scope_key = 'metadata_search' BEGIN SELECT RAISE(ABORT, 'fixture rejects Search scope'); END;").unwrap();
        let error = migrate_v16(&connection)
            .expect_err("scope write must be part of the migration transaction");
        assert!(error.to_string().contains("fixture rejects Search scope"));
        assert!(connection.is_autocommit());
        assert!(search_grants(&connection).is_empty());
        assert_eq!(
            connection
                .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            15
        );
        assert_eq!(connection.query_row("SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('search_pages', 'local_search_grams', 'metadata_claim_provenance_recent_idx')", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
        connection
            .execute_batch("DROP TRIGGER reject_search_scope_fixture")
            .unwrap();
        migrate_v16(&connection).unwrap();
        assert_eq!(search_grants(&connection), [owner]);
    }
}
