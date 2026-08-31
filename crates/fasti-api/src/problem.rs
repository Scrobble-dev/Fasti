use axum::{
    extract::rejection::JsonRejection,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use fasti_application::{CapabilityKey, FastiProblem, ProblemCode, Violation};
use fasti_contracts::{public_capability_id, ProblemDetails};
use fasti_domain::RequestCorrelationId;

const DOCUMENTATION_BASE: &str = "https://fasti.scrobble.dev";

pub(crate) struct HttpProblem {
    status: StatusCode,
    body: Box<ProblemDetails>,
}

impl HttpProblem {
    pub(crate) fn code(&self) -> &str {
        &self.body.code
    }
}

impl IntoResponse for HttpProblem {
    fn into_response(self) -> Response {
        (
            self.status,
            [
                (header::CONTENT_TYPE, "application/problem+json"),
                (header::CACHE_CONTROL, "private, no-store"),
            ],
            Json(self.body),
        )
            .into_response()
    }
}

pub(crate) fn json_rejection(
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

pub(crate) fn application_problem(problem: Box<FastiProblem>) -> HttpProblem {
    let capability_id = public_capability_id(problem.capability());
    let dto = ProblemDetails::from_application(&problem, capability_id, DOCUMENTATION_BASE);
    let status = StatusCode::from_u16(dto.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    HttpProblem {
        status,
        body: Box::new(dto),
    }
}
