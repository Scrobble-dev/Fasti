mod search_candidate_policy_tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use fasti_application::{ProviderResponseCachePolicy, ProviderResponseReuse};
    use fasti_domain::FieldClaimStatus;
    use std::time::Duration as StdDuration;

    fn observed_policy(
        reuse: ProviderResponseReuse,
        observed: DateTime<Utc>,
        freshness: Option<StdDuration>,
    ) -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(reuse, observed, StdDuration::ZERO, freshness, None)
    }

    fn seed(
        node: &TestNode,
        request: &SearchPageRequest,
        policy: &ProviderResponseCachePolicy,
    ) -> SearchCandidateActionCommand {
        let prepared = node.kernel.prepare_search_page(request).unwrap();
        let saved = node
            .kernel
            .commit_search_page(
                request,
                &prepared,
                &[candidate("42")],
                &Sha256Digest::from_bytes(&[7; 32]),
                None,
                policy,
            )
            .unwrap();
        SearchCandidateActionCommand {
            request: details(request, saved.candidates[0].id()),
            operation_id: OperationId::new_v7(),
            action: SearchRecordAction::Create,
            evidence_mode: SearchCandidateEvidenceMode::Cached,
        }
    }

    fn enable_details(node: &TestNode) {
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                state_for("metadata.read", 1),
            )
            .unwrap();
    }

    // Cross the actual persisted deadline, not an arbitrary delay and not a
    // rewritten SQLite snapshot. A missing commit-time policy check must fail.
    fn cross_short_deadline(deadline: DateTime<Utc>) {
        let remaining = (deadline - now()).to_std().unwrap_or(StdDuration::ZERO);
        assert!(remaining <= StdDuration::from_secs(2));
        std::thread::sleep(remaining + StdDuration::from_millis(5));
        assert!(now() >= deadline);
    }

    #[test]
    fn candidate_policy_reusable_save_retention_outlives_page_fallback_without_freshness_renewal() {
        for age in [601, 86_390] {
            let (node, request) = setup();
            let policy = observed_policy(
                ProviderResponseReuse::Reusable,
                now() - Duration::seconds(age),
                None,
            );
            let command = seed(&node, &request, &policy);
            assert!(node
                .kernel
                .read_cached_search_page(&request, true)
                .unwrap()
                .is_none());
            let snapshot = node
                .kernel
                .read_search_candidate(&command.request)
                .unwrap()
                .unwrap();
            assert!(snapshot.payload_is_reusable(now()));
            assert_eq!(snapshot.response_policy, policy);
            let fields = snapshot.metadata_fields().unwrap();
            assert_eq!(fields.len(), 1);
            let claim = fields[0].claim();
            assert_eq!(claim.fetched_at(), snapshot.receipt.lifetime().created_at());
            assert_eq!(
                claim.expires_at(),
                Some(snapshot.receipt.lifetime().fresh_until())
            );
            assert_eq!(claim.initial_status(), FieldClaimStatus::Fresh);
            assert!(!claim.is_fresh(now()));
            let unrelated = rows(&node, UNRELATED_TABLES);
            let saved = act(&node, &command).unwrap();
            assert_eq!(saved.fetched_at, snapshot.receipt.lifetime().created_at());
            assert_eq!(
                saved.expires_at,
                Some(snapshot.receipt.lifetime().fresh_until())
            );
            assert_eq!(
                saved.search_response_digest,
                *snapshot.receipt.response_digest()
            );
            assert_eq!(saved.initial_status, FieldClaimStatus::Fresh);
            assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
            assert_eq!(act(&node, &command).unwrap(), saved);
        }
    }

    #[test]
    fn candidate_policy_reusable_zero_freshness_remains_explicit_stale_save_evidence() {
        for freshness in [StdDuration::ZERO, StdDuration::from_nanos(1)] {
        let (node, request) = setup();
        let command = seed(
            &node,
            &request,
            &observed_policy(
                ProviderResponseReuse::Reusable,
                chrono::DateTime::from_timestamp(now().timestamp() - 601, 0).unwrap(),
                Some(freshness),
            ),
        );
        let snapshot = node
            .kernel
            .read_search_candidate(&command.request)
            .unwrap()
            .unwrap();
        assert!(snapshot.payload_is_reusable(now()));
        let fields = snapshot.metadata_fields().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].claim().initial_status(), FieldClaimStatus::Stale);
        assert_eq!(fields[0].claim().expires_at(), None);
        let saved = act(&node, &command).unwrap();
        assert_eq!(saved.initial_status, FieldClaimStatus::Stale);
        assert_eq!(saved.expires_at, None);
        assert_eq!(saved.fetched_at, snapshot.receipt.lifetime().created_at());
        assert_eq!(
            saved.search_response_digest,
            *snapshot.receipt.response_digest()
        );
        }
    }

    #[test]
    fn candidate_policy_no_cache_denies_payload_and_cached_actions_but_preserves_refetch_coordinates(
    ) {
        let (node, request) = setup();
        enable_details(&node);
        let command = seed(
            &node,
            &request,
            &observed_policy(ProviderResponseReuse::ValidateEveryReuse, now(), None),
        );
        let before = rows(&node, MUTATION_TABLES);
        let unrelated = rows(&node, UNRELATED_TABLES);
        assert!(node
            .kernel
            .read_search_candidate(&command.request)
            .unwrap()
            .is_none());
        let coordinates = node
            .kernel
            .prepare_search_candidate_details(&command.request)
            .unwrap()
            .unwrap();
        assert!(!coordinates.candidate.payload_is_reusable(now()));
        // Historical projection does not grant permission to save the payload.
        let historical = coordinates.candidate.metadata_fields().unwrap();
        assert_eq!(
            historical[0].claim().initial_status(),
            FieldClaimStatus::Stale
        );
        assert_eq!(historical[0].claim().expires_at(), None);
        assert_eq!(
            coordinates.candidate.receipt.candidate().identifier(),
            candidate("42").identifier()
        );
        assert!(node
            .kernel
            .prepare_search_candidate_action(&command)
            .is_err());
        assert!(node
            .kernel
            .commit_search_candidate_action(
                &command,
                &SearchCandidateActionPreparation::Cached(coordinates.candidate.clone()),
                None,
            )
            .is_err());
        let mut refetch = command;
        refetch.evidence_mode = SearchCandidateEvidenceMode::Refetch;
        assert!(matches!(
            node.kernel
                .prepare_search_candidate_action(&refetch)
                .unwrap(),
            SearchCandidateActionPreparation::Refetch(_)
        ));
        assert_eq!(rows(&node, MUTATION_TABLES), before);
        assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
    }

    #[test]
    fn candidate_policy_expired_must_revalidate_denies_payload_and_cached_action_without_writes() {
        let (node, request) = setup();
        enable_details(&node);
        let command = seed(
            &node,
            &request,
            &observed_policy(
                ProviderResponseReuse::ValidateWhenStale,
                now() - Duration::seconds(5),
                Some(StdDuration::from_secs(1)),
            ),
        );
        let before = rows(&node, MUTATION_TABLES);
        assert!(node
            .kernel
            .read_search_candidate(&command.request)
            .unwrap()
            .is_none());
        let coordinates = node
            .kernel
            .prepare_search_candidate_details(&command.request)
            .unwrap()
            .unwrap();
        assert!(!coordinates.candidate.payload_is_reusable(now()));
        let historical = coordinates.candidate.metadata_fields().unwrap();
        assert_eq!(
            historical[0].claim().initial_status(),
            FieldClaimStatus::Fresh
        );
        assert!(!historical[0].claim().is_fresh(now()));
        assert!(node
            .kernel
            .prepare_search_candidate_action(&command)
            .is_err());
        assert!(node
            .kernel
            .commit_search_candidate_action(
                &command,
                &SearchCandidateActionPreparation::Cached(coordinates.candidate),
                None,
            )
            .is_err());
        assert_eq!(rows(&node, MUTATION_TABLES), before);
    }

    #[test]
    fn candidate_policy_expiry_rechecks_unchanged_preparation_but_never_invalidates_completed_replay(
    ) {
        let (node, request) = setup();
        let completed = seed(
            &node,
            &request,
            &observed_policy(
                ProviderResponseReuse::ValidateWhenStale,
                now(),
                Some(StdDuration::from_secs(2)),
            ),
        );
        let prepared = node
            .kernel
            .prepare_search_candidate_action(&completed)
            .unwrap();
        let SearchCandidateActionPreparation::Cached(snapshot) = &prepared else {
            panic!("fresh must-revalidate evidence permits explicit cached Save");
        };
        let deadline = snapshot.receipt.lifetime().fresh_until();
        assert!(snapshot.payload_is_reusable(deadline - Duration::microseconds(1)));
        assert!(!snapshot.payload_is_reusable(deadline));
        assert!(snapshot.metadata_fields().is_ok());
        let original = node
            .kernel
            .commit_search_candidate_action(&completed, &prepared, None)
            .unwrap();
        let mut pending = completed.clone();
        pending.operation_id = OperationId::new_v7();
        let pending_prepared = node
            .kernel
            .prepare_search_candidate_action(&pending)
            .unwrap();
        assert_eq!(pending_prepared, prepared);
        let page_rows = rows(&node, &["search_pages", "search_candidate_receipts"]);
        let durable = rows(&node, MUTATION_TABLES);
        let unrelated = rows(&node, UNRELATED_TABLES);

        cross_short_deadline(deadline);
        // Expiry changes permission at the transaction boundary, not the
        // original projection's timestamps or initial historical status.
        let historical = snapshot.metadata_fields().unwrap();
        assert_eq!(historical[0].claim().expires_at(), Some(deadline));
        assert_eq!(
            historical[0].claim().initial_status(),
            FieldClaimStatus::Fresh
        );
        assert!(!historical[0].claim().is_fresh(now()));
        assert!(node
            .kernel
            .prepare_search_candidate_action(&pending)
            .is_err());
        assert!(node
            .kernel
            .commit_search_candidate_action(&pending, &pending_prepared, None)
            .is_err());
        assert_eq!(rows(&node, MUTATION_TABLES), durable);
        assert_eq!(
            rows(&node, &["search_pages", "search_candidate_receipts"]),
            page_rows
        );

        remove_scope(&node, "metadata_search");
        assert_eq!(
            node.kernel
                .prepare_search_candidate_action(&completed)
                .unwrap(),
            SearchCandidateActionPreparation::Replay(Box::new(original.clone()))
        );
        assert_eq!(
            node.kernel
                .commit_search_candidate_action(&completed, &prepared, None)
                .unwrap(),
            original
        );
        assert_eq!(act(&node, &completed).unwrap(), original);
        assert_eq!(rows(&node, MUTATION_TABLES), durable);
        assert_eq!(rows(&node, UNRELATED_TABLES), unrelated);
        assert_eq!(
            rows(&node, &["search_pages", "search_candidate_receipts"]),
            page_rows
        );
    }
}
