mod candidate_details_tests {
    use super::*;

    fn read_state(
        version: u64,
        status: ProviderCapabilityStatus,
        configuration: &str,
    ) -> ProviderCapabilityState {
        let search = state(version);
        ProviderCapabilityState::try_new(
            search.provider_id().clone(),
            ProviderCapabilityId::try_new("metadata.read").unwrap(),
            status,
            version,
            search.credential_requirement(),
            search.credential_reference().cloned(),
            search.credential_status(),
            ConfigurationDigest::parse(configuration.repeat(64)).unwrap(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .unwrap()
    }

    fn assert_no_record_writes(node: &TestNode) {
        let counts: (i64, i64, i64) = node
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT (SELECT COUNT(*) FROM records), \
                        (SELECT COUNT(*) FROM external_identifiers), \
                        (SELECT COUNT(*) FROM metadata_field_claims)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts, (0, 0, 0));
    }

    #[test]
    fn missing_or_disabled_read_capability_blocks_fetch_preparation_not_cached_details() {
        let (node, request) = setup();
        let saved = commit(&node, &request, &[candidate("42")]);
        let read = details(&request, saved.candidates[0].id());
        let original = node.kernel.read_search_candidate(&read).unwrap().unwrap();
        let search_partition = node.kernel.prepare_search_page(&request).unwrap().partition;

        assert_eq!(
            node.kernel
                .prepare_search_candidate_details(&read)
                .unwrap_err()
                .code(),
            ProblemCode::CapabilityUnavailable
        );
        assert_eq!(
            node.kernel.read_search_candidate(&read).unwrap(),
            Some(original.clone())
        );

        for (version, status) in [
            (1, ProviderCapabilityStatus::Disabled),
            (2, ProviderCapabilityStatus::Unavailable),
        ] {
            node.kernel
                .put_provider_capability_state(
                    node.access.workspace_id(),
                    read_state(version, status, "a"),
                )
                .unwrap();
            assert_eq!(
                node.kernel
                    .prepare_search_candidate_details(&read)
                    .unwrap_err()
                    .code(),
                ProblemCode::CapabilityUnavailable
            );
            assert_eq!(
                node.kernel.read_search_candidate(&read).unwrap(),
                Some(original.clone())
            );
            assert_eq!(
                node.kernel.prepare_search_page(&request).unwrap().partition,
                search_partition
            );
        }
        assert_no_record_writes(&node);
    }

    #[test]
    fn enabled_read_preparation_preserves_receipt_context_and_never_creates_a_record() {
        let (node, mut request) = setup();
        request.query = SearchProviderQuery::try_new(
            SearchQuery::try_new("Private original query").unwrap(),
            ProviderId::try_new("tmdb").unwrap(),
            3,
            Some(fasti_domain::MetadataLocale::try_new("fr-FR").unwrap()),
            Some(fasti_domain::MetadataRegion::try_new("FR").unwrap()),
            vec![fasti_domain::Grain::Film],
        )
        .unwrap();
        let search = node.kernel.prepare_search_page(&request).unwrap();
        let saved = node
            .kernel
.commit_search_page(
                &request,
                &search,
                &[candidate("42")],
                &Sha256Digest::from_bytes(&[7; 32]),
                Some(4),
                &crate::search::tests::response_policy(),
            )
            .unwrap();
        let read = details(&request, saved.candidates[0].id());
        let original = node.kernel.read_search_candidate(&read).unwrap().unwrap();
        let partition = search.partition;
        let mut authority = None;

        for (version, status) in [
            (1, ProviderCapabilityStatus::Available),
            (2, ProviderCapabilityStatus::Degraded),
        ] {
            let current = read_state(version, status, "a");
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), current.clone())
                .unwrap();
            let prepared = node
                .kernel
                .prepare_search_candidate_details(&read)
                .unwrap()
                .unwrap();
            assert_eq!(prepared.provider_state, current);
            if let Some(previous) =
                authority.replace(prepared.provider_authority_fingerprint.clone())
            {
                assert_eq!(previous, prepared.provider_authority_fingerprint);
            }
            assert_eq!(prepared.candidate, original);
            assert_eq!(prepared.candidate.context, request.query.receipt_context());
            assert_eq!(prepared.candidate.receipt, saved.candidates[0]);
            assert_eq!(
                node.kernel.prepare_search_page(&request).unwrap().partition,
                partition
            );
            assert_eq!(
                node.kernel.read_search_candidate(&read).unwrap(),
                Some(original.clone())
            );
        }
        assert_no_record_writes(&node);
    }

    #[test]
    fn read_configuration_away_and_back_changes_authority_not_search_partition() {
        let (node, request) = setup();
        let saved = commit(&node, &request, &[candidate("42")]);
        let read = details(&request, saved.candidates[0].id());
        let partition = node.kernel.prepare_search_page(&request).unwrap().partition;
        let mut snapshots = Vec::new();
        for (version, configuration) in [(1, "a"), (2, "b"), (3, "a")] {
            node.kernel
                .put_provider_capability_state(
                    node.access.workspace_id(),
                    read_state(version, ProviderCapabilityStatus::Available, configuration),
                )
                .unwrap();
            snapshots.push(
                node.kernel
                    .prepare_search_candidate_details(&read)
                    .unwrap()
                    .unwrap(),
            );
            assert_eq!(
                node.kernel.prepare_search_page(&request).unwrap().partition,
                partition
            );
        }
        assert_eq!(
            snapshots[0].provider_state.configuration_digest(),
            snapshots[2].provider_state.configuration_digest()
        );
        for snapshot in &snapshots[1..] {
            assert_eq!(snapshot.candidate, snapshots[0].candidate);
            assert_ne!(
                snapshot.provider_authority_fingerprint,
                snapshots[0].provider_authority_fingerprint
            );
        }
        assert_ne!(
            snapshots[1].provider_authority_fingerprint,
            snapshots[2].provider_authority_fingerprint
        );
        let authority_version: i64 = node
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT authority_version FROM provider_capability_states \
                 WHERE workspace_id = ?1 AND provider_id = 'tmdb' \
                   AND capability_id = 'metadata.read'",
                [node.access.workspace_id().to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(authority_version, 3);
        assert_no_record_writes(&node);
    }

    #[test]
    fn detail_preparation_rechecks_receipt_profile_and_current_search_scope() {
        let (node, request) = setup();
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                read_state(1, ProviderCapabilityStatus::Available, "a"),
            )
            .unwrap();
        let saved = commit(&node, &request, &[candidate("42")]);
        let read = details(&request, saved.candidates[0].id());
        let mut other_profile = read.clone();
        other_profile.access = node
            .add_profile_with_scopes(&[fasti_application::ScopeKey::MetadataSearch])
            .into();
        assert!(node
            .kernel
            .prepare_search_candidate_details(&other_profile)
            .unwrap()
            .is_none());
        assert!(node
            .kernel
            .prepare_search_candidate_details(&read)
            .unwrap()
            .is_some());
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM grant_scopes WHERE grant_id = ?1 AND scope_key = 'metadata_search'",
                [node.access.grant_id().to_string()],
            )
            .unwrap();
        assert_eq!(
            node.kernel
                .prepare_search_candidate_details(&read)
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(
            node.kernel.read_search_candidate(&read).unwrap_err().code(),
            ProblemCode::Forbidden
        );
        assert_no_record_writes(&node);
    }

    #[test]
    fn expired_or_mismatched_receipts_cannot_prepare_online_details() {
        let (node, request) = setup();
        let saved = commit(&node, &request, &[candidate("42")]);
        let read = details(&request, saved.candidates[0].id());
        // The absent metadata.read capability must not replace an invalid
        // receipt's absent result with a provider-state error.
        for mutation in 0..3 {
            let mut changed = read.clone();
            match mutation {
                0 => changed.provider = ProviderId::try_new("google-books").unwrap(),
                1 => changed.grain = fasti_domain::Grain::Series,
                _ => changed.terms_revision = "tmdb-v2".into(),
            }
            assert!(node
                .kernel
                .prepare_search_candidate_details(&changed)
                .unwrap()
                .is_none());
        }
        age_page(&node, saved.sequence, 86_401);
        assert!(node
            .kernel
            .prepare_search_candidate_details(&read)
            .unwrap()
            .is_none());
        assert!(node.kernel.read_search_candidate(&read).unwrap().is_none());
        assert_no_record_writes(&node);
    }
}
