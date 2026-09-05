//! Provider Search representations. Receipt authority remains server-side.

use fasti_application::{SearchCacheState, SearchCandidateReceipt, SearchReceiptLifetime};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
        let data = value.candidate().data();
        Self {
            candidate_receipt_id: value.id().to_string(),
            grain: value.candidate().identifier().grain().as_str().to_owned(),
            candidate: SearchCandidateDto {
                provider: data.provider.clone(),
                provider_id: data.provider_id.clone(),
                kind: data.kind.clone(),
                title: data.title.clone(),
                original_title: data.original_title.clone(),
                release_year: data.release_year,
                authors: data.authors.clone(),
                image_url: data.image_url.clone(),
                overview: data.overview.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
