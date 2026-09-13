//! Public Kitsu JSON:API responses. Resource type is part of identity.

use super::*;

#[derive(Deserialize)]
struct MediaResource {
    id: Option<String>,
    #[serde(rename = "type")]
    resource_type: Option<String>,
    attributes: Option<Attributes>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attributes {
    canonical_title: Option<String>,
    description: Option<String>,
    synopsis: Option<String>,
    start_date: Option<String>,
    subtype: Option<serde_json::Value>,
    poster_image: Option<PosterImage>,
}

#[derive(Deserialize)]
struct PosterImage {
    small: Option<String>,
}

#[derive(Deserialize)]
struct Page {
    data: Vec<serde_json::Value>,
    links: Links,
}

#[derive(Deserialize)]
struct Links {
    next: Option<String>,
}

fn invalid() -> ProviderRuntimeError {
    ProviderRuntimeError::response_invalid("Kitsu returned an invalid typed media response.")
}

// One response per continuation keeps each receipt bound to its original body.
// Modes 1/2 alternate Anime/Manga; 3/4 drain the remaining source alone.
pub(super) struct Continuation(u32);

impl Continuation {
    pub(super) fn decode(token: u32) -> Result<Self, ProviderRuntimeError> {
        let value = Self(token);
        token.checked_sub(1).ok_or_else(invalid)?;
        (value.source_page() - 1)
            .checked_mul(RESULT_LIMIT as u32)
            .ok_or_else(invalid)?;
        Ok(value)
    }

    pub(super) fn kind(&self) -> &'static str {
        if self.0 % 2 == 1 {
            "anime"
        } else {
            "manga"
        }
    }

    pub(super) fn source_page(&self) -> u32 {
        (self.0 - 1) / 4 + 1
    }

    pub(super) fn advance(&self, has_next: bool) -> Result<Option<u32>, ProviderRuntimeError> {
        if has_next {
            self.source_page()
                .checked_mul(RESULT_LIMIT as u32)
                .ok_or_else(invalid)?;
        }
        let delta = match ((self.0 - 1) % 4, has_next) {
            (0, true) => 1,
            (0, false) | (1, true) => 3,
            (1, false) => 5,
            (_, true) => 4,
            (_, false) => return Ok(None),
        };
        let next = self.0.checked_add(delta).ok_or_else(invalid)?;
        Self::decode(next)?;
        Ok(Some(next))
    }
}

pub(super) fn source_search_url(
    query: &SearchQuery,
    page: u32,
    kind: &str,
) -> Result<reqwest::Url, ProviderRuntimeError> {
    let mut url = media_url(kind)?;
    let offset = page
        .checked_sub(1)
        .and_then(|value| value.checked_mul(RESULT_LIMIT as u32))
        .ok_or_else(invalid)?;
    url.query_pairs_mut()
        .append_pair("filter[text]", query.as_str())
        .append_pair("page[limit]", &RESULT_LIMIT.to_string())
        .append_pair("page[offset]", &offset.to_string());
    Ok(url)
}

pub(super) fn media_url(kind: &str) -> Result<reqwest::Url, ProviderRuntimeError> {
    provider_identity_mapping(KITSU_PROVIDER, kind).ok_or_else(invalid)?;
    let mut url = reqwest::Url::parse(KITSU_URL).map_err(|_| invalid())?;
    url.set_path(&format!("/api/edge/{kind}"));
    Ok(url)
}

pub(super) fn validate_health_response(
    body: &[u8],
    kind: &'static str,
) -> Result<(), ProviderRuntimeError> {
    let page: Page = serde_json::from_slice(body).map_err(|_| invalid())?;
    if page.data.len() > 1
        || page
            .data
            .into_iter()
            .any(|item| candidate(item, provider_evidence_digest(body), kind).is_none())
    {
        return Err(invalid());
    }
    Ok(())
}

fn candidate(
    value: serde_json::Value,
    digest: Sha256Digest,
    kind: &'static str,
) -> Option<ProviderCandidate> {
    let item: MediaResource = serde_json::from_value(value).ok()?;
    if item.resource_type.as_deref() != Some(kind) {
        return None;
    }
    let id = item.id?;
    provider_identity_mapping(KITSU_PROVIDER, kind)?
        .identifier(&id)
        .ok()?;
    let attributes = item.attributes?;
    let title = attributes.canonical_title?;
    if !valid_candidate_text(&title, 512) {
        return None;
    }
    Some(ProviderCandidate {
        provider: KITSU_PROVIDER,
        provider_id: id,
        title,
        original_title: None,
        kind,
        release_year: attributes.start_date.as_deref().and_then(release_year),
        authors: Vec::new(),
        image_url: attributes
            .poster_image
            .and_then(|image| image.small)
            .filter(|url| valid_search_candidate_image(KITSU_PROVIDER, url)),
        overview: attributes
            .description
            .filter(|text| valid_candidate_text(text, 4096))
            .or_else(|| {
                attributes
                    .synopsis
                    .filter(|text| valid_candidate_text(text, 4096))
            }),
        google_books_print_type: None,
        kitsu_manga_subtype: (kind == "manga").then(|| {
            KitsuMangaSubtype::from_source(
                attributes
                    .subtype
                    .as_ref()
                    .and_then(serde_json::Value::as_str),
            )
        }),
        evidence_digest: digest,
        response_cache_policy: None,
    })
}

pub(super) fn parse_kitsu_candidates(
    body: &[u8],
    page: u32,
    query: &SearchQuery,
    kind: &'static str,
) -> Result<ProviderSearchPage, ProviderRuntimeError> {
    let parsed: Page = serde_json::from_slice(body).map_err(|_| invalid())?;
    if page == 0 || parsed.data.len() > RESULT_LIMIT {
        return Err(invalid());
    }
    let mut next_page = parsed
        .links
        .next
        .map(|link| {
            let next = page.checked_add(1).ok_or_else(invalid)?;
            let expected = source_search_url(query, next, kind)?;
            let actual = reqwest::Url::parse(&link).map_err(|_| invalid())?;
            let pairs = |url: &reqwest::Url| {
                let mut pairs: Vec<_> = url.query_pairs().into_owned().collect();
                pairs.sort();
                pairs
            };
            if actual.origin() != expected.origin()
                || actual.path() != expected.path()
                || !actual.username().is_empty()
                || actual.password().is_some()
                || actual.fragment().is_some()
                || pairs(&actual) != pairs(&expected)
            {
                return Err(invalid());
            }
            Ok(next)
        })
        .transpose()?;
    // Kitsu's deployed offset conversion can repeat the first page and omit
    // the final link early. A full body needs a following-page observation;
    // never skip offsets or guess which backend is deployed. Correct backends
    // may return one extra empty page when their total is an exact multiple.
    if next_page.is_none() && parsed.data.len() == RESULT_LIMIT {
        let next = page.checked_add(1).ok_or_else(invalid)?;
        source_search_url(query, next, kind)?;
        next_page = Some(next);
    }
    let evidence_digest = provider_evidence_digest(body);
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for item in parsed.data {
        if let Some(candidate) = candidate(item, evidence_digest.clone(), kind) {
            if !seen.insert(candidate.provider_id.clone()) {
                return Err(invalid());
            }
            candidates.push(candidate);
        }
    }
    Ok(ProviderSearchPage {
        candidates,
        next_page,
        evidence_digest,
        response_cache_policy: None,
    })
}

pub(super) fn parse_kitsu_selection(
    body: &[u8],
    provider_id: &str,
    kind: &'static str,
) -> Result<ProviderCandidate, ProviderRuntimeError> {
    #[derive(Deserialize)]
    struct Selection {
        data: serde_json::Value,
    }
    let selection: Selection = serde_json::from_slice(body).map_err(|_| invalid())?;
    verify_selected_candidate(
        candidate(selection.data, provider_evidence_digest(body), kind).ok_or_else(invalid)?,
        provider_id,
        kind,
    )
}
