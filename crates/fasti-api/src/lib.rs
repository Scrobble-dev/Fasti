//! Fasti HTTP REST API definitions and router construction.

use axum::{routing::get, Json, Router};
use fasti_application::LocalKernel;
use fasti_contracts::{HealthResponse, ProblemActionDto, ProblemDetails, ViolationDto};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use utoipa::OpenApi;

mod integrations;
mod local;
mod observation;
mod problem;
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

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        local::initialize_node,
        local::enroll_first_client,
        observation::submit_observation,
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
        fasti_contracts::ClientEnrollmentResponse,
        fasti_contracts::CreateRecordRequest,
        fasti_contracts::CreateRecordResponse,
        fasti_contracts::CredentialSchemeDto,
        fasti_contracts::EnrollFirstClientRequest,
        fasti_contracts::InitializeNodeRequest,
        fasti_contracts::IntegrationObservationRequest,
        fasti_contracts::IntegrationStatusDto,
        fasti_contracts::IntegrationStatusListResponse,
        fasti_contracts::ListRecordsResponse,
        fasti_contracts::NodeInitializationResponse,
        fasti_contracts::ObservationIdentifierInput,
        fasti_contracts::ObservationIngressKind,
        fasti_contracts::RecordActivityDto,
        fasti_contracts::RecordSummaryDto,
        fasti_contracts::RegisterNamespaceRequest,
        fasti_contracts::RegisterNamespaceResponse,
        fasti_contracts::ResolvedFieldDto,
        fasti_contracts::SubmitObservationRequest,
        fasti_contracts::SubmitObservationResponse,
        ProblemActionDto,
        ProblemDetails,
        ViolationDto
    ))
)]
struct ApiDoc;

/// Builds the OpenAPI 3.1 contract for routes actually mounted by [`api_router`]
/// and the dedicated integration listener.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Constructs the health-only router used by every non-loopback general
/// listener. Integration ingress uses [`integration_router`] instead of widening
/// this surface.
pub fn health_router() -> Router {
    Router::new().route("/api/v1/health", get(health_check))
}

/// Constructs the dedicated integration ingress surface.
///
/// It intentionally exposes only health, integration status, and authenticated
/// provider adapters. Node bootstrap, generic record mutation, and the generic
/// observation endpoint are never mounted here.
pub fn integration_router(kernel: Arc<dyn LocalKernel>) -> Router {
    let state = local::LocalApiState { kernel };
    health_router().merge(integrations::router().with_state(state))
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
/// an explicit data root. Non-loopback general listeners or missing data roots
/// must use [`health_router`] instead.
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
        !data_root.as_os_str().is_empty(),
        "api_router requires non-empty data_root"
    );
    let integration_state = local::LocalApiState {
        kernel: Arc::clone(&kernel),
    };
    health_router()
        .merge(local::router(kernel))
        .merge(integrations::router().with_state(integration_state))
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
    fn openapi_is_3_1_and_documents_the_real_routes() {
        let document = openapi();

        assert!(matches!(document.openapi, OpenApiVersion::Version31));
        for path in [
            "/api/v1/health",
            "/api/v1/node/initialization",
            "/api/v1/client-enrollments",
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
        ] {
            assert!(document.paths.paths.contains_key(path), "missing {path}");
        }
        assert_eq!(document.paths.paths.len(), 13);

        let serialized = serde_json::to_string(&document).expect("serializable OpenAPI document");
        assert!(serialized.contains("#/components/schemas/HealthResponse"));

        let schemas = &document.components.expect("OpenAPI components").schemas;
        for schema in [
            "HealthResponse",
            "NodeInitializationResponse",
            "ClientEnrollmentResponse",
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
            "RecordActivityDto",
            "RecordSummaryDto",
            "RegisterNamespaceRequest",
            "RegisterNamespaceResponse",
            "ResolvedFieldDto",
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
    async fn enroll_admin(app: &Router) -> fasti_contracts::ClientEnrollmentResponse {
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

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn observation_requires_bearer_and_replays_one_source_event_exactly_once() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());
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

        let credential = enroll_admin(&app).await.credential;
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
        let app = api_router(kernel, test_bind_addr(), root.path());
        let credential = enroll_admin(&app).await.credential;
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

        for path in ["/api/v1/node/initialization", "/api/v1/records", "/api/v1/observations"] {
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

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn records_require_bearer_and_support_create_list_attach_and_namespace_registration() {
        let (root, kernel) = test_kernel();
        let app = api_router(kernel, test_bind_addr(), root.path());

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

        let credential = enroll_admin(&app).await.credential;
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
    }
}
