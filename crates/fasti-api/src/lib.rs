//! Fasti HTTP REST API definitions and router construction.

use axum::{
    routing::{get, post},
    Json, Router,
};
use fasti_activity::{ActivityEvent, EventReceipt, ReceiptStatus};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub async fn submit_event(Json(event): Json<ActivityEvent>) -> Json<EventReceipt> {
    Json(EventReceipt {
        event_id: event.event_id,
        received_at: chrono::Utc::now(),
        status: ReceiptStatus::Committed,
    })
}

/// Constructs the primary API router for fastid.
pub fn api_router() -> Router {
    Router::new()
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/events", post(submit_event))
}
