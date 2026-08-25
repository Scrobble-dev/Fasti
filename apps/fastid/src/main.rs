use anyhow::{Context, Result};
use fasti_api::{api_router, health_router};
use fasti_store::SqliteKernel;
use std::env;
use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info, warn};

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

fn uses_remote_health_surface(addr: SocketAddr) -> bool {
    !addr.ip().is_loopback()
}

fn parse_data_root(value: Option<OsString>) -> Result<Option<PathBuf>> {
    let Some(value) = value else {
        return Ok(None);
    };
    anyhow::ensure!(
        !value.is_empty(),
        "FASTI_DATA_ROOT must name a directory when it is set"
    );
    Ok(Some(PathBuf::from(value)))
}

fn data_root() -> Result<Option<PathBuf>> {
    parse_data_root(env::var_os("FASTI_DATA_ROOT"))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let addr = listen_addr()?;
    let remote_health_only = uses_remote_health_surface(addr);
    if remote_health_only {
        info!("Fasti health-only listener starting on http://{}", addr);
    }

    let configured_data_root = if remote_health_only {
        None
    } else {
        data_root()?
    };
    let app = match configured_data_root {
        Some(data_root) => {
            let kernel = Arc::new(
                SqliteKernel::open(&data_root)
                    .with_context(|| format!("failed to open Fasti data root {data_root:?}"))?,
            );
            info!(
                "Fasti durable local listener starting on http://{} with data root {:?}",
                addr, data_root
            );
            api_router(kernel, addr, &data_root)
        }
        None => {
            if !remote_health_only {
                warn!(
                    "Fasti local capability routes are disabled because FASTI_DATA_ROOT is not set"
                );
            }
            health_router()
        }
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_explicit_socket_addresses() {
        let local = parse_listen_addr("127.0.0.1:8420").expect("IPv4 loopback address");
        let remote = parse_listen_addr("0.0.0.0:8420").expect("explicit wildcard address");

        assert_eq!(local, SocketAddr::from(([127, 0, 0, 1], 8420)));
        assert_eq!(remote, SocketAddr::from(([0, 0, 0, 0], 8420)));
        assert!(parse_listen_addr("[::1]:8420").is_ok());
    }

    #[test]
    fn selects_the_health_only_surface_for_non_loopback_listeners() {
        assert!(!uses_remote_health_surface(SocketAddr::from((
            [127, 0, 0, 1],
            8420
        ))));
        assert!(!uses_remote_health_surface(
            "[::1]:8420".parse().expect("IPv6 loopback")
        ));
        assert!(uses_remote_health_surface(SocketAddr::from((
            [0, 0, 0, 0],
            8420
        ))));
        assert!(uses_remote_health_surface(SocketAddr::from((
            [192, 0, 2, 10],
            8420
        ))));
    }

    #[test]
    fn rejects_a_bare_port() {
        let error = parse_listen_addr("8420").expect_err("bare ports are ambiguous");
        assert!(error.to_string().contains("IP:PORT"));
    }

    #[test]
    fn data_root_is_explicit_and_never_defaults_to_the_working_directory() {
        assert_eq!(parse_data_root(None).expect("absent data root"), None);
        assert!(parse_data_root(Some(OsString::new())).is_err());
        assert_eq!(
            parse_data_root(Some(OsString::from("/tmp/fasti-data"))).expect("data root"),
            Some(PathBuf::from("/tmp/fasti-data"))
        );
    }
}
