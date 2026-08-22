//! Fasti HTTP REST API definitions and router construction.

use axum::{routing::get, Json, Router};
use fasti_contracts::{HealthResponse, ProblemActionDto, ProblemDetails, ViolationDto};
use utoipa::OpenApi;

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
    paths(health_check),
    components(schemas(HealthResponse, ProblemActionDto, ProblemDetails, ViolationDto))
)]
struct ApiDoc;

/// Builds the OpenAPI 3.1 contract for routes actually mounted by [`api_router`].
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Constructs the primary API router for fastid.
pub fn api_router() -> Router {
    Router::new().route("/api/v1/health", get(health_check))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    use utoipa::openapi::OpenApiVersion;

    #[test]
    fn openapi_is_3_1_and_documents_the_real_health_route() {
        let document = openapi();

        assert!(matches!(document.openapi, OpenApiVersion::Version31));
        assert!(document.paths.paths.contains_key("/api/v1/health"));
        assert_eq!(document.paths.paths.len(), 1);

        let serialized = serde_json::to_string(&document).expect("serializable OpenAPI document");
        assert!(serialized.contains("#/components/schemas/HealthResponse"));

        let schemas = &document.components.expect("OpenAPI components").schemas;
        for schema in [
            "HealthResponse",
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

    #[tokio::test]
    async fn documented_health_route_is_mounted() {
        let response = api_router()
            .oneshot(
                Request::get("/api/v1/health")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn event_submission_is_absent_until_it_can_persist() {
        let response = api_router()
            .oneshot(
                Request::post("/api/v1/events")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("router response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn all_b1_fixture_routes_are_absent_from_production() {
        for (method, path) in [
            ("GET", "/api/v1/capabilities"),
            ("POST", "/api/v1/node/initialization"),
            ("POST", "/api/v1/client-enrollments"),
            ("POST", "/api/v1/observations"),
            ("GET", "/api/v1/receipts/rcp_not-a-real-id"),
        ] {
            let response = api_router()
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
