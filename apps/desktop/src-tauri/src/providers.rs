use crate::outbound_http::{bounded_body, pinned_client, resolve_once};
use crate::secure_storage::{Entry, Error as KeyringError};
use crate::setup::{DesktopProblem, KEYRING_SERVICE};
use fasti_application::{
    authorize_outbound, NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy,
};
use fasti_store::DataRootIdentity;
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Duration;
use zeroize::Zeroize;

const METADATA_SEARCH_CAPABILITY: &str = "metadata.search";
const QUERY_LIMIT: usize = 256;
const CREDENTIAL_LIMIT: usize = 512;
const RESPONSE_LIMIT: usize = 2_000_000;
const RESULT_LIMIT: usize = 10;
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderKind {
    GoogleBooks,
    Tmdb,
}

#[derive(Debug, Clone, Copy)]
struct ProviderDefinition {
    kind: ProviderKind,
    id: &'static str,
    label: &'static str,
    host: &'static str,
    endpoint: &'static str,
    environment: &'static str,
    account: &'static str,
    docs_url: &'static str,
    authorization_header: &'static str,
    access: OutboundAccessDeclaration<'static>,
}

const GOOGLE_BOOKS: ProviderDefinition = ProviderDefinition {
    kind: ProviderKind::GoogleBooks,
    id: "google-books",
    label: "Google Books",
    host: "www.googleapis.com",
    endpoint: "https://www.googleapis.com/books/v1/volumes",
    environment: "GOOGLE_BOOKS_API_KEY",
    account: "provider/google-books/api-key",
    docs_url: "https://developers.google.com/books/docs/v1/using",
    authorization_header: "X-Goog-Api-Key",
    access: OutboundAccessDeclaration {
        provider: "google-books",
        capabilities: &[METADATA_SEARCH_CAPABILITY],
        hosts: &["www.googleapis.com"],
        networks: &[NetworkClass::Public],
    },
};

const TMDB: ProviderDefinition = ProviderDefinition {
    kind: ProviderKind::Tmdb,
    id: "tmdb",
    label: "TMDB",
    host: "api.themoviedb.org",
    endpoint: "https://api.themoviedb.org/3/search/multi",
    environment: "TMDB_API_KEY",
    account: "provider/tmdb/read-access-token",
    docs_url: "https://developer.themoviedb.org/docs/authentication-application",
    authorization_header: "Authorization",
    access: OutboundAccessDeclaration {
        provider: "tmdb",
        capabilities: &[METADATA_SEARCH_CAPABILITY],
        hosts: &["api.themoviedb.org"],
        networks: &[NetworkClass::Public],
    },
};

const PROVIDERS: [ProviderDefinition; 2] = [GOOGLE_BOOKS, TMDB];

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

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    #[serde(default)]
    results: Vec<TmdbSearchResult>,
}

#[derive(Debug, Deserialize)]
struct TmdbSearchResult {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    media_type: Option<TmdbMediaType>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    adult: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TmdbMediaType {
    Movie,
    Tv,
    #[serde(other)]
    Other,
}

pub(crate) fn credential_statuses(
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    PROVIDERS
        .iter()
        .map(|provider| credential_status(*provider, identity))
        .collect()
}

pub(crate) fn save_credential(
    mut input: SaveProviderCredentialInput,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    let result = (|| {
        let provider = writable_provider(&input.provider)?;
        validate_credential(provider, &input.credential)?;
        let entry = provider_entry(provider, identity)?;
        entry.set_secret(input.credential.as_bytes()).map_err(|_| {
            DesktopProblem::secure_storage(format!(
                "Fasti could not save the {} credential securely.",
                provider.label
            ))
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
    let provider = writable_provider(&input.provider)?;
    match provider_entry(provider, identity)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => credential_statuses(identity),
        Err(_) => Err(DesktopProblem::secure_storage(format!(
            "Fasti could not remove the {} credential.",
            provider.label
        ))),
    }
}

fn writable_provider(id: &str) -> Result<ProviderDefinition, DesktopProblem> {
    let provider = provider(id)?;
    if environment_is_configured(provider)? {
        return Err(DesktopProblem::secure_storage(format!(
            "The {} credential is managed by {}.",
            provider.label, provider.environment
        )));
    }
    Ok(provider)
}

pub(crate) async fn search(
    input: ProviderSearchInput,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let provider = provider(&input.provider)?;
    validate_query(&input.query)?;

    let addresses = resolve_once(provider.host, 443)
        .await
        .map_err(DesktopProblem::provider)?;
    let address_values = addresses.iter().map(|value| value.ip()).collect::<Vec<_>>();
    authorize_outbound(
        provider.access,
        policy,
        METADATA_SEARCH_CAPABILITY,
        provider.host,
        &address_values,
    )
    .map_err(|denial| {
        DesktopProblem::provider(format!(
            "The outbound policy denied the {} {}.",
            provider.label,
            denial.dimension()
        ))
    })?;
    let client = pinned_client(provider.host, &addresses, PROVIDER_TIMEOUT)
        .map_err(DesktopProblem::provider)?;

    // Credential access follows DNS resolution, declaration checks, policy checks,
    // and construction of a proxy-free, redirect-free client.
    let credential = load_credential(provider, identity)?;
    let url = search_url(provider, &input.query)?;
    let request = authenticated_request(provider, &client, url, credential)?;
    let response = request.send().await.map_err(|_| {
        DesktopProblem::provider(format!("{} could not be reached.", provider.label))
    })?;
    if matches!(
        response.status(),
        reqwest::StatusCode::BAD_REQUEST
            | reqwest::StatusCode::UNAUTHORIZED
            | reqwest::StatusCode::FORBIDDEN
    ) {
        return Err(DesktopProblem::provider_credential(format!(
            "{} rejected the configured credential.",
            provider.label
        )));
    }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(DesktopProblem::provider(format!(
            "{} rate limited the request. Wait, then retry.",
            provider.label
        )));
    }
    if response.status() != reqwest::StatusCode::OK {
        return Err(DesktopProblem::provider(format!(
            "{} returned HTTP {}.",
            provider.label,
            response.status().as_u16()
        )));
    }
    let json_content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("application/json"));
    if !json_content_type {
        return Err(DesktopProblem::provider(format!(
            "{} returned an unexpected content type.",
            provider.label
        )));
    }
    let body = bounded_body(response, RESPONSE_LIMIT)
        .await
        .map_err(DesktopProblem::provider)?;
    parse_candidates(provider, &body)
}

fn credential_status(
    provider: ProviderDefinition,
    identity: DataRootIdentity,
) -> Result<ProviderCredentialStatus, DesktopProblem> {
    if environment_is_configured(provider)? {
        return Ok(status(provider, true, CredentialSource::Environment, false));
    }
    let configured = match provider_entry(provider, identity)?.get_secret() {
        Ok(mut secret) => {
            let valid = validate_credential_bytes(provider, &secret).is_ok();
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
        provider,
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
    provider: ProviderDefinition,
    configured: bool,
    source: CredentialSource,
    writable: bool,
) -> ProviderCredentialStatus {
    ProviderCredentialStatus {
        provider: provider.id,
        label: provider.label,
        configured,
        source,
        writable,
        docs_url: provider.docs_url,
    }
}

fn provider_entry(
    provider: ProviderDefinition,
    identity: DataRootIdentity,
) -> Result<Entry, DesktopProblem> {
    let account = crate::secure_storage::scoped_account(provider.account, identity);
    Entry::new(KEYRING_SERVICE, &account).map_err(|_| {
        DesktopProblem::secure_storage("Fasti could not open the system credential store.")
    })
}

fn environment_credential(provider: ProviderDefinition) -> Result<Option<String>, DesktopProblem> {
    match std::env::var(provider.environment) {
        Ok(mut value) => {
            if let Err(problem) = validate_credential(provider, &value) {
                value.zeroize();
                return Err(problem);
            }
            Ok(Some(value))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(DesktopProblem::secure_storage(format!(
            "{} must contain valid UTF-8.",
            provider.environment
        ))),
    }
}

fn environment_is_configured(provider: ProviderDefinition) -> Result<bool, DesktopProblem> {
    match environment_credential(provider)? {
        Some(mut value) => {
            value.zeroize();
            Ok(true)
        }
        None => Ok(false),
    }
}

fn load_credential(
    provider: ProviderDefinition,
    identity: DataRootIdentity,
) -> Result<Option<String>, DesktopProblem> {
    if let Some(value) = environment_credential(provider)? {
        return Ok(Some(value));
    }
    match provider_entry(provider, identity)?.get_secret() {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(mut value) => {
                if let Err(problem) = validate_credential(provider, &value) {
                    value.zeroize();
                    return Err(problem);
                }
                Ok(Some(value))
            }
            Err(error) => {
                let mut bytes = error.into_bytes();
                bytes.zeroize();
                Err(DesktopProblem::provider_credential(format!(
                    "The saved {} credential is invalid.",
                    provider.label
                )))
            }
        },
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(DesktopProblem::secure_storage(
            "Fasti could not read the system credential store.",
        )),
    }
}

fn authenticated_request(
    provider: ProviderDefinition,
    client: &reqwest::Client,
    url: reqwest::Url,
    credential: Option<String>,
) -> Result<reqwest::RequestBuilder, DesktopProblem> {
    let mut secret = credential.ok_or_else(|| {
        DesktopProblem::provider_credential(format!(
            "Add a {} credential in Settings before searching {}.",
            provider.label, provider.label
        ))
    })?;
    let header_result = match provider.kind {
        ProviderKind::GoogleBooks => HeaderValue::from_str(&secret),
        ProviderKind::Tmdb => {
            let mut bearer = String::with_capacity("Bearer ".len() + secret.len());
            bearer.push_str("Bearer ");
            bearer.push_str(&secret);
            let result = HeaderValue::from_str(&bearer);
            bearer.zeroize();
            result
        }
    };
    secret.zeroize();
    let mut header = header_result.map_err(|_| {
        DesktopProblem::provider_credential(format!(
            "The {} credential is invalid.",
            provider.label
        ))
    })?;
    header.set_sensitive(true);
    Ok(client
        .get(url)
        .header(ACCEPT, "application/json")
        .header(provider.authorization_header, header))
}

fn provider(id: &str) -> Result<ProviderDefinition, DesktopProblem> {
    PROVIDERS
        .iter()
        .copied()
        .find(|provider| provider.id == id)
        .ok_or_else(|| {
            DesktopProblem::configuration("The requested metadata provider is not supported.")
        })
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

fn validate_credential(provider: ProviderDefinition, value: &str) -> Result<(), DesktopProblem> {
    validate_credential_bytes(provider, value.as_bytes())
}

fn validate_credential_bytes(
    provider: ProviderDefinition,
    value: &[u8],
) -> Result<(), DesktopProblem> {
    if value.is_empty()
        || value.len() > CREDENTIAL_LIMIT
        || !value.iter().all(|byte| byte.is_ascii_graphic())
    {
        return Err(DesktopProblem::provider_credential(format!(
            "The {} credential must contain 1 to 512 visible ASCII characters.",
            provider.label
        )));
    }
    Ok(())
}

fn search_url(provider: ProviderDefinition, query: &str) -> Result<reqwest::Url, DesktopProblem> {
    let mut url = reqwest::Url::parse(provider.endpoint).map_err(|_| {
        DesktopProblem::provider(format!("The {} endpoint is invalid.", provider.label))
    })?;
    match provider.kind {
        ProviderKind::GoogleBooks => {
            url.query_pairs_mut()
                .append_pair("q", query)
                .append_pair("startIndex", "0")
                .append_pair("maxResults", &RESULT_LIMIT.to_string())
                .append_pair("projection", "lite");
        }
        ProviderKind::Tmdb => {
            url.query_pairs_mut()
                .append_pair("query", query)
                .append_pair("include_adult", "false")
                .append_pair("language", "en-US")
                .append_pair("page", "1");
        }
    }
    Ok(url)
}

fn parse_candidates(
    provider: ProviderDefinition,
    body: &[u8],
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    match provider.kind {
        ProviderKind::GoogleBooks => parse_google_books_candidates(body),
        ProviderKind::Tmdb => parse_tmdb_candidates(body),
    }
}

fn parse_google_books_candidates(body: &[u8]) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
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
                provider: GOOGLE_BOOKS.id,
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

fn parse_tmdb_candidates(body: &[u8]) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let response: TmdbSearchResponse = serde_json::from_slice(body)
        .map_err(|_| DesktopProblem::provider("TMDB returned invalid JSON."))?;
    let mut seen = BTreeSet::new();
    Ok(response
        .results
        .into_iter()
        .filter_map(|item| {
            if item.adult {
                return None;
            }
            let (kind, title) = match item.media_type? {
                TmdbMediaType::Movie => ("movie", item.title?),
                TmdbMediaType::Tv => ("show", item.name?),
                TmdbMediaType::Other => return None,
            };
            let id = item.id?.to_string();
            if !valid_candidate_text(&id, 32)
                || !valid_candidate_text(&title, 512)
                || !seen.insert(format!("{kind}:{id}"))
            {
                return None;
            }
            Some(ProviderCandidate {
                provider: TMDB.id,
                provider_id: id,
                title,
                kind,
                authors: Vec::new(),
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
        let candidates =
            parse_candidates(GOOGLE_BOOKS, body.as_bytes()).expect("provider candidates");

        assert_eq!(candidates.len(), RESULT_LIMIT);
        assert_eq!(candidates[0].provider, GOOGLE_BOOKS.id);
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
        let candidates = parse_candidates(GOOGLE_BOOKS, body).expect("partial response");

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
        let candidates = parse_candidates(GOOGLE_BOOKS, body).expect("deduplicated response");

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
        let candidates = parse_candidates(GOOGLE_BOOKS, body).expect("valid duplicate candidate");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].provider_id, "same");
        assert_eq!(candidates[0].title, "Valid");
    }

    #[test]
    fn credentials_and_queries_are_strictly_bounded() {
        assert!(validate_credential(GOOGLE_BOOKS, "valid-key").is_ok());
        assert!(validate_credential(TMDB, "valid-token").is_ok());
        assert!(validate_credential(GOOGLE_BOOKS, "").is_err());
        assert!(validate_credential(TMDB, "key with spaces").is_err());
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
        let url = reqwest::Url::parse(GOOGLE_BOOKS.endpoint).expect("provider URL");

        assert!(authenticated_request(GOOGLE_BOOKS, &client, url, None).is_err());
    }

    #[test]
    fn credential_is_sent_only_in_a_sensitive_header() {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client");
        let url = reqwest::Url::parse(GOOGLE_BOOKS.endpoint).expect("provider URL");
        let request = authenticated_request(GOOGLE_BOOKS, &client, url, Some("test-key".into()))
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

    #[test]
    fn tmdb_search_returns_only_bounded_neutral_film_and_series_candidates() {
        let results = (0..12)
            .map(|index| {
                let media_type = if index % 2 == 0 { "movie" } else { "tv" };
                let title = if media_type == "movie" {
                    format!(r#""title":"Film {index}""#)
                } else {
                    format!(r#""name":"Series {index}""#)
                };
                format!(
                    r#"{{"id":{},"media_type":"{media_type}",{title},"adult":false}}"#,
                    index + 1
                )
            })
            .chain([
                r#"{"id":90,"media_type":"person","name":"A Person"}"#.to_owned(),
                r#"{"id":91,"media_type":"movie","title":"Adult","adult":true}"#.to_owned(),
            ])
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(r#"{{"results":[{results}]}}"#);
        let candidates = parse_candidates(TMDB, body.as_bytes()).expect("TMDB candidates");

        assert_eq!(candidates.len(), RESULT_LIMIT);
        assert_eq!(candidates[0].provider, TMDB.id);
        assert_eq!(candidates[0].kind, "movie");
        assert_eq!(candidates[1].kind, "show");
        assert!(candidates
            .iter()
            .all(|candidate| candidate.authors.is_empty()));
        assert!(candidates
            .iter()
            .all(|candidate| candidate.image_url.is_none()));
    }

    #[test]
    fn tmdb_skips_people_adult_partial_unsafe_and_duplicate_results() {
        let body = br#"{
          "results": [
            {"id":1,"media_type":"person","name":"A Person"},
            {"id":2,"media_type":"movie","title":"Adult","adult":true},
            {"id":3,"media_type":"movie"},
            {"id":4,"media_type":"tv","name":"Bad\nTitle"},
            {"id":5,"media_type":"movie","title":"A Film"},
            {"id":5,"media_type":"movie","title":"Duplicate Film"},
            {"id":5,"media_type":"tv","name":"A Series"}
          ]
        }"#;
        let candidates = parse_candidates(TMDB, body).expect("filtered TMDB candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].title, "A Film");
        assert_eq!(candidates[0].kind, "movie");
        assert_eq!(candidates[1].title, "A Series");
        assert_eq!(candidates[1].kind, "show");
    }

    #[test]
    fn tmdb_credential_is_sent_only_in_a_sensitive_bearer_header() {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client");
        let url = search_url(TMDB, "The Bear").expect("TMDB search URL");
        let request = authenticated_request(TMDB, &client, url, Some("test-token".into()))
            .expect("authenticated request")
            .build()
            .expect("built request");
        let header = request
            .headers()
            .get("Authorization")
            .expect("credential header");

        assert_eq!(header, "Bearer test-token");
        assert!(header.is_sensitive());
        assert!(!request.url().as_str().contains("test-token"));
        assert_eq!(
            request
                .url()
                .query_pairs()
                .find(|(key, _)| key == "api_key"),
            None
        );
        assert_eq!(
            request
                .url()
                .query_pairs()
                .find(|(key, _)| key == "include_adult")
                .map(|(_, value)| value.into_owned()),
            Some("false".to_owned())
        );
    }

    #[test]
    fn provider_declarations_are_distinct_and_cannot_cross_hosts() {
        let public = ["18.160.10.1".parse().expect("public address")];
        assert!(authorize_outbound(
            TMDB.access,
            &OutboundAccessPolicy::default(),
            METADATA_SEARCH_CAPABILITY,
            TMDB.host,
            &public,
        )
        .is_ok());
        assert!(authorize_outbound(
            TMDB.access,
            &OutboundAccessPolicy::default(),
            METADATA_SEARCH_CAPABILITY,
            GOOGLE_BOOKS.host,
            &public,
        )
        .is_err());
        assert_ne!(TMDB.account, GOOGLE_BOOKS.account);
    }
}
