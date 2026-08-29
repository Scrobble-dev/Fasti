use anyhow::{Context, Result};
use fasti_api::{
    api_router, ensure_development_test_account, health_router, integration_router,
    remote_api_router, with_static_fallback,
};
use fasti_store::SqliteKernel;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
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

fn is_remote_listener(addr: SocketAddr) -> bool {
    !addr.ip().is_loopback()
}

fn parse_external_bind_ip(value: Option<&str>) -> Result<Option<IpAddr>> {
    let Some(value) = value else {
        return Ok(None);
    };
    anyhow::ensure!(
        !value.is_empty(),
        "FASTI_EXTERNAL_BIND_IP must be a loopback IP address when it is set"
    );
    value
        .parse()
        .map(Some)
        .with_context(|| format!("FASTI_EXTERNAL_BIND_IP must be an IP address, got {value:?}"))
}

fn external_bind_ip() -> Result<Option<IpAddr>> {
    parse_external_bind_ip(env::var("FASTI_EXTERNAL_BIND_IP").ok().as_deref())
}

fn has_container_boundary() -> bool {
    Path::new("/run/.containerenv").is_file() || Path::new("/.dockerenv").is_file()
}

fn local_api_exposure_addr(
    listen_addr: SocketAddr,
    external_bind_ip: Option<IpAddr>,
    container_boundary: bool,
) -> Result<Option<SocketAddr>> {
    let Some(external_bind_ip) = external_bind_ip else {
        return Ok((!is_remote_listener(listen_addr)).then_some(listen_addr));
    };
    anyhow::ensure!(
        container_boundary,
        "FASTI_EXTERNAL_BIND_IP requires a container isolation boundary"
    );
    anyhow::ensure!(
        listen_addr.ip().is_unspecified(),
        "FASTI_EXTERNAL_BIND_IP is valid only when FASTI_LISTEN uses a wildcard address"
    );
    anyhow::ensure!(
        external_bind_ip.is_loopback(),
        "FASTI_EXTERNAL_BIND_IP must be a loopback IP address"
    );
    Ok(Some(SocketAddr::new(external_bind_ip, listen_addr.port())))
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

/// `FASTI_STATIC_DIR` reuses the same "unset -> None, empty -> error" shape
/// as `FASTI_DATA_ROOT` -- see `parse_data_root`. It names a pre-built web
/// UI bundle (e.g. `apps/web`'s `vite build` output) for fastid to serve
/// directly. This is optional and orthogonal to the durable data root: a
/// health-only or remote listener can still serve the UI.
fn parse_static_dir(value: Option<OsString>) -> Result<Option<PathBuf>> {
    let Some(value) = value else {
        return Ok(None);
    };
    anyhow::ensure!(
        !value.is_empty(),
        "FASTI_STATIC_DIR must name a directory when it is set"
    );
    Ok(Some(PathBuf::from(value)))
}

fn static_dir() -> Result<Option<PathBuf>> {
    parse_static_dir(env::var_os("FASTI_STATIC_DIR"))
}

fn parse_boolean(name: &str, value: Option<String>, default: bool) -> Result<bool> {
    match value.as_deref() {
        None => Ok(default),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(value) => anyhow::bail!("{name} must be true or false, got {value:?}"),
    }
}

fn remote_proxy_is_trusted() -> Result<bool> {
    parse_boolean(
        "FASTI_REMOTE_TRUSTED_PROXY",
        env::var("FASTI_REMOTE_TRUSTED_PROXY").ok(),
        false,
    )
}

fn parse_development_test_account(value: Option<String>, remote_listener: bool) -> Result<bool> {
    let enabled = parse_boolean("FASTI_DEVELOPMENT_TEST_ACCOUNT", value, false)?;
    anyhow::ensure!(
        !enabled || !remote_listener,
        "FASTI_DEVELOPMENT_TEST_ACCOUNT is allowed only on a loopback durable listener"
    );
    Ok(enabled)
}

fn development_test_account_enabled(remote_listener: bool) -> Result<bool> {
    parse_development_test_account(
        env::var("FASTI_DEVELOPMENT_TEST_ACCOUNT").ok(),
        remote_listener,
    )
}

fn require_https_public_url() -> Result<()> {
    let value = env::var("FASTI_PUBLIC_URL")
        .context("FASTI_PUBLIC_URL is required for a remote durable listener")?;
    let uri = value
        .parse::<axum::http::Uri>()
        .context("FASTI_PUBLIC_URL must be an absolute HTTPS URL")?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https") && uri.authority().is_some(),
        "FASTI_PUBLIC_URL must be an absolute HTTPS URL"
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let requested_addr = listen_addr()?;
    let local_api_addr = local_api_exposure_addr(
        requested_addr,
        external_bind_ip()?,
        has_container_boundary(),
    )?;
    let remote_listener = local_api_addr.is_none();
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

    let app = match (&configured_data_root, &kernel) {
        (Some(data_root), Some(kernel)) => {
            if remote_listener {
                anyhow::ensure!(
                    remote_proxy_is_trusted()?,
                    "remote durable routes require FASTI_REMOTE_TRUSTED_PROXY=true"
                );
                require_https_public_url()?;
            }
            let seed_development_account = development_test_account_enabled(remote_listener)?;
            if seed_development_account {
                ensure_development_test_account(kernel.as_ref()).map_err(|problem| {
                    anyhow::anyhow!(
                        "failed to seed the one-time development browser account: {}",
                        problem.message()
                    )
                })?;
            }
            if remote_listener {
                info!(
                    "Fasti durable remote listener starting behind the trusted HTTPS proxy on http://{} with data root {:?}",
                    addr, data_root
                );
                remote_api_router(kernel.clone(), addr, data_root)
            } else {
                info!(
                    "Fasti durable local listener starting on http://{} with data root {:?}",
                    addr, data_root
                );
                api_router(
                    kernel.clone(),
                    local_api_addr.expect("configured durable local routes require local exposure"),
                    data_root,
                )
            }
        }
        _ => {
            if remote_listener {
                info!("Fasti health-only listener starting on http://{}", addr);
            } else {
                warn!(
                    "Fasti local capability routes are disabled because FASTI_DATA_ROOT is not set"
                );
            }
            health_router()
        }
    };
    let app = with_static_fallback(app, static_dir()?.as_deref());

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
        if !requested.ip().is_loopback() {
            anyhow::ensure!(
                remote_proxy_is_trusted()?,
                "a non-loopback integration listener requires FASTI_REMOTE_TRUSTED_PROXY=true"
            );
        }
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

    match integration_task {
        Some(task) => {
            let abort_handle = task.abort_handle();
            tokio::select! {
                            result = axum::serve(listener, app) => {
                                abort_handle.abort();
                                result?;
                            }
                            joined = task => {
                                match joined {
            Ok(Ok(())) => return Err(anyhow::anyhow!("Fasti isolated integration listener exited unexpectedly")),
                                    Ok(Err(err)) => return Err(err).context("Fasti isolated integration listener failed"),
                                    Err(join_err) => return Err(join_err).context("Fasti isolated integration listener task panicked"),
                                }
                            }
                        }
        }
        None => axum::serve(listener, app).await?,
    }
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
    fn identifies_non_loopback_listeners() {
        assert!(!is_remote_listener(SocketAddr::from((
            [127, 0, 0, 1],
            8420
        ))));
        assert!(!is_remote_listener(
            "[::1]:8420".parse().expect("IPv6 loopback")
        ));
        assert!(is_remote_listener(SocketAddr::from(([0, 0, 0, 0], 8420))));
        assert!(is_remote_listener(SocketAddr::from((
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
    fn permits_durable_routes_through_an_explicit_loopback_port_forward() {
        let wildcard = SocketAddr::from(([0, 0, 0, 0], 8420));
        let loopback = IpAddr::from([127, 0, 0, 1]);

        assert_eq!(
            local_api_exposure_addr(wildcard, Some(loopback), true)
                .expect("trusted container port forward"),
            Some(SocketAddr::new(loopback, 8420))
        );
        assert_eq!(
            local_api_exposure_addr(wildcard, None, false).expect("health-only wildcard"),
            None
        );
        assert!(local_api_exposure_addr(wildcard, Some(loopback), false).is_err());
        assert!(
            local_api_exposure_addr(wildcard, Some(IpAddr::from([192, 0, 2, 1])), true).is_err()
        );
        assert!(local_api_exposure_addr(
            SocketAddr::from(([192, 0, 2, 1], 8420)),
            Some(loopback),
            true,
        )
        .is_err());
    }

    #[test]
    fn validates_the_external_bind_ip() {
        assert_eq!(
            parse_external_bind_ip(Some("127.0.0.1")).expect("loopback IP"),
            Some(IpAddr::from([127, 0, 0, 1]))
        );
        assert!(parse_external_bind_ip(Some("")).is_err());
        assert!(parse_external_bind_ip(Some("localhost")).is_err());
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
    fn static_dir_is_explicit_and_optional() {
        assert_eq!(parse_static_dir(None).expect("absent static dir"), None);
        assert!(parse_static_dir(Some(OsString::new())).is_err());
        assert_eq!(
            parse_static_dir(Some(OsString::from("/srv/web"))).expect("static dir"),
            Some(PathBuf::from("/srv/web"))
        );
    }

    #[test]
    fn remote_security_flags_are_strict_booleans() {
        assert!(parse_boolean("TEST", None, true).expect("default"));
        assert!(!parse_boolean("TEST", Some("false".to_owned()), true).expect("false"));
        assert!(parse_boolean("TEST", Some("yes".to_owned()), false).is_err());
    }

    #[test]
    fn development_account_is_explicit_and_loopback_only() {
        assert!(!parse_development_test_account(None, false).expect("default off"));
        assert!(
            parse_development_test_account(Some("true".to_owned()), false)
                .expect("explicit loopback development account")
        );
        assert!(
            !parse_development_test_account(Some("false".to_owned()), true)
                .expect("remote listener without development account")
        );
        assert!(parse_development_test_account(Some("true".to_owned()), true).is_err());
        assert!(parse_development_test_account(Some("yes".to_owned()), false).is_err());
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
