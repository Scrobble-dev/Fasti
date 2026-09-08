//! Public Kitsu Manga JSON:API responses. Resource type is part of identity.

use super::*;

#[derive(Deserialize)]
struct Manga {
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
    ProviderRuntimeError::response_invalid("Kitsu returned an invalid Manga response.")
}

pub(super) fn validate_health_response(body: &[u8]) -> Result<(), ProviderRuntimeError> {
    let page: Page = serde_json::from_slice(body).map_err(|_| invalid())?;
    if page.data.len() > 1
        || page
            .data
            .into_iter()
            .any(|item| candidate(item, provider_evidence_digest(body)).is_none())
    {
        return Err(invalid());
    }
    Ok(())
}

fn candidate(value: serde_json::Value, digest: Sha256Digest) -> Option<ProviderCandidate> {
    let item: Manga = serde_json::from_value(value).ok()?;
    if item.resource_type.as_deref() != Some("manga") {
        return None;
    }
    let id = item.id?;
    provider_identity_mapping(KITSU_PROVIDER, "manga")?
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
        kind: "manga",
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
        kitsu_manga_subtype: Some(KitsuMangaSubtype::from_source(
            attributes
                .subtype
                .as_ref()
                .and_then(serde_json::Value::as_str),
        )),
        evidence_digest: digest,
        response_cache_policy: None,
    })
}

pub(super) fn parse_kitsu_candidates(
    body: &[u8],
    page: u32,
    query: &SearchQuery,
) -> Result<ProviderSearchPage, ProviderRuntimeError> {
    let parsed: Page = serde_json::from_slice(body).map_err(|_| invalid())?;
    if page == 0 || parsed.data.len() > RESULT_LIMIT {
        return Err(invalid());
    }
    let next_page = parsed
        .links
        .next
        .map(|link| {
            let next = page.checked_add(1).ok_or_else(invalid)?;
            let expected = search_url(KITSU_PROVIDER, query, next, None)?;
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
    let evidence_digest = provider_evidence_digest(body);
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    for item in parsed.data {
        if let Some(candidate) = candidate(item, evidence_digest.clone()) {
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
) -> Result<ProviderCandidate, ProviderRuntimeError> {
    #[derive(Deserialize)]
    struct Selection {
        data: serde_json::Value,
    }
    let selection: Selection = serde_json::from_slice(body).map_err(|_| invalid())?;
    verify_selected_candidate(
        candidate(selection.data, provider_evidence_digest(body)).ok_or_else(invalid)?,
        provider_id,
        "manga",
    )
}
