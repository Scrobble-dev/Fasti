pub(crate) mod kitsu_tests {
    use super::google_print_type_tests::rows;
    use super::*;
    use crate::metadata::load_field_claims;
    use fasti_application::{
        provider_candidate_metadata_fields, CreateRecordCommand, IdentityPort, KitsuMangaSubtype,
        ProviderMetadataField, ProviderResponseCachePolicy, ProviderResponseReuse,
        SearchCandidateActionCommand, SearchCandidateActionPreparation,
        SearchCandidateEvidenceMode, SearchRecordAction, GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
        KITSU_MANGA_SUBTYPE_FIELD_KEY,
    };
    use fasti_domain::{
        FieldClaim, FieldClaimStatus, FieldKey, Grain, MetadataClaimId, OperationId, ReceivedAt,
    };

    const ACTION_TABLES: &[&str] = &[
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

    pub(crate) fn fixture(value: KitsuMangaSubtype) -> (TestNode, SearchCandidateActionCommand) {
        let (node, mut request) = setup();
        for capability in ["metadata.search", "metadata.read"] {
            node.kernel
                .put_provider_capability_state(
                    node.access.workspace_id(),
                    ProviderCapabilityState::try_new(
                        ProviderId::try_new("kitsu").unwrap(),
                        ProviderCapabilityId::try_new(capability).unwrap(),
                        ProviderCapabilityStatus::Available,
                        1,
                        CredentialRequirement::None,
                        None,
                        ProviderCredentialStatus::NotRequired,
                        ConfigurationDigest::parse("c".repeat(64)).unwrap(),
                        ProviderCheckMetadata::never_run(),
                        ProviderCheckMetadata::never_run(),
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        request.query = SearchProviderQuery::try_new(
            SearchQuery::try_new("Kitsu native evidence").unwrap(),
            ProviderId::try_new("kitsu").unwrap(),
            1,
            None,
            None,
            vec![Grain::Work],
        )
        .unwrap();
        request.terms_revision = "kitsu-v1".into();
        let candidate = SearchCandidate::try_new(SearchCandidateData {
            provider: "kitsu".into(),
            kind: "manga".into(),
            title: "A manga resource".into(),
            kitsu_manga_subtype: Some(value),
            google_books_print_type: None,
            ..candidate("42").data().clone()
        })
        .unwrap();
        let saved = commit(&node, &request, &[candidate]);
        age_page(&node, saved.sequence, 180);
        let mut read = details(&request, saved.candidates[0].id());
        read.grain = Grain::Work;
        assert_eq!(
            node.kernel
                .read_search_candidate(&read)
                .unwrap()
                .unwrap()
                .receipt
                .candidate()
                .data()
                .kitsu_manga_subtype,
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

    fn attach_target(node: &TestNode, command: &mut SearchCandidateActionCommand) {
        let target = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Work,
            ))
            .unwrap()
            .record_id();
        command.action = SearchRecordAction::Attach(target);
    }

    fn refetched_fields(
        candidate: &SearchCandidate,
    ) -> (Vec<ProviderMetadataField>, ProviderResponseCachePolicy) {
        let at = chrono::DateTime::from_timestamp_micros(now().timestamp_micros()).unwrap();
        let policy = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::Reusable,
            at,
            std::time::Duration::ZERO,
            None,
            None,
        );
        let fields = provider_candidate_metadata_fields(
            candidate,
            None,
            None,
            &Sha256Digest::from_bytes(&[9; 32]),
            ReceivedAt::from_application_clock(at),
            Some(at + Duration::seconds(fasti_domain::METADATA_FRESH_SECONDS)),
            FieldClaimStatus::Fresh,
        )
        .unwrap();
        (fields, policy)
    }

    #[test]
    fn kitsu_cached_and_refetched_create_attach_preserve_native_observation_and_identity() {
        for value in [
            KitsuMangaSubtype::Manga,
            KitsuMangaSubtype::Novel,
            KitsuMangaSubtype::Unknown,
        ] {
            for refetch in [false, true] {
                for attach in [false, true] {
                    let (node, mut command) = fixture(if refetch {
                        KitsuMangaSubtype::Manga
                    } else {
                        value
                    });
                    if attach {
                        attach_target(&node, &mut command);
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
                        data.kitsu_manga_subtype = Some(value);
                        refetched_fields(&SearchCandidate::try_new(data).unwrap())
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
                    let outcome = node
                        .kernel
                        .commit_search_candidate_action(
                            &command,
                            &prepared,
                            refetch.then_some((fields.as_slice(), &policy)),
                        )
                        .unwrap();
                    if let SearchRecordAction::Attach(target) = command.action {
                        assert_eq!(outcome.record_id, target);
                    }
                    assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
                    let connection = node.kernel.inner.connection.lock().unwrap();
                    let key = FieldKey::try_new(KITSU_MANGA_SUBTYPE_FIELD_KEY).unwrap();
                    let claims = load_field_claims(
                        &connection,
                        node.access.workspace_id(),
                        outcome.record_id,
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
                    assert_eq!(claim.record_id(), Some(outcome.record_id));
                    assert_eq!(claim.field_key(), Some(&key));
                    assert_eq!(claim.provenance().provider_id().unwrap().as_str(), "kitsu");
                    assert_eq!(claim.source().as_str(), "kitsu.manga");
                    assert_eq!(claim.provenance().source_identifier(), Some("42"));
                    let stored_policy: String = connection
                        .query_row(
                            "SELECT response_policy_json FROM metadata_claims WHERE claim_id = ?1",
                            [claim.claim_id().to_string()],
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert_eq!(stored_policy, policy.to_canonical_json());
                    let grain: String = connection
                        .query_row(
                            "SELECT grain FROM records WHERE record_id = ?1",
                            [outcome.record_id.to_string()],
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert_eq!(grain, "work");
                    let identifier: (String, String) = connection.query_row(
                        "SELECT namespace, value FROM external_identifiers WHERE record_id = ?1",
                        [outcome.record_id.to_string()], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
                    assert_eq!(identifier, ("kitsu.manga".into(), "42".into()));
                    if refetch {
                        assert_eq!(
                            claim.provenance().evidence_digest(),
                            Some(&Sha256Digest::from_bytes(&[9; 32]))
                        );
                        assert_ne!(
                            claim.provenance().evidence_digest(),
                            Some(snapshot.receipt.response_digest())
                        );
                    } else {
                        assert_eq!(
                            claim.provenance().evidence_digest(),
                            Some(snapshot.receipt.response_digest())
                        );
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
    fn kitsu_invalid_native_value_or_wrong_provider_fact_rejects_create_attach_atomically() {
        for (key, value) in [
            (KITSU_MANGA_SUBTYPE_FIELD_KEY, "MANGA"),
            (GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY, "BOOK"),
        ] {
            for attach in [false, true] {
                let (node, mut command) = fixture(KitsuMangaSubtype::Manga);
                command.evidence_mode = SearchCandidateEvidenceMode::Refetch;
                if attach {
                    attach_target(&node, &mut command);
                }
                let prepared = node
                    .kernel
                    .prepare_search_candidate_action(&command)
                    .unwrap();
                let SearchCandidateActionPreparation::Refetch(details) = &prepared else {
                    panic!("refetch required");
                };
                let (mut fields, policy) = refetched_fields(details.candidate.receipt.candidate());
                fields.retain(|field| field.field_key().as_str() != KITSU_MANGA_SUBTYPE_FIELD_KEY);
                let first = fields[0].claim();
                // All response coordinates agree; only the native fact is inadmissible.
                let forged = FieldClaim::try_new_unbound_provider(
                    MetadataClaimId::new_v7(),
                    value,
                    first.provenance().clone(),
                    ReceivedAt::from_application_clock(first.fetched_at()),
                    first.expires_at(),
                    first.initial_status(),
                )
                .unwrap();
                fields.push(ProviderMetadataField::new(
                    FieldKey::try_new(key).unwrap(),
                    forged,
                ));
                let before = rows(&node, ACTION_TABLES);
                let unrelated = rows(&node, UNRELATED_TABLES);
                let error = node
                    .kernel
                    .commit_search_candidate_action(&command, &prepared, Some((&fields, &policy)))
                    .unwrap_err();
                assert_eq!(error.code(), ProblemCode::ValidationFailed);
                assert_eq!(rows(&node, ACTION_TABLES), before);
                assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
            }
        }
    }
}
