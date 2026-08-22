use anyhow::{Context, Result};
use fasti_api::api_router;
use std::env;
use std::net::SocketAddr;
use tracing::info;

const DEFAULT_LISTEN: &str = "127.0.0.1:8420";

fn parse_listen_addr(value: &str) -> Result<SocketAddr> {
    value
        .parse()
        .with_context(|| format!("FASTI_LISTEN must be an IP:PORT socket address, got {value:?}"))
}

fn listen_addr() -> Result<SocketAddr> {
    let value = env::var("FASTI_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_owned());
    parse_listen_addr(&value)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let app = api_router();
    let addr = listen_addr()?;

    info!("Fasti daemon starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_an_explicit_socket_address() {
        let addr = parse_listen_addr("0.0.0.0:8420").expect("valid listen address");
        assert_eq!(addr, SocketAddr::from(([0, 0, 0, 0], 8420)));
    }

    #[test]
    fn rejects_a_bare_port() {
        let error = parse_listen_addr("8420").expect_err("bare ports are ambiguous");
        assert!(error.to_string().contains("IP:PORT"));
    }
}
