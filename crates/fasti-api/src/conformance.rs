//! Feature-gated HTTP adapter for the ephemeral B1 conformance fixture.
//!
//! This module is absent from default builds. Its router is separate from the
//! production router, every success says `fixture_only` and `durability: none`,
//! and no route is mounted by `api_router()`.

use axum::{
    extract::{rejection::JsonRejection, DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{sse::Event, sse::Sse, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use fasti_application::{
    conformance::B1ConformanceFixture, conformance::FixtureEnrollment,
    conformance::FixtureInitialization, AcceptObservationCommand, AcceptObservationReceipt,
    CapabilityKey, FastiProblem, ProblemCode, ReplayReceiptQuery, StreamReceiptsQuery, Violation,
};
use fasti_contracts::{
    public_capability_id, AcceptObservationRequest, AcceptObservationResponse,
    CapabilityDiscoveryResponse, ClaimedPrecisionDto, ClaimedTrustDto, ConformanceMarkerDto,
    CredentialSchemeDto, EnrollFirstClientRequest, EnrollFirstClientResponse,
    GeneratedCapabilitiesDto, InitializeNodeRequest, InitializeNodeResponse, ObservationReceiptDto,
    ObservationResolutionDto, ObservedTimeDto, OccurredTimeDto, ProblemDetails,
    ReceiptDispositionDto, ReplayReceiptResponse,
};
use fasti_domain::{
    ClaimedPrecision, ClaimedTrust, EvidenceId, EvidenceReference, ObservedAt, OccurredAt,
    OperationId, ReceiptId, RequestCorrelationId, Sha256Digest,
};
use std::convert::Infallible;
use std::sync::{Arc, Mutex, OnceLock};
use utoipa::{
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};

const DOCUMENTATION_BASE: &str = "https://fasti.scrobble.dev";
const GENERATED_CAPABILITIES: &str =
    include_str!("../../../contracts/generated/v1/capabilities.json");
const BEARER_PREFIX: &str = "Bearer ";
const LAST_EVENT_ID: &str = "last-event-id";
const MAX_JSON_BODY_BYTES: usize = 4 * 1024;

type HttpResult<T> = Result<Json<T>, HttpProblem>;

struct HttpProblem {
    status: StatusCode,
    body: Box<ProblemDetails>,
}

impl IntoResponse for HttpProblem {
    fn into_response(self) -> Response {
        (
            self.status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(self.body),
        )
            .into_response()
    }
}

#[derive(Default)]
struct TransportState {
    initialization: Option<FixtureInitialization>,
    initialization_proof: Option<InitializationProof>,
    enrollment: Option<FixtureEnrollment>,
}

struct InitializationProof([u8; 32]);

impl InitializationProof {
    fn fresh() -> Self {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes).expect("the operating system CSPRNG must be available");
        Self(bytes)
    }

    fn expose_for_fixture(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for InitializationProof {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

struct B1HttpState {
    fixture: B1ConformanceFixture,
    transport: Mutex<TransportState>,
}

impl Default for B1HttpState {
    fn default() -> Self {
        Self {
            fixture: B1ConformanceFixture::new(),
            transport: Mutex::new(TransportState::default()),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/capabilities",
    tag = "b1-conformance-fixture",
    security(("fixture_bearer" = [])),
    description = "Ephemeral conformance endpoint. Runtime availability is fixture_only and durability is none.",
    responses(
        (status = 200, description = "Registry-owned capability descriptors", body = CapabilityDiscoveryResponse),
        (status = 403, description = "Credential or capability grant denied", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn discover_capabilities(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
) -> HttpResult<CapabilityDiscoveryResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let transport = state
        .transport
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let enrollment = authenticate_enrollment(
        &headers,
        &transport,
        CapabilityKey::DiscoverCapabilities,
        correlation_id,
    )?;
    state
        .fixture
        .authorize_capability_discovery(enrollment.credential_secret(), correlation_id)
        .map_err(application_problem)?;

    static CAPABILITIES: OnceLock<GeneratedCapabilitiesDto> = OnceLock::new();
    let capabilities = CAPABILITIES.get_or_init(|| {
        serde_json::from_str(GENERATED_CAPABILITIES)
            .expect("generated capability registry must match its public DTO")
    });
    Ok(Json(CapabilityDiscoveryResponse {
        conformance: ConformanceMarkerDto::FIXTURE_ONLY,
        contract_version: capabilities.contract_version.clone(),
        capability_base_uri: capabilities.capability_base_uri.clone(),
        surface_profiles: capabilities.surface_profiles.clone(),
        capabilities: capabilities.capabilities.clone(),
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/profile-selection",
    tag = "b1-conformance-fixture",
    security(("fixture_bearer" = [])),
    description = "Problem-only finalized binding. B2 owns successful profile selection semantics.",
    responses(
        (status = 403, description = "Credential or capability grant denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Successful profile selection is unavailable until B2", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn select_profile_unavailable(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    unavailable_admin(state, headers, CapabilityKey::SelectProfile).await
}

#[utoipa::path(
    post,
    path = "/api/v1/credential-rotations",
    tag = "b1-conformance-fixture",
    security(("fixture_bearer" = [])),
    description = "Problem-only finalized binding. B2 owns successful credential rotation semantics.",
    responses(
        (status = 403, description = "Credential or capability grant denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Successful credential rotation is unavailable until B2", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn rotate_credential_unavailable(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    unavailable_admin(state, headers, CapabilityKey::RotateCredential).await
}

#[utoipa::path(
    post,
    path = "/api/v1/credential-revocations",
    tag = "b1-conformance-fixture",
    security(("fixture_bearer" = [])),
    description = "Problem-only finalized binding. B2 owns successful credential revocation semantics.",
    responses(
        (status = 403, description = "Credential or capability grant denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Successful credential revocation is unavailable until B2", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn revoke_credential_unavailable(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    unavailable_admin(state, headers, CapabilityKey::RevokeCredential).await
}

#[utoipa::path(
    put,
    path = "/api/v1/listener-configuration",
    tag = "b1-conformance-fixture",
    security(("fixture_bearer" = [])),
    description = "Problem-only finalized binding. B2 owns successful listener configuration semantics.",
    responses(
        (status = 403, description = "Credential or capability grant denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "Successful listener configuration is unavailable until B2", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn configure_listener_unavailable(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    unavailable_admin(state, headers, CapabilityKey::ConfigureListener).await
}

async fn unavailable_admin(
    state: Arc<B1HttpState>,
    headers: HeaderMap,
    capability: CapabilityKey,
) -> Result<Response, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let transport = state
        .transport
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let enrollment = authenticate_enrollment(&headers, &transport, capability, correlation_id)?;
    state
        .fixture
        .reject_unavailable_admin_capability(
            enrollment.credential_secret(),
            capability,
            correlation_id,
        )
        .map_err(application_problem)?;
    unreachable!("unavailable admin capabilities never return success")
}

#[utoipa::path(
    post,
    path = "/api/v1/node/initialization",
    tag = "b1-conformance-fixture",
    description = "Ephemeral bootstrap transition. Runtime availability is fixture_only and durability is none.",
    request_body = InitializeNodeRequest,
    responses(
        (status = 200, description = "One-time initialization proof", body = InitializeNodeResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Node was already initialized", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the fixture bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "JSON does not match the request schema", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn initialize_node(
    State(state): State<Arc<B1HttpState>>,
    request: Result<Json<InitializeNodeRequest>, JsonRejection>,
) -> HttpResult<InitializeNodeResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(_request) = request.map_err(|rejection| {
        json_rejection(CapabilityKey::InitializeNode, correlation_id, rejection)
    })?;
    let initialization = state
        .fixture
        .initialize_node(correlation_id)
        .map_err(application_problem)?
        .into_inner();
    let proof = InitializationProof::fresh();
    let encoded_proof = encode_secret(proof.expose_for_fixture());
    let mut transport = state
        .transport
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    transport.initialization = Some(initialization);
    transport.initialization_proof = Some(proof);
    Ok(Json(InitializeNodeResponse {
        conformance: ConformanceMarkerDto::FIXTURE_ONLY,
        initialization_proof: encoded_proof,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/client-enrollments",
    tag = "b1-conformance-fixture",
    description = "First-client enrollment with one-time credential delivery. Runtime availability is fixture_only and durability is none.",
    request_body = EnrollFirstClientRequest,
    responses(
        (status = 200, description = "One-time opaque bearer credential", body = EnrollFirstClientResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Initialization proof denied or enrollment already completed", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the fixture bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "JSON does not match the request schema", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn enroll_first_client(
    State(state): State<Arc<B1HttpState>>,
    request: Result<Json<EnrollFirstClientRequest>, JsonRejection>,
) -> HttpResult<EnrollFirstClientResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let Json(request) = request.map_err(|rejection| {
        json_rejection(CapabilityKey::EnrollFirstClient, correlation_id, rejection)
    })?;
    let mut transport = state
        .transport
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut proof_bytes = request.initialization_proof.into_bytes();
    let mut presented_proof = std::str::from_utf8(&proof_bytes)
        .ok()
        .and_then(decode_secret);
    proof_bytes.fill(0);
    let proof_matches = transport
        .initialization_proof
        .as_ref()
        .zip(presented_proof.as_ref())
        .is_some_and(|(expected, presented)| {
            constant_time_eq(expected.expose_for_fixture(), presented)
        });
    if let Some(presented) = presented_proof.as_mut() {
        presented.fill(0);
    }
    if !proof_matches {
        return Err(forbidden(CapabilityKey::EnrollFirstClient, correlation_id));
    }
    let initialization = transport
        .initialization
        .as_ref()
        .ok_or_else(|| forbidden(CapabilityKey::EnrollFirstClient, correlation_id))?;
    let enrollment = state
        .fixture
        .enroll_first_client(correlation_id, initialization)
        .map_err(application_problem)?
        .into_inner();
    let credential = encode_secret(enrollment.credential_secret().expose_for_fixture());
    transport.initialization = None;
    transport.initialization_proof = None;
    transport.enrollment = Some(enrollment);

    Ok(Json(EnrollFirstClientResponse {
        conformance: ConformanceMarkerDto::FIXTURE_ONLY,
        credential_scheme: CredentialSchemeDto::Bearer,
        credential,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/observations",
    tag = "b1-conformance-fixture",
    security(("fixture_bearer" = [])),
    description = "Ephemeral observation acceptance. Runtime availability is fixture_only and durability is none.",
    request_body = AcceptObservationRequest,
    responses(
        (status = 200, description = "Capability-bound fixture receipt", body = AcceptObservationResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential or capability grant denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "Operation ID reused with different request semantics", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the fixture bound", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Observation contract rejected", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 507, description = "Bounded fixture operation capacity reached", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn accept_observation(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
    request: Result<Json<AcceptObservationRequest>, JsonRejection>,
) -> HttpResult<AcceptObservationResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let transport = state
        .transport
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let enrollment = authenticate_enrollment(
        &headers,
        &transport,
        CapabilityKey::AcceptObservation,
        correlation_id,
    )?;
    let Json(request) = request.map_err(|rejection| {
        json_rejection(CapabilityKey::AcceptObservation, correlation_id, rejection)
    })?;
    let command = map_observation(request, *enrollment.access(), correlation_id)?;
    let outcome = state
        .fixture
        .accept_fixture(enrollment.credential_secret(), command)
        .map_err(application_problem)?
        .into_inner();
    // Keep the guard live through the fixture call so enrollment state cannot
    // be swapped between transport authentication and application authorization.
    drop(transport);

    let disposition = if outcome.is_replay() {
        ReceiptDispositionDto::Replayed
    } else {
        ReceiptDispositionDto::Committed
    };
    Ok(Json(AcceptObservationResponse {
        conformance: ConformanceMarkerDto::FIXTURE_ONLY,
        disposition,
        receipt: receipt_dto(outcome.receipt()),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/receipts/{receipt_id}",
    tag = "b1-conformance-fixture",
    security(("fixture_bearer" = [])),
    description = "Ephemeral exact receipt replay. Runtime availability is fixture_only and durability is none.",
    params(("receipt_id" = String, Path, description = "Typed Fasti receipt ID")),
    responses(
        (status = 200, description = "Exact original fixture receipt", body = ReplayReceiptResponse),
        (status = 403, description = "Credential or capability grant denied", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "Receipt absent", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
async fn replay_receipt(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
    Path(receipt_id): Path<String>,
) -> HttpResult<ReplayReceiptResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let receipt_id = receipt_id.parse::<ReceiptId>().map_err(|_| {
        application_problem(Box::new(FastiProblem::receipt_not_found(
            CapabilityKey::ReplayReceipt,
            correlation_id,
        )))
    })?;
    let transport = state
        .transport
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let enrollment = authenticate_enrollment(
        &headers,
        &transport,
        CapabilityKey::ReplayReceipt,
        correlation_id,
    )?;
    let query = ReplayReceiptQuery::new(correlation_id, *enrollment.access(), receipt_id);
    let receipt = state
        .fixture
        .replay_fixture(enrollment.credential_secret(), query)
        .map_err(application_problem)?
        .into_inner();
    Ok(Json(ReplayReceiptResponse {
        conformance: ConformanceMarkerDto::FIXTURE_ONLY,
        receipt: receipt_dto(&receipt),
    }))
}

/// AsyncAPI-governed finite fixture replay. This route is deliberately absent
/// from the finite conformance OpenAPI document.
async fn stream_receipts(
    State(state): State<Arc<B1HttpState>>,
    headers: HeaderMap,
) -> Result<Response, HttpProblem> {
    let correlation_id = RequestCorrelationId::new_v7();
    let last_event_id = headers.get(LAST_EVENT_ID).map(|value| {
        value
            .to_str()
            .map(str::to_owned)
            .unwrap_or_else(|_| "invalid-non-utf8-cursor".to_owned())
    });
    let transport = state
        .transport
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let enrollment = authenticate_enrollment(
        &headers,
        &transport,
        CapabilityKey::StreamReceipts,
        correlation_id,
    )?;
    let batch = state
        .fixture
        .stream_receipts_fixture(
            enrollment.credential_secret(),
            StreamReceiptsQuery::new(correlation_id, *enrollment.access(), last_event_id),
        )
        .map_err(application_problem)?
        .into_inner();
    drop(transport);

    let events = batch.events().iter().map(|stream_event| {
        let receipt = stream_event.receipt();
        let payload = serde_json::json!({
            "capability_id": public_capability_id(CapabilityKey::AcceptObservation),
            "correlation_id": stream_event.correlation_id().to_string(),
            "receipt_id": receipt.receipt_id().to_string(),
            "operation_id": receipt.operation_id().to_string(),
            "observation_id": receipt.observation_id().to_string(),
            "resolution": "unresolved",
            "committed_at": receipt.committed_at().value().to_rfc3339()
        });
        Ok::<Event, Infallible>(
            Event::default()
                .event("receiptCommitted")
                .id(stream_event.cursor().to_string())
                .json_data(payload)
                .expect("governed receipt envelope is serializable"),
        )
    });
    let response = Sse::new(tokio_stream::iter(events.collect::<Vec<_>>())).into_response();
    Ok(response)
}

fn authenticate_enrollment<'a>(
    headers: &HeaderMap,
    transport: &'a TransportState,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Result<&'a FixtureEnrollment, HttpProblem> {
    let enrollment = transport
        .enrollment
        .as_ref()
        .ok_or_else(|| forbidden(capability, correlation_id))?;
    let encoded = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(BEARER_PREFIX))
        .ok_or_else(|| forbidden(capability, correlation_id))?;
    let mut presented =
        decode_secret(encoded).ok_or_else(|| forbidden(capability, correlation_id))?;
    let allowed = constant_time_eq(
        enrollment.credential_secret().expose_for_fixture(),
        &presented,
    );
    presented.fill(0);
    if !allowed {
        return Err(forbidden(capability, correlation_id));
    }
    Ok(enrollment)
}

fn map_observation(
    request: AcceptObservationRequest,
    access: fasti_application::RequestAccessContext,
    correlation_id: RequestCorrelationId,
) -> Result<AcceptObservationCommand, HttpProblem> {
    let operation_id = request
        .operation_id
        .parse::<OperationId>()
        .map_err(|_| invalid_observation(correlation_id, "/operation_id", "typed operation ID"))?;
    let evidence_id = request
        .evidence
        .evidence_id
        .parse::<EvidenceId>()
        .map_err(|_| {
            invalid_observation(correlation_id, "/evidence/evidence_id", "typed evidence ID")
        })?;
    let digest = Sha256Digest::parse(request.evidence.digest).map_err(|_| {
        invalid_observation(
            correlation_id,
            "/evidence/digest",
            "canonical sha256 digest",
        )
    })?;
    let occurred_at = request
        .occurred_at
        .map(|value| map_occurred_at(value, correlation_id))
        .transpose()?;
    let observed_at = map_observed_at(request.observed_at, correlation_id)?;
    Ok(AcceptObservationCommand::new(
        correlation_id,
        access,
        operation_id,
        occurred_at,
        observed_at,
        EvidenceReference::new(evidence_id, digest, request.evidence.byte_length),
    ))
}

fn map_occurred_at(
    value: OccurredTimeDto,
    correlation_id: RequestCorrelationId,
) -> Result<OccurredAt, HttpProblem> {
    let occurred = OccurredAt::parse(value.original, map_trust(value.trust)).map_err(|_| {
        invalid_observation(
            correlation_id,
            "/occurred_at",
            "ISO date or RFC 3339 timestamp",
        )
    })?;
    if occurred.claim().precision() != map_precision(value.precision) {
        return Err(invalid_observation(
            correlation_id,
            "/occurred_at/precision",
            "precision matching the original value",
        ));
    }
    Ok(occurred)
}

fn map_observed_at(
    value: ObservedTimeDto,
    correlation_id: RequestCorrelationId,
) -> Result<ObservedAt, HttpProblem> {
    let observed = ObservedAt::parse(value.original, map_trust(value.trust))
        .map_err(|_| invalid_observation(correlation_id, "/observed_at", "RFC 3339 timestamp"))?;
    if observed.claim().precision() != map_precision(value.precision) {
        return Err(invalid_observation(
            correlation_id,
            "/observed_at/precision",
            "precision matching the original value",
        ));
    }
    Ok(observed)
}

const fn map_precision(value: ClaimedPrecisionDto) -> ClaimedPrecision {
    match value {
        ClaimedPrecisionDto::Date => ClaimedPrecision::Date,
        ClaimedPrecisionDto::Second => ClaimedPrecision::Second,
        ClaimedPrecisionDto::Millisecond => ClaimedPrecision::Millisecond,
        ClaimedPrecisionDto::Microsecond => ClaimedPrecision::Microsecond,
        ClaimedPrecisionDto::Nanosecond => ClaimedPrecision::Nanosecond,
    }
}

const fn map_trust(value: ClaimedTrustDto) -> ClaimedTrust {
    match value {
        ClaimedTrustDto::SourceClaim => ClaimedTrust::SourceClaim,
        ClaimedTrustDto::DeviceObserved => ClaimedTrust::DeviceObserved,
        ClaimedTrustDto::UserEntered => ClaimedTrust::UserEntered,
        ClaimedTrustDto::Inferred => ClaimedTrust::Inferred,
    }
}

fn receipt_dto(receipt: &AcceptObservationReceipt) -> ObservationReceiptDto {
    ObservationReceiptDto {
        receipt_id: receipt.receipt_id().to_string(),
        operation_id: receipt.operation_id().to_string(),
        workspace_id: receipt.workspace_id().to_string(),
        profile_id: receipt.profile_id().to_string(),
        source_client_id: receipt.source_client_id().to_string(),
        observation_id: receipt.observation_id().to_string(),
        evidence_id: receipt.evidence_id().to_string(),
        payload_digest: receipt.payload_digest().to_string(),
        resolution: ObservationResolutionDto::Unresolved,
        received_at: receipt.received_at().value().to_rfc3339(),
        committed_at: receipt.committed_at().value().to_rfc3339(),
    }
}

fn invalid_observation(
    correlation_id: RequestCorrelationId,
    pointer: &str,
    expected: &str,
) -> HttpProblem {
    let violation = Violation::try_new("invalid_value", pointer, "field is invalid", expected)
        .expect("static violation metadata is valid");
    let problem = FastiProblem::invalid_observation(correlation_id, vec![violation])
        .expect("one static violation is within bounds");
    application_problem(Box::new(problem))
}

fn json_rejection(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    rejection: JsonRejection,
) -> HttpProblem {
    let status = rejection.status();
    let code = match status {
        StatusCode::BAD_REQUEST => ProblemCode::MalformedJson,
        StatusCode::PAYLOAD_TOO_LARGE => ProblemCode::PayloadTooLarge,
        StatusCode::UNSUPPORTED_MEDIA_TYPE => ProblemCode::UnsupportedMediaType,
        _ => ProblemCode::ValidationFailed,
    };
    let violation_contract = code
        .representation_violation()
        .expect("representation problem owns violation metadata");
    let violation = Violation::try_new(
        violation_contract.code(),
        violation_contract.pointer(),
        violation_contract.reason(),
        violation_contract.expected(),
    )
    .expect("application-owned rejection metadata is valid");
    let problem = match code {
        ProblemCode::MalformedJson => {
            FastiProblem::malformed_json(capability, correlation_id, vec![violation])
        }
        ProblemCode::PayloadTooLarge => {
            FastiProblem::payload_too_large(capability, correlation_id, vec![violation])
        }
        ProblemCode::UnsupportedMediaType => {
            FastiProblem::unsupported_media_type(capability, correlation_id, vec![violation])
        }
        ProblemCode::ValidationFailed => {
            FastiProblem::validation_failed(capability, correlation_id, vec![violation])
        }
        _ => unreachable!("only representation problem codes are selected"),
    }
    .expect("one static violation is within bounds");
    application_problem(Box::new(problem))
}

fn forbidden(capability: CapabilityKey, correlation_id: RequestCorrelationId) -> HttpProblem {
    application_problem(Box::new(FastiProblem::forbidden(
        capability,
        correlation_id,
    )))
}

fn application_problem(problem: Box<FastiProblem>) -> HttpProblem {
    let capability_id = public_capability_id(problem.capability());
    let dto = ProblemDetails::from_application(&problem, capability_id, DOCUMENTATION_BASE);
    let status = StatusCode::from_u16(dto.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    HttpProblem {
        status,
        body: Box::new(dto),
    }
}

fn encode_secret(secret: &[u8; 32]) -> String {
    use std::fmt::Write as _;
    let mut encoded = String::with_capacity(64);
    for byte in secret {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn decode_secret(encoded: &str) -> Option<[u8; 32]> {
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, chunk) in encoded.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        decoded[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(decoded)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "fixture_bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("opaque fixture credential")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        discover_capabilities,
        select_profile_unavailable,
        rotate_credential_unavailable,
        revoke_credential_unavailable,
        configure_listener_unavailable,
        initialize_node,
        enroll_first_client,
        accept_observation,
        replay_receipt
    ),
    components(schemas(
        fasti_contracts::AcceptObservationRequest,
        fasti_contracts::AcceptObservationResponse,
        fasti_contracts::CapabilityDescriptorDto,
        fasti_contracts::CapabilityDiscoveryResponse,
        fasti_contracts::CapabilityLifecycleDto,
        fasti_contracts::CapabilityUatDto,
        fasti_contracts::ClaimedPrecisionDto,
        fasti_contracts::ClaimedTrustDto,
        fasti_contracts::ConformanceMarkerDto,
        fasti_contracts::CredentialSchemeDto,
        fasti_contracts::DurabilityDto,
        fasti_contracts::EnrollFirstClientRequest,
        fasti_contracts::EnrollFirstClientResponse,
        fasti_contracts::EvidenceReferenceDto,
        fasti_contracts::InitializeNodeRequest,
        fasti_contracts::InitializeNodeResponse,
        fasti_contracts::ObservationReceiptDto,
        fasti_contracts::ObservationResolutionDto,
        fasti_contracts::ObservedTimeDto,
        fasti_contracts::OccurredTimeDto,
        fasti_contracts::ProblemActionDto,
        fasti_contracts::ProblemDetails,
        fasti_contracts::ReceiptDispositionDto,
        fasti_contracts::ReplayReceiptResponse,
        fasti_contracts::RuntimeAvailabilityDto,
        fasti_contracts::ViolationDto
    )),
    modifiers(&SecurityAddon),
    info(
        title = "Fasti B1 conformance fixture",
        description = "Non-production executable contract surface. Every success is fixture_only with durability none."
    )
)]
struct B1ConformanceDoc;

pub fn b1_conformance_openapi() -> utoipa::openapi::OpenApi {
    B1ConformanceDoc::openapi()
}

pub fn b1_conformance_router() -> Router {
    Router::new()
        .route("/api/v1/capabilities", get(discover_capabilities))
        .route("/api/v1/profile-selection", put(select_profile_unavailable))
        .route(
            "/api/v1/credential-rotations",
            post(rotate_credential_unavailable),
        )
        .route(
            "/api/v1/credential-revocations",
            post(revoke_credential_unavailable),
        )
        .route(
            "/api/v1/listener-configuration",
            put(configure_listener_unavailable),
        )
        .route("/api/v1/node/initialization", post(initialize_node))
        .route("/api/v1/client-enrollments", post(enroll_first_client))
        .route("/api/v1/observations", post(accept_observation))
        .route("/api/v1/receipts/stream", get(stream_receipts))
        .route("/api/v1/receipts/{receipt_id}", get(replay_receipt))
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_BYTES))
        .with_state(Arc::new(B1HttpState::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, Response},
    };
    use fasti_contracts::{DurabilityDto, RuntimeAvailabilityDto};
    use fasti_domain::{EvidenceId, OperationId, ReceiptId};
    use serde::de::DeserializeOwned;
    use static_assertions::assert_not_impl_any;
    use tower::ServiceExt;

    assert_not_impl_any!(InitializationProof: std::fmt::Debug, Clone);

    async fn response_json<T: DeserializeOwned>(response: Response<Body>) -> T {
        let bytes = to_bytes(response.into_body(), 256 * 1024)
            .await
            .expect("bounded response body");
        serde_json::from_slice(&bytes).expect("JSON response")
    }

    async fn assert_problem_response(
        response: Response<Body>,
        status: StatusCode,
    ) -> ProblemDetails {
        assert_eq!(response.status(), status);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/problem+json")
        );
        let problem: ProblemDetails = response_json(response).await;
        assert_eq!(problem.status, status.as_u16());
        problem
    }

    fn json_request(method: &str, uri: &str, value: &impl serde::Serialize) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(value).expect("serializable request"),
            ))
            .expect("valid request")
    }

    fn authorized_json_request(
        method: &str,
        uri: &str,
        credential: &str,
        value: &impl serde::Serialize,
    ) -> Request<Body> {
        let mut request = json_request(method, uri, value);
        request.headers_mut().insert(
            header::AUTHORIZATION,
            format!("Bearer {credential}")
                .parse()
                .expect("valid credential header"),
        );
        request
    }

    fn authorized_empty_request(method: &str, uri: &str, credential: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header(header::AUTHORIZATION, format!("Bearer {credential}"))
            .body(Body::empty())
            .expect("valid authorized request")
    }

    async fn initialize_and_enroll(router: &Router) -> String {
        let initialize = router
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/v1/node/initialization",
                &InitializeNodeRequest {},
            ))
            .await
            .expect("initialize response");
        assert_eq!(initialize.status(), StatusCode::OK);
        let initialized: InitializeNodeResponse = response_json(initialize).await;
        assert_eq!(
            initialized.conformance.availability,
            RuntimeAvailabilityDto::FixtureOnly
        );
        assert_eq!(initialized.conformance.durability, DurabilityDto::None);

        let enroll = router
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/v1/client-enrollments",
                &EnrollFirstClientRequest {
                    initialization_proof: initialized.initialization_proof,
                },
            ))
            .await
            .expect("enrollment response");
        assert_eq!(enroll.status(), StatusCode::OK);
        let enrolled: EnrollFirstClientResponse = response_json(enroll).await;
        assert_eq!(enrolled.credential_scheme, CredentialSchemeDto::Bearer);
        assert_eq!(enrolled.credential.len(), 64);
        enrolled.credential
    }

    fn observation_request() -> AcceptObservationRequest {
        AcceptObservationRequest {
            operation_id: OperationId::new_v7().to_string(),
            occurred_at: None,
            observed_at: ObservedTimeDto {
                original: "2026-08-22T10:11:12.123Z".to_owned(),
                precision: ClaimedPrecisionDto::Millisecond,
                trust: ClaimedTrustDto::DeviceObserved,
            },
            evidence: fasti_contracts::EvidenceReferenceDto {
                evidence_id: EvidenceId::new_v7().to_string(),
                digest: format!("sha256:{}", "ab".repeat(32)),
                byte_length: 42,
            },
        }
    }

    #[test]
    fn finite_conformance_openapi_excludes_asyncapi_stream_and_matches_bindings() {
        let document = b1_conformance_openapi();
        let actual = document
            .paths
            .paths
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        let expected = [
            "/api/v1/capabilities",
            "/api/v1/client-enrollments",
            "/api/v1/credential-revocations",
            "/api/v1/credential-rotations",
            "/api/v1/listener-configuration",
            "/api/v1/node/initialization",
            "/api/v1/observations",
            "/api/v1/profile-selection",
            "/api/v1/receipts/{receipt_id}",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(actual, expected);
        assert!(!document.paths.paths.contains_key("/api/v1/health"));
        assert!(!document.paths.paths.contains_key("/api/v1/receipts/stream"));

        let serialized = serde_json::to_string(&document).expect("serializable OpenAPI");
        assert!(serialized.contains("fixture_only"));
        assert!(serialized.contains("durability is none"));
        assert!(serialized.contains("fixture_bearer"));
        assert!(serialized.contains(r#"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"#));
        assert!(serialized.contains(r#"^sha256:[0-9a-f]{64}$"#));
        assert!(serialized.contains(r#""format":"date-time""#));

        for path in [
            "/api/v1/node/initialization",
            "/api/v1/client-enrollments",
            "/api/v1/observations",
        ] {
            let operation = document.paths.paths[path]
                .post
                .as_ref()
                .expect("documented POST operation");
            for status in ["400", "413", "415", "422"] {
                assert!(
                    operation.responses.responses.contains_key(status),
                    "{path} is missing governed response {status}"
                );
                let response = &operation.responses.responses[status];
                let utoipa::openapi::RefOr::T(response) = response else {
                    panic!("{path} response {status} must be inline");
                };
                assert!(
                    response.content.contains_key("application/problem+json"),
                    "{path} response {status} has the wrong media type"
                );
            }
        }

        for (path, method) in [
            ("/api/v1/profile-selection", "put"),
            ("/api/v1/credential-rotations", "post"),
            ("/api/v1/credential-revocations", "post"),
            ("/api/v1/listener-configuration", "put"),
        ] {
            let item = &document.paths.paths[path];
            let operation = match method {
                "post" => item.post.as_ref(),
                "put" => item.put.as_ref(),
                _ => unreachable!(),
            }
            .expect("problem-only operation is documented");
            assert!(!operation.responses.responses.contains_key("200"));
            for status in ["403", "501"] {
                let utoipa::openapi::RefOr::T(response) = &operation.responses.responses[status]
                else {
                    panic!("{path} response {status} must be inline");
                };
                assert!(response.content.contains_key("application/problem+json"));
            }
        }
    }

    #[tokio::test]
    async fn json_rejections_are_bounded_governed_problem_responses() {
        let cases = [
            (
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{"))
                    .expect("malformed request"),
                StatusCode::BAD_REQUEST,
                "malformed_json",
            ),
            (
                Request::post("/api/v1/node/initialization")
                    .body(Body::from("{}"))
                    .expect("missing content type request"),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
            ),
            (
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"extra":true}"#))
                    .expect("schema mismatch request"),
                StatusCode::UNPROCESSABLE_ENTITY,
                "validation_failed",
            ),
            (
                Request::post("/api/v1/node/initialization")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(" ".repeat(MAX_JSON_BODY_BYTES + 1)))
                    .expect("oversized request"),
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
            ),
        ];

        for (request, expected, expected_code) in cases {
            let response = b1_conformance_router()
                .oneshot(request)
                .await
                .expect("governed rejection response");
            let problem = assert_problem_response(response, expected).await;
            assert_eq!(problem.code, expected_code);
            assert_eq!(problem.safe_state, "no_mutation");
            assert_eq!(problem.actual, ());
            if expected_code == "validation_failed" {
                let mut governed: serde_json::Value = serde_json::from_str(include_str!(
                    "../../../contracts/examples/v1/node.initialize.validation_failed.json"
                ))
                .expect("governed validation example");
                governed["correlation_id"] =
                    serde_json::Value::String(problem.correlation_id.clone());
                assert_eq!(
                    serde_json::to_value(&problem).expect("runtime problem JSON"),
                    governed,
                    "runtime validation semantics must equal the governed example after correlation normalization"
                );
                assert_eq!(problem.violations.len(), 1);
                let violation = &problem.violations[0];
                assert_eq!(violation.code, "invalid_representation");
                assert_eq!(violation.pointer, "/");
                assert_eq!(
                    violation.reason,
                    "request JSON does not match the governed schema"
                );
                assert_eq!(violation.expected, "the documented request schema");
            }
        }
    }

    #[tokio::test]
    async fn observation_validation_runtime_matches_its_governed_example() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;
        let response = router
            .oneshot(authorized_json_request(
                "POST",
                "/api/v1/observations",
                &credential,
                &serde_json::json!({ "unexpected": true }),
            ))
            .await
            .expect("observation validation response");
        let problem = assert_problem_response(response, StatusCode::UNPROCESSABLE_ENTITY).await;
        assert_eq!(problem.code, "validation_failed");
        let mut governed: serde_json::Value = serde_json::from_str(include_str!(
            "../../../contracts/examples/v1/observation.accept.validation_failed.json"
        ))
        .expect("governed observation validation example");
        governed["correlation_id"] = serde_json::Value::String(problem.correlation_id.clone());
        assert_eq!(
            serde_json::to_value(problem).expect("runtime observation problem JSON"),
            governed,
            "observation validation runtime must equal its governed example after correlation normalization"
        );
    }

    #[tokio::test]
    async fn full_fixture_flow_returns_exact_receipts_and_registry_discovery() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;

        let capabilities = router
            .clone()
            .oneshot(authorized_json_request(
                "GET",
                "/api/v1/capabilities",
                &credential,
                &serde_json::Value::Null,
            ))
            .await
            .expect("capabilities response");
        assert_eq!(capabilities.status(), StatusCode::OK);
        let capabilities: CapabilityDiscoveryResponse = response_json(capabilities).await;
        assert_eq!(capabilities.capabilities.len(), CapabilityKey::ALL.len());
        assert!(capabilities
            .capabilities
            .iter()
            .any(|capability| capability.id == "observation.accept"));

        let request = observation_request();
        let accepted = router
            .clone()
            .oneshot(authorized_json_request(
                "POST",
                "/api/v1/observations",
                &credential,
                &request,
            ))
            .await
            .expect("accept response");
        assert_eq!(accepted.status(), StatusCode::OK);
        let accepted: AcceptObservationResponse = response_json(accepted).await;
        assert_eq!(accepted.disposition, ReceiptDispositionDto::Committed);
        let accepted_receipt = accepted.receipt.clone();

        let repeated = router
            .clone()
            .oneshot(authorized_json_request(
                "POST",
                "/api/v1/observations",
                &credential,
                &request,
            ))
            .await
            .expect("repeat response");
        assert_eq!(repeated.status(), StatusCode::OK);
        let repeated: AcceptObservationResponse = response_json(repeated).await;
        assert_eq!(repeated.disposition, ReceiptDispositionDto::Replayed);
        assert_eq!(repeated.receipt, accepted_receipt);

        let replay = router
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/receipts/{}", accepted_receipt.receipt_id))
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("valid replay request"),
            )
            .await
            .expect("replay response");
        assert_eq!(replay.status(), StatusCode::OK);
        let replay: ReplayReceiptResponse = response_json(replay).await;
        assert_eq!(replay.receipt, accepted_receipt);
        let serialized = serde_json::to_value(&replay).expect("serializable replay");
        assert!(serialized.get("record_id").is_none());
        assert!(serialized.get("occurrence_id").is_none());
        assert!(serialized["receipt"].get("record_id").is_none());
        assert!(serialized["receipt"].get("occurrence_id").is_none());
    }

    #[tokio::test]
    async fn problem_only_admin_bindings_authorize_then_return_typed_501() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;
        for (method, path, capability_id) in [
            ("PUT", "/api/v1/profile-selection", "profile.select"),
            ("POST", "/api/v1/credential-rotations", "credential.rotate"),
            (
                "POST",
                "/api/v1/credential-revocations",
                "credential.revoke",
            ),
            (
                "PUT",
                "/api/v1/listener-configuration",
                "listener.configure",
            ),
        ] {
            let response = router
                .clone()
                .oneshot(authorized_empty_request(method, path, &credential))
                .await
                .expect("problem-only response");
            let problem = assert_problem_response(response, StatusCode::NOT_IMPLEMENTED).await;
            assert_eq!(problem.code, "capability_unavailable");
            assert_eq!(problem.capability_id, capability_id);
            assert_eq!(problem.safe_state, "no_mutation");
        }

        let unauthenticated = router
            .oneshot(
                Request::put("/api/v1/profile-selection")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("denial response");
        let problem = assert_problem_response(unauthenticated, StatusCode::FORBIDDEN).await;
        assert_eq!(problem.capability_id, "profile.select");
    }

    #[tokio::test]
    async fn asyncapi_receipt_stream_is_ordered_cursor_bounded_and_authenticated() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;
        let mut receipts = Vec::new();
        for _ in 0..3 {
            let response = router
                .clone()
                .oneshot(authorized_json_request(
                    "POST",
                    "/api/v1/observations",
                    &credential,
                    &observation_request(),
                ))
                .await
                .expect("acceptance response");
            let accepted: AcceptObservationResponse = response_json(response).await;
            receipts.push(accepted.receipt);
        }

        let response = router
            .clone()
            .oneshot(authorized_empty_request(
                "GET",
                "/api/v1/receipts/stream",
                &credential,
            ))
            .await
            .expect("SSE response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/event-stream")
        );
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("bounded finite stream body");
        let body = std::str::from_utf8(&body).expect("UTF-8 SSE");
        let frames = body
            .split("\n\n")
            .filter(|frame| !frame.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(frames.len(), 3);
        for (frame, receipt) in frames.iter().zip(&receipts) {
            assert!(frame.contains("event: receiptCommitted"));
            assert!(frame.contains(&format!("id: {}", receipt.receipt_id)));
            let data = frame
                .lines()
                .find_map(|line| line.strip_prefix("data: "))
                .expect("SSE data line");
            let payload: serde_json::Value = serde_json::from_str(data).expect("event JSON");
            assert_eq!(payload["capability_id"], "observation.accept");
            assert_eq!(payload["receipt_id"], receipt.receipt_id);
            assert_eq!(payload["operation_id"], receipt.operation_id);
            assert_eq!(payload["observation_id"], receipt.observation_id);
            assert_eq!(payload["resolution"], "unresolved");
            assert!(payload.get("correlation_id").is_some());
            assert!(payload.get("committed_at").is_some());
            assert!(payload.get("record_id").is_none());
            assert!(payload.get("occurrence_id").is_none());
        }

        let mut cursor_request =
            authorized_empty_request("GET", "/api/v1/receipts/stream", &credential);
        cursor_request.headers_mut().insert(
            LAST_EVENT_ID,
            receipts[1].receipt_id.parse().expect("valid cursor header"),
        );
        let cursor_response = router
            .clone()
            .oneshot(cursor_request)
            .await
            .expect("cursor response");
        let cursor_body = to_bytes(cursor_response.into_body(), 16 * 1024)
            .await
            .expect("bounded cursor body");
        let cursor_body = std::str::from_utf8(&cursor_body).expect("UTF-8 SSE");
        assert!(!cursor_body.contains(&receipts[0].receipt_id));
        assert!(!cursor_body.contains(&receipts[1].receipt_id));
        assert!(cursor_body.contains(&receipts[2].receipt_id));

        for cursor in ["invalid-cursor".to_owned(), ReceiptId::new_v7().to_string()] {
            let mut request =
                authorized_empty_request("GET", "/api/v1/receipts/stream", &credential);
            request
                .headers_mut()
                .insert(LAST_EVENT_ID, cursor.parse().expect("header value"));
            let response = router
                .clone()
                .oneshot(request)
                .await
                .expect("cursor denial");
            let problem = assert_problem_response(response, StatusCode::NOT_FOUND).await;
            assert_eq!(problem.code, "receipt_not_found");
            assert_eq!(problem.capability_id, "receipt.stream");
            assert_eq!(problem.param.as_deref(), Some("/last_event_id"));
        }

        let missing_auth = router
            .oneshot(
                Request::get("/api/v1/receipts/stream")
                    .body(Body::empty())
                    .expect("valid request"),
            )
            .await
            .expect("auth denial");
        let problem = assert_problem_response(missing_auth, StatusCode::FORBIDDEN).await;
        assert_eq!(problem.capability_id, "receipt.stream");
    }

    #[tokio::test]
    async fn runtime_rejects_schema_mutations_and_impossible_observed_instants() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;
        let baseline = observation_request();

        let mut invalid_operation = baseline.clone();
        invalid_operation.operation_id = format!("op_{}4{}", "0".repeat(12), "0".repeat(19));
        let mut invalid_evidence = baseline.clone();
        invalid_evidence.evidence.evidence_id = "evd_not-an-id".to_owned();
        let mut invalid_digest = baseline.clone();
        invalid_digest.evidence.digest = format!("sha256:{}", "AB".repeat(32));
        let mut impossible_time = baseline;
        impossible_time.observed_at.original = "2026-02-31T10:11:12Z".to_owned();

        for request in [
            invalid_operation,
            invalid_evidence,
            invalid_digest,
            impossible_time,
        ] {
            let response = router
                .clone()
                .oneshot(authorized_json_request(
                    "POST",
                    "/api/v1/observations",
                    &credential,
                    &request,
                ))
                .await
                .expect("validation response");
            let problem = assert_problem_response(response, StatusCode::UNPROCESSABLE_ENTITY).await;
            assert_eq!(problem.code, "invalid_observation");
            assert_eq!(problem.safe_state, "no_mutation");
        }
    }

    #[tokio::test]
    async fn http_capacity_is_bounded_and_returns_typed_507_problem() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;

        for _ in 0..fasti_application::conformance::MAX_FIXTURE_OPERATIONS {
            let response = router
                .clone()
                .oneshot(authorized_json_request(
                    "POST",
                    "/api/v1/observations",
                    &credential,
                    &observation_request(),
                ))
                .await
                .expect("bounded acceptance response");
            assert_eq!(response.status(), StatusCode::OK);
        }
        let response = router
            .oneshot(authorized_json_request(
                "POST",
                "/api/v1/observations",
                &credential,
                &observation_request(),
            ))
            .await
            .expect("capacity response");
        let problem = assert_problem_response(response, StatusCode::INSUFFICIENT_STORAGE).await;
        assert_eq!(problem.code, "capacity_exceeded");
        assert_eq!(problem.safe_state, "no_mutation");
        assert_eq!(problem.retryability, "retry_after_correction");
    }

    #[tokio::test]
    async fn bootstrap_proofs_are_fixed_csprng_values_without_identifier_prefixes() {
        let mut proofs = Vec::new();
        for _ in 0..2 {
            let response = b1_conformance_router()
                .oneshot(json_request(
                    "POST",
                    "/api/v1/node/initialization",
                    &InitializeNodeRequest {},
                ))
                .await
                .expect("initialization response");
            let initialized: InitializeNodeResponse = response_json(response).await;
            assert_eq!(initialized.initialization_proof.len(), 64);
            assert!(initialized
                .initialization_proof
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
            assert!(!initialized.initialization_proof.contains("req_"));
            proofs.push(initialized.initialization_proof);
        }
        assert_ne!(proofs[0], proofs[1]);
    }

    #[tokio::test]
    async fn conflicting_operation_is_409_without_replacing_original_receipt() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;
        let request = observation_request();
        let first = router
            .clone()
            .oneshot(authorized_json_request(
                "POST",
                "/api/v1/observations",
                &credential,
                &request,
            ))
            .await
            .expect("first response");
        let first: AcceptObservationResponse = response_json(first).await;

        let mut conflicting = request;
        conflicting.evidence.digest = format!("sha256:{}", "cd".repeat(32));
        let conflict = router
            .clone()
            .oneshot(authorized_json_request(
                "POST",
                "/api/v1/observations",
                &credential,
                &conflicting,
            ))
            .await
            .expect("conflict response");
        let problem = assert_problem_response(conflict, StatusCode::CONFLICT).await;
        assert_eq!(problem.code, "idempotency_conflict");
        assert_eq!(problem.capability_id, "observation.accept");

        let replay = router
            .oneshot(
                Request::get(format!("/api/v1/receipts/{}", first.receipt.receipt_id))
                    .header(header::AUTHORIZATION, format!("Bearer {credential}"))
                    .body(Body::empty())
                    .expect("valid replay request"),
            )
            .await
            .expect("replay response");
        let replay: ReplayReceiptResponse = response_json(replay).await;
        assert_eq!(replay.receipt, first.receipt);
    }

    #[tokio::test]
    async fn missing_and_foreign_credentials_are_non_enumerating_and_redacted() {
        let router = b1_conformance_router();
        let credential = initialize_and_enroll(&router).await;
        let request = observation_request();

        for authorization in [None, Some("0".repeat(64))] {
            let mut http = json_request("POST", "/api/v1/observations", &request);
            if let Some(value) = authorization {
                http.headers_mut().insert(
                    header::AUTHORIZATION,
                    format!("Bearer {value}").parse().expect("valid header"),
                );
            }
            let response = router.clone().oneshot(http).await.expect("denial response");
            let problem = assert_problem_response(response, StatusCode::FORBIDDEN).await;
            assert_eq!(problem.code, "forbidden");
            let rendered = serde_json::to_string(&problem).expect("serializable problem");
            assert!(!rendered.contains(&credential));
            assert!(!rendered.contains(&request.evidence.digest));
        }
    }

    #[tokio::test]
    async fn enrollment_proof_is_one_time_and_never_echoed_by_problem() {
        let router = b1_conformance_router();
        let initialize = router
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/v1/node/initialization",
                &InitializeNodeRequest {},
            ))
            .await
            .expect("initialize response");
        let initialized: InitializeNodeResponse = response_json(initialize).await;
        let proof = initialized.initialization_proof;
        let request = EnrollFirstClientRequest {
            initialization_proof: proof.clone(),
        };
        let first = router
            .clone()
            .oneshot(json_request("POST", "/api/v1/client-enrollments", &request))
            .await
            .expect("first enrollment response");
        assert_eq!(first.status(), StatusCode::OK);
        let second = router
            .oneshot(json_request("POST", "/api/v1/client-enrollments", &request))
            .await
            .expect("second enrollment response");
        let problem = assert_problem_response(second, StatusCode::FORBIDDEN).await;
        assert!(!serde_json::to_string(&problem)
            .expect("serializable problem")
            .contains(&proof));
    }
}
