#![cfg(target_os = "linux")]

use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
    Router,
};
use fasti_api::{api_router, integration_router};
use fasti_contracts::{
    ClientEnrollmentResponse, NodeInitializationResponse, SubmitObservationResponse,
};
use fasti_store::SqliteKernel;
use std::path::Path;
use std::sync::Arc;
use tower::ServiceExt;

async fn routers() -> (tempfile::TempDir, Router, Router) {
    let root = tempfile::tempdir().expect("temporary data root");
    let kernel = Arc::new(SqliteKernel::open(root.path()).expect("SQLite kernel"));
    let local = api_router(
        kernel.clone(),
        "127.0.0.1:8420".parse().expect("loopback"),
        root.path(),
        fasti_application::MAX_SESSION_MINUTES,
    );
    let integrations = integration_router(kernel);
    (root, local, integrations)
}

async fn enroll_admin(app: &Router, data_root: &Path) -> String {
    let bootstrap_secret = std::fs::read_to_string(data_root.join("bootstrap.secret"))
        .expect("host bootstrap secret must be created before local routes are served");
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
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(initialized.status(), StatusCode::OK);
    let initialized: NodeInitializationResponse = serde_json::from_slice(
        &to_bytes(initialized.into_body(), 4096)
            .await
            .expect("bounded body"),
    )
    .expect("initialization response");

    let enrolled = app
        .clone()
        .oneshot(
            Request::post("/api/v1/client-enrollments")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "initialization_proof": initialized.initialization_proof
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(enrolled.status(), StatusCode::OK);
    let enrolled: ClientEnrollmentResponse = serde_json::from_slice(
        &to_bytes(enrolled.into_body(), 4096)
            .await
            .expect("bounded body"),
    )
    .expect("enrollment response");
    enrolled.credential
}

fn bearer(builder: axum::http::request::Builder, credential: &str) -> axum::http::request::Builder {
    builder.header(header::AUTHORIZATION, format!("Bearer {credential}"))
}

fn template_event(event_id: &str, completed: bool, title: &str) -> serde_json::Value {
    serde_json::json!({
        "source_event_id": event_id,
        "observed_at": "2026-08-27T12:00:00Z",
        "occurred_at": "2026-08-27T11:59:58Z",
        "item_type": "episode",
        "title": title,
        "series_title": "Fixture series",
        "season_number": 1,
        "episode_number": 2,
        "completed": completed,
        "position_seconds": 1440,
        "duration_seconds": 1440,
        "provider_ids": {"imdb": "tt1234567"},
        "series_provider_ids": {"tmdb": "12345"},
        "server_id": "server-fixture",
        "user_id": "user-fixture",
        "device_id": "device-fixture"
    })
}

#[tokio::test]
async fn template_webhook_requires_auth_and_replays_stable_event_identity() {
    let (root, local, integrations) = routers().await;
    let credential = enroll_admin(&local, root.path()).await;
    let body = template_event("fixture-session:complete:1", true, "Episode title");

    let unauthorized = integrations
        .clone()
        .oneshot(
            Request::post("/api/v1/integrations/tautulli/webhook")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let send = |payload: serde_json::Value| {
        bearer(
            Request::post("/api/v1/integrations/tautulli/webhook"),
            &credential,
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .expect("request")
    };

    let committed = integrations
        .clone()
        .oneshot(send(body.clone()))
        .await
        .expect("response");
    assert_eq!(committed.status(), StatusCode::OK);
    let committed: SubmitObservationResponse = serde_json::from_slice(
        &to_bytes(committed.into_body(), 16 * 1024)
            .await
            .expect("bounded body"),
    )
    .expect("commit receipt");
    assert_eq!(committed.disposition, "committed");

    let replayed = integrations
        .clone()
        .oneshot(send(body.clone()))
        .await
        .expect("response");
    assert_eq!(replayed.status(), StatusCode::OK);
    let replayed: SubmitObservationResponse = serde_json::from_slice(
        &to_bytes(replayed.into_body(), 16 * 1024)
            .await
            .expect("bounded body"),
    )
    .expect("replay receipt");
    assert_eq!(replayed.disposition, "replayed");
    assert_eq!(replayed.receipt_id, committed.receipt_id);

    let mut changed = body;
    changed["title"] = serde_json::json!("Changed evidence");
    let conflict = integrations.oneshot(send(changed)).await.expect("response");
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn template_webhook_rejects_partial_progress_without_history_mutation() {
    let (root, local, integrations) = routers().await;
    let credential = enroll_admin(&local, root.path()).await;
    let event_id = "fixture-session:progress:1";
    let response = integrations
        .clone()
        .oneshot(
            bearer(
                Request::post("/api/v1/integrations/jellyfin/webhook"),
                &credential,
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                template_event(event_id, false, "Episode title").to_string(),
            ))
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // The rejected partial-progress request must not have persisted a
    // history record under this event identity: a genuinely completed event
    // with the same source_event_id must still commit fresh, not replay a
    // phantom prior receipt.
    let completed = integrations
        .oneshot(
            bearer(
                Request::post("/api/v1/integrations/jellyfin/webhook"),
                &credential,
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                template_event(event_id, true, "Episode title").to_string(),
            ))
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(completed.status(), StatusCode::OK);
    let completed: SubmitObservationResponse = serde_json::from_slice(
        &to_bytes(completed.into_body(), 16 * 1024)
            .await
            .expect("bounded body"),
    )
    .expect("commit receipt");
    assert_eq!(completed.disposition, "committed");
}

#[tokio::test]
async fn emby_native_completion_is_normalized_through_the_shared_boundary() {
    let (root, local, integrations) = routers().await;
    let credential = enroll_admin(&local, root.path()).await;
    let body = serde_json::json!({
        "Event": "playback.stop",
        "PlayedToCompletion": true,
        "UtcTimestamp": "2026-08-27T12:00:00Z",
        "Item": {
            "Id": "emby-item-1",
            "Type": "Movie",
            "Name": "Fixture movie",
            "RunTimeTicks": 72000000000_u64,
            "ProviderIds": {"Imdb": "tt7654321", "Tmdb": "4567"}
        },
        "Session": {"PlayState": {"PositionTicks": 72000000000_u64}}
    });
    let response = integrations
        .oneshot(
            bearer(
                Request::post("/api/v1/integrations/emby/webhook"),
                &credential,
            )
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn plex_multipart_accepts_payload_with_binary_image_without_parsing_the_image() {
    let (root, local, integrations) = routers().await;
    let credential = enroll_admin(&local, root.path()).await;
    let boundary = "fasti-fixture-boundary";
    let payload = serde_json::json!({
        "event": "media.scrobble",
        "Server": {"uuid": "plex-server"},
        "Metadata": {
            "type": "movie",
            "ratingKey": "100",
            "title": "Fixture Plex movie",
            "duration": 7200000_u64,
            "viewOffset": 7200000_u64,
            "lastViewedAt": 1787832000_i64,
            "Guid": [{"id": "imdb://tt2222222"}]
        }
    })
    .to_string();
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"payload\"\r\nContent-Type: application/json\r\n\r\n{payload}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"thumb\"; filename=\"thumb.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&[0xff, 0xd8, 0xff, 0x00, 0xfe, 0x80, 0xff, 0xd9]);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let response = integrations
        .oneshot(
            bearer(
                Request::post("/api/v1/integrations/plex/webhook"),
                &credential,
            )
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
}
