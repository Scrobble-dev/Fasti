mod search_authorization_tests {
    use super::*;
    use fasti_application::{
        AccessAdministrationPort, BrowserRequestBoundaryPolicy, BrowserSessionAccessContext,
        BrowserSessionMutationCommand, BrowserSessionPort, BrowserSessionQuery,
        CreateAuthSubjectCommand, CreateBrowserSessionCommand, CreatedBrowserSession,
        RevokeCredentialCommand, ScopeKey, SecretMaterial, SessionPolicy,
    };
    use fasti_domain::{
        AuthSubject, AuthSubjectId, AuthSubjectLifecycle, MembershipId, TrailBaseInstanceId,
    };
    use rusqlite::types::Value;

    fn page_authority(node: &TestNode, access: &ApplicationAccessContext) -> ApplicationResult<()> {
        node.kernel
            .authorize_search_page_request(RequestCorrelationId::new_v7(), access)
    }

    fn read_authority(node: &TestNode, access: &ApplicationAccessContext) -> ApplicationResult<()> {
        node.kernel
            .authorize_search_candidate_read_request(RequestCorrelationId::new_v7(), access)
    }

    fn action_authority(
        node: &TestNode,
        access: &ApplicationAccessContext,
    ) -> ApplicationResult<()> {
        node.kernel
            .authorize_search_candidate_action_request(RequestCorrelationId::new_v7(), access)
    }

    fn remove_scope(node: &TestNode, scope: &str) {
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = ?2",
                params![node.access.grant_id().to_string(), scope],
            )
            .unwrap();
    }

    // Compare full persisted values, not just row counts: authorization must not
    // refresh provider health or rewrite an existing cache entry either.
    fn source_state(node: &TestNode) -> Vec<Vec<Vec<Value>>> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        [
            "provider_capability_states",
            "search_pages",
            "search_candidate_receipts",
            "search_action_receipts",
            "metadata_refresh_receipts",
            "records",
            "external_identifiers",
            "metadata_field_claims",
        ]
        .into_iter()
        .map(|table| {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table}"))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |row| {
                    (0..columns).map(|column| row.get(column)).collect()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<Vec<Value>>>>()
                .unwrap();
            rows
        })
        .collect()
    }

    fn copy_secret(secret: &SecretMaterial) -> SecretMaterial {
        SecretMaterial::try_from_hex(&secret.expose_hex()).unwrap()
    }

    fn browser_session(node: &TestNode) -> CreatedBrowserSession {
        // Reuse the real session owner and the same explicit installation,
        // membership and authentication-evidence pattern as browser_auth tests.
        let created_at = now() - Duration::seconds(15);
        let subject_id = AuthSubjectId::new_v7();
        let instance_id = TrailBaseInstanceId::new_v7();
        node.kernel
            .create_auth_subject(CreateAuthSubjectCommand::new(
                RequestCorrelationId::new_v7(),
                AuthSubject::try_new(
                    subject_id,
                    AuthSubjectLifecycle::Active,
                    1,
                    1,
                    created_at,
                    created_at,
                )
                .unwrap(),
            ))
            .unwrap();
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            connection.execute(
                "INSERT INTO trailbase_installation(singleton, trailbase_instance_id, physical_root_identity, release_lock_identity, activation_state, activation_blocker, activation_generation, created_at, updated_at) VALUES (1, ?1, ?2, ?3, 'active', NULL, 1, ?4, ?4)",
                params![instance_id.to_string(), Sha256Digest::from_bytes(&[31; 32]).to_string(), Sha256Digest::from_bytes(&[32; 32]).to_string(), timestamp(created_at)],
            ).unwrap();
            connection.execute(
                "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                params![subject_id.to_string(), node.access.grant_id().to_string()],
            ).unwrap();
            connection.execute(
                "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'member', ?4, ?4)",
                params![MembershipId::new_v7().to_string(), subject_id.to_string(), node.access.workspace_id().to_string(), timestamp(created_at)],
            ).unwrap();
        }
        let policy = SessionPolicy::try_new(
            std::time::Duration::from_secs(120),
            std::time::Duration::from_secs(240),
            std::time::Duration::from_secs(480),
            std::time::Duration::from_secs(10),
        )
        .unwrap();
        let created = node
            .kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    subject_id,
                    node.access.workspace_id(),
                    vec![node.access.grant_id()],
                    node.access.grant_id(),
                    policy,
                    false,
                    created_at,
                )
                .unwrap(),
            )
            .unwrap();
        node.kernel.inner.connection.lock().unwrap().execute(
            "INSERT INTO fasti_browser_session_authentication(browser_session_id, trailbase_instance_id, activation_generation, method, verified_at, recent_authentication_expires_at) VALUES (?1, ?2, 1, 'trailbase_password', ?3, NULL)",
            params![created.session().id().to_string(), instance_id.to_string(), timestamp(created_at)],
        ).unwrap();
        created
    }

    fn browser_access(
        created: &CreatedBrowserSession,
        mutation: bool,
        wrong_csrf: bool,
    ) -> ApplicationAccessContext {
        let boundary =
            BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example")
                .unwrap();
        if mutation {
            BrowserSessionAccessContext::mutation(BrowserSessionMutationCommand::new(
                RequestCorrelationId::new_v7(),
                copy_secret(created.session_secret()),
                if wrong_csrf {
                    SecretMaterial::from_bytes([99; 32])
                } else {
                    copy_secret(created.csrf_secret())
                },
                boundary
                    .validate(Some("https://fasti.example"), Some("fasti.example"))
                    .unwrap(),
                now(),
            ))
            .into()
        } else {
            BrowserSessionAccessContext::read(
                BrowserSessionQuery::new(
                    RequestCorrelationId::new_v7(),
                    copy_secret(created.session_secret()),
                    now(),
                ),
                boundary.validate_read(Some("fasti.example")).unwrap(),
            )
            .into()
        }
    }

    fn last_seen(node: &TestNode, created: &CreatedBrowserSession) -> String {
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT last_seen_at FROM fasti_browser_sessions WHERE browser_session_id = ?1",
                [created.session().id().to_string()],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn search_page_authorization_checks_current_scoped_authority_without_source_writes() {
        let (node, request) = setup();
        commit(&node, &request, &[candidate("42")]);
        let before = source_state(&node);
        page_authority(&node, &request.access).unwrap();
        let denied = node.add_profile_with_scopes(&[ScopeKey::IdentityRead]);
        assert_eq!(
            page_authority(&node, &denied.into()).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        node.kernel
            .revoke_credential(RevokeCredentialCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                node.access.credential_id(),
            ))
            .unwrap();
        assert_eq!(
            page_authority(&node, &request.access).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        assert_eq!(source_state(&node), before);
    }

    #[test]
    fn search_page_authorization_commits_only_successful_browser_activity() {
        let (node, request) = setup();
        commit(&node, &request, &[candidate("42")]);
        let created = browser_session(&node);
        let before = source_state(&node);
        let initial_activity = last_seen(&node, &created);
        for access in [
            browser_access(&created, false, false),
            browser_access(&created, true, true),
        ] {
            assert_eq!(
                page_authority(&node, &access).unwrap_err().code(),
                ProblemCode::Forbidden
            );
            assert_eq!(last_seen(&node, &created), initial_activity);
        }
        page_authority(&node, &browser_access(&created, true, false)).unwrap();
        assert!(last_seen(&node, &created) > initial_activity);
        assert_eq!(source_state(&node), before);
    }

    #[test]
    fn search_page_authorization_missing_browser_scope_rolls_back_activity() {
        let (node, _) = setup();
        let created = browser_session(&node);
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = 'metadata_search'",
                [node.access.grant_id().to_string()],
            )
            .unwrap();
        let before = source_state(&node);
        let activity = last_seen(&node, &created);
        assert_eq!(
            page_authority(&node, &browser_access(&created, true, false))
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(last_seen(&node, &created), activity);
        assert_eq!(source_state(&node), before);
    }

    #[test]
    fn search_page_discard_requires_browser_mutation_and_commits_only_successful_activity() {
        let (node, bearer_request) = setup();
        let created = browser_session(&node);
        let mut request = bearer_request.clone();
        request.access = browser_access(&created, true, false);
        let saved = commit(&node, &request, &[candidate("42")]);
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        let wrong_partition = node.kernel.prepare_search_page(&bearer_request).unwrap();

        // Seeding the real browser-owned page already refreshed activity. Put
        // this fixture back at its original, still-active timestamp so a
        // subsequent authorization crosses the real owner's write interval.
        let initial_activity = timestamp(created.session().last_seen_at());
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE fasti_browser_sessions SET last_seen_at = ?1 WHERE browser_session_id = ?2",
                params![initial_activity, created.session().id().to_string()],
            )
            .unwrap();
        let before = source_state(&node);
        assert_eq!(before[1].len(), 1);
        assert_eq!(before[2].len(), 1);

        for access in [
            browser_access(&created, false, false),
            browser_access(&created, true, true),
        ] {
            let mut invalid = request.clone();
            invalid.access = access;
            assert_eq!(
                node.kernel
                    .discard_cached_search_page(&invalid, &prepared)
                    .unwrap_err()
                    .code(),
                ProblemCode::Forbidden
            );
            assert_eq!(last_seen(&node, &created), initial_activity);
            assert_eq!(source_state(&node), before);
        }
        // A valid mutation authenticates and attempts an activity write before
        // the prepared-partition mismatch. Both that write and purge roll back.
        assert_eq!(
            node.kernel
                .discard_cached_search_page(&request, &wrong_partition)
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(last_seen(&node, &created), initial_activity);
        assert_eq!(source_state(&node), before);

        node.kernel
            .discard_cached_search_page(&request, &prepared)
            .unwrap();
        assert!(last_seen(&node, &created) > initial_activity);
        let mut expected = before;
        expected[1].clear();
        expected[2].clear();
        assert_eq!(source_state(&node), expected);
        assert!(node
            .kernel
            .read_search_candidate(&details(&request, saved.candidates[0].id()))
            .unwrap()
            .is_none());
    }

    #[test]
    fn search_candidate_preflights_use_distinct_current_scopes_without_source_writes() {
        let (node, request) = setup();
        commit(&node, &request, &[candidate("42")]);
        let before = source_state(&node);
        read_authority(&node, &request.access).unwrap();
        action_authority(&node, &request.access).unwrap();

        let identity_only: ApplicationAccessContext = node
            .add_profile_with_scopes(&[ScopeKey::IdentityWrite])
            .into();
        let search_only: ApplicationAccessContext = node
            .add_profile_with_scopes(&[ScopeKey::MetadataSearch])
            .into();
        action_authority(&node, &identity_only).unwrap();
        read_authority(&node, &search_only).unwrap();
        assert_eq!(
            read_authority(&node, &identity_only).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        assert_eq!(
            action_authority(&node, &search_only).unwrap_err().code(),
            ProblemCode::Forbidden
        );

        // Use the already-issued access snapshot: current durable scopes win.
        remove_scope(&node, "metadata_search");
        assert_eq!(
            read_authority(&node, &request.access).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        action_authority(&node, &request.access).unwrap();
        remove_scope(&node, "identity_write");
        assert_eq!(
            action_authority(&node, &request.access).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        assert_eq!(source_state(&node), before);
    }

    #[test]
    fn search_candidate_preflights_reject_current_credential_revocation() {
        let (node, request) = setup();
        commit(&node, &request, &[candidate("42")]);
        let before = source_state(&node);
        read_authority(&node, &request.access).unwrap();
        action_authority(&node, &request.access).unwrap();
        node.kernel
            .revoke_credential(RevokeCredentialCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                node.access.credential_id(),
            ))
            .unwrap();
        assert_eq!(
            read_authority(&node, &request.access).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        assert_eq!(
            action_authority(&node, &request.access).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        assert_eq!(source_state(&node), before);
    }

    #[test]
    fn search_candidate_preflights_preserve_browser_read_and_mutation_boundaries() {
        for action in [false, true] {
            let (node, request) = setup();
            commit(&node, &request, &[candidate("42")]);
            let created = browser_session(&node);
            let before = source_state(&node);
            let activity = last_seen(&node, &created);
            let authorize = if action {
                action_authority
            } else {
                read_authority
            };

            assert_eq!(
                authorize(&node, &browser_access(&created, true, true))
                    .unwrap_err()
                    .code(),
                ProblemCode::Forbidden
            );
            assert_eq!(last_seen(&node, &created), activity);
            if action {
                assert_eq!(
                    authorize(&node, &browser_access(&created, false, false))
                        .unwrap_err()
                        .code(),
                    ProblemCode::Forbidden
                );
                assert_eq!(last_seen(&node, &created), activity);
                // Completed saves must not acquire a Search dependency in preflight.
                remove_scope(&node, "metadata_search");
            } else {
                remove_scope(&node, "identity_write");
            }
            authorize(&node, &browser_access(&created, action, false)).unwrap();
            assert!(last_seen(&node, &created) > activity);
            assert_eq!(source_state(&node), before);
        }
    }

    #[test]
    fn search_candidate_preflights_missing_browser_scope_roll_back_activity() {
        for action in [false, true] {
            let (node, request) = setup();
            commit(&node, &request, &[candidate("42")]);
            let created = browser_session(&node);
            let access = browser_access(&created, action, false);
            remove_scope(
                &node,
                if action {
                    "identity_write"
                } else {
                    "metadata_search"
                },
            );
            let before = source_state(&node);
            let activity = last_seen(&node, &created);
            let result = if action {
                action_authority(&node, &access)
            } else {
                read_authority(&node, &access)
            };
            assert_eq!(result.unwrap_err().code(), ProblemCode::Forbidden);
            assert_eq!(last_seen(&node, &created), activity);
            assert_eq!(source_state(&node), before);
        }
    }

    #[test]
    fn search_candidate_preflights_reject_real_browser_session_revocation() {
        let (node, request) = setup();
        commit(&node, &request, &[candidate("42")]);
        let created = browser_session(&node);
        let read = browser_access(&created, false, false);
        let action = browser_access(&created, true, false);
        read_authority(&node, &read).unwrap();
        action_authority(&node, &action).unwrap();
        let before = source_state(&node);
        let boundary =
            BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example")
                .unwrap();
        assert!(node
            .kernel
            .revoke_current_browser_session(BrowserSessionMutationCommand::new(
                RequestCorrelationId::new_v7(),
                copy_secret(created.session_secret()),
                copy_secret(created.csrf_secret()),
                boundary
                    .validate(Some("https://fasti.example"), Some("fasti.example"))
                    .unwrap(),
                now(),
            ))
            .unwrap());
        let activity = last_seen(&node, &created);
        assert_eq!(
            read_authority(&node, &read).unwrap_err().code(),
            ProblemCode::BrowserSessionRevoked
        );
        assert_eq!(
            action_authority(&node, &action).unwrap_err().code(),
            ProblemCode::BrowserSessionRevoked
        );
        assert_eq!(last_seen(&node, &created), activity);
        assert_eq!(source_state(&node), before);
    }
}
