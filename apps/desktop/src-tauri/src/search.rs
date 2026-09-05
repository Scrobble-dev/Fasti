use crate::{
    artwork::ArtworkCache,
    records,
    setup::{DesktopProblem, SetupSecretStore},
};
use fasti_application::{
    CapabilityKey, FastiProblem, LocalSearchRequest, OutboundAccessPolicy, ProviderId,
    ProviderOperationLease, ReadSearchCandidateRequest, RequestAccessContext,
    ProviderIdentifierActionCommand, SearchCandidateActionCommand, SearchPageRequest,
    SearchPersistencePort, SearchProviderQuery, SearchRecordAction,
};
use fasti_contracts::{
    LocalSearchCursorDto, LocalSearchRequestDto, RecordSummaryDto,
    ProviderIdentifierActionRequest, ProviderIdentifierActionResponse, SearchCandidateActionRequest,
    SearchCandidateActionResponse, SearchCandidateDetailsResponse, SearchProviderPageRequest,
    SearchProviderPageResponse, SearchRecordActionDto,
};
use fasti_domain::{Grain, MetadataLocale, MetadataRegion, RequestCorrelationId, SearchQuery};
use fasti_provider_runtime::{ProviderRuntime, ProviderSearchService};
use fasti_store::SqliteKernel;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderPageInput {
    provider_id: String,
    request: SearchProviderPageRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateDetailsInput {
    provider_id: String,
    grain: String,
    candidate_receipt_id: String,
    offline: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateActionInput {
    provider_id: String,
    grain: String,
    candidate_receipt_id: String,
    request: SearchCandidateActionRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderIdentifierActionInput {
    provider_id: String,
    grain: String,
    request: ProviderIdentifierActionRequest,
}

fn provider_query(
    provider_id: String,
    request: SearchProviderPageRequest,
) -> Result<(SearchProviderQuery, bool), DesktopProblem> {
    let offline = request.offline;
    let query = SearchQuery::try_new(request.query)
        .map_err(|_| DesktopProblem::invalid_input("The provider Search query is invalid."))?;
    let provider = ProviderId::try_new(provider_id)
        .map_err(|_| DesktopProblem::invalid_input("The Search provider is invalid."))?;
    let locale = request
        .locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("The Search locale is invalid."))?;
    let region = request
        .region
        .map(MetadataRegion::try_new)
        .transpose()
        .map_err(|_| DesktopProblem::invalid_input("The Search region is invalid."))?;
    if request.grains.len() > 32 {
        return Err(DesktopProblem::invalid_input("Too many provider Search grains."));
    }
    let grains = request
        .grains
        .into_iter()
        .map(|grain| grain.parse::<Grain>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| DesktopProblem::invalid_input("A provider Search grain is invalid."))?;
    let query = SearchProviderQuery::try_new(
        query,
        provider,
        request.page,
        locale,
        region,
        grains,
    )
    .map_err(|_| DesktopProblem::invalid_input("The provider Search page is invalid."))?;
    Ok((query, offline))
}

pub(crate) async fn provider_page(
    runtime: Arc<ProviderRuntime>,
    kernel: Arc<SqliteKernel>,
    access: RequestAccessContext,
    policy: OutboundAccessPolicy,
    input: ProviderPageInput,
    lease: ProviderOperationLease,
) -> Result<SearchProviderPageResponse, DesktopProblem> {
    let (query, offline) = provider_query(input.provider_id, input.request)?;
    let outcome = ProviderSearchService::new(runtime, kernel)
        .search_page(
            SearchPageRequest {
                correlation_id: RequestCorrelationId::new_v7(),
                access: access.into(),
                query: query.clone(),
                outbound_policy: policy,
                terms_revision: String::new(),
            },
            offline,
            lease,
        )
        .await
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(SearchProviderPageResponse::from_outcome(&query, outcome))
}

fn candidate_request(
    access: RequestAccessContext,
    policy: OutboundAccessPolicy,
    provider_id: String,
    grain: String,
    candidate_receipt_id: String,
) -> Result<ReadSearchCandidateRequest, DesktopProblem> {
    Ok(ReadSearchCandidateRequest {
        correlation_id: RequestCorrelationId::new_v7(),
        access: access.into(),
        candidate_receipt_id: candidate_receipt_id
            .parse()
            .map_err(|_| DesktopProblem::invalid_input("The Search receipt is invalid."))?,
        provider: ProviderId::try_new(provider_id)
            .map_err(|_| DesktopProblem::invalid_input("The Search provider is invalid."))?,
        grain: grain
            .parse()
            .map_err(|_| DesktopProblem::invalid_input("The Search grain is invalid."))?,
        outbound_policy: policy,
        terms_revision: String::new(),
    })
}

pub(crate) async fn candidate_details(
    runtime: Arc<ProviderRuntime>,
    kernel: Arc<SqliteKernel>,
    access: RequestAccessContext,
    policy: OutboundAccessPolicy,
    input: CandidateDetailsInput,
    lease: ProviderOperationLease,
) -> Result<SearchCandidateDetailsResponse, DesktopProblem> {
    let request = candidate_request(
        access,
        policy,
        input.provider_id,
        input.grain,
        input.candidate_receipt_id,
    )?;
    let outcome = ProviderSearchService::new(runtime, kernel)
        .candidate_details(request, input.offline, lease)
        .await
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(SearchCandidateDetailsResponse::from(outcome))
}

pub(crate) async fn save_candidate(
    runtime: Arc<ProviderRuntime>,
    kernel: Arc<SqliteKernel>,
    access: RequestAccessContext,
    policy: OutboundAccessPolicy,
    input: CandidateActionInput,
    lease: ProviderOperationLease,
) -> Result<SearchCandidateActionResponse, DesktopProblem> {
    let capability = CapabilityKey::AttachIdentifier;
    let request = candidate_request(
        access,
        policy,
        input.provider_id,
        input.grain,
        input.candidate_receipt_id,
    )?;
    let operation_id = input
        .request
        .operation_id
        .parse()
        .map_err(|_| DesktopProblem::invalid_input("The Search operation is invalid."))?;
    let action = match input.request.action {
        SearchRecordActionDto::Create {} => SearchRecordAction::Create,
        SearchRecordActionDto::Attach { record_id } => SearchRecordAction::Attach(
            record_id
                .parse()
                .map_err(|_| DesktopProblem::invalid_input("The target Record is invalid."))?,
        ),
    };
    let correlation_id = request.correlation_id;
    let outcome = ProviderSearchService::new(runtime, kernel)
        .save_candidate(
            SearchCandidateActionCommand {
                request,
                operation_id,
                action,
                evidence_mode: input.request.evidence_mode.into(),
            },
            lease,
        )
        .await
        .map_err(|problem| DesktopProblem::application(&problem))?;
    SearchCandidateActionResponse::try_from(outcome).map_err(|_| {
        DesktopProblem::application(&FastiProblem::from_code(
            fasti_application::ProblemCode::IntegrityFailed,
            capability,
            correlation_id,
        ))
    })
}

pub(crate) async fn save_provider_identifier(
    runtime: Arc<ProviderRuntime>,
    kernel: Arc<SqliteKernel>,
    access: RequestAccessContext,
    policy: OutboundAccessPolicy,
    input: ProviderIdentifierActionInput,
    lease: ProviderOperationLease,
) -> Result<ProviderIdentifierActionResponse, DesktopProblem> {
    let action = match input.request.action {
        SearchRecordActionDto::Create {} => SearchRecordAction::Create,
        SearchRecordActionDto::Attach { record_id } => SearchRecordAction::Attach(
            record_id
                .parse()
                .map_err(|_| DesktopProblem::invalid_input("The target Record is invalid."))?,
        ),
    };
    let command = ProviderIdentifierActionCommand {
        correlation_id: RequestCorrelationId::new_v7(),
        access: access.into(),
        outbound_policy: policy,
        terms_revision: String::new(),
        operation_id: input
            .request
            .operation_id
            .parse()
            .map_err(|_| DesktopProblem::invalid_input("The Search operation is invalid."))?,
        provider: ProviderId::try_new(input.provider_id)
            .map_err(|_| DesktopProblem::invalid_input("The Search provider is invalid."))?,
        provider_record_id: input.request.provider_record_id,
        grain: input
            .grain
            .parse()
            .map_err(|_| DesktopProblem::invalid_input("The Search grain is invalid."))?,
        action,
    };
    ProviderSearchService::new(runtime, kernel)
        .save_provider_identifier(command, lease)
        .await
        .map(ProviderIdentifierActionResponse::from)
        .map_err(|problem| DesktopProblem::application(&problem))
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

    #[test]
    fn native_provider_search_inputs_preserve_page_mode_and_candidate_scope() {
        let (query, offline) = provider_query(
            "tmdb".to_owned(),
            SearchProviderPageRequest {
                query: "Example".to_owned(),
                page: 3,
                locale: Some("en-IE".to_owned()),
                region: Some("IE".to_owned()),
                grains: vec!["film".to_owned()],
                offline: true,
            },
        )
        .unwrap();
        assert_eq!(query.provider().as_str(), "tmdb");
        assert_eq!(query.query().as_str(), "Example");
        assert_eq!(query.page(), 3);
        assert_eq!(query.locale().unwrap().as_str(), "en-ie");
        assert_eq!(query.region().unwrap().as_str(), "IE");
        assert_eq!(query.grains(), &[Grain::Film]);
        assert!(offline);

        let root = tempfile::tempdir().unwrap();
        let kernel = SqliteKernel::open(root.path()).unwrap();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).unwrap();
        let access = records::require_access(&kernel, &store).unwrap();
        let receipt = fasti_domain::SearchCandidateReceiptId::new_v7();
        let request = candidate_request(
            access,
            OutboundAccessPolicy::default(),
            "tmdb".to_owned(),
            "film".to_owned(),
            receipt.to_string(),
        )
        .unwrap();
        assert_eq!(request.provider.as_str(), "tmdb");
        assert_eq!(request.grain, Grain::Film);
        assert_eq!(request.candidate_receipt_id, receipt);

        assert!(provider_query(
            "tmdb".to_owned(),
            SearchProviderPageRequest {
                query: "Example".to_owned(),
                page: 0,
                locale: None,
                region: None,
                grains: Vec::new(),
                offline: false,
            },
        )
        .is_err());
    }
}
