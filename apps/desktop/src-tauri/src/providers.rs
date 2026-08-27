use crate::outbound_http::{bounded_body, pinned_client, resolve_once};
use crate::secure_storage::{Entry, Error as KeyringError};
use crate::setup::{DesktopProblem, KEYRING_SERVICE};
use fasti_application::{
    authorize_outbound, NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy,
};
use fasti_store::DataRootIdentity;
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Duration;
use zeroize::Zeroize;

const GOOGLE_BOOKS_PROVIDER: &str = "google-books";
const GOOGLE_BOOKS_LABEL: &str = "Google Books";
const GOOGLE_BOOKS_HOST: &str = "www.googleapis.com";
const GOOGLE_BOOKS_URL: &str = "https://www.googleapis.com/books/v1/volumes";
const GOOGLE_BOOKS_ENV: &str = "GOOGLE_BOOKS_API_KEY";
const GOOGLE_BOOKS_ACCOUNT: &str = "provider/google-books/api-key";
const GOOGLE_BOOKS_DOCS: &str = "https://developers.google.com/books/docs/v1/using";
const GOOGLE_BOOKS_CAPABILITY: &str = "metadata.search";
const QUERY_LIMIT: usize = 256;
const CREDENTIAL_LIMIT: usize = 512;
const RESPONSE_LIMIT: usize = 2_000_000;
const RESULT_LIMIT: usize = 10;
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);

const GOOGLE_BOOKS_ACCESS: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
    provider: GOOGLE_BOOKS_PROVIDER,
    capabilities: &[GOOGLE_BOOKS_CAPABILITY],
    hosts: &[GOOGLE_BOOKS_HOST],
    networks: &[NetworkClass::Public],
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CredentialSource {
    None,
    Environment,
    CredentialStore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderCredentialStatus {
    provider: &'static str,
    label: &'static str,
    configured: bool,
    source: CredentialSource,
    writable: bool,
    docs_url: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SaveProviderCredentialInput {
    provider: String,
    credential: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteProviderCredentialInput {
    provider: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderSearchInput {
    provider: String,
    query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderCandidate {
    provider: &'static str,
    provider_id: String,
    title: String,
    kind: &'static str,
    authors: Vec<String>,
    image_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleVolumesResponse {
    #[serde(default)]
    items: Vec<GoogleVolume>,
}

#[derive(Debug, Deserialize)]
struct GoogleVolume {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "volumeInfo")]
    #[serde(default)]
    volume_info: Option<GoogleVolumeInfo>,
}

#[derive(Debug, Deserialize)]
struct GoogleVolumeInfo {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    authors: Vec<String>,
}

pub(crate) fn credential_statuses(
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    Ok(vec![google_books_status(identity)?])
}

pub(crate) fn save_credential(
    mut input: SaveProviderCredentialInput,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    let result = (|| {
        require_google_books(&input.provider)?;
        if environment_is_configured()? {
            return Err(DesktopProblem::secure_storage(
                "The Google Books credential is managed by GOOGLE_BOOKS_API_KEY.",
            ));
        }
        validate_credential(&input.credential)?;
        let entry = provider_entry(identity)?;
        entry.set_secret(input.credential.as_bytes()).map_err(|_| {
            DesktopProblem::secure_storage(
                "Fasti could not save the Google Books credential securely.",
            )
        })?;
        let mut stored = entry.get_secret().map_err(|_| {
            DesktopProblem::secure_storage(
                "The system credential store did not return the saved provider credential.",
            )
        })?;
        let matches = stored == input.credential.as_bytes();
        stored.zeroize();
        if !matches {
            return Err(DesktopProblem::secure_storage(
                "The system credential store did not retain the provider credential.",
            ));
        }
        Ok(())
    })();
    input.credential.zeroize();
    result?;
    credential_statuses(identity)
}

pub(crate) fn delete_credential(
    input: DeleteProviderCredentialInput,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    require_google_books(&input.provider)?;
    if environment_is_configured()? {
        return Err(DesktopProblem::secure_storage(
            "The Google Books credential is managed by GOOGLE_BOOKS_API_KEY.",
        ));
    }
    match provider_entry(identity)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => credential_statuses(identity),
        Err(_) => Err(DesktopProblem::secure_storage(
            "Fasti could not remove the Google Books credential.",
        )),
    }
}

pub(crate) async fn search(
    input: ProviderSearchInput,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    require_google_books(&input.provider)?;
    validate_query(&input.query)?;

    let addresses = resolve_once(GOOGLE_BOOKS_HOST, 443)
        .await
        .map_err(DesktopProblem::provider)?;
    let address_values = addresses.iter().map(|value| value.ip()).collect::<Vec<_>>();
    authorize_outbound(
        GOOGLE_BOOKS_ACCESS,
        policy,
        GOOGLE_BOOKS_CAPABILITY,
        GOOGLE_BOOKS_HOST,
        &address_values,
    )
    .map_err(|denial| {
        DesktopProblem::provider(format!(
            "The outbound policy denied the Google Books {}.",
            denial.dimension()
        ))
    })?;
    let client = pinned_client(GOOGLE_BOOKS_HOST, &addresses, PROVIDER_TIMEOUT)
        .map_err(DesktopProblem::provider)?;

    // Credential access follows DNS resolution, declaration checks, policy checks,
    // and construction of a proxy-free, redirect-free client.
    let credential = load_credential(identity)?;
    let mut url = reqwest::Url::parse(GOOGLE_BOOKS_URL)
        .map_err(|_| DesktopProblem::provider("The Google Books endpoint is invalid."))?;
    url.query_pairs_mut()
        .append_pair("q", &input.query)
        .append_pair("startIndex", "0")
        .append_pair("maxResults", &RESULT_LIMIT.to_string())
        .append_pair("projection", "lite");
    let request = authenticated_request(&client, url, credential)?;
    let response = request
        .send()
        .await
        .map_err(|_| DesktopProblem::provider("Google Books could not be reached."))?;
    if matches!(
        response.status(),
        reqwest::StatusCode::BAD_REQUEST
            | reqwest::StatusCode::UNAUTHORIZED
            | reqwest::StatusCode::FORBIDDEN
    ) {
        return Err(DesktopProblem::provider_credential(
            "Google Books rejected the configured API key.",
        ));
    }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(DesktopProblem::provider(
            "Google Books rate limited the request. Wait, then retry.",
        ));
    }
    if response.status() != reqwest::StatusCode::OK {
        return Err(DesktopProblem::provider(format!(
            "Google Books returned HTTP {}.",
            response.status().as_u16()
        )));
    }
    let json_content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"));
    if !json_content_type {
        return Err(DesktopProblem::provider(
            "Google Books returned an unexpected content type.",
        ));
    }
    let body = bounded_body(response, RESPONSE_LIMIT)
        .await
        .map_err(DesktopProblem::provider)?;
    parse_candidates(&body)
}

fn google_books_status(
    identity: DataRootIdentity,
) -> Result<ProviderCredentialStatus, DesktopProblem> {
    if environment_is_configured()? {
        return Ok(status(true, CredentialSource::Environment, false));
    }
    let configured = match provider_entry(identity)?.get_secret() {
        Ok(mut secret) => {
            let valid = validate_credential_bytes(&secret).is_ok();
            secret.zeroize();
            valid
        }
        Err(KeyringError::NoEntry) => false,
        Err(_) => {
            return Err(DesktopProblem::secure_storage(
                "Fasti could not read the system credential store.",
            ))
        }
    };
    Ok(status(
        configured,
        if configured {
            CredentialSource::CredentialStore
        } else {
            CredentialSource::None
        },
        true,
    ))
}

const fn status(
    configured: bool,
    source: CredentialSource,
    writable: bool,
) -> ProviderCredentialStatus {
    ProviderCredentialStatus {
        provider: GOOGLE_BOOKS_PROVIDER,
        label: GOOGLE_BOOKS_LABEL,
        configured,
        source,
        writable,
        docs_url: GOOGLE_BOOKS_DOCS,
    }
}

fn provider_entry(identity: DataRootIdentity) -> Result<Entry, DesktopProblem> {
    let account = crate::secure_storage::scoped_account(GOOGLE_BOOKS_ACCOUNT, identity);
    Entry::new(KEYRING_SERVICE, &account).map_err(|_| {
        DesktopProblem::secure_storage("Fasti could not open the system credential store.")
    })
}

fn environment_credential() -> Result<Option<String>, DesktopProblem> {
    match std::env::var(GOOGLE_BOOKS_ENV) {
        Ok(mut value) => {
            if let Err(problem) = validate_credential(&value) {
                value.zeroize();
                return Err(problem);
            }
            Ok(Some(value))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(DesktopProblem::secure_storage(
            "GOOGLE_BOOKS_API_KEY must contain valid UTF-8.",
        )),
    }
}

fn environment_is_configured() -> Result<bool, DesktopProblem> {
    match environment_credential()? {
        Some(mut value) => {
            value.zeroize();
            Ok(true)
        }
        None => Ok(false),
    }
}

fn load_credential(identity: DataRootIdentity) -> Result<Option<String>, DesktopProblem> {
    if let Some(value) = environment_credential()? {
        return Ok(Some(value));
    }
    match provider_entry(identity)?.get_secret() {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(mut value) => {
                if let Err(problem) = validate_credential(&value) {
                    value.zeroize();
                    return Err(problem);
                }
                Ok(Some(value))
            }
            Err(error) => {
                let mut bytes = error.into_bytes();
                bytes.zeroize();
                Err(DesktopProblem::provider_credential(
                    "The saved Google Books credential is invalid.",
                ))
            }
        },
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(DesktopProblem::secure_storage(
            "Fasti could not read the system credential store.",
        )),
    }
}

fn authenticated_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    credential: Option<String>,
) -> Result<reqwest::RequestBuilder, DesktopProblem> {
    let mut secret = credential.ok_or_else(|| {
        DesktopProblem::provider_credential(
            "Add a Google Books API key in Settings before searching Google Books.",
        )
    })?;
    let header_result = HeaderValue::from_str(&secret);
    secret.zeroize();
    let mut header = header_result.map_err(|_| {
        DesktopProblem::provider_credential("The Google Books credential is invalid.")
    })?;
    header.set_sensitive(true);
    Ok(client.get(url).header("X-Goog-Api-Key", header))
}

fn require_google_books(provider: &str) -> Result<(), DesktopProblem> {
    if provider == GOOGLE_BOOKS_PROVIDER {
        Ok(())
    } else {
        Err(DesktopProblem::configuration(
            "The requested metadata provider is not supported.",
        ))
    }
}

fn validate_query(query: &str) -> Result<(), DesktopProblem> {
    if query.is_empty()
        || query.trim() != query
        || query.len() > QUERY_LIMIT
        || query.chars().any(char::is_control)
    {
        return Err(DesktopProblem::configuration(
            "The provider query must contain 1 to 256 bytes without leading, trailing, or control characters.",
        ));
    }
    Ok(())
}

fn validate_credential(value: &str) -> Result<(), DesktopProblem> {
    validate_credential_bytes(value.as_bytes())
}

fn validate_credential_bytes(value: &[u8]) -> Result<(), DesktopProblem> {
    if value.is_empty()
        || value.len() > CREDENTIAL_LIMIT
        || !value.iter().all(|byte| byte.is_ascii_graphic())
    {
        return Err(DesktopProblem::provider_credential(
            "The Google Books credential must contain 1 to 512 visible ASCII characters.",
        ));
    }
    Ok(())
}

/// Parses a Google Books response into validated book candidates, omitting incomplete, unsafe, duplicate, and excess entries.
///
/// # Errors
///
/// Returns an error when the response body is not valid JSON.
///
/// # Examples
///
/// ```
/// let body = br#"{
///     "items": [{
///         "id": "volume-1",
///         "volumeInfo": {
///             "title": "Example Book",
///             "authors": ["Example Author"]
///         }
///     }]
/// }"#;
///
/// let candidates = parse_candidates(body).unwrap();
/// assert_eq!(candidates.len(), 1);
/// assert_eq!(candidates[0].title, "Example Book");
/// ```
fn parse_candidates(body: &[u8]) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let response: GoogleVolumesResponse = serde_json::from_slice(body)
        .map_err(|_| DesktopProblem::provider("Google Books returned invalid JSON."))?;
    let mut seen = BTreeSet::new();
    Ok(response
        .items
        .into_iter()
        .filter_map(|item| {
            let id = item.id?;
            let volume_info = item.volume_info?;
            let title = volume_info.title?;
            if !valid_candidate_text(&id, 256)
                || !valid_candidate_text(&title, 512)
                || volume_info.authors.len() > 10
                || volume_info
                    .authors
                    .iter()
                    .any(|author| !valid_candidate_text(author, 128))
            {
                return None;
            }
            if !seen.insert(id.clone()) {
                return None;
            }
            Some(ProviderCandidate {
                provider: GOOGLE_BOOKS_PROVIDER,
                provider_id: id,
                title,
                kind: "book",
                authors: volume_info.authors,
                image_url: None,
            })
        })
        .take(RESULT_LIMIT)
        .collect())
}

fn valid_candidate_text(value: &str, limit: usize) -> bool {
    !value.is_empty()
        && value.len() <= limit
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_at_most_ten_neutral_book_candidates() {
        let items = (0..12)
            .map(|index| {
                format!(
                    r#"{{"id":"book-{index}","volumeInfo":{{"title":"Book {index}","authors":["Author"]}}}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(r#"{{"items":[{items}]}}"#);
        let candidates = parse_candidates(body.as_bytes()).expect("provider candidates");

        assert_eq!(candidates.len(), RESULT_LIMIT);
        assert_eq!(candidates[0].provider, GOOGLE_BOOKS_PROVIDER);
        assert_eq!(candidates[0].kind, "book");
        assert!(candidates[0].image_url.is_none());
    }

    #[test]
    fn skips_partial_or_unsafe_provider_items() {
        let body = br#"{
          "items": [
            {"id":"valid","volumeInfo":{"title":"A Book","authors":["An Author"]}},
            {"id":"missing-title","volumeInfo":{"authors":[]}},
            {"id":"control","volumeInfo":{"title":"Bad\nTitle","authors":[]}}
          ]
        }"#;
        let candidates = parse_candidates(body).expect("partial response");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].provider_id, "valid");
    }

    #[test]
    fn duplicate_provider_ids_are_removed_before_the_ui() {
        let body = br#"{
          "items": [
            {"id":"same","volumeInfo":{"title":"First","authors":["Author"]}},
            {"id":"same","volumeInfo":{"title":"Second","authors":["Author"]}}
          ]
        }"#;
        let candidates = parse_candidates(body).expect("deduplicated response");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "First");
    }

    #[test]
    fn invalid_duplicate_does_not_hide_a_later_valid_candidate() {
        let body = br#"{
          "items": [
            {"id":"same","volumeInfo":{"authors":["Author"]}},
            {"id":"same","volumeInfo":{"title":"Valid","authors":["Author"]}}
          ]
        }"#;
        let candidates = parse_candidates(body).expect("valid duplicate candidate");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].provider_id, "same");
        assert_eq!(candidates[0].title, "Valid");
    }

    #[test]
    fn credentials_and_queries_are_strictly_bounded() {
        assert!(validate_credential("valid-key").is_ok());
        assert!(validate_credential("").is_err());
        assert!(validate_credential("key with spaces").is_err());
        assert!(validate_query("isbn:9780140328721").is_ok());
        assert!(validate_query(" leading").is_err());
        assert!(validate_query(&"x".repeat(QUERY_LIMIT + 1)).is_err());
    }

    #[test]
    fn missing_credential_stops_before_a_request_can_be_sent() {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client");
        let url = reqwest::Url::parse(GOOGLE_BOOKS_URL).expect("provider URL");

        assert!(authenticated_request(&client, url, None).is_err());
    }

    #[test]
    fn credential_is_sent_only_in_a_sensitive_header() {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client");
        let url = reqwest::Url::parse(GOOGLE_BOOKS_URL).expect("provider URL");
        let request = authenticated_request(&client, url, Some("test-key".into()))
            .expect("authenticated request")
            .build()
            .expect("built request");
        let header = request
            .headers()
            .get("X-Goog-Api-Key")
            .expect("credential header");

        assert_eq!(header, "test-key");
        assert!(header.is_sensitive());
        assert!(!request.url().as_str().contains("test-key"));
    }
}
