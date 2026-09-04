//! Normalized Search evidence. Candidates remain separate from Fasti Records.

use crate::{
    provider_identity_mapping, ApplicationAccessContext, ApplicationResult, AuthorizedActor,
    AuthorizedApplicationAccess, OutboundAccessPolicy, ProviderCapabilityState, ProviderId,
};
use chrono::{DateTime, Duration, Utc};
use fasti_domain::{
    AuthSubjectId, ClientId, ExternalIdentifierClaim, Grain, MetadataLocale, MetadataRegion,
    ProfileGrantId, ProfileId, RequestCorrelationId, SearchCandidateReceiptId, SearchQuery,
    Sha256Digest, WorkspaceId,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

pub const MAX_SEARCH_CANDIDATE_BYTES: usize = 64 * 1024;
pub const SEARCH_FRESH_SECONDS: i64 = 120;
pub const SEARCH_STALE_ON_ERROR_SECONDS: i64 = 600;
pub const SEARCH_RECEIPT_SECONDS: i64 = 24 * 60 * 60;
pub const MAX_SEARCH_PAGE_CANDIDATES: usize = 100;

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
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(&(
            "fasti.search.page.v1",
            self.query.as_str(),
            self.provider.as_str(),
            self.page,
            &self.locale,
            &self.region,
            &self.grains,
        ))
        .expect("fixed Search tuple contains only serializable values");
        Sha256Digest::from_bytes(&Sha256::digest(bytes).into())
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
}

pub trait SearchPersistencePort {
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
    query_digest: Sha256Digest,
    grant_digest: Sha256Digest,
    configuration_digest: Sha256Digest,
    terms_revision: String,
}

impl SearchReceiptPartition {
    pub fn try_new(
        access: AuthorizedApplicationAccess,
        query_digest: Sha256Digest,
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
            query_digest,
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
            |p| p.query_digest = Sha256Digest::from_bytes(&[9; 32]),
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
