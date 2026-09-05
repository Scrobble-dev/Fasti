//! Provider Search representations. Receipt authority remains server-side.

use fasti_application::{
    ProviderCandidateDetailsOutcome, ProviderIdentifierActionOutcome, ProviderSearchActionOutcome,
    ProviderSearchOutcome, SearchCacheState, SearchCandidate, SearchCandidateReceipt,
    SearchProviderQuery, SearchReceiptLifetime, StoredSearchCandidate,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalSearchRequestDto {
    #[schemars(length(min = 1, max = 256))]
    #[schema(min_length = 1, max_length = 256)]
    pub query: String,
    #[schemars(length(max = 16))]
    #[schema(max_items = 16)]
    pub grains: Vec<String>,
    pub after: Option<LocalSearchCursorDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalSearchCursorDto {
    #[schemars(
        length(equal = 36),
        regex(pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub last_record_id: String,
    #[schemars(length(equal = 71), regex(pattern = r"^sha256:[0-9a-f]{64}$"))]
    #[schema(min_length = 71, max_length = 71, pattern = r"^sha256:[0-9a-f]{64}$")]
    pub context_digest: String,
}

impl From<fasti_application::LocalSearchCursor> for LocalSearchCursorDto {
    fn from(value: fasti_application::LocalSearchCursor) -> Self {
        Self {
            last_record_id: value.last_record_id.to_string(),
            context_digest: value.context_digest.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalSearchResponseDto {
    #[schemars(length(max = 100))]
    #[schema(max_items = 100)]
    pub records: Vec<crate::RecordSummaryDto>,
    pub next: Option<LocalSearchCursorDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchProviderPageRequest {
    #[schemars(length(min = 1, max = 256))]
    #[schema(min_length = 1, max_length = 256)]
    pub query: String,
    #[schemars(range(min = 1))]
    #[schema(minimum = 1)]
    pub page: u32,
    #[schemars(length(min = 2, max = 16))]
    #[schema(min_length = 2, max_length = 16)]
    pub locale: Option<String>,
    /// Reserved query context; current provider Search routes do not filter by region.
    #[schemars(length(min = 2, max = 8))]
    #[schema(min_length = 2, max_length = 8)]
    pub region: Option<String>,
    #[schemars(length(max = 32))]
    #[schema(max_items = 32)]
    pub grains: Vec<String>,
    pub offline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum SearchProviderPageResponse {
    Live {
        provider_id: String,
        page: u32,
        #[schemars(length(max = 100))]
        #[schema(max_items = 100)]
        candidates: Vec<SearchCandidateDto>,
        next_page: Option<u32>,
    },
    Page {
        provider_id: String,
        page: u32,
        #[schemars(length(max = 100))]
        #[schema(max_items = 100)]
        candidates: Vec<SearchCandidateReceiptDto>,
        next_page: Option<u32>,
        cache_state: SearchCacheStateDto,
        lifetime: SearchReceiptLifetimeDto,
        upstream_problem: Option<String>,
    },
    Unavailable {
        provider_id: String,
        problem_code: String,
    },
}

impl SearchProviderPageResponse {
    /// Project a governed result with the validated request's route coordinates.
    pub fn from_outcome(query: &SearchProviderQuery, outcome: ProviderSearchOutcome) -> Self {
        let provider_id = query.provider().as_str().to_owned();
        match outcome {
            ProviderSearchOutcome::Live {
                candidates,
                next_page,
            } => Self::Live {
                provider_id,
                page: query.page(),
                candidates: candidates.iter().map(Into::into).collect(),
                next_page,
            },
            ProviderSearchOutcome::Page {
                page,
                upstream_problem,
            } => Self::Page {
                provider_id,
                page: query.page(),
                candidates: page.candidates.iter().map(Into::into).collect(),
                next_page: page.next_page,
                cache_state: page.cache_state.into(),
                lifetime: (&page.lifetime).into(),
                upstream_problem: upstream_problem.map(|code| code.as_str().to_owned()),
            },
            ProviderSearchOutcome::Unavailable { problem } => Self::Unavailable {
                provider_id,
                problem_code: problem.as_str().to_owned(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchCacheStateDto {
    Observed,
    Fresh,
    StaleOnError,
}

impl From<SearchCacheState> for SearchCacheStateDto {
    fn from(value: SearchCacheState) -> Self {
        match value {
            SearchCacheState::Observed => Self::Observed,
            SearchCacheState::Fresh => Self::Fresh,
            SearchCacheState::StaleOnError => Self::StaleOnError,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchReceiptLifetimeDto {
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub fresh_until: String,
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub stale_until: String,
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub expires_at: String,
}

impl From<&SearchReceiptLifetime> for SearchReceiptLifetimeDto {
    fn from(value: &SearchReceiptLifetime) -> Self {
        Self {
            created_at: value.created_at().to_rfc3339(),
            fresh_until: value.fresh_until().to_rfc3339(),
            stale_until: value.stale_until().to_rfc3339(),
            expires_at: value.expires_at().to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCandidateReceiptDto {
    #[schemars(
        length(equal = 36),
        regex(pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub candidate_receipt_id: String,
    pub grain: String,
    pub candidate: SearchCandidateDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCandidateDto {
    pub provider: String,
    pub provider_id: String,
    pub grain: String,
    pub kind: String,
    #[schemars(length(min = 1, max = 512))]
    #[schema(min_length = 1, max_length = 512)]
    pub title: String,
    #[schemars(length(min = 1, max = 512))]
    #[schema(min_length = 1, max_length = 512)]
    pub original_title: Option<String>,
    #[schemars(range(min = 1000, max = 9999))]
    #[schema(minimum = 1000, maximum = 9999)]
    pub release_year: Option<u16>,
    #[schemars(length(max = 10))]
    #[schema(max_items = 10)]
    pub authors: Vec<String>,
    #[schemars(length(max = 2048))]
    #[schema(max_length = 2048)]
    pub image_url: Option<String>,
    #[schemars(length(min = 1, max = 4096))]
    #[schema(min_length = 1, max_length = 4096)]
    pub overview: Option<String>,
}

impl From<&SearchCandidateReceipt> for SearchCandidateReceiptDto {
    fn from(value: &SearchCandidateReceipt) -> Self {
        Self {
            candidate_receipt_id: value.id().to_string(),
            grain: value.candidate().identifier().grain().as_str().to_owned(),
            candidate: value.candidate().into(),
        }
    }
}

impl From<&SearchCandidate> for SearchCandidateDto {
    fn from(value: &SearchCandidate) -> Self {
        let data = value.data();
        Self {
            provider: data.provider.clone(),
            provider_id: data.provider_id.clone(),
            grain: value.identifier().grain().as_str().to_owned(),
            kind: data.kind.clone(),
            title: data.title.clone(),
            original_title: data.original_title.clone(),
            release_year: data.release_year,
            authors: data.authors.clone(),
            image_url: data.image_url.clone(),
            overview: data.overview.clone(),
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema, utoipa::IntoParams,
)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct SearchCandidateDetailsQueryParameters {
    pub offline: bool,
}

/// Original Search evidence. Its lifetime never describes a subsequent refetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCandidateSnapshotDto {
    pub receipt: SearchCandidateReceiptDto,
    pub lifetime: SearchReceiptLifetimeDto,
    pub locale: Option<String>,
}

impl From<&StoredSearchCandidate> for SearchCandidateSnapshotDto {
    fn from(value: &StoredSearchCandidate) -> Self {
        Self {
            receipt: (&value.receipt).into(),
            lifetime: value.receipt.lifetime().into(),
            locale: value
                .context
                .locale()
                .map(|locale| locale.as_str().to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum SearchCandidateDetailsResponse {
    Missing {},
    Snapshot {
        snapshot: SearchCandidateSnapshotDto,
    },
    Refetched {
        snapshot: Box<SearchCandidateSnapshotDto>,
        details: SearchCandidateDto,
        locale: Option<String>,
    },
    Unavailable {
        snapshot: SearchCandidateSnapshotDto,
        problem_code: String,
    },
    RefetchedWithoutSnapshot {
        #[schemars(
            length(equal = 36),
            regex(pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
        )]
        #[schema(
            min_length = 36,
            max_length = 36,
            pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
        )]
        candidate_receipt_id: String,
        provider_id: String,
        grain: String,
        details: SearchCandidateDto,
        locale: Option<String>,
    },
    UnavailableWithoutSnapshot {
        #[schemars(
            length(equal = 36),
            regex(pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
        )]
        #[schema(
            min_length = 36,
            max_length = 36,
            pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
        )]
        candidate_receipt_id: String,
        provider_id: String,
        grain: String,
        problem_code: String,
    },
}

impl From<Option<ProviderCandidateDetailsOutcome>> for SearchCandidateDetailsResponse {
    fn from(value: Option<ProviderCandidateDetailsOutcome>) -> Self {
        match value {
            None => Self::Missing {},
            Some(ProviderCandidateDetailsOutcome::Snapshot(snapshot)) => Self::Snapshot {
                snapshot: (&snapshot).into(),
            },
            Some(ProviderCandidateDetailsOutcome::Unavailable { snapshot, problem }) => {
                Self::Unavailable {
                    snapshot: (&snapshot).into(),
                    problem_code: problem.as_str().to_owned(),
                }
            }
            Some(ProviderCandidateDetailsOutcome::Refetched {
                snapshot,
                details,
                locale,
            }) => Self::Refetched {
                snapshot: Box::new((&snapshot).into()),
                details: details.as_ref().into(),
                locale: locale.map(|value| value.as_str().to_owned()),
            },
            Some(ProviderCandidateDetailsOutcome::RefetchedWithoutSnapshot {
                candidate_receipt_id,
                provider,
                grain,
                details,
                locale,
            }) => Self::RefetchedWithoutSnapshot {
                candidate_receipt_id: candidate_receipt_id.to_string(),
                provider_id: provider.as_str().to_owned(),
                grain: grain.as_str().to_owned(),
                details: details.as_ref().into(),
                locale: locale.map(|value| value.as_str().to_owned()),
            },
            Some(ProviderCandidateDetailsOutcome::UnavailableWithoutSnapshot {
                candidate_receipt_id,
                provider,
                grain,
                problem,
            }) => Self::UnavailableWithoutSnapshot {
                candidate_receipt_id: candidate_receipt_id.to_string(),
                provider_id: provider.as_str().to_owned(),
                grain: grain.as_str().to_owned(),
                problem_code: problem.as_str().to_owned(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCandidateActionRequest {
    #[schemars(
        length(equal = 35),
        regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub operation_id: String,
    pub action: SearchRecordActionDto,
    pub evidence_mode: SearchCandidateEvidenceModeDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SearchRecordActionDto {
    Create {},
    Attach {
        #[schemars(
            length(equal = 36),
            regex(pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
        )]
        #[schema(
            min_length = 36,
            max_length = 36,
            pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
        )]
        record_id: String,
    },
}

impl From<fasti_application::SearchRecordAction> for SearchRecordActionDto {
    fn from(value: fasti_application::SearchRecordAction) -> Self {
        match value {
            fasti_application::SearchRecordAction::Create => Self::Create {},
            fasti_application::SearchRecordAction::Attach(record_id) => Self::Attach {
                record_id: record_id.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchCandidateEvidenceModeDto {
    Cached,
    Refetch,
}

impl From<SearchCandidateEvidenceModeDto> for fasti_application::SearchCandidateEvidenceMode {
    fn from(value: SearchCandidateEvidenceModeDto) -> Self {
        match value {
            SearchCandidateEvidenceModeDto::Cached => Self::Cached,
            SearchCandidateEvidenceModeDto::Refetch => Self::Refetch,
        }
    }
}

impl From<fasti_application::SearchCandidateEvidenceMode> for SearchCandidateEvidenceModeDto {
    fn from(value: fasti_application::SearchCandidateEvidenceMode) -> Self {
        match value {
            fasti_application::SearchCandidateEvidenceMode::Cached => Self::Cached,
            fasti_application::SearchCandidateEvidenceMode::Refetch => Self::Refetch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchRecordActionDispositionDto {
    Created,
    Reused,
    Attached,
    AlreadyAttached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchEvidenceStatusDto {
    Fresh,
    Stale,
}

/// Immutable acceptance history, not current Record state or current freshness.
/// Internal actor, authorization and query-context data never enter this DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchCandidateActionReceiptDto {
    #[schemars(
        length(equal = 35),
        regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub operation_id: String,
    #[schemars(
        length(equal = 36),
        regex(pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^scr_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub candidate_receipt_id: String,
    pub provider_id: String,
    pub grain: String,
    pub action: SearchRecordActionDto,
    pub evidence_mode: SearchCandidateEvidenceModeDto,
    #[schemars(
        length(equal = 36),
        regex(pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub record_id: String,
    pub disposition: SearchRecordActionDispositionDto,
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub fetched_at: String,
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub expires_at: Option<String>,
    pub initial_status: SearchEvidenceStatusDto,
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub committed_at: String,
}

impl TryFrom<&fasti_application::SearchCandidateActionReceipt> for SearchCandidateActionReceiptDto {
    type Error = &'static str;
    fn try_from(
        value: &fasti_application::SearchCandidateActionReceipt,
    ) -> Result<Self, Self::Error> {
        use fasti_application::SearchRecordActionDisposition as Disposition;
        let initial_status = match value.initial_status {
            fasti_domain::FieldClaimStatus::Fresh => SearchEvidenceStatusDto::Fresh,
            fasti_domain::FieldClaimStatus::Stale => SearchEvidenceStatusDto::Stale,
            _ => return Err("Search action receipt has invalid historical status"),
        };
        Ok(Self {
            operation_id: value.operation_id.to_string(),
            candidate_receipt_id: value.candidate_receipt_id.to_string(),
            provider_id: value.provider.clone(),
            grain: value.grain.as_str().to_owned(),
            action: value.action.into(),
            evidence_mode: value.evidence_mode.into(),
            record_id: value.record_id.to_string(),
            disposition: match value.disposition {
                Disposition::Created => SearchRecordActionDispositionDto::Created,
                Disposition::Reused => SearchRecordActionDispositionDto::Reused,
                Disposition::Attached => SearchRecordActionDispositionDto::Attached,
                Disposition::AlreadyAttached => SearchRecordActionDispositionDto::AlreadyAttached,
            },
            fetched_at: value.fetched_at.to_rfc3339(),
            expires_at: value.expires_at.map(|at| at.to_rfc3339()),
            initial_status,
            committed_at: value.committed_at.to_rfc3339(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum SearchCandidateActionResponse {
    Saved {
        receipt: SearchCandidateActionReceiptDto,
    },
    Unavailable {
        problem_code: String,
    },
}

impl TryFrom<ProviderSearchActionOutcome> for SearchCandidateActionResponse {
    type Error = &'static str;

    fn try_from(value: ProviderSearchActionOutcome) -> Result<Self, Self::Error> {
        Ok(match value {
            ProviderSearchActionOutcome::Saved(receipt) => Self::Saved {
                receipt: receipt.as_ref().try_into()?,
            },
            ProviderSearchActionOutcome::Unavailable { problem } => Self::Unavailable {
                problem_code: problem.as_str().to_owned(),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderIdentifierActionRequest {
    #[schemars(
        length(equal = 35),
        regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub operation_id: String,
    #[schemars(length(min = 1, max = 256))]
    #[schema(min_length = 1, max_length = 256)]
    pub provider_record_id: String,
    pub action: SearchRecordActionDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderIdentifierActionOriginDto {
    UserSelectedProviderIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderIdentifierActionReceiptDto {
    #[schemars(
        length(equal = 35),
        regex(pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 35,
        max_length = 35,
        pattern = r"^op_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub operation_id: String,
    pub provider_id: String,
    #[schemars(length(min = 1, max = 256))]
    #[schema(min_length = 1, max_length = 256)]
    pub provider_record_id: String,
    pub grain: String,
    pub action: SearchRecordActionDto,
    pub origin: ProviderIdentifierActionOriginDto,
    #[schemars(
        length(equal = 36),
        regex(pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$")
    )]
    #[schema(
        min_length = 36,
        max_length = 36,
        pattern = r"^rec_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$"
    )]
    pub record_id: String,
    pub disposition: SearchRecordActionDispositionDto,
    #[schemars(extend("format" = "date-time"))]
    #[schema(format = "date-time")]
    pub committed_at: String,
}

impl From<&fasti_application::ProviderIdentifierActionReceipt>
    for ProviderIdentifierActionReceiptDto
{
    fn from(value: &fasti_application::ProviderIdentifierActionReceipt) -> Self {
        use fasti_application::SearchRecordActionDisposition as Disposition;
        Self {
            operation_id: value.operation_id.to_string(),
            provider_id: value.provider.clone(),
            provider_record_id: value.provider_record_id.clone(),
            grain: value.grain.as_str().to_owned(),
            action: value.action.into(),
            origin: ProviderIdentifierActionOriginDto::UserSelectedProviderIdentifier,
            record_id: value.record_id.to_string(),
            disposition: match value.disposition {
                Disposition::Created => SearchRecordActionDispositionDto::Created,
                Disposition::Reused => SearchRecordActionDispositionDto::Reused,
                Disposition::Attached => SearchRecordActionDispositionDto::Attached,
                Disposition::AlreadyAttached => SearchRecordActionDispositionDto::AlreadyAttached,
            },
            committed_at: value.committed_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProviderIdentifierActionResponse {
    Saved {
        receipt: ProviderIdentifierActionReceiptDto,
    },
    Unavailable {
        problem_code: String,
    },
}

impl From<ProviderIdentifierActionOutcome> for ProviderIdentifierActionResponse {
    fn from(value: ProviderIdentifierActionOutcome) -> Self {
        match value {
            ProviderIdentifierActionOutcome::Saved(receipt) => Self::Saved {
                receipt: receipt.as_ref().into(),
            },
            ProviderIdentifierActionOutcome::Unavailable { problem } => Self::Unavailable {
                problem_code: problem.as_str().to_owned(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_application::{
        AuthorizedActor, AuthorizedApplicationAccess, ProblemCode, ProviderId,
        ProviderResponseCachePolicy, ProviderResponseReuse, SearchCandidateActionReceipt,
        SearchCandidateData, SearchCandidateEvidenceMode, SearchReceiptPartition,
        SearchRecordAction, SearchRecordActionDisposition, StoredSearchPage,
    };
    use fasti_domain::{
        AuthSubjectId, BrowserSessionId, ClientId, FieldClaimStatus, Grain, MetadataLocale,
        OperationId, ProfileGrantId, ProfileId, RecordId, SearchCandidateReceiptId, SearchQuery,
        Sha256Digest, WorkspaceId,
    };

    fn query() -> SearchProviderQuery {
        SearchProviderQuery::try_new(
            SearchQuery::try_new("private query").unwrap(),
            ProviderId::try_new("tmdb").unwrap(),
            7,
            Some(MetadataLocale::try_new("fr-FR").unwrap()),
            None,
            vec![Grain::Film],
        )
        .unwrap()
    }

    fn snapshot() -> StoredSearchCandidate {
        let context = query().receipt_context();
        let life = SearchReceiptLifetime::try_new(
            "2026-09-05T10:00:00Z".parse().unwrap(),
            "2026-09-05T10:02:00Z".parse().unwrap(),
            "2026-09-05T10:10:00Z".parse().unwrap(),
            "2026-09-06T10:00:00Z".parse().unwrap(),
        )
        .unwrap();
        let partition = SearchReceiptPartition::try_new(
            AuthorizedApplicationAccess::new(
                WorkspaceId::new_v7(),
                ProfileId::new_v7(),
                ProfileGrantId::new_v7(),
                AuthorizedActor::BrowserSession {
                    auth_subject_id: AuthSubjectId::new_v7(),
                    browser_session_id: BrowserSessionId::new_v7(),
                    grant_owner_client_id: ClientId::new_v7(),
                },
            ),
            context.digest(),
            Sha256Digest::from_bytes(&[1; 32]),
            Sha256Digest::from_bytes(&[2; 32]),
            "fixture.v1".into(),
        )
        .unwrap();
        StoredSearchCandidate {
            response_policy: ProviderResponseCachePolicy::new(
                ProviderResponseReuse::Reusable,
                life.created_at(),
                std::time::Duration::ZERO,
                None,
                None,
            ),
            receipt: SearchCandidateReceipt::new(
                SearchCandidateReceiptId::new_v7(),
                partition,
                SearchCandidate::try_new(SearchCandidateData {
                    provider: "tmdb".into(),
                    provider_id: "42".into(),
                    kind: "movie".into(),
                    title: "Original observation".into(),
                    original_title: None,
                    release_year: Some(2026),
                    authors: vec![],
                    image_url: None,
                    overview: None,
                })
                .unwrap(),
                Sha256Digest::from_bytes(&[3; 32]),
                life,
            ),
            context,
        }
    }

    #[test]
    fn page_projection_preserves_context_continuation_receipts_and_lifetime() {
        let snapshot = snapshot();
        let candidate = snapshot.receipt.candidate();
        let life = snapshot.receipt.lifetime();
        for populated in [false, true] {
            for next_page in [None, Some(8)] {
                let candidates = if populated {
                    vec![candidate.clone()]
                } else {
                    vec![]
                };
                assert_eq!(
                    serde_json::to_value(SearchProviderPageResponse::from_outcome(
                        &query(),
                        ProviderSearchOutcome::Live {
                            candidates,
                            next_page
                        }
                    ))
                    .unwrap(),
                    serde_json::json!({"outcome":"live", "provider_id":"tmdb", "page":7,
                        "candidates": if populated { vec![SearchCandidateDto::from(candidate)] } else { vec![] },
                        "next_page":next_page}),
                );
                for (cache_state, state) in [
                    (SearchCacheState::Fresh, "fresh"),
                    (SearchCacheState::Observed, "observed"),
                    (SearchCacheState::StaleOnError, "stale_on_error"),
                ] {
                    let upstream_problem = (cache_state == SearchCacheState::StaleOnError)
                        .then_some(ProblemCode::ProviderResponseInvalid);
                    let response = SearchProviderPageResponse::from_outcome(
                        &query(),
                        ProviderSearchOutcome::Page {
                            page: StoredSearchPage {
                                sequence: 91,
                                candidates: if populated {
                                    vec![snapshot.receipt.clone()]
                                } else {
                                    vec![]
                                },
                                next_page,
                                cache_state,
                                lifetime: life.clone(),
                                response_digest: Sha256Digest::from_bytes(&[4; 32]),
                            },
                            upstream_problem,
                        },
                    );
                    assert_eq!(
                        serde_json::to_value(response).unwrap(),
                        serde_json::json!({
                            "outcome":"page", "provider_id":"tmdb", "page":7,
                            "candidates": if populated { vec![SearchCandidateReceiptDto::from(&snapshot.receipt)] } else { vec![] },
                            "next_page":next_page, "cache_state":state,
                            "lifetime": SearchReceiptLifetimeDto::from(life),
                            "upstream_problem":upstream_problem.map(|code| code.as_str()),
                        })
                    );
                }
            }
        }
        assert_eq!(
            serde_json::to_value(SearchProviderPageResponse::from_outcome(
                &query(),
                ProviderSearchOutcome::Unavailable {
                    problem: ProblemCode::ProviderResponseInvalid
                }
            ))
            .unwrap(),
            serde_json::json!({"outcome":"unavailable", "provider_id":"tmdb", "problem_code":"provider_response_invalid"})
        );
    }

    #[test]
    fn details_projection_keeps_original_evidence_separate_from_refetch() {
        let snapshot = snapshot();
        let original = SearchCandidateSnapshotDto::from(&snapshot);
        let mut data = snapshot.receipt.candidate().data().clone();
        data.title = "New observation".into();
        let details = SearchCandidate::try_new(data).unwrap();
        let dto = SearchCandidateDto::from(&details);
        let receipt = snapshot.receipt.id();
        for locale in [None, Some(MetadataLocale::try_new("en-US").unwrap())] {
            for (outcome, expected) in [
                (None, serde_json::json!({"outcome":"missing"})),
                (
                    Some(ProviderCandidateDetailsOutcome::Snapshot(snapshot.clone())),
                    serde_json::json!({"outcome":"snapshot", "snapshot":original}),
                ),
                (
                    Some(ProviderCandidateDetailsOutcome::Unavailable {
                        snapshot: snapshot.clone(),
                        problem: ProblemCode::ProviderResponseInvalid,
                    }),
                    serde_json::json!({"outcome":"unavailable", "snapshot":original, "problem_code":"provider_response_invalid"}),
                ),
                (
                    Some(ProviderCandidateDetailsOutcome::Refetched {
                        snapshot: snapshot.clone(),
                        details: Box::new(details.clone()),
                        locale: locale.clone(),
                    }),
                    serde_json::json!({"outcome":"refetched", "snapshot":original, "details":dto, "locale":locale.as_ref().map(|value| value.as_str())}),
                ),
                (
                    Some(ProviderCandidateDetailsOutcome::RefetchedWithoutSnapshot {
                        candidate_receipt_id: receipt,
                        provider: query().provider().clone(),
                        grain: Grain::Film,
                        details: Box::new(details.clone()),
                        locale: locale.clone(),
                    }),
                    serde_json::json!({"outcome":"refetched_without_snapshot", "candidate_receipt_id":receipt, "provider_id":"tmdb", "grain":"film", "details":dto, "locale":locale.as_ref().map(|value| value.as_str())}),
                ),
                (
                    Some(
                        ProviderCandidateDetailsOutcome::UnavailableWithoutSnapshot {
                            candidate_receipt_id: receipt,
                            provider: query().provider().clone(),
                            grain: Grain::Film,
                            problem: ProblemCode::ProviderResponseInvalid,
                        },
                    ),
                    serde_json::json!({"outcome":"unavailable_without_snapshot", "candidate_receipt_id":receipt, "provider_id":"tmdb", "grain":"film", "problem_code":"provider_response_invalid"}),
                ),
            ] {
                let actual =
                    serde_json::to_value(SearchCandidateDetailsResponse::from(outcome)).unwrap();
                assert_eq!(actual, expected);
                assert!(
                    serde_json::from_value::<SearchCandidateDetailsResponse>(actual.clone())
                        .is_ok()
                );
                let mut mixed = actual.clone();
                mixed[if actual.get("snapshot").is_none() {
                    "snapshot"
                } else {
                    "candidate_receipt_id"
                }] = serde_json::json!({});
                assert!(serde_json::from_value::<SearchCandidateDetailsResponse>(mixed).is_err());
                for required in ["candidate_receipt_id", "details"] {
                    let mut incomplete = actual.clone();
                    if incomplete
                        .as_object_mut()
                        .unwrap()
                        .remove(required)
                        .is_some()
                    {
                        assert!(serde_json::from_value::<SearchCandidateDetailsResponse>(
                            incomplete
                        )
                        .is_err());
                    }
                }
            }
        }
    }

    #[test]
    fn action_projection_preserves_history_and_rejects_invalid_status() {
        let snapshot = snapshot();
        let life = snapshot.receipt.lifetime();
        let mut receipt = SearchCandidateActionReceipt {
            workspace_id: WorkspaceId::new_v7(),
            profile_id: ProfileId::new_v7(),
            actor_client_id: ClientId::new_v7(),
            actor_subject_id: Some(AuthSubjectId::new_v7()),
            operation_id: OperationId::new_v7(),
            candidate_receipt_id: snapshot.receipt.id(),
            provider: "tmdb".into(),
            grain: Grain::Film,
            action: SearchRecordAction::Create,
            evidence_mode: SearchCandidateEvidenceMode::Cached,
            record_id: RecordId::new_v7(),
            disposition: SearchRecordActionDisposition::Created,
            search_context_digest: snapshot.context.digest(),
            search_response_digest: Sha256Digest::from_bytes(&[3; 32]),
            provenance: snapshot.metadata_fields().unwrap()[0]
                .claim()
                .provenance()
                .clone(),
            fetched_at: life.created_at(),
            expires_at: Some(life.fresh_until()),
            initial_status: FieldClaimStatus::Fresh,
            committed_at: life.stale_until(),
        };
        for (
            action,
            mode,
            status,
            expiry,
            disposition,
            action_json,
            mode_json,
            status_json,
            disposition_json,
        ) in [
            (
                SearchRecordAction::Create,
                SearchCandidateEvidenceMode::Cached,
                FieldClaimStatus::Fresh,
                None,
                SearchRecordActionDisposition::Created,
                serde_json::json!({"kind":"create"}),
                "cached",
                "fresh",
                "created",
            ),
            (
                SearchRecordAction::Create,
                SearchCandidateEvidenceMode::Refetch,
                FieldClaimStatus::Stale,
                Some(life.fresh_until()),
                SearchRecordActionDisposition::Reused,
                serde_json::json!({"kind":"create"}),
                "refetch",
                "stale",
                "reused",
            ),
            (
                SearchRecordAction::Attach(receipt.record_id),
                SearchCandidateEvidenceMode::Cached,
                FieldClaimStatus::Stale,
                None,
                SearchRecordActionDisposition::Attached,
                serde_json::json!({"kind":"attach", "record_id":receipt.record_id}),
                "cached",
                "stale",
                "attached",
            ),
            (
                SearchRecordAction::Attach(receipt.record_id),
                SearchCandidateEvidenceMode::Refetch,
                FieldClaimStatus::Fresh,
                Some(life.fresh_until()),
                SearchRecordActionDisposition::AlreadyAttached,
                serde_json::json!({"kind":"attach", "record_id":receipt.record_id}),
                "refetch",
                "fresh",
                "already_attached",
            ),
        ] {
            receipt.action = action;
            receipt.evidence_mode = mode;
            receipt.initial_status = status;
            receipt.expires_at = expiry;
            receipt.disposition = disposition;
            let response = SearchCandidateActionResponse::try_from(
                ProviderSearchActionOutcome::Saved(Box::new(receipt.clone())),
            )
            .unwrap();
            assert_eq!(
                serde_json::to_value(response).unwrap(),
                serde_json::json!({
                    "outcome":"saved", "receipt": {
                        "operation_id":receipt.operation_id, "candidate_receipt_id":receipt.candidate_receipt_id,
                        "provider_id":"tmdb", "grain":"film", "action":action_json,
                        "evidence_mode":mode_json, "record_id":receipt.record_id, "disposition":disposition_json,
                        "fetched_at":life.created_at().to_rfc3339(), "expires_at":expiry.map(|at| at.to_rfc3339()),
                        "initial_status":status_json, "committed_at":life.stale_until().to_rfc3339(),
                    }
                })
            );
        }
        for status in [
            FieldClaimStatus::Invalid,
            FieldClaimStatus::Revoked,
            FieldClaimStatus::Superseded,
            FieldClaimStatus::Unavailable,
        ] {
            receipt.initial_status = status;
            assert_eq!(
                SearchCandidateActionResponse::try_from(ProviderSearchActionOutcome::Saved(
                    Box::new(receipt.clone())
                ))
                .unwrap_err(),
                "Search action receipt has invalid historical status"
            );
        }
        assert_eq!(
            serde_json::to_value(
                SearchCandidateActionResponse::try_from(ProviderSearchActionOutcome::Unavailable {
                    problem: ProblemCode::ProviderResponseInvalid
                })
                .unwrap()
            )
            .unwrap(),
            serde_json::json!({"outcome":"unavailable", "problem_code":"provider_response_invalid"})
        );
    }

    #[test]
    fn create_action_rejects_attach_target_and_caller_metadata() {
        assert!(serde_json::from_value::<SearchRecordActionDto>(
            serde_json::json!({"kind":"create"})
        )
        .is_ok());
        for extra in ["record_id", "title", "provider_id", "provenance"] {
            let mut value = serde_json::json!({"kind":"create"});
            value[extra] = "not-authorized-input".into();
            assert!(
                serde_json::from_value::<SearchRecordActionDto>(value).is_err(),
                "{extra}"
            );
        }
    }

    #[test]
    fn missing_candidate_cannot_carry_hidden_evidence() {
        assert!(serde_json::from_value::<SearchCandidateDetailsResponse>(
            serde_json::json!({"outcome":"missing"})
        )
        .is_ok());
        assert!(serde_json::from_value::<SearchCandidateDetailsResponse>(
            serde_json::json!({"outcome":"missing", "snapshot":{}})
        )
        .is_err());
    }

    #[test]
    fn search_request_rejects_authority_fields_and_requires_explicit_mode() {
        let mut request = serde_json::json!({"query":"Dune", "page":1,
            "locale":null, "region":null, "grains":[], "offline":true});
        assert!(serde_json::from_value::<SearchProviderPageRequest>(request.clone()).is_ok());
        request["terms_revision"] = "caller-controlled".into();
        assert!(serde_json::from_value::<SearchProviderPageRequest>(request.clone()).is_err());
        request.as_object_mut().unwrap().remove("terms_revision");
        request.as_object_mut().unwrap().remove("offline");
        assert!(serde_json::from_value::<SearchProviderPageRequest>(request).is_err());
    }
}
