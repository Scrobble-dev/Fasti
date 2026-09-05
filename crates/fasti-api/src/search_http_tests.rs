#[cfg(target_os = "linux")]
mod search_http_tests {
    use super::*;
    use fasti_application::{
        provider_metadata_response_locale, ApplicationAccessContext, BrowserSessionAccessContext,
        BrowserSessionMutationCommand, BrowserSessionPort, ConfigurationDigest,
        CredentialReference, CredentialRequirement, CredentialSecret, CredentialVaultError,
        CredentialVaultPort, CredentialVaultSource, OutboundAccessPolicy, ProviderCapabilityId,
        ProviderCapabilityState, ProviderCapabilityStatus, ProviderCheckMetadata,
        ProviderCredentialStatus, ProviderId, ProviderStatePort, RequestAccessContext,
        SearchCandidate, SearchCandidateData, SearchPageRequest, SearchPersistencePort,
        SearchProviderQuery, StoredCredential,
    };
    use fasti_domain::SearchQuery;
    use fasti_provider_runtime::{ProviderRuntime, ProviderSearchService};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio_stream::StreamExt;

    const PATH: &str = "/api/v1/search/providers/tmdb";

    #[derive(Default)]
    struct CountingVault(AtomicUsize);

    impl CountingVault {
        fn forbidden<T>(&self) -> Result<T, CredentialVaultError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(CredentialVaultError::Unavailable)
        }
    }

    impl CredentialVaultPort for CountingVault {
        fn source(
            &self,
            _: &CredentialReference,
        ) -> Result<CredentialVaultSource, CredentialVaultError> {
            self.forbidden()
        }
        fn store(
            &self,
            _: &CredentialReference,
            _: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            self.forbidden()
        }
        fn replace(
            &self,
            _: &CredentialReference,
            _: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            self.forbidden()
        }
        fn load(&self, _: &CredentialReference) -> Result<CredentialSecret, CredentialVaultError> {
            self.forbidden()
        }
        fn revoke(&self, _: &CredentialReference) -> Result<(), CredentialVaultError> {
            self.forbidden()
        }
    }

    struct Fixture {
        _root: tempfile::TempDir,
        kernel: Arc<fasti_store::SqliteKernel>,
        runtime: Arc<ProviderRuntime>,
        vault: Arc<CountingVault>,
        app: Router,
        generic: Router,
        locks: ProviderOperationLocks,
        credential: String,
        access: RequestAccessContext,
        browser: ApplicationAccessContext,
        cookie: String,
        csrf: String,
    }

    async fn fixture() -> Fixture {
        let (root, kernel) = test_kernel();
        let bootstrap = api_router(kernel.clone(), test_bind_addr(), root.path());
        let enrolled = enroll_admin(&bootstrap, root.path()).await;
        let access = kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                RequestCorrelationId::new_v7(),
                CapabilityKey::SearchMetadata,
                SecretMaterial::try_from_hex(&enrolled.credential).unwrap(),
            ))
            .unwrap();
        let at = chrono::Utc::now() - chrono::TimeDelta::seconds(5);
        let installation = kernel
            .verify_trailbase_installation(VerifyTrailBaseInstallationCommand::new(
                TrailBaseInstanceId::new_v7(),
                Sha256Digest::from_bytes(&[31; 32]),
                Sha256Digest::from_bytes(&[32; 32]),
                false,
                RequestCorrelationId::new_v7(),
                at,
            ))
            .unwrap();
        let purpose = AuthCeremonyPurpose::FirstAdministratorBootstrap;
        let ceremony = AuthCeremony::try_new(
            OperationId::new_v7(),
            purpose,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            installation.id(),
            installation.activation_generation(),
            Sha256Digest::from_bytes(&[33; 32]),
            Some(
                AuthCeremonySelection::try_new(
                    purpose,
                    access.workspace_id(),
                    access.grant_id(),
                    None,
                    None,
                )
                .unwrap(),
            ),
            false,
            AuthCallbackPath::parse("/api/access/v1/trailbase/callback").unwrap(),
            purpose.return_target(),
            RequestCorrelationId::new_v7(),
            at,
            at + chrono::TimeDelta::minutes(10),
        )
        .unwrap();
        kernel
            .start_trailbase_bootstrap(StartTrailBaseBootstrapCommand::new(
                ceremony.clone(),
                kernel.ensure_bootstrap_secret().unwrap(),
            ))
            .unwrap();
        kernel
            .claim_auth_ceremony(ClaimAuthCeremonyCommand::new(
                ceremony.browser_binding_digest().clone(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path().clone(),
                RequestCorrelationId::new_v7(),
                at + chrono::TimeDelta::seconds(1),
            ))
            .unwrap();
        let authorization = PreauthorizeTrailBaseBootstrapCommand::new(
            ceremony.id(),
            ConfirmedTrailBaseIdentity::new(
                installation.id(),
                TrailBaseSubject::from_bytes([34; 16]),
                AuthenticationProvenance::new(
                    AuthenticationMethod::TrailBasePassword,
                    at + chrono::TimeDelta::seconds(2),
                    installation.activation_generation(),
                ),
            ),
            RequestCorrelationId::new_v7(),
            at + chrono::TimeDelta::seconds(2),
        );
        kernel
            .preauthorize_trailbase_bootstrap(authorization)
            .unwrap();
        let session = kernel
            .complete_trailbase_bootstrap(CompleteTrailBaseBootstrapCommand::new(
                authorization,
                kernel.ensure_bootstrap_secret().unwrap(),
            ))
            .unwrap();
        let csrf = session.csrf_secret().expose_hex();
        let cookie = format!(
            "{}={}; {}={csrf}",
            local::SESSION_COOKIE,
            session.session_secret().expose_hex(),
            local::CSRF_COOKIE
        );
        let boundary = DirectLoopbackAccessRuntime::new(
            kernel.clone(),
            FASTI_ACCESS_HOST.parse().unwrap(),
            false,
            root.path(),
            None,
        )
        .unwrap()
        .browser_boundary();
        let browser = BrowserSessionAccessContext::mutation(BrowserSessionMutationCommand::new(
            RequestCorrelationId::new_v7(),
            SecretMaterial::try_from_hex(&session.session_secret().expose_hex()).unwrap(),
            SecretMaterial::try_from_hex(&csrf).unwrap(),
            boundary
                .validate(Some(FASTI_ACCESS_ORIGIN), Some(FASTI_ACCESS_HOST))
                .unwrap(),
            chrono::Utc::now(),
        ))
        .into();
        kernel
            .put_provider_capability_state(
                access.workspace_id(),
                ProviderCapabilityState::try_new(
                    ProviderId::try_new("tmdb").unwrap(),
                    ProviderCapabilityId::try_new("metadata.search").unwrap(),
                    ProviderCapabilityStatus::Available,
                    1,
                    CredentialRequirement::BearerToken,
                    Some(CredentialReference::try_new("secret:search-http-test").unwrap()),
                    ProviderCredentialStatus::StoredUnverified,
                    ConfigurationDigest::parse("a".repeat(64)).unwrap(),
                    ProviderCheckMetadata::never_run(),
                    ProviderCheckMetadata::never_run(),
                )
                .unwrap(),
            )
            .unwrap();
        let vault = Arc::new(CountingVault::default());
        let runtime = Arc::new(ProviderRuntime::new(vault.clone()));
        let service = Arc::new(ProviderSearchService::new(runtime.clone(), kernel.clone()));
        let locks = ProviderOperationLocks::new(&runtime);
        let app = search_api_router(
            kernel.clone(),
            kernel.clone(),
            service.clone(),
            locks.clone(),
            Some(boundary),
        );
        let generic =
            search_api_router(kernel.clone(), kernel.clone(), service, locks.clone(), None);
        Fixture {
            _root: root,
            kernel,
            runtime,
            vault,
            app,
            generic,
            locks,
            credential: enrolled.credential,
            access,
            browser,
            cookie,
            csrf,
        }
    }

    fn body(query: &str) -> String {
        serde_json::json!({"query":query,"page":1,"locale":null,"region":null,"grains":[],"offline":true}).to_string()
    }

    fn request(f: &Fixture, browser: bool, path: &str, body: String) -> Request<Body> {
        let mut builder = Request::post(path).header(header::CONTENT_TYPE, "application/json");
        if browser {
            builder = builder
                .header(header::HOST, FASTI_ACCESS_HOST)
                .header(header::ORIGIN, FASTI_ACCESS_ORIGIN)
                .header(header::COOKIE, &f.cookie)
                .header(local::CSRF_HEADER, &f.csrf);
        } else {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", f.credential));
        }
        builder.body(Body::from(body)).unwrap()
    }

    async fn response(app: &Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
        let response = app.clone().oneshot(request).await.unwrap();
        decode_response(response).await
    }

    async fn decode_response(
        response: axum::response::Response,
    ) -> (StatusCode, serde_json::Value) {
        let status = response.status();
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
        if status.is_client_error() || status.is_server_error() {
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                "application/problem+json"
            );
        }
        let value =
            serde_json::from_slice(&to_bytes(response.into_body(), 256 * 1024).await.unwrap())
                .unwrap();
        (status, value)
    }

    fn observed_request(
        f: &Fixture,
        browser: bool,
    ) -> (Request<Body>, tokio::sync::oneshot::Receiver<()>) {
        let text = body("Dune");
        let mut req = request(f, browser, PATH, text.clone());
        let (entered, observed) = tokio::sync::oneshot::channel();
        let mut entered = Some(entered);
        // Axum consumes the body only after SearchPageAccess has completed its
        // real store authorization. The provider gate is already held by the
        // test, so the service cannot have started when this signal arrives.
        *req.body_mut() = Body::from_stream(
            tokio_stream::once(Ok::<_, std::io::Error>(axum::body::Bytes::from(text))).map(
                move |chunk| {
                    entered.take().unwrap().send(()).unwrap();
                    chunk
                },
            ),
        );
        (req, observed)
    }

    fn seed(f: &Fixture, browser: bool) -> String {
        let request = SearchPageRequest {
            correlation_id: RequestCorrelationId::new_v7(),
            access: if browser {
                f.browser.clone()
            } else {
                f.access.into()
            },
            query: SearchProviderQuery::try_new(
                SearchQuery::try_new("Dune").unwrap(),
                ProviderId::try_new("tmdb").unwrap(),
                1,
                provider_metadata_response_locale("tmdb", None),
                None,
                vec![],
            )
            .unwrap(),
            outbound_policy: OutboundAccessPolicy::default(),
            terms_revision: f
                .runtime
                .descriptor("tmdb")
                .unwrap()
                .cache_policy
                .to_owned(),
        };
        let prepared = f.kernel.prepare_search_page(&request).unwrap();
        let candidate = SearchCandidate::try_new(SearchCandidateData {
            provider: "tmdb".into(),
            provider_id: "438631".into(),
            kind: "movie".into(),
            title: "Dune".into(),
            original_title: None,
            release_year: Some(2021),
            authors: vec![],
            image_url: None,
            overview: None,
        })
        .unwrap();
        f.kernel
            .commit_search_page(
                &request,
                &prepared,
                &[candidate],
                &Sha256Digest::from_bytes(&[7; 32]),
                None,
            )
            .unwrap()
            .candidates[0]
            .id()
            .to_string()
    }

    fn counts(f: &Fixture) -> Vec<i64> {
        let connection = rusqlite::Connection::open(f.kernel.database_path()).unwrap();
        [
            "search_pages",
            "search_candidate_receipts",
            "records",
            "provider_capability_states",
        ]
        .into_iter()
        .map(|table| {
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap()
        })
        .collect()
    }

    #[tokio::test]
    async fn search_http_offline_real_receipts_preserve_actor_partition_without_vault_access() {
        let f = fixture().await;
        let bearer_receipt = seed(&f, false);
        let browser_receipt = seed(&f, true);
        assert_ne!(bearer_receipt, browser_receipt);
        let before = counts(&f);
        for (browser, receipt) in [(false, bearer_receipt), (true, browser_receipt)] {
            let (status, page) = response(&f.app, request(&f, browser, PATH, body("Dune"))).await;
            assert_eq!(status, StatusCode::OK, "{page}");
            assert_eq!(page["outcome"], "page");
            assert_eq!(page["cache_state"], "fresh");
            assert_eq!(page["candidates"][0]["candidate_receipt_id"], receipt);
            assert_eq!(page["candidates"][0]["candidate"]["title"], "Dune");
            let (status, miss) =
                response(&f.app, request(&f, browser, PATH, body("Not cached"))).await;
            assert_eq!(status, StatusCode::OK, "{miss}");
            assert_eq!(miss["outcome"], "unavailable");
            assert_eq!(miss["problem_code"], "provider_unavailable");
        }
        assert_eq!(counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn search_http_authority_precedes_malformed_body_and_provider() {
        let f = fixture().await;
        let before = counts(&f);
        for browser in [false, true] {
            let (status, problem) = response(&f.app, request(&f, browser, PATH, "{".into())).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{problem}");
        }
        let connection = rusqlite::Connection::open(f.kernel.database_path()).unwrap();
        connection
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = 'metadata_search'",
                [f.access.grant_id().to_string()],
            )
            .unwrap();
        for browser in [false, true] {
            for text in ["{", "{}"] {
                let (status, problem) = response(
                    &f.app,
                    request(
                        &f,
                        browser,
                        "/api/v1/search/providers/unknown-provider",
                        text.into(),
                    ),
                )
                .await;
                assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
            }
        }
        let unauthenticated = Request::post(PATH)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{"))
            .unwrap();
        let (status, problem) = response(&f.app, unauthenticated).await;
        // The existing browser mutation boundary denies an absent session
        // cookie before attempting JSON extraction; generic bearer-only routes
        // instead report authentication_failed.
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(problem["code"], "forbidden");
        assert_eq!(counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn search_http_authorized_representation_errors_are_typed_and_do_not_write_sources() {
        let f = fixture().await;
        seed(&f, false);
        seed(&f, true);
        let before = counts(&f);
        for browser in [false, true] {
            let cases = [
                (
                    PATH,
                    "{".to_owned(),
                    Some("application/json"),
                    StatusCode::BAD_REQUEST,
                    "malformed_json",
                ),
                (
                    PATH,
                    " ".repeat(8 * 1024 + 1),
                    Some("application/json"),
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "payload_too_large",
                ),
                (
                    PATH,
                    body("Dune"),
                    Some("text/plain"),
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "unsupported_media_type",
                ),
                (
                    PATH,
                    body("Dune"),
                    None,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    "unsupported_media_type",
                ),
                (
                    PATH,
                    "{}".to_owned(),
                    Some("application/json"),
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "validation_failed",
                ),
                (
                    "/api/v1/search/providers/%FF",
                    body("Dune"),
                    Some("application/json"),
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "validation_failed",
                ),
            ];
            for (path, text, content_type, expected_status, expected_code) in cases {
                let mut req = request(&f, browser, path, text);
                if let Some(content_type) = content_type {
                    req.headers_mut()
                        .insert(header::CONTENT_TYPE, content_type.parse().unwrap());
                } else {
                    req.headers_mut().remove(header::CONTENT_TYPE);
                }
                let (status, problem) = response(&f.app, req).await;
                assert_eq!(status, expected_status, "{problem}");
                assert_eq!(problem["code"], expected_code);
                assert_eq!(problem["capability_id"], "metadata.search");
            }
        }
        assert_eq!(counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn search_http_cookie_boundary_denies_missing_csrf_mixed_auth_and_generic_listener() {
        let f = fixture().await;
        seed(&f, true);
        let before = counts(&f);
        for header_name in [local::CSRF_HEADER, "origin", "host"] {
            let mut req = request(&f, true, PATH, body("Dune"));
            req.headers_mut().remove(header_name);
            let (status, problem) = response(&f.app, req).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
        }
        let mut mixed = request(&f, true, PATH, body("Dune"));
        mixed.headers_mut().insert(
            header::AUTHORIZATION,
            format!("Bearer {}", f.credential).parse().unwrap(),
        );
        assert_eq!(response(&f.app, mixed).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            response(&f.generic, request(&f, true, PATH, body("Dune")))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, result) =
            response(&f.generic, request(&f, false, PATH, body("Not cached"))).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["outcome"], "unavailable");
        assert_eq!(counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn search_http_cancelled_gate_waiter_never_reads_cached_receipts_or_vault() {
        for browser in [false, true] {
            let f = fixture().await;
            seed(&f, browser);
            let before = counts(&f);
            let gate = f.locks.get("tmdb").unwrap();
            let held = gate.clone().lock_owned().await;
            let (req, authorized) = observed_request(&f, browser);
            let caller = tokio::spawn(f.app.clone().oneshot(req));
            tokio::time::timeout(std::time::Duration::from_secs(5), authorized)
                .await
                .unwrap()
                .unwrap();
            assert!(
                !caller.is_finished(),
                "held provider gate must block the response"
            );
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
            drop(held);
            let next = tokio::time::timeout(std::time::Duration::from_secs(5), gate.lock_owned())
                .await
                .expect("cancelled waiter must not retain the provider gate");
            drop(next);
            assert_eq!(counts(&f), before);
            assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
        }
    }

    #[tokio::test]
    async fn search_http_real_router_boundaries_exclude_health_and_integration_and_remote_cookies()
    {
        let f = fixture().await;
        seed(&f, true);
        let before = counts(&f);
        for app in [health_router(), integration_router(f.kernel.clone())] {
            for browser in [false, true] {
                let reply = app
                    .clone()
                    .oneshot(request(&f, browser, PATH, body("Dune")))
                    .await
                    .unwrap();
                assert_eq!(reply.status(), StatusCode::NOT_FOUND);
            }
        }
        // Exercise the actual remote router constructor and Search merge.
        // Selection of this composition by fastid is separately source-checked.
        let remote = remote_api_router(
            f.kernel.clone(),
            "0.0.0.0:8420".parse().unwrap(),
            f._root.path(),
        )
        .merge(f.generic.clone());
        let (status, problem) = response(&remote, request(&f, true, PATH, body("Dune"))).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(problem["code"], "authentication_failed");
        let (status, result) =
            response(&remote, request(&f, false, PATH, body("Not cached"))).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["outcome"], "unavailable");
        assert_eq!(counts(&f), before);
        assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn search_http_gate_waiter_rechecks_revoked_authority_before_cached_disclosure() {
        for (browser, revoke_session) in [(false, false), (true, false), (true, true)] {
            let f = fixture().await;
            let receipt = seed(&f, browser);
            let before = counts(&f);
            let held = f.locks.get("tmdb").unwrap().lock_owned().await;
            let (req, authorized) = observed_request(&f, browser);
            let caller = tokio::spawn(f.app.clone().oneshot(req));
            tokio::time::timeout(std::time::Duration::from_secs(5), authorized)
                .await
                .unwrap()
                .unwrap();
            assert!(!caller.is_finished());
            let (expected_status, expected_code) = if revoke_session {
                let ApplicationAccessContext::BrowserSession(proof) = &f.browser else {
                    panic!("browser fixture");
                };
                let boundary =
                    BrowserRequestBoundaryPolicy::try_new(FASTI_ACCESS_ORIGIN, FASTI_ACCESS_HOST)
                        .unwrap();
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
                (StatusCode::UNAUTHORIZED, "browser_session_revoked")
            } else {
                rusqlite::Connection::open(f.kernel.database_path()).unwrap().execute(
                    "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = 'metadata_search'",
                    [f.access.grant_id().to_string()],
                ).unwrap();
                (StatusCode::FORBIDDEN, "forbidden")
            };
            drop(held);
            let result = tokio::time::timeout(std::time::Duration::from_secs(5), caller)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let (status, problem) = decode_response(result).await;
            assert_eq!(status, expected_status, "{problem}");
            assert_eq!(problem["code"], expected_code);
            assert_eq!(problem["capability_id"], "metadata.search");
            assert!(problem.get("candidates").is_none());
            assert!(!problem.to_string().contains(&receipt));
            assert_eq!(counts(&f), before);
            assert_eq!(f.vault.0.load(Ordering::SeqCst), 0);
        }
    }
}
