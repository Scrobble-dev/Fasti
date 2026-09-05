use fasti_application::{NuvioCollectionsDocument, NuvioCollectionsError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(transparent)]
pub struct NuvioCollectionsDocumentDto(
    #[schemars(length(min = 1, max = 64))] pub Vec<NuvioCollectionDto>,
);

impl NuvioCollectionsDocumentDto {
    pub fn into_application(self) -> Result<NuvioCollectionsDocument, NuvioCollectionsError> {
        let json = serde_json::to_string(&self)
            .expect("Nuvio Collections DTO contains only serializable JSON values");
        NuvioCollectionsDocument::try_from_json(&json)
    }

    pub fn from_application(document: &NuvioCollectionsDocument) -> Self {
        serde_json::from_str(document.canonical_json())
            .expect("application Nuvio Collections document is canonical JSON")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NuvioCollectionDto {
    #[schemars(length(min = 1, max = 8192))]
    #[schema(min_length = 1, max_length = 8192)]
    pub id: String,
    #[schemars(length(max = 8192))]
    #[schema(max_length = 8192)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_to_top: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_glow_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_all_tab: Option<bool>,
    #[schemars(length(max = 256))]
    #[schema(max_items = 256)]
    pub folders: Vec<NuvioCollectionFolderDto>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NuvioCollectionFolderDto {
    #[schemars(length(min = 1, max = 8192))]
    #[schema(min_length = 1, max_length = 8192)]
    pub id: String,
    #[schemars(length(max = 8192))]
    #[schema(max_length = 8192)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_gif_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focus_gif_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_emoji: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_shape: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_title: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_backdrop_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hero_video_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_logo_url: Option<String>,
    #[schemars(length(max = 128))]
    #[schema(max_items = 128)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<NuvioCollectionSourceDto>>,
    #[schemars(length(max = 128))]
    #[schema(max_items = 128)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_sources: Option<Vec<NuvioCatalogSourceDto>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NuvioCatalogSourceDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon_id: Option<Value>,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NuvioCollectionSourceDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon_id: Option<Value>,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genre: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmdb_source_type: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmdb_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trakt_list_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_how: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NuvioCollectionsStateDto {
    pub document: Option<NuvioCollectionsDocumentDto>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_bare_array_round_trips_through_the_application_contract() {
        let json = r#"[{"id":"collection","title":"Collection","folders":[{"id":"folder","title":"Folder","sources":[{"provider":"tmdb","tmdbSourceType":"discover","filters":{"voteCountGte":10,"vote_count.gte":10},"id":"source"}]}]}]"#;
        let dto: NuvioCollectionsDocumentDto = serde_json::from_str(json).expect("wire DTO");
        let application = dto.into_application().expect("application document");
        let projected = NuvioCollectionsDocumentDto::from_application(&application);
        let value = serde_json::to_value(projected).expect("projected JSON");
        assert_eq!(value[0]["folders"][0]["sources"][0]["id"], "source");
        assert_eq!(
            value[0]["folders"][0]["sources"][0]["filters"]["vote_count.gte"],
            10
        );
    }
}
