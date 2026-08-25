use crate::outbound_http::bounded_body;
use crate::setup::DesktopProblem;
use reqwest::{redirect::Policy, Client, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const MAX_HEALTH_RESPONSE_BYTES: usize = 16 * 1024;

#[derive(Debug, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConnectionStatus {
    endpoint: String,
    status: String,
    version: String,
}

pub(crate) async fn test(endpoint: String) -> Result<ConnectionStatus, DesktopProblem> {
    let origin = origin_url(&endpoint)?;
    let health_url = origin
        .join("/api/v1/health")
        .map_err(|_| invalid_endpoint("Fasti could not construct the health URL."))?;
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| connection_failed("Fasti could not initialize its HTTP client."))?;
    let response = client
        .get(health_url)
        .send()
        .await
        .map_err(|error| connection_failed(connection_error_detail(&error)))?;
    if !response.status().is_success() {
        return Err(connection_failed(format!(
            "The health request returned HTTP {}.",
            response.status().as_u16()
        )));
    }
    let body = bounded_body(response, MAX_HEALTH_RESPONSE_BYTES)
        .await
        .map_err(invalid_response)?;
    let health: HealthResponse = serde_json::from_slice(&body)
        .map_err(|_| invalid_response("The endpoint did not return a Fasti health response."))?;
    if health.status != "healthy" || health.version.trim().is_empty() {
        return Err(invalid_response(
            "The endpoint returned an invalid Fasti health status.",
        ));
    }

    Ok(ConnectionStatus {
        endpoint: origin.origin().ascii_serialization(),
        status: health.status,
        version: health.version,
    })
}

fn origin_url(value: &str) -> Result<Url, DesktopProblem> {
    let url = Url::parse(value).map_err(|_| invalid_endpoint("Enter a complete URL."))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host().is_none()
    {
        return Err(invalid_endpoint(
            "Use an HTTP or HTTPS origin without credentials, a path, a query, or a fragment.",
        ));
    }
    Ok(url)
}

fn connection_error_detail(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "The connection timed out after 5 seconds."
    } else if error.is_connect() {
        "Fasti could not connect to the endpoint. Check its host, port, and certificate trust."
    } else {
        "The endpoint request failed before Fasti received a response."
    }
}

fn invalid_endpoint(detail: impl Into<String>) -> DesktopProblem {
    DesktopProblem::connection(
        "invalid_endpoint",
        "The endpoint is not valid",
        detail,
        "Enter the node origin, such as http://127.0.0.1:8420 or https://fasti.internal.",
    )
}

fn connection_failed(detail: impl Into<String>) -> DesktopProblem {
    DesktopProblem::connection(
        "connection_failed",
        "Fasti could not reach the node",
        detail,
        "Check the endpoint, listener, reverse proxy, DNS, and certificate trust, then retry.",
    )
}

fn invalid_response(detail: impl Into<String>) -> DesktopProblem {
    DesktopProblem::connection(
        "invalid_health_response",
        "The endpoint is not a compatible Fasti node",
        detail,
        "Check the endpoint and Fasti version, then retry.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn accepts_origins_and_preserves_internal_hosts() {
        assert_eq!(
            origin_url("https://fasti.internal:9443")
                .expect("internal origin")
                .origin()
                .ascii_serialization(),
            "https://fasti.internal:9443"
        );
        assert!(origin_url("http://127.0.0.1:8420").is_ok());
        assert!(origin_url("http://[::1]:8420").is_ok());
    }

    #[test]
    fn rejects_values_that_are_not_origins() {
        for value in [
            "ftp://fasti.internal",
            "http://user:secret@fasti.internal",
            "http://fasti.internal/path",
            "http://fasti.internal?query=yes",
            "http://fasti.internal#fragment",
        ] {
            assert!(origin_url(value).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn verifies_a_bounded_fasti_health_response() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener");
        let address = listener.local_addr().expect("listener address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("health request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("read request");
            let body = r#"{"status":"healthy","version":"0.1.0"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("write response");
        });

        let status = tauri::async_runtime::block_on(test(format!("http://{address}")))
            .expect("healthy endpoint");
        assert_eq!(status.status, "healthy");
        assert_eq!(status.version, "0.1.0");
        assert_eq!(status.endpoint, format!("http://{address}"));
        server.join().expect("server thread");
    }

    #[test]
    fn rejects_an_oversized_health_response_before_reading_it() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener");
        let address = listener.local_addr().expect("listener address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("health request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("read request");
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                MAX_HEALTH_RESPONSE_BYTES + 1
            )
            .expect("write response");
        });

        let problem = tauri::async_runtime::block_on(test(format!("http://{address}")))
            .expect_err("oversized response");
        assert!(format!("{problem:?}").contains("invalid_health_response"));
        server.join().expect("server thread");
    }
}
