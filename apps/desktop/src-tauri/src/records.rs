use crate::artwork::ArtworkCache;
use crate::providers::ProviderCandidate;
use crate::setup::{authenticate, DesktopProblem, SetupSecretStore};
use fasti_application::{
    ApplyProviderMetadataCommand, AttachIdentifierCommand, CreateProviderRecordCommand,
    CreateRecordCommand, IdentityPort, ListRecordsQuery, ListTrackingDispositionsQuery,
    ProfileRecordStatePort, ProviderMetadataPort, RegisterNamespaceDefinitionCommand,
    RequestAccessContext, SetTrackingDispositionCommand,
};
use fasti_contracts::{
    ListRecordsQueryParameters, ListTrackingDispositionsResponse, TrackingDispositionDto,
    TrackingDispositionStateDto, TrackingDispositionUpdateDto,
};
use fasti_domain::{
    ExternalIdentifierClaim, Grain, InterpretationState, NamespaceDefinition,
    NamespaceLicencePosture, OccurredAt, RecordId, RecordStatus, RequestCorrelationId,
    ResolvedField, TrackingDisposition,
};
use fasti_store::SqliteKernel;
use serde::{Deserialize, Serialize};

/// Wire projection of [`fasti_domain::ResolvedField`]. Reuses the domain
/// enum's own `Serialize` impl for `tier` rather than re-deriving the same
/// snake_case mapping here.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolvedFieldView {
    tier: fasti_domain::FieldResolutionTier,
    value: Option<String>,
    source: Option<String>,
    is_stale: bool,
}

impl From<&ResolvedField> for ResolvedFieldView {
    fn from(field: &ResolvedField) -> Self {
        Self {
            tier: field.tier(),
            value: field.value().map(ToOwned::to_owned),
            source: field.source().map(ToString::to_string),
            is_stale: field.is_stale(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RecordActivityView {
    occurred_at: Option<OccurredAt>,
    interpretation_state: InterpretationState,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RecordIdentifierView {
    namespace: String,
    grain: Grain,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RecordSummary {
    record_id: String,
    /// Identity granularity, not the frontend's display `MediaKind`. A later
    /// frontend-wiring pass owns the `Grain` -> `MediaKind` projection.
    grain: Grain,
    status: RecordStatus,
    title: ResolvedFieldView,
    poster: ResolvedFieldView,
    poster_asset_path: Option<String>,
    original_title: ResolvedFieldView,
    overview: ResolvedFieldView,
    release_year: ResolvedFieldView,
    identifiers: Vec<RecordIdentifierView>,
    latest_activity: Option<RecordActivityView>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RecordPage {
    records: Vec<RecordSummary>,
    truncated: bool,
}

pub(crate) fn require_access(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<fasti_application::RequestAccessContext, DesktopProblem> {
    authenticate(kernel, store)?.ok_or_else(DesktopProblem::not_authenticated)
}

pub(crate) fn list_records(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    artwork: &ArtworkCache,
    query: Option<ListRecordsQueryParameters>,
) -> Result<RecordPage, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
    let selector = query
        .and_then(|query| query.record_id)
        .map(|id| {
            id.parse::<RecordId>()
                .map_err(|_| fasti_application::InvalidRecordSelector)
        })
        .transpose();
    let summaries = kernel
        .list_records(ListRecordsQuery::new(correlation_id, access).with_record_selector(selector))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    let truncated = summaries.truncated();
    let records = summaries
        .into_records()
        .into_iter()
        .map(|summary| {
            let poster_asset_path = summary
                .poster()
                .source()
                .zip(summary.poster().value())
                .and_then(|(source, url)| artwork.local_path(source.as_str(), url));
            RecordSummary {
                record_id: summary.record_id().to_string(),
                grain: summary.grain(),
                status: summary.status(),
                title: summary.title().into(),
                poster: summary.poster().into(),
                poster_asset_path,
                original_title: summary.original_title().into(),
                overview: summary.overview().into(),
                release_year: summary.release_year().into(),
                identifiers: summary
                    .identifiers()
                    .iter()
                    .map(|identifier| RecordIdentifierView {
                        namespace: identifier.namespace().to_string(),
                        grain: identifier.grain(),
                        value: identifier.value().to_owned(),
                    })
                    .collect(),
                latest_activity: summary
                    .latest_activity()
                    .map(|activity| RecordActivityView {
                        occurred_at: activity.occurred_at().cloned(),
                        interpretation_state: activity.interpretation_state(),
                    }),
            }
        })
        .collect();
    Ok(RecordPage { records, truncated })
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateRecordView {
    record_id: String,
    grain: Grain,
}

pub(crate) fn create_record(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    grain: Grain,
) -> Result<CreateRecordView, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();
    let outcome = kernel
        .create_record(CreateRecordCommand::new(correlation_id, access, grain))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(CreateRecordView {
        record_id: outcome.record_id().to_string(),
        grain: outcome.grain(),
    })
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AttachIdentifierInput {
    record_id: String,
    namespace: String,
    grain: Grain,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AttachIdentifierView {
    external_identifier_id: String,
    record_id: String,
    created: bool,
}

pub(crate) fn attach_identifier(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    input: AttachIdentifierInput,
) -> Result<AttachIdentifierView, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();
    let record_id = input
        .record_id
        .parse::<RecordId>()
        .map_err(|_| DesktopProblem::invalid_input("record_id is not a valid record identifier"))?;
    let claim = ExternalIdentifierClaim::try_new(input.namespace, input.grain, input.value)
        .map_err(|_| DesktopProblem::invalid_input("the identifier claim is invalid"))?;
    let outcome = kernel
        .attach_identifier(AttachIdentifierCommand::new(
            correlation_id,
            access,
            record_id,
            claim,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(AttachIdentifierView {
        external_identifier_id: outcome.external_identifier_id().to_string(),
        record_id: outcome.record_id().to_string(),
        created: outcome.created(),
    })
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RegisterNamespaceInput {
    namespace: String,
    label: String,
    grains: Vec<Grain>,
    id_pattern: String,
    normalization: String,
    licence_posture: NamespaceLicencePosture,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RegisterNamespaceView {
    namespace: String,
    created: bool,
}

/// Declares a namespace's supported grains and ID shape so
/// [`attach_identifier`] can accept claims under it -- `attach_identifier_tx`
/// (crates/fasti-store/src/identity.rs) rejects any claim whose namespace
/// has no matching `namespace_definitions` row for the workspace.
pub(crate) fn register_namespace(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    input: RegisterNamespaceInput,
) -> Result<RegisterNamespaceView, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();
    let definition = NamespaceDefinition::try_new(
        input.namespace,
        input.label,
        input.grains,
        input.id_pattern,
        input.normalization,
        input.licence_posture,
    )
    .map_err(|_| DesktopProblem::invalid_input("the namespace definition is invalid"))?;
    let outcome = kernel
        .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
            correlation_id,
            access,
            definition,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(RegisterNamespaceView {
        namespace: outcome.namespace().to_string(),
        created: outcome.created(),
    })
}

fn register_provider_namespace(
    kernel: &SqliteKernel,
    access: fasti_application::RequestAccessContext,
    candidate: &ProviderCandidate,
) -> Result<(), DesktopProblem> {
    let definition = candidate.namespace_definition()?;
    kernel
        .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            definition,
        ))
        .map(|_| ())
        .map_err(|problem| DesktopProblem::application(&problem))
}

pub(crate) fn create_provider_record(
    kernel: &SqliteKernel,
    access: RequestAccessContext,
    candidate: ProviderCandidate,
) -> Result<CreateRecordView, DesktopProblem> {
    register_provider_namespace(kernel, access, &candidate)?;
    let grain = candidate.grain()?;
    let identifier = candidate.identifier()?;
    let fields = candidate.metadata_fields(None, None)?;
    let outcome = kernel
        .create_provider_record(CreateProviderRecordCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            grain,
            identifier,
            fields,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(CreateRecordView {
        record_id: outcome.record_id().to_string(),
        grain: outcome.grain(),
    })
}

pub(crate) fn apply_provider_metadata(
    kernel: &SqliteKernel,
    access: RequestAccessContext,
    record_id: RecordId,
    candidate: ProviderCandidate,
) -> Result<(), DesktopProblem> {
    register_provider_namespace(kernel, access, &candidate)?;
    let identifier = candidate.identifier()?;
    let fields = candidate.metadata_fields(None, None)?;
    kernel
        .apply_provider_metadata(ApplyProviderMetadataCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            record_id,
            identifier,
            fields,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))
}

fn disposition_dto(disposition: TrackingDisposition) -> TrackingDispositionDto {
    match disposition {
        TrackingDisposition::Watching => TrackingDispositionDto::Watching,
        TrackingDisposition::OnHold => TrackingDispositionDto::OnHold,
        TrackingDisposition::Dropped => TrackingDispositionDto::Dropped,
    }
}

pub(crate) fn list_tracking_dispositions(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<ListTrackingDispositionsResponse, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();
    kernel
        .list_tracking_dispositions(ListTrackingDispositionsQuery::new(correlation_id, access))
        .map(|page| {
            let truncated = page.truncated();
            ListTrackingDispositionsResponse {
                states: page
                    .into_states()
                    .into_iter()
                    .map(|state| TrackingDispositionStateDto {
                        record_id: state.record_id().to_string(),
                        disposition: Some(disposition_dto(state.disposition())),
                    })
                    .collect(),
                truncated,
            }
        })
        .map_err(|problem| DesktopProblem::application(&problem))
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SetTrackingDispositionInput {
    record_id: String,
    disposition: TrackingDispositionUpdateDto,
}

pub(crate) fn set_tracking_disposition(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    input: SetTrackingDispositionInput,
) -> Result<TrackingDispositionStateDto, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();
    let record_id = input
        .record_id
        .parse::<RecordId>()
        .map_err(|_| DesktopProblem::invalid_input("record_id is not a valid record identifier"))?;
    let disposition = match input.disposition {
        TrackingDispositionUpdateDto::Watching => Some(TrackingDisposition::Watching),
        TrackingDispositionUpdateDto::OnHold => Some(TrackingDisposition::OnHold),
        TrackingDispositionUpdateDto::Dropped => Some(TrackingDisposition::Dropped),
        TrackingDispositionUpdateDto::Unset => None,
    };
    let state = kernel
        .set_tracking_disposition(SetTrackingDispositionCommand::new(
            correlation_id,
            access,
            record_id,
            disposition,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(TrackingDispositionStateDto {
        record_id: record_id.to_string(),
        disposition: state.map(|value| disposition_dto(value.disposition())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::complete_setup;
    use crate::setup::test_support::{new_kernel, MemoryStore};

    #[test]
    fn list_records_refuses_before_setup_completes() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let artwork = ArtworkCache::new(root.path().join("artwork"));

        assert!(matches!(
            list_records(&kernel, &store, &artwork, None),
            Err(problem) if problem.code() == "not_authenticated"
        ));
    }

    #[test]
    fn list_records_is_honestly_empty_on_a_fresh_node() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let artwork = ArtworkCache::new(root.path().join("artwork"));
        complete_setup(&kernel, &store).expect("complete setup");

        let records = list_records(&kernel, &store, &artwork, None).expect("list records");
        assert!(records.records.is_empty());
        assert!(!records.truncated);
    }

    #[test]
    fn list_records_exact_selector_preserves_default_and_missing_pages() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let artwork = ArtworkCache::new(root.path().join("artwork"));
        complete_setup(&kernel, &store).expect("complete setup");
        let first = create_record(&kernel, &store, Grain::Film).expect("first record");
        let selected = create_record(&kernel, &store, Grain::Work).expect("selected record");

        for query in [None, Some(ListRecordsQueryParameters::default())] {
            let page = list_records(&kernel, &store, &artwork, query).expect("default list");
            assert_eq!(page.records.len(), 2);
            assert!(!page.truncated);
            assert!(page
                .records
                .iter()
                .any(|record| record.record_id == first.record_id));
        }

        let page = list_records(
            &kernel,
            &store,
            &artwork,
            Some(ListRecordsQueryParameters {
                record_id: Some(selected.record_id.clone()),
            }),
        )
        .expect("exact selection");
        assert_eq!(page.records.len(), 1);
        assert_eq!(page.records[0].record_id, selected.record_id);
        assert_eq!(page.records[0].grain, Grain::Work);
        assert!(!page.truncated);

        let page = list_records(
            &kernel,
            &store,
            &artwork,
            Some(ListRecordsQueryParameters {
                record_id: Some(RecordId::new_v7().to_string()),
            }),
        )
        .expect("unknown selection");
        assert!(page.records.is_empty());
        assert!(!page.truncated);
    }

    #[test]
    fn list_records_invalid_selector_reaches_existing_authorization_owner() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let artwork = ArtworkCache::new(root.path().join("artwork"));
        let query = ListRecordsQueryParameters {
            record_id: Some("not-a-record-id".to_owned()),
        };
        assert_eq!(
            list_records(&kernel, &store, &artwork, Some(query.clone()))
                .expect_err("setup must precede selector validation")
                .code(),
            "not_authenticated",
        );
        complete_setup(&kernel, &store).expect("complete setup");
        assert_eq!(
            list_records(&kernel, &store, &artwork, Some(query))
                .expect_err("authorized malformed selector")
                .code(),
            "validation_failed",
        );
    }

    #[test]
    fn create_record_refuses_before_setup_completes() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();

        assert!(matches!(
            create_record(&kernel, &store, Grain::Film),
            Err(problem) if problem.code() == "not_authenticated"
        ));
    }

    #[test]
    fn create_record_makes_the_new_record_listable() {
        let (root, kernel) = new_kernel();
        let store = MemoryStore::default();
        let artwork = ArtworkCache::new(root.path().join("artwork"));
        complete_setup(&kernel, &store).expect("complete setup");

        let created = create_record(&kernel, &store, Grain::Film).expect("create record");
        assert_eq!(created.grain, Grain::Film);

        let records = list_records(&kernel, &store, &artwork, None).expect("list records");
        assert_eq!(records.records.len(), 1);
        assert_eq!(records.records[0].record_id, created.record_id);
        assert!(!records.truncated);
    }

    #[test]
    fn attach_identifier_refuses_before_setup_completes() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();

        let input = AttachIdentifierInput {
            record_id: fasti_domain::RecordId::new_v7().to_string(),
            namespace: "google_books".to_owned(),
            grain: Grain::Work,
            value: "abc123".to_owned(),
        };

        assert!(matches!(
            attach_identifier(&kernel, &store, input),
            Err(problem) if problem.code() == "not_authenticated"
        ));
    }

    #[test]
    fn attach_identifier_rejects_a_record_id_that_does_not_parse() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        let input = AttachIdentifierInput {
            record_id: "not-a-record-id".to_owned(),
            namespace: "google_books".to_owned(),
            grain: Grain::Work,
            value: "abc123".to_owned(),
        };

        assert!(matches!(
            attach_identifier(&kernel, &store, input),
            Err(problem) if problem.code() == "invalid_input"
        ));
    }

    #[test]
    fn attach_identifier_refuses_an_unregistered_namespace() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        let created = create_record(&kernel, &store, Grain::Work).expect("create record");
        let input = AttachIdentifierInput {
            record_id: created.record_id,
            namespace: "google_books".to_owned(),
            grain: Grain::Work,
            value: "abc123".to_owned(),
        };

        assert!(matches!(
            attach_identifier(&kernel, &store, input),
            Err(problem) if problem.code() == "invalid_identifier"
        ));
    }

    #[test]
    fn attach_identifier_attaches_a_claim_once_the_namespace_is_registered() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        register_namespace(
            &kernel,
            &store,
            RegisterNamespaceInput {
                namespace: "google_books".to_owned(),
                label: "Google Books".to_owned(),
                grains: vec![Grain::Work],
                id_pattern: ".+".to_owned(),
                normalization: "identity".to_owned(),
                licence_posture: NamespaceLicencePosture::IdentifiersOnly,
            },
        )
        .expect("register namespace");

        let created = create_record(&kernel, &store, Grain::Work).expect("create record");
        let input = AttachIdentifierInput {
            record_id: created.record_id.clone(),
            namespace: "google_books".to_owned(),
            grain: Grain::Work,
            value: "abc123".to_owned(),
        };

        let attached = attach_identifier(&kernel, &store, input).expect("attach identifier");
        assert_eq!(attached.record_id, created.record_id);
        assert!(attached.created);
    }

    #[test]
    fn register_namespace_is_idempotent() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        let input = || RegisterNamespaceInput {
            namespace: "google_books".to_owned(),
            label: "Google Books".to_owned(),
            grains: vec![Grain::Work],
            id_pattern: ".+".to_owned(),
            normalization: "identity".to_owned(),
            licence_posture: NamespaceLicencePosture::IdentifiersOnly,
        };

        let first = register_namespace(&kernel, &store, input()).expect("register namespace");
        assert!(first.created);

        let second =
            register_namespace(&kernel, &store, input()).expect("register namespace again");
        assert!(!second.created);
    }

    #[test]
    fn tracking_disposition_round_trips_through_the_desktop_adapter() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");
        let created = create_record(&kernel, &store, Grain::Film).expect("create record");

        let updated = set_tracking_disposition(
            &kernel,
            &store,
            SetTrackingDispositionInput {
                record_id: created.record_id.clone(),
                disposition: TrackingDispositionUpdateDto::OnHold,
            },
        )
        .expect("set disposition");
        assert_eq!(updated.record_id, created.record_id);
        assert_eq!(updated.disposition, Some(TrackingDispositionDto::OnHold));

        assert_eq!(
            list_tracking_dispositions(&kernel, &store).expect("list dispositions"),
            ListTrackingDispositionsResponse {
                states: vec![updated],
                truncated: false,
            }
        );
    }
}
