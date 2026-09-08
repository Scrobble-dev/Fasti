use crate::ProviderRuntimeError;
#[cfg(feature = "tmdb-smoke-fixture")]
use fasti_application::NetworkClass;
use fasti_application::{authorize_outbound, OutboundAccessDeclaration, OutboundAccessPolicy};
use reqwest::{Client, Response};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

// TMDB's dual-stack image CDN returns four IPv4 plus eight IPv6 addresses.
// Bound the complete answer set; never truncate before authorizing every address.
const DNS_ANSWER_LIMIT: usize = 16;
const DNS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_CONCURRENCY_LIMIT: usize = 4;
#[cfg(feature = "tmdb-smoke-fixture")]
const SMOKE_CA_PEM_LIMIT: usize = 16 * 1024;
// ponytail: one system lookup at a time; use a bounded pool if provider throughput requires it.
static DNS_LOOKUP_GATE: OnceLock<Arc<Semaphore>> = OnceLock::new();

pub async fn resolve_once(host: &str, port: u16) -> Result<Vec<SocketAddr>, &'static str> {
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
    collect_addresses(
        (host, port)
            .to_socket_addrs()
            .map_err(|_| "The host name could not be resolved.")?,
    )
}

fn collect_addresses(
    addresses: impl IntoIterator<Item = SocketAddr>,
) -> Result<Vec<SocketAddr>, &'static str> {
    let mut unique = BTreeSet::new();
    for address in addresses {
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
    let deadline = Instant::now() + timeout;
    let permit = tokio::time::timeout_at(deadline, gate.acquire_owned())
        .await
        .map_err(|_| "The host name lookup timed out.")?
        .map_err(|_| "The host name lookup gate is unavailable.")?;
    let lookup = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        resolver(&host, port)
    });
    match tokio::time::timeout_at(deadline, lookup).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("The host name lookup did not complete."),
        Err(_) => Err("The host name lookup timed out."),
    }
}

pub fn pinned_client(
    host: &str,
    addresses: &[SocketAddr],
    timeout: Duration,
) -> Result<Client, &'static str> {
    pinned_client_with_timeouts(host, addresses, timeout, timeout)
}

pub fn pinned_client_with_timeouts(
    host: &str,
    addresses: &[SocketAddr],
    connect_timeout: Duration,
    total_timeout: Duration,
) -> Result<Client, &'static str> {
    pinned_client_builder(host, addresses, connect_timeout, total_timeout)?
        .build()
        .map_err(|_| "The secure HTTP client could not be created.")
}

fn pinned_client_builder(
    host: &str,
    addresses: &[SocketAddr],
    connect_timeout: Duration,
    total_timeout: Duration,
) -> Result<reqwest::ClientBuilder, &'static str> {
    if addresses.is_empty() {
        return Err("The host name did not return an address.");
    }
    Ok(Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(connect_timeout)
        .timeout(total_timeout)
        .resolve_to_addrs(host, addresses))
}

#[cfg(feature = "tmdb-smoke-fixture")]
fn smoke_fixture_client(
    host: &str,
    address: SocketAddr,
    timeout: Duration,
    certificate: reqwest::Certificate,
) -> Result<Client, &'static str> {
    pinned_client_builder(host, &[address], timeout, timeout)?
        .tls_certs_only([certificate])
        .build()
        .map_err(|_| "The secure HTTP client could not be created.")
}

pub async fn bounded_body(mut response: Response, limit: usize) -> Result<Vec<u8>, &'static str> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err("The response exceeded the size limit.");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The response body could not be read.")?
    {
        if body.len().saturating_add(chunk.len()) > limit {
            return Err("The response exceeded the size limit.");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[derive(Clone)]
pub struct GovernedTransport {
    requests: Arc<Semaphore>,
    timeout: Duration,
    #[cfg(feature = "tmdb-smoke-fixture")]
    smoke_fixture: Option<SmokeFixture>,
}

#[cfg(feature = "tmdb-smoke-fixture")]
#[derive(Clone)]
struct SmokeFixture {
    production: OutboundAccessDeclaration<'static>,
    loopback: OutboundAccessDeclaration<'static>,
    address: SocketAddr,
    certificate: reqwest::Certificate,
}

impl Default for GovernedTransport {
    fn default() -> Self {
        Self::new(Duration::from_secs(15))
    }
}

impl GovernedTransport {
    pub fn new(timeout: Duration) -> Self {
        Self {
            requests: Arc::new(Semaphore::new(REQUEST_CONCURRENCY_LIMIT)),
            timeout,
            #[cfg(feature = "tmdb-smoke-fixture")]
            smoke_fixture: None,
        }
    }

    #[cfg(feature = "tmdb-smoke-fixture")]
    pub(crate) fn tmdb_smoke_fixture(
        production: OutboundAccessDeclaration<'static>,
        loopback: OutboundAccessDeclaration<'static>,
        address: SocketAddr,
        ca_pem: &[u8],
    ) -> Result<Self, ProviderRuntimeError> {
        if address.ip() != IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) || address.port() == 0 {
            return Err(ProviderRuntimeError::configuration(
                "The TMDB smoke fixture address must be 127.0.0.1 with a nonzero port.",
            ));
        }
        if production.provider != loopback.provider
            || production.capabilities != loopback.capabilities
            || production.hosts.len() != 1
            || production.hosts != loopback.hosts
            || production.networks != [NetworkClass::Public]
            || loopback.networks != [NetworkClass::Loopback]
        {
            return Err(ProviderRuntimeError::configuration(
                "The TMDB smoke fixture declaration is invalid.",
            ));
        }
        if ca_pem.is_empty() || ca_pem.len() > SMOKE_CA_PEM_LIMIT {
            return Err(ProviderRuntimeError::configuration(
                "The TMDB smoke fixture CA is invalid.",
            ));
        }
        let mut certificates = reqwest::Certificate::from_pem_bundle(ca_pem).map_err(|_| {
            ProviderRuntimeError::configuration("The TMDB smoke fixture CA is invalid.")
        })?;
        if certificates.len() != 1 {
            return Err(ProviderRuntimeError::configuration(
                "The TMDB smoke fixture requires exactly one CA certificate.",
            ));
        }
        Ok(Self {
            requests: Arc::new(Semaphore::new(REQUEST_CONCURRENCY_LIMIT)),
            timeout: Duration::from_secs(15),
            smoke_fixture: Some(SmokeFixture {
                production,
                loopback,
                address,
                certificate: certificates.pop().expect("one checked certificate"),
            }),
        })
    }

    pub async fn authorize(
        &self,
        declaration: OutboundAccessDeclaration<'static>,
        policy: &OutboundAccessPolicy,
        capability: &'static str,
        endpoint: &reqwest::Url,
    ) -> Result<AuthorizedClient, ProviderRuntimeError> {
        let origin = Origin::try_from(endpoint).map_err(ProviderRuntimeError::configuration)?;
        let permit = tokio::time::timeout(self.timeout, Arc::clone(&self.requests).acquire_owned())
            .await
            .map_err(|_| ProviderRuntimeError::network("The provider request queue timed out."))?
            .map_err(|_| {
                ProviderRuntimeError::network("The provider request queue is unavailable.")
            })?;
        #[cfg(feature = "tmdb-smoke-fixture")]
        let fixture = self.smoke_fixture.as_ref();
        #[cfg(feature = "tmdb-smoke-fixture")]
        let (effective_declaration, addresses) = if let Some(fixture) = fixture {
            if !same_declaration(declaration, fixture.production)
                || endpoint.port().is_some()
                || origin.port != 443
                || !fixture.production.hosts.contains(&origin.host.as_str())
            {
                return Err(ProviderRuntimeError::configuration(
                    "The TMDB smoke fixture rejected an unrelated provider endpoint.",
                ));
            }
            (fixture.loopback, vec![fixture.address])
        } else {
            (
                declaration,
                resolve_once(&origin.host, origin.port)
                    .await
                    .map_err(ProviderRuntimeError::network)?,
            )
        };
        #[cfg(not(feature = "tmdb-smoke-fixture"))]
        let (effective_declaration, addresses) = (
            declaration,
            resolve_once(&origin.host, origin.port)
                .await
                .map_err(ProviderRuntimeError::network)?,
        );
        let address_values = addresses.iter().map(|value| value.ip()).collect::<Vec<_>>();
        authorize_outbound(
            effective_declaration,
            policy,
            capability,
            &origin.host,
            &address_values,
        )
        .map_err(|denial| {
            ProviderRuntimeError::configuration(format!(
                "The outbound policy denied the {} {}.",
                declaration.provider,
                denial.dimension()
            ))
        })?;
        #[cfg(feature = "tmdb-smoke-fixture")]
        let client = if let Some(fixture) = fixture {
            smoke_fixture_client(
                &origin.host,
                fixture.address,
                self.timeout,
                fixture.certificate.clone(),
            )
        } else {
            pinned_client(&origin.host, &addresses, self.timeout)
        }
        .map_err(ProviderRuntimeError::configuration)?;
        #[cfg(not(feature = "tmdb-smoke-fixture"))]
        let client = pinned_client(&origin.host, &addresses, self.timeout)
            .map_err(ProviderRuntimeError::configuration)?;
        Ok(AuthorizedClient {
            client,
            origin,
            configuration_digest: configuration_digest(declaration.provider, capability, endpoint)
                .map_err(ProviderRuntimeError::configuration)?,
            _permit: permit,
        })
    }
}

#[cfg(feature = "tmdb-smoke-fixture")]
fn same_declaration(
    left: OutboundAccessDeclaration<'static>,
    right: OutboundAccessDeclaration<'static>,
) -> bool {
    left.provider == right.provider
        && left.capabilities == right.capabilities
        && left.hosts == right.hosts
        && left.networks == right.networks
}

pub struct AuthorizedClient {
    client: Client,
    origin: Origin,
    configuration_digest: String,
    _permit: OwnedSemaphorePermit,
}

impl AuthorizedClient {
    pub fn configuration_digest(&self) -> &str {
        &self.configuration_digest
    }

    pub fn get(&self, url: reqwest::Url) -> Result<reqwest::RequestBuilder, String> {
        if Origin::try_from(&url)? != self.origin {
            return Err("The provider request origin changed after authorization.".to_owned());
        }
        Ok(self.client.get(url))
    }
}

#[cfg(test)]
pub(crate) fn test_authorized_client(
    provider: &str,
    capability: &str,
    endpoint: &str,
) -> AuthorizedClient {
    let url = reqwest::Url::parse(endpoint).expect("test provider URL");
    let origin = Origin::try_from(&url).expect("test provider origin");
    let addresses = ["127.0.0.1:443".parse().expect("test socket address")];
    AuthorizedClient {
        client: pinned_client(&origin.host, &addresses, Duration::from_secs(1))
            .expect("test pinned client"),
        origin,
        configuration_digest: configuration_digest(provider, capability, &url)
            .expect("test configuration digest"),
        _permit: Arc::new(Semaphore::new(1))
            .try_acquire_owned()
            .expect("test permit"),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Origin {
    scheme: &'static str,
    host: String,
    port: u16,
}

impl TryFrom<&reqwest::Url> for Origin {
    type Error = String;

    fn try_from(value: &reqwest::Url) -> Result<Self, Self::Error> {
        if value.scheme() != "https"
            || !value.username().is_empty()
            || value.password().is_some()
            || value.fragment().is_some()
        {
            return Err("The provider endpoint origin is unsafe.".to_owned());
        }
        let host = value
            .host_str()
            .ok_or_else(|| "The provider endpoint has no host.".to_owned())?
            .to_ascii_lowercase();
        Ok(Self {
            scheme: "https",
            host,
            port: value.port_or_known_default().unwrap_or(443),
        })
    }
}

pub fn configuration_digest(
    provider: &str,
    capability: &str,
    endpoint: &reqwest::Url,
) -> Result<String, String> {
    let origin = Origin::try_from(endpoint)?;
    let mut digest = Sha256::new();
    for part in [provider, capability, origin.scheme, origin.host.as_str()] {
        digest.update(part.as_bytes());
        digest.update([0]);
    }
    digest.update(origin.port.to_be_bytes());
    let bytes = digest.finalize();
    let mut value = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "tmdb-smoke-fixture")]
    // Public Amazon Root CA 3 from reqwest's certificate parser tests; no private key is retained.
    const PUBLIC_TEST_CA_PEM: &[u8] = b"-----BEGIN CERTIFICATE-----\n\
MIIBtjCCAVugAwIBAgITBmyf1XSXNmY/Owua2eiedgPySjAKBggqhkjOPQQDAjA5\n\
MQswCQYDVQQGEwJVUzEPMA0GA1UEChMGQW1hem9uMRkwFwYDVQQDExBBbWF6b24g\n\
Um9vdCBDQSAzMB4XDTE1MDUyNjAwMDAwMFoXDTQwMDUyNjAwMDAwMFowOTELMAkG\n\
A1UEBhMCVVMxDzANBgNVBAoTBkFtYXpvbjEZMBcGA1UEAxMQQW1hem9uIFJvb3Qg\n\
Q0EgMzBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABCmXp8ZBf8ANm+gBG1bG8lKl\n\
ui2yEujSLtf6ycXYqm0fc4E7O5hrOXwzpcVOho6AF2hiRVd9RFgdszflZwjrZt6j\n\
QjBAMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQWBBSr\n\
ttvXBp43rDCGB5Fwx5zEGbF4wDAKBggqhkjOPQQDAgNJADBGAiEA4IWSoxe3jfkr\n\
BqWTrBqYaGFy+uGh0PsceGCmQ5nFuMQCIQCcAu/xlJyzlvnrxir4tiz+OpAUFteM\n\
YyRIHN8wfdVoOw==\n\
-----END CERTIFICATE-----\n";

    #[cfg(feature = "tmdb-smoke-fixture")]
    const SMOKE_PRODUCTION: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
        provider: "tmdb",
        capabilities: &["metadata.search", "metadata.read"],
        hosts: &["api.themoviedb.org"],
        networks: &[NetworkClass::Public],
    };

    #[cfg(feature = "tmdb-smoke-fixture")]
    const SMOKE_LOOPBACK: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
        provider: "tmdb",
        capabilities: &["metadata.search", "metadata.read"],
        hosts: &["api.themoviedb.org"],
        networks: &[NetworkClass::Loopback],
    };

    #[cfg(feature = "tmdb-smoke-fixture")]
    fn smoke_transport() -> GovernedTransport {
        GovernedTransport::tmdb_smoke_fixture(
            SMOKE_PRODUCTION,
            SMOKE_LOOPBACK,
            "127.0.0.1:38421".parse().expect("fixture address"),
            PUBLIC_TEST_CA_PEM,
        )
        .expect("TMDB smoke transport")
    }

    #[test]
    fn dual_stack_cdn_answers_are_complete_bounded_and_all_authorized() {
        use fasti_application::NetworkClass;
        let answers = (1..=4)
            .map(|n| format!("18.238.49.{n}:443").parse::<SocketAddr>().unwrap())
            .chain((1..=8).map(|n| format!("[2600:9000:261f:{n:x}::1]:443").parse().unwrap()))
            .collect::<Vec<_>>();
        let collected = collect_addresses(answers.iter().chain(&answers).copied()).unwrap();
        assert_eq!(collected.len(), 12, "retain every unique dual-stack answer");
        let declaration = OutboundAccessDeclaration {
            provider: "tmdb",
            capabilities: &["metadata.artwork"],
            hosts: &["image.tmdb.org"],
            networks: &[NetworkClass::Public],
        };
        let ips = collected.iter().map(SocketAddr::ip).collect::<Vec<_>>();
        assert!(authorize_outbound(
            declaration,
            &OutboundAccessPolicy::default(),
            "metadata.artwork",
            "image.tmdb.org",
            &ips
        )
        .is_ok());
        let mut mixed = answers;
        mixed.push("127.0.0.1:443".parse().unwrap());
        let all = collect_addresses(mixed).unwrap();
        assert_eq!(all.len(), 13, "never truncate away an unsafe answer");
        assert!(authorize_outbound(
            declaration,
            &OutboundAccessPolicy::default(),
            "metadata.artwork",
            "image.tmdb.org",
            &all.iter().map(SocketAddr::ip).collect::<Vec<_>>()
        )
        .is_err());
        let boundary = (1..=16).map(|n| SocketAddr::from(([18, 238, 49, n], 443)));
        assert_eq!(collect_addresses(boundary.clone()).unwrap().len(), 16);
        assert!(collect_addresses(boundary.chain(["18.238.49.17:443".parse().unwrap()])).is_err());
        assert!(collect_addresses([]).is_err());
    }

    #[test]
    fn pinned_client_disables_ambient_routing() {
        let addresses = ["127.0.0.1:8420".parse().expect("socket address")];
        assert!(pinned_client("localhost", &addresses, Duration::from_secs(1)).is_ok());
    }

    #[cfg(feature = "tmdb-smoke-fixture")]
    #[test]
    fn tmdb_smoke_fixture_rejects_invalid_addresses_and_ca_bundles() {
        for address in ["127.0.0.1:0", "127.0.0.2:38421", "[::1]:38421"] {
            assert!(GovernedTransport::tmdb_smoke_fixture(
                SMOKE_PRODUCTION,
                SMOKE_LOOPBACK,
                address.parse().expect("socket address"),
                PUBLIC_TEST_CA_PEM,
            )
            .is_err());
        }
        for pem in [
            Vec::new(),
            b"not a certificate".to_vec(),
            PUBLIC_TEST_CA_PEM.repeat(2),
            vec![b'x'; SMOKE_CA_PEM_LIMIT + 1],
        ] {
            assert!(GovernedTransport::tmdb_smoke_fixture(
                SMOKE_PRODUCTION,
                SMOKE_LOOPBACK,
                "127.0.0.1:38421".parse().expect("fixture address"),
                &pem,
            )
            .is_err());
        }
    }

    #[cfg(feature = "tmdb-smoke-fixture")]
    #[tokio::test(flavor = "current_thread")]
    async fn tmdb_smoke_fixture_authorizes_only_its_exact_origin_and_declaration() {
        let endpoint = reqwest::Url::parse("https://api.themoviedb.org/3/search/multi")
            .expect("TMDB endpoint");
        let client = smoke_transport()
            .authorize(
                SMOKE_PRODUCTION,
                &OutboundAccessPolicy::default(),
                "metadata.search",
                &endpoint,
            )
            .await
            .expect("authorized fixture client");
        assert_eq!(
            client.configuration_digest(),
            configuration_digest("tmdb", "metadata.search", &endpoint).expect("digest")
        );

        let unrelated = OutboundAccessDeclaration {
            provider: "other",
            ..SMOKE_PRODUCTION
        };
        assert!(smoke_transport()
            .authorize(
                unrelated,
                &OutboundAccessPolicy::default(),
                "metadata.search",
                &endpoint,
            )
            .await
            .is_err());
        assert!(smoke_transport()
            .authorize(
                SMOKE_PRODUCTION,
                &OutboundAccessPolicy::default(),
                "metadata.search",
                &reqwest::Url::parse("https://www.googleapis.com/books/v1/volumes")
                    .expect("unrelated endpoint"),
            )
            .await
            .is_err());
        assert!(smoke_transport()
            .authorize(
                SMOKE_PRODUCTION,
                &OutboundAccessPolicy::default(),
                "metadata.search",
                &reqwest::Url::parse("https://api.themoviedb.org:444/3/search/multi")
                    .expect("explicit-port endpoint"),
            )
            .await
            .is_err());
    }

    #[cfg(feature = "tmdb-smoke-fixture")]
    #[tokio::test(flavor = "current_thread")]
    async fn tmdb_smoke_fixture_preserves_every_operator_deny() {
        let endpoint = reqwest::Url::parse("https://api.themoviedb.org/3/search/multi")
            .expect("TMDB endpoint");
        let policies = [
            OutboundAccessPolicy {
                deny_providers: vec!["tmdb".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            OutboundAccessPolicy {
                deny_capabilities: vec!["metadata.search".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            OutboundAccessPolicy {
                deny_hosts: vec!["api.themoviedb.org".to_owned()],
                ..OutboundAccessPolicy::default()
            },
            OutboundAccessPolicy {
                deny_networks: vec![NetworkClass::Loopback],
                ..OutboundAccessPolicy::default()
            },
        ];
        for policy in policies {
            assert!(smoke_transport()
                .authorize(SMOKE_PRODUCTION, &policy, "metadata.search", &endpoint)
                .await
                .is_err());
        }
    }

    #[cfg(feature = "tmdb-smoke-fixture")]
    #[test]
    fn ordinary_transport_stays_fixture_free_when_the_feature_is_compiled() {
        assert!(GovernedTransport::default().smoke_fixture.is_none());
    }

    #[test]
    fn authorized_client_rejects_origin_drift() {
        let addresses = ["127.0.0.1:443".parse().expect("socket address")];
        let client = pinned_client("api.example.test", &addresses, Duration::from_secs(1))
            .expect("pinned client");
        let origin_url = reqwest::Url::parse("https://api.example.test/v1/search").expect("URL");
        let authorized = AuthorizedClient {
            client,
            origin: Origin::try_from(&origin_url).expect("origin"),
            configuration_digest: configuration_digest("example", "metadata.search", &origin_url)
                .expect("digest"),
            _permit: Arc::new(Semaphore::new(1))
                .try_acquire_owned()
                .expect("permit"),
        };
        assert!(authorized
            .get(reqwest::Url::parse("https://api.example.test/v1/items").expect("URL"))
            .is_ok());
        assert!(authorized
            .get(reqwest::Url::parse("https://other.example.test/v1/items").expect("URL"))
            .is_err());
    }

    #[test]
    fn dispatcher_is_bounded_to_four_requests() {
        let transport = GovernedTransport::default();
        let permits = (0..REQUEST_CONCURRENCY_LIMIT)
            .map(|_| {
                Arc::clone(&transport.requests)
                    .try_acquire_owned()
                    .expect("bounded permit")
            })
            .collect::<Vec<_>>();
        assert!(Arc::clone(&transport.requests).try_acquire_owned().is_err());
        drop(permits);
        assert!(Arc::clone(&transport.requests).try_acquire_owned().is_ok());
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
        assert_eq!(second, Err("The host name lookup timed out."));

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
