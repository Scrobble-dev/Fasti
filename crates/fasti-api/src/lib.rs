//! Fasti HTTP REST API definitions and router construction.

use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use fasti_application::LocalKernel;
use fasti_contracts::{HealthResponse, ProblemActionDto, ProblemDetails, ViolationDto};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tower_http::services::{ServeDir, ServeFile};
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

mod browser_auth;
mod integrations;
mod local;
mod nuvio_collections;
mod observation;
mod problem;
mod profile_state;
mod records;

#[cfg(feature = "conformance-fixture")]
mod conformance;

#[cfg(feature = "conformance-fixture")]
pub use conformance::{b1_conformance_openapi, b1_conformance_router};

#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "system",
    responses(
        (status = 200, description = "The Fasti service is healthy", body = HealthResponse)
    )
)]
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
}

struct ProductionSecurityAddon;

impl Modify for ProductionSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bootstrap_bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("64-character lowercase hexadecimal secret")
                        .description(Some(
                            "One-time local data-root bootstrap secret. Never use an enrolled client credential here.",
                        ))
                        .build(),
                ),
            );
            components.add_security_scheme(
                "credential_bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("64-character lowercase hexadecimal credential")
                        .description(Some(
                            "Enrolled Fasti client credential sent only in the Authorization header.",
                        ))
                        .build(),
                ),
            );
            components.add_security_scheme(
                "browser_session",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    "fasti_session",
                    "Opaque HttpOnly browser session cookie",
                ))),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        local::initialize_node,
        local::enroll_first_client,
        browser_auth::create_session,
        browser_auth::read_session,
        browser_auth::end_session,
        browser_auth::list_users,
        browser_auth::update_user,
        browser_auth::delete_user,
        nuvio_collections::clear_nuvio_collections,
        nuvio_collections::get_nuvio_collections,
        nuvio_collections::replace_nuvio_collections,
        observation::submit_observation,
        profile_state::list_tracking_dispositions,
        profile_state::set_tracking_disposition,
        records::create_record,
        records::attach_identifier,
        records::list_records,
        records::register_namespace,
        integrations::integration_status,
        integrations::nuvio_webhook,
        integrations::tautulli_webhook,
        integrations::jellyfin_webhook,
        integrations::emby_webhook,
        integrations::plex_webhook
    ),
    components(schemas(
        HealthResponse,
        fasti_contracts::AttachIdentifierRequest,
        fasti_contracts::AttachIdentifierResponse,
        fasti_contracts::BrowserSessionResponse,
        fasti_contracts::BrowserUserDto,
        fasti_contracts::ClientEnrollmentResponse,
        fasti_contracts::CreateBrowserSessionRequest,
        fasti_contracts::CreateRecordRequest,
        fasti_contracts::CreateRecordResponse,
        fasti_contracts::CredentialSchemeDto,
        fasti_contracts::EnrollFirstClientRequest,
        fasti_contracts::InitializeNodeRequest,
        fasti_contracts::DeleteBrowserUserRequest,
        fasti_contracts::IntegrationObservationRequest,
        fasti_contracts::IntegrationStatusDto,
        fasti_contracts::IntegrationStatusListResponse,
        fasti_contracts::ListRecordsResponse,
        fasti_contracts::ListBrowserUsersResponse,
        fasti_contracts::ListTrackingDispositionsResponse,
        fasti_contracts::NodeInitializationResponse,
        fasti_contracts::NuvioCatalogSourceDto,
        fasti_contracts::NuvioCollectionDto,
        fasti_contracts::NuvioCollectionFolderDto,
        fasti_contracts::NuvioCollectionSourceDto,
        fasti_contracts::NuvioCollectionsDocumentDto,
        fasti_contracts::NuvioCollectionsStateDto,
        fasti_contracts::ObservationIdentifierInput,
        fasti_contracts::ObservationIngressKind,
        fasti_contracts::RecordActivityDto,
        fasti_contracts::RecordIdentifierDto,
        fasti_contracts::RecordSummaryDto,
        fasti_contracts::RegisterNamespaceRequest,
        fasti_contracts::RegisterNamespaceResponse,
        fasti_contracts::ResolvedFieldDto,
        fasti_contracts::SetTrackingDispositionRequest,
        fasti_contracts::SubmitObservationRequest,
        fasti_contracts::SubmitObservationResponse,
        fasti_contracts::TrackingDispositionDto,
        fasti_contracts::TrackingDispositionStateDto,
        fasti_contracts::TrackingDispositionUpdateDto,
        fasti_contracts::UpdateBrowserUserRequest,
        ProblemActionDto,
        ProblemDetails,
        ViolationDto
    )),
    modifiers(&ProductionSecurityAddon)
)]
struct ApiDoc;

/// Builds the OpenAPI 3.1 contract for routes actually mounted by [`api_router`]
/// and the dedicated integration listener.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Constructs the public health router used alone when no data root is configured
/// and as the common base for both durable routers.
pub fn health_router() -> Router {
    Router::new().route("/api/v1/health", get(health_check))
}

/// Constructs the dedicated integration ingress surface.
///
/// It intentionally exposes only health, integration status, and authenticated
/// provider adapters. Node bootstrap, generic record mutation, and the generic
/// observation endpoint are never mounted here.
pub fn integration_router(kernel: Arc<dyn LocalKernel>) -> Router {
    let state = local::LocalApiState {
        kernel,
        secure_cookies: false,
        // Unused: this router never mounts local::router()'s browser session
        // endpoints, only the isolated webhook adapters.
        max_session_minutes: fasti_application::MAX_SESSION_MINUTES,
    };
    health_router().merge(integrations::router().with_state(state))
}

/// Constructs the durable local API router for fastid.
///
/// # Contract
///
/// This function merges health and durable local routes and validates that:
/// - `local_exposure_addr` is the effective loopback address clients use
///   directly or through a trusted loopback-only port forward (panics if not)
/// - `data_root` is non-empty (panics if empty)
///
/// These validations enforce the local durable route security model: the router
/// must stay on direct loopback or an explicitly declared loopback-only port
/// forward and must have an explicit data root. Intentional non-loopback durable
/// listeners use [`remote_api_router`]; missing data roots use [`health_router`].
///
/// # Panics
///
/// Panics if `local_exposure_addr` is not a loopback address, if `data_root` is
/// empty, or if the bootstrap secret cannot be prepared (durable state is
/// unavailable at startup either way; failing fast here matches every other
/// durable precondition this function already enforces).
pub fn api_router(
    kernel: Arc<dyn LocalKernel>,
    local_exposure_addr: SocketAddr,
    data_root: &Path,
    max_session_minutes: u32,
) -> Router {
    assert!(
        local_exposure_addr.ip().is_loopback(),
        "api_router requires loopback client exposure, got non-loopback {local_exposure_addr}"
    );
    assert!(
        !data_root.as_os_str().is_empty(),
        "api_router requires non-empty data_root"
    );
    // Primed here, before the router serves anything, so a legitimate first
    // client can read <data_root>/bootstrap.secret and present it back to
    // /api/v1/node/initialization -- see
    // AccessAdministrationPort::ensure_bootstrap_secret.
    kernel
        .ensure_bootstrap_secret()
        .expect("bootstrap secret must be preparable before serving any route");
    let integration_state = local::LocalApiState {
        kernel: Arc::clone(&kernel),
        secure_cookies: false,
        max_session_minutes,
    };
    health_router()
        .merge(local::router(kernel, true, false, max_session_minutes))
        .merge(integrations::router().with_state(integration_state))
}

/// Constructs the authenticated durable router for a non-loopback listener.
/// The daemon enables this only behind an explicitly configured HTTPS proxy.
pub fn remote_api_router(
    kernel: Arc<dyn LocalKernel>,
    bind_addr: SocketAddr,
    data_root: &Path,
    max_session_minutes: u32,
) -> Router {
    assert!(
        !bind_addr.ip().is_loopback(),
        "remote_api_router requires a non-loopback bind address"
    );
    assert!(
        !data_root.as_os_str().is_empty(),
        "remote_api_router requires non-empty data_root"
    );
    let integration_state = local::LocalApiState {
        kernel: Arc::clone(&kernel),
        secure_cookies: true,
        max_session_minutes,
    };
    health_router()
        .merge(local::router(kernel, false, true, max_session_minutes))
        .merge(integrations::router().with_state(integration_state))
}

/// Adds a static-file fallback to `router`, serving a pre-built single-page
/// app from `static_dir` for any request that doesn't match an `/api/*`
/// route. A missing file (including client-side routes like `/status`)
/// falls back to `static_dir/index.html`, so the SPA's own router handles
/// the path. When `static_dir` is `None`, `router` is returned unchanged --
/// existing callers that never pass a static dir see no behavior change.
///
/// This is applied once, after the durable/remote/health router is chosen,
/// rather than duplicated into each of those three constructors above.
pub fn with_static_fallback(router: Router, static_dir: Option<&Path>) -> Router {
    let Some(static_dir) = static_dir else {
        return router;
    };
    // `.fallback()`, not `.not_found_service()` -- the latter forces the
    // response status to 404, which is right for a custom error page but
    // wrong for SPA client-side routing: `/status` is a real page in the
    // app, so it must come back 200 with index.html for the SPA's own
    // router to take over. That SPA behavior must not extend to `/api/*`,
    // though: an unmatched API path (a typo, a removed endpoint) would
    // otherwise silently come back 200 with the HTML shell instead of a 404,
    // which every API client -- browser fetch, SDK, curl -- expects to see.
    let index_html = static_dir.join("index.html");
    let serve_dir = ServeDir::new(static_dir).fallback(ServeFile::new(index_html));
    router.fallback_service(tower::service_fn(move |request: Request| {
        let serve_dir = serve_dir.clone();
        async move {
            if request.uri().path().starts_with("/api/") {
                return Ok::<Response, std::convert::Infallible>(
                    StatusCode::NOT_FOUND.into_response(),
                );
            }
            let response = tower::ServiceExt::oneshot(serve_dir, request)
                .await
                .into_response();
            Ok(response)
        }
    }))
}

/// Seeds the one-time development account with the given credential. The
/// durable marker prevents this call from recreating an account after it is
/// renamed or deleted. Returns `true` when this call actually created the
/// account (the caller should surface `password` to the operator then), or
/// `false` when an account already existed (`password` was not used).
pub fn ensure_development_test_account(
    kernel: &dyn LocalKernel,
    password: fasti_application::BrowserPassword,
) -> fasti_application::ApplicationResult<bool> {
    kernel.ensure_development_browser_user(
        fasti_application::BrowserUsername::try_new("testadmin")
            .expect("development username is valid"),
        password,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Method, Request, StatusCode},
    };
    #[cfg(target_os = "linux")]
    use fasti_application::{
        AccessAdministrationPort, AuthenticateCredentialQuery, CapabilityKey, SecretMaterial,
    };
    use tower::ServiceExt;
    use utoipa::openapi::OpenApiVersion;

    #[cfg(target_os = "linux")]
    fn test_kernel() -> (tempfile::TempDir, Arc<fasti_store::SqliteKernel>) {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = Arc::new(fasti_store::SqliteKernel::open(root.path()).expect("SQLite kernel"));
        (root, kernel)
    }

    #[cfg(target_os = "linux")]
    fn test_bind_addr() -> SocketAddr {
        "127.0.0.1:8420".parse().expect("loopback address")
    }

    #[tokio::test]
    async fn static_fallback_is_a_no_op_when_no_dir_is_configured() {
        let router = with_static_fallback(health_router(), None);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/nonexistent")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        // No static dir configured -> unmatched routes still 404, exactly as
        // health_router() alone behaves. This is the regression guard: adding
        // FASTI_STATIC_DIR support must not change behavior when it's unset.
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn static_fallback_serves_index_html_for_unmatched_spa_routes() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(
            static_dir.path().join("index.html"),
            "<html>fasti workbench</html>",
        )
        .expect("write index.html");

        // The real API route still wins over the static fallback.
        let response = with_static_fallback(health_router(), Some(static_dir.path()))
            .oneshot(
                Request::builder()
                    .uri("/api/v1/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);

        // A client-side route (not a real file, not an API route) falls
        // back to index.html so the SPA's own router can handle it.
        let response = with_static_fallback(health_router(), Some(static_dir.path()))
            .oneshot(
                Request::builder()
                    .uri("/status")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert_eq!(body, "<html>fasti workbench</html>".as_bytes());
    }

    #[tokio::test]
    async fn static_fallback_serves_a_real_asset_file_directly() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(static_dir.path().join("index.html"), "shell").expect("write index.html");
        std::fs::write(static_dir.path().join("app.js"), "console.log(1)").expect("write app.js");

        let router = with_static_fallback(health_router(), Some(static_dir.path()));

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/app.js")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert_eq!(body, "console.log(1)".as_bytes());
    }

    #[tokio::test]
    async fn static_fallback_leaves_unmatched_api_paths_as_a_plain_404() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(
            static_dir.path().join("index.html"),
            "<html>fasti workbench</html>",
        )
        .expect("write index.html");

        // A path under /api/* that no route matches must not fall back to
        // the SPA shell with a 200 -- every API client expects a 404 there,
        // not an HTML document.
        let response = with_static_fallback(health_router(), Some(static_dir.path()))
            .oneshot(
                Request::get("/api/v1/not-a-real-route")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert!(
            !String::from_utf8_lossy(&body).contains("fasti workbench"),
            "an unmatched API path must not receive the SPA shell"
        );
    }

    #[tokio::test]
    async fn static_fallback_rejects_non_get_methods_with_not_found_not_method_not_allowed() {
        let static_dir = tempfile::tempdir().expect("temporary static dir");
        std::fs::write(static_dir.path().join("index.html"), "shell").expect("write index.html");

        let router = with_static_fallback(health_router(), Some(static_dir.path()));

        // A route this server never registers must stay a uniform 404 for
        // every method -- not 405, which would leak "a route matched this
        // path" and contradict SECURITY.md's absent-route guarantee.
        let response = router
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/events")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }


    #[test]
    fn openapi_is_3_1_and_documents_the_real_routes() {
        let document = openapi();

        assert!(matches!(document.openapi, OpenApiVersion::Version31));
        for path in [
            "/api/v1/health",
            "/api/v1/node/initialization",
            "/api/v1/client-enrollments",
            "/api/v1/browser/session",
            "/api/v1/browser/users",
            "/api/v1/browser/users/{user_id}",
            "/api/v1/observations",
            "/api/v1/records",
            "/api/v1/records/identifiers",
            "/api/v1/namespaces",
            "/api/v1/integrations",
            "/api/v1/integrations/nuvio/webhook",
            "/api/v1/integrations/tautulli/webhook",
            "/api/v1/integrations/jellyfin/webhook",
            "/api/v1/integrations/emby/webhook",
            "/api/v1/integrations/plex/webhook",
            "/api/v1/profile/record-tracking-dispositions",
            "/api/v1/profile/record-tracking-dispositions/{record_id}",
            "/api/v1/profile/nuvio-collections",
        ] {
            assert!(document.paths.paths.contains_key(path), "missing {path}");
        }
        assert_eq!(document.paths.paths.len(), 19);

        let serialized = serde_json::to_string(&document).expect("serializable OpenAPI document");
        assert!(serialized.contains("#/components/schemas/HealthResponse"));
        let value = serde_json::to_value(&document).expect("OpenAPI JSON value");
        assert_eq!(
            value.pointer("/components/securitySchemes/browser_session/type"),
            Some(&serde_json::json!("apiKey"))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/credential_bearer/scheme"),
            Some(&serde_json::json!("bearer"))
        );
        assert_eq!(
            value.pointer("/components/securitySchemes/bootstrap_bearer/scheme"),
            Some(&serde_json::json!("bearer"))
        );
        assert_eq!(
            value
                .pointer("/paths/~1api~1v1~1records/get/security")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(2)
        );
        assert_eq!(
            value
                .pointer("/paths/~1api~1v1~1browser~1session/get/security/0/browser_session")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert!(value
            .pointer("/paths/~1api~1v1~1health/get/security")
            .is_none());
        assert!(value
            .pointer("/paths/~1api~1v1~1client-enrollments/post/security")
            .is_none());

        let schemas = &document.components.expect("OpenAPI components").schemas;
        for schema in [
            "HealthResponse",
            "NodeInitializationResponse",
            "NuvioCatalogSourceDto",
            "NuvioCollectionDto",
            "NuvioCollectionFolderDto",
            "NuvioCollectionSourceDto",
            "NuvioCollectionsDocumentDto",
            "NuvioCollectionsStateDto",
            "ClientEnrollmentResponse",
            "CreateBrowserSessionRequest",
            "BrowserSessionResponse",
            "BrowserUserDto",
            "ListBrowserUsersResponse",
            "UpdateBrowserUserRequest",
            "DeleteBrowserUserRequest",
            "ObservationIdentifierInput",
            "ObservationIngressKind",
            "SubmitObservationRequest",
            "SubmitObservationResponse",
            "IntegrationObservationRequest",
            "IntegrationStatusDto",
            "IntegrationStatusListResponse",
            "AttachIdentifierRequest",
            "AttachIdentifierResponse",
            "CreateRecordRequest",
            "CreateRecordResponse",
            "ListRecordsResponse",
            "ListTrackingDispositionsResponse",
            "RecordActivityDto",
            "RecordIdentifierDto",
            "RecordSummaryDto",
            "RegisterNamespaceRequest",
            "RegisterNamespaceResponse",
            "ResolvedFieldDto",
            "SetTrackingDispositionRequest",
            "TrackingDispositionDto",
            "TrackingDispositionStateDto",
            "TrackingDispositionUpdateDto",
            "ProblemActionDto",
            "ProblemDetails",
            "ViolationDto",
        ] {
            assert!(
                schemas.contains_key(schema),
                "missing shared schema {schema}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn documented_health_route_is_mounted() {
        let (root, kernel) = test_kernel();
        let response = api_router(
            kernel,
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        )
        .oneshot(
            Request::get("/api/v1/health")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn nuvio_collections_replace_get_and_clear_use_the_authenticated_profile() {
        let (root, kernel) = test_kernel();
        let app = api_router(
            kernel,
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );
        let credential = enroll_admin(&app, root.path()).await.credential;
        let document = r#"[{"id":"collection","title":"Collection","folders":[{"id":"folder","title":"Folder","sources":[{"provider":"tmdb","tmdbSourceType":"discover","mediaType":"movie","filters":{"voteCountGte":10,"vote_count.gte":10},"id":"source"}]}]}]"#;

        let replaced = app
            .clone()
            .oneshot(
                Request::put("/api/v1/profile/nuvio-collections")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::from(document))
                    .expect("replace request"),
            )
            .await
            .expect("replace response");
        assert_eq!(replaced.status(), StatusCode::OK);
        let replaced: fasti_contracts::NuvioCollectionsStateDto = serde_json::from_slice(
            &to_bytes(replaced.into_body(), 64 * 1024)
                .await
                .expect("bounded replace body"),
        )
        .expect("replace state");
        let replaced = serde_json::to_value(replaced.document.expect("stored document"))
            .expect("document JSON");
        assert_eq!(
            replaced[0]["folders"][0]["sources"][0]["mediaType"],
            "MOVIE"
        );
        assert_eq!(
            replaced[0]["folders"][0]["sources"][0]["filters"]["vote_count.gte"],
            10
        );

        let read = app
            .clone()
            .oneshot(
                Request::get("/api/v1/profile/nuvio-collections")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("get request"),
            )
            .await
            .expect("get response");
        assert_eq!(read.status(), StatusCode::OK);
        let read: fasti_contracts::NuvioCollectionsStateDto = serde_json::from_slice(
            &to_bytes(read.into_body(), 64 * 1024)
                .await
                .expect("bounded get body"),
        )
        .expect("get state");
        assert!(read.document.is_some());

        let cleared = app
            .oneshot(
                Request::delete("/api/v1/profile/nuvio-collections")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("clear request"),
            )
            .await
            .expect("clear response");
        assert_eq!(cleared.status(), StatusCode::OK);
        let cleared: fasti_contracts::NuvioCollectionsStateDto = serde_json::from_slice(
            &to_bytes(cleared.into_body(), 4096)
                .await
                .expect("bounded clear body"),
        )
        .expect("clear state");
        assert!(cleared.document.is_none());
    }

    #[cfg(target_os = "linux")]
    async fn enroll_admin(
        app: &Router,
        data_root: &std::path::Path,
    ) -> fasti_contracts::ClientEnrollmentResponse {
        // api_router primes this file at construction time -- read it the
        // same way a legitimate first client would, proving local
        // filesystem access to this data root.
        let bootstrap_secret = std::fs::read_to_string(data_root.join("bootstrap.secret"))
            .expect("bootstrap secret readable after api_router primes it");
        let initialized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(initialized.status(), StatusCode::OK);
        let initialized: fasti_contracts::NodeInitializationResponse = serde_json::from_slice(
            &to_bytes(initialized.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("initialization response");

        let enrollment_request = serde_json::json!({
            "initialization_proof": initialized.initialization_proof
        });
        let enrolled = app
            .clone()
            .oneshot(
                Request::post("/api/v1/client-enrollments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(enrollment_request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(enrolled.status(), StatusCode::OK);
        serde_json::from_slice(
            &to_bytes(enrolled.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("enrollment response")
    }

    #[cfg(target_os = "linux")]
    fn browser_cookie_pair(response: &axum::response::Response) -> (String, String) {
        let cookies: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| {
                value
                    .to_str()
                    .expect("set-cookie header")
                    .split(';')
                    .next()
                    .expect("cookie pair")
                    .to_owned()
            })
            .collect();
        let csrf = cookies
            .iter()
            .find_map(|cookie| cookie.strip_prefix("fasti_csrf="))
            .expect("CSRF cookie")
            .to_owned();
        (cookies.join("; "), csrf)
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn development_browser_account_signs_in_edits_and_retains_the_last_administrator() {
        let (root, kernel) = test_kernel();
        ensure_development_test_account(
            kernel.as_ref(),
            fasti_application::BrowserPassword::try_new("testadmin").expect("test password"),
        )
        .expect("seed test account");
        let app = api_router(
            kernel.clone(),
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );

        let login = app
            .clone()
            .oneshot(
                Request::post("/api/v1/browser/session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"testadmin","password":"testadmin","session_timeout_minutes":60}"#,
                    ))
                    .expect("login request"),
            )
            .await
            .expect("login response");
        assert_eq!(login.status(), StatusCode::OK);
        assert_eq!(
            login
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );
        let (cookies, csrf) = browser_cookie_pair(&login);
        let session: fasti_contracts::BrowserSessionResponse =
            serde_json::from_slice(&to_bytes(login.into_body(), 4096).await.expect("login body"))
                .expect("session response");

        let listed = app
            .clone()
            .oneshot(
                Request::get("/api/v1/records")
                    .header(header::COOKIE, &cookies)
                    .body(Body::empty())
                    .expect("list request"),
            )
            .await
            .expect("list response");
        assert_eq!(listed.status(), StatusCode::OK);

        let missing_csrf = app
            .clone()
            .oneshot(
                Request::post("/api/v1/records")
                    .header(header::COOKIE, &cookies)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"film"}"#))
                    .expect("create request"),
            )
            .await
            .expect("create response");
        assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

        let created = app
            .clone()
            .oneshot(
                Request::post("/api/v1/records")
                    .header(header::COOKIE, &cookies)
                    .header("x-fasti-csrf", &csrf)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"film"}"#))
                    .expect("create request"),
            )
            .await
            .expect("create response");
        assert_eq!(created.status(), StatusCode::OK);

        let collections = app
            .clone()
            .oneshot(
                Request::put("/api/v1/profile/nuvio-collections")
                    .header(header::COOKIE, &cookies)
                    .header("x-fasti-csrf", &csrf)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"[{"id":"remote","title":"Remote","folders":[]}]"#,
                    ))
                    .expect("Nuvio Collections request"),
            )
            .await
            .expect("Nuvio Collections response");
        assert_eq!(collections.status(), StatusCode::OK);

        let updated = app
            .clone()
            .oneshot(
                Request::patch(format!(
                    "/api/v1/browser/users/{}",
                    session.user.user_id
                ))
                .header(header::COOKIE, &cookies)
                .header("x-fasti-csrf", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    r#"{"current_password":"testadmin","username":"editedadmin","password":"editedadmin","active":null}"#,
                ))
                .expect("update request"),
            )
            .await
            .expect("update response");
        assert_eq!(updated.status(), StatusCode::OK);

        let expired = app
            .clone()
            .oneshot(
                Request::get("/api/v1/browser/session")
                    .header(header::COOKIE, &cookies)
                    .body(Body::empty())
                    .expect("session request"),
            )
            .await
            .expect("session response");
        assert_eq!(expired.status(), StatusCode::UNAUTHORIZED);

        let login = app
            .clone()
            .oneshot(
                Request::post("/api/v1/browser/session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"editedadmin","password":"editedadmin","session_timeout_minutes":60}"#,
                    ))
                    .expect("login request"),
            )
            .await
            .expect("login response");
        assert_eq!(login.status(), StatusCode::OK);
        let (cookies, csrf) = browser_cookie_pair(&login);

        let deleted = app
            .oneshot(
                Request::delete(format!("/api/v1/browser/users/{}", session.user.user_id))
                    .header(header::COOKIE, &cookies)
                    .header("x-fasti-csrf", &csrf)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"current_password":"editedadmin"}"#))
                    .expect("delete request"),
            )
            .await
            .expect("delete response");
        assert_eq!(deleted.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let problem: fasti_contracts::ProblemDetails = serde_json::from_slice(
            &to_bytes(deleted.into_body(), 4096)
                .await
                .expect("delete problem body"),
        )
        .expect("delete problem");
        assert_eq!(problem.code, "validation_failed");
        assert_eq!(problem.violations.len(), 1);
        assert_eq!(
            problem.violations[0].code,
            "last_active_administrator_required"
        );
        assert_eq!(problem.violations[0].pointer, "/");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn remote_router_omits_bootstrap_and_sets_secure_session_cookies() {
        let (root, kernel) = test_kernel();
        ensure_development_test_account(
            kernel.as_ref(),
            fasti_application::BrowserPassword::try_new("testadmin").expect("test password"),
        )
        .expect("seed test account");
        let app = remote_api_router(
            kernel,
            "0.0.0.0:8420".parse().expect("remote bind address"),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );

        let bootstrap = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .body(Body::empty())
                    .expect("bootstrap request"),
            )
            .await
            .expect("bootstrap response");
        assert_eq!(bootstrap.status(), StatusCode::NOT_FOUND);

        let login = app
            .oneshot(
                Request::post("/api/v1/browser/session")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"username":"testadmin","password":"testadmin","session_timeout_minutes":60}"#,
                    ))
                    .expect("login request"),
            )
            .await
            .expect("login response");
        assert_eq!(login.status(), StatusCode::OK);
        let set_cookies: Vec<_> = login
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().expect("set-cookie header"))
            .collect();
        assert_eq!(set_cookies.len(), 2);
        assert!(set_cookies.iter().all(|cookie| cookie.contains("; Secure")));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn node_initialization_refuses_a_missing_or_wrong_bootstrap_secret() {
        let (root, kernel) = test_kernel();
        let app = api_router(
            kernel,
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );

        let missing_header = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(missing_header.status(), StatusCode::FORBIDDEN);

        let wrong_secret = SecretMaterial::from_bytes([7_u8; 32]).expose_hex();
        let wrong_header = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {wrong_secret}"))
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(wrong_header.status(), StatusCode::FORBIDDEN);

        // A second process that can read the same data root -- exactly the
        // legitimate-first-client scenario this whole mechanism exists for --
        // is not blocked by a wrong attempt that came before it.
        let bootstrap_secret = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after api_router primes it");
        let correct_header = app
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(correct_header.status(), StatusCode::OK);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn bootstrap_secret_survives_a_router_rebuild_and_has_owner_only_permissions() {
        let (root, kernel) = test_kernel();
        let _first_router = api_router(
            kernel.clone(),
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );
        let first_read = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after first priming");

        // Simulates a daemon restart against the same data root: a second
        // api_router build must not regenerate (and thereby invalidate) the
        // secret a legitimate client may have already read.
        let _second_router = api_router(
            kernel,
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );
        let second_read = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after second priming");
        assert_eq!(first_read, second_read);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(root.path().join("bootstrap.secret"))
                .expect("bootstrap secret metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                mode, 0o600,
                "bootstrap secret must be owner-read-write only"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn durable_bootstrap_issues_one_credential_and_closes_initialization() {
        let (root, kernel) = test_kernel();
        let app = api_router(
            kernel.clone(),
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );
        let bootstrap_secret = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret readable after api_router primes it");

        let initialized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(initialized.status(), StatusCode::OK);
        let initialized: fasti_contracts::NodeInitializationResponse = serde_json::from_slice(
            &to_bytes(initialized.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("initialization response");

        let invalid_secret = "not-a-secret";
        let denied = app
            .clone()
            .oneshot(
                Request::post("/api/v1/client-enrollments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "initialization_proof": invalid_secret }).to_string(),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(denied.status(), StatusCode::CONFLICT);
        let denied_body = to_bytes(denied.into_body(), 4096)
            .await
            .expect("bounded body");
        assert!(!String::from_utf8_lossy(&denied_body).contains(invalid_secret));
        let denied: ProblemDetails =
            serde_json::from_slice(&denied_body).expect("problem response");
        assert_eq!(denied.code, "bootstrap_closed");

        let enrollment_request = serde_json::json!({
            "initialization_proof": initialized.initialization_proof
        });
        let enrolled = app
            .clone()
            .oneshot(
                Request::post("/api/v1/client-enrollments")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(enrollment_request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(enrolled.status(), StatusCode::OK);
        let enrolled: fasti_contracts::ClientEnrollmentResponse = serde_json::from_slice(
            &to_bytes(enrolled.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("enrollment response");
        assert_eq!(
            enrolled.credential_scheme,
            fasti_contracts::CredentialSchemeDto::Bearer
        );
        kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                CapabilityKey::InspectReview,
                SecretMaterial::try_from_hex(&enrolled.credential).expect("issued credential"),
            ))
            .expect("durable credential authenticates");

        let repeated = app
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", bootstrap_secret.trim()),
                    )
                    .body(Body::from("{}"))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(repeated.status(), StatusCode::CONFLICT);
        let problem: ProblemDetails = serde_json::from_slice(
            &to_bytes(repeated.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("problem response");
        assert_eq!(problem.code, "already_initialized");
        assert_eq!(problem.safe_state, "prior_state_retained");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn observation_requires_bearer_and_replays_one_source_event_exactly_once() {
        let (root, kernel) = test_kernel();
        let app = api_router(
            kernel.clone(),
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );
        let request = serde_json::json!({
            "kind": "consumption_occurrence",
            "source": "nuvio",
            "source_event_id": "session-42:stop:episode-7",
            "observed_at": "2026-08-26T18:10:00Z",
            "occurred_at": "2026-08-26T18:09:58Z",
            "target_grain": "episode",
            "identifiers": [
                {"namespace":"imdb.title","grain":"series","value":"tt1234567"},
                {"namespace":"kitsu.anime","grain":"release","value":"7442"}
            ],
            "title": "Example episode",
            "progress_percent": 100.0,
            "position_seconds": 1440,
            "duration_seconds": 1440
        });

        let unauthorized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/observations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let credential = enroll_admin(&app, root.path()).await.credential;
        let send = |body: serde_json::Value| {
            Request::post("/api/v1/observations")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                .body(Body::from(body.to_string()))
                .expect("valid request")
        };

        let committed = app
            .clone()
            .oneshot(send(request.clone()))
            .await
            .expect("router response");
        assert_eq!(committed.status(), StatusCode::OK);
        let committed: fasti_contracts::SubmitObservationResponse = serde_json::from_slice(
            &to_bytes(committed.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("observation response");
        assert_eq!(committed.disposition, "committed");

        let replayed = app
            .clone()
            .oneshot(send(request.clone()))
            .await
            .expect("router response");
        assert_eq!(replayed.status(), StatusCode::OK);
        let replayed: fasti_contracts::SubmitObservationResponse = serde_json::from_slice(
            &to_bytes(replayed.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("replayed response");
        assert_eq!(replayed.disposition, "replayed");
        assert_eq!(replayed.receipt_id, committed.receipt_id);
        assert_eq!(replayed.observation_id, committed.observation_id);

        let mut changed = request;
        changed["title"] = serde_json::json!("Changed evidence for the same source event");
        let conflict = app.oneshot(send(changed)).await.expect("router response");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        let problem: ProblemDetails = serde_json::from_slice(
            &to_bytes(conflict.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("problem response");
        assert_eq!(problem.code, "idempotency_conflict");
        assert_eq!(problem.safe_state, "prior_state_retained");
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn partial_progress_is_rejected_without_creating_false_history() {
        let (root, kernel) = test_kernel();
        let app = api_router(
            kernel,
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );
        let credential = enroll_admin(&app, root.path()).await.credential;
        let request = serde_json::json!({
            "kind": "consumption_occurrence",
            "source": "nuvio",
            "source_event_id": "session-42:progress:episode-7",
            "observed_at": "2026-08-26T18:10:00Z",
            "target_grain": "episode",
            "identifiers": [],
            "progress_percent": 72.5,
            "position_seconds": 1044,
            "duration_seconds": 1440
        });
        let response = app
            .oneshot(
                Request::post("/api/v1/observations")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::from(request.to_string()))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let problem: ProblemDetails = serde_json::from_slice(
            &to_bytes(response.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("problem response");
        assert_eq!(problem.code, "invalid_observation");
        assert_eq!(problem.safe_state, "no_mutation");
    }

    #[tokio::test]
    async fn remote_health_router_exposes_no_local_capability_route() {
        let response = health_router()
            .oneshot(
                Request::post("/api/v1/observations")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn integration_router_exposes_adapters_but_not_bootstrap_or_generic_mutation() {
        let (_root, kernel) = test_kernel();
        let app = integration_router(kernel);

        let status = app
            .clone()
            .oneshot(
                Request::get("/api/v1/integrations")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(status.status(), StatusCode::OK);

        for path in [
            "/api/v1/node/initialization",
            "/api/v1/records",
            "/api/v1/observations",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::post(path)
                        .body(Body::empty())
                        .expect("valid request"),
                )
                .await
                .expect("router response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn event_submission_alias_is_absent() {
        let (root, kernel) = test_kernel();
        let response = api_router(
            kernel,
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        )
        .oneshot(
            Request::post("/api/v1/events")
                .body(Body::empty())
                .expect("valid request"),
        )
        .await
        .expect("router response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn other_b1_fixture_routes_remain_absent_from_production() {
        let (root, kernel) = test_kernel();
        for (method, path) in [
            ("GET", "/api/v1/capabilities"),
            ("GET", "/api/v1/receipts/stream"),
            ("GET", "/api/v1/receipts/rcp_not-a-real-id"),
            ("PUT", "/api/v1/profile-selection"),
            ("POST", "/api/v1/credential-rotations"),
            ("POST", "/api/v1/credential-revocations"),
            ("PUT", "/api/v1/listener-configuration"),
        ] {
            let response = api_router(
                kernel.clone(),
                test_bind_addr(),
                root.path(),
                fasti_application::MAX_SESSION_MINUTES,
            )
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
        }
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn records_require_bearer_and_support_create_list_attach_and_namespace_registration() {
        let (root, kernel) = test_kernel();
        let app = api_router(
            kernel.clone(),
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );

        let unauthorized = app
            .clone()
            .oneshot(
                Request::get("/api/v1/records")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let credential = enroll_admin(&app, root.path()).await.credential;
        let auth = |builder: axum::http::request::Builder| {
            builder.header(header::AUTHORIZATION, format!("Bearer {credential}"))
        };

        let empty_list = app
            .clone()
            .oneshot(
                auth(Request::get("/api/v1/records"))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(empty_list.status(), StatusCode::OK);
        let empty_list: fasti_contracts::ListRecordsResponse = serde_json::from_slice(
            &to_bytes(empty_list.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("list response");
        assert!(empty_list.records.is_empty());

        let created = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/records"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"work"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(created.status(), StatusCode::OK);
        let created: fasti_contracts::CreateRecordResponse = serde_json::from_slice(
            &to_bytes(created.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("create-record response");
        assert_eq!(created.grain, "work");

        let namespace = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/namespaces"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "namespace": "google-books",
                            "label": "Google Books",
                            "grains": ["work"],
                            "id_pattern": ".+",
                            "normalization": "identity",
                            "licence_posture": "identifiers_only",
                        })
                        .to_string(),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(namespace.status(), StatusCode::OK);
        let namespace: fasti_contracts::RegisterNamespaceResponse = serde_json::from_slice(
            &to_bytes(namespace.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("register-namespace response");
        assert!(namespace.created);

        let attached = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/records/identifiers"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "record_id": created.record_id,
                            "namespace": "google-books",
                            "grain": "work",
                            "value": "abc123",
                        })
                        .to_string(),
                    ))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(attached.status(), StatusCode::OK);
        let attached: fasti_contracts::AttachIdentifierResponse = serde_json::from_slice(
            &to_bytes(attached.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("attach-identifier response");
        assert!(attached.created);

        let populated_list = app
            .clone()
            .oneshot(
                auth(Request::get("/api/v1/records"))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(populated_list.status(), StatusCode::OK);
        let populated_list: fasti_contracts::ListRecordsResponse = serde_json::from_slice(
            &to_bytes(populated_list.into_body(), 16 * 1024)
                .await
                .expect("bounded body"),
        )
        .expect("list response");
        assert_eq!(populated_list.records.len(), 1);
        assert_eq!(populated_list.records[0].record_id, created.record_id);
        assert_eq!(populated_list.records[0].identifiers.len(), 1);
        assert_eq!(populated_list.records[0].identifiers[0].value, "abc123");
        assert_eq!(
            populated_list.records[0]
                .overview
                .as_ref()
                .and_then(|field| field.value.as_deref()),
            None,
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn profile_tracking_disposition_is_authenticated_set_list_and_unset() {
        let (root, kernel) = test_kernel();
        let app = api_router(
            kernel,
            test_bind_addr(),
            root.path(),
            fasti_application::MAX_SESSION_MINUTES,
        );
        let unauthorized = app
            .clone()
            .oneshot(
                Request::get("/api/v1/profile/record-tracking-dispositions")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        let credential = enroll_admin(&app, root.path()).await.credential;
        let auth = |builder: axum::http::request::Builder| {
            builder.header(header::AUTHORIZATION, format!("Bearer {credential}"))
        };

        let created = app
            .clone()
            .oneshot(
                auth(Request::post("/api/v1/records"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"grain":"work"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(created.status(), StatusCode::OK);
        let created: fasti_contracts::CreateRecordResponse = serde_json::from_slice(
            &to_bytes(created.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("create-record response");
        let state_path = format!(
            "/api/v1/profile/record-tracking-dispositions/{}",
            created.record_id
        );

        let set = app
            .clone()
            .oneshot(
                auth(Request::put(&state_path))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"disposition":"watching"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(set.status(), StatusCode::OK);
        let set: fasti_contracts::TrackingDispositionStateDto =
            serde_json::from_slice(&to_bytes(set.into_body(), 4096).await.expect("bounded body"))
                .expect("set tracking response");
        assert_eq!(set.record_id, created.record_id);
        assert_eq!(
            set.disposition,
            Some(fasti_contracts::TrackingDispositionDto::Watching)
        );

        let listed = app
            .clone()
            .oneshot(
                auth(Request::get("/api/v1/profile/record-tracking-dispositions"))
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(listed.status(), StatusCode::OK);
        let listed: fasti_contracts::ListTrackingDispositionsResponse = serde_json::from_slice(
            &to_bytes(listed.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("list tracking response");
        assert_eq!(listed.states, vec![set]);

        let unset = app
            .clone()
            .oneshot(
                auth(Request::put(&state_path))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"disposition":"unset"}"#))
                    .expect("valid request"),
            )
            .await
            .expect("router response");
        assert_eq!(unset.status(), StatusCode::OK);
        let unset: fasti_contracts::TrackingDispositionStateDto = serde_json::from_slice(
            &to_bytes(unset.into_body(), 4096)
                .await
                .expect("bounded body"),
        )
        .expect("unset tracking response");
        assert_eq!(unset.record_id, created.record_id);
        assert_eq!(unset.disposition, None);
    }
}
