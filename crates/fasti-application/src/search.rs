//! Normalized Search evidence. Candidates remain separate from Fasti Records.

use crate::{
    provider_identity_mapping, ApplicationAccessContext, ApplicationResult, AuthorizedActor,
    AuthorizedApplicationAccess, OutboundAccessPolicy, ProviderCapabilityState, ProviderId,
};
use chrono::{DateTime, Duration, Utc};
use fasti_domain::{
    AuthSubjectId, ClientId, ExternalIdentifierClaim, Grain, MetadataLocale, MetadataRegion,
    ProfileGrantId, ProfileId, RecordId, RequestCorrelationId, SearchCandidateReceiptId,
    SearchQuery, Sha256Digest, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

pub const MAX_SEARCH_CANDIDATE_BYTES: usize = 64 * 1024;
pub const SEARCH_FRESH_SECONDS: i64 = 120;
pub const SEARCH_STALE_ON_ERROR_SECONDS: i64 = 600;
pub const SEARCH_RECEIPT_SECONDS: i64 = 24 * 60 * 60;
pub const MAX_SEARCH_PAGE_CANDIDATES: usize = 100;
pub const MAX_SEARCH_CONTEXT_BYTES: usize = 2048;

/// Literal Unicode default-case substring matching; never SQL/FTS query syntax.
/// Locale-specific casing and accent folding are not implicitly applied.
pub fn normalize_local_search_text(value: &str) -> String {
    value.to_lowercase()
}

#[derive(Debug, Clone)]
pub struct LocalSearchRequest {
    pub correlation_id: RequestCorrelationId,
    pub access: ApplicationAccessContext,
    pub query: SearchQuery,
    pub grains: Vec<Grain>,
    pub after: Option<LocalSearchCursor>,
}

impl LocalSearchRequest {
    pub fn context_digest(&self, access: &AuthorizedApplicationAccess) -> Sha256Digest {
        let mut grains = self.grains.clone();
        grains.sort_by_key(|grain| grain.as_str());
        grains.dedup();
        search_digest(&(
            "fasti.search.local.v1",
            access.workspace_id(),
            access.profile_id(),
            access.grant_id(),
            self.query.as_str(),
            grains,
        ))
    }
}

/// A position, not authorization. Every page rechecks current application access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSearchCursor {
    pub last_record_id: RecordId,
    pub context_digest: Sha256Digest,
}

pub struct LocalSearchPage {
    pub records: Vec<crate::RecordSummary>,
    /// Last inspected ID, including rejected candidates. Empty pages can continue.
    pub next: Option<LocalSearchCursor>,
}

fn search_digest(value: &impl Serialize) -> Sha256Digest {
    use sha2::{Digest, Sha256};
    let bytes =
        serde_json::to_vec(value).expect("Search context contains only serializable values");
    Sha256Digest::from_bytes(&Sha256::digest(bytes).into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchProviderQuery {
    query: SearchQuery,
    provider: ProviderId,
    page: u32,
    locale: Option<MetadataLocale>,
    region: Option<MetadataRegion>,
    grains: Vec<Grain>,
}

impl SearchProviderQuery {
    pub fn try_new(
        query: SearchQuery,
        provider: ProviderId,
        page: u32,
        locale: Option<MetadataLocale>,
        region: Option<MetadataRegion>,
        mut grains: Vec<Grain>,
    ) -> Result<Self, SearchEvidenceError> {
        if page == 0 || grains.len() > 32 {
            return Err(SearchEvidenceError::InvalidPartition);
        }
        grains.sort_by_key(|grain| grain.as_str());
        grains.dedup();
        Ok(Self {
            query,
            provider,
            page,
            locale,
            region,
            grains,
        })
    }
    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }
    pub fn query(&self) -> &SearchQuery {
        &self.query
    }
    pub const fn page(&self) -> u32 {
        self.page
    }
    pub fn locale(&self) -> Option<&MetadataLocale> {
        self.locale.as_ref()
    }
    pub fn region(&self) -> Option<&MetadataRegion> {
        self.region.as_ref()
    }
    pub fn grains(&self) -> &[Grain] {
        &self.grains
    }
    pub fn digest(&self) -> Sha256Digest {
        search_digest(&(
            "fasti.search.page.v1",
            self.query.as_str(),
            self.provider.as_str(),
            self.page,
            &self.locale,
            &self.region,
            &self.grains,
        ))
    }

    pub fn receipt_context(&self) -> SearchPageContext {
        SearchPageContext {
            query_digest: self.digest(),
            provider: self.provider.as_str().to_owned(),
            page: self.page,
            locale: self.locale.clone(),
            region: self.region.clone(),
            grains: self.grains.clone(),
        }
    }
}

/// Stored route coordinates and query evidence, without the user's query text.
/// Reopening a candidate must not depend on browser-supplied metadata or locale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchPageContext {
    query_digest: Sha256Digest,
    provider: String,
    page: u32,
    locale: Option<MetadataLocale>,
    region: Option<MetadataRegion>,
    grains: Vec<Grain>,
}

impl SearchPageContext {
    pub fn provider(&self) -> &str {
        &self.provider
    }
    pub const fn page(&self) -> u32 {
        self.page
    }
    pub fn locale(&self) -> Option<&MetadataLocale> {
        self.locale.as_ref()
    }
    pub fn region(&self) -> Option<&MetadataRegion> {
        self.region.as_ref()
    }
    pub fn grains(&self) -> &[Grain] {
        &self.grains
    }
    pub fn digest(&self) -> Sha256Digest {
        search_digest(&("fasti.search.context.v1", self))
    }
    pub fn accepts(&self, candidate: &SearchCandidate) -> bool {
        candidate.data().provider == self.provider
            && (self.grains.is_empty() || self.grains.contains(&candidate.identifier().grain()))
    }
    pub fn to_json(&self) -> Result<String, SearchEvidenceError> {
        let value =
            serde_json::to_string(self).map_err(|_| SearchEvidenceError::InvalidPartition)?;
        if value.len() > MAX_SEARCH_CONTEXT_BYTES {
            return Err(SearchEvidenceError::InvalidPartition);
        }
        Ok(value)
    }
    pub fn from_json(value: &str) -> Result<Self, SearchEvidenceError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Stored {
            query_digest: Sha256Digest,
            provider: String,
            page: u32,
            locale: Option<String>,
            region: Option<String>,
            grains: Vec<Grain>,
        }
        if value.len() > MAX_SEARCH_CONTEXT_BYTES {
            return Err(SearchEvidenceError::InvalidPartition);
        }
        let stored: Stored =
            serde_json::from_str(value).map_err(|_| SearchEvidenceError::InvalidPartition)?;
        ProviderId::try_new(&stored.provider).map_err(|_| SearchEvidenceError::InvalidPartition)?;
        if stored.page == 0
            || stored.grains.len() > Grain::ALL.len()
            || stored
                .grains
                .windows(2)
                .any(|pair| pair[0].as_str() >= pair[1].as_str())
        {
            return Err(SearchEvidenceError::InvalidPartition);
        }
        let context = Self {
            query_digest: stored.query_digest,
            provider: stored.provider,
            page: stored.page,
            locale: stored
                .locale
                .map(MetadataLocale::try_new)
                .transpose()
                .map_err(|_| SearchEvidenceError::InvalidPartition)?,
            region: stored
                .region
                .map(MetadataRegion::try_new)
                .transpose()
                .map_err(|_| SearchEvidenceError::InvalidPartition)?,
            grains: stored.grains,
        };
        // Only our exact normalized persisted representation is accepted.
        if context.to_json()? != value {
            return Err(SearchEvidenceError::InvalidPartition);
        }
        Ok(context)
    }
}

/// Server-side request context, never a public wire DTO. Policy and terms come
/// from the configured provider owner; grant/configuration digests come from storage.
#[derive(Debug, Clone)]
pub struct SearchPageRequest {
    pub correlation_id: RequestCorrelationId,
    pub access: ApplicationAccessContext,
    pub query: SearchProviderQuery,
    pub outbound_policy: OutboundAccessPolicy,
    /// Trusted provider-runtime cache policy; never a browser-selected revision.
    pub terms_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSearchPage {
    pub partition: SearchReceiptPartition,
    pub provider_state: ProviderCapabilityState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSearchPage {
    pub sequence: u64,
    pub candidates: Vec<SearchCandidateReceipt>,
    pub next_page: Option<u32>,
    pub cache_state: SearchCacheState,
    pub lifetime: SearchReceiptLifetime,
    pub response_digest: Sha256Digest,
}

/// Server-side read context for the canonical candidate route. Source and grain
/// are checked against stored evidence; they never select an upstream URL.
#[derive(Debug, Clone)]
pub struct ReadSearchCandidateRequest {
    pub correlation_id: RequestCorrelationId,
    pub access: ApplicationAccessContext,
    pub candidate_receipt_id: SearchCandidateReceiptId,
    pub provider: ProviderId,
    pub grain: Grain,
    pub outbound_policy: OutboundAccessPolicy,
    /// Derive from the current provider descriptor, not the stored receipt.
    pub terms_revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSearchCandidate {
    pub receipt: SearchCandidateReceipt,
    pub context: SearchPageContext,
}

impl StoredSearchCandidate {
    /// Project original cached evidence only. This does not authorize a save or
    /// extend receipt readability; the action transaction must recheck both.
    pub fn metadata_fields(
        &self,
    ) -> Result<Vec<crate::ProviderMetadataField>, SearchEvidenceError> {
        if !self.context.accepts(self.receipt.candidate())
            || self.context.digest() != self.receipt.partition().context_digest
        {
            return Err(SearchEvidenceError::InvalidPartition);
        }
        let lifetime = self.receipt.lifetime();
        // A historical zero-freshness snapshot is permanently stale. Never
        // invent a positive TTL to satisfy the claim's strict expiry ordering.
        let (expiry, status) = if lifetime.fresh_until() == lifetime.created_at() {
            (None, fasti_domain::FieldClaimStatus::Stale)
        } else {
            (
                Some(lifetime.fresh_until()),
                fasti_domain::FieldClaimStatus::Fresh,
            )
        };
        crate::provider_candidate_metadata_fields(
            self.receipt.candidate(),
            self.context.locale().cloned(),
            None,
            self.receipt.response_digest(),
            fasti_domain::ReceivedAt::from_application_clock(lifetime.created_at()),
            expiry,
            status,
        )
    }
}

/// Atomic receipt authorization and provider-read state for one detail fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSearchCandidateDetails {
    pub candidate: StoredSearchCandidate,
    pub provider_state: ProviderCapabilityState,
    pub provider_authority_fingerprint: Sha256Digest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "record_id",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum SearchRecordAction {
    Create,
    Attach(RecordId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchCandidateEvidenceMode {
    Refetch,
    Cached,
}

/// Internal command: the caller supplies intent, never metadata or provenance.
#[derive(Debug, Clone)]
pub struct SearchCandidateActionCommand {
    pub request: ReadSearchCandidateRequest,
    pub operation_id: fasti_domain::OperationId,
    pub action: SearchRecordAction,
    pub evidence_mode: SearchCandidateEvidenceMode,
}

impl SearchCandidateActionCommand {
    pub fn semantic_digest(&self) -> Sha256Digest {
        candidate_action_digest(
            self.request.candidate_receipt_id,
            self.request.provider.as_str(),
            self.request.grain,
            self.action,
            self.evidence_mode,
        )
    }
}

fn candidate_action_digest(
    receipt_id: SearchCandidateReceiptId,
    provider: &str,
    grain: Grain,
    action: SearchRecordAction,
    evidence_mode: SearchCandidateEvidenceMode,
) -> Sha256Digest {
    search_digest(&(
        "fasti.search.record-action.v1",
        receipt_id,
        provider,
        grain,
        action,
        evidence_mode,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchRecordActionDisposition {
    Created,
    Reused,
    Attached,
    AlreadyAttached,
}

/// Historical evidence, not current authorization or a fresh Record projection.
/// Subject IDs survive only as audit values; no session or grant is portable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchCandidateActionReceipt {
    pub workspace_id: WorkspaceId,
    pub profile_id: ProfileId,
    pub actor_client_id: ClientId,
    pub actor_subject_id: Option<AuthSubjectId>,
    pub operation_id: fasti_domain::OperationId,
    pub candidate_receipt_id: SearchCandidateReceiptId,
    pub provider: String,
    pub grain: Grain,
    pub action: SearchRecordAction,
    pub evidence_mode: SearchCandidateEvidenceMode,
    pub record_id: RecordId,
    pub disposition: SearchRecordActionDisposition,
    pub search_context_digest: Sha256Digest,
    pub search_response_digest: Sha256Digest,
    #[serde(deserialize_with = "deserialize_action_provenance")]
    pub provenance: fasti_domain::FieldClaimProvenance,
    pub fetched_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(deserialize_with = "deserialize_action_status")]
    pub initial_status: fasti_domain::FieldClaimStatus,
    pub committed_at: DateTime<Utc>,
}

impl SearchCandidateActionReceipt {
    pub fn semantic_digest(&self) -> Sha256Digest {
        candidate_action_digest(
            self.candidate_receipt_id,
            &self.provider,
            self.grain,
            self.action,
            self.evidence_mode,
        )
    }
}

fn deserialize_action_status<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<fasti_domain::FieldClaimStatus, D::Error> {
    match String::deserialize(deserializer)?.as_str() {
        "fresh" => Ok(fasti_domain::FieldClaimStatus::Fresh),
        "stale" => Ok(fasti_domain::FieldClaimStatus::Stale),
        _ => Err(serde::de::Error::custom("invalid action evidence status")),
    }
}

fn deserialize_action_provenance<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<fasti_domain::FieldClaimProvenance, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Stored {
        provider_id: String,
        source_namespace: String,
        source_identifier: String,
        locale: Option<String>,
        region: Option<String>,
        source_version: Option<String>,
        evidence_digest: Sha256Digest,
    }
    let stored = Stored::deserialize(deserializer)?;
    fasti_domain::FieldClaimProvenance::try_new(
        fasti_domain::MetadataProviderId::try_new(stored.provider_id)
            .map_err(serde::de::Error::custom)?,
        fasti_domain::NamespaceKey::try_new(stored.source_namespace)
            .map_err(serde::de::Error::custom)?,
        stored.source_identifier,
        stored
            .locale
            .map(MetadataLocale::try_new)
            .transpose()
            .map_err(serde::de::Error::custom)?,
        stored
            .region
            .map(MetadataRegion::try_new)
            .transpose()
            .map_err(serde::de::Error::custom)?,
        stored.source_version,
        stored.evidence_digest,
    )
    .map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchCandidateActionPreparation {
    Replay(Box<SearchCandidateActionReceipt>),
    Cached(StoredSearchCandidate),
    Refetch(PreparedSearchCandidateDetails),
}

pub trait SearchPersistencePort: Send + Sync {
    /// Page acquisition can persist evidence, including on a cache miss. This
    /// preliminary check reveals no provider state and grants no reusable proof.
    fn authorize_search_page_request(
        &self,
        correlation_id: RequestCorrelationId,
        access: &ApplicationAccessContext,
    ) -> ApplicationResult<()>;

    /// A candidate snapshot read accepts current Search read authority. It does
    /// not renew the receipt or authorize a later mutation.
    fn authorize_search_candidate_read_request(
        &self,
        correlation_id: RequestCorrelationId,
        access: &ApplicationAccessContext,
    ) -> ApplicationResult<()>;

    /// Save preflight requires current IdentityWrite, not Search. The atomic
    /// owner checks durable replay before requiring Search for a new save.
    fn authorize_search_candidate_action_request(
        &self,
        correlation_id: RequestCorrelationId,
        access: &ApplicationAccessContext,
    ) -> ApplicationResult<()>;

    fn search_local_records(
        &self,
        request: &LocalSearchRequest,
    ) -> ApplicationResult<LocalSearchPage>;
    fn prepare_search_page(
        &self,
        request: &SearchPageRequest,
    ) -> ApplicationResult<PreparedSearchPage>;
    fn commit_search_page(
        &self,
        request: &SearchPageRequest,
        prepared: &PreparedSearchPage,
        candidates: &[SearchCandidate],
        response_digest: &Sha256Digest,
        next_page: Option<u32>,
    ) -> ApplicationResult<StoredSearchPage>;
    fn read_cached_search_page(
        &self,
        request: &SearchPageRequest,
        upstream_unavailable: bool,
    ) -> ApplicationResult<Option<StoredSearchPage>>;
    fn read_search_candidate(
        &self,
        request: &ReadSearchCandidateRequest,
    ) -> ApplicationResult<Option<StoredSearchCandidate>>;
    fn prepare_search_candidate_details(
        &self,
        request: &ReadSearchCandidateRequest,
    ) -> ApplicationResult<Option<PreparedSearchCandidateDetails>>;
    fn prepare_search_candidate_action(
        &self,
        command: &SearchCandidateActionCommand,
    ) -> ApplicationResult<SearchCandidateActionPreparation>;
    fn commit_search_candidate_action(
        &self,
        command: &SearchCandidateActionCommand,
        prepared: &SearchCandidateActionPreparation,
        refetched_fields: Option<&[crate::ProviderMetadataField]>,
    ) -> ApplicationResult<SearchCandidateActionReceipt>;
}

/// The allowlist persisted from provider search. No raw body or request headers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchCandidateData {
    pub provider: String,
    pub provider_id: String,
    pub kind: String,
    pub title: String,
    pub original_title: Option<String>,
    pub release_year: Option<u16>,
    pub authors: Vec<String>,
    pub image_url: Option<String>,
    pub overview: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchEvidenceError {
    InvalidCandidate,
    InvalidPartition,
    InvalidLifetime,
}

impl fmt::Display for SearchEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidCandidate => "search candidate evidence is invalid or exceeds its bound",
            Self::InvalidPartition => "search receipt authorization partition is invalid",
            Self::InvalidLifetime => {
                "search receipt lifetime exceeds policy or has invalid ordering"
            }
        })
    }
}

impl Error for SearchEvidenceError {}

/// Validated normalized data and its canonical provider coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCandidate {
    data: SearchCandidateData,
    identifier: ExternalIdentifierClaim,
}

impl SearchCandidate {
    pub fn try_new(data: SearchCandidateData) -> Result<Self, SearchEvidenceError> {
        let mapping = provider_identity_mapping(&data.provider, &data.kind)
            .ok_or(SearchEvidenceError::InvalidCandidate)?;
        let identifier = mapping
            .identifier(&data.provider_id)
            .map_err(|_| SearchEvidenceError::InvalidCandidate)?;
        if !valid_search_candidate_text(&data.title, 512)
            || data
                .original_title
                .as_deref()
                .is_some_and(|value| !valid_search_candidate_text(value, 512))
            || data
                .overview
                .as_deref()
                .is_some_and(|value| !valid_search_candidate_text(value, 4096))
            || data
                .release_year
                .is_some_and(|year| !(1000..=9999).contains(&year))
            || data.authors.len() > 10
            || data
                .authors
                .iter()
                .any(|value| !valid_search_candidate_text(value, 128))
            || data
                .image_url
                .as_deref()
                .is_some_and(|value| !valid_search_candidate_image(&data.provider, value))
        {
            return Err(SearchEvidenceError::InvalidCandidate);
        }
        let candidate = Self { data, identifier };
        candidate.to_json()?;
        Ok(candidate)
    }

    pub fn data(&self) -> &SearchCandidateData {
        &self.data
    }

    pub fn identifier(&self) -> &ExternalIdentifierClaim {
        &self.identifier
    }

    pub fn to_json(&self) -> Result<String, SearchEvidenceError> {
        let json =
            serde_json::to_string(&self.data).map_err(|_| SearchEvidenceError::InvalidCandidate)?;
        if json.len() > MAX_SEARCH_CANDIDATE_BYTES {
            return Err(SearchEvidenceError::InvalidCandidate);
        }
        Ok(json)
    }

    pub fn from_json(json: &str) -> Result<Self, SearchEvidenceError> {
        if json.len() > MAX_SEARCH_CANDIDATE_BYTES {
            return Err(SearchEvidenceError::InvalidCandidate);
        }
        Self::try_new(
            serde_json::from_str(json).map_err(|_| SearchEvidenceError::InvalidCandidate)?,
        )
    }
}

pub fn valid_search_candidate_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

pub fn valid_search_candidate_image(provider: &str, value: &str) -> bool {
    if value.len() > 2048
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return false;
    }
    let prefixes: &[&str] = match provider {
        "tmdb" => &["https://image.tmdb.org/t/p/w500/"],
        "google-books" => &[
            "https://books.google.com/",
            "https://books.googleusercontent.com/",
        ],
        _ => return false,
    };
    prefixes.iter().any(|prefix| value.starts_with(prefix))
}

/// Stable actor identity survives browser-session rotation. Current grant and
/// provider digests must still be recomputed before every cache/receipt read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SearchReceiptPartition {
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    actor_client_id: ClientId,
    actor_subject_id: Option<AuthSubjectId>,
    grant_id: ProfileGrantId,
    context_digest: Sha256Digest,
    grant_digest: Sha256Digest,
    configuration_digest: Sha256Digest,
    terms_revision: String,
}

impl SearchReceiptPartition {
    pub fn try_new(
        access: AuthorizedApplicationAccess,
        context_digest: Sha256Digest,
        grant_digest: Sha256Digest,
        configuration_digest: Sha256Digest,
        terms_revision: String,
    ) -> Result<Self, SearchEvidenceError> {
        if !valid_search_candidate_text(&terms_revision, fasti_domain::MAX_TERMS_REVISION_BYTES)
            || !terms_revision.bytes().all(|byte| {
                !byte.is_ascii_uppercase()
                    && (byte.is_ascii_alphanumeric() || b"._:-/".contains(&byte))
            })
        {
            return Err(SearchEvidenceError::InvalidPartition);
        }
        Ok(Self {
            workspace_id: access.workspace_id(),
            profile_id: access.profile_id(),
            actor_client_id: access.attribution_client_id(),
            grant_id: access.grant_id(),
            actor_subject_id: match access.actor() {
                AuthorizedActor::BrowserSession {
                    auth_subject_id, ..
                } => Some(auth_subject_id),
                AuthorizedActor::Credential { .. } => None,
            },
            context_digest,
            grant_digest,
            configuration_digest,
            terms_revision,
        })
    }

    pub const fn workspace_id(&self) -> WorkspaceId {
        self.workspace_id
    }
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }
    pub const fn actor_client_id(&self) -> ClientId {
        self.actor_client_id
    }
    pub const fn actor_subject_id(&self) -> Option<AuthSubjectId> {
        self.actor_subject_id
    }
    pub const fn grant_id(&self) -> ProfileGrantId {
        self.grant_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchCacheState {
    Fresh,
    StaleOnError,
}

/// Cache freshness does not extend the candidate receipt's independent expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchReceiptLifetime {
    created_at: DateTime<Utc>,
    fresh_until: DateTime<Utc>,
    stale_until: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl SearchReceiptLifetime {
    pub fn try_new(
        created_at: DateTime<Utc>,
        fresh_until: DateTime<Utc>,
        stale_until: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, SearchEvidenceError> {
        let cap = |seconds| {
            created_at
                .checked_add_signed(Duration::seconds(seconds))
                .ok_or(SearchEvidenceError::InvalidLifetime)
        };
        if fresh_until < created_at
            || fresh_until > cap(SEARCH_FRESH_SECONDS)?
            || stale_until < fresh_until
            || stale_until > cap(SEARCH_STALE_ON_ERROR_SECONDS)?
            || expires_at < stale_until
            || expires_at > cap(SEARCH_RECEIPT_SECONDS)?
        {
            return Err(SearchEvidenceError::InvalidLifetime);
        }
        Ok(Self {
            created_at,
            fresh_until,
            stale_until,
            expires_at,
        })
    }

    pub fn receipt_is_current(&self, now: DateTime<Utc>) -> bool {
        self.created_at <= now && now < self.expires_at
    }

    pub fn cache_state(
        &self,
        now: DateTime<Utc>,
        upstream_unavailable: bool,
    ) -> Option<SearchCacheState> {
        if !self.receipt_is_current(now) {
            return None;
        }
        if now < self.fresh_until {
            Some(SearchCacheState::Fresh)
        } else if upstream_unavailable && now < self.stale_until {
            Some(SearchCacheState::StaleOnError)
        } else {
            None
        }
    }

    pub const fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub const fn fresh_until(&self) -> DateTime<Utc> {
        self.fresh_until
    }
    pub const fn stale_until(&self) -> DateTime<Utc> {
        self.stale_until
    }
    pub const fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCandidateReceipt {
    id: SearchCandidateReceiptId,
    partition: SearchReceiptPartition,
    candidate: SearchCandidate,
    response_digest: Sha256Digest,
    lifetime: SearchReceiptLifetime,
}

impl SearchCandidateReceipt {
    pub fn new(
        id: SearchCandidateReceiptId,
        partition: SearchReceiptPartition,
        candidate: SearchCandidate,
        response_digest: Sha256Digest,
        lifetime: SearchReceiptLifetime,
    ) -> Self {
        Self {
            id,
            partition,
            candidate,
            response_digest,
            lifetime,
        }
    }

    pub const fn id(&self) -> SearchCandidateReceiptId {
        self.id
    }
    pub fn partition(&self) -> &SearchReceiptPartition {
        &self.partition
    }
    pub fn candidate(&self) -> &SearchCandidate {
        &self.candidate
    }
    pub fn response_digest(&self) -> &Sha256Digest {
        &self.response_digest
    }
    pub fn lifetime(&self) -> &SearchReceiptLifetime {
        &self.lifetime
    }

    pub fn is_readable(&self, current: &SearchReceiptPartition, now: DateTime<Utc>) -> bool {
        &self.partition == current && self.lifetime.receipt_is_current(now)
    }
}

#[cfg(test)]
mod tests {
    include!("search_metadata_tests.rs");
    include!("search_action_tests.rs");
    use super::*;
    use fasti_domain::BrowserSessionId;

    #[test]
    fn query_digest_binds_all_coordinates_without_rewriting_search_syntax() {
        let query = SearchProviderQuery::try_new(
            SearchQuery::try_new("Star OR title:Moon").unwrap(),
            ProviderId::try_new("tmdb").unwrap(),
            1,
            None,
            None,
            vec![Grain::Film, Grain::Series],
        )
        .unwrap();
        let mutations: [fn(&mut SearchProviderQuery); 6] = [
            |q| q.query = SearchQuery::try_new("star OR title:Moon").unwrap(),
            |q| q.provider = ProviderId::try_new("google-books").unwrap(),
            |q| q.page = 2,
            |q| q.locale = Some(MetadataLocale::try_new("fr-FR").unwrap()),
            |q| q.region = Some(MetadataRegion::try_new("FR").unwrap()),
            |q| q.grains = vec![Grain::Series],
        ];
        for change in mutations {
            let mut changed = query.clone();
            change(&mut changed);
            assert_ne!(query.digest(), changed.digest());
        }
        let reordered = SearchProviderQuery::try_new(
            query.query.clone(),
            query.provider.clone(),
            1,
            None,
            None,
            vec![Grain::Series, Grain::Film, Grain::Film],
        )
        .unwrap();
        assert_eq!(query.digest(), reordered.digest());
        assert!(!format!("{query:?}").contains("Star OR"));
    }

    fn candidate_data() -> SearchCandidateData {
        SearchCandidateData {
            provider: "tmdb".into(),
            provider_id: "42".into(),
            kind: "movie".into(),
            title: "A film".into(),
            original_title: None,
            release_year: Some(2026),
            authors: vec![],
            image_url: Some("https://image.tmdb.org/t/p/w500/film.jpg".into()),
            overview: Some("A description.".into()),
        }
    }

    #[test]
    fn candidate_receipt_context_is_canonical_bounded_and_contains_no_query_text() {
        let context = SearchProviderQuery::try_new(
            SearchQuery::try_new("Private exact query").unwrap(),
            ProviderId::try_new("tmdb").unwrap(),
            2,
            Some(MetadataLocale::try_new("fr-FR").unwrap()),
            Some(MetadataRegion::try_new("fr").unwrap()),
            vec![Grain::Series, Grain::Film, Grain::Film],
        )
        .unwrap()
        .receipt_context();
        let json = context.to_json().unwrap();
        assert!(!json.contains("Private exact query"));
        assert_eq!(SearchPageContext::from_json(&json).unwrap(), context);
        assert!(context.accepts(&SearchCandidate::try_new(candidate_data()).unwrap()));
        let mutations: [fn(&mut SearchPageContext); 6] = [
            |c| c.query_digest = Sha256Digest::from_bytes(&[0; 32]),
            |c| c.provider = "google-books".into(),
            |c| c.page = 3,
            |c| c.locale = None,
            |c| c.region = None,
            |c| c.grains = vec![Grain::Film],
        ];
        for mutate in mutations {
            let mut changed = context.clone();
            mutate(&mut changed);
            assert_ne!(changed.digest(), context.digest());
        }
        for hostile in [
            json.replace("fr-fr", "FR-fr"),
            json.replace("\"FR\"", "\"fr\""),
            json.replace("\"page\":2", "\"page\":0"),
            json.replace("\"film\",\"series\"", "\"film\",\"film\""),
            json.replace("\"film\",\"series\"", "\"series\",\"film\""),
            json.replace("\"region\":\"FR\",", ""),
            json.replace("\"query_digest\":", "\"extra\":true,\"query_digest\":"),
            " ".repeat(MAX_SEARCH_CONTEXT_BYTES + 1),
        ] {
            assert!(SearchPageContext::from_json(&hostile).is_err(), "{hostile}");
        }
    }

    fn lifetime() -> SearchReceiptLifetime {
        let created = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        SearchReceiptLifetime::try_new(
            created,
            created + Duration::seconds(120),
            created + Duration::seconds(600),
            created + Duration::seconds(86400),
        )
        .unwrap()
    }

    #[test]
    fn persisted_candidates_revalidate_coordinates_bounds_and_allowlisted_artwork() {
        let candidate = SearchCandidate::try_new(candidate_data()).unwrap();
        assert_eq!(
            SearchCandidate::from_json(&candidate.to_json().unwrap()).unwrap(),
            candidate
        );
        let mut data = candidate_data();
        data.provider_id = "42/../43".into();
        assert!(SearchCandidate::try_new(data).is_err());
        for title in [
            String::new(),
            " title".into(),
            "line\nbreak".into(),
            "é".repeat(257),
        ] {
            let mut data = candidate_data();
            data.title = title;
            assert!(SearchCandidate::try_new(data).is_err());
        }
        for image in [
            "http://image.tmdb.org/t/p/w500/a.jpg",
            "https://image.tmdb.org.evil/t/p/w500/a.jpg",
            "https://image.tmdb.org@evil/t/p/w500/a.jpg",
            "https://image.tmdb.org/t/p/w500/bad image.jpg",
        ] {
            let mut data = candidate_data();
            data.image_url = Some(image.into());
            assert!(SearchCandidate::try_new(data).is_err());
        }
        let mut json: serde_json::Value =
            serde_json::from_str(&candidate.to_json().unwrap()).unwrap();
        json["secret"] = "not retained".into();
        assert!(SearchCandidate::from_json(&json.to_string()).is_err());
        assert!(SearchCandidate::from_json(&" ".repeat(MAX_SEARCH_CANDIDATE_BYTES + 1)).is_err());
        let mut data = candidate_data();
        data.authors = vec!["Author".into(); 11];
        assert!(SearchCandidate::try_new(data).is_err());
        let mut data = candidate_data();
        data.release_year = Some(999);
        assert!(SearchCandidate::try_new(data).is_err());
    }

    #[test]
    fn cache_and_details_have_separate_exclusive_deadlines_without_swr() {
        let life = lifetime();
        let created = life.created_at();
        assert_eq!(
            life.cache_state(created, false),
            Some(SearchCacheState::Fresh)
        );
        assert_eq!(life.cache_state(created - Duration::seconds(1), true), None);
        assert_eq!(life.cache_state(life.fresh_until(), false), None);
        assert_eq!(
            life.cache_state(life.fresh_until(), true),
            Some(SearchCacheState::StaleOnError)
        );
        assert_eq!(life.cache_state(life.stale_until(), true), None);
        assert!(life.receipt_is_current(life.stale_until()));
        assert!(!life.receipt_is_current(life.expires_at()));
        assert!(SearchReceiptLifetime::try_new(
            created,
            life.fresh_until() + Duration::seconds(1),
            life.stale_until(),
            life.expires_at()
        )
        .is_err());
        assert!(SearchReceiptLifetime::try_new(
            created,
            life.fresh_until(),
            life.stale_until() + Duration::seconds(1),
            life.expires_at()
        )
        .is_err());
        assert!(SearchReceiptLifetime::try_new(
            created,
            life.fresh_until(),
            life.stale_until(),
            life.expires_at() + Duration::seconds(1)
        )
        .is_err());
        assert!(SearchReceiptLifetime::try_new(
            created,
            created - Duration::seconds(1),
            life.stale_until(),
            life.expires_at()
        )
        .is_err());
        assert!(SearchReceiptLifetime::try_new(
            DateTime::<Utc>::MAX_UTC,
            DateTime::<Utc>::MAX_UTC,
            DateTime::<Utc>::MAX_UTC,
            DateTime::<Utc>::MAX_UTC
        )
        .is_err());
    }

    #[test]
    fn receipt_rechecks_every_partition_dimension_but_allows_session_rotation() {
        let subject = AuthSubjectId::new_v7();
        let client = ClientId::new_v7();
        let workspace = WorkspaceId::new_v7();
        let profile = ProfileId::new_v7();
        let grant = ProfileGrantId::new_v7();
        let partition = |session| {
            SearchReceiptPartition::try_new(
                AuthorizedApplicationAccess::new(
                    workspace,
                    profile,
                    grant,
                    AuthorizedActor::BrowserSession {
                        auth_subject_id: subject,
                        browser_session_id: session,
                        grant_owner_client_id: client,
                    },
                ),
                Sha256Digest::from_bytes(&[1; 32]),
                Sha256Digest::from_bytes(&[2; 32]),
                Sha256Digest::from_bytes(&[3; 32]),
                "provider-v1".into(),
            )
            .unwrap()
        };
        let first = partition(BrowserSessionId::new_v7());
        let receipt = SearchCandidateReceipt::new(
            SearchCandidateReceiptId::new_v7(),
            first.clone(),
            SearchCandidate::try_new(candidate_data()).unwrap(),
            Sha256Digest::from_bytes(&[4; 32]),
            lifetime(),
        );
        assert!(receipt.is_readable(
            &partition(BrowserSessionId::new_v7()),
            lifetime().created_at()
        ));
        let mutations: [fn(&mut SearchReceiptPartition); 10] = [
            |p| p.workspace_id = WorkspaceId::new_v7(),
            |p| p.profile_id = ProfileId::new_v7(),
            |p| p.actor_client_id = ClientId::new_v7(),
            |p| p.actor_subject_id = Some(AuthSubjectId::new_v7()),
            |p| p.actor_subject_id = None,
            |p| p.grant_id = ProfileGrantId::new_v7(),
            |p| p.context_digest = Sha256Digest::from_bytes(&[9; 32]),
            |p| p.grant_digest = Sha256Digest::from_bytes(&[9; 32]),
            |p| p.configuration_digest = Sha256Digest::from_bytes(&[9; 32]),
            |p| p.terms_revision = "provider-v2".into(),
        ];
        for mutate in mutations {
            let mut changed = first.clone();
            mutate(&mut changed);
            assert!(!receipt.is_readable(&changed, lifetime().created_at()));
        }
        assert!(!receipt.is_readable(&first, lifetime().expires_at()));
    }
}
