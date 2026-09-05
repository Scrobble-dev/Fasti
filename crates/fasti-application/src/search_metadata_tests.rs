mod search_metadata_tests {
    use super::*;
    use crate::{provider_candidate_metadata_fields, ProviderMetadataField};
    use fasti_domain::{
        FieldClaimStatus, ReceivedAt, ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY,
        POSTER_FIELD_KEY, RELEASE_YEAR_FIELD_KEY, TITLE_FIELD_KEY,
    };

    fn snapshot(data: SearchCandidateData, life: SearchReceiptLifetime) -> StoredSearchCandidate {
        let candidate = SearchCandidate::try_new(data).unwrap();
        let locale = (candidate.data().provider == "tmdb")
            .then(|| MetadataLocale::try_new("fr-FR").unwrap());
        let context = SearchProviderQuery::try_new(
            SearchQuery::try_new("original query").unwrap(),
            ProviderId::try_new(&candidate.data().provider).unwrap(),
            1,
            locale,
            Some(MetadataRegion::try_new("FR").unwrap()),
            vec![candidate.identifier().grain()],
        )
        .unwrap()
        .receipt_context();
        let partition = SearchReceiptPartition::try_new(
            AuthorizedApplicationAccess::new(
                WorkspaceId::new_v7(),
                ProfileId::new_v7(),
                ProfileGrantId::new_v7(),
                AuthorizedActor::BrowserSession {
                    auth_subject_id: AuthSubjectId::new_v7(),
                    browser_session_id: BrowserSessionId::new_v7(),
                    grant_owner_client_id: ClientId::new_v7(),
                },
            ),
            context.digest(),
            Sha256Digest::from_bytes(&[1; 32]),
            Sha256Digest::from_bytes(&[2; 32]),
            "fasti.public-metadata-cache.v1".into(),
        )
        .unwrap();
        StoredSearchCandidate {
            receipt: SearchCandidateReceipt::new(
                SearchCandidateReceiptId::new_v7(),
                partition,
                candidate,
                Sha256Digest::from_bytes(&[3; 32]),
                life,
            ),
            context,
        }
    }

    fn all_fields() -> SearchCandidateData {
        let mut data = candidate_data();
        data.original_title = Some("Original title".into());
        data.authors = vec!["Author evidence has no allocated metadata field".into()];
        data
    }

    fn semantics(fields: &[ProviderMetadataField]) -> Vec<(String, serde_json::Value)> {
        fields
            .iter()
            .map(|field| {
                let mut claim = serde_json::to_value(field.claim()).unwrap();
                assert!(claim.as_object_mut().unwrap().remove("claim_id").is_some());
                (field.field_key().as_str().to_owned(), claim)
            })
            .collect()
    }

    #[test]
    fn snapshot_projects_exact_five_fields_in_owner_order_with_original_provenance() {
        let stored = snapshot(all_fields(), lifetime());
        let fields = stored.metadata_fields().unwrap();
        let values: Vec<_> = fields
            .iter()
            .map(|field| (field.field_key().as_str(), field.claim().value()))
            .collect();
        assert_eq!(
            values,
            [
                (TITLE_FIELD_KEY, "A film"),
                (ORIGINAL_TITLE_FIELD_KEY, "Original title"),
                (OVERVIEW_FIELD_KEY, "A description."),
                (POSTER_FIELD_KEY, "https://image.tmdb.org/t/p/w500/film.jpg"),
                (RELEASE_YEAR_FIELD_KEY, "2026"),
            ]
        );
        assert_eq!(stored.context.region().unwrap().as_str(), "FR");
        for field in fields {
            let claim = field.claim();
            assert!(claim.record_id().is_none());
            assert!(claim.field_key().is_none());
            assert_eq!(claim.fetched_at(), stored.receipt.lifetime().created_at());
            assert_eq!(
                claim.expires_at(),
                Some(stored.receipt.lifetime().fresh_until())
            );
            assert_eq!(claim.initial_status(), FieldClaimStatus::Fresh);
            let provenance = claim.provenance();
            assert!(provenance.is_complete());
            assert_eq!(provenance.provider_id().unwrap().as_str(), "tmdb");
            assert_eq!(provenance.source_namespace().as_str(), "tmdb.movie");
            assert_eq!(provenance.source_identifier(), Some("42"));
            assert_eq!(provenance.locale().unwrap().as_str(), "fr-fr");
            assert_eq!(provenance.region(), None);
            assert_eq!(provenance.source_version(), None);
            assert_eq!(
                provenance.evidence_digest(),
                Some(stored.receipt.response_digest())
            );
        }
    }

    #[test]
    fn title_only_snapshot_does_not_invent_optional_fields() {
        let mut data = candidate_data();
        data.original_title = None;
        data.overview = None;
        data.image_url = None;
        data.release_year = None;
        let stored = snapshot(data, lifetime());
        let fields = stored.metadata_fields().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].field_key().as_str(), TITLE_FIELD_KEY);
        assert_eq!(fields[0].claim().value(), "A film");
    }

    #[test]
    fn snapshot_projection_never_renews_freshness_and_repeated_claim_semantics_are_identical() {
        let stored = snapshot(all_fields(), lifetime());
        let life = stored.receipt.lifetime();
        let first = stored.metadata_fields().unwrap();
        for field in &first {
            let claim = field.claim();
            assert_eq!(
                claim.status_at(life.fresh_until() - Duration::nanoseconds(1)),
                FieldClaimStatus::Fresh
            );
            assert_eq!(claim.status_at(life.fresh_until()), FieldClaimStatus::Stale);
            assert_eq!(
                claim.status_at(life.expires_at() + Duration::days(1)),
                FieldClaimStatus::Stale
            );
        }
        // Projection is pure with respect to time. Even long after this fixed
        // historical receipt expired, it must not move its source clock/TTL.
        let repeated = stored.metadata_fields().unwrap();
        assert_eq!(semantics(&first), semantics(&repeated));
        assert!(first
            .iter()
            .zip(&repeated)
            .all(|(left, right)| left.claim().claim_id() != right.claim().claim_id()));
        assert!(!life.receipt_is_current(life.expires_at()));
        // Constructing evidence is intentionally not authorization to save an
        // expired receipt; that remains the atomic action owner's gate.
    }

    #[test]
    fn zero_freshness_historical_snapshot_is_always_stale_without_an_invented_ttl() {
        let created = lifetime().created_at();
        let life = SearchReceiptLifetime::try_new(
            created,
            created,
            created + Duration::seconds(600),
            created + Duration::days(1),
        )
        .unwrap();
        let stored = snapshot(all_fields(), life);
        let first = stored.metadata_fields().unwrap();
        for field in &first {
            let claim = field.claim();
            assert_eq!(claim.fetched_at(), created);
            assert_eq!(claim.expires_at(), None);
            assert_eq!(claim.initial_status(), FieldClaimStatus::Stale);
            for at in [
                created - Duration::seconds(1),
                created,
                created + Duration::days(30),
            ] {
                assert!(!claim.is_fresh(at));
                assert_eq!(claim.status_at(at), FieldClaimStatus::Stale);
            }
        }
        assert_eq!(
            semantics(&first),
            semantics(&stored.metadata_fields().unwrap())
        );
    }

    #[test]
    fn snapshot_provider_grain_locale_page_and_digest_mismatches_fail_closed() {
        let stored = snapshot(all_fields(), lifetime());
        for mutate in [
            (|context: &mut SearchPageContext| context.provider = "google-books".into())
                as fn(&mut SearchPageContext),
            |context: &mut SearchPageContext| context.grains = vec![Grain::Series],
            |context: &mut SearchPageContext| {
                context.locale = Some(MetadataLocale::try_new("de-DE").unwrap())
            },
            |context: &mut SearchPageContext| context.page = 2,
            |context: &mut SearchPageContext| {
                context.query_digest = Sha256Digest::from_bytes(&[8; 32])
            },
            |context: &mut SearchPageContext| context.region = None,
        ] {
            let mut changed = stored.clone();
            mutate(&mut changed.context);
            assert_eq!(
                changed.metadata_fields().unwrap_err(),
                SearchEvidenceError::InvalidPartition
            );
        }
        let mut changed = stored;
        changed.receipt.partition.context_digest = Sha256Digest::from_bytes(&[9; 32]);
        assert_eq!(
            changed.metadata_fields().unwrap_err(),
            SearchEvidenceError::InvalidPartition
        );
    }

    #[test]
    fn provider_source_identifier_and_response_digest_remain_distinct() {
        for (provider, kind, id, namespace) in [
            ("tmdb", "movie", "42", "tmdb.movie"),
            ("tmdb", "show", "42", "tmdb.tv"),
            ("google-books", "book", "volume_42", "googlebooks.volume"),
        ] {
            let mut data = candidate_data();
            data.provider = provider.into();
            data.kind = kind.into();
            data.provider_id = id.into();
            data.image_url = None;
            let stored = snapshot(data, lifetime());
            let first = stored.metadata_fields().unwrap();
            for field in &first {
                let provenance = field.claim().provenance();
                assert_eq!(provenance.provider_id().unwrap().as_str(), provider);
                assert_eq!(provenance.source_namespace().as_str(), namespace);
                assert_eq!(provenance.source_identifier(), Some(id));
                assert_eq!(provenance.locale(), stored.context.locale());
            }
            let mut changed = stored.clone();
            changed.receipt.response_digest = Sha256Digest::from_bytes(&[9; 32]);
            let second = changed.metadata_fields().unwrap();
            assert_ne!(semantics(&first), semantics(&second));
            for field in second {
                assert_eq!(
                    field.claim().provenance().evidence_digest(),
                    Some(changed.receipt.response_digest())
                );
                assert_eq!(
                    field.claim().fetched_at(),
                    stored.receipt.lifetime().created_at()
                );
            }
        }
    }

    #[test]
    fn shared_constructor_preserves_explicit_context_and_rejects_invalid_expiry() {
        let candidate = SearchCandidate::try_new(all_fields()).unwrap();
        let at = lifetime().created_at();
        let digest = Sha256Digest::from_bytes(&[7; 32]);
        let locale = MetadataLocale::try_new("de-DE").unwrap();
        let region = MetadataRegion::try_new("DE").unwrap();
        let fields = provider_candidate_metadata_fields(
            &candidate,
            Some(locale.clone()),
            Some(region.clone()),
            &digest,
            ReceivedAt::from_application_clock(at),
            Some(at + Duration::days(1)),
            FieldClaimStatus::Fresh,
        )
        .unwrap();
        assert_eq!(fields.len(), 5);
        for field in fields {
            let claim = field.claim();
            assert_eq!(claim.fetched_at(), at);
            assert_eq!(claim.expires_at(), Some(at + Duration::days(1)));
            assert_eq!(claim.provenance().locale(), Some(&locale));
            assert_eq!(claim.provenance().region(), Some(&region));
            assert_eq!(claim.provenance().evidence_digest(), Some(&digest));
        }
        for expiry in [at - Duration::nanoseconds(1), at] {
            for status in [FieldClaimStatus::Fresh, FieldClaimStatus::Stale] {
                assert_eq!(
                    provider_candidate_metadata_fields(
                        &candidate,
                        None,
                        None,
                        &digest,
                        ReceivedAt::from_application_clock(at),
                        Some(expiry),
                        status,
                    )
                    .unwrap_err(),
                    SearchEvidenceError::InvalidCandidate
                );
            }
        }
        // Even an internal corrupted value must still reach the domain field
        // constructor rather than bypassing its independent value bound.
        let mut corrupted = candidate;
        corrupted.data.overview = Some("x".repeat(fasti_domain::MAX_FIELD_VALUE_BYTES + 1));
        assert_eq!(
            provider_candidate_metadata_fields(
                &corrupted,
                None,
                None,
                &digest,
                ReceivedAt::from_application_clock(at),
                Some(at + Duration::days(1)),
                FieldClaimStatus::Fresh,
            )
            .unwrap_err(),
            SearchEvidenceError::InvalidCandidate
        );
    }
}
