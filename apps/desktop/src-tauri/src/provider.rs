use crate::outbound_http::bounded_body;
use crate::setup::{DesktopProblem, KEYRING_SERVICE};
use fasti_application::{
    authorize_outbound, NetworkClass, OutboundAccessPolicy, ProviderAccessDeclaration,
    ProviderCandidate,
};
use reqwest::{redirect::Policy, Client, Url};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

const PROVIDER_ID: &str = "google-books";
const PROVIDER_LABEL: &str = "Google Books";
const PROVIDER_HOST: &str = "www.googleapis.com";
const PROVIDER_KEY_ACCOUNT: &str = "provider/google-books/api-key";
const PROVIDER_KEY_ENV: &str = "GOOGLE_BOOKS_API_KEY";
const MAX_KEY_BYTES: usize = 512;
const MAX_QUERY_BYTES: usize = 256;
const MAX_RESPONSE_BYTES: usize = 2_000_000;
const MAX_RESULTS: usize = 10;
const DECLARATION: ProviderAccessDeclaration<'static> = ProviderAccessDeclaration {
    provider: PROVIDER_ID,
    capabilities: &["metadata.search"],
    hosts: &[PROVIDER_HOST],
    networks: &[NetworkClass::Public],
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProviderCredentialStatus {
    provider: &'static str,
    label: &'static str,
    configured: bool,
    source: &'static str,
    writable: bool,
    docs_url: &'static str,
}

pub(crate) fn credential_status() -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    let source = credential_source()?;
    Ok(vec![ProviderCredentialStatus {
        provider: PROVIDER_ID,
        label: PROVIDER_LABEL,
        configured: source.is_some(),
        source: source.unwrap_or("none"),
        writable: env::var_os(PROVIDER_KEY_ENV).is_none(),
        docs_url: "https://developers.google.com/books/docs/v1/using",
    }])
}

pub(crate) fn save_key(provider: &str, key: Option<String>) -> Result<(), DesktopProblem> {
    require_provider(provider)?;
    if env::var_os(PROVIDER_KEY_ENV).is_some() {
        return Err(DesktopProblem::connection(
            "managed_provider_credential",
            "The provider key is managed",
            format!("{PROVIDER_KEY_ENV} supplies this credential."),
            "Change the environment secret, then restart Fasti.",
        ));
    }
    let entry = provider_entry()?;
    match key.map(|value| value.trim().to_owned()) {
        Some(value) if !value.is_empty() => {
            validate_key(&value)?;
            entry.set_password(&value).map_err(|_| {
                DesktopProblem::secure_storage(
                    "Fasti could not save the Google Books key in the system credential store.",
                )
            })?;
        }
        _ => match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => {
                return Err(DesktopProblem::secure_storage(
                    "Fasti could not remove the Google Books key.",
                ));
            }
        },
    }
    Ok(())
}

pub(crate) async fn search(
    provider: String,
    query: String,
    policy: OutboundAccessPolicy,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    require_provider(&provider)?;
    let query = query.trim();
    if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.chars().any(char::is_control) {
        return Err(provider_problem(
            "invalid_provider_query",
            "The search query is not valid",
            "Enter 1 to 256 characters without control characters.",
            "Edit the query, then search again.",
        ));
    }

    let addresses = resolve_provider().await?;
    authorize_outbound(
        DECLARATION,
        &policy,
        "metadata.search",
        PROVIDER_HOST,
        &addresses,
    )
    .map_err(|denial| {
        provider_problem(
            "provider_access_denied",
            "Provider access is denied",
            format!(
                "The effective policy denied {} {}.",
                denial.dimension(),
                denial.value()
            ),
            "Review Provider access in Settings, then retry.",
        )
    })?;

    let socket_addresses: Vec<SocketAddr> = addresses
        .iter()
        .copied()
        .map(|address| SocketAddr::new(address, 443))
        .collect();
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(15))
        .resolve_to_addrs(PROVIDER_HOST, &socket_addresses)
        .build()
        .map_err(|_| provider_unavailable("Fasti could not initialize the provider client."))?;
    let mut url =
        Url::parse("https://www.googleapis.com/books/v1/volumes").expect("constant provider URL");
    {
        let mut parameters = url.query_pairs_mut();
        parameters
            .append_pair("q", query)
            .append_pair("startIndex", "0")
            .append_pair("maxResults", &MAX_RESULTS.to_string())
            .append_pair("projection", "lite");
        if let Some(key) = load_key()? {
            parameters.append_pair("key", &key);
        }
    }
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| provider_unavailable(provider_error_detail(&error)))?;
    if response.status().as_u16() == 429 {
        return Err(provider_problem(
            "provider_rate_limited",
            "Google Books limited the request",
            "The provider returned HTTP 429.",
            "Wait before searching again. Fasti local data is unchanged.",
        ));
    }
    if !response.status().is_success() {
        return Err(provider_unavailable(format!(
            "Google Books returned HTTP {}.",
            response.status().as_u16()
        )));
    }
    let body = bounded_body(response, MAX_RESPONSE_BYTES)
        .await
        .map_err(provider_unavailable)?;
    let response: GoogleBooksResponse = serde_json::from_slice(&body).map_err(|_| {
        provider_problem(
            "invalid_provider_response",
            "Google Books returned invalid data",
            "The response did not match the expected volumes shape.",
            "Retry later. Fasti local data is unchanged.",
        )
    })?;
    Ok(response
        .items
        .unwrap_or_default()
        .into_iter()
        .filter_map(candidate)
        .take(MAX_RESULTS)
        .collect())
}

fn candidate(volume: GoogleBook) -> Option<ProviderCandidate> {
    let authors = volume.volume_info.authors.unwrap_or_default().join(", ");
    let description = (!authors.is_empty()).then_some(authors);
    ProviderCandidate::try_new(
        PROVIDER_ID,
        volume.id,
        volume.volume_info.title,
        "book",
        description,
        None,
    )
    .ok()
}

async fn resolve_provider() -> Result<Vec<IpAddr>, DesktopProblem> {
    let addresses: Vec<IpAddr> = tokio::net::lookup_host((PROVIDER_HOST, 443))
        .await
        .map_err(|_| provider_unavailable("Fasti could not resolve the Google Books host."))?
        .map(|address| address.ip())
        .collect();
    if addresses.is_empty() {
        return Err(provider_unavailable(
            "The Google Books host returned no addresses.",
        ));
    }
    Ok(addresses)
}

fn provider_entry() -> Result<keyring::Entry, DesktopProblem> {
    keyring::Entry::new(KEYRING_SERVICE, PROVIDER_KEY_ACCOUNT).map_err(|_| {
        DesktopProblem::secure_storage("Fasti could not open the system credential store.")
    })
}

fn credential_source() -> Result<Option<&'static str>, DesktopProblem> {
    match env::var(PROVIDER_KEY_ENV) {
        Ok(value) => {
            validate_key(&value)?;
            return Ok(Some("environment"));
        }
        Err(env::VarError::NotPresent) => {}
        Err(env::VarError::NotUnicode(_)) => {
            return Err(provider_problem(
                "invalid_provider_credential",
                "The provider key is not valid",
                format!("{PROVIDER_KEY_ENV} is not valid text."),
                "Replace the environment secret, then restart Fasti.",
            ));
        }
    }
    match provider_entry()?.get_password() {
        Ok(value) => {
            validate_key(&value)?;
            Ok(Some("keyring"))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(DesktopProblem::secure_storage(
            "Fasti could not read provider credential status.",
        )),
    }
}

fn load_key() -> Result<Option<String>, DesktopProblem> {
    if let Ok(value) = env::var(PROVIDER_KEY_ENV) {
        validate_key(&value)?;
        return Ok(Some(value));
    }
    match provider_entry()?.get_password() {
        Ok(value) => {
            validate_key(&value)?;
            Ok(Some(value))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(DesktopProblem::secure_storage(
            "Fasti could not read the Google Books key.",
        )),
    }
}

fn validate_key(value: &str) -> Result<(), DesktopProblem> {
    if value.is_empty()
        || value.len() > MAX_KEY_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(provider_problem(
            "invalid_provider_credential",
            "The provider key is not valid",
            "Use 1 to 512 visible ASCII characters.",
            "Check the key in Google Cloud, then enter it again.",
        ));
    }
    Ok(())
}

fn require_provider(provider: &str) -> Result<(), DesktopProblem> {
    if provider == PROVIDER_ID {
        Ok(())
    } else {
        Err(provider_problem(
            "unknown_provider",
            "The provider is not available",
            "This build supports Google Books search only.",
            "Select Google Books, then retry.",
        ))
    }
}

fn provider_error_detail(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "The Google Books request timed out after 15 seconds."
    } else if error.is_connect() {
        "Fasti could not connect to Google Books. Check network access and certificate trust."
    } else {
        "The Google Books request failed before Fasti received a response."
    }
}

fn provider_unavailable(detail: impl Into<String>) -> DesktopProblem {
    provider_problem(
        "provider_unavailable",
        "Google Books is unavailable",
        detail,
        "Check Provider access and the network, then retry. Fasti local data is unchanged.",
    )
}

fn provider_problem(
    code: &'static str,
    title: &'static str,
    detail: impl Into<String>,
    next_action: &'static str,
) -> DesktopProblem {
    DesktopProblem::connection(code, title, detail, next_action)
}

#[derive(Debug, Deserialize)]
struct GoogleBooksResponse {
    items: Option<Vec<GoogleBook>>,
}

#[derive(Debug, Deserialize)]
struct GoogleBook {
    id: String,
    #[serde(rename = "volumeInfo")]
    volume_info: GoogleBookInfo,
}

#[derive(Debug, Deserialize)]
struct GoogleBookInfo {
    title: String,
    authors: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_google_books_to_neutral_bounded_candidates() {
        let response: GoogleBooksResponse = serde_json::from_str(
            r#"{"items":[{"id":"volume-1","volumeInfo":{"title":"The Left Hand of Darkness","authors":["Ursula K. Le Guin"]}}]}"#,
        )
        .expect("Google Books fixture");
        let candidates: Vec<_> = response
            .items
            .expect("items")
            .into_iter()
            .filter_map(candidate)
            .collect();
        assert_eq!(candidates.len(), 1);
        let value = serde_json::to_value(&candidates[0]).expect("candidate JSON");
        assert_eq!(value["provider"], PROVIDER_ID);
        assert_eq!(value["provider_id"], "volume-1");
        assert_eq!(value["kind"], "book");
    }

    #[test]
    fn credentials_are_bounded_and_never_accept_whitespace() {
        assert!(validate_key("visible-key").is_ok());
        assert!(validate_key("").is_err());
        assert!(validate_key("contains a space").is_err());
        assert!(validate_key(&"x".repeat(MAX_KEY_BYTES + 1)).is_err());
    }
}
