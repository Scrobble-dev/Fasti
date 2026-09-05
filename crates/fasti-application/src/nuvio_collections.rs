//! Bounded import and canonical storage shape for Nuvio custom Collections.
//!
//! Nuvio Collections describe external catalog browse configuration. They are
//! not Fasti media lists and their opaque provider IDs are never Fasti IDs.

use crate::{ApplicationAccessContext, ApplicationResult};
use fasti_domain::RequestCorrelationId;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;

pub const MAX_NUVIO_COLLECTIONS_JSON_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_NUVIO_COLLECTIONS: usize = 64;
pub const MAX_NUVIO_FOLDERS: usize = 1_024;
pub const MAX_NUVIO_FOLDERS_PER_COLLECTION: usize = 256;
pub const MAX_NUVIO_SOURCES: usize = 4_096;
pub const MAX_NUVIO_SOURCES_PER_FOLDER: usize = 128;
const MAX_NUVIO_JSON_NODES: usize = 200_000;
const MAX_NUVIO_JSON_DEPTH: usize = 16;
const MAX_NUVIO_STRING_BYTES: usize = 8 * 1024;
const MAX_NUVIO_KEY_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetNuvioCollectionsQuery {
    correlation_id: RequestCorrelationId,
    access: ApplicationAccessContext,
}

impl GetNuvioCollectionsQuery {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: impl Into<ApplicationAccessContext>,
    ) -> Self {
        Self {
            correlation_id,
            access: access.into(),
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &ApplicationAccessContext {
        &self.access
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceNuvioCollectionsCommand {
    correlation_id: RequestCorrelationId,
    access: ApplicationAccessContext,
    document: NuvioCollectionsDocument,
}

impl ReplaceNuvioCollectionsCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: impl Into<ApplicationAccessContext>,
        document: NuvioCollectionsDocument,
    ) -> Self {
        Self {
            correlation_id,
            access: access.into(),
            document,
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &ApplicationAccessContext {
        &self.access
    }

    pub const fn document(&self) -> &NuvioCollectionsDocument {
        &self.document
    }

    pub fn into_document(self) -> NuvioCollectionsDocument {
        self.document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearNuvioCollectionsCommand {
    correlation_id: RequestCorrelationId,
    access: ApplicationAccessContext,
}

impl ClearNuvioCollectionsCommand {
    pub fn new(
        correlation_id: RequestCorrelationId,
        access: impl Into<ApplicationAccessContext>,
    ) -> Self {
        Self {
            correlation_id,
            access: access.into(),
        }
    }

    pub const fn correlation_id(&self) -> RequestCorrelationId {
        self.correlation_id
    }

    pub const fn access(&self) -> &ApplicationAccessContext {
        &self.access
    }
}

pub trait NuvioCollectionsPort: Send + Sync {
    fn get_nuvio_collections(
        &self,
        query: GetNuvioCollectionsQuery,
    ) -> ApplicationResult<Option<NuvioCollectionsDocument>>;

    fn replace_nuvio_collections(
        &self,
        command: ReplaceNuvioCollectionsCommand,
    ) -> ApplicationResult<NuvioCollectionsDocument>;

    fn clear_nuvio_collections(
        &self,
        command: ClearNuvioCollectionsCommand,
    ) -> ApplicationResult<()>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NuvioCollectionsSummary {
    collection_count: usize,
    folder_count: usize,
    source_count: usize,
    dropped_source_count: usize,
    deduplicated_collection_count: usize,
}

impl NuvioCollectionsSummary {
    pub const fn collection_count(self) -> usize {
        self.collection_count
    }

    pub const fn folder_count(self) -> usize {
        self.folder_count
    }

    pub const fn source_count(self) -> usize {
        self.source_count
    }

    pub const fn dropped_source_count(self) -> usize {
        self.dropped_source_count
    }

    pub const fn deduplicated_collection_count(self) -> usize {
        self.deduplicated_collection_count
    }
}

#[derive(Debug, Clone)]
pub struct NuvioCollectionsDocument {
    canonical_json: String,
    summary: NuvioCollectionsSummary,
}

impl NuvioCollectionsDocument {
    pub fn try_from_json(json: &str) -> Result<Self, NuvioCollectionsError> {
        if json.is_empty() {
            return Err(NuvioCollectionsError::new("/", "document is empty"));
        }
        if json.len() > MAX_NUVIO_COLLECTIONS_JSON_BYTES {
            return Err(NuvioCollectionsError::new(
                "/",
                "document exceeds the 4 MiB import limit",
            ));
        }

        let mut value: Value = serde_json::from_str(json)
            .map_err(|_| NuvioCollectionsError::new("/", "document is not valid JSON"))?;
        let mut node_count = 0;
        validate_json_tree(&value, 0, &mut node_count)?;
        let collections = value
            .as_array_mut()
            .ok_or_else(|| NuvioCollectionsError::new("/", "document must be a top-level array"))?;
        if collections.is_empty() {
            return Err(NuvioCollectionsError::new(
                "/",
                "document must contain at least one collection",
            ));
        }
        if collections.len() > MAX_NUVIO_COLLECTIONS {
            return Err(NuvioCollectionsError::new(
                "/",
                "document contains too many collections",
            ));
        }

        let mut summary = NuvioCollectionsSummary::default();
        let mut normalized = Vec::with_capacity(collections.len());
        let mut collection_positions = HashMap::with_capacity(collections.len());
        for (index, collection) in std::mem::take(collections).into_iter().enumerate() {
            let (id, collection) = normalize_collection(collection, index, &mut summary)?;
            if let Some(position) = collection_positions.get(&id).copied() {
                normalized[position] = collection;
                summary.deduplicated_collection_count += 1;
            } else {
                collection_positions.insert(id, normalized.len());
                normalized.push(collection);
            }
        }
        summary.collection_count = normalized.len();
        (summary.folder_count, summary.source_count) = count_normalized(&normalized);

        let canonical_json = serde_json::to_string(&normalized)
            .map_err(|_| NuvioCollectionsError::new("/", "document could not be normalized"))?;
        if canonical_json.len() > MAX_NUVIO_COLLECTIONS_JSON_BYTES {
            return Err(NuvioCollectionsError::new(
                "/",
                "normalized document exceeds the 4 MiB import limit",
            ));
        }
        Ok(Self {
            canonical_json,
            summary,
        })
    }

    pub fn try_from_canonical_json(json: &str) -> Result<Self, NuvioCollectionsError> {
        let document = Self::try_from_json(json)?;
        if document.canonical_json != json {
            return Err(NuvioCollectionsError::new(
                "/",
                "stored document is not canonical",
            ));
        }
        Ok(document)
    }

    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }

    pub const fn summary(&self) -> NuvioCollectionsSummary {
        self.summary
    }
}

impl PartialEq for NuvioCollectionsDocument {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_json == other.canonical_json
    }
}

impl Eq for NuvioCollectionsDocument {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuvioCollectionsError {
    pointer: String,
    reason: &'static str,
}

impl NuvioCollectionsError {
    fn new(pointer: impl Into<String>, reason: &'static str) -> Self {
        Self {
            pointer: pointer.into(),
            reason,
        }
    }

    pub fn pointer(&self) -> &str {
        &self.pointer
    }

    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for NuvioCollectionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.pointer, self.reason)
    }
}

impl std::error::Error for NuvioCollectionsError {}

fn validate_json_tree(
    value: &Value,
    depth: usize,
    node_count: &mut usize,
) -> Result<(), NuvioCollectionsError> {
    if depth > MAX_NUVIO_JSON_DEPTH {
        return Err(NuvioCollectionsError::new(
            "/",
            "document nesting is too deep",
        ));
    }
    *node_count = node_count.saturating_add(1);
    if *node_count > MAX_NUVIO_JSON_NODES {
        return Err(NuvioCollectionsError::new(
            "/",
            "document contains too many JSON values",
        ));
    }
    match value {
        Value::Array(values) => {
            for value in values {
                validate_json_tree(value, depth + 1, node_count)?;
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if key.len() > MAX_NUVIO_KEY_BYTES {
                    return Err(NuvioCollectionsError::new(
                        "/",
                        "document contains an oversized object key",
                    ));
                }
                validate_json_tree(value, depth + 1, node_count)?;
            }
        }
        Value::String(value) if value.len() > MAX_NUVIO_STRING_BYTES => {
            return Err(NuvioCollectionsError::new(
                "/",
                "document contains an oversized string",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn count_normalized(collections: &[Value]) -> (usize, usize) {
    let mut folders = 0;
    let mut sources = 0;
    for collection in collections {
        let Some(collection_folders) = collection.get("folders").and_then(Value::as_array) else {
            continue;
        };
        folders += collection_folders.len();
        for folder in collection_folders {
            sources += folder
                .get("sources")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
        }
    }
    (folders, sources)
}

fn normalize_collection(
    value: Value,
    index: usize,
    summary: &mut NuvioCollectionsSummary,
) -> Result<(String, Value), NuvioCollectionsError> {
    let pointer = format!("/{index}");
    let mut collection = object(value, &pointer, "collection must be an object")?;
    let id = required_nonempty_string(&collection, "id", &format!("{pointer}/id"))?;
    required_string(&collection, "title", &format!("{pointer}/title"))?;
    optional_string(&collection, "backdropImageUrl", &pointer)?;
    normalize_bool(&mut collection, "pinToTop", false, &pointer)?;
    normalize_bool(&mut collection, "focusGlowEnabled", true, &pointer)?;
    normalize_bool(&mut collection, "showAllTab", true, &pointer)?;
    normalize_enum(
        &mut collection,
        "viewMode",
        "TABBED_GRID",
        &["TABBED_GRID", "ROWS", "FOLLOW_LAYOUT"],
        &pointer,
    )?;

    let folders = collection
        .get_mut("folders")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            NuvioCollectionsError::new(format!("{pointer}/folders"), "folders must be an array")
        })?;
    if folders.len() > MAX_NUVIO_FOLDERS_PER_COLLECTION
        || summary.folder_count.saturating_add(folders.len()) > MAX_NUVIO_FOLDERS
    {
        return Err(NuvioCollectionsError::new(
            format!("{pointer}/folders"),
            "document contains too many folders",
        ));
    }
    for (folder_index, folder) in folders.iter_mut().enumerate() {
        normalize_folder(folder, index, folder_index, summary)?;
    }
    summary.folder_count += folders.len();
    Ok((id, Value::Object(collection)))
}

fn normalize_folder(
    value: &mut Value,
    collection_index: usize,
    folder_index: usize,
    summary: &mut NuvioCollectionsSummary,
) -> Result<(), NuvioCollectionsError> {
    let pointer = format!("/{collection_index}/folders/{folder_index}");
    let folder = value
        .as_object_mut()
        .ok_or_else(|| NuvioCollectionsError::new(&pointer, "folder must be an object"))?;
    required_nonempty_string(folder, "id", &format!("{pointer}/id"))?;
    required_string(folder, "title", &format!("{pointer}/title"))?;
    for key in [
        "coverImageUrl",
        "focusGifUrl",
        "coverEmoji",
        "heroBackdropUrl",
        "heroVideoUrl",
        "titleLogoUrl",
    ] {
        optional_string(folder, key, &pointer)?;
    }
    normalize_bool(folder, "focusGifEnabled", true, &pointer)?;
    normalize_bool(folder, "hideTitle", false, &pointer)?;
    normalize_enum(
        folder,
        "tileShape",
        "POSTER",
        &["POSTER", "LANDSCAPE", "SQUARE"],
        &pointer,
    )?;

    if let Some(legacy) = folder.get("catalogSources") {
        if !legacy.is_null() && !legacy.is_array() {
            return Err(NuvioCollectionsError::new(
                format!("{pointer}/catalogSources"),
                "catalogSources must be an array",
            ));
        }
    }
    if folder.get("sources").is_none_or(Value::is_null) {
        let legacy = folder
            .get("catalogSources")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                NuvioCollectionsError::new(
                    format!("{pointer}/sources"),
                    "sources or catalogSources must be an array",
                )
            })?
            .clone();
        folder.insert("sources".to_owned(), Value::Array(legacy));
    }
    let sources = folder
        .get_mut("sources")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| {
            NuvioCollectionsError::new(format!("{pointer}/sources"), "sources must be an array")
        })?;
    if sources.len() > MAX_NUVIO_SOURCES_PER_FOLDER
        || summary
            .source_count
            .saturating_add(summary.dropped_source_count)
            .saturating_add(sources.len())
            > MAX_NUVIO_SOURCES
    {
        return Err(NuvioCollectionsError::new(
            format!("{pointer}/sources"),
            "document contains too many sources",
        ));
    }

    let before = sources.len();
    let mut normalized = Vec::with_capacity(before);
    for source in std::mem::take(sources) {
        if let Some(source) = normalize_source(source) {
            normalized.push(source);
        }
    }
    summary.source_count += normalized.len();
    summary.dropped_source_count += before - normalized.len();
    *sources = normalized;
    let catalog_sources = sources
        .iter()
        .filter_map(|source| {
            let source = source.as_object()?;
            if source.get("provider")?.as_str()? != "addon" {
                return None;
            }
            let mut legacy = Map::new();
            for key in ["addonId", "type", "catalogId", "genre"] {
                if let Some(value) = source.get(key).filter(|value| !value.is_null()) {
                    legacy.insert(key.to_owned(), value.clone());
                }
            }
            Some(Value::Object(legacy))
        })
        .collect();
    folder.insert("catalogSources".to_owned(), Value::Array(catalog_sources));
    Ok(())
}

fn normalize_source(value: Value) -> Option<Value> {
    let mut source = value.as_object()?.clone();
    let provider = source
        .get("provider")
        .map(Value::as_str)
        .unwrap_or(Some("addon"))?
        .trim()
        .to_ascii_lowercase();

    match provider.as_str() {
        "tmdb" => normalize_tmdb_source(&mut source)?,
        "trakt" => normalize_trakt_source(&mut source)?,
        _ => normalize_addon_source(&source)?,
    }
    source.insert(
        "provider".to_owned(),
        Value::String(
            match provider.as_str() {
                "tmdb" => "tmdb",
                "trakt" => "trakt",
                _ => "addon",
            }
            .to_owned(),
        ),
    );
    Some(Value::Object(source))
}

fn normalize_addon_source(source: &Map<String, Value>) -> Option<()> {
    for key in ["addonId", "type", "catalogId"] {
        if source.get(key)?.as_str()?.trim().is_empty() {
            return None;
        }
    }
    if source
        .get("genre")
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        return None;
    }
    Some(())
}

fn normalize_tmdb_source(source: &mut Map<String, Value>) -> Option<()> {
    let source_type = source
        .get("tmdbSourceType")?
        .as_str()?
        .trim()
        .to_ascii_uppercase();
    if ![
        "LIST",
        "COLLECTION",
        "COMPANY",
        "NETWORK",
        "DISCOVER",
        "PERSON",
        "DIRECTOR",
    ]
    .contains(&source_type.as_str())
    {
        return None;
    }
    source.insert(
        "tmdbSourceType".to_owned(),
        Value::String(source_type.clone()),
    );
    normalize_media_type(source)?;
    if source.get("tmdbId").is_some_and(|value| {
        !value.is_null()
            && value
                .as_i64()
                .is_none_or(|id| id < i32::MIN as i64 || id > i32::MAX as i64)
    }) {
        return None;
    }
    if source
        .get("filters")
        .is_some_and(|value| !value.is_null() && !value.is_object())
    {
        return None;
    }
    if source.get("filters").is_none_or(Value::is_null) {
        source.insert("filters".to_owned(), Value::Object(Map::new()));
    }

    let title = source
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| title_case_ascii(&source_type));
    source.insert("title".to_owned(), Value::String(title));

    let mut sort_by = source
        .get("sortBy")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|sort| !sort.is_empty())
        .unwrap_or("popularity.desc")
        .to_owned();
    if matches!(source_type.as_str(), "LIST" | "COLLECTION") && sort_by == "popularity.desc" {
        sort_by = "original".to_owned();
    }
    source.insert("sortBy".to_owned(), Value::String(sort_by));
    Some(())
}

fn normalize_trakt_source(source: &mut Map<String, Value>) -> Option<()> {
    let list_id = source.get("traktListId")?.as_i64().filter(|id| *id > 0)?;
    normalize_media_type(source)?;
    let title = source
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Trakt List {list_id}"));
    source.insert("title".to_owned(), Value::String(title));

    let sort_by = source
        .get("sortBy")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| {
            [
                "rank",
                "added",
                "title",
                "released",
                "runtime",
                "popularity",
                "percentage",
                "votes",
            ]
            .contains(&value.as_str())
        })
        .unwrap_or_else(|| "rank".to_owned());
    source.insert("sortBy".to_owned(), Value::String(sort_by));
    let sort_how = source
        .get("sortHow")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "asc" | "desc"))
        .unwrap_or_else(|| "asc".to_owned());
    source.insert("sortHow".to_owned(), Value::String(sort_how));
    Some(())
}

fn normalize_media_type(source: &mut Map<String, Value>) -> Option<()> {
    let media_type = match source.get("mediaType") {
        None | Some(Value::Null) => "MOVIE".to_owned(),
        Some(Value::String(value)) => {
            let value = value.trim().to_ascii_uppercase();
            if matches!(value.as_str(), "MOVIE" | "TV") {
                value
            } else {
                "MOVIE".to_owned()
            }
        }
        Some(_) => return None,
    };
    source.insert("mediaType".to_owned(), Value::String(media_type));
    Some(())
}

fn object(
    value: Value,
    pointer: &str,
    reason: &'static str,
) -> Result<Map<String, Value>, NuvioCollectionsError> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| NuvioCollectionsError::new(pointer, reason))
}

fn required_nonempty_string(
    object: &Map<String, Value>,
    key: &str,
    pointer: &str,
) -> Result<String, NuvioCollectionsError> {
    let value = required_string(object, key, pointer)?;
    if value.trim().is_empty() {
        return Err(NuvioCollectionsError::new(
            pointer,
            "value must not be blank",
        ));
    }
    Ok(value.to_owned())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    pointer: &str,
) -> Result<&'a str, NuvioCollectionsError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| NuvioCollectionsError::new(pointer, "value must be a string"))
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    pointer: &str,
) -> Result<(), NuvioCollectionsError> {
    if object
        .get(key)
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        return Err(NuvioCollectionsError::new(
            format!("{pointer}/{key}"),
            "value must be a string or null",
        ));
    }
    Ok(())
}

fn normalize_bool(
    object: &mut Map<String, Value>,
    key: &str,
    default: bool,
    pointer: &str,
) -> Result<(), NuvioCollectionsError> {
    let value = match object.get(key) {
        None | Some(Value::Null) => default,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return Err(NuvioCollectionsError::new(
                format!("{pointer}/{key}"),
                "value must be a boolean",
            ));
        }
    };
    object.insert(key.to_owned(), Value::Bool(value));
    Ok(())
}

fn normalize_enum(
    object: &mut Map<String, Value>,
    key: &str,
    default: &str,
    allowed: &[&str],
    pointer: &str,
) -> Result<(), NuvioCollectionsError> {
    let value = match object.get(key) {
        None | Some(Value::Null) => default.to_owned(),
        Some(Value::String(value)) => {
            let normalized = value.trim().to_ascii_uppercase();
            if allowed.contains(&normalized.as_str()) {
                normalized
            } else {
                default.to_owned()
            }
        }
        Some(_) => {
            return Err(NuvioCollectionsError::new(
                format!("{pointer}/{key}"),
                "value must be a string",
            ));
        }
    };
    object.insert(key.to_owned(), Value::String(value));
    Ok(())
}

fn title_case_ascii(value: &str) -> String {
    let mut chars = value.to_ascii_lowercase().chars().collect::<Vec<_>>();
    if let Some(first) = chars.first_mut() {
        first.make_ascii_uppercase();
    }
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn representative_document() -> String {
        json!([
            {
                "id": "collection-a",
                "title": "First value",
                "folders": [{
                    "id": "folder-a",
                    "title": "Mixed providers",
                    "tileShape": "landscape",
                    "sources": [
                        {"provider":"addon","addonId":"aio-metadata","type":"movie","catalogId":"popular","id":"src-addon"},
                        {"provider":"tmdb","tmdbSourceType":"discover","mediaType":"movie","filters":{"voteCountGte":10,"vote_count.gte":10},"id":"src-tmdb"},
                        {"provider":"trakt","traktListId":42,"mediaType":"tv","sortBy":"votes","sortHow":"desc","id":"src-trakt"},
                        {"provider":"tmdb","tmdbSourceType":"unknown"}
                    ]
                }]
            },
            {
                "id": "collection-a",
                "title": "Last value",
                "folders": [{
                    "id": "folder-legacy",
                    "title": "Legacy",
                    "catalogSources": [{"addonId":"legacy","type":"series","catalogId":"latest"}]
                }]
            },
            {"id":"collection-b","title":"Second position","folders":[]}
        ])
        .to_string()
    }

    #[test]
    fn import_normalizes_the_nuvio_wire_without_losing_filter_keys() {
        let document = NuvioCollectionsDocument::try_from_json(&representative_document())
            .expect("representative document");
        let summary = document.summary();
        assert_eq!(summary.collection_count(), 2);
        assert_eq!(summary.folder_count(), 1);
        assert_eq!(summary.source_count(), 1);
        assert_eq!(summary.dropped_source_count(), 1);
        assert_eq!(summary.deduplicated_collection_count(), 1);

        let value: Value = serde_json::from_str(document.canonical_json()).expect("canonical JSON");
        assert_eq!(value[0]["title"], "Last value");
        assert_eq!(value[0]["folders"][0]["sources"][0]["provider"], "addon");
        assert_eq!(value[0]["folders"][0]["tileShape"], "POSTER");
        assert_eq!(value[1]["id"], "collection-b");

        let first_only = json!([{
            "id":"filters",
            "title":"Filters",
            "folders":[{
                "id":"folder",
                "title":"Folder",
                "sources":[{"provider":"tmdb","tmdbSourceType":"discover","filters":{"voteCountGte":10,"vote_count.gte":10}}]
            }]
        }])
        .to_string();
        let filters =
            NuvioCollectionsDocument::try_from_json(&first_only).expect("filter document");
        let value: Value = serde_json::from_str(filters.canonical_json()).expect("canonical JSON");
        assert_eq!(
            value[0]["folders"][0]["sources"][0]["filters"]["voteCountGte"],
            10
        );
        assert_eq!(
            value[0]["folders"][0]["sources"][0]["filters"]["vote_count.gte"],
            10
        );
    }

    #[test]
    fn invalid_sources_are_dropped_and_empty_documents_are_rejected() {
        let value = json!([{
            "id":"collection",
            "title":"Collection",
            "folders":[{
                "id":"folder",
                "title":"Folder",
                "sources":[
                    {"provider":"addon","addonId":"","type":"movie","catalogId":"x"},
                    {"provider":"trakt","traktListId":0},
                    {"provider":"tmdb","tmdbSourceType":"unknown"}
                ]
            }]
        }])
        .to_string();
        let document = NuvioCollectionsDocument::try_from_json(&value).expect("valid shell");
        assert_eq!(document.summary().source_count(), 0);
        assert_eq!(document.summary().dropped_source_count(), 3);
        assert!(NuvioCollectionsDocument::try_from_json("[]").is_err());
    }

    #[test]
    fn canonical_reparse_is_stable_and_hostile_bounds_fail_closed() {
        let document = NuvioCollectionsDocument::try_from_json(&representative_document())
            .expect("representative document");
        let reparsed = NuvioCollectionsDocument::try_from_canonical_json(document.canonical_json())
            .expect("canonical document");
        assert_eq!(document, reparsed);

        let oversized = format!(
            "[{{\"id\":\"collection\",\"title\":\"{}\",\"folders\":[]}}]",
            "x".repeat(MAX_NUVIO_STRING_BYTES + 1)
        );
        assert!(NuvioCollectionsDocument::try_from_json(&oversized).is_err());
    }
}
