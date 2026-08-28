use crate::network_config::parse_origin;
use crate::outbound_http::{bounded_body, pinned_client, resolve_once};
use crate::setup::DesktopProblem;
use fasti_application::{
    authorize_outbound, NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy,
};
use fasti_contracts::HealthResponse;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const HEALTH_BODY_LIMIT: usize = 16 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EndpointConnectionInput {
    endpoint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ConnectionScheme {
    Http,
    Https,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EndpointConnectionStatus {
    endpoint: String,
    scheme: ConnectionScheme,
    status: String,
    version: String,
}

pub(crate) async fn test_connection(
    input: EndpointConnectionInput,
) -> Result<EndpointConnectionStatus, DesktopProblem> {
    let mut endpoint = parse_origin(&input.endpoint, "endpoint")?;
    let host = endpoint
        .host_str()
        .ok_or_else(|| DesktopProblem::configuration("The endpoint must include a host."))?
        .to_owned();
    let port = endpoint.port_or_known_default().ok_or_else(|| {
        DesktopProblem::configuration("The endpoint must include a valid HTTP or HTTPS port.")
    })?;
    let addresses = resolve_once(&host, port)
        .await
        .map_err(DesktopProblem::connection)?;
    let address_values = addresses.iter().map(|value| value.ip()).collect::<Vec<_>>();
    authorize_endpoint(&host, &address_values)?;

    let client =
        pinned_client(&host, &addresses, CONNECTION_TIMEOUT).map_err(DesktopProblem::connection)?;
    endpoint.set_path("/api/v1/health");
    let response = client
        .get(endpoint.clone())
        .send()
        .await
        .map_err(|_| DesktopProblem::connection("Fasti could not reach the endpoint."))?;
    if response.status() != reqwest::StatusCode::OK {
        return Err(DesktopProblem::connection(format!(
            "The endpoint returned HTTP {}.",
            response.status().as_u16()
        )));
    }
    let body = bounded_body(response, HEALTH_BODY_LIMIT)
        .await
        .map_err(DesktopProblem::connection)?;
    let health: HealthResponse = serde_json::from_slice(&body).map_err(|_| {
        DesktopProblem::invalid_response("The endpoint returned an invalid health response.")
    })?;
    if health.status != "healthy"
        || health.version.is_empty()
        || health.version.len() > 64
        || health.version.chars().any(char::is_control)
    {
        return Err(DesktopProblem::invalid_response(
            "The endpoint returned an invalid health response.",
        ));
    }

    endpoint.set_path("");
    Ok(EndpointConnectionStatus {
        endpoint: endpoint.as_str().trim_end_matches('/').to_owned(),
        scheme: if endpoint.scheme() == "https" {
            ConnectionScheme::Https
        } else {
            ConnectionScheme::Http
        },
        status: health.status,
        version: health.version,
    })
}

fn authorize_endpoint(host: &str, addresses: &[std::net::IpAddr]) -> Result<(), DesktopProblem> {
    let hosts = [host];
    authorize_outbound(
        OutboundAccessDeclaration {
            provider: "fasti-service",
            capabilities: &["system.health"],
            hosts: &hosts,
            networks: &[
                NetworkClass::Loopback,
                NetworkClass::Private,
                NetworkClass::Public,
            ],
        },
        &OutboundAccessPolicy::default(),
        "system.health",
        host,
        addresses,
    )
    .map_err(|denial| {
        DesktopProblem::connection(format!(
            "The endpoint safety policy denied the resolved {}.",
            denial.dimension()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_shape_rejects_port_zero_and_unsafe_plain_http() {
        assert!(parse_origin("http://localhost:0", "endpoint").is_err());
        assert!(parse_origin("http://fasti.internal", "endpoint").is_err());
        assert!(parse_origin("https://fasti.internal", "endpoint").is_ok());
    }

    #[test]
    fn node_endpoint_safety_is_separate_from_provider_narrowing() {
        assert!(authorize_endpoint(
            "fasti.internal",
            &["10.0.0.2".parse().expect("private endpoint")]
        )
        .is_ok());
        assert!(authorize_endpoint(
            "metadata.internal",
            &["169.254.169.254".parse().expect("link-local endpoint")]
        )
        .is_err());
    }
}
