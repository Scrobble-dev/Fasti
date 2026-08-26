use futures_util::StreamExt;
use reqwest::{Client, Response};
use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Semaphore;

const DNS_ANSWER_LIMIT: usize = 8;
const DNS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
// ponytail: one system lookup at a time; use a bounded pool if provider throughput requires it.
static DNS_LOOKUP_GATE: OnceLock<Arc<Semaphore>> = OnceLock::new();

pub(crate) async fn resolve_once(host: &str, port: u16) -> Result<Vec<SocketAddr>, &'static str> {
    if port == 0 {
        return Err("The endpoint port is invalid.");
    }
    if let Ok(address) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(address, port)]);
    }

    resolve_host_with(
        host.to_owned(),
        port,
        Arc::clone(DNS_LOOKUP_GATE.get_or_init(|| Arc::new(Semaphore::new(1)))),
        DNS_LOOKUP_TIMEOUT,
        system_resolve,
    )
    .await
}

fn system_resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, &'static str> {
    let mut unique = BTreeSet::new();
    for address in (host, port)
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
}

async fn resolve_host_with<F>(
    host: String,
    port: u16,
    gate: Arc<Semaphore>,
    timeout: Duration,
    resolver: F,
) -> Result<Vec<SocketAddr>, &'static str>
where
    F: FnOnce(&str, u16) -> Result<Vec<SocketAddr>, &'static str> + Send + 'static,
{
    let permit = gate
        .try_acquire_owned()
        .map_err(|_| "A host name lookup is already in progress.")?;
    let lookup = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        resolver(&host, port)
    });
    match tokio::time::timeout(timeout, lookup).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("The host name lookup did not complete."),
        Err(_) => Err("The host name lookup timed out."),
    }
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

    #[tokio::test(flavor = "current_thread")]
    async fn stalled_lookup_times_out_without_releasing_its_gate() {
        let gate = Arc::new(Semaphore::new(1));
        let (release, wait) = std::sync::mpsc::channel();
        let first = resolve_host_with(
            "stalled.internal".to_owned(),
            443,
            Arc::clone(&gate),
            Duration::from_millis(10),
            move |_, _| {
                wait.recv().expect("release stalled resolver");
                Ok(vec!["127.0.0.1:443".parse().expect("socket address")])
            },
        )
        .await;
        assert_eq!(first, Err("The host name lookup timed out."));

        let second = resolve_host_with(
            "second.internal".to_owned(),
            443,
            Arc::clone(&gate),
            Duration::from_secs(1),
            |_, _| panic!("a second resolver must not start"),
        )
        .await;
        assert_eq!(second, Err("A host name lookup is already in progress."));

        release.send(()).expect("release resolver");
        tokio::time::timeout(Duration::from_secs(1), async {
            while gate.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("resolver released gate");
    }
}
