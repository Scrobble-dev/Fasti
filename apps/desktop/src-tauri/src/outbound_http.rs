use futures_util::StreamExt;
use reqwest::{Client, Response};
use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::time::Duration;

const DNS_ANSWER_LIMIT: usize = 8;

pub(crate) async fn resolve_once(host: &str, port: u16) -> Result<Vec<SocketAddr>, &'static str> {
    if port == 0 {
        return Err("The endpoint port is invalid.");
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(address, port)]);
    }
    let owned_host = host.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut unique = BTreeSet::new();
        for address in (owned_host.as_str(), port)
            .to_socket_addrs()
            .map_err(|_| "The host name could not be resolved.")?
        {
            unique.insert(address);
            if unique.len() > DNS_ANSWER_LIMIT {
                return Err("The host name returned too many addresses.");
            }
        }
        if unique.is_empty() {
            return Err("The host name did not return an address.");
        }
        Ok(unique.into_iter().collect())
    })
    .await
    .map_err(|_| "The host name lookup did not complete.")?
}

pub(crate) fn pinned_client(
    host: &str,
    addresses: &[SocketAddr],
    timeout: Duration,
) -> Result<Client, &'static str> {
    if addresses.is_empty() {
        return Err("The host name did not return an address.");
    }
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(timeout)
        .timeout(timeout)
        .resolve_to_addrs(host, addresses)
        .build()
        .map_err(|_| "The secure HTTP client could not be created.")
}

pub(crate) async fn bounded_body(
    response: Response,
    limit: usize,
) -> Result<Vec<u8>, &'static str> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("The response exceeded the size limit.");
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "The response body could not be read.")?;
        if body.len().saturating_add(chunk.len()) > limit {
            return Err("The response exceeded the size limit.");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_client_disables_ambient_routing() {
        let addresses = ["127.0.0.1:8420".parse().expect("socket address")];
        assert!(pinned_client("localhost", &addresses, Duration::from_secs(1)).is_ok());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn literal_addresses_do_not_require_dns() {
        assert_eq!(
            resolve_once("127.0.0.1", 8420).await.expect("literal IP"),
            ["127.0.0.1:8420".parse().expect("socket address")]
        );
        assert!(resolve_once("127.0.0.1", 0).await.is_err());
    }
}
