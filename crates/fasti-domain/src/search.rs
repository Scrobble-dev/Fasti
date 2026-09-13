//! Search input shared by local queries and provider adapters.

pub const MAX_SEARCH_QUERY_BYTES: usize = 256;

/// Validated text. Adapters must bind it as data, never as SQL or query syntax.
#[derive(Clone, PartialEq, Eq)]
pub struct SearchQuery(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("search query must contain 1 to 256 UTF-8 bytes without leading, trailing, or control characters")]
pub struct SearchQueryError;

impl SearchQuery {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SearchQueryError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_SEARCH_QUERY_BYTES
            || value.trim() != value
            || value.chars().any(char::is_control)
        {
            return Err(SearchQueryError);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SearchQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SearchQuery([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_bounds_measure_utf8_bytes_and_preserve_provider_syntax() {
        for value in [
            "海".repeat(85),
            "é".repeat(128),
            "isbn:9780140328721".to_owned(),
            "title:\"L'été\" OR 海".to_owned(),
        ] {
            let query = SearchQuery::try_new(value.clone()).expect("valid query");
            assert_eq!(query.as_str(), value);
            assert_eq!(format!("{query:?}"), "SearchQuery([redacted])");
        }
        for value in [
            "".to_owned(),
            " leading".to_owned(),
            "trailing\u{a0}".to_owned(),
            "line\nfeed".to_owned(),
            "null\0byte".to_owned(),
            "delete\u{7f}".to_owned(),
            "海".repeat(86),
            "é".repeat(129),
        ] {
            assert_eq!(SearchQuery::try_new(value), Err(SearchQueryError));
        }
    }
}
