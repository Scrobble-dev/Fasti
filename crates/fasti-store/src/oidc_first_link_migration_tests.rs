mod oidc_first_link_migration_tests {
    use super::*;
    use fasti_domain::{
        AuthSubjectId, MembershipId, OperationId, RequestCorrelationId, TrailBaseInstanceId,
        WorkspaceId,
    };

    fn version_eighteen_connection() -> Connection {
        let connection = version_seventeen_connection();
        migrate_v18(&connection).unwrap();
        connection
    }

    struct BootstrapHistory {
        instance: String,
        subject: String,
        workspace: String,
        membership: String,
    }

    fn bootstrap_history(connection: &Connection) -> BootstrapHistory {
        let history = BootstrapHistory {
            instance: TrailBaseInstanceId::new_v7().to_string(),
            subject: AuthSubjectId::new_v7().to_string(),
            workspace: WorkspaceId::new_v7().to_string(),
            membership: MembershipId::new_v7().to_string(),
        };
        connection.execute(
            "INSERT INTO trailbase_installation(singleton, trailbase_instance_id, physical_root_identity, activation_state, activation_generation, created_at, updated_at) VALUES (1, ?1, ?2, 'inactive', 0, ?3, ?3)",
            params![history.instance, format!("sha256:{}", "a".repeat(64)), CREATED_AT],
        ).unwrap();
        insert_subject(connection, &history.subject);
        connection.execute(
            "INSERT INTO trailbase_auth_anchors(trailbase_instance_id, trailbase_subject, auth_subject_id, linked_at) VALUES (?1, zeroblob(16), ?2, ?3)",
            params![history.instance, history.subject, CREATED_AT],
        ).unwrap();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![history.workspace, CREATED_AT],
            )
            .unwrap();
        connection.execute(
            "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'administrator', ?4, ?4)",
            params![history.membership, history.subject, history.workspace, CREATED_AT],
        ).unwrap();
        history
    }

    fn insert_subject(connection: &Connection, subject: &str) {
        connection.execute(
            "INSERT INTO auth_subjects(auth_subject_id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at) VALUES (?1, 'active', 0, 0, ?2, ?2)",
            params![subject, CREATED_AT],
        ).unwrap();
    }

    fn insert_bootstrap_audit(connection: &Connection, history: &BootstrapHistory, subject: &str) {
        connection.execute(
            "INSERT INTO access_audit_events(event_kind, trailbase_instance_id, auth_subject_id, workspace_id, membership_id, operation_id, correlation_id, occurred_at) VALUES ('first_administrator_bootstrapped', ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![history.instance, subject, history.workspace, history.membership, OperationId::new_v7().to_string(), RequestCorrelationId::new_v7().to_string(), CREATED_AT],
        ).unwrap();
    }

    fn eligibility(connection: &Connection) -> Vec<(String, String, Option<String>)> {
        connection.prepare("SELECT trailbase_instance_id, original_administrator_subject_id, consumed_by FROM first_oidc_link_eligibility ORDER BY trailbase_instance_id").unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap()
            .collect::<std::result::Result<Vec<_>, _>>().unwrap()
    }

    #[test]
    fn published_v18_schema_fingerprint() {
        let fingerprint = crate::portability::schema_fingerprint(
            &version_eighteen_connection(),
            RequestCorrelationId::new_v7(),
        )
        .unwrap();
        assert_eq!(fingerprint.migration_version(), 18);
        assert_eq!(
            fingerprint.digest().as_str(),
            "sha256:dc37c51ed8566673dff5523c20d6e201811bfac1f8592ed736bbbd3c26f90bdf"
        );
    }

    #[test]
    fn v19_backfills_only_unambiguous_matching_original_bootstrap_history() {
        for case in [
            "single",
            "missing",
            "duplicate",
            "conflicting",
            "mismatched",
        ] {
            let connection = version_eighteen_connection();
            let history = bootstrap_history(&connection);
            let other_subject = AuthSubjectId::new_v7().to_string();
            insert_subject(&connection, &other_subject);
            if case != "missing" && case != "mismatched" {
                insert_bootstrap_audit(&connection, &history, &history.subject);
            }
            if case == "duplicate" {
                insert_bootstrap_audit(&connection, &history, &history.subject);
            }
            if case == "conflicting" || case == "mismatched" {
                // A valid audit ID and existing subject do not establish the
                // immutable same-installation anchor. Count conflicting raw
                // events before filtering the matching anchor.
                insert_bootstrap_audit(&connection, &history, &other_subject);
            }
            migrate_v19(&connection).unwrap();
            let expected = if case == "single" {
                vec![(history.instance, history.subject, None)]
            } else {
                vec![]
            };
            assert_eq!(eligibility(&connection), expected, "{case}");
        }
    }

    #[test]
    fn v19_history_does_not_depend_on_current_administrator_role_or_lifecycle() {
        let connection = version_eighteen_connection();
        let history = bootstrap_history(&connection);
        insert_bootstrap_audit(&connection, &history, &history.subject);
        connection
            .execute(
                "UPDATE workspace_memberships SET role = 'member', lifecycle = 'suspended'",
                [],
            )
            .unwrap();
        connection.execute("UPDATE auth_subjects SET lifecycle = 'disabled', auth_epoch = 1, authorization_epoch = 1", []).unwrap();
        migrate_v19(&connection).unwrap();
        assert_eq!(
            eligibility(&connection),
            vec![(history.instance, history.subject, None)]
        );
    }

    #[test]
    fn v19_eligibility_is_permanent_and_consumption_is_one_way() {
        let connection = version_eighteen_connection();
        let history = bootstrap_history(&connection);
        insert_bootstrap_audit(&connection, &history, &history.subject);
        migrate_v19(&connection).unwrap();
        let initial = eligibility(&connection);
        assert!(connection.query_row(
            "SELECT strict = 1 AND wr = 1 FROM pragma_table_list WHERE name = 'first_oidc_link_eligibility'", [], |row| row.get::<_, bool>(0),
        ).unwrap());

        // recursive_triggers is not required for replacement safety.
        connection
            .pragma_update(None, "recursive_triggers", "OFF")
            .unwrap();
        for verb in ["INSERT", "INSERT OR REPLACE", "REPLACE"] {
            assert!(connection.execute(
                &format!("{verb} INTO first_oidc_link_eligibility(trailbase_instance_id, original_administrator_subject_id, consumed_by) VALUES (?1, ?2, NULL)"),
                params![history.instance, history.subject],
            ).is_err(), "{verb}");
            assert_eq!(eligibility(&connection), initial);
        }
        for sql in [
            "DELETE FROM first_oidc_link_eligibility",
            "UPDATE first_oidc_link_eligibility SET trailbase_instance_id = 'changed'",
            "UPDATE first_oidc_link_eligibility SET original_administrator_subject_id = 'changed'",
            "UPDATE first_oidc_link_eligibility SET consumed_by = NULL",
        ] {
            assert!(connection.execute(sql, []).is_err(), "{sql}");
            assert_eq!(eligibility(&connection), initial);
        }
        for invalid_operation in [
            "",
            "op_invalid",
            "op_00000000000040008000000000000000",
            "op_00000000000070007000000000000000",
            "op_0000000000007000800000000000000A",
        ] {
            assert!(
                connection
                    .execute(
                        "UPDATE first_oidc_link_eligibility SET consumed_by = ?1",
                        [invalid_operation]
                    )
                    .is_err(),
                "{invalid_operation}"
            );
            assert_eq!(eligibility(&connection), initial);
        }

        let replacement_subject = AuthSubjectId::new_v7().to_string();
        insert_subject(&connection, &replacement_subject);
        // Both the replacement subject and operation are otherwise valid.
        // A combined identity/consumption write must still fail.
        assert!(connection.execute(
            "UPDATE first_oidc_link_eligibility SET original_administrator_subject_id = ?1, consumed_by = ?2",
            params![replacement_subject, OperationId::new_v7().to_string()],
        ).is_err());
        assert_eq!(eligibility(&connection), initial);

        let consumed_by = OperationId::new_v7().to_string();
        assert_eq!(connection.execute("UPDATE first_oidc_link_eligibility SET consumed_by = ?1 WHERE consumed_by IS NULL", [&consumed_by]).unwrap(), 1);
        let consumed = vec![(history.instance, history.subject, Some(consumed_by.clone()))];
        assert_eq!(eligibility(&connection), consumed);
        for next in [
            None,
            Some(consumed_by),
            Some(OperationId::new_v7().to_string()),
        ] {
            assert!(connection
                .execute(
                    "UPDATE first_oidc_link_eligibility SET consumed_by = ?1",
                    [next]
                )
                .is_err());
            assert_eq!(eligibility(&connection), consumed);
        }
        assert!(connection
            .execute("DELETE FROM first_oidc_link_eligibility", [])
            .is_err());
        assert!(connection.execute(
            "INSERT OR REPLACE INTO first_oidc_link_eligibility(trailbase_instance_id, original_administrator_subject_id, consumed_by) VALUES (?1, ?2, NULL)",
            params![consumed[0].0, consumed[0].1],
        ).is_err());
        assert_eq!(eligibility(&connection), consumed);
        connection
            .execute("DELETE FROM access_audit_events", [])
            .unwrap();
        connection.execute("UPDATE trailbase_installation SET activation_state = 'blocked', activation_blocker = 'declared_restore', activation_generation = 1", []).unwrap();
        assert_eq!(eligibility(&connection), consumed);
    }

    #[test]
    fn v19_rejects_missing_or_mismatched_anchor_on_new_eligibility() {
        let connection = version_eighteen_connection();
        let history = bootstrap_history(&connection);
        let unanchored_subject = AuthSubjectId::new_v7().to_string();
        insert_subject(&connection, &unanchored_subject);
        migrate_v19(&connection).unwrap();
        for (instance, subject) in [
            (history.instance.clone(), unanchored_subject),
            (
                history.instance.clone(),
                AuthSubjectId::new_v7().to_string(),
            ),
            (
                TrailBaseInstanceId::new_v7().to_string(),
                history.subject.clone(),
            ),
        ] {
            assert!(connection.execute(
                "INSERT INTO first_oidc_link_eligibility(trailbase_instance_id, original_administrator_subject_id) VALUES (?1, ?2)",
                params![instance, subject],
            ).is_err());
            assert!(eligibility(&connection).is_empty());
        }
        connection.execute(
            "INSERT INTO first_oidc_link_eligibility(trailbase_instance_id, original_administrator_subject_id) VALUES (?1, ?2)",
            params![history.instance, history.subject],
        ).unwrap();
        assert_eq!(
            eligibility(&connection),
            vec![(history.instance, history.subject, None)]
        );
    }

    #[test]
    fn v19_consumption_rolls_back_with_its_enclosing_transaction() {
        let connection = version_eighteen_connection();
        let history = bootstrap_history(&connection);
        insert_bootstrap_audit(&connection, &history, &history.subject);
        migrate_v19(&connection).unwrap();
        let before = eligibility(&connection);
        {
            let transaction =
                Transaction::new_unchecked(&connection, TransactionBehavior::Immediate).unwrap();
            transaction.execute("UPDATE first_oidc_link_eligibility SET consumed_by = ?1 WHERE consumed_by IS NULL", [OperationId::new_v7().to_string()]).unwrap();
            // A later failure must not publish consumption independently.
            assert!(transaction
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![history.workspace, CREATED_AT]
                )
                .is_err());
            transaction.rollback().unwrap();
        }
        assert_eq!(eligibility(&connection), before);
    }

    #[test]
    fn v19_failed_migration_rolls_back_ddl_and_preserves_bootstrap_history() {
        let connection = version_eighteen_connection();
        let history = bootstrap_history(&connection);
        insert_bootstrap_audit(&connection, &history, &history.subject);
        connection.execute_batch(
            "CREATE TRIGGER first_oidc_link_eligibility_insert_guard BEFORE INSERT ON workspaces BEGIN SELECT RAISE(ABORT, 'injected migration name collision'); END;",
        ).unwrap();
        let correlation = RequestCorrelationId::new_v7();
        let before = crate::portability::schema_fingerprint(&connection, correlation).unwrap();
        assert!(migrate_v19(&connection).is_err());
        let after = crate::portability::schema_fingerprint(&connection, correlation).unwrap();
        assert_eq!(after.migration_version(), 18);
        assert_eq!(after.digest(), before.digest());
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'first_oidc_link_eligibility'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(connection.query_row(
            "SELECT COUNT(*) FROM access_audit_events WHERE event_kind = 'first_administrator_bootstrapped'", [], |row| row.get::<_, i64>(0),
        ).unwrap(), 1);
        connection
            .execute_batch("DROP TRIGGER first_oidc_link_eligibility_insert_guard;")
            .unwrap();
        migrate_v19(&connection).unwrap();
        assert_eq!(
            eligibility(&connection),
            vec![(history.instance, history.subject, None)]
        );
        let retried = crate::portability::schema_fingerprint(&connection, correlation).unwrap();
        let fresh =
            crate::portability::schema_fingerprint(&migrated_connection(), correlation).unwrap();
        assert_eq!(retried.digest(), fresh.digest());
    }

    #[test]
    fn v19_upgrade_matches_fresh_schema() {
        let connection = version_eighteen_connection();
        migrate_v19(&connection).unwrap();
        let correlation = RequestCorrelationId::new_v7();
        let upgraded = crate::portability::schema_fingerprint(&connection, correlation).unwrap();
        let fresh =
            crate::portability::schema_fingerprint(&migrated_connection(), correlation).unwrap();
        assert_eq!(upgraded.migration_version(), 19);
        assert_eq!(upgraded.digest(), fresh.digest());
    }
}
