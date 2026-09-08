mod provider_identifier_details_http_tests {
    use super::*;
    use fasti_application::{AccessAdministrationPort, RevokeCredentialCommand};
    use rusqlite::types::Value;

    fn details_path(provider: &str, grain: &str, id: &str, offline: &str) -> String {
        format!(
            "/api/v1/search/providers/{provider}/{grain}/details?provider_record_id={id}&offline={offline}"
        )
    }

    // Compare full values for every durable Search/detail destination. Browser
    // session activity is intentionally outside this projection.
    pub(super) fn content_state(f: &Fixture) -> Vec<Vec<Vec<Value>>> {
        let connection = rusqlite::Connection::open(f.kernel.database_path()).unwrap();
        [
            "provider_capability_states",
            "search_pages",
            "search_candidate_receipts",
            "search_action_receipts",
            "metadata_refresh_receipts",
            "records",
            "external_identifiers",
            "metadata_field_claims",
            "metadata_claims",
            "metadata_claim_provenance",
            "local_search_grams",
            "profile_record_tracking_dispositions",
            "metadata_profile_field_overrides",
            "observations",
            "workspace_revisions",
        ]
        .into_iter()
        .map(|table| {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table}"))
                .unwrap();
            let columns = statement.column_count();
            statement
                .query_map([], |row| {
                    (0..columns).map(|column| row.get(column)).collect()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<Vec<Value>>>>()
                .unwrap()
        })
        .collect()
    }

    fn revoke_browser(f: &Fixture) {
        let ApplicationAccessContext::BrowserSession(proof) = &f.browser else {
            panic!("browser fixture")
        };
        let boundary =
            BrowserRequestBoundaryPolicy::try_new(FASTI_ACCESS_ORIGIN, FASTI_ACCESS_HOST).unwrap();
        assert!(f
            .kernel
            .revoke_current_browser_session(BrowserSessionMutationCommand::new(
                RequestCorrelationId::new_v7(),
                SecretMaterial::try_from_hex(&proof.session_secret().expose_hex()).unwrap(),
                SecretMaterial::try_from_hex(&f.csrf).unwrap(),
                boundary
                    .validate(Some(FASTI_ACCESS_ORIGIN), Some(FASTI_ACCESS_HOST))
                    .unwrap(),
                chrono::Utc::now(),
            ))
            .unwrap());
    }

    #[tokio::test]
    async fn provider_identifier_details_http_offline_needs_no_provider_state_or_vault() {
        let f = fixture().await;
        rusqlite::Connection::open(f.kernel.database_path())
            .unwrap()
            .execute("DELETE FROM provider_capability_states", [])
            .unwrap();
        let before = content_state(&f);
        let path = format!(
            "{}&locale=fr-FR",
            details_path("tmdb", "film", "42", "true")
        );

        for browser in [false, true] {
            let (status, body) = response(&f.app, candidate_get(&f, browser, &path)).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(
                body,
                serde_json::json!({
                    "outcome": "unavailable",
                    "provider_id": "tmdb",
                    "grain": "film",
                    "provider_record_id": "42",
                    "problem_code": "provider_unavailable"
                })
            );
        }

        assert_eq!(content_state(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn provider_identifier_details_http_online_missing_read_capability_stops_before_vault() {
        let f = fixture().await;
        let before = content_state(&f);
        let path = details_path("tmdb", "film", "42", "false");

        for browser in [false, true] {
            let (status, problem) = response(&f.app, candidate_get(&f, browser, &path)).await;
            assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{problem}");
            assert_eq!(problem["code"], "capability_unavailable");
            assert_eq!(problem["capability_id"], "metadata.search");
        }

        assert_eq!(content_state(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn provider_identifier_details_http_validates_after_current_read_authority() {
        let f = fixture().await;
        let before = content_state(&f);
        let cases = [
            "/api/v1/search/providers/tmdb/film/details?offline=true".to_owned(),
            "/api/v1/search/providers/tmdb/film/details?provider_record_id=42".to_owned(),
            details_path("tmdb", "film", "42", "maybe"),
            "/api/v1/search/providers/tmdb/film/details?provider_record_id=42&provider_record_id=43&offline=true".to_owned(),
            "/api/v1/search/providers/tmdb/film/details?provider_record_id=42&offline=true&offline=false".to_owned(),
            format!(
                "{}&unknown=value",
                details_path("tmdb", "film", "42", "true")
            ),
            format!(
                "{}&locale=f",
                details_path("tmdb", "film", "42", "true")
            ),
            format!(
                "{}&locale={}",
                details_path("tmdb", "film", "42", "true"),
                "x".repeat(17)
            ),
            details_path("tmdb", "not-a-grain", "42", "true"),
            details_path("unknown-provider", "film", "42", "true"),
            details_path("tmdb", "episode", "42", "true"),
            details_path("tmdb", "film", "", "true"),
            details_path("tmdb", "film", "0", "true"),
            details_path("tmdb", "film", "042", "true"),
            details_path("tmdb", "film", "not-a-positive-decimal", "true"),
            details_path("%FF", "film", "42", "true"),
        ];

        for browser in [false, true] {
            for path in &cases {
                let (status, problem) = response(&f.app, candidate_get(&f, browser, path)).await;
                assert_eq!(
                    status,
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "{path}: {problem}"
                );
                assert_eq!(problem["code"], "validation_failed", "{path}");
                assert_eq!(problem["capability_id"], "metadata.search", "{path}");
            }
        }
        assert_eq!(content_state(&f), before);

        rusqlite::Connection::open(f.kernel.database_path())
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id=?1 AND scope_key='metadata_search'",
                [f.access.grant_id().to_string()],
            )
            .unwrap();
        let denied_baseline = content_state(&f);
        for browser in [false, true] {
            for path in &cases {
                let (status, problem) = response(&f.app, candidate_get(&f, browser, path)).await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{path}: {problem}");
                assert_eq!(problem["code"], "forbidden", "{path}");
                assert_eq!(problem["capability_id"], "metadata.search", "{path}");
            }
        }
        assert_eq!(content_state(&f), denied_baseline);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn provider_identifier_details_http_browser_boundary_and_revocation_fail_closed() {
        let f = fixture().await;
        let path = details_path("tmdb", "film", "42", "true");
        let before = content_state(&f);

        for host in [None, Some("untrusted.example")] {
            let mut request = candidate_get(&f, true, &path);
            request.headers_mut().remove(header::HOST);
            if let Some(host) = host {
                request
                    .headers_mut()
                    .insert(header::HOST, host.parse().unwrap());
            }
            let (status, problem) = response(&f.app, request).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
            assert_eq!(problem["code"], "browser_session_revoked");
        }
        let mut mixed = candidate_get(&f, true, &path);
        mixed.headers_mut().insert(
            header::AUTHORIZATION,
            format!("Bearer {}", f.credential).parse().unwrap(),
        );
        let (status, problem) = response(&f.app, mixed).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        assert_eq!(problem["code"], "authentication_failed");
        let (status, problem) = response(&f.generic, candidate_get(&f, true, &path)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        assert_eq!(problem["code"], "authentication_failed");

        let (status, body) = response(&f.app, candidate_get(&f, true, &path)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        revoke_browser(&f);
        let (status, problem) = response(&f.app, candidate_get(&f, true, &path)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        assert_eq!(problem["code"], "browser_session_revoked");

        assert_eq!(content_state(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn provider_identifier_details_http_bearer_revocation_fails_closed() {
        let f = fixture().await;
        let path = details_path("tmdb", "film", "42", "true");
        let before = content_state(&f);
        let (status, body) = response(&f.app, candidate_get(&f, false, &path)).await;
        assert_eq!(status, StatusCode::OK, "{body}");

        f.kernel
            .revoke_credential(RevokeCredentialCommand::new(
                RequestCorrelationId::new_v7(),
                f.access,
                f.access.credential_id(),
            ))
            .unwrap();
        let (status, problem) = response(&f.app, candidate_get(&f, false, &path)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        assert_eq!(problem["code"], "authentication_failed");

        assert_eq!(content_state(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }
}
