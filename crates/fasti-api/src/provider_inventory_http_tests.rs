mod provider_inventory_http_tests {
    use super::*;
    use fasti_application::SelectBrowserSessionProfileCommand;
    use fasti_domain::{ProfileGrantId, ProfileId};

    const PROVIDERS_PATH: &str = "/api/v1/providers";

    fn direct_boundary(f: &Fixture) -> BrowserRequestBoundaryPolicy {
        DirectLoopbackAccessRuntime::new(
            f.kernel.clone(),
            FASTI_ACCESS_HOST.parse().expect("Fasti access host"),
            false,
            f._root.path(),
            None,
        )
        .expect("direct browser runtime")
        .browser_boundary()
    }

    fn provider_app(f: &Fixture, browser: bool) -> Router {
        provider_api_router(
            f.kernel.clone(),
            f.kernel.clone(),
            f.runtime.clone(),
            f.locks.clone(),
            browser.then(|| direct_boundary(f)),
        )
    }

    fn provider_capability<'a>(
        response: &'a serde_json::Value,
        provider_id: &str,
        capability_id: &str,
    ) -> &'a serde_json::Value {
        response["providers"]
            .as_array()
            .expect("provider inventory")
            .iter()
            .find(|provider| provider["provider_id"] == provider_id)
            .and_then(|provider| provider["capabilities"].as_array())
            .and_then(|capabilities| {
                capabilities
                    .iter()
                    .find(|capability| capability["capability_id"] == capability_id)
            })
            .expect("provider capability")
    }

    fn provider_state_snapshot(f: &Fixture) -> Vec<String> {
        let connection = rusqlite::Connection::open(f.kernel.database_path())
            .expect("provider state database");
        let mut statement = connection
            .prepare(
                r#"
                SELECT json_array(
                    workspace_id, provider_id, capability_id, capability_status,
                    capability_version, credential_requirement, credential_reference,
                    credential_status, configuration_digest, health_status,
                    health_checked_at, health_problem_code, credential_test_status,
                    credential_test_checked_at, credential_test_problem_code, updated_at
                )
                FROM provider_capability_states
                ORDER BY workspace_id, provider_id, capability_id
                "#,
            )
            .expect("provider state snapshot query");
        statement
            .query_map([], |row| row.get(0))
            .expect("provider state rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("provider state snapshot")
    }

    fn browser_provider_request(
        f: &Fixture,
        method: axum::http::Method,
        path: &str,
        body: Body,
    ) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, FASTI_ACCESS_HOST)
            .header(header::ORIGIN, FASTI_ACCESS_ORIGIN)
            .header(header::COOKIE, &f.cookie)
            .header(local::CSRF_HEADER, &f.csrf)
            .header(header::CONTENT_TYPE, "application/json")
            .body(body)
            .expect("browser provider request")
    }

    #[tokio::test]
    async fn provider_inventory_browser_read_is_scoped_non_actionable_and_private() {
        let f = fixture().await;
        let other_workspace = fasti_domain::WorkspaceId::new_v7();
        let connection = rusqlite::Connection::open(f.kernel.database_path())
            .expect("provider state database");
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id,created_at) VALUES (?1,?2)",
                rusqlite::params![
                    other_workspace.to_string(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .expect("other workspace");
        f.kernel
            .put_provider_capability_state(
                other_workspace,
                ProviderCapabilityState::try_new(
                    ProviderId::try_new("tmdb").expect("TMDB provider"),
                    ProviderCapabilityId::try_new("metadata.search")
                        .expect("Search capability"),
                    ProviderCapabilityStatus::Disabled,
                    99,
                    CredentialRequirement::BearerToken,
                    None,
                    ProviderCredentialStatus::Missing,
                    ConfigurationDigest::parse("b".repeat(64))
                        .expect("configuration digest"),
                    ProviderCheckMetadata::never_run(),
                    ProviderCheckMetadata::never_run(),
                )
                .expect("other workspace provider state"),
            )
            .expect("store other workspace provider state");
        let selected_scope_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM grant_scopes WHERE grant_id=?1 AND scope_key='provider_read'",
                [f.access.grant_id().to_string()],
                |row| row.get(0),
            )
            .expect("selected profile provider scope");
        assert_eq!(selected_scope_count, 1);

        let app = provider_app(&f, true);
        let (status, browser_inventory) =
            response(&app, candidate_get(&f, true, PROVIDERS_PATH)).await;
        assert_eq!(status, StatusCode::OK, "{browser_inventory}");
        let browser_capability =
            provider_capability(&browser_inventory, "tmdb", "metadata.search");
        assert_eq!(browser_capability["version"], 1);
        assert_eq!(browser_capability["writable"], false);
        assert_eq!(browser_capability["testable"], false);

        let serialized = browser_inventory.to_string();
        let private_values = [
            f.credential.clone(),
            "secret:search-http-test".to_owned(),
            "a".repeat(64),
            "b".repeat(64),
            f.access.workspace_id().to_string(),
            other_workspace.to_string(),
            f.access.profile_id().to_string(),
            f.access.client_id().to_string(),
            f.access.grant_id().to_string(),
        ];
        for private_value in &private_values {
            assert!(
                !serialized.contains(private_value.as_str()),
                "provider inventory exposed a private authority or storage value"
            );
        }

        let calls_after_browser = f.vault.0.load(Ordering::SeqCst);
        assert!(
            calls_after_browser > 0,
            "authorized inventory should resolve safe credential presence"
        );
        let (status, bearer_inventory) =
            response(&app, candidate_get(&f, false, PROVIDERS_PATH)).await;
        assert_eq!(status, StatusCode::OK, "{bearer_inventory}");
        let bearer_capability = provider_capability(&bearer_inventory, "tmdb", "metadata.search");
        assert_eq!(bearer_capability["version"], 1);
        assert_eq!(bearer_capability["writable"], false);
        assert_eq!(bearer_capability["testable"], true);
        assert!(f.vault.0.load(Ordering::SeqCst) > calls_after_browser);

        let mut browser_projection = browser_capability.clone();
        let mut bearer_projection = bearer_capability.clone();
        browser_projection
            .as_object_mut()
            .expect("browser capability object")
            .remove("writable");
        browser_projection
            .as_object_mut()
            .expect("browser capability object")
            .remove("testable");
        bearer_projection
            .as_object_mut()
            .expect("bearer capability object")
            .remove("writable");
        bearer_projection
            .as_object_mut()
            .expect("bearer capability object")
            .remove("testable");
        assert_eq!(browser_projection, bearer_projection);
    }

    #[tokio::test]
    async fn provider_inventory_denies_invalid_browser_authority_before_state_or_vault_access() {
        let f = fixture().await;
        let app = provider_app(&f, true);
        let generic = provider_app(&f, false);
        let initial_state = provider_state_snapshot(&f);

        let cases = [
            (
                Request::get(PROVIDERS_PATH)
                    .header(header::HOST, FASTI_ACCESS_HOST)
                    .body(Body::empty())
                    .expect("missing browser cookie"),
                "browser_session_revoked",
            ),
            ({
                let mut request = candidate_get(&f, true, PROVIDERS_PATH);
                request.headers_mut().insert(
                    header::HOST,
                    "untrusted.example".parse().expect("untrusted host"),
                );
                request
            }, "browser_session_revoked"),
            ({
                let mut request = candidate_get(&f, true, PROVIDERS_PATH);
                request.headers_mut().insert(
                    header::AUTHORIZATION,
                    format!("Bearer {}", f.credential)
                        .parse()
                        .expect("bearer header"),
                );
                request
            }, "authentication_failed"),
            ({
                let mut request = candidate_get(&f, true, PROVIDERS_PATH);
                request.headers_mut().insert(
                    header::COOKIE,
                    format!("{}=not-hex", local::SESSION_COOKIE)
                        .parse()
                        .expect("malformed cookie"),
                );
                request
            }, "browser_session_revoked"),
            ({
                let mut request = candidate_get(&f, true, PROVIDERS_PATH);
                request.headers_mut().append(
                    header::COOKIE,
                    f.cookie
                        .split(';')
                        .next()
                        .expect("session cookie")
                        .parse()
                        .expect("duplicate cookie"),
                );
                request
            }, "authentication_failed"),
        ];
        for (request, expected_code) in cases {
            let (status, problem) = response(&app, request).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
            assert_eq!(problem["code"], expected_code);
            assert_eq!(problem["capability_id"], "provider.list");
        }
        let (status, problem) =
            response(&generic, candidate_get(&f, true, PROVIDERS_PATH)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        assert_eq!(problem["code"], "authentication_failed");
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
        assert_eq!(provider_state_snapshot(&f), initial_state);

        rusqlite::Connection::open(f.kernel.database_path())
            .expect("provider state database")
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id=?1 AND scope_key='provider_read'",
                [f.access.grant_id().to_string()],
            )
            .expect("remove selected profile provider scope");
        let (status, problem) =
            response(&app, candidate_get(&f, true, PROVIDERS_PATH)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
        assert_eq!(problem["code"], "forbidden");
        assert_eq!(problem["capability_id"], "provider.list");
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
        assert_eq!(provider_state_snapshot(&f), initial_state);
    }

    #[tokio::test]
    async fn provider_inventory_rechecks_session_policy_before_disclosing_node_state() {
        {
            let f = fixture().await;
            let app = provider_app(&f, true);
            let initial_state = provider_state_snapshot(&f);
            let ApplicationAccessContext::BrowserSession(proof) = &f.browser else {
                panic!("browser fixture")
            };
            assert!(f
                .kernel
                .revoke_current_browser_session(BrowserSessionMutationCommand::new(
                    RequestCorrelationId::new_v7(),
                    SecretMaterial::try_from_hex(&proof.session_secret().expose_hex()).unwrap(),
                    SecretMaterial::try_from_hex(&f.csrf).unwrap(),
                    direct_boundary(&f)
                        .validate(Some(FASTI_ACCESS_ORIGIN), Some(FASTI_ACCESS_HOST))
                        .unwrap(),
                    chrono::Utc::now(),
                ))
                .unwrap());
            let (status, problem) =
                response(&app, candidate_get(&f, true, PROVIDERS_PATH)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
            assert_eq!(problem["code"], "browser_session_revoked");
            assert_eq!(problem["capability_id"], "provider.list");
            assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
            assert_eq!(provider_state_snapshot(&f), initial_state);
        }

        for (statement, expected_code) in [
            (
                "UPDATE workspace_memberships SET lifecycle='suspended'",
                "session_policy_changed",
            ),
            (
                "UPDATE fasti_browser_sessions SET last_seen_at=created_at, idle_expires_at=strftime('%Y-%m-%dT%H:%M:%fZ',created_at,'+1 second')",
                "browser_session_expired",
            ),
            (
                "UPDATE auth_subjects SET auth_epoch=auth_epoch+1",
                "session_policy_changed",
            ),
            (
                "UPDATE auth_subjects SET authorization_epoch=authorization_epoch+1",
                "session_policy_changed",
            ),
        ] {
            let f = fixture().await;
            let app = provider_app(&f, true);
            let initial_state = provider_state_snapshot(&f);
            rusqlite::Connection::open(f.kernel.database_path())
                .expect("provider state database")
                .execute(statement, [])
                .expect("invalidate browser authority");
            let (status, problem) =
                response(&app, candidate_get(&f, true, PROVIDERS_PATH)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
            assert_eq!(problem["code"], expected_code);
            assert_eq!(problem["capability_id"], "provider.list");
            assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
            assert_eq!(provider_state_snapshot(&f), initial_state);
        }
    }

    #[tokio::test]
    async fn provider_inventory_profile_rotation_rejects_the_old_session_and_accepts_the_new_one() {
        let f = fixture().await;
        let app = provider_app(&f, true);
        let initial_state = provider_state_snapshot(&f);
        let grant = ProfileGrantId::new_v7();
        let profile = ProfileId::new_v7();
        let mut connection = rusqlite::Connection::open(f.kernel.database_path())
            .expect("provider state database");
        let transaction = connection.transaction().expect("profile transaction");
        let at = chrono::Utc::now().to_rfc3339();
        transaction
            .execute(
                "INSERT INTO profiles(profile_id,workspace_id,created_at) VALUES (?1,?2,?3)",
                rusqlite::params![
                    profile.to_string(),
                    f.access.workspace_id().to_string(),
                    at
                ],
            )
            .expect("second profile");
        transaction
            .execute(
                "INSERT INTO profile_grants(grant_id,workspace_id,profile_id,client_id,status,created_at) VALUES (?1,?2,?3,?4,'active',?5)",
                rusqlite::params![
                    grant.to_string(),
                    f.access.workspace_id().to_string(),
                    profile.to_string(),
                    f.access.client_id().to_string(),
                    at
                ],
            )
            .expect("second profile grant");
        transaction
            .execute(
                "INSERT INTO grant_scopes(grant_id,scope_key) VALUES (?1,'provider_read')",
                [grant.to_string()],
            )
            .expect("second profile provider scope");
        transaction
            .execute(
                "INSERT INTO auth_subject_profile_grants(auth_subject_id,profile_grant_id) SELECT auth_subject_id,?1 FROM auth_subject_profile_grants WHERE profile_grant_id=?2",
                rusqlite::params![grant.to_string(), f.access.grant_id().to_string()],
            )
            .expect("subject second profile grant");
        transaction
            .execute(
                "INSERT INTO fasti_browser_session_grants(browser_session_id,profile_grant_id) SELECT browser_session_id,?1 FROM fasti_browser_session_grants WHERE profile_grant_id=?2",
                rusqlite::params![grant.to_string(), f.access.grant_id().to_string()],
            )
            .expect("session second profile grant");
        transaction.commit().expect("commit second profile");

        let ApplicationAccessContext::BrowserSession(proof) = &f.browser else {
            panic!("browser fixture")
        };
        let selected = f
            .kernel
            .select_browser_session_profile(SelectBrowserSessionProfileCommand::new(
                BrowserSessionMutationCommand::new(
                    RequestCorrelationId::new_v7(),
                    SecretMaterial::try_from_hex(&proof.session_secret().expose_hex()).unwrap(),
                    SecretMaterial::try_from_hex(&f.csrf).unwrap(),
                    direct_boundary(&f)
                        .validate(Some(FASTI_ACCESS_ORIGIN), Some(FASTI_ACCESS_HOST))
                        .unwrap(),
                    chrono::Utc::now(),
                ),
                grant,
            ))
            .expect("select second profile");

        let (status, problem) =
            response(&app, candidate_get(&f, true, PROVIDERS_PATH)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        assert_eq!(problem["code"], "browser_session_revoked");
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
        assert_eq!(provider_state_snapshot(&f), initial_state);

        let mut fresh = candidate_get(&f, true, PROVIDERS_PATH);
        fresh.headers_mut().insert(
            header::COOKIE,
            format!(
                "{}={}",
                local::SESSION_COOKIE,
                selected.session_secret().expose_hex()
            )
            .parse()
            .expect("rotated session cookie"),
        );
        let (status, inventory) = response(&app, fresh).await;
        assert_eq!(status, StatusCode::OK, "{inventory}");
        assert_eq!(
            provider_capability(&inventory, "tmdb", "metadata.search")["version"],
            1
        );
        assert!(f.vault.0.load(Ordering::SeqCst) > 0);
        assert_eq!(provider_state_snapshot(&f), initial_state);
    }

    #[tokio::test]
    async fn provider_inventory_preserves_corrupt_state_problem_before_vault_access() {
        let f = fixture().await;
        let app = provider_app(&f, true);
        rusqlite::Connection::open(f.kernel.database_path())
            .unwrap()
            .execute(
                "UPDATE provider_capability_states SET health_status='passed', health_checked_at='not-a-timestamp'",
                [],
            )
            .unwrap();
        for browser in [false, true] {
            let (status, problem) = response(&app, candidate_get(&f, browser, PROVIDERS_PATH)).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{problem}");
            assert_eq!(problem["code"], "integrity_failed");
            assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn provider_credential_and_health_routes_remain_bearer_only_for_browser_sessions() {
        let f = fixture().await;
        let app = provider_app(&f, true);
        let initial_state = provider_state_snapshot(&f);
        let cases = [
            browser_provider_request(
                &f,
                axum::http::Method::PUT,
                "/api/v1/providers/tmdb/credentials/metadata.search",
                Body::from(r#"{"secret":"must-not-store"}"#),
            ),
            browser_provider_request(
                &f,
                axum::http::Method::DELETE,
                "/api/v1/providers/tmdb/credentials/metadata.search",
                Body::empty(),
            ),
            browser_provider_request(
                &f,
                axum::http::Method::POST,
                "/api/v1/providers/tmdb/credentials/metadata.search/tests",
                Body::empty(),
            ),
            browser_provider_request(
                &f,
                axum::http::Method::GET,
                "/api/v1/providers/tmdb/health",
                Body::empty(),
            ),
        ];
        for request in cases {
            let (status, problem) = response(&app, request).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
            assert_eq!(problem["code"], "authentication_failed");
        }
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
        assert_eq!(provider_state_snapshot(&f), initial_state);
    }
}
