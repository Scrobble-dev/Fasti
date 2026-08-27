use crate::{
    local::{authenticate_request, request_authentication, run_kernel, LocalApiState},
    problem::{application_problem, json_rejection, HttpProblem},
};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use fasti_application::{
    derive_deterministic_operation_id, AcceptObservationCommand, CapabilityKey,
    EvidenceUploadRequest, FastiProblem, LocalKernel, Violation,
};
use fasti_contracts::{ProblemDetails, SubmitObservationRequest, SubmitObservationResponse};
use fasti_domain::{
    ClaimedTrust, ExternalIdentifierClaim, Grain, ObservationResolution, ObservedAt, OccurredAt,
    RequestCorrelationId,
};
use std::sync::Arc;

const MAX_NORMALIZED_EVIDENCE_BYTES: usize = 64 * 1024;

type HttpResult<T> = Result<Json<T>, HttpProblem>;

fn invalid_observation(
    correlation_id: RequestCorrelationId,
    pointer: &str,
    reason: &str,
    expected: &str,
) -> HttpProblem {
    let violation = Violation::try_new("invalid_observation", pointer, reason, expected)
        .expect("adapter-owned observation violation is valid");
    let problem = FastiProblem::invalid_observation(correlation_id, vec![violation])
        .expect("one observation violation is within bounds");
    application_problem(Box::new(problem))
}

fn validate_request(
    request: &SubmitObservationRequest,
    correlation_id: RequestCorrelationId,
) -> Result<(), HttpProblem> {
    if request.source.is_empty() || request.source.len() > 64 {
        return Err(invalid_observation(
            correlation_id,
            "/source",
            "source must contain between 1 and 64 bytes",
            "a stable source identifier",
        ));
    }
    if request.source_event_id.is_empty() || request.source_event_id.len() > 256 {
        return Err(invalid_observation(
            correlation_id,
            "/source_event_id",
            "source event identity must contain between 1 and 256 bytes",
            "a stable event identity reused for retries",
        ));
    }
    if request.identifiers.len() > 16 {
        return Err(invalid_observation(
            correlation_id,
            "/identifiers",
            "too many external identifier clues were supplied",
            "at most 16 identifier clues",
        ));
    }
    if request
        .title
        .as_ref()
        .is_some_and(|title| title.len() > 512)
    {
        return Err(invalid_observation(
            correlation_id,
            "/title",
            "title exceeds the bounded evidence field",
            "at most 512 bytes",
        ));
    }
    if let Some(progress) = request.progress_percent {
        if !progress.is_finite() || !(0.0..=100.0).contains(&progress) {
            return Err(invalid_observation(
                correlation_id,
                "/progress_percent",
                "progress is outside the valid percentage range",
                "a finite value from 0 through 100",
            ));
        }
        if progress < 100.0 {
            return Err(invalid_observation(
                correlation_id,
                "/progress_percent",
                "partial progress is not a Chronicle occurrence",
                "100 for a completed occurrence; use the progress capability when it becomes available",
            ));
        }
    }
    if request.duration_seconds == Some(0) {
        return Err(invalid_observation(
            correlation_id,
            "/duration_seconds",
            "duration must be greater than zero when supplied",
            "a positive duration in seconds",
        ));
    }
    if let (Some(position), Some(duration)) = (request.position_seconds, request.duration_seconds) {
        if position > duration {
            return Err(invalid_observation(
                correlation_id,
                "/position_seconds",
                "position cannot exceed the supplied duration",
                "a position less than or equal to duration_seconds",
            ));
        }
    }
    Ok(())
}

fn resolution_name(value: ObservationResolution) -> &'static str {
    match value {
        ObservationResolution::Unresolved => "unresolved",
        ObservationResolution::Resolved => "resolved",
        ObservationResolution::Conflicted => "conflicted",
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/observations",
    tag = "observations",
    security(("bearer_credential" = []), ("browser_session" = [])),
    request_body = SubmitObservationRequest,
    responses(
        (status = 200, description = "Durable observation receipt; a safe retry can return a replayed disposition", body = SubmitObservationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Credential or browser session is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Authenticated principal lacks observation acceptance scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Source event identity was reused with different evidence", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request or evidence exceeds a bounded limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Observation does not satisfy the governed contract", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded evidence or observation capacity is exhausted", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn submit_observation(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    request: Result<Json<SubmitObservationRequest>, JsonRejection>,
) -> HttpResult<SubmitObservationResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) = request.map_err(|rejection| {
        json_rejection(CapabilityKey::AcceptObservation, correlation_id, rejection)
    })?;
    let authentication = request_authentication(
        &headers,
        CapabilityKey::AcceptObservation,
        correlation_id,
        true,
    )?;
    validate_request(&request, correlation_id)?;

    let observed_at = ObservedAt::parse(&request.observed_at, ClaimedTrust::DeviceObserved)
        .map_err(|_| {
            invalid_observation(
                correlation_id,
                "/observed_at",
                "observed_at is not an RFC 3339 instant with an explicit offset",
                "an RFC 3339 timestamp such as 2026-08-26T18:10:00Z",
            )
        })?;
    let occurred_at = request
        .occurred_at
        .as_deref()
        .map(|value| OccurredAt::parse(value, ClaimedTrust::SourceClaim))
        .transpose()
        .map_err(|_| {
            invalid_observation(
                correlation_id,
                "/occurred_at",
                "occurred_at is not an ISO date or RFC 3339 timestamp with an explicit offset",
                "an ISO date or RFC 3339 timestamp",
            )
        })?;
    let target_grain = request
        .target_grain
        .as_deref()
        .map(str::parse::<Grain>)
        .transpose()
        .map_err(|_| {
            invalid_observation(
                correlation_id,
                "/target_grain",
                "target grain is not registered",
                "one registered Fasti grain",
            )
        })?;

    let mut clues = Vec::with_capacity(request.identifiers.len());
    for (index, identifier) in request.identifiers.iter().enumerate() {
        let grain = identifier.grain.parse::<Grain>().map_err(|_| {
            invalid_observation(
                correlation_id,
                &format!("/identifiers/{index}/grain"),
                "identifier grain is not registered",
                "one registered Fasti grain",
            )
        })?;
        let clue =
            ExternalIdentifierClaim::try_new(&identifier.namespace, grain, &identifier.value)
                .map_err(|_| {
                    invalid_observation(
                        correlation_id,
                        &format!("/identifiers/{index}"),
                        "external identifier does not satisfy the domain contract",
                        "a registered namespace, grain, and bounded identifier value",
                    )
                })?;
        clues.push(clue);
    }

    let evidence_bytes = serde_json::to_vec(&request).map_err(|_| {
        application_problem(Box::new(FastiProblem::integrity_failed(
            CapabilityKey::AcceptObservation,
            correlation_id,
        )))
    })?;
    if evidence_bytes.len() > MAX_NORMALIZED_EVIDENCE_BYTES {
        return Err(application_problem(Box::new(
            FastiProblem::payload_too_large(
                CapabilityKey::AcceptObservation,
                correlation_id,
                vec![Violation::try_new(
                    "invalid_representation",
                    "/",
                    "normalized observation evidence exceeds the ingress bound",
                    "at most 65536 bytes",
                )
                .expect("adapter-owned representation violation is valid")],
            )
            .expect("one violation is within bounds"),
        )));
    }

    let source = request.source;
    let source_event_id = request.source_event_id;
    let kernel = state.kernel;
    let outcome = run_kernel(
        CapabilityKey::AcceptObservation,
        correlation_id,
        move || {
            let access = authenticate_request(
                kernel.as_ref(),
                authentication,
                CapabilityKey::AcceptObservation,
                correlation_id,
                true,
            )?;
            let operation_material = serde_json::to_string(&(
                "observation",
                access.client_id().to_string(),
                &source,
                &source_event_id,
            ))
            .map_err(|_| {
                Box::new(FastiProblem::integrity_failed(
                    CapabilityKey::AcceptObservation,
                    correlation_id,
                ))
            })?;
            let operation_id = derive_deterministic_operation_id(&operation_material);
            let mut upload = kernel.begin_evidence_upload(EvidenceUploadRequest::new(
                correlation_id,
                access,
                Some(evidence_bytes.len() as u64),
            ))?;
            upload.write_chunk(&evidence_bytes)?;
            let evidence = upload.finish()?;
            let command = AcceptObservationCommand::new(
                correlation_id,
                access,
                operation_id,
                occurred_at,
                observed_at,
                evidence,
            )
            .with_identity_clues(clues, target_grain);
            kernel.authorize_and_accept(command)
        },
    )
    .await?;

    let receipt = outcome.receipt();
    Ok(Json(SubmitObservationResponse {
        disposition: if outcome.is_replay() {
            "replayed".to_owned()
        } else {
            "committed".to_owned()
        },
        receipt_id: receipt.receipt_id().to_string(),
        operation_id: receipt.operation_id().to_string(),
        workspace_id: receipt.workspace_id().to_string(),
        profile_id: receipt.profile_id().to_string(),
        source_client_id: receipt.source_client_id().to_string(),
        observation_id: receipt.observation_id().to_string(),
        occurrence_id: receipt.occurrence_id().map(|value| value.to_string()),
        interpretation_id: receipt.interpretation_id().map(|value| value.to_string()),
        record_id: receipt.record_id().map(|value| value.to_string()),
        review_item_id: receipt.review_item_id().map(|value| value.to_string()),
        evidence_id: receipt.evidence_id().to_string(),
        payload_digest: receipt.payload_digest().to_string(),
        resolution: resolution_name(receipt.resolution()).to_owned(),
        received_at: receipt.received_at().value().to_rfc3339(),
        committed_at: receipt.committed_at().value().to_rfc3339(),
    }))
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new().route("/api/v1/observations", post(submit_observation))
}

#[allow(dead_code)]
fn _assert_kernel_is_object_safe(_: Arc<dyn LocalKernel>) {}
