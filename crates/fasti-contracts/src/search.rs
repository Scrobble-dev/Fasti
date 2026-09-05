//! Provider Search representations. Receipt authority remains server-side.

use fasti_application::{
    SearchCacheState, SearchCandidate, SearchCandidateReceipt, SearchReceiptLifetime,
    StoredSearchCandidate,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchCacheStateDto {
    Fresh,
    StaleOnError,
}

impl From<SearchCacheState> for SearchCacheStateDto {
    fn from(value: SearchCacheState) -> Self {
        match value {
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
        snapshot: SearchCandidateSnapshotDto,
        details: SearchCandidateDto,
        locale: Option<String>,
    },
    Unavailable {
        snapshot: SearchCandidateSnapshotDto,
        problem_code: String,
    },
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

#[cfg(test)]
mod tests {
    use super::*;

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
