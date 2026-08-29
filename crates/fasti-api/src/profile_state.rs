use crate::local::{authenticate_request, request_authentication, run_kernel, LocalApiState};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::HeaderMap,
    routing::{get, put},
    Json, Router,
};
use fasti_application::{
    CapabilityKey, FastiProblem, ListTrackingDispositionsQuery, SetTrackingDispositionCommand,
};
use fasti_contracts::{
    ListTrackingDispositionsResponse, ProblemDetails, SetTrackingDispositionRequest,
    TrackingDispositionDto, TrackingDispositionStateDto, TrackingDispositionUpdateDto,
};
use fasti_domain::{RecordId, RequestCorrelationId, TrackingDisposition};

type HttpResult<T> = Result<Json<T>, HttpProblem>;

fn disposition_dto(disposition: TrackingDisposition) -> TrackingDispositionDto {
    match disposition {
        TrackingDisposition::Watching => TrackingDispositionDto::Watching,
        TrackingDisposition::OnHold => TrackingDispositionDto::OnHold,
        TrackingDisposition::Dropped => TrackingDispositionDto::Dropped,
    }
}

fn requested_disposition(disposition: TrackingDispositionUpdateDto) -> Option<TrackingDisposition> {
    match disposition {
        TrackingDispositionUpdateDto::Watching => Some(TrackingDisposition::Watching),
        TrackingDispositionUpdateDto::OnHold => Some(TrackingDisposition::OnHold),
        TrackingDispositionUpdateDto::Dropped => Some(TrackingDisposition::Dropped),
        TrackingDispositionUpdateDto::Unset => None,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/profile/record-tracking-dispositions",
    tag = "profile",
    security(("credential_bearer" = [])),
    responses(
        (status = 200, description = "The authenticated profile's explicit record tracking dispositions", body = ListTrackingDispositionsResponse),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn list_tracking_dispositions(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> HttpResult<ListTrackingDispositionsResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ListTrackingDispositions;
    let authentication = request_authentication(&headers, capability, correlation_id)?;
    let kernel = state.kernel;
    let page = run_kernel(capability, correlation_id, move || {
        let access = authenticate_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
        )?;
        kernel
            .list_tracking_dispositions(ListTrackingDispositionsQuery::new(correlation_id, access))
    })
    .await?;

    let truncated = page.truncated();
    Ok(Json(ListTrackingDispositionsResponse {
        states: page
            .into_states()
            .into_iter()
            .map(|state| TrackingDispositionStateDto {
                record_id: state.record_id().to_string(),
                disposition: Some(disposition_dto(state.disposition())),
            })
            .collect(),
        truncated,
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/profile/record-tracking-dispositions/{record_id}",
    tag = "profile",
    security(("credential_bearer" = [])),
    params(("record_id" = String, Path, description = "Fasti Record identifier")),
    request_body = SetTrackingDispositionRequest,
    responses(
        (status = 200, description = "The authenticated profile's resulting explicit tracking disposition", body = TrackingDispositionStateDto),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks profile-state write scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "Record does not exist", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Record ID or disposition does not satisfy the domain contract", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Capability is unavailable in this runtime", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn set_tracking_disposition(
    State(state): State<LocalApiState>,
    Path(record_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<SetTrackingDispositionRequest>, JsonRejection>,
) -> HttpResult<TrackingDispositionStateDto> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::SetTrackingDisposition;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let authentication = request_authentication(&headers, capability, correlation_id)?;
    let record_id = record_id.parse::<RecordId>().map_err(|_| {
        application_problem(Box::new(FastiProblem::record_not_found(
            capability,
            correlation_id,
        )))
    })?;
    let requested = requested_disposition(request.disposition);
    let kernel = state.kernel;
    let state = run_kernel(capability, correlation_id, move || {
        let access = authenticate_request(
            kernel.as_ref(),
            authentication,
            capability,
            correlation_id,
        )?;
        kernel.set_tracking_disposition(SetTrackingDispositionCommand::new(
            correlation_id,
            access,
            record_id,
            requested,
        ))
    })
    .await?;

    Ok(Json(TrackingDispositionStateDto {
        record_id: record_id.to_string(),
        disposition: state.map(|value| disposition_dto(value.disposition())),
    }))
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new()
        .route(
            "/api/v1/profile/record-tracking-dispositions",
            get(list_tracking_dispositions),
        )
        .route(
            "/api/v1/profile/record-tracking-dispositions/{record_id}",
            put(set_tracking_disposition),
        )
}
