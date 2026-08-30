use crate::transport::{bounded_body, GovernedTransport};
use crate::ProviderRuntimeError;
use fasti_application::{
    provider_identity_mapping, ConfigurationDigest, CredentialReference, CredentialRequirement,
    CredentialSecret, CredentialVaultError, CredentialVaultPort, CredentialVaultSource,
    NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy, ProviderCapabilityState,
    ProviderCapabilityStatus, ProviderCheckKind, ProviderCredentialStatus, ProviderIdentityMapping,
    ProviderMetadataField, StoredCredential, GOOGLE_BOOKS_PROVIDER_ID, TMDB_PROVIDER_ID,
};
use fasti_domain::{
    ExternalIdentifierClaim, FieldClaim, FieldKey, Grain, NamespaceDefinition, NamespaceKey,
    ReceivedAt, ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY, POSTER_FIELD_KEY,
    RELEASE_YEAR_FIELD_KEY, TITLE_FIELD_KEY,
};
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

pub const GOOGLE_BOOKS_PROVIDER: &str = GOOGLE_BOOKS_PROVIDER_ID;
const GOOGLE_BOOKS_LABEL: &str = "Google Books";
const GOOGLE_BOOKS_HOST: &str = "www.googleapis.com";
const GOOGLE_BOOKS_URL: &str = "https://www.googleapis.com/books/v1/volumes";
const GOOGLE_BOOKS_ENV: &str = "GOOGLE_BOOKS_API_KEY";
const GOOGLE_BOOKS_ACCOUNT: &str = "provider/google-books/api-key";
const GOOGLE_BOOKS_DOCS: &str = "https://developers.google.com/books/docs/v1/using";
pub const TMDB_PROVIDER: &str = TMDB_PROVIDER_ID;
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
const RESPONSE_LIMIT: usize = 2_000_000;
const RESULT_LIMIT: usize = 10;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderRequestLimits {
    pub query_bytes: usize,
    pub response_bytes: usize,
    pub result_count: usize,
    pub timeout_seconds: u64,
    pub shared_concurrency: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderCapabilitySpec {
    pub capability_id: &'static str,
    pub credential_requirement: CredentialRequirement,
    pub health_test: bool,
    pub credential_test: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProviderSpec {
    pub provider: &'static str,
    pub label: &'static str,
    pub kind: ProviderKind,
    pub environment: &'static str,
    pub account: &'static str,
    pub docs_url: &'static str,
    pub attribution: &'static str,
    pub media_grains: &'static [&'static str],
    pub capabilities: &'static [ProviderCapabilitySpec],
    pub network_hosts: &'static [&'static str],
    pub identity_namespaces: &'static [&'static str],
    pub locale_support: &'static str,
    pub region_support: &'static str,
    pub rate_limit_policy: &'static str,
    pub cache_policy: &'static str,
    pub offline_behavior: &'static str,
    pub licence_and_terms: &'static str,
    pub request_limits: ProviderRequestLimits,
    pub runtime_available: bool,
}

const REQUEST_LIMITS: ProviderRequestLimits = ProviderRequestLimits {
    query_bytes: QUERY_LIMIT,
    response_bytes: RESPONSE_LIMIT,
    result_count: RESULT_LIMIT,
    timeout_seconds: 15,
    shared_concurrency: 4,
};

const GOOGLE_BOOKS_CAPABILITIES: &[ProviderCapabilitySpec] = &[
    ProviderCapabilitySpec {
        capability_id: SEARCH_CAPABILITY,
        credential_requirement: CredentialRequirement::ApiKey,
        health_test: true,
        credential_test: true,
    },
    ProviderCapabilitySpec {
        capability_id: READ_CAPABILITY,
        credential_requirement: CredentialRequirement::ApiKey,
        health_test: true,
        credential_test: true,
    },
];

const TMDB_CAPABILITIES: &[ProviderCapabilitySpec] = &[
    ProviderCapabilitySpec {
        capability_id: SEARCH_CAPABILITY,
        credential_requirement: CredentialRequirement::BearerToken,
        health_test: true,
        credential_test: true,
    },
    ProviderCapabilitySpec {
        capability_id: READ_CAPABILITY,
        credential_requirement: CredentialRequirement::BearerToken,
        health_test: true,
        credential_test: true,
    },
];

macro_rules! unavailable_capabilities {
    ($name:ident, $requirement:expr) => {
        const $name: &[ProviderCapabilitySpec] = &[
            ProviderCapabilitySpec {
                capability_id: SEARCH_CAPABILITY,
                credential_requirement: $requirement,
                health_test: false,
                credential_test: false,
            },
            ProviderCapabilitySpec {
                capability_id: READ_CAPABILITY,
                credential_requirement: $requirement,
                health_test: false,
                credential_test: false,
            },
        ];
    };
}

unavailable_capabilities!(OPEN_LIBRARY_CAPABILITIES, CredentialRequirement::None);
unavailable_capabilities!(KITSU_CAPABILITIES, CredentialRequirement::None);
unavailable_capabilities!(ANILIST_CAPABILITIES, CredentialRequirement::None);
unavailable_capabilities!(
    MUSICBRAINZ_CAPABILITIES,
    CredentialRequirement::UserAgentOnly
);
unavailable_capabilities!(TVDB_CAPABILITIES, CredentialRequirement::ApiKey);
unavailable_capabilities!(MAL_CAPABILITIES, CredentialRequirement::Oauth2);
unavailable_capabilities!(RAWG_CAPABILITIES, CredentialRequirement::ApiKey);
unavailable_capabilities!(IGDB_CAPABILITIES, CredentialRequirement::Oauth2);
unavailable_capabilities!(COMICVINE_CAPABILITIES, CredentialRequirement::ApiKey);
unavailable_capabilities!(
    PODCAST_INDEX_CAPABILITIES,
    CredentialRequirement::CustomHeader
);

const GOOGLE_BOOKS_SPEC: ProviderSpec = ProviderSpec {
    provider: GOOGLE_BOOKS_PROVIDER,
    label: GOOGLE_BOOKS_LABEL,
    kind: ProviderKind::Metadata,
    environment: GOOGLE_BOOKS_ENV,
    account: GOOGLE_BOOKS_ACCOUNT,
    docs_url: GOOGLE_BOOKS_DOCS,
    attribution: "Metadata from Google Books",
    media_grains: &["edition"],
    capabilities: GOOGLE_BOOKS_CAPABILITIES,
    network_hosts: &[GOOGLE_BOOKS_HOST],
    identity_namespaces: &["googlebooks.volume"],
    locale_support: "provider_default",
    region_support: "not_supported",
    rate_limit_policy: "respect_provider_responses",
    cache_policy: "no_runtime_cache",
    offline_behavior: "fail_without_mutating_local_state",
    licence_and_terms: "operator_review_required_before_activation",
    request_limits: REQUEST_LIMITS,
    runtime_available: true,
};

const TMDB_SPEC: ProviderSpec = ProviderSpec {
    provider: TMDB_PROVIDER,
    label: TMDB_LABEL,
    kind: ProviderKind::Metadata,
    environment: TMDB_ENV,
    account: TMDB_ACCOUNT,
    docs_url: TMDB_DOCS,
    attribution: "This product uses the TMDB API but is not endorsed or certified by TMDB.",
    media_grains: &["film", "series"],
    capabilities: TMDB_CAPABILITIES,
    network_hosts: &[TMDB_HOST],
    identity_namespaces: &["tmdb.movie", "tmdb.tv"],
    locale_support: "en-US",
    region_support: "not_configured_in_m1",
    rate_limit_policy: "respect_provider_responses",
    cache_policy: "no_runtime_cache",
    offline_behavior: "fail_without_mutating_local_state",
    licence_and_terms: "tmdb_attribution_required",
    request_limits: REQUEST_LIMITS,
    runtime_available: true,
};

const fn unavailable_provider(
    provider: &'static str,
    label: &'static str,
    docs_url: &'static str,
    media_grains: &'static [&'static str],
    capabilities: &'static [ProviderCapabilitySpec],
) -> ProviderSpec {
    ProviderSpec {
        provider,
        label,
        kind: ProviderKind::Metadata,
        environment: "",
        account: "",
        docs_url,
        attribution: "Activation requires a reviewed provider-specific attribution policy.",
        media_grains,
        capabilities,
        network_hosts: &[],
        identity_namespaces: &[],
        locale_support: "unavailable",
        region_support: "unavailable",
        rate_limit_policy: "unavailable",
        cache_policy: "no_runtime_cache",
        offline_behavior: "unavailable_without_mutating_local_state",
        licence_and_terms: "not_reviewed_for_activation",
        request_limits: REQUEST_LIMITS,
        runtime_available: false,
    }
}

const OPEN_LIBRARY_SPEC: ProviderSpec = unavailable_provider(
    "open-library",
    "Open Library (Books)",
    "https://openlibrary.org/developers/api",
    &["edition", "work"],
    OPEN_LIBRARY_CAPABILITIES,
);
const KITSU_SPEC: ProviderSpec = unavailable_provider(
    "kitsu",
    "Kitsu (Anime & Manga)",
    "https://kitsu.docs.apiary.io",
    &["film", "series", "edition", "work"],
    KITSU_CAPABILITIES,
);
const ANILIST_SPEC: ProviderSpec = unavailable_provider(
    "anilist",
    "AniList GraphQL (Anime/Manga)",
    "https://docs.anilist.co",
    &["film", "series", "edition", "work"],
    ANILIST_CAPABILITIES,
);
const MUSICBRAINZ_SPEC: ProviderSpec = unavailable_provider(
    "musicbrainz",
    "MusicBrainz (Music)",
    "https://musicbrainz.org/doc/MusicBrainz_API",
    &["release", "track"],
    MUSICBRAINZ_CAPABILITIES,
);
const TVDB_SPEC: ProviderSpec = unavailable_provider(
    "tvdb",
    "TheTVDB v4",
    "https://thetvdb.com/api-information",
    &["film", "series", "episode"],
    TVDB_CAPABILITIES,
);
const MAL_SPEC: ProviderSpec = unavailable_provider(
    "mal",
    "MyAnimeList API v2",
    "https://myanimelist.net/apiconfig/references/api/v2",
    &["film", "series", "edition", "work"],
    MAL_CAPABILITIES,
);
const RAWG_SPEC: ProviderSpec = unavailable_provider(
    "rawg",
    "RAWG Video Games Database",
    "https://rawg.io/apidocs",
    &["game_release"],
    RAWG_CAPABILITIES,
);
const IGDB_SPEC: ProviderSpec = unavailable_provider(
    "igdb",
    "IGDB (Games)",
    "https://api-docs.igdb.com",
    &["game_release"],
    IGDB_CAPABILITIES,
);
const COMICVINE_SPEC: ProviderSpec = unavailable_provider(
    "comicvine",
    "ComicVine (Comics)",
    "https://comicvine.gamespot.com/api/documentation",
    &["edition", "work"],
    COMICVINE_CAPABILITIES,
);
const PODCAST_INDEX_SPEC: ProviderSpec = unavailable_provider(
    "podcast-index",
    "Podcast Index (Podcasts)",
    "https://podcastindex-org.github.io/docs-api",
    &["podcast_feed", "podcast_episode"],
    PODCAST_INDEX_CAPABILITIES,
);

pub const fn registry() -> &'static [ProviderSpec] {
    &[
        OPEN_LIBRARY_SPEC,
        KITSU_SPEC,
        ANILIST_SPEC,
        MUSICBRAINZ_SPEC,
        TMDB_SPEC,
        TVDB_SPEC,
        GOOGLE_BOOKS_SPEC,
        MAL_SPEC,
        RAWG_SPEC,
        IGDB_SPEC,
        COMICVINE_SPEC,
        PODCAST_INDEX_SPEC,
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSearchInput {
    pub provider: String,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSelectionInput {
    pub provider: String,
    pub provider_id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderCandidate {
    pub provider: &'static str,
    pub provider_id: String,
    pub title: String,
    pub original_title: Option<String>,
    pub kind: &'static str,
    pub release_year: Option<u16>,
    pub authors: Vec<String>,
    pub image_url: Option<String>,
    pub overview: Option<String>,
}

impl ProviderCandidate {
    fn identity_mapping(&self) -> Result<ProviderIdentityMapping, ProviderRuntimeError> {
        provider_identity_mapping(self.provider, self.kind).ok_or_else(|| {
            ProviderRuntimeError::response_invalid(
                "The provider returned an unsupported media type.",
            )
        })
    }

    pub fn grain(&self) -> Result<Grain, ProviderRuntimeError> {
        Ok(self.identity_mapping()?.grain())
    }

    pub fn namespace_definition(&self) -> Result<NamespaceDefinition, ProviderRuntimeError> {
        self.identity_mapping()?
            .namespace_definition()
            .map_err(|_| {
                ProviderRuntimeError::response_invalid(
                    "The provider namespace definition is invalid.",
                )
            })
    }

    pub fn identifier(&self) -> Result<ExternalIdentifierClaim, ProviderRuntimeError> {
        self.identity_mapping()?
            .identifier(&self.provider_id)
            .map_err(|_| {
                ProviderRuntimeError::provider("The provider returned an invalid identifier.")
            })
    }

    pub fn metadata_fields(&self) -> Result<Vec<ProviderMetadataField>, ProviderRuntimeError> {
        let source = NamespaceKey::try_new(self.identity_mapping()?.namespace()).map_err(|_| {
            ProviderRuntimeError::response_invalid("The provider namespace is invalid.")
        })?;
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
) -> Result<ProviderMetadataField, ProviderRuntimeError> {
    let field_key = FieldKey::try_new(key)
        .map_err(|_| ProviderRuntimeError::provider("The provider field key is invalid."))?;
    let claim = FieldClaim::try_new(source.clone(), value, None, fetched_at, None)
        .map_err(|_| ProviderRuntimeError::provider("The provider field value is invalid."))?;
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
    adult: Option<bool>,
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

pub struct ProviderRuntime {
    transport: GovernedTransport,
    vault: Arc<dyn CredentialVaultPort>,
}

impl ProviderRuntime {
    pub fn new(vault: Arc<dyn CredentialVaultPort>) -> Self {
        Self {
            transport: GovernedTransport::default(),
            vault,
        }
    }

    pub fn with_transport(
        vault: Arc<dyn CredentialVaultPort>,
        transport: GovernedTransport,
    ) -> Self {
        Self { transport, vault }
    }

    pub const fn descriptors(&self) -> &'static [ProviderSpec] {
        registry()
    }

    pub const fn transport(&self) -> &GovernedTransport {
        &self.transport
    }

    pub fn descriptor(
        &self,
        provider: &str,
    ) -> Result<&'static ProviderSpec, ProviderRuntimeError> {
        registry()
            .iter()
            .find(|spec| spec.provider == provider)
            .ok_or_else(unsupported_provider)
    }

    pub fn credential_reference(
        &self,
        provider: &str,
    ) -> Result<CredentialReference, ProviderRuntimeError> {
        let spec = self.active_spec(provider)?;
        CredentialReference::try_new(spec.account).map_err(|_| {
            ProviderRuntimeError::configuration("The provider credential reference is invalid.")
        })
    }

    pub fn configuration_digest(
        &self,
        provider: &str,
        capability: &str,
    ) -> Result<ConfigurationDigest, ProviderRuntimeError> {
        self.active_spec(provider)?;
        let (_, endpoint) = endpoint(provider, capability)?;
        let digest = crate::configuration_digest(provider, capability, &endpoint)
            .map_err(ProviderRuntimeError::configuration)?;
        ConfigurationDigest::parse(digest).map_err(|_| {
            ProviderRuntimeError::configuration("The provider configuration digest is invalid.")
        })
    }

    pub fn store_credential(
        &self,
        reference: &CredentialReference,
        secret: CredentialSecret,
    ) -> Result<StoredCredential, ProviderRuntimeError> {
        self.vault.store(reference, secret).map_err(vault_error)
    }

    pub fn credential_source(
        &self,
        reference: &CredentialReference,
    ) -> Result<CredentialVaultSource, ProviderRuntimeError> {
        self.vault.source(reference).map_err(vault_error)
    }

    pub fn replace_credential(
        &self,
        reference: &CredentialReference,
        secret: CredentialSecret,
    ) -> Result<StoredCredential, ProviderRuntimeError> {
        self.vault.replace(reference, secret).map_err(vault_error)
    }

    pub fn revoke_credential(
        &self,
        reference: &CredentialReference,
    ) -> Result<(), ProviderRuntimeError> {
        self.vault.revoke(reference).map_err(vault_error)
    }

    pub async fn search(
        &self,
        input: ProviderSearchInput,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<Vec<ProviderCandidate>, ProviderRuntimeError> {
        validate_query(&input.query)?;
        match input.provider.as_str() {
            GOOGLE_BOOKS_PROVIDER => self.search_google_books(&input.query, policy, state).await,
            TMDB_PROVIDER => self.search_tmdb(&input.query, policy, state).await,
            _ => Err(unsupported_provider()),
        }
    }

    pub async fn fetch_selection(
        &self,
        input: ProviderSelectionInput,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<ProviderCandidate, ProviderRuntimeError> {
        let mapping = provider_identity_mapping(&input.provider, &input.kind).ok_or_else(|| {
            if matches!(
                input.provider.as_str(),
                GOOGLE_BOOKS_PROVIDER | TMDB_PROVIDER
            ) {
                ProviderRuntimeError::configuration(
                    "The selected provider does not support that media type.",
                )
            } else {
                unsupported_provider()
            }
        })?;
        mapping.identifier(&input.provider_id).map_err(|_| {
            ProviderRuntimeError::configuration("The selected provider ID is invalid.")
        })?;
        match input.provider.as_str() {
            GOOGLE_BOOKS_PROVIDER => {
                self.fetch_google_book(&input.provider_id, policy, state)
                    .await
            }
            TMDB_PROVIDER => {
                self.fetch_tmdb(&input.provider_id, &input.kind, policy, state)
                    .await
            }
            _ => Err(unsupported_provider()),
        }
    }

    pub async fn check(
        &self,
        provider: &str,
        kind: ProviderCheckKind,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<(), ProviderRuntimeError> {
        let spec = self.active_spec(provider)?;
        let capability = spec
            .capabilities
            .iter()
            .find(|entry| {
                entry.capability_id == state.capability_id().as_str()
                    && match kind {
                        ProviderCheckKind::Health => entry.health_test,
                        ProviderCheckKind::Credential => entry.credential_test,
                    }
            })
            .map(|entry| entry.capability_id)
            .ok_or_else(|| {
                ProviderRuntimeError::configuration("The provider does not support that check.")
            })?;
        let (access, endpoint) = endpoint(provider, capability)?;
        let client = self
            .authorized_credential(access, spec, capability, endpoint.clone(), policy, state)
            .await?;
        let url = match provider {
            GOOGLE_BOOKS_PROVIDER => {
                let mut url = endpoint;
                url.query_pairs_mut()
                    .append_pair("q", "isbn:9780140328721")
                    .append_pair("maxResults", "1");
                url
            }
            TMDB_PROVIDER => reqwest::Url::parse("https://api.themoviedb.org/3/configuration")
                .map_err(|_| {
                    ProviderRuntimeError::configuration("The TMDB check URL is invalid.")
                })?,
            _ => return Err(unsupported_provider()),
        };
        let credential = self.load_bound_credential(&client, spec, state)?;
        let request = credential_request(provider, &client, url, &credential)?;
        let body = send_json(request, spec).await?;
        serde_json::from_slice::<serde_json::Value>(&body)
            .map(|_| ())
            .map_err(|_| {
                ProviderRuntimeError::response_invalid("The provider check returned invalid JSON.")
            })
    }

    async fn search_google_books(
        &self,
        query: &str,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<Vec<ProviderCandidate>, ProviderRuntimeError> {
        let mut url = reqwest::Url::parse(GOOGLE_BOOKS_URL)
            .map_err(|_| ProviderRuntimeError::provider("The Google Books endpoint is invalid."))?;
        let client = self
            .authorized_credential(
                GOOGLE_BOOKS_ACCESS,
                GOOGLE_BOOKS_SPEC,
                SEARCH_CAPABILITY,
                url.clone(),
                policy,
                state,
            )
            .await?;
        let credential = self.load_bound_credential(&client, GOOGLE_BOOKS_SPEC, state)?;
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("startIndex", "0")
            .append_pair("maxResults", &RESULT_LIMIT.to_string())
            .append_pair("projection", "lite");
        let body = send_json(
            credential_request(GOOGLE_BOOKS_PROVIDER, &client, url, &credential)?,
            GOOGLE_BOOKS_SPEC,
        )
        .await?;
        parse_google_candidates(&body)
    }

    async fn search_tmdb(
        &self,
        query: &str,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<Vec<ProviderCandidate>, ProviderRuntimeError> {
        let mut url = reqwest::Url::parse(TMDB_SEARCH_URL)
            .map_err(|_| ProviderRuntimeError::provider("The TMDB endpoint is invalid."))?;
        let client = self
            .authorized_credential(
                TMDB_ACCESS,
                TMDB_SPEC,
                SEARCH_CAPABILITY,
                url.clone(),
                policy,
                state,
            )
            .await?;
        let credential = self.load_bound_credential(&client, TMDB_SPEC, state)?;
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("include_adult", "false")
            .append_pair("language", "en-US")
            .append_pair("page", "1");
        let body = send_json(
            credential_request(TMDB_PROVIDER, &client, url, &credential)?,
            TMDB_SPEC,
        )
        .await?;
        parse_tmdb_candidates(&body)
    }

    async fn fetch_google_book(
        &self,
        provider_id: &str,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<ProviderCandidate, ProviderRuntimeError> {
        let mut url = reqwest::Url::parse(GOOGLE_BOOKS_URL)
            .map_err(|_| ProviderRuntimeError::provider("The Google Books endpoint is invalid."))?;
        let client = self
            .authorized_credential(
                GOOGLE_BOOKS_ACCESS,
                GOOGLE_BOOKS_SPEC,
                READ_CAPABILITY,
                url.clone(),
                policy,
                state,
            )
            .await?;
        let credential = self.load_bound_credential(&client, GOOGLE_BOOKS_SPEC, state)?;
        url.path_segments_mut()
            .map_err(|_| ProviderRuntimeError::provider("The Google Books endpoint is invalid."))?
            .push(provider_id);
        url.query_pairs_mut().append_pair("projection", "full");
        let body = send_json(
            credential_request(GOOGLE_BOOKS_PROVIDER, &client, url, &credential)?,
            GOOGLE_BOOKS_SPEC,
        )
        .await?;
        let volume: GoogleVolume = serde_json::from_slice(&body).map_err(|_| {
            ProviderRuntimeError::response_invalid("Google Books returned invalid JSON.")
        })?;
        let candidate = google_candidate(volume).ok_or_else(|| {
            ProviderRuntimeError::response_invalid(
                "Google Books returned incomplete or unsafe metadata.",
            )
        })?;
        verify_selected_candidate(candidate, provider_id, "book")
    }

    async fn fetch_tmdb(
        &self,
        provider_id: &str,
        kind: &str,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<ProviderCandidate, ProviderRuntimeError> {
        let mut url = reqwest::Url::parse(TMDB_BASE_URL)
            .map_err(|_| ProviderRuntimeError::provider("The TMDB endpoint is invalid."))?;
        let client = self
            .authorized_credential(
                TMDB_ACCESS,
                TMDB_SPEC,
                READ_CAPABILITY,
                url.clone(),
                policy,
                state,
            )
            .await?;
        let credential = self.load_bound_credential(&client, TMDB_SPEC, state)?;
        url.path_segments_mut()
            .map_err(|_| ProviderRuntimeError::provider("The TMDB endpoint is invalid."))?
            .extend([if kind == "show" { "tv" } else { "movie" }, provider_id]);
        url.query_pairs_mut().append_pair("language", "en-US");
        let body = send_json(
            credential_request(TMDB_PROVIDER, &client, url, &credential)?,
            TMDB_SPEC,
        )
        .await?;
        let item: TmdbItem = serde_json::from_slice(&body)
            .map_err(|_| ProviderRuntimeError::response_invalid("TMDB returned invalid JSON."))?;
        let candidate = tmdb_candidate(item, Some(kind)).ok_or_else(|| {
            ProviderRuntimeError::response_invalid("TMDB returned incomplete or unsafe metadata.")
        })?;
        verify_selected_candidate(candidate, provider_id, kind)
    }

    async fn authorized_credential(
        &self,
        access: OutboundAccessDeclaration<'static>,
        spec: ProviderSpec,
        capability: &'static str,
        endpoint: reqwest::Url,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<crate::AuthorizedClient, ProviderRuntimeError> {
        validate_state(spec, capability, state)?;
        self.transport
            .authorize(access, policy, capability, &endpoint)
            .await
            .map_err(ProviderRuntimeError::network)
    }

    fn load_bound_credential(
        &self,
        client: &crate::AuthorizedClient,
        spec: ProviderSpec,
        state: &ProviderCapabilityState,
    ) -> Result<CredentialSecret, ProviderRuntimeError> {
        if state.configuration_digest().as_str() != client.configuration_digest() {
            return Err(ProviderRuntimeError::configuration(format!(
                "The {} provider configuration changed after authorization.",
                spec.label
            )));
        }
        let reference = state.credential_reference().ok_or_else(|| {
            ProviderRuntimeError::credential_missing(format!(
                "Add a {} credential before using this provider capability.",
                spec.label
            ))
        })?;
        // The only vault read occurs after DNS, policy, origin, and address pinning.
        self.vault.load(reference).map_err(vault_error)
    }

    fn active_spec(&self, provider: &str) -> Result<ProviderSpec, ProviderRuntimeError> {
        let spec = *self.descriptor(provider)?;
        if !spec.runtime_available {
            return Err(ProviderRuntimeError::configuration(
                "The requested metadata provider is not available in this runtime.",
            ));
        }
        Ok(spec)
    }
}

fn verify_selected_candidate(
    candidate: ProviderCandidate,
    provider_id: &str,
    kind: &str,
) -> Result<ProviderCandidate, ProviderRuntimeError> {
    if candidate.provider_id != provider_id || candidate.kind != kind {
        return Err(ProviderRuntimeError::response_invalid(
            "The provider returned a different item than requested.",
        ));
    }
    Ok(candidate)
}

async fn send_json(
    request: reqwest::RequestBuilder,
    spec: ProviderSpec,
) -> Result<Vec<u8>, ProviderRuntimeError> {
    let response = request.send().await.map_err(|_| {
        ProviderRuntimeError::provider(format!("{} could not be reached.", spec.label))
    })?;
    if matches!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) {
        return Err(ProviderRuntimeError::credential(format!(
            "{} rejected the configured credential.",
            spec.label
        )));
    }
    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(ProviderRuntimeError::rate_limited(format!(
            "{} rate limited the request. Wait, then retry.",
            spec.label
        )));
    }
    if response.status() != reqwest::StatusCode::OK {
        return Err(ProviderRuntimeError::response_invalid(format!(
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
        return Err(ProviderRuntimeError::response_invalid(format!(
            "{} returned an unexpected content type.",
            spec.label
        )));
    }
    bounded_body(response, RESPONSE_LIMIT)
        .await
        .map_err(ProviderRuntimeError::response_invalid)
}

fn unsupported_provider() -> ProviderRuntimeError {
    ProviderRuntimeError::configuration("The requested metadata provider is not supported.")
}

fn endpoint(
    provider: &str,
    capability: &str,
) -> Result<(OutboundAccessDeclaration<'static>, reqwest::Url), ProviderRuntimeError> {
    let (access, value) = match (provider, capability) {
        (GOOGLE_BOOKS_PROVIDER, SEARCH_CAPABILITY | READ_CAPABILITY) => {
            (GOOGLE_BOOKS_ACCESS, GOOGLE_BOOKS_URL)
        }
        (TMDB_PROVIDER, SEARCH_CAPABILITY) => (TMDB_ACCESS, TMDB_SEARCH_URL),
        (TMDB_PROVIDER, READ_CAPABILITY) => (TMDB_ACCESS, TMDB_BASE_URL),
        _ => return Err(unsupported_provider()),
    };
    reqwest::Url::parse(value)
        .map(|url| (access, url))
        .map_err(|_| ProviderRuntimeError::configuration("The provider endpoint is invalid."))
}

fn validate_state(
    spec: ProviderSpec,
    capability: &str,
    state: &ProviderCapabilityState,
) -> Result<(), ProviderRuntimeError> {
    let declaration = spec
        .capabilities
        .iter()
        .find(|entry| entry.capability_id == capability)
        .ok_or_else(|| {
            ProviderRuntimeError::configuration("The provider capability is not declared.")
        })?;
    if state.provider_id().as_str() != spec.provider
        || state.capability_id().as_str() != capability
        || state.credential_requirement() != declaration.credential_requirement
    {
        return Err(ProviderRuntimeError::configuration(
            "The provider capability state does not authorize this request.",
        ));
    }
    match state.credential_status() {
        ProviderCredentialStatus::StoredUnverified | ProviderCredentialStatus::Valid => {}
        ProviderCredentialStatus::Missing | ProviderCredentialStatus::Revoked => Err(
            ProviderRuntimeError::credential_missing("The provider credential is missing."),
        )?,
        ProviderCredentialStatus::Invalid => Err(ProviderRuntimeError::credential(
            "The provider credential is invalid.",
        ))?,
        ProviderCredentialStatus::Expired => Err(ProviderRuntimeError::credential_expired(
            "The provider credential is expired.",
        ))?,
        ProviderCredentialStatus::Unavailable => Err(ProviderRuntimeError::vault(
            "The provider credential vault is unavailable.",
        ))?,
        ProviderCredentialStatus::NotRequired | ProviderCredentialStatus::Optional => {
            Err(ProviderRuntimeError::configuration(
                "The provider capability credential state is inconsistent.",
            ))?
        }
    }
    if !matches!(
        state.capability_status(),
        ProviderCapabilityStatus::Available | ProviderCapabilityStatus::Degraded
    ) {
        return Err(ProviderRuntimeError::provider(
            "The provider capability is unavailable.",
        ));
    }
    Ok(())
}

fn credential_request(
    provider: &str,
    client: &crate::AuthorizedClient,
    url: reqwest::Url,
    credential: &CredentialSecret,
) -> Result<reqwest::RequestBuilder, ProviderRuntimeError> {
    let bytes = credential.expose();
    if bytes.is_empty() || bytes.len() > 512 || !bytes.iter().all(|byte| byte.is_ascii_graphic()) {
        return Err(ProviderRuntimeError::credential(
            "A provider credential must contain 1 to 512 visible ASCII characters.",
        ));
    }
    match provider {
        GOOGLE_BOOKS_PROVIDER => sensitive_header_request(
            client,
            url,
            bytes,
            reqwest::header::HeaderName::from_static("x-goog-api-key"),
        ),
        TMDB_PROVIDER => {
            let mut value = Vec::with_capacity("Bearer ".len() + bytes.len());
            value.extend_from_slice(b"Bearer ");
            value.extend_from_slice(bytes);
            sensitive_header_request(client, url, &value, AUTHORIZATION)
        }
        _ => Err(unsupported_provider()),
    }
}

fn sensitive_header_request(
    client: &crate::AuthorizedClient,
    url: reqwest::Url,
    credential: &[u8],
    header_name: reqwest::header::HeaderName,
) -> Result<reqwest::RequestBuilder, ProviderRuntimeError> {
    let mut header = HeaderValue::from_bytes(credential)
        .map_err(|_| ProviderRuntimeError::credential("The provider credential is invalid."))?;
    header.set_sensitive(true);
    Ok(client
        .get(url)
        .map_err(ProviderRuntimeError::network)?
        .header(header_name, header))
}

fn vault_error(error: CredentialVaultError) -> ProviderRuntimeError {
    match error {
        CredentialVaultError::Missing => {
            ProviderRuntimeError::credential_missing("The provider credential is missing.")
        }
        CredentialVaultError::Unavailable => {
            ProviderRuntimeError::vault("The credential vault is unavailable.")
        }
        CredentialVaultError::Rejected => {
            ProviderRuntimeError::credential("The credential vault rejected the operation.")
        }
    }
}

fn validate_query(query: &str) -> Result<(), ProviderRuntimeError> {
    if query.is_empty()
        || query.trim() != query
        || query.len() > QUERY_LIMIT
        || query.chars().any(char::is_control)
    {
        return Err(ProviderRuntimeError::configuration(
            "The provider query must contain 1 to 256 bytes without leading, trailing, or control characters.",
        ));
    }
    Ok(())
}

fn parse_google_candidates(body: &[u8]) -> Result<Vec<ProviderCandidate>, ProviderRuntimeError> {
    let response: GoogleVolumesResponse = serde_json::from_slice(body).map_err(|_| {
        ProviderRuntimeError::response_invalid("Google Books returned invalid JSON.")
    })?;
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
    if provider_identity_mapping(GOOGLE_BOOKS_PROVIDER, "book")?
        .identifier(provider_id.as_str())
        .is_err()
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

fn parse_tmdb_candidates(body: &[u8]) -> Result<Vec<ProviderCandidate>, ProviderRuntimeError> {
    let response: TmdbSearchResponse = serde_json::from_slice(body)
        .map_err(|_| ProviderRuntimeError::response_invalid("TMDB returned invalid JSON."))?;
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
    if item.adult != Some(false) {
        return None;
    }
    let kind = match forced_kind.or(item.media_type.as_deref())? {
        "movie" => "movie",
        "tv" | "show" => "show",
        _ => return None,
    };
    let provider_id = item.id?.to_string();
    provider_identity_mapping(TMDB_PROVIDER, kind)?
        .identifier(provider_id.as_str())
        .ok()?;
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
    use fasti_application::{
        ConfigurationDigest, ProviderCapabilityId, ProviderCheckMetadata, ProviderId,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

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
            .expect("canonical namespace");
        assert_eq!(candidates[0].grain().expect("book grain"), Grain::Edition);
        assert_eq!(namespace.namespace().as_str(), "googlebooks.volume");
        assert_eq!(namespace.grains(), [Grain::Edition]);
        assert_eq!(namespace.id_pattern(), "[A-Za-z0-9_-]+");
    }

    #[test]
    fn skips_partial_or_unsafe_provider_items() {
        let body = br#"{
          "items": [
            {"id":"valid","volumeInfo":{"title":"A Book","authors":["An Author"]}},
            {"id":"../other-path","volumeInfo":{"title":"Unsafe ID","authors":[]}},
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
        assert!(validate_query("isbn:9780140328721").is_ok());
        assert!(validate_query(" leading").is_err());
        assert!(validate_query(&"x".repeat(QUERY_LIMIT + 1)).is_err());
        let client = crate::transport::test_authorized_client(
            GOOGLE_BOOKS_PROVIDER,
            SEARCH_CAPABILITY,
            GOOGLE_BOOKS_URL,
        );
        let url = reqwest::Url::parse(GOOGLE_BOOKS_URL).expect("provider URL");
        let valid = CredentialSecret::try_from_bytes(b"valid-key".to_vec()).expect("credential");
        assert!(credential_request(GOOGLE_BOOKS_PROVIDER, &client, url.clone(), &valid).is_ok());
        let invalid = CredentialSecret::try_from_bytes(b"key with spaces".to_vec())
            .expect("bounded credential");
        assert!(credential_request(GOOGLE_BOOKS_PROVIDER, &client, url, &invalid).is_err());
    }

    #[test]
    fn configuration_drift_stops_before_the_vault_is_read() {
        let vault = Arc::new(CountingVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let client = crate::transport::test_authorized_client(
            GOOGLE_BOOKS_PROVIDER,
            SEARCH_CAPABILITY,
            GOOGLE_BOOKS_URL,
        );
        let state = provider_state("ab".repeat(32));

        assert!(runtime
            .load_bound_credential(&client, GOOGLE_BOOKS_SPEC, &state)
            .is_err());
        assert_eq!(vault.loads.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn credential_is_sent_only_in_a_sensitive_header() {
        let client = crate::transport::test_authorized_client(
            GOOGLE_BOOKS_PROVIDER,
            SEARCH_CAPABILITY,
            GOOGLE_BOOKS_URL,
        );
        let url = reqwest::Url::parse(GOOGLE_BOOKS_URL).expect("provider URL");
        let credential =
            CredentialSecret::try_from_bytes(b"test-key".to_vec()).expect("credential");
        let request = credential_request(GOOGLE_BOOKS_PROVIDER, &client, url, &credential)
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
            {"id": 42, "adult": false, "media_type": "movie", "title": "A Film", "original_title": "Original Film", "release_date": "2025-04-03", "overview": "Film overview", "poster_path": "/film.jpg"},
            {"id": 42, "adult": false, "media_type": "tv", "name": "A Show", "first_air_date": "2024-01-02", "poster_path": "/show.jpg"},
            {"id": 7, "adult": false, "media_type": "person", "name": "Not media"},
            {"id": 8, "adult": true, "media_type": "movie", "title": "Adult media"},
            {"id": 0, "adult": false, "media_type": "movie", "title": "Invalid identity"}
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
        let movie_identifier = candidates[0].identifier().expect("movie identifier");
        assert_eq!(movie_identifier.namespace(), "tmdb.movie");
        assert_eq!(movie_identifier.grain(), Grain::Film);
        let show_identifier = candidates[1].identifier().expect("show identifier");
        assert_eq!(show_identifier.namespace(), "tmdb.tv");
        assert_eq!(show_identifier.grain(), Grain::Series);
        assert_eq!(release_year("2025oops"), None);
        assert!(verify_selected_candidate(candidates[0].clone(), "42", "movie").is_ok());
        assert!(verify_selected_candidate(candidates[0].clone(), "7", "movie").is_err());
        assert!(verify_selected_candidate(candidates[0].clone(), "42", "show").is_err());
    }

    #[test]
    fn tmdb_token_is_header_only_and_sensitive() {
        let client = crate::transport::test_authorized_client(
            TMDB_PROVIDER,
            SEARCH_CAPABILITY,
            TMDB_SEARCH_URL,
        );
        let url = reqwest::Url::parse(TMDB_SEARCH_URL).expect("provider URL");
        let credential =
            CredentialSecret::try_from_bytes(b"test-token".to_vec()).expect("credential");
        let request = credential_request(TMDB_PROVIDER, &client, url, &credential)
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

    #[test]
    fn registry_includes_active_and_honest_unavailable_providers() {
        assert_eq!(registry().len(), 12);
        assert!(registry()
            .iter()
            .find(|entry| entry.provider == TMDB_PROVIDER)
            .is_some_and(|entry| entry.runtime_available && entry.capabilities.len() == 2));
        assert!(registry()
            .iter()
            .find(|entry| entry.provider == "open-library")
            .is_some_and(|entry| {
                !entry.runtime_available
                    && entry.capabilities.len() == 2
                    && entry.capabilities.iter().all(|capability| {
                        capability.credential_requirement == CredentialRequirement::None
                            && !capability.health_test
                            && !capability.credential_test
                    })
            }));
        assert!(registry()
            .iter()
            .filter(|entry| !entry.runtime_available)
            .all(|entry| entry.capabilities.len() == 2));
    }

    #[derive(Default)]
    struct CountingVault {
        loads: AtomicUsize,
    }

    impl CredentialVaultPort for CountingVault {
        fn source(
            &self,
            _reference: &CredentialReference,
        ) -> Result<fasti_application::CredentialVaultSource, CredentialVaultError> {
            Ok(fasti_application::CredentialVaultSource::CredentialStore)
        }

        fn store(
            &self,
            _reference: &CredentialReference,
            _secret: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            Err(CredentialVaultError::Unavailable)
        }

        fn replace(
            &self,
            _reference: &CredentialReference,
            _secret: CredentialSecret,
        ) -> Result<StoredCredential, CredentialVaultError> {
            Err(CredentialVaultError::Unavailable)
        }

        fn load(
            &self,
            _reference: &CredentialReference,
        ) -> Result<CredentialSecret, CredentialVaultError> {
            self.loads.fetch_add(1, Ordering::Relaxed);
            CredentialSecret::try_from_bytes(b"test-key".to_vec())
                .map_err(|_| CredentialVaultError::Rejected)
        }

        fn revoke(&self, _reference: &CredentialReference) -> Result<(), CredentialVaultError> {
            Err(CredentialVaultError::Unavailable)
        }
    }

    fn provider_state(digest: String) -> ProviderCapabilityState {
        ProviderCapabilityState::try_new(
            ProviderId::try_new(GOOGLE_BOOKS_PROVIDER).expect("provider ID"),
            ProviderCapabilityId::try_new(SEARCH_CAPABILITY).expect("capability ID"),
            ProviderCapabilityStatus::Available,
            1,
            CredentialRequirement::ApiKey,
            Some(CredentialReference::try_new(GOOGLE_BOOKS_ACCOUNT).expect("credential reference")),
            ProviderCredentialStatus::Valid,
            ConfigurationDigest::parse(digest).expect("configuration digest"),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .expect("provider state")
    }
}
