mod candidate_metadata_tests {
    use super::*;
    use crate::metadata::load_field_claims;
    use fasti_application::{
        provider_identity_mapping, ApplyProviderMetadataCommand, CreateProviderRecordCommand,
        IdentityPort, ProviderMetadataField, ProviderMetadataPort,
        RegisterNamespaceDefinitionCommand,
    };
    use fasti_domain::{FieldClaim, FieldClaimStatus, FieldKey, Grain, NamespaceKey, RecordId};

    fn fixture() -> (TestNode, StoredSearchCandidate) {
        let (node, mut request) = setup();
        let mapping = provider_identity_mapping("tmdb", "movie").unwrap();
        node.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                mapping.namespace_definition().unwrap(),
            ))
            .unwrap();
        request.query = SearchProviderQuery::try_new(
            SearchQuery::try_new("Original film").unwrap(),
            ProviderId::try_new("tmdb").unwrap(),
            1,
            Some(fasti_domain::MetadataLocale::try_new("fr-FR").unwrap()),
            Some(fasti_domain::MetadataRegion::try_new("FR").unwrap()),
            vec![Grain::Film],
        )
        .unwrap();
        let candidate = SearchCandidate::try_new(SearchCandidateData {
            title: "Titre original".into(),
            original_title: Some("Original title".into()),
            overview: Some("Original overview".into()),
            release_year: Some(2026),
            image_url: Some("https://image.tmdb.org/t/p/w500/poster.jpg".into()),
            ..candidate("42").data().clone()
        })
        .unwrap();
        let page = commit(&node, &request, &[candidate]);
        // Saving is deliberately later than freshness, but within the separate
        // receipt-read lifetime. This must not renew the observed claims.
        age_page(&node, page.sequence, 180);
        let snapshot = node
            .kernel
            .read_search_candidate(&details(&request, page.candidates[0].id()))
            .unwrap()
            .unwrap();
        (node, snapshot)
    }

    fn create(
        node: &TestNode,
        snapshot: &StoredSearchCandidate,
        fields: Vec<ProviderMetadataField>,
    ) -> ApplicationResult<fasti_application::CreateProviderRecordOutcome> {
        node.kernel
            .create_provider_record(CreateProviderRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
                snapshot.receipt.candidate().identifier().clone(),
                fields,
            ))
    }

    fn claims(
        node: &TestNode,
        record: RecordId,
        fields: &[ProviderMetadataField],
    ) -> Vec<FieldClaim> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        fields
            .iter()
            .map(|field| {
                let mut values = load_field_claims(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    field.field_key(),
                    CAPABILITY,
                    RequestCorrelationId::new_v7(),
                )
                .unwrap();
                assert_eq!(values.len(), 1);
                values.remove(0)
            })
            .collect()
    }

    fn source_rows(node: &TestNode) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        [
            "records",
            "external_identifiers",
            "metadata_field_claims",
            "metadata_claims",
            "metadata_claim_provenance",
            "local_search_grams",
            "workspace_revisions",
        ]
        .into_iter()
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

    #[test]
    fn cached_metadata_replay_preserves_stored_claim_ids_and_original_freshness() {
        let (node, snapshot) = fixture();
        let fields = snapshot.metadata_fields().unwrap();
        assert_eq!(fields.len(), 5);
        let record = create(&node, &snapshot, fields.clone())
            .unwrap()
            .record_id();
        let original = claims(&node, record, &fields);
        let before = source_rows(&node);
        let replay = snapshot.metadata_fields().unwrap();
        for (first, repeated) in fields.iter().zip(&replay) {
            assert_ne!(first.claim().claim_id(), repeated.claim().claim_id());
        }
        node.kernel
            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                snapshot.receipt.candidate().identifier().clone(),
                replay,
            ))
            .unwrap();
        assert_eq!(source_rows(&node), before);
        assert_eq!(claims(&node, record, &fields), original);
        let lifetime = snapshot.receipt.lifetime();
        assert_eq!(
            lifetime.fresh_until() - lifetime.created_at(),
            Duration::seconds(120)
        );
        assert!(now() > lifetime.fresh_until());
        for (field, stored) in fields.iter().zip(original) {
            assert_eq!(stored.claim_id(), field.claim().claim_id());
            assert_eq!(stored.fetched_at(), lifetime.created_at());
            assert_eq!(stored.expires_at(), Some(lifetime.fresh_until()));
            assert_ne!(stored.expires_at(), Some(lifetime.expires_at()));
            assert_eq!(stored.initial_status(), FieldClaimStatus::Fresh);
            assert!(stored.is_fresh(lifetime.fresh_until() - Duration::microseconds(1)));
            assert!(!stored.is_fresh(lifetime.fresh_until()));
            assert!(!stored.is_fresh(now()));
            assert_eq!(
                stored.provenance().evidence_digest(),
                Some(snapshot.receipt.response_digest())
            );
            assert_eq!(stored.source().as_str(), "tmdb.movie");
            assert_eq!(stored.provenance().source_identifier(), Some("42"));
            assert_eq!(stored.locale(), Some("fr-fr"));
            assert_eq!(stored.provenance().region(), None);
        }
        assert_eq!(snapshot.context.region().unwrap().as_str(), "FR");
    }

    #[test]
    fn cached_metadata_same_storage_microsecond_different_digest_preserves_original() {
        let (node, snapshot) = fixture();
        let fields = snapshot.metadata_fields().unwrap();
        let record = create(&node, &snapshot, fields.clone())
            .unwrap()
            .record_id();
        let before = source_rows(&node);
        let original = claims(&node, record, &fields);
        let life = snapshot.receipt.lifetime();
        let shift = Duration::nanoseconds(100);
        let conflict = StoredSearchCandidate {
            context: snapshot.context.clone(),
            receipt: SearchCandidateReceipt::new(
                SearchCandidateReceiptId::new_v7(),
                snapshot.receipt.partition().clone(),
                snapshot.receipt.candidate().clone(),
                Sha256Digest::from_bytes(&[8; 32]),
                SearchReceiptLifetime::try_new(
                    life.created_at() + shift,
                    life.fresh_until() + shift,
                    life.stale_until() + shift,
                    life.expires_at() + shift,
                )
                .unwrap(),
            ),
        };
        assert_ne!(life.created_at(), conflict.receipt.lifetime().created_at());
        assert_eq!(
            timestamp(life.created_at()),
            timestamp(conflict.receipt.lifetime().created_at())
        );
        let error = node
            .kernel
            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                snapshot.receipt.candidate().identifier().clone(),
                conflict.metadata_fields().unwrap(),
            ))
            .unwrap_err();
        assert_eq!(error.code(), ProblemCode::IntegrityFailed);
        assert_eq!(source_rows(&node), before);
        assert_eq!(claims(&node, record, &fields), original);
    }

    #[test]
    fn cached_metadata_later_wrong_namespace_rolls_back_existing_create_owner() {
        let (node, snapshot) = fixture();
        let mut fields = snapshot.metadata_fields().unwrap();
        let original = fields[1].claim();
        fields[1] = ProviderMetadataField::new(
            FieldKey::try_new("core.original_title").unwrap(),
            FieldClaim::try_new(
                NamespaceKey::try_new("wrong.namespace").unwrap(),
                original.value(),
                original.locale().map(str::to_owned),
                fasti_domain::ReceivedAt::from_application_clock(original.fetched_at()),
                original.expires_at(),
            )
            .unwrap(),
        );
        let before = source_rows(&node);
        assert!(create(&node, &snapshot, fields).is_err());
        assert_eq!(source_rows(&node), before);
    }

    #[test]
    fn cached_metadata_later_provenance_failure_rolls_back_existing_create_owner() {
        let (node, snapshot) = fixture();
        let before = source_rows(&node);
        node.kernel.inner.connection.lock().unwrap().execute_batch(
            "CREATE TRIGGER reject_second_cached_metadata_field BEFORE INSERT ON metadata_claim_provenance
             WHEN NEW.field_key = 'core.original_title'
               AND EXISTS (SELECT 1 FROM metadata_claim_provenance WHERE record_id = NEW.record_id AND field_key = 'core.title')
             BEGIN SELECT RAISE(ABORT, 'fixture later provenance failure'); END;",
        ).unwrap();
        assert!(create(&node, &snapshot, snapshot.metadata_fields().unwrap()).is_err());
        assert_eq!(source_rows(&node), before);
    }
}
