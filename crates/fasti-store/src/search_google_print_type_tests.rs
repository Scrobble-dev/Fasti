pub(crate) mod google_print_type_tests {
    use super::*;
    use crate::metadata::load_field_claims;
    use fasti_application::{
        provider_candidate_metadata_fields, CreateRecordCommand, GoogleBooksPrintType,
        IdentityPort, ProviderMetadataField, ProviderResponseCachePolicy, ProviderResponseReuse,
        SearchCandidateActionCommand, SearchCandidateActionPreparation,
        SearchCandidateEvidenceMode, SearchRecordAction, GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
    };
    use fasti_domain::{
        FieldClaim, FieldClaimStatus, FieldKey, Grain, MetadataClaimId, OperationId, ReceivedAt,
        TITLE_FIELD_KEY,
    };

    pub(crate) fn google_fixture(
        value: GoogleBooksPrintType,
    ) -> (TestNode, SearchCandidateActionCommand) {
        let (node, mut request) = setup();
        for capability in ["metadata.search", "metadata.read"] {
            let state = ProviderCapabilityState::try_new(
                ProviderId::try_new("google-books").unwrap(),
                ProviderCapabilityId::try_new(capability).unwrap(),
                ProviderCapabilityStatus::Available,
                1,
                CredentialRequirement::OptionalApiKey,
                None,
                ProviderCredentialStatus::Optional,
                ConfigurationDigest::parse("b".repeat(64)).unwrap(),
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .unwrap();
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), state)
                .unwrap();
        }
        request.query = SearchProviderQuery::try_new(
            SearchQuery::try_new("Native type evidence").unwrap(),
            ProviderId::try_new("google-books").unwrap(),
            1,
            None,
            None,
            vec![Grain::Edition],
        )
        .unwrap();
        request.terms_revision = "google-books-v1".into();
        let candidate = SearchCandidate::try_new(SearchCandidateData {
            provider: "google-books".into(),
            kind: "book".into(),
            google_books_print_type: Some(value),
            ..candidate("42").data().clone()
        })
        .unwrap();
        let saved = commit(&node, &request, &[candidate]);
        // Reuse the existing historical receipt fixture: saving must not renew it.
        age_page(&node, saved.sequence, 180);
        let mut read = details(&request, saved.candidates[0].id());
        read.grain = Grain::Edition;
        let snapshot = node.kernel.read_search_candidate(&read).unwrap().unwrap();
        assert_eq!(
            snapshot.receipt.candidate().data().google_books_print_type,
            Some(value)
        );
        (
            node,
            SearchCandidateActionCommand {
                request: read,
                operation_id: OperationId::new_v7(),
                action: SearchRecordAction::Create,
                evidence_mode: SearchCandidateEvidenceMode::Cached,
            },
        )
    }

    pub(super) fn rows(node: &TestNode, tables: &[&str]) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        tables
            .iter()
            .map(|table| {
                let mut statement = connection
                    .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2"))
                    .unwrap();
                let columns = statement.column_count();
                statement
                    .query_map([], |row| {
                        (0..columns)
                            .map(|column| row.get::<_, rusqlite::types::Value>(column))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            })
            .collect()
    }

    const MUTATION_TABLES: &[&str] = &[
        "records",
        "namespace_definitions",
        "external_identifiers",
        "metadata_field_claims",
        "metadata_claims",
        "metadata_claim_provenance",
        "local_search_grams",
        "search_action_receipts",
        "workspace_revisions",
    ];
    const UNRELATED_TABLES: &[&str] = &[
        "profile_record_tracking_dispositions",
        "observations",
        "occurrences",
        "interpretations",
        "metadata_profile_field_overrides",
        "metadata_projection_policies",
        "metadata_rating_claims",
        "profile_nuvio_collections",
        "search_pages",
        "search_candidate_receipts",
    ];

    #[test]
    fn google_print_type_cached_and_refetched_create_attach_keep_observation() {
        for value in [
            GoogleBooksPrintType::Book,
            GoogleBooksPrintType::Magazine,
            GoogleBooksPrintType::Unknown,
        ] {
            for refetch in [false, true] {
                for attach in [false, true] {
                    // A refetch differs from the cached BOOK observation, including UNKNOWN.
                    let cached = if refetch {
                        GoogleBooksPrintType::Book
                    } else {
                        value
                    };
                    let (node, mut command) = google_fixture(cached);
                    if attach {
                        let record = node
                            .kernel
                            .create_record(CreateRecordCommand::new(
                                RequestCorrelationId::new_v7(),
                                node.access,
                                Grain::Edition,
                            ))
                            .unwrap()
                            .record_id();
                        command.action = SearchRecordAction::Attach(record);
                    }
                    if refetch {
                        command.evidence_mode = SearchCandidateEvidenceMode::Refetch;
                    }
                    let prepared = node
                        .kernel
                        .prepare_search_candidate_action(&command)
                        .unwrap();
                    let snapshot = node
                        .kernel
                        .read_search_candidate(&command.request)
                        .unwrap()
                        .unwrap();
                    let (fields, policy) = if refetch {
                        assert!(matches!(
                            prepared,
                            SearchCandidateActionPreparation::Refetch(_)
                        ));
                        let mut data = snapshot.receipt.candidate().data().clone();
                        data.google_books_print_type = Some(value);
                        let at = chrono::DateTime::from_timestamp_micros(now().timestamp_micros())
                            .unwrap();
                        let policy = ProviderResponseCachePolicy::new(
                            ProviderResponseReuse::Reusable,
                            at,
                            std::time::Duration::ZERO,
                            None,
                            None,
                        );
                        let fields = provider_candidate_metadata_fields(
                            &SearchCandidate::try_new(data).unwrap(),
                            None,
                            None,
                            &Sha256Digest::from_bytes(&[9; 32]),
                            ReceivedAt::from_application_clock(at),
                            Some(at + Duration::seconds(fasti_domain::METADATA_FRESH_SECONDS)),
                            FieldClaimStatus::Fresh,
                        )
                        .unwrap();
                        (fields, policy)
                    } else {
                        assert!(matches!(
                            prepared,
                            SearchCandidateActionPreparation::Cached(_)
                        ));
                        (
                            snapshot.metadata_fields().unwrap(),
                            snapshot.response_policy,
                        )
                    };
                    let unrelated = rows(&node, UNRELATED_TABLES);
                    let result = node
                        .kernel
                        .commit_search_candidate_action(
                            &command,
                            &prepared,
                            refetch.then_some((fields.as_slice(), &policy)),
                        )
                        .unwrap();
                    assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
                    let connection = node.kernel.inner.connection.lock().unwrap();
                    let key = FieldKey::try_new(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY).unwrap();
                    let claims = load_field_claims(
                        &connection,
                        node.access.workspace_id(),
                        result.record_id,
                        &key,
                        CAPABILITY,
                        command.request.correlation_id,
                        now(),
                    )
                    .unwrap();
                    assert_eq!(claims.len(), 1);
                    let claim = &claims[0];
                    let expected = fields
                        .iter()
                        .find(|field| field.field_key() == &key)
                        .unwrap()
                        .claim();
                    assert_eq!(claim.value(), value.as_str());
                    assert_eq!(claim.provenance(), expected.provenance());
                    assert_eq!(claim.fetched_at(), expected.fetched_at());
                    assert_eq!(claim.expires_at(), expected.expires_at());
                    assert_eq!(claim.initial_status(), expected.initial_status());
                    assert_eq!(claim.record_id(), Some(result.record_id));
                    assert_eq!(claim.field_key(), Some(&key));
                    let title_key = FieldKey::try_new(TITLE_FIELD_KEY).unwrap();
                    let title = load_field_claims(
                        &connection,
                        node.access.workspace_id(),
                        result.record_id,
                        &title_key,
                        CAPABILITY,
                        command.request.correlation_id,
                        now(),
                    )
                    .unwrap();
                    assert_eq!(title.len(), 1);
                    assert_eq!(claim.provenance(), title[0].provenance());
                    assert_eq!(claim.expires_at(), title[0].expires_at());
                    assert_eq!(
                        claim.provenance().provider_id().unwrap().as_str(),
                        "google-books"
                    );
                    assert_eq!(claim.source().as_str(), "googlebooks.volume");
                    assert_eq!(claim.provenance().source_identifier(), Some("42"));
                    if refetch {
                        assert_ne!(
                            claim.provenance().evidence_digest(),
                            Some(snapshot.receipt.response_digest())
                        );
                    } else {
                        assert_eq!(claim.fetched_at(), snapshot.receipt.lifetime().created_at());
                        assert_eq!(
                            claim.expires_at(),
                            Some(snapshot.receipt.lifetime().fresh_until())
                        );
                        assert!(!claim.is_fresh(now()));
                    }
                }
            }
        }
    }

    #[test]
    fn google_print_type_invalid_or_cross_provider_refetch_is_atomic() {
        for cross_provider in [false, true] {
            for attach in [false, true] {
                let (node, mut command) = if cross_provider {
                    let (node, request) = setup();
                    node.kernel
                        .put_provider_capability_state(
                            node.access.workspace_id(),
                            state_for("metadata.read", 1),
                        )
                        .unwrap();
                    let page = commit(&node, &request, &[candidate("42")]);
                    let command = SearchCandidateActionCommand {
                        request: details(&request, page.candidates[0].id()),
                        operation_id: OperationId::new_v7(),
                        action: SearchRecordAction::Create,
                        evidence_mode: SearchCandidateEvidenceMode::Refetch,
                    };
                    (node, command)
                } else {
                    google_fixture(GoogleBooksPrintType::Book)
                };
                command.evidence_mode = SearchCandidateEvidenceMode::Refetch;
                if attach {
                    let target = node
                        .kernel
                        .create_record(CreateRecordCommand::new(
                            RequestCorrelationId::new_v7(),
                            node.access,
                            command.request.grain,
                        ))
                        .unwrap()
                        .record_id();
                    command.action = SearchRecordAction::Attach(target);
                }
                let prepared = node
                    .kernel
                    .prepare_search_candidate_action(&command)
                    .unwrap();
                let SearchCandidateActionPreparation::Refetch(details) = &prepared else {
                    panic!("expected refetch preparation")
                };
                let at = chrono::DateTime::from_timestamp_micros(now().timestamp_micros()).unwrap();
                let policy = ProviderResponseCachePolicy::new(
                    ProviderResponseReuse::Reusable,
                    at,
                    std::time::Duration::ZERO,
                    None,
                    None,
                );
                let mut fields = provider_candidate_metadata_fields(
                    details.candidate.receipt.candidate(),
                    fasti_application::provider_metadata_response_locale(
                        command.request.provider.as_str(),
                        None,
                    ),
                    None,
                    &Sha256Digest::from_bytes(&[11; 32]),
                    ReceivedAt::from_application_clock(at),
                    Some(at + Duration::seconds(fasti_domain::METADATA_FRESH_SECONDS)),
                    FieldClaimStatus::Fresh,
                )
                .unwrap();
                fields.retain(|field| {
                    field.field_key().as_str() != GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY
                });
                let first = fields[0].claim();
                // All observation coordinates agree. Only the native type's provider/value is invalid.
                let forged = FieldClaim::try_new_unbound_provider(
                    MetadataClaimId::new_v7(),
                    if cross_provider { "BOOK" } else { "NOVEL" },
                    first.provenance().clone(),
                    ReceivedAt::from_application_clock(first.fetched_at()),
                    first.expires_at(),
                    first.initial_status(),
                )
                .unwrap();
                fields.push(ProviderMetadataField::new(
                    FieldKey::try_new(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY).unwrap(),
                    forged,
                ));
                let before = rows(&node, MUTATION_TABLES);
                let unrelated = rows(&node, UNRELATED_TABLES);
                let error = node
                    .kernel
                    .commit_search_candidate_action(&command, &prepared, Some((&fields, &policy)))
                    .unwrap_err();
                assert_eq!(error.code(), ProblemCode::ValidationFailed);
                assert_eq!(rows(&node, MUTATION_TABLES), before);
                assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
            }
        }
    }
}
