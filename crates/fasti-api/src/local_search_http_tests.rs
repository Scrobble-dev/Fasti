mod local_search_http_tests {
    use super::*;
    use fasti_application::{
        ConfigureMetadataProjectionCommand, CreateRecordCommand, IdentityPort,
        MetadataOverrideMutation, MetadataProjectionPort, SelectBrowserSessionProfileCommand,
    };
    use fasti_domain::{
        FieldKey, Grain, MetadataProjectionPolicy, ProfileGrantId, ProfileId, RecordId,
        TITLE_FIELD_KEY,
    };

    const LOCAL_PATH: &str = "/api/v1/search/records";

    fn local_body(query: &str) -> serde_json::Value {
        serde_json::json!({"query": query, "grains": [], "after": null})
    }

    fn local_request(f: &Fixture, browser: bool, body: String) -> Request<Body> {
        let mut req = candidate_get(f, browser, LOCAL_PATH);
        *req.method_mut() = axum::http::Method::POST;
        req.headers_mut()
            .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        *req.body_mut() = Body::from(body);
        req
    }

    fn set_titles(f: &Fixture, records: &[RecordId], title: &str) {
        for chunk in records.chunks(64) {
            f.kernel
                .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                    RequestCorrelationId::new_v7(),
                    f.access,
                    MetadataProjectionPolicy::default_for_profile(f.access.profile_id()),
                    None,
                    vec![],
                    chunk
                        .iter()
                        .map(|record| MetadataOverrideMutation::Set {
                            record_id: *record,
                            field_key: FieldKey::try_new(TITLE_FIELD_KEY).unwrap(),
                            value: title.to_owned(),
                        })
                        .collect(),
                ))
                .unwrap();
        }
    }

    fn seed_records(f: &Fixture, count: usize, title: &str) -> Vec<RecordId> {
        let mut records: Vec<_> = (0..count)
            .map(|_| {
                f.kernel
                    .create_record(CreateRecordCommand::new(
                        RequestCorrelationId::new_v7(),
                        f.access,
                        Grain::Film,
                    ))
                    .unwrap()
                    .record_id()
            })
            .collect();
        records.sort_by_key(ToString::to_string);
        set_titles(f, &records, title);
        records
    }

    #[tokio::test]
    async fn local_search_http_offline_605_records_progress_without_provider_locks_or_csrf() {
        let f = fixture().await;
        let expected: Vec<_> = seed_records(&f, 605, "Common 東京 %_")
            .into_iter()
            .map(|id| id.to_string())
            .collect();
        // No provider state is required for local Search. Hold both actual
        // runtime gates as well, so accidentally acquiring either would time out.
        rusqlite::Connection::open(f.kernel.database_path())
            .unwrap()
            .execute("DELETE FROM provider_capability_states", [])
            .unwrap();
        let _tmdb = f.locks.get("tmdb").unwrap().lock_owned().await;
        let _books = f.locks.get("google-books").unwrap().lock_owned().await;
        let before = action_counts(&f);
        let sources = counts(&f);
        for browser in [false, true] {
            let mut body = local_body("Common 東京 %_");
            let mut found = vec![];
            let mut sizes = vec![];
            loop {
                let (status, page) = tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    response(&f.app, local_request(&f, browser, body.to_string())),
                )
                .await
                .expect("local Search must not wait for provider gates");
                assert_eq!(status, StatusCode::OK, "{page}");
                let records = page["records"].as_array().unwrap();
                sizes.push(records.len());
                found.extend(
                    records
                        .iter()
                        .map(|record| record["record_id"].as_str().unwrap().to_owned()),
                );
                for record in records {
                    assert_eq!(record["title"]["value"], "Common 東京 %_");
                    assert_eq!(record["grain"], "film");
                    assert!(record["latest_activity"].is_null());
                }
                if page["next"].is_null() {
                    break;
                }
                assert_ne!(body["after"], page["next"], "cursor must advance");
                body["after"] = page["next"].clone();
                assert!(sizes.len() < 10, "continuation must terminate");
            }
            assert_eq!(sizes, vec![100, 100, 100, 100, 100, 100, 5]);
            assert_eq!(found, expected);
        }
        assert_eq!(action_counts(&f), before);
        assert_eq!(counts(&f), sources);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn local_search_http_empty_continuation_reaches_later_authoritative_match() {
        let f = fixture().await;
        let records = seed_records(&f, 101, "needle absent");
        set_titles(&f, &records[100..], "needle present");
        let before = action_counts(&f);
        for browser in [false, true] {
            let mut body = local_body("needle present");
            let (status, first) =
                response(&f.app, local_request(&f, browser, body.to_string())).await;
            assert_eq!(status, StatusCode::OK, "{first}");
            assert_eq!(first["records"], serde_json::json!([]));
            assert!(!first["next"].is_null());
            body["after"] = first["next"].clone();
            let (status, second) =
                response(&f.app, local_request(&f, browser, body.to_string())).await;
            assert_eq!(status, StatusCode::OK, "{second}");
            assert_eq!(second["records"].as_array().unwrap().len(), 1);
            assert_eq!(second["records"][0]["record_id"], records[100].to_string());
            assert!(second["next"].is_null());
        }
        assert_eq!(action_counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn local_search_http_cursor_context_and_current_scope_are_rechecked() {
        let f = fixture().await;
        seed_records(&f, 101, "Common title");
        let before = action_counts(&f);
        let mut body = local_body("Common");
        let (status, page) = response(&f.app, local_request(&f, false, body.to_string())).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(!page["next"].is_null());
        body["after"] = page["next"].clone();
        let mut changed_query = body.clone();
        changed_query["query"] = "Common title".into();
        let mut changed_grain = body.clone();
        changed_grain["grains"] = serde_json::json!(["film"]);
        let mut changed_digest = body.clone();
        changed_digest["after"]["context_digest"] = format!("sha256:{}", "f".repeat(64)).into();
        for browser in [false, true] {
            for invalid in [&changed_query, &changed_grain, &changed_digest] {
                let (status, problem) =
                    response(&f.app, local_request(&f, browser, invalid.to_string())).await;
                assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
                assert_eq!(problem["code"], "validation_failed");
            }
        }
        rusqlite::Connection::open(f.kernel.database_path())
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id=?1 AND scope_key='metadata_search'",
                [f.access.grant_id().to_string()],
            )
            .unwrap();
        for browser in [false, true] {
            let (status, problem) =
                response(&f.app, local_request(&f, browser, body.to_string())).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
            assert_eq!(problem["code"], "forbidden");
            assert!(problem.get("records").is_none());
        }
        assert_eq!(action_counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn local_search_http_malformed_input_is_typed_only_after_read_authorization() {
        let f = fixture().await;
        let before = action_counts(&f);
        let mut authority = local_body("title");
        authority["profile_id"] = f.access.profile_id().to_string().into();
        let cases = [
            ("{".to_owned(), StatusCode::BAD_REQUEST),
            (authority.to_string(), StatusCode::UNPROCESSABLE_ENTITY),
            (local_body("").to_string(), StatusCode::UNPROCESSABLE_ENTITY),
            (local_body(" leading").to_string(), StatusCode::UNPROCESSABLE_ENTITY),
            (local_body(&"海".repeat(86)).to_string(), StatusCode::UNPROCESSABLE_ENTITY),
            (serde_json::json!({"query":"title","grains":["invalid"],"after":null}).to_string(), StatusCode::UNPROCESSABLE_ENTITY),
            (serde_json::json!({"query":"title","grains":[],"after":{"last_record_id":"invalid","context_digest":"invalid"}}).to_string(), StatusCode::UNPROCESSABLE_ENTITY),
        ];
        for browser in [false, true] {
            for (body, expected) in &cases {
                let (status, problem) =
                    response(&f.app, local_request(&f, browser, body.clone())).await;
                assert_eq!(status, *expected, "{body}: {problem}");
                assert_eq!(problem["capability_id"], "metadata.search");
            }
        }
        rusqlite::Connection::open(f.kernel.database_path())
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id=?1 AND scope_key='metadata_search'",
                [f.access.grant_id().to_string()],
            )
            .unwrap();
        for browser in [false, true] {
            for (body, _) in &cases {
                let (status, problem) =
                    response(&f.app, local_request(&f, browser, body.clone())).await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
            }
        }
        assert_eq!(action_counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn local_search_http_post_read_still_rejects_wrong_browser_boundary_and_mixed_auth() {
        let f = fixture().await;
        let before = action_counts(&f);
        for host in [None, Some("untrusted.example")] {
            let mut req = local_request(&f, true, "{".into());
            req.headers_mut().remove(header::HOST);
            if let Some(host) = host {
                req.headers_mut()
                    .insert(header::HOST, host.parse().unwrap());
            }
            let (status, problem) = response(&f.app, req).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        }
        let mut req = local_request(&f, true, "{".into());
        req.headers_mut().insert(
            header::AUTHORIZATION,
            format!("Bearer {}", f.credential).parse().unwrap(),
        );
        let (status, problem) = response(&f.app, req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        let (status, problem) = response(&f.generic, local_request(&f, true, "{".into())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        let (status, page) = response(
            &f.generic,
            local_request(&f, false, local_body("absent").to_string()),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page, serde_json::json!({"records":[],"next":null}));
        assert_eq!(action_counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn local_search_http_real_profile_switch_rejects_old_cursor_and_private_title() {
        let f = fixture().await;
        seed_records(&f, 101, "Private title");
        let mut body = local_body("Private");
        let (status, page) = response(&f.app, local_request(&f, true, body.to_string())).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(!page["next"].is_null());
        body["after"] = page["next"].clone();
        let grant = ProfileGrantId::new_v7();
        let profile = ProfileId::new_v7();
        let mut connection = rusqlite::Connection::open(f.kernel.database_path()).unwrap();
        let tx = connection.transaction().unwrap();
        let at = chrono::Utc::now().to_rfc3339();
        // Explicit existing-owner fixture: authorize a second profile for the
        // real enrolled subject/session, then use the real rotation command.
        tx.execute(
            "INSERT INTO profiles(profile_id,workspace_id,created_at) VALUES (?1,?2,?3)",
            rusqlite::params![profile.to_string(), f.access.workspace_id().to_string(), at],
        )
        .unwrap();
        tx.execute("INSERT INTO profile_grants(grant_id,workspace_id,profile_id,client_id,status,created_at) VALUES (?1,?2,?3,?4,'active',?5)", rusqlite::params![grant.to_string(),f.access.workspace_id().to_string(),profile.to_string(),f.access.client_id().to_string(),at]).unwrap();
        tx.execute(
            "INSERT INTO grant_scopes(grant_id,scope_key) VALUES (?1,'metadata_search')",
            [grant.to_string()],
        )
        .unwrap();
        tx.execute("INSERT INTO auth_subject_profile_grants(auth_subject_id,profile_grant_id) SELECT auth_subject_id,?1 FROM auth_subject_profile_grants WHERE profile_grant_id=?2",rusqlite::params![grant.to_string(),f.access.grant_id().to_string()]).unwrap();
        tx.execute("INSERT INTO fasti_browser_session_grants(browser_session_id,profile_grant_id) SELECT browser_session_id,?1 FROM fasti_browser_session_grants WHERE profile_grant_id=?2",rusqlite::params![grant.to_string(),f.access.grant_id().to_string()]).unwrap();
        tx.commit().unwrap();
        let ApplicationAccessContext::BrowserSession(proof) = &f.browser else {
            panic!("browser fixture")
        };
        let boundary =
            BrowserRequestBoundaryPolicy::try_new(FASTI_ACCESS_ORIGIN, FASTI_ACCESS_HOST).unwrap();
        let selected = f
            .kernel
            .select_browser_session_profile(SelectBrowserSessionProfileCommand::new(
                BrowserSessionMutationCommand::new(
                    RequestCorrelationId::new_v7(),
                    SecretMaterial::try_from_hex(&proof.session_secret().expose_hex()).unwrap(),
                    SecretMaterial::try_from_hex(&f.csrf).unwrap(),
                    boundary
                        .validate(Some(FASTI_ACCESS_ORIGIN), Some(FASTI_ACCESS_HOST))
                        .unwrap(),
                    chrono::Utc::now(),
                ),
                grant,
            ))
            .unwrap();
        let before = action_counts(&f);
        let (status, problem) = response(&f.app, local_request(&f, true, body.to_string())).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
        assert_eq!(problem["code"], "browser_session_revoked");
        let fresh_cookie = format!(
            "{}={}",
            local::SESSION_COOKIE,
            selected.session_secret().expose_hex()
        );
        let mut req = local_request(&f, true, body.to_string());
        req.headers_mut()
            .insert(header::COOKIE, fresh_cookie.parse().unwrap());
        let (status, problem) = response(&f.app, req).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
        assert_eq!(problem["code"], "validation_failed");
        body["after"] = serde_json::Value::Null;
        let mut req = local_request(&f, true, body.to_string());
        req.headers_mut()
            .insert(header::COOKIE, fresh_cookie.parse().unwrap());
        let (status, page) = response(&f.app, req).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page, serde_json::json!({"records":[],"next":null}));
        assert_eq!(action_counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }
}
