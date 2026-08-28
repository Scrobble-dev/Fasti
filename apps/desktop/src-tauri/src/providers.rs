use crate::outbound_http::{bounded_body, pinned_client, resolve_once};
use crate::secure_storage::{Entry, Error as KeyringError};
use crate::setup::{DesktopProblem, KEYRING_SERVICE};
use fasti_application::{
    authorize_outbound, NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy,
    ProviderMetadataField,
};
use fasti_domain::{
    ExternalIdentifierClaim, FieldClaim, FieldKey, Grain, NamespaceDefinition, NamespaceKey,
    NamespaceLicencePosture, ReceivedAt, ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY,
    POSTER_FIELD_KEY, RELEASE_YEAR_FIELD_KEY, TITLE_FIELD_KEY,
};
use fasti_store::DataRootIdentity;
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::Duration;
use zeroize::Zeroize;

pub(crate) const GOOGLE_BOOKS_PROVIDER: &str = "google-books";
const GOOGLE_BOOKS_LABEL: &str = "Google Books";
const GOOGLE_BOOKS_HOST: &str = "www.googleapis.com";
const GOOGLE_BOOKS_URL: &str = "https://www.googleapis.com/books/v1/volumes";
const GOOGLE_BOOKS_ENV: &str = "GOOGLE_BOOKS_API_KEY";
const GOOGLE_BOOKS_ACCOUNT: &str = "provider/google-books/api-key";
const GOOGLE_BOOKS_DOCS: &str = "https://developers.google.com/books/docs/v1/using";
pub(crate) const TMDB_PROVIDER: &str = "tmdb";
const TMDB_LABEL: &str = "The Movie Database (TMDB)";
const TMDB_HOST: &str = "api.themoviedb.org";
const TMDB_SEARCH_URL: &str = "https://api.themoviedb.org/3/search/multi";
const TMDB_BASE_URL: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p/w500";
const TMDB_ENV: &str = "TMDB_API_READ_ACCESS_TOKEN";
const TMDB_ACCOUNT: &str = "provider/tmdb/read-access-token";
const TMDB_DOCS: &str = "https://developer.themoviedb.org/docs/authentication-application";
const SEARCH_CAPABILITY: &str = "metadata.search";
const READ_CAPABILITY: &str = "metadata.read";
const QUERY_LIMIT: usize = 256;
const CREDENTIAL_LIMIT: usize = 512;
const RESPONSE_LIMIT: usize = 2_000_000;
const RESULT_LIMIT: usize = 10;
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(15);

const GOOGLE_BOOKS_ACCESS: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
    provider: GOOGLE_BOOKS_PROVIDER,
    capabilities: &[SEARCH_CAPABILITY, READ_CAPABILITY],
    hosts: &[GOOGLE_BOOKS_HOST],
    networks: &[NetworkClass::Public],
};

const TMDB_ACCESS: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
    provider: TMDB_PROVIDER,
    capabilities: &[SEARCH_CAPABILITY, READ_CAPABILITY],
    hosts: &[TMDB_HOST],
    networks: &[NetworkClass::Public],
};

#[derive(Clone, Copy)]
struct ProviderSpec {
    provider: &'static str,
    label: &'static str,
    environment: &'static str,
    account: &'static str,
    docs_url: &'static str,
}

const GOOGLE_BOOKS_SPEC: ProviderSpec = ProviderSpec {
    provider: GOOGLE_BOOKS_PROVIDER,
    label: GOOGLE_BOOKS_LABEL,
    environment: GOOGLE_BOOKS_ENV,
    account: GOOGLE_BOOKS_ACCOUNT,
    docs_url: GOOGLE_BOOKS_DOCS,
};

const TMDB_SPEC: ProviderSpec = ProviderSpec {
    provider: TMDB_PROVIDER,
    label: TMDB_LABEL,
    environment: TMDB_ENV,
    account: TMDB_ACCOUNT,
    docs_url: TMDB_DOCS,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderSelectionInput {
    pub(crate) provider: String,
    pub(crate) provider_id: String,
    pub(crate) kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProviderCandidate {
    pub(crate) provider: &'static str,
    pub(crate) provider_id: String,
    pub(crate) title: String,
    pub(crate) original_title: Option<String>,
    pub(crate) kind: &'static str,
    pub(crate) release_year: Option<u16>,
    pub(crate) authors: Vec<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) overview: Option<String>,
}

impl ProviderCandidate {
    pub(crate) fn grain(&self) -> Result<Grain, DesktopProblem> {
        match self.kind {
            "book" => Ok(Grain::Chapter),
            "movie" => Ok(Grain::Film),
            "show" => Ok(Grain::Series),
            _ => Err(DesktopProblem::provider(
                "The provider returned an unsupported media type.",
            )),
        }
    }

    pub(crate) fn namespace_definition(&self) -> Result<NamespaceDefinition, DesktopProblem> {
        let (label, grains, pattern) = match self.provider {
            // Keep the namespace declaration byte-for-byte compatible with
            // records created by the earlier Discover fallback.
            GOOGLE_BOOKS_PROVIDER => (GOOGLE_BOOKS_PROVIDER, vec![Grain::Chapter], ".+"),
            TMDB_PROVIDER => (TMDB_LABEL, vec![Grain::Film, Grain::Series], "[0-9]+"),
            _ => return Err(unsupported_provider()),
        };
        NamespaceDefinition::try_new(
            self.provider,
            label,
            grains,
            pattern,
            "identity",
            NamespaceLicencePosture::IdentifiersOnly,
        )
        .map_err(|_| DesktopProblem::provider("The provider namespace definition is invalid."))
    }

    pub(crate) fn identifier(&self) -> Result<ExternalIdentifierClaim, DesktopProblem> {
        ExternalIdentifierClaim::try_new(self.provider, self.grain()?, &self.provider_id)
            .map_err(|_| DesktopProblem::provider("The provider returned an invalid identifier."))
    }

    pub(crate) fn metadata_fields(&self) -> Result<Vec<ProviderMetadataField>, DesktopProblem> {
        let source = NamespaceKey::try_new(self.provider)
            .map_err(|_| DesktopProblem::provider("The provider namespace is invalid."))?;
        let fetched_at = ReceivedAt::from_application_clock(chrono::Utc::now());
        let mut fields = vec![provider_field(
            &source,
            TITLE_FIELD_KEY,
            &self.title,
            fetched_at,
        )?];
        for (key, value) in [
            (ORIGINAL_TITLE_FIELD_KEY, self.original_title.as_deref()),
            (OVERVIEW_FIELD_KEY, self.overview.as_deref()),
            (POSTER_FIELD_KEY, self.image_url.as_deref()),
        ] {
            if let Some(value) = value {
                fields.push(provider_field(&source, key, value, fetched_at)?);
            }
        }
        if let Some(year) = self.release_year {
            fields.push(provider_field(
                &source,
                RELEASE_YEAR_FIELD_KEY,
                &year.to_string(),
                fetched_at,
            )?);
        }
        Ok(fields)
    }
}

fn provider_field(
    source: &NamespaceKey,
    key: &str,
    value: &str,
    fetched_at: ReceivedAt,
) -> Result<ProviderMetadataField, DesktopProblem> {
    let field_key = FieldKey::try_new(key)
        .map_err(|_| DesktopProblem::provider("The provider field key is invalid."))?;
    let claim = FieldClaim::try_new(source.clone(), value, None, fetched_at, None)
        .map_err(|_| DesktopProblem::provider("The provider field value is invalid."))?;
    Ok(ProviderMetadataField::new(field_key, claim))
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
    #[serde(default)]
    description: Option<String>,
    #[serde(rename = "publishedDate")]
    #[serde(default)]
    published_date: Option<String>,
    #[serde(rename = "imageLinks")]
    #[serde(default)]
    image_links: Option<GoogleImageLinks>,
}

#[derive(Debug, Deserialize)]
struct GoogleImageLinks {
    #[serde(default)]
    thumbnail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TmdbSearchResponse {
    #[serde(default)]
    results: Vec<TmdbItem>,
}

#[derive(Debug, Deserialize)]
struct TmdbItem {
    id: Option<u64>,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    original_title: Option<String>,
    #[serde(default)]
    original_name: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    first_air_date: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    poster_path: Option<String>,
}

pub(crate) fn credential_statuses(
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    [GOOGLE_BOOKS_SPEC, TMDB_SPEC]
        .into_iter()
        .map(|spec| credential_status(spec, identity))
        .collect()
}

pub(crate) fn save_credential(
    mut input: SaveProviderCredentialInput,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCredentialStatus>, DesktopProblem> {
    let result = (|| {
        let spec = provider_spec(&input.provider)?;
        if environment_is_configured(spec)? {
            return Err(DesktopProblem::secure_storage(format!(
                "The {} credential is managed by {}.",
                spec.label, spec.environment
            )));
        }
        validate_credential(&input.credential)?;
        let entry = provider_entry(spec, identity)?;
        entry.set_secret(input.credential.as_bytes()).map_err(|_| {
            DesktopProblem::secure_storage(format!(
                "Fasti could not save the {} credential securely.",
                spec.label
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
    let spec = provider_spec(&input.provider)?;
    if environment_is_configured(spec)? {
        return Err(DesktopProblem::secure_storage(format!(
            "The {} credential is managed by {}.",
            spec.label, spec.environment
        )));
    }
    match provider_entry(spec, identity)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => credential_statuses(identity),
        Err(_) => Err(DesktopProblem::secure_storage(format!(
            "Fasti could not remove the {} credential.",
            spec.label
        ))),
    }
}

pub(crate) async fn search(
    input: ProviderSearchInput,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    validate_query(&input.query)?;
    match input.provider.as_str() {
        GOOGLE_BOOKS_PROVIDER => search_google_books(&input.query, policy, identity).await,
        TMDB_PROVIDER => search_tmdb(&input.query, policy, identity).await,
        _ => Err(unsupported_provider()),
    }
}

pub(crate) async fn fetch(
    provider: &str,
    provider_id: &str,
    kind: &str,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<ProviderCandidate, DesktopProblem> {
    validate_provider_id(provider_id)?;
    match provider {
        GOOGLE_BOOKS_PROVIDER if kind == "book" => {
            fetch_google_book(provider_id, policy, identity).await
        }
        TMDB_PROVIDER if matches!(kind, "movie" | "show") => {
            fetch_tmdb(provider_id, kind, policy, identity).await
        }
        GOOGLE_BOOKS_PROVIDER | TMDB_PROVIDER => Err(DesktopProblem::configuration(
            "The selected provider does not support that media type.",
        )),
        _ => Err(unsupported_provider()),
    }
}

pub(crate) async fn fetch_selection(
    input: ProviderSelectionInput,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<ProviderCandidate, DesktopProblem> {
    fetch(
        &input.provider,
        &input.provider_id,
        &input.kind,
        policy,
        identity,
    )
    .await
}

async fn search_google_books(
    query: &str,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let client = authorized_client(
        GOOGLE_BOOKS_ACCESS,
        GOOGLE_BOOKS_SPEC,
        GOOGLE_BOOKS_HOST,
        SEARCH_CAPABILITY,
        policy,
    )
    .await?;
    // Credential access follows DNS and policy checks and construction of a
    // proxy-free, redirect-free, address-pinned client.
    let credential = load_credential(GOOGLE_BOOKS_SPEC, identity)?;
    let mut url = reqwest::Url::parse(GOOGLE_BOOKS_URL)
        .map_err(|_| DesktopProblem::provider("The Google Books endpoint is invalid."))?;
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("startIndex", "0")
        .append_pair("maxResults", &RESULT_LIMIT.to_string())
        .append_pair("projection", "lite");
    let body = send_json(google_request(&client, url, credential)?, GOOGLE_BOOKS_SPEC).await?;
    parse_google_candidates(&body)
}

async fn search_tmdb(
    query: &str,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let client =
        authorized_client(TMDB_ACCESS, TMDB_SPEC, TMDB_HOST, SEARCH_CAPABILITY, policy).await?;
    let credential = load_credential(TMDB_SPEC, identity)?;
    let mut url = reqwest::Url::parse(TMDB_SEARCH_URL)
        .map_err(|_| DesktopProblem::provider("The TMDB endpoint is invalid."))?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("include_adult", "false")
        .append_pair("language", "en-US")
        .append_pair("page", "1");
    let body = send_json(tmdb_request(&client, url, credential)?, TMDB_SPEC).await?;
    parse_tmdb_candidates(&body)
}

async fn fetch_google_book(
    provider_id: &str,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<ProviderCandidate, DesktopProblem> {
    let client = authorized_client(
        GOOGLE_BOOKS_ACCESS,
        GOOGLE_BOOKS_SPEC,
        GOOGLE_BOOKS_HOST,
        READ_CAPABILITY,
        policy,
    )
    .await?;
    let credential = load_credential(GOOGLE_BOOKS_SPEC, identity)?;
    let mut url = reqwest::Url::parse(GOOGLE_BOOKS_URL)
        .map_err(|_| DesktopProblem::provider("The Google Books endpoint is invalid."))?;
    url.path_segments_mut()
        .map_err(|_| DesktopProblem::provider("The Google Books endpoint is invalid."))?
        .push(provider_id);
    url.query_pairs_mut().append_pair("projection", "full");
    let body = send_json(google_request(&client, url, credential)?, GOOGLE_BOOKS_SPEC).await?;
    let volume: GoogleVolume = serde_json::from_slice(&body)
        .map_err(|_| DesktopProblem::provider("Google Books returned invalid JSON."))?;
    let candidate = google_candidate(volume).ok_or_else(|| {
        DesktopProblem::provider("Google Books returned incomplete or unsafe metadata.")
    })?;
    verify_selected_candidate(candidate, provider_id, "book")
}

async fn fetch_tmdb(
    provider_id: &str,
    kind: &str,
    policy: &OutboundAccessPolicy,
    identity: DataRootIdentity,
) -> Result<ProviderCandidate, DesktopProblem> {
    let client =
        authorized_client(TMDB_ACCESS, TMDB_SPEC, TMDB_HOST, READ_CAPABILITY, policy).await?;
    let credential = load_credential(TMDB_SPEC, identity)?;
    let mut url = reqwest::Url::parse(TMDB_BASE_URL)
        .map_err(|_| DesktopProblem::provider("The TMDB endpoint is invalid."))?;
    url.path_segments_mut()
        .map_err(|_| DesktopProblem::provider("The TMDB endpoint is invalid."))?
        .extend([if kind == "show" { "tv" } else { "movie" }, provider_id]);
    url.query_pairs_mut().append_pair("language", "en-US");
    let body = send_json(tmdb_request(&client, url, credential)?, TMDB_SPEC).await?;
    let item: TmdbItem = serde_json::from_slice(&body)
        .map_err(|_| DesktopProblem::provider("TMDB returned invalid JSON."))?;
    let candidate = tmdb_candidate(item, Some(kind))
        .ok_or_else(|| DesktopProblem::provider("TMDB returned incomplete or unsafe metadata."))?;
    verify_selected_candidate(candidate, provider_id, kind)
}

fn verify_selected_candidate(
    candidate: ProviderCandidate,
    provider_id: &str,
    kind: &str,
) -> Result<ProviderCandidate, DesktopProblem> {
    if candidate.provider_id != provider_id || candidate.kind != kind {
        return Err(DesktopProblem::provider(
            "The provider returned a different item than requested.",
        ));
    }
    Ok(candidate)
}

async fn authorized_client(
    access: OutboundAccessDeclaration<'static>,
    spec: ProviderSpec,
    host: &str,
    capability: &str,
    policy: &OutboundAccessPolicy,
) -> Result<reqwest::Client, DesktopProblem> {
    let addresses = resolve_once(host, 443)
        .await
        .map_err(DesktopProblem::provider)?;
    let address_values = addresses.iter().map(|value| value.ip()).collect::<Vec<_>>();
    authorize_outbound(access, policy, capability, host, &address_values).map_err(|denial| {
        DesktopProblem::provider(format!(
            "The outbound policy denied the {} {}.",
            spec.label,
            denial.dimension()
        ))
    })?;
    pinned_client(host, &addresses, PROVIDER_TIMEOUT).map_err(DesktopProblem::provider)
}

async fn send_json(
    request: reqwest::RequestBuilder,
    spec: ProviderSpec,
) -> Result<Vec<u8>, DesktopProblem> {
    let response = request
        .send()
        .await
        .map_err(|_| DesktopProblem::provider(format!("{} could not be reached.", spec.label)))?;
    if matches!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        return Err(DesktopProblem::provider_credential(format!(
            "{} rejected the configured credential.",
            spec.label
        )));
    }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(DesktopProblem::provider(format!(
            "{} rate limited the request. Wait, then retry.",
            spec.label
        )));
    }
    if response.status() != reqwest::StatusCode::OK {
        return Err(DesktopProblem::provider(format!(
            "{} returned HTTP {}.",
            spec.label,
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
            spec.label
        )));
    }
    bounded_body(response, RESPONSE_LIMIT)
        .await
        .map_err(DesktopProblem::provider)
}

fn credential_status(
    spec: ProviderSpec,
    identity: DataRootIdentity,
) -> Result<ProviderCredentialStatus, DesktopProblem> {
    if environment_is_configured(spec)? {
        return Ok(status(spec, true, CredentialSource::Environment, false));
    }
    let configured = match provider_entry(spec, identity)?.get_secret() {
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
        spec,
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
    spec: ProviderSpec,
    configured: bool,
    source: CredentialSource,
    writable: bool,
) -> ProviderCredentialStatus {
    ProviderCredentialStatus {
        provider: spec.provider,
        label: spec.label,
        configured,
        source,
        writable,
        docs_url: spec.docs_url,
    }
}

fn provider_spec(provider: &str) -> Result<ProviderSpec, DesktopProblem> {
    match provider {
        GOOGLE_BOOKS_PROVIDER => Ok(GOOGLE_BOOKS_SPEC),
        TMDB_PROVIDER => Ok(TMDB_SPEC),
        _ => Err(unsupported_provider()),
    }
}

fn unsupported_provider() -> DesktopProblem {
    DesktopProblem::configuration("The requested metadata provider is not supported.")
}

fn provider_entry(spec: ProviderSpec, identity: DataRootIdentity) -> Result<Entry, DesktopProblem> {
    let account = crate::secure_storage::scoped_account(spec.account, identity);
    Entry::new(KEYRING_SERVICE, &account).map_err(|_| {
        DesktopProblem::secure_storage("Fasti could not open the system credential store.")
    })
}

fn environment_credential(spec: ProviderSpec) -> Result<Option<String>, DesktopProblem> {
    match std::env::var(spec.environment) {
        Ok(mut value) => {
            if let Err(problem) = validate_credential(&value) {
                value.zeroize();
                return Err(problem);
            }
            Ok(Some(value))
        }
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(DesktopProblem::secure_storage(format!(
            "{} must contain valid UTF-8.",
            spec.environment
        ))),
    }
}

fn environment_is_configured(spec: ProviderSpec) -> Result<bool, DesktopProblem> {
    match environment_credential(spec)? {
        Some(mut value) => {
            value.zeroize();
            Ok(true)
        }
        None => Ok(false),
    }
}

fn load_credential(
    spec: ProviderSpec,
    identity: DataRootIdentity,
) -> Result<Option<String>, DesktopProblem> {
    if let Some(value) = environment_credential(spec)? {
        return Ok(Some(value));
    }
    match provider_entry(spec, identity)?.get_secret() {
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
                Err(DesktopProblem::provider_credential(format!(
                    "The saved {} credential is invalid.",
                    spec.label
                )))
            }
        },
        Err(KeyringError::NoEntry) => Ok(None),
        Err(_) => Err(DesktopProblem::secure_storage(
            "Fasti could not read the system credential store.",
        )),
    }
}

fn google_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    credential: Option<String>,
) -> Result<reqwest::RequestBuilder, DesktopProblem> {
    sensitive_header_request(
        client,
        url,
        credential,
        "X-Goog-Api-Key",
        GOOGLE_BOOKS_LABEL,
    )
}

fn tmdb_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    credential: Option<String>,
) -> Result<reqwest::RequestBuilder, DesktopProblem> {
    let mut secret = credential.ok_or_else(|| {
        DesktopProblem::provider_credential(
            "Add a TMDB API Read Access Token in Settings before searching TMDB.",
        )
    })?;
    secret.insert_str(0, "Bearer ");
    let header_result = HeaderValue::from_str(&secret);
    secret.zeroize();
    let mut header = header_result
        .map_err(|_| DesktopProblem::provider_credential("The TMDB credential is invalid."))?;
    header.set_sensitive(true);
    Ok(client.get(url).header(AUTHORIZATION, header))
}

fn sensitive_header_request(
    client: &reqwest::Client,
    url: reqwest::Url,
    credential: Option<String>,
    header_name: &'static str,
    label: &str,
) -> Result<reqwest::RequestBuilder, DesktopProblem> {
    let mut secret = credential.ok_or_else(|| {
        DesktopProblem::provider_credential(format!(
            "Add a {label} credential in Settings before searching {label}."
        ))
    })?;
    let header_result = HeaderValue::from_str(&secret);
    secret.zeroize();
    let mut header = header_result.map_err(|_| {
        DesktopProblem::provider_credential(format!("The {label} credential is invalid."))
    })?;
    header.set_sensitive(true);
    Ok(client.get(url).header(header_name, header))
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
            "A provider credential must contain 1 to 512 visible ASCII characters.",
        ));
    }
    Ok(())
}

fn validate_provider_id(provider_id: &str) -> Result<(), DesktopProblem> {
    if valid_candidate_text(provider_id, 256) {
        Ok(())
    } else {
        Err(DesktopProblem::configuration(
            "The provider ID must contain 1 to 256 bytes without leading, trailing, or control characters.",
        ))
    }
}

fn parse_google_candidates(body: &[u8]) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let response: GoogleVolumesResponse = serde_json::from_slice(body)
        .map_err(|_| DesktopProblem::provider("Google Books returned invalid JSON."))?;
    let mut seen = BTreeSet::new();
    Ok(response
        .items
        .into_iter()
        .filter_map(google_candidate)
        .filter(|candidate| seen.insert(candidate.provider_id.clone()))
        .take(RESULT_LIMIT)
        .collect())
}

fn google_candidate(item: GoogleVolume) -> Option<ProviderCandidate> {
    let provider_id = item.id?;
    let volume_info = item.volume_info?;
    let title = volume_info.title?;
    if !valid_candidate_text(&provider_id, 256)
        || !valid_candidate_text(&title, 512)
        || volume_info.authors.len() > 10
        || volume_info
            .authors
            .iter()
            .any(|author| !valid_candidate_text(author, 128))
    {
        return None;
    }
    Some(ProviderCandidate {
        provider: GOOGLE_BOOKS_PROVIDER,
        provider_id,
        title,
        original_title: None,
        kind: "book",
        release_year: volume_info.published_date.as_deref().and_then(release_year),
        authors: volume_info.authors,
        image_url: volume_info
            .image_links
            .and_then(|links| links.thumbnail)
            .and_then(normalize_google_image),
        overview: volume_info
            .description
            .filter(|value| valid_candidate_text(value, 4096)),
    })
}

fn parse_tmdb_candidates(body: &[u8]) -> Result<Vec<ProviderCandidate>, DesktopProblem> {
    let response: TmdbSearchResponse = serde_json::from_slice(body)
        .map_err(|_| DesktopProblem::provider("TMDB returned invalid JSON."))?;
    let mut seen = BTreeSet::new();
    Ok(response
        .results
        .into_iter()
        .filter_map(|item| tmdb_candidate(item, None))
        .filter(|candidate| seen.insert((candidate.kind, candidate.provider_id.clone())))
        .take(RESULT_LIMIT)
        .collect())
}

fn tmdb_candidate(item: TmdbItem, forced_kind: Option<&str>) -> Option<ProviderCandidate> {
    let kind = match forced_kind.or(item.media_type.as_deref())? {
        "movie" => "movie",
        "tv" | "show" => "show",
        _ => return None,
    };
    let provider_id = item.id?.to_string();
    let title = if kind == "show" {
        item.name
    } else {
        item.title
    }?;
    let original_title = if kind == "show" {
        item.original_name
    } else {
        item.original_title
    }
    .filter(|value| value != &title && valid_candidate_text(value, 512));
    if !valid_candidate_text(&title, 512) {
        return None;
    }
    let date = if kind == "show" {
        item.first_air_date
    } else {
        item.release_date
    };
    let overview = item
        .overview
        .filter(|value| !value.is_empty() && valid_candidate_text(value, 4096));
    let image_url = item.poster_path.and_then(|path| {
        (path.starts_with('/') && path.len() <= 256 && !path.chars().any(char::is_control))
            .then(|| format!("{TMDB_IMAGE_BASE_URL}{path}"))
    });
    Some(ProviderCandidate {
        provider: TMDB_PROVIDER,
        provider_id,
        title,
        original_title,
        kind,
        release_year: date.as_deref().and_then(release_year),
        authors: Vec::new(),
        image_url,
        overview,
    })
}

fn release_year(value: &str) -> Option<u16> {
    if value.len() > 4 && value.as_bytes().get(4) != Some(&b'-') {
        return None;
    }
    let year = value.get(..4)?.parse::<u16>().ok()?;
    (1000..=9999).contains(&year).then_some(year)
}

fn normalize_google_image(value: String) -> Option<String> {
    let mut url = reqwest::Url::parse(&value).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !matches!(
            url.host_str(),
            Some("books.google.com" | "books.googleusercontent.com")
        )
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    url.set_scheme("https").ok()?;
    url.set_fragment(None);
    let normalized = url.to_string();
    (normalized.len() <= 2048).then_some(normalized)
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
        let candidates = parse_google_candidates(body.as_bytes()).expect("provider candidates");

        assert_eq!(candidates.len(), RESULT_LIMIT);
        assert_eq!(candidates[0].provider, GOOGLE_BOOKS_PROVIDER);
        assert_eq!(candidates[0].kind, "book");
        assert!(candidates[0].image_url.is_none());
        let namespace = candidates[0]
            .namespace_definition()
            .expect("legacy-compatible namespace");
        assert_eq!(namespace.label(), GOOGLE_BOOKS_PROVIDER);
        assert_eq!(namespace.id_pattern(), ".+");
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
        let candidates = parse_google_candidates(body).expect("partial response");

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
        let candidates = parse_google_candidates(body).expect("deduplicated response");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "First");
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

        assert!(google_request(&client, url, None).is_err());
    }

    #[test]
    fn credential_is_sent_only_in_a_sensitive_header() {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client");
        let url = reqwest::Url::parse(GOOGLE_BOOKS_URL).expect("provider URL");
        let request = google_request(&client, url, Some("test-key".into()))
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
    fn tmdb_candidates_keep_movie_and_show_identity_distinct() {
        let body = br#"{
          "results": [
            {"id": 42, "media_type": "movie", "title": "A Film", "original_title": "Original Film", "release_date": "2025-04-03", "overview": "Film overview", "poster_path": "/film.jpg"},
            {"id": 42, "media_type": "tv", "name": "A Show", "first_air_date": "2024-01-02", "poster_path": "/show.jpg"},
            {"id": 7, "media_type": "person", "name": "Not media"}
          ]
        }"#;
        let candidates = parse_tmdb_candidates(body).expect("TMDB candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].kind, "movie");
        assert_eq!(candidates[0].release_year, Some(2025));
        assert_eq!(
            candidates[0].original_title.as_deref(),
            Some("Original Film")
        );
        assert_eq!(
            candidates[0].image_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w500/film.jpg")
        );
        assert_eq!(candidates[1].kind, "show");
        assert_eq!(release_year("2025oops"), None);
        assert!(verify_selected_candidate(candidates[0].clone(), "42", "movie").is_ok());
        assert!(verify_selected_candidate(candidates[0].clone(), "7", "movie").is_err());
        assert!(verify_selected_candidate(candidates[0].clone(), "42", "show").is_err());
    }

    #[test]
    fn tmdb_token_is_header_only_and_sensitive() {
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test client");
        let url = reqwest::Url::parse(TMDB_SEARCH_URL).expect("provider URL");
        let request = tmdb_request(&client, url, Some("test-token".into()))
            .expect("authenticated request")
            .build()
            .expect("built request");
        let header = request
            .headers()
            .get(AUTHORIZATION)
            .expect("authorization header");

        assert_eq!(header, "Bearer test-token");
        assert!(header.is_sensitive());
        assert!(!request.url().as_str().contains("test-token"));
    }
}
