use anyhow::{Context, Result};
use fasti_api::{api_router, health_router, integration_router};
use fasti_store::SqliteKernel;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};

const DEFAULT_LISTEN: &str = "127.0.0.1:8420";
const DEFAULT_PORT_FALLBACK: &str = "fail";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortFallback {
    Auto,
    Fail,
}

fn parse_listen_addr(value: &str) -> Result<SocketAddr> {
    value
        .parse()
        .with_context(|| format!("listener must be an IP:PORT socket address, got {value:?}"))
}

fn listen_addr() -> Result<SocketAddr> {
    let value = env::var("FASTI_LISTEN").unwrap_or_else(|_| DEFAULT_LISTEN.to_owned());
    parse_listen_addr(&value).context("FASTI_LISTEN is invalid")
}

fn integration_listen_addr() -> Result<Option<SocketAddr>> {
    let Ok(value) = env::var("FASTI_INTEGRATION_LISTEN") else {
        return Ok(None);
    };
    anyhow::ensure!(
        !value.trim().is_empty(),
        "FASTI_INTEGRATION_LISTEN must not be empty when it is set"
    );
    parse_listen_addr(&value)
        .context("FASTI_INTEGRATION_LISTEN is invalid")
        .map(Some)
}

fn integration_transport_allowed(addr: SocketAddr, tls_terminated: bool) -> bool {
    addr.ip().is_loopback() || tls_terminated
}

fn integration_tls_terminated() -> Result<bool> {
    match env::var("FASTI_INTEGRATION_TLS_TERMINATED") {
        Err(env::VarError::NotPresent) => Ok(false),
        Ok(value) if value == "1" || value.eq_ignore_ascii_case("true") => Ok(true),
        Ok(value) if value == "0" || value.eq_ignore_ascii_case("false") => Ok(false),
        Ok(value) => anyhow::bail!(
            "FASTI_INTEGRATION_TLS_TERMINATED must be true/false or 1/0, got {value:?}"
        ),
        Err(error) => Err(error.into()),
    }
}

fn parse_port_fallback(value: &str) -> Result<PortFallback> {
    match value {
        "auto" => Ok(PortFallback::Auto),
        "fail" => Ok(PortFallback::Fail),
        _ => anyhow::bail!("FASTI_PORT_FALLBACK must be auto or fail, got {value:?}"),
    }
}

fn port_fallback() -> Result<PortFallback> {
    let value =
        env::var("FASTI_PORT_FALLBACK").unwrap_or_else(|_| DEFAULT_PORT_FALLBACK.to_owned());
    parse_port_fallback(&value)
}

async fn bind_listener(
    requested: SocketAddr,
    fallback: PortFallback,
) -> Result<(TcpListener, bool)> {
    match TcpListener::bind(requested).await {
        Ok(listener) => Ok((listener, false)),
        Err(error)
            if error.kind() == ErrorKind::AddrInUse
                && requested.ip().is_loopback()
                && requested.port() != 0
                && fallback == PortFallback::Auto =>
        {
            let fallback_addr = SocketAddr::new(requested.ip(), 0);
            let listener = TcpListener::bind(fallback_addr)
                .await
                .with_context(|| format!("failed to bind a fallback for {requested}"))?;
            Ok((listener, true))
        }
        Err(error) => Err(error).with_context(|| format!("failed to bind {requested}")),
    }
}

fn write_bound_addr(path: &Path, addr: SocketAddr) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("bound-address file setting must name a file")?;
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", process::id()));
    fs::write(&temporary, format!("{addr}\n"))
        .with_context(|| format!("failed to write {}", temporary.display()))?;
    fs::rename(&temporary, path)
        .with_context(|| format!("failed to publish {}", path.display()))?;
    Ok(())
}

fn publish_bound_addr(variable: &str, addr: SocketAddr) -> Result<()> {
    let Ok(path) = env::var(variable) else {
        return Ok(());
    };
    if path.trim().is_empty() {
        anyhow::bail!("{variable} must not be empty");
    }
    write_bound_addr(Path::new(&path), addr)
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

    let requested_addr = listen_addr()?;
    let remote_health_only = uses_remote_health_surface(requested_addr);
    let (listener, used_fallback) = bind_listener(requested_addr, port_fallback()?).await?;
    let addr = listener.local_addr()?;

    let configured_data_root = data_root()?;
    let kernel = configured_data_root
        .as_ref()
        .map(|data_root| {
            SqliteKernel::open(data_root)
                .with_context(|| format!("failed to open Fasti data root {data_root:?}"))
                .map(Arc::new)
        })
        .transpose()?;

    let app = if remote_health_only {
        info!("Fasti health-only listener starting on http://{}", addr);
        health_router()
    } else if let (Some(data_root), Some(kernel)) = (&configured_data_root, &kernel) {
        info!(
            "Fasti durable local listener starting on http://{} with data root {:?}",
            addr, data_root
        );
        api_router(kernel.clone(), addr, data_root)
    } else {
        warn!("Fasti local capability routes are disabled because FASTI_DATA_ROOT is not set");
        health_router()
    };

    publish_bound_addr("FASTI_BOUND_ADDR_FILE", addr)?;
    if used_fallback {
        info!(requested = %requested_addr, actual = %addr, "preferred loopback port was occupied; Fasti selected an available port");
    }

    let integration_task = if let Some(requested) = integration_listen_addr()? {
        let kernel = kernel
            .as_ref()
            .context("FASTI_INTEGRATION_LISTEN requires FASTI_DATA_ROOT")?
            .clone();
        let tls_terminated = integration_tls_terminated()?;
        anyhow::ensure!(
            integration_transport_allowed(requested, tls_terminated),
            "non-loopback FASTI_INTEGRATION_LISTEN requires FASTI_INTEGRATION_TLS_TERMINATED=true and a trusted TLS reverse proxy"
        );
        let (integration_listener, used_integration_fallback) =
            bind_listener(requested, PortFallback::Fail).await?;
        debug_assert!(!used_integration_fallback);
        let integration_addr = integration_listener.local_addr()?;
        publish_bound_addr("FASTI_INTEGRATION_BOUND_ADDR_FILE", integration_addr)?;
        info!(
            address = %integration_addr,
            tls_terminated,
            "Fasti isolated integration listener starting"
        );
        let integration_app = integration_router(kernel);
        Some(tokio::spawn(async move {
            axum::serve(integration_listener, integration_app).await
        }))
    } else {
        None
    };

    let primary_result = axum::serve(listener, app).await;
    if let Some(task) = integration_task {
        task.abort();
    }
    primary_result?;
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
    fn integration_listener_requires_a_protected_transport_off_loopback() {
        let loopback = SocketAddr::from(([127, 0, 0, 1], 8421));
        let private = SocketAddr::from(([192, 168, 1, 5], 8421));
        assert!(integration_transport_allowed(loopback, false));
        assert!(!integration_transport_allowed(private, false));
        assert!(integration_transport_allowed(private, true));
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

    #[test]
    fn validates_port_fallback_mode() {
        assert_eq!(
            parse_port_fallback("auto").expect("automatic fallback"),
            PortFallback::Auto
        );
        assert_eq!(
            parse_port_fallback("fail").expect("fail-closed mode"),
            PortFallback::Fail
        );
        assert!(parse_port_fallback("random").is_err());
    }

    #[tokio::test]
    async fn occupied_loopback_port_uses_an_os_assigned_fallback() {
        let occupied = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("reserve loopback port");
        let requested = occupied.local_addr().expect("reserved address");

        let (fallback, used_fallback) = bind_listener(requested, PortFallback::Auto)
            .await
            .expect("fallback listener");
        let actual = fallback.local_addr().expect("fallback address");

        assert!(used_fallback);
        assert_eq!(actual.ip(), requested.ip());
        assert_ne!(actual.port(), requested.port());
    }

    #[tokio::test]
    async fn fail_mode_does_not_move_an_occupied_port() {
        let occupied = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("reserve loopback port");
        let requested = occupied.local_addr().expect("reserved address");

        assert!(bind_listener(requested, PortFallback::Fail).await.is_err());
    }
}
