use crate::{
    artwork::ArtworkCache,
    records,
    setup::{DesktopProblem, SetupSecretStore},
};
use fasti_application::{CapabilityKey, FastiProblem, LocalSearchRequest, SearchPersistencePort};
use fasti_contracts::{LocalSearchCursorDto, LocalSearchRequestDto, RecordSummaryDto};
use fasti_domain::{Grain, RequestCorrelationId, SearchQuery};
use fasti_store::SqliteKernel;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct LocalRecordSummary {
    #[serde(flatten)]
    record: RecordSummaryDto,
    poster_asset_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LocalSearchResponse {
    records: Vec<LocalRecordSummary>,
    next: Option<LocalSearchCursorDto>,
}

pub(crate) fn local_records(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    artwork: &ArtworkCache,
    input: LocalSearchRequestDto,
) -> Result<LocalSearchResponse, DesktopProblem> {
    let access = records::require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();
    if input.grains.len() > Grain::ALL.len() {
        return Err(DesktopProblem::invalid_input("Too many Search grains."));
    }
    let request = LocalSearchRequest {
        correlation_id,
        access: access.into(),
        query: SearchQuery::try_new(input.query)
            .map_err(|_| DesktopProblem::invalid_input("The Search query is invalid."))?,
        grains: input
            .grains
            .into_iter()
            .map(|grain| grain.parse::<Grain>())
            .collect::<Result<_, _>>()
            .map_err(|_| DesktopProblem::invalid_input("A Search grain is invalid."))?,
        after: input
            .after
            .map(|cursor| {
                Ok::<_, DesktopProblem>(fasti_application::LocalSearchCursor {
                    last_record_id: cursor.last_record_id.parse().map_err(|_| {
                        DesktopProblem::invalid_input("The Search cursor Record is invalid.")
                    })?,
                    context_digest: cursor.context_digest.parse().map_err(|_| {
                        DesktopProblem::invalid_input("The Search cursor digest is invalid.")
                    })?,
                })
            })
            .transpose()?,
    };
    let page = kernel
        .search_local_records(&request)
        .map_err(|problem| DesktopProblem::application(&problem))?;
    let response = LocalSearchResponse {
        records: page
            .records
            .into_iter()
            .map(|summary| {
                let poster_asset_path = summary
                    .poster()
                    .provenance()
                    .and_then(|provenance| provenance.claim_provenance().provider_id())
                    .zip(summary.poster().value())
                    .and_then(|(provider, url)| {
                        artwork.cached_locator(
                            provider.as_str(),
                            url,
                            access,
                            summary.record_id(),
                        )
                    });
                LocalRecordSummary {
                    record: summary.into(),
                    poster_asset_path,
                }
            })
            .collect(),
        next: page.next.map(Into::into),
    };

    let mut buffer = vec![0; fasti_application::MAX_LOCAL_SEARCH_RESPONSE_BYTES];
    serde_json::to_writer(buffer.as_mut_slice(), &response).map_err(|_| {
        DesktopProblem::application(&FastiProblem::capacity_exceeded(
            CapabilityKey::SearchMetadata,
            correlation_id,
        ))
    })?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::{complete_setup, test_support::MemoryStore};
    use fasti_application::{
        ConfigureMetadataProjectionCommand, CreateRecordCommand, IdentityPort,
        MetadataOverrideMutation, MetadataProjectionPort,
    };
    use fasti_domain::{
        FieldKey, MetadataProjectionPolicy, ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY,
        RELEASE_YEAR_FIELD_KEY, TITLE_FIELD_KEY,
    };

    fn input(query: &str) -> LocalSearchRequestDto {
        LocalSearchRequestDto {
            query: query.to_owned(),
            grains: Vec::new(),
            after: None,
        }
    }

    #[test]
    fn native_local_search_authenticates_before_input_and_never_needs_provider_state() {
        let root = tempfile::tempdir().unwrap();
        let kernel = SqliteKernel::open(root.path()).unwrap();
        let store = MemoryStore::default();
        let artwork = ArtworkCache::new(root.path().join("artwork"), kernel.data_root_identity());

        assert!(matches!(
            local_records(&kernel, &store, &artwork, input(" bad")),
            Err(problem) if problem.code() == "not_authenticated"
        ));
        complete_setup(&kernel, &store).unwrap();
        let page = local_records(&kernel, &store, &artwork, input("nothing")).unwrap();
        assert!(page.records.is_empty());
        assert!(page.next.is_none());
        assert!(matches!(
            local_records(&kernel, &store, &artwork, input(" bad")),
            Err(problem) if problem.code() == "invalid_input"
        ));

        let access = records::require_access(&kernel, &store).unwrap();
        let record = kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                Grain::Film,
            ))
            .unwrap()
            .record_id();
        let overrides = [
            (TITLE_FIELD_KEY, "Native Search title"),
            (ORIGINAL_TITLE_FIELD_KEY, "Original title"),
            (OVERVIEW_FIELD_KEY, "Complete local summary"),
            (RELEASE_YEAR_FIELD_KEY, "2026"),
        ]
        .into_iter()
        .map(|(field, value)| MetadataOverrideMutation::Set {
            record_id: record,
            field_key: FieldKey::try_new(field).unwrap(),
            value: value.to_owned(),
        })
        .collect();
        kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                access,
                MetadataProjectionPolicy::default_for_profile(access.profile_id()),
                None,
                Vec::new(),
                overrides,
            ))
            .unwrap();

        let page = serde_json::to_value(
            local_records(&kernel, &store, &artwork, input("Search title")).unwrap(),
        )
        .unwrap();
        assert_eq!(page["records"][0]["record_id"], record.to_string());
        assert_eq!(page["records"][0]["title"]["value"], "Native Search title");
        assert_eq!(page["records"][0]["original_title"]["value"], "Original title");
        assert_eq!(page["records"][0]["overview"]["value"], "Complete local summary");
        assert_eq!(page["records"][0]["release_year"]["value"], "2026");
        assert_eq!(page["records"][0]["poster_asset_path"], serde_json::Value::Null);
    }
}
