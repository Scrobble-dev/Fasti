//! Fasti HTTP REST API definitions and router construction.

use axum::{routing::get, Json, Router};
use fasti_application::LocalKernel;
use fasti_contracts::{HealthResponse, ProblemActionDto, ProblemDetails, ViolationDto};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use utoipa::OpenApi;

mod local;
mod problem;

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

#[derive(OpenApi)]
#[openapi(
    paths(health_check, local::initialize_node, local::enroll_first_client),
    components(schemas(
        HealthResponse,
        fasti_contracts::ClientEnrollmentResponse,
        fasti_contracts::CredentialSchemeDto,
        fasti_contracts::EnrollFirstClientRequest,
        fasti_contracts::InitializeNodeRequest,
        fasti_contracts::NodeInitializationResponse,
        ProblemActionDto,
        ProblemDetails,
        ViolationDto
    ))
)]
struct ApiDoc;

/// Builds the OpenAPI 3.1 contract for routes actually mounted by [`api_router`].
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Constructs the health-only router used by every non-loopback listener.
///
/// This router is intentionally separate from the local application router so a
/// future local capability cannot become remotely reachable through container or
/// operator listener configuration by accident.
pub fn health_router() -> Router {
    Router::new().route("/api/v1/health", get(health_check))
}

/// Constructs the durable loopback API router for fastid.
///
/// # Contract
///
/// This function merges health and durable local routes and validates that:
/// - `bind_addr` is a loopback address (panics if not)
/// - `data_root` is non-empty (panics if empty)
///
/// These validations enforce the durable route security model: local capability
/// routes must never be exposed on non-loopback listeners, and must always have
/// an explicit data root. Non-loopback listeners or missing data roots must use
/// [`health_router`] instead.
///
/// # Panics
///
/// Panics if `bind_addr` is not a loopback address or if `data_root` is empty.
pub fn api_router(kernel: Arc<dyn LocalKernel>, bind_addr: SocketAddr, data_root: &Path) -> Router {
    assert!(
        bind_addr.ip().is_loopback(),
        "api_router requires loopback bind address, got non-loopback {bind_addr}"
    );
    assert!(
        data_root.as_os_str().len() > 0,
        "api_router requires non-empty data_root"
    );
    health_router().merge(local::router(kernel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{header, Request, StatusCode},
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

    #[test]
    fn openapi_is_3_1_and_documents_the_real_health_route() {
        let document = openapi();

        assert!(matches!(document.openapi, OpenApiVersion::Version31));
        assert!(document.paths.paths.contains_key("/api/v1/health"));
        assert!(document
            .paths
            .paths
            .contains_key("/api/v1/node/initialization"));
        assert!(document
            .paths
            .paths
            .contains_key("/api/v1/client-enrollments"));
        assert_eq!(document.paths.paths.len(), 3);

        let serialized = serde_json::to_string(&document).expect("serializable OpenAPI document");
        assert!(serialized.contains("#/components/schemas/HealthResponse"));

        let schemas = &document.components.expect("OpenAPI components").schemas;
        for schema in [
            "HealthResponse",
            "NodeInitializationResponse",
            "ClientEnrollmentResponse",
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
        let response = api_router(kernel, test_bind_addr(), root.path())
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
    async fn durable_bootstrap_issues_one_credential_and_closes_initialization() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel.clone(), test_bind_addr(), root.path());

        let initialized = app
            .clone()
            .oneshot(
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
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

    #[tokio::test]
    async fn remote_health_router_exposes_no_local_capability_route() {
        let response = health_router()
            .oneshot(
                Request::get("/api/v1/capabilities")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn event_submission_is_absent_until_it_can_persist() {
        let (root, kernel) = test_kernel();
        let response = api_router(kernel, test_bind_addr(), root.path())
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
    async fn all_b1_fixture_routes_are_absent_from_production() {
        let (root, kernel) = test_kernel();
        for (method, path) in [
            ("GET", "/api/v1/capabilities"),
            ("POST", "/api/v1/observations"),
            ("GET", "/api/v1/receipts/stream"),
            ("GET", "/api/v1/receipts/rcp_not-a-real-id"),
            ("PUT", "/api/v1/profile-selection"),
            ("POST", "/api/v1/credential-rotations"),
            ("POST", "/api/v1/credential-revocations"),
            ("PUT", "/api/v1/listener-configuration"),
        ] {
            let response = api_router(kernel.clone(), test_bind_addr(), root.path())
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
}
