//! Loopback-only executable host for black-box B1 contract tests.
//!
//! This binary is compiled only with `conformance-fixture`; `fastid` does not
//! depend on it. It accepts one explicit `127.0.0.1:PORT` argument, or the same
//! value through `FASTI_CONFORMANCE_ADDR` when no argument is supplied.

use fasti_api::b1_conformance_router;
use std::{
    env,
    error::Error,
    io::{self, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    process::ExitCode,
};

const ADDRESS_ENV: &str = "FASTI_CONFORMANCE_ADDR";
const USAGE: &str = "B1 loopback-only conformance fixture (nondurable; not a production server)\n\nUsage:\n  b1-conformance-server 127.0.0.1:PORT\n\nEnvironment:\n  FASTI_CONFORMANCE_ADDR  Used when no address argument is supplied\n\nOptions:\n  -h, --help              Print this help";

#[tokio::main]
async fn main() -> ExitCode {
    let requested = match requested_invocation(env::args().skip(1), env::var(ADDRESS_ENV).ok()) {
        Ok(Invocation::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Ok(Invocation::Serve(address)) => address,
        Err(error) => {
            eprintln!("error: {error}");
            eprintln!("help: run `b1-conformance-server --help`");
            return ExitCode::from(2);
        }
    };

    match run(requested).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: conformance fixture failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(requested: SocketAddr) -> Result<(), Box<dyn Error>> {
    let listener = tokio::net::TcpListener::bind(requested).await?;
    let bound = listener.local_addr()?;
    debug_assert_eq!(bound.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));

    let readiness = serde_json::json!({
        "event": "ready",
        "address": bound.to_string(),
        "availability": "fixture_only",
        "durability": "none"
    });
    println!("{readiness}");
    io::stdout().flush()?;

    axum::serve(listener, b1_conformance_router())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn requested_invocation(
    arguments: impl IntoIterator<Item = String>,
    environment: Option<String>,
) -> Result<Invocation, AddressError> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let raw = match arguments.as_slice() {
        [] => environment.ok_or(AddressError::Missing)?,
        [flag] if flag == "-h" || flag == "--help" => return Ok(Invocation::Help),
        [address] => address.clone(),
        _ => return Err(AddressError::TooManyArguments),
    };
    let address = raw
        .parse::<SocketAddr>()
        .map_err(|_| AddressError::Invalid)?;
    if address.ip() != IpAddr::V4(Ipv4Addr::LOCALHOST) {
        return Err(AddressError::NotLoopbackV4);
    }
    Ok(Invocation::Serve(address))
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("failed to install Ctrl-C handler: {error}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressError {
    Missing,
    TooManyArguments,
    Invalid,
    NotLoopbackV4,
}

impl std::fmt::Display for AddressError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "supply explicit 127.0.0.1:PORT argument or FASTI_CONFORMANCE_ADDR",
            Self::TooManyArguments => "expected exactly one 127.0.0.1:PORT argument",
            Self::Invalid => "conformance address must be a valid 127.0.0.1:PORT socket address",
            Self::NotLoopbackV4 => "conformance server may bind only 127.0.0.1",
        })
    }
}

impl Error for AddressError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Invocation {
    Help,
    Serve(SocketAddr),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_argument_or_environment_must_be_ipv4_loopback() {
        assert_eq!(
            requested_invocation(["127.0.0.1:0".to_owned()], None).expect("explicit loopback"),
            Invocation::Serve("127.0.0.1:0".parse().expect("valid socket"))
        );
        assert_eq!(
            requested_invocation([], Some("127.0.0.1:43127".to_owned()))
                .expect("environment loopback"),
            Invocation::Serve("127.0.0.1:43127".parse().expect("valid socket"))
        );
        assert_eq!(
            requested_invocation(["--help".to_owned()], None),
            Ok(Invocation::Help)
        );
        for invalid in [
            "0.0.0.0:43127",
            "[::1]:43127",
            "localhost:43127",
            "127.0.0.2:43127",
        ] {
            assert!(requested_invocation([invalid.to_owned()], None).is_err());
        }
        assert_eq!(requested_invocation([], None), Err(AddressError::Missing));
        assert_eq!(
            requested_invocation(["127.0.0.1:1".to_owned(), "127.0.0.1:2".to_owned()], None),
            Err(AddressError::TooManyArguments)
        );
    }
}
