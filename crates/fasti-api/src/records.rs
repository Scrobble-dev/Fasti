use crate::local::{bearer_secret, run_kernel, LocalApiState};
use crate::problem::{application_problem, json_rejection, HttpProblem};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use fasti_application::{
    AttachIdentifierCommand, AuthenticateCredentialQuery, CapabilityKey, CreateRecordCommand,
    FastiProblem, ListRecordsQuery, RegisterNamespaceDefinitionCommand, Violation,
};
use fasti_contracts::{
    AttachIdentifierRequest, AttachIdentifierResponse, ClaimedPrecisionDto, ClaimedTrustDto,
    CreateRecordRequest, CreateRecordResponse, ListRecordsResponse, OccurredTimeDto,
    ProblemDetails, RecordActivityDto, RecordSummaryDto, RegisterNamespaceRequest,
    RegisterNamespaceResponse, ResolvedFieldDto,
};
use fasti_domain::{
    ClaimedPrecision, ClaimedTime, ClaimedTrust, ExternalIdentifierClaim, Grain,
    NamespaceDefinition, NamespaceLicencePosture, RequestCorrelationId, ResolvedField,
};
use std::str::FromStr;

type HttpResult<T> = Result<Json<T>, HttpProblem>;

fn invalid_identifier_input(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    application_problem(Box::new(FastiProblem::invalid_identifier(
        capability,
        correlation_id,
    )))
}

/// Namespace-definition rejections, unlike record/identifier input, are
/// declared in the registry catalog as `validation_failed`, not
/// `invalid_identifier` -- see `identity.namespace.register` in
/// `contracts/registry/v1/capabilities.yaml`.
fn invalid_namespace_definition(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> HttpProblem {
    let violation = Violation::try_new(
        "invalid_namespace_definition",
        "/",
        "namespace definition does not satisfy the domain contract",
        "a registered grain list, a known licence posture, and a valid identifier pattern",
    )
    .expect("adapter-owned namespace violation is valid");
    let problem = FastiProblem::validation_failed(capability, correlation_id, vec![violation])
        .expect("one namespace violation is within bounds");
    application_problem(Box::new(problem))
}

fn occurred_time_dto(claim: &ClaimedTime) -> OccurredTimeDto {
    OccurredTimeDto {
        original: claim.original().to_owned(),
        precision: match claim.precision() {
            ClaimedPrecision::Date => ClaimedPrecisionDto::Date,
            ClaimedPrecision::Second => ClaimedPrecisionDto::Second,
            ClaimedPrecision::Millisecond => ClaimedPrecisionDto::Millisecond,
            ClaimedPrecision::Microsecond => ClaimedPrecisionDto::Microsecond,
            ClaimedPrecision::Nanosecond => ClaimedPrecisionDto::Nanosecond,
        },
        trust: match claim.trust() {
            ClaimedTrust::SourceClaim => ClaimedTrustDto::SourceClaim,
            ClaimedTrust::DeviceObserved => ClaimedTrustDto::DeviceObserved,
            ClaimedTrust::UserEntered => ClaimedTrustDto::UserEntered,
            ClaimedTrust::Inferred => ClaimedTrustDto::Inferred,
        },
    }
}

fn resolved_field_dto(field: &ResolvedField) -> ResolvedFieldDto {
    ResolvedFieldDto {
        tier: serde_json::to_value(field.tier())
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default(),
        value: field.value().map(ToOwned::to_owned),
        source: field.source().map(ToString::to_string),
        is_stale: field.is_stale(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/records",
    tag = "records",
    security(("credential_bearer" = [])),
    request_body = CreateRecordRequest,
    responses(
        (status = 200, description = "The new record's identity", body = CreateRecordResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks record-write scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Grain is not a registered Fasti grain", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "This capability is not available in the current runtime body", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn create_record(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    request: Result<Json<CreateRecordRequest>, JsonRejection>,
) -> HttpResult<CreateRecordResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::CreateRecord;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let secret = bearer_secret(&headers, capability, correlation_id)?;
    let grain = Grain::from_str(&request.grain)
        .map_err(|_| invalid_identifier_input(capability, correlation_id))?;

    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        let access = kernel.authenticate_credential(AuthenticateCredentialQuery::new(
            correlation_id,
            capability,
            secret,
        ))?;
        kernel.create_record(CreateRecordCommand::new(correlation_id, access, grain))
    })
    .await?;

    Ok(Json(CreateRecordResponse {
        record_id: outcome.record_id().to_string(),
        grain: request.grain,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/records/identifiers",
    tag = "records",
    security(("credential_bearer" = [])),
    request_body = AttachIdentifierRequest,
    responses(
        (status = 200, description = "The attached (or already-present) identifier claim", body = AttachIdentifierResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks record-write scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 404, description = "Record does not exist", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 409, description = "This identifier is already attached to a different active record", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Record ID, grain, or namespace does not satisfy the domain contract", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "This capability is not available in the current runtime body", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn attach_identifier(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    request: Result<Json<AttachIdentifierRequest>, JsonRejection>,
) -> HttpResult<AttachIdentifierResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::AttachIdentifier;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let secret = bearer_secret(&headers, capability, correlation_id)?;
    let record_id = request
        .record_id
        .parse()
        .map_err(|_| invalid_identifier_input(capability, correlation_id))?;
    let grain = Grain::from_str(&request.grain)
        .map_err(|_| invalid_identifier_input(capability, correlation_id))?;
    let claim = ExternalIdentifierClaim::try_new(&request.namespace, grain, &request.value)
        .map_err(|_| invalid_identifier_input(capability, correlation_id))?;

    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        let access = kernel.authenticate_credential(AuthenticateCredentialQuery::new(
            correlation_id,
            capability,
            secret,
        ))?;
        kernel.attach_identifier(AttachIdentifierCommand::new(
            correlation_id,
            access,
            record_id,
            claim,
        ))
    })
    .await?;

    Ok(Json(AttachIdentifierResponse {
        external_identifier_id: outcome.external_identifier_id().to_string(),
        record_id: outcome.record_id().to_string(),
        created: outcome.created(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/records",
    tag = "records",
    security(("credential_bearer" = [])),
    responses(
        (status = 200, description = "Records visible to this credential's workspace", body = ListRecordsResponse),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks record-read scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "This capability is not available in the current runtime body", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn list_records(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
) -> HttpResult<ListRecordsResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::ListRecords;
    let secret = bearer_secret(&headers, capability, correlation_id)?;

    let kernel = state.kernel;
    let summaries = run_kernel(capability, correlation_id, move || {
        let access = kernel.authenticate_credential(AuthenticateCredentialQuery::new(
            correlation_id,
            capability,
            secret,
        ))?;
        kernel.list_records(ListRecordsQuery::new(correlation_id, access))
    })
    .await?;

    Ok(Json(ListRecordsResponse {
        records: summaries
            .into_iter()
            .map(|summary| RecordSummaryDto {
                record_id: summary.record_id().to_string(),
                grain: summary.grain().as_str().to_owned(),
                status: serde_json::to_value(summary.status())
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_else(|| "active".to_owned()),
                title: resolved_field_dto(summary.title()),
                poster: resolved_field_dto(summary.poster()),
                latest_activity: summary.latest_activity().map(|activity| RecordActivityDto {
                    occurred_at: activity
                        .occurred_at()
                        .map(|value| occurred_time_dto(value.claim())),
                    interpretation_state: serde_json::to_value(activity.interpretation_state())
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                }),
            })
            .collect(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/namespaces",
    tag = "records",
    security(("credential_bearer" = [])),
    request_body = RegisterNamespaceRequest,
    responses(
        (status = 200, description = "The registered (or already-present) namespace", body = RegisterNamespaceResponse),
        (status = 400, description = "Malformed JSON", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 401, description = "Bearer credential is missing or inactive", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 403, description = "Credential lacks record-write scope", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 413, description = "Request body exceeds the bounded transport limit", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 415, description = "Content-Type is not application/json", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 422, description = "Namespace definition does not satisfy the domain contract", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 500, description = "Durable state failed an integrity check", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 501, description = "This capability is not available in the current runtime body", body = ProblemDetails, content_type = "application/problem+json"),
        (status = 503, description = "Local storage is unavailable", body = ProblemDetails, content_type = "application/problem+json")
    )
)]
pub(crate) async fn register_namespace(
    State(state): State<LocalApiState>,
    headers: HeaderMap,
    request: Result<Json<RegisterNamespaceRequest>, JsonRejection>,
) -> HttpResult<RegisterNamespaceResponse> {
    let correlation_id = RequestCorrelationId::new_v7();
    let capability = CapabilityKey::RegisterNamespace;
    let Json(request) =
        request.map_err(|rejection| json_rejection(capability, correlation_id, rejection))?;
    let secret = bearer_secret(&headers, capability, correlation_id)?;

    let mut grains = Vec::with_capacity(request.grains.len());
    for grain in &request.grains {
        grains.push(
            Grain::from_str(grain)
                .map_err(|_| invalid_namespace_definition(capability, correlation_id))?,
        );
    }
    let licence_posture = match request.licence_posture.as_str() {
        "open" => NamespaceLicencePosture::Open,
        "identifiers_only" => NamespaceLicencePosture::IdentifiersOnly,
        "indirect_only" => NamespaceLicencePosture::IndirectOnly,
        "excluded" => NamespaceLicencePosture::Excluded,
        "unknown" => NamespaceLicencePosture::Unknown,
        _ => return Err(invalid_namespace_definition(capability, correlation_id)),
    };
    let definition = NamespaceDefinition::try_new(
        &request.namespace,
        &request.label,
        grains,
        &request.id_pattern,
        &request.normalization,
        licence_posture,
    )
    .map_err(|_| invalid_namespace_definition(capability, correlation_id))?;

    let kernel = state.kernel;
    let outcome = run_kernel(capability, correlation_id, move || {
        let access = kernel.authenticate_credential(AuthenticateCredentialQuery::new(
            correlation_id,
            capability,
            secret,
        ))?;
        kernel.register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
            correlation_id,
            access,
            definition,
        ))
    })
    .await?;

    Ok(Json(RegisterNamespaceResponse {
        namespace: outcome.namespace().to_string(),
        created: outcome.created(),
    }))
}

pub(crate) fn router() -> Router<LocalApiState> {
    Router::new()
        .route("/api/v1/records", post(create_record).get(list_records))
        .route("/api/v1/records/identifiers", post(attach_identifier))
        .route("/api/v1/namespaces", post(register_namespace))
}
