use crate::transport::{bounded_body, GovernedTransport};
use crate::ProviderRuntimeError;
use fasti_application::{
    provider_candidate_metadata_fields, provider_identity_mapping, valid_search_candidate_image,
    valid_search_candidate_text as valid_candidate_text, ConfigurationDigest, CredentialReference,
    CredentialRequirement, CredentialSecret, CredentialVaultError, CredentialVaultPort,
    CredentialVaultSource, NetworkClass, OutboundAccessDeclaration, OutboundAccessPolicy,
    ProviderCapabilityState, ProviderCapabilityStatus, ProviderCheckKind, ProviderCredentialStatus,
    ProviderIdentityMapping, ProviderMetadataField, ProviderResponseCachePolicy, SearchCandidate,
    SearchCandidateData, StoredCredential, GOOGLE_BOOKS_PROVIDER_ID, MAX_PROVIDER_CREDENTIAL_BYTES,
    TMDB_PROVIDER_ID,
};
use fasti_domain::{
    ExternalIdentifierClaim, FieldClaimStatus, Grain, MetadataLocale, NamespaceDefinition,
    ReceivedAt, SearchQuery, Sha256Digest, MAX_SEARCH_QUERY_BYTES, METADATA_FRESH_SECONDS,
};
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
const RESPONSE_LIMIT: usize = 2_000_000;
const RESULT_LIMIT: usize = 10;
const TMDB_PAGE_SIZE: usize = 20;

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

#[cfg(feature = "tmdb-smoke-fixture")]
const TMDB_SMOKE_ACCESS: OutboundAccessDeclaration<'static> = OutboundAccessDeclaration {
    provider: TMDB_PROVIDER,
    capabilities: &[SEARCH_CAPABILITY, READ_CAPABILITY],
    hosts: &[TMDB_HOST],
    networks: &[NetworkClass::Loopback],
};

/// Builds the compile-time-only transport used by the real fastid TMDB smoke journey.
#[cfg(feature = "tmdb-smoke-fixture")]
pub fn tmdb_smoke_fixture_transport(
    address: std::net::SocketAddr,
    ca_pem: &[u8],
) -> Result<GovernedTransport, ProviderRuntimeError> {
    GovernedTransport::tmdb_smoke_fixture(TMDB_ACCESS, TMDB_SMOKE_ACCESS, address, ca_pem)
}

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
    query_bytes: MAX_SEARCH_QUERY_BYTES,
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

// Fasti's implementation policy revision, not a vendor legal-terms version or
// permission to redistribute metadata or share profile-bound Search receipts.
const PUBLIC_METADATA_CACHE_POLICY: &str = "fasti.public-metadata-cache.v1";

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
    cache_policy: PUBLIC_METADATA_CACHE_POLICY,
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
    cache_policy: PUBLIC_METADATA_CACHE_POLICY,
    offline_behavior: "fail_without_mutating_local_state",
    licence_and_terms: "tmdb_attribution_required",
    request_limits: ProviderRequestLimits {
        result_count: TMDB_PAGE_SIZE,
        ..REQUEST_LIMITS
    },
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

#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSearchInput {
    pub provider: String,
    pub query: String,
}

impl std::fmt::Debug for ProviderSearchInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderSearchInput")
            .field("provider", &self.provider)
            .field("query", &"[redacted]")
            .finish()
    }
}

/// One normalized upstream page. Callers must persist its ordered candidates
/// before issuing a stable Fasti cursor; an upstream page is not a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSearchPage {
    pub candidates: Vec<ProviderCandidate>,
    pub next_page: Option<u32>,
    pub evidence_digest: Sha256Digest,
    response_cache_policy: Option<ProviderResponseCachePolicy>,
}

impl ProviderSearchPage {
    pub const fn response_cache_policy(&self) -> Option<&ProviderResponseCachePolicy> {
        self.response_cache_policy.as_ref()
    }

    pub(crate) fn with_response_cache_policy(
        mut self,
        policy: ProviderResponseCachePolicy,
    ) -> Self {
        self.response_cache_policy = Some(policy);
        for candidate in &mut self.candidates {
            candidate.response_cache_policy = Some(policy);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSelectionInput {
    pub provider: String,
    pub provider_id: String,
    pub kind: String,
    pub locale: Option<String>,
    pub region: Option<String>,
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
    #[serde(skip)]
    evidence_digest: Sha256Digest,
    #[serde(skip)]
    response_cache_policy: Option<ProviderResponseCachePolicy>,
}

impl ProviderCandidate {
    pub const fn response_cache_policy(&self) -> Option<&ProviderResponseCachePolicy> {
        self.response_cache_policy.as_ref()
    }

    pub fn recorded_response_policy(
        &self,
    ) -> Result<&ProviderResponseCachePolicy, ProviderRuntimeError> {
        self.response_cache_policy().ok_or_else(|| {
            ProviderRuntimeError::response_invalid(
                "The provider response has no recorded cache policy.",
            )
        })
    }

    fn with_response_cache_policy(mut self, policy: ProviderResponseCachePolicy) -> Self {
        self.response_cache_policy = Some(policy);
        self
    }

    /// Admit only normalized public fields to the durable Search receipt.
    pub fn search_evidence(&self) -> Result<SearchCandidate, ProviderRuntimeError> {
        SearchCandidate::try_new(SearchCandidateData {
            provider: self.provider.to_owned(),
            provider_id: self.provider_id.clone(),
            kind: self.kind.to_owned(),
            title: self.title.clone(),
            original_title: self.original_title.clone(),
            release_year: self.release_year,
            authors: self.authors.clone(),
            image_url: self.image_url.clone(),
            overview: self.overview.clone(),
        })
        .map_err(|error| ProviderRuntimeError::response_invalid(error.to_string()))
    }

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

    pub fn metadata_fields(
        &self,
        locale: Option<MetadataLocale>,
        region: Option<fasti_domain::MetadataRegion>,
    ) -> Result<Vec<ProviderMetadataField>, ProviderRuntimeError> {
        let policy = self.recorded_response_policy()?;
        let fetched_at = ReceivedAt::from_application_clock(policy.received_at());
        let (fresh_until, _) = policy
            .deadlines(
                std::time::Duration::from_secs(METADATA_FRESH_SECONDS as u64),
                std::time::Duration::from_secs(
                    fasti_domain::METADATA_STALE_ON_ERROR_SECONDS as u64,
                ),
            )
            .ok_or_else(|| {
                ProviderRuntimeError::response_invalid("The provider response cannot be stored.")
            })?;
        let expires_at = (fresh_until > fetched_at.value()).then_some(fresh_until);
        provider_candidate_metadata_fields(
            &self.search_evidence()?,
            locale,
            region,
            &self.evidence_digest,
            fetched_at,
            expires_at,
            if expires_at.is_some() {
                FieldClaimStatus::Fresh
            } else {
                FieldClaimStatus::Stale
            },
        )
        .map_err(|_| {
            ProviderRuntimeError::response_invalid("The provider metadata evidence is invalid.")
        })
    }
}

#[derive(Debug, Deserialize)]
struct GoogleVolumesResponse {
    #[serde(rename = "totalItems")]
    total_items: u64,
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
    page: u32,
    total_pages: u32,
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
        Ok(self
            .search_page(input, 1, None, policy, state)
            .await?
            .candidates)
    }

    pub async fn search_page(
        &self,
        input: ProviderSearchInput,
        page: u32,
        locale: Option<&MetadataLocale>,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<ProviderSearchPage, ProviderRuntimeError> {
        let query = SearchQuery::try_new(input.query)
            .map_err(|error| ProviderRuntimeError::configuration(error.to_string()))?;
        let url = search_url(&input.provider, &query, page, locale)?;
        let spec = self.active_spec(&input.provider)?;
        let (access, endpoint) = endpoint(&input.provider, SEARCH_CAPABILITY)?;
        let client = self
            .authorized_credential(access, spec, SEARCH_CAPABILITY, endpoint, policy, state)
            .await?;
        let credential = self.load_bound_credential(&client, spec, state)?;
        let response = send_json(
            credential_request(&input.provider, &client, url, &credential)?,
            spec,
        )
        .await?;
        match input.provider.as_str() {
            GOOGLE_BOOKS_PROVIDER => parse_google_candidates(&response.body, page),
            TMDB_PROVIDER => parse_tmdb_candidates(&response.body, page),
            _ => Err(unsupported_provider()),
        }
        .map(|page| page.with_response_cache_policy(response.cache_policy))
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
                self.fetch_tmdb(
                    &input.provider_id,
                    &input.kind,
                    input.locale.as_deref().unwrap_or("en-US"),
                    input.region.as_deref(),
                    policy,
                    state,
                )
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
        let response = send_json(request, spec).await?;
        serde_json::from_slice::<serde_json::Value>(&response.body)
            .map(|_| ())
            .map_err(|_| {
                ProviderRuntimeError::response_invalid("The provider check returned invalid JSON.")
            })
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
        let response = send_json(
            credential_request(GOOGLE_BOOKS_PROVIDER, &client, url, &credential)?,
            GOOGLE_BOOKS_SPEC,
        )
        .await?;
        let volume: GoogleVolume = serde_json::from_slice(&response.body).map_err(|_| {
            ProviderRuntimeError::response_invalid("Google Books returned invalid JSON.")
        })?;
        let candidate = google_candidate(volume, provider_evidence_digest(&response.body))
            .ok_or_else(|| {
                ProviderRuntimeError::response_invalid(
                    "Google Books returned incomplete or unsafe metadata.",
                )
            })?;
        verify_selected_candidate(candidate, provider_id, "book")
            .map(|candidate| candidate.with_response_cache_policy(response.cache_policy))
    }

    async fn fetch_tmdb(
        &self,
        provider_id: &str,
        kind: &str,
        locale: &str,
        region: Option<&str>,
        policy: &OutboundAccessPolicy,
        state: &ProviderCapabilityState,
    ) -> Result<ProviderCandidate, ProviderRuntimeError> {
        let url = tmdb_details_url(provider_id, kind, locale, region)?;
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
        let response = send_json(
            credential_request(TMDB_PROVIDER, &client, url, &credential)?,
            TMDB_SPEC,
        )
        .await?;
        let item: TmdbItem = serde_json::from_slice(&response.body)
            .map_err(|_| ProviderRuntimeError::response_invalid("TMDB returned invalid JSON."))?;
        let candidate = tmdb_candidate(item, Some(kind), provider_evidence_digest(&response.body))
            .ok_or_else(|| {
                ProviderRuntimeError::response_invalid(
                    "TMDB returned incomplete or unsafe metadata.",
                )
            })?;
        verify_selected_candidate(candidate, provider_id, kind)
            .map(|candidate| candidate.with_response_cache_policy(response.cache_policy))
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

fn tmdb_details_url(
    provider_id: &str,
    kind: &str,
    locale: &str,
    region: Option<&str>,
) -> Result<reqwest::Url, ProviderRuntimeError> {
    let mut url = reqwest::Url::parse(TMDB_BASE_URL)
        .map_err(|_| ProviderRuntimeError::provider("The TMDB endpoint is invalid."))?;
    url.path_segments_mut()
        .map_err(|_| ProviderRuntimeError::provider("The TMDB endpoint is invalid."))?
        .extend([if kind == "show" { "tv" } else { "movie" }, provider_id]);
    url.query_pairs_mut().append_pair("language", locale);
    if let Some(region) = region {
        url.query_pairs_mut().append_pair("region", region);
    }
    Ok(url)
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

struct ProviderJsonResponse {
    body: Vec<u8>,
    cache_policy: ProviderResponseCachePolicy,
}

async fn send_json(
    request: reqwest::RequestBuilder,
    spec: ProviderSpec,
) -> Result<ProviderJsonResponse, ProviderRuntimeError> {
    let started = std::time::Instant::now();
    let response = request.send().await.map_err(|_| {
        ProviderRuntimeError::provider(format!("{} could not be reached.", spec.label))
    })?;
    let received_at = chrono::Utc::now();
    let policy = crate::cache_policy::observe(response.headers(), received_at, started.elapsed());
    if let Some(error) = provider_status_error(spec, response.status()) {
        return Err(error);
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
    let body = bounded_body(response, RESPONSE_LIMIT)
        .await
        .map_err(ProviderRuntimeError::response_invalid)?;
    Ok(ProviderJsonResponse {
        body,
        cache_policy: policy,
    })
}

pub(crate) fn provider_status_error(
    spec: ProviderSpec,
    status: reqwest::StatusCode,
) -> Option<ProviderRuntimeError> {
    if matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
    ) || (status == reqwest::StatusCode::BAD_REQUEST && spec.provider == GOOGLE_BOOKS_PROVIDER)
    {
        return Some(ProviderRuntimeError::credential(format!(
            "{} rejected the configured credential.",
            spec.label
        )));
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Some(ProviderRuntimeError::rate_limited(format!(
            "{} rate limited the request. Wait, then retry.",
            spec.label
        )));
    }
    if matches!(
        status,
        reqwest::StatusCode::INTERNAL_SERVER_ERROR
            | reqwest::StatusCode::BAD_GATEWAY
            | reqwest::StatusCode::SERVICE_UNAVAILABLE
            | reqwest::StatusCode::GATEWAY_TIMEOUT
    ) {
        return Some(ProviderRuntimeError::provider(format!(
            "{} is temporarily unavailable (HTTP {}).",
            spec.label,
            status.as_u16()
        )));
    }
    (status != reqwest::StatusCode::OK).then(|| {
        ProviderRuntimeError::response_invalid(format!(
            "{} returned HTTP {}.",
            spec.label,
            status.as_u16()
        ))
    })
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
    if bytes.is_empty()
        || bytes.len() > MAX_PROVIDER_CREDENTIAL_BYTES
        || !bytes.iter().all(|byte| byte.is_ascii_graphic())
    {
        return Err(ProviderRuntimeError::credential(
            "A provider credential must contain 1 to 4096 visible ASCII characters.",
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
            let mut value =
                zeroize::Zeroizing::new(Vec::with_capacity("Bearer ".len() + bytes.len()));
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

fn search_url(
    provider: &str,
    query: &SearchQuery,
    page: u32,
    locale: Option<&MetadataLocale>,
) -> Result<reqwest::Url, ProviderRuntimeError> {
    let invalid_page =
        || ProviderRuntimeError::configuration("The provider search page is invalid.");
    let offset = page.checked_sub(1).ok_or_else(invalid_page)?;
    let (_, mut url) = endpoint(provider, SEARCH_CAPABILITY)?;
    match provider {
        GOOGLE_BOOKS_PROVIDER => {
            let start = offset
                .checked_mul(RESULT_LIMIT as u32)
                .ok_or_else(invalid_page)?;
            url.query_pairs_mut()
                .append_pair("q", query.as_str())
                .append_pair("startIndex", &start.to_string())
                .append_pair("maxResults", &RESULT_LIMIT.to_string())
                .append_pair("projection", "lite");
        }
        TMDB_PROVIDER => {
            // TMDB rejects pages after 500 even when total_pages is larger.
            if page > 500 {
                return Err(invalid_page());
            }
            url.query_pairs_mut()
                .append_pair("query", query.as_str())
                .append_pair("include_adult", "false")
                .append_pair("language", locale.map_or("en-US", MetadataLocale::as_str))
                .append_pair("page", &page.to_string());
        }
        _ => return Err(unsupported_provider()),
    }
    Ok(url)
}

fn parse_google_candidates(
    body: &[u8],
    page: u32,
) -> Result<ProviderSearchPage, ProviderRuntimeError> {
    let response: GoogleVolumesResponse = serde_json::from_slice(body).map_err(|_| {
        ProviderRuntimeError::response_invalid("Google Books returned invalid JSON.")
    })?;
    if page == 0 || response.items.len() > RESULT_LIMIT {
        return Err(ProviderRuntimeError::response_invalid(
            "Google Books returned an invalid search page.",
        ));
    }
    let next_page = (!response.items.is_empty()
        && u64::from(page) * (RESULT_LIMIT as u64) < response.total_items)
        .then_some(page)
        .and_then(|page| page.checked_add(1))
        .filter(|page| (page - 1).checked_mul(RESULT_LIMIT as u32).is_some());
    let mut seen = BTreeSet::new();
    let evidence_digest = provider_evidence_digest(body);
    let candidates = response
        .items
        .into_iter()
        .filter_map(|item| google_candidate(item, evidence_digest.clone()))
        .filter(|candidate| seen.insert(candidate.provider_id.clone()))
        .collect();
    Ok(ProviderSearchPage {
        candidates,
        next_page,
        evidence_digest,
        response_cache_policy: None,
    })
}

fn google_candidate(
    item: GoogleVolume,
    evidence_digest: Sha256Digest,
) -> Option<ProviderCandidate> {
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
        evidence_digest,
        response_cache_policy: None,
    })
}

fn parse_tmdb_candidates(
    body: &[u8],
    page: u32,
) -> Result<ProviderSearchPage, ProviderRuntimeError> {
    let response: TmdbSearchResponse = serde_json::from_slice(body)
        .map_err(|_| ProviderRuntimeError::response_invalid("TMDB returned invalid JSON."))?;
    if page == 0 || page > 500 || response.page != page || response.results.len() > TMDB_PAGE_SIZE {
        return Err(ProviderRuntimeError::response_invalid(
            "TMDB returned an invalid search page.",
        ));
    }
    let next_page =
        (!response.results.is_empty() && page < response.total_pages.min(500)).then_some(page + 1);
    let mut seen = BTreeSet::new();
    let evidence_digest = provider_evidence_digest(body);
    let candidates = response
        .results
        .into_iter()
        .filter_map(|item| tmdb_candidate(item, None, evidence_digest.clone()))
        .filter(|candidate| seen.insert((candidate.kind, candidate.provider_id.clone())))
        .collect();
    Ok(ProviderSearchPage {
        candidates,
        next_page,
        evidence_digest,
        response_cache_policy: None,
    })
}

#[cfg(test)]
pub(crate) fn search_page_fixture() -> ProviderSearchPage {
    parse_tmdb_candidates(br#"{"page":1,"total_pages":2,"results":[{"id":42,"media_type":"movie","title":"Fixture film","adult":false}]}"#, 1)
        .expect("valid parser fixture")
        .with_response_cache_policy(crate::cache_policy::observe(&reqwest::header::HeaderMap::new(), chrono::Utc::now(), std::time::Duration::ZERO))
}

fn tmdb_candidate(
    item: TmdbItem,
    forced_kind: Option<&str>,
    evidence_digest: Sha256Digest,
) -> Option<ProviderCandidate> {
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
    let image_url = item
        .poster_path
        .and_then(|path| {
            (path.starts_with('/') && path.len() <= 256 && !path.chars().any(char::is_control))
                .then(|| format!("{TMDB_IMAGE_BASE_URL}{path}"))
        })
        .filter(|image| valid_search_candidate_image(TMDB_PROVIDER, image));
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
        evidence_digest,
        response_cache_policy: None,
    })
}

fn provider_evidence_digest(body: &[u8]) -> Sha256Digest {
    Sha256Digest::from_bytes(&Sha256::digest(body).into())
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
    valid_search_candidate_image(GOOGLE_BOOKS_PROVIDER, &normalized).then_some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("provider_cache_transport_tests.rs");
    use fasti_application::{
        ConfigurationDigest, ProblemCode, ProviderCapabilityId, ProviderCheckMetadata, ProviderId,
    };
    use fasti_domain::MetadataProviderId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn metadata_conversion_uses_one_received_time_and_existing_freshness_policy() {
        let candidate = search_page_fixture().candidates.remove(0);
        let observed = candidate.recorded_response_policy().unwrap().received_at();
        let fields = candidate.metadata_fields(None, None).unwrap();
        let fetched = fields[0].claim().fetched_at();
        assert_eq!(fetched, observed);
        for (original, repeated) in fields
            .iter()
            .zip(candidate.metadata_fields(None, None).unwrap())
        {
            assert_eq!(original.claim().fetched_at(), repeated.claim().fetched_at());
            assert_eq!(original.claim().expires_at(), repeated.claim().expires_at());
        }
        for field in fields {
            let claim = field.claim();
            assert_eq!(claim.fetched_at(), fetched);
            assert_eq!(
                claim.expires_at(),
                Some(fetched + chrono::Duration::seconds(METADATA_FRESH_SECONDS))
            );
            assert_eq!(claim.initial_status(), FieldClaimStatus::Fresh);
            assert_eq!(
                claim.provenance().evidence_digest(),
                Some(&candidate.evidence_digest)
            );
            assert_eq!(
                claim.provenance().source_identifier(),
                Some(candidate.provider_id.as_str())
            );
        }
        let mut invalid = candidate;
        invalid.overview = Some("x".repeat(4097));
        assert_eq!(
            invalid
                .metadata_fields(None, None)
                .unwrap_err()
                .problem_code(),
            fasti_application::ProblemCode::ProviderResponseInvalid
        );
    }

    #[test]
    fn invalid_optional_artwork_does_not_poison_search_evidence() {
        let tmdb = br#"{"page":1,"total_pages":1,"results":[{"id":42,"media_type":"movie","title":"Film","poster_path":"/bad image.jpg"}]}"#;
        let google = br#"{"totalItems":1,"items":[{"id":"book-1","volumeInfo":{"title":"Book","imageLinks":{"thumbnail":"https://books.google.com:8443/image"}}}]}"#;
        for candidate in parse_tmdb_candidates(tmdb, 1)
            .unwrap()
            .candidates
            .into_iter()
            .chain(parse_google_candidates(google, 1).unwrap().candidates)
        {
            assert!(candidate.image_url.is_none());
            assert!(candidate.search_evidence().is_ok());
        }
    }

    #[test]
    fn parses_one_complete_neutral_book_page() {
        let items = (0..10)
            .map(|index| {
                format!(
                    r#"{{"id":"book-{index}","volumeInfo":{{"title":"Book {index}","authors":["Author"]}}}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(r#"{{"totalItems":12,"items":[{items}]}}"#);
        let page = parse_google_candidates(body.as_bytes(), 1).expect("provider candidates");
        assert!(page.candidates[0].metadata_fields(None, None).is_err());
        let page = page.with_response_cache_policy(crate::cache_policy::observe(
            &reqwest::header::HeaderMap::new(),
            chrono::Utc::now(),
            std::time::Duration::ZERO,
        ));
        assert_eq!(page.next_page, Some(2));
        let candidates = page.candidates;

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
        let fields = candidates[0]
            .metadata_fields(None, None)
            .expect("metadata claims");
        assert!(!fields.is_empty());
        assert!(fields.iter().all(|field| {
            let claim = field.claim();
            claim.record_id().is_none()
                && claim.field_key().is_none()
                && claim.provenance().is_complete()
                && claim
                    .provenance()
                    .provider_id()
                    .map(MetadataProviderId::as_str)
                    == Some(GOOGLE_BOOKS_PROVIDER)
                && claim.provenance().source_identifier() == Some("book-0")
                && claim.provenance().evidence_digest()
                    == Some(&provider_evidence_digest(body.as_bytes()))
                && claim.expires_at().is_some()
        }));
    }

    #[test]
    fn skips_partial_or_unsafe_provider_items() {
        let body = br#"{
          "totalItems": 4,
          "items": [
            {"id":"valid","volumeInfo":{"title":"A Book","authors":["An Author"]}},
            {"id":"../other-path","volumeInfo":{"title":"Unsafe ID","authors":[]}},
            {"id":"missing-title","volumeInfo":{"authors":[]}},
            {"id":"control","volumeInfo":{"title":"Bad\nTitle","authors":[]}}
          ]
        }"#;
        let candidates = parse_google_candidates(body, 1)
            .expect("partial response")
            .candidates;

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].provider_id, "valid");
    }

    #[test]
    fn duplicate_provider_ids_are_removed_before_the_ui() {
        let body = br#"{
          "totalItems": 2,
          "items": [
            {"id":"same","volumeInfo":{"title":"First","authors":["Author"]}},
            {"id":"same","volumeInfo":{"title":"Second","authors":["Author"]}}
          ]
        }"#;
        let candidates = parse_google_candidates(body, 1)
            .expect("deduplicated response")
            .candidates;

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].title, "First");
    }

    #[test]
    fn credentials_and_queries_are_strictly_bounded() {
        assert!(SearchQuery::try_new("isbn:9780140328721").is_ok());
        assert!(SearchQuery::try_new(" leading").is_err());
        assert!(SearchQuery::try_new("x".repeat(MAX_SEARCH_QUERY_BYTES + 1)).is_err());
        let client = crate::transport::test_authorized_client(
            GOOGLE_BOOKS_PROVIDER,
            SEARCH_CAPABILITY,
            GOOGLE_BOOKS_URL,
        );
        let url = reqwest::Url::parse(GOOGLE_BOOKS_URL).expect("provider URL");
        let valid = CredentialSecret::try_from_bytes(b"valid-key".to_vec()).expect("credential");
        assert!(credential_request(GOOGLE_BOOKS_PROVIDER, &client, url.clone(), &valid).is_ok());
        let maximum = CredentialSecret::try_from_bytes(vec![b'x'; MAX_PROVIDER_CREDENTIAL_BYTES])
            .expect("maximum credential");
        assert!(credential_request(GOOGLE_BOOKS_PROVIDER, &client, url.clone(), &maximum).is_ok());
        let too_long =
            CredentialSecret::try_from_bytes(vec![b'x'; MAX_PROVIDER_CREDENTIAL_BYTES + 1])
                .expect("general secret bound remains larger");
        assert!(
            credential_request(GOOGLE_BOOKS_PROVIDER, &client, url.clone(), &too_long).is_err()
        );
        let invalid = CredentialSecret::try_from_bytes(b"key with spaces".to_vec())
            .expect("bounded credential");
        assert!(credential_request(GOOGLE_BOOKS_PROVIDER, &client, url, &invalid).is_err());
    }

    #[test]
    fn search_urls_keep_query_data_encoded_and_select_the_requested_page() {
        let mut input = ProviderSearchInput {
            provider: GOOGLE_BOOKS_PROVIDER.to_owned(),
            query: "海 &page=99#title".to_owned(),
        };
        let query = SearchQuery::try_new(input.query.clone()).expect("query");
        let books = search_url(&input.provider, &query, 3, None).expect("book page");
        assert_eq!(books.host_str(), Some(GOOGLE_BOOKS_HOST));
        assert_eq!(books.fragment(), None);
        let pairs: std::collections::BTreeMap<_, _> = books.query_pairs().collect();
        assert_eq!(
            pairs.get("q").map(|v| v.as_ref()),
            Some(input.query.as_str())
        );
        assert_eq!(pairs.get("startIndex").map(|v| v.as_ref()), Some("20"));
        assert!(!pairs.contains_key("page"));
        assert!(search_url(&input.provider, &query, 0, None).is_err());
        assert!(search_url(&input.provider, &query, u32::MAX, None).is_err());
        assert!(!format!("{input:?}").contains(&input.query));

        input.provider = TMDB_PROVIDER.to_owned();
        let locale = MetadataLocale::try_new("fr-FR").expect("locale");
        let tmdb = search_url(&input.provider, &query, 2, Some(&locale)).expect("TMDB page");
        let pairs: std::collections::BTreeMap<_, _> = tmdb.query_pairs().collect();
        assert_eq!(
            pairs.get("query").map(|v| v.as_ref()),
            Some(input.query.as_str())
        );
        assert_eq!(pairs.get("page").map(|v| v.as_ref()), Some("2"));
        assert_eq!(pairs.get("language").map(|v| v.as_ref()), Some("fr-fr"));
        assert_eq!(
            pairs.get("include_adult").map(|v| v.as_ref()),
            Some("false")
        );
        assert!(search_url(&input.provider, &query, 501, None).is_err());
    }

    #[test]
    fn tmdb_pages_retain_all_twenty_candidates_and_verify_the_requested_page() {
        let items: Vec<_> = (1..=20)
            .map(|id| {
                serde_json::json!({
                    "id": id, "adult": false, "media_type": "movie", "title": format!("Film {id}")
                })
            })
            .collect();
        let body = serde_json::to_vec(&serde_json::json!({
            "page": 2, "total_pages": 3, "results": items
        }))
        .expect("fixture");
        let page = parse_tmdb_candidates(&body, 2).expect("whole page");
        assert_eq!(page.candidates.len(), 20);
        assert_eq!(page.candidates[19].provider_id, "20");
        assert_eq!(page.next_page, Some(3));
        assert_eq!(page.evidence_digest, provider_evidence_digest(&body));
        assert!(parse_tmdb_candidates(&body, 1).is_err());
        let too_many = serde_json::to_vec(&serde_json::json!({
            "page": 1, "total_pages": 1, "results": vec![items[0].clone(); 21]
        }))
        .expect("fixture");
        assert!(parse_tmdb_candidates(&too_many, 1).is_err());
        assert!(parse_tmdb_candidates(br#"{"page":1,"total_pages":1}"#, 1).is_err());
    }

    #[test]
    fn filtered_and_empty_pages_have_bounded_continuations() {
        let filtered = br#"{"page":1,"total_pages":2,"results":[{"id":7,"adult":false,"media_type":"person","name":"Person"}]}"#;
        let page = parse_tmdb_candidates(filtered, 1).expect("filtered page");
        assert!(page.candidates.is_empty());
        assert_eq!(page.next_page, Some(2));
        let last = br#"{"page":500,"total_pages":800,"results":[{"id":7,"adult":false,"media_type":"person","name":"Person"}]}"#;
        assert_eq!(
            parse_tmdb_candidates(last, 500)
                .expect("last page")
                .next_page,
            None
        );
        assert_eq!(
            parse_tmdb_candidates(br#"{"page":1,"total_pages":800,"results":[]}"#, 1)
                .expect("empty page")
                .next_page,
            None
        );
        assert_eq!(
            parse_google_candidates(br#"{"totalItems":100}"#, 1)
                .expect("empty books page")
                .next_page,
            None
        );
        assert!(parse_google_candidates(br#"{"items":[]}"#, 1).is_err());
        let body = serde_json::to_vec(
            &serde_json::json!({"totalItems":11,"items":vec![serde_json::json!({});11]}),
        )
        .expect("fixture");
        assert!(parse_google_candidates(&body, 1).is_err());
    }

    #[tokio::test]
    async fn invalid_search_input_fails_before_provider_authorization_or_credential_load() {
        let vault = Arc::new(CountingVault::default());
        let runtime = ProviderRuntime::new(vault.clone());
        let state = provider_state("ab".repeat(32));
        let policy = OutboundAccessPolicy::default();
        for (query, page) in [
            ("".to_owned(), 1),
            ("bad\nquery".to_owned(), 1),
            ("海".repeat(86), 1),
            ("book".to_owned(), 0),
            ("book".to_owned(), u32::MAX),
        ] {
            let error = runtime
                .search_page(
                    ProviderSearchInput {
                        provider: GOOGLE_BOOKS_PROVIDER.to_owned(),
                        query,
                    },
                    page,
                    None,
                    &policy,
                    &state,
                )
                .await
                .expect_err("invalid input");
            assert!(error.detail().contains("query") || error.detail().contains("page"));
        }
        assert_eq!(vault.loads.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn google_books_bad_request_matches_the_manifest_credential_problem() {
        let error = provider_status_error(GOOGLE_BOOKS_SPEC, reqwest::StatusCode::BAD_REQUEST)
            .expect("Google Books 400 is a provider problem");
        assert_eq!(error.problem_code(), ProblemCode::ProviderCredentialInvalid);
        let tmdb = provider_status_error(TMDB_SPEC, reqwest::StatusCode::BAD_REQUEST)
            .expect("TMDB 400 is a provider problem");
        assert_eq!(tmdb.problem_code(), ProblemCode::ProviderResponseInvalid);
    }

    #[test]
    fn http_status_mapping_distinguishes_outages_from_invalid_responses() {
        for spec in [GOOGLE_BOOKS_SPEC, TMDB_SPEC] {
            assert!(provider_status_error(spec, reqwest::StatusCode::OK).is_none());
            for (status, expected) in [
                (401, ProblemCode::ProviderCredentialInvalid),
                (403, ProblemCode::ProviderCredentialInvalid),
                (404, ProblemCode::ProviderResponseInvalid),
                (429, ProblemCode::ProviderRateLimited),
                (500, ProblemCode::ProviderUnavailable),
                (501, ProblemCode::ProviderResponseInvalid),
                (502, ProblemCode::ProviderUnavailable),
                (503, ProblemCode::ProviderUnavailable),
                (504, ProblemCode::ProviderUnavailable),
                (505, ProblemCode::ProviderResponseInvalid),
            ] {
                let error =
                    provider_status_error(spec, reqwest::StatusCode::from_u16(status).unwrap())
                        .unwrap();
                assert_eq!(
                    error.problem_code(),
                    expected,
                    "{} HTTP {status}",
                    spec.provider
                );
                if expected == ProblemCode::ProviderUnavailable {
                    assert_eq!(error.kind(), crate::ProviderRuntimeErrorKind::Provider);
                }
            }
        }
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
          "page": 1, "total_pages": 1,
          "results": [
            {"id": 42, "adult": false, "media_type": "movie", "title": "A Film", "original_title": "Original Film", "release_date": "2025-04-03", "overview": "Film overview", "poster_path": "/film.jpg"},
            {"id": 42, "adult": false, "media_type": "tv", "name": "A Show", "first_air_date": "2024-01-02", "poster_path": "/show.jpg"},
            {"id": 7, "adult": false, "media_type": "person", "name": "Not media"},
            {"id": 8, "adult": true, "media_type": "movie", "title": "Adult media"},
            {"id": 0, "adult": false, "media_type": "movie", "title": "Invalid identity"}
          ]
        }"#;
        let candidates = parse_tmdb_candidates(body, 1)
            .expect("TMDB candidates")
            .with_response_cache_policy(crate::cache_policy::observe(
                &reqwest::header::HeaderMap::new(),
                chrono::Utc::now(),
                std::time::Duration::ZERO,
            ))
            .candidates;

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
        let fields = candidates[0]
            .metadata_fields(
                Some(MetadataLocale::try_new("en-US").expect("locale")),
                None,
            )
            .expect("TMDB metadata claims");
        assert!(fields.iter().all(|field| {
            field
                .claim()
                .provenance()
                .locale()
                .map(MetadataLocale::as_str)
                == Some("en-us")
                && field.claim().provenance().source_identifier() == Some("42")
        }));
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
    fn tmdb_details_url_preserves_requested_locale_and_region() {
        let url =
            tmdb_details_url("438631", "movie", "fr-FR", Some("FR")).expect("TMDB details URL");
        assert_eq!(url.path(), "/3/movie/438631");
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![
                ("language".into(), "fr-FR".into()),
                ("region".into(), "FR".into())
            ]
        );
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
        for entry in registry() {
            assert_eq!(
                entry.cache_policy,
                if entry.runtime_available {
                    PUBLIC_METADATA_CACHE_POLICY
                } else {
                    "no_runtime_cache"
                }
            );
            assert_ne!(entry.cache_policy, entry.licence_and_terms);
        }
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

    #[test]
    fn tmdb_production_network_grant_remains_public_only() {
        assert_eq!(TMDB_ACCESS.networks, &[NetworkClass::Public]);
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
