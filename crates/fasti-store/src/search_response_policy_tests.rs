mod search_response_policy_tests {
    use super::*;
    use chrono::{DateTime, Timelike, Utc};
    use fasti_application::{
        AccessAdministrationPort, ProviderResponseCachePolicy, ProviderResponseReuse,
        RevokeCredentialCommand, ScopeKey, SearchCacheState,
    };
    use std::time::Duration as StdDuration;

    fn policy(
        reuse: ProviderResponseReuse,
        observed: DateTime<Utc>,
    ) -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(reuse, observed, StdDuration::ZERO, None, None)
    }

    fn commit_policy(
        node: &TestNode,
        request: &SearchPageRequest,
        policy: &ProviderResponseCachePolicy,
    ) -> ApplicationResult<StoredSearchPage> {
        let prepared = node.kernel.prepare_search_page(request)?;
        node.kernel.commit_search_page(
            request,
            &prepared,
            &[candidate("42")],
            &Sha256Digest::from_bytes(&[7; 32]),
            Some(2),
            policy,
        )
    }

    fn canonical(value: DateTime<Utc>) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(&timestamp(value))
            .unwrap()
            .with_timezone(&Utc)
    }

    fn source_counts(node: &TestNode) -> (i64, i64, i64, i64) {
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT (SELECT COUNT(*) FROM search_pages),
                    (SELECT COUNT(*) FROM search_candidate_receipts),
                    (SELECT COUNT(*) FROM records),
                    (SELECT COUNT(*) FROM metadata_field_claims)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap()
    }

    fn replace_context(node: &TestNode, sequence: u64, context: &str) {
        let connection = node.kernel.inner.connection.lock().unwrap();
        connection
            .execute_batch("DROP TRIGGER search_pages_immutable_update")
            .unwrap();
        assert_eq!(
            connection
                .execute(
                    "UPDATE search_pages SET context_json = ?1 WHERE sequence = ?2",
                    params![context, i64::try_from(sequence).unwrap()],
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn response_policy_original_observation_and_deadlines_survive_restart_without_renewal() {
        let (node, request) = setup();
        let observed = (now() - Duration::seconds(5))
            .with_nanosecond(123_456_789)
            .unwrap();
        let response_policy = policy(ProviderResponseReuse::Reusable, observed);
        let saved = commit_policy(&node, &request, &response_policy).unwrap();
        assert_eq!(saved.lifetime.created_at(), canonical(observed));
        assert_eq!(
            saved.lifetime.fresh_until(),
            canonical(observed + Duration::seconds(120))
        );
        assert_eq!(
            saved.lifetime.stale_until(),
            canonical(observed + Duration::seconds(600))
        );
        assert_eq!(
            saved.lifetime.expires_at(),
            canonical(observed + Duration::hours(24))
        );
        let context_json: String = node
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT context_json FROM search_pages WHERE sequence = ?1",
                [i64::try_from(saved.sequence).unwrap()],
                |row| row.get(0),
            )
            .unwrap();
        let (context, persisted_policy) =
            SearchPageContext::from_response_json(&context_json).unwrap();
        assert_eq!(context, request.query.receipt_context());
        assert_eq!(persisted_policy, Some(response_policy));
        assert_eq!(context.digest(), request.query.receipt_context().digest());

        let (root, _) = node.into_stopped();
        let reopened = SqliteKernel::open(root.path()).unwrap();
        for _ in 0..2 {
            let cached = reopened
                .read_cached_search_page(&request, false)
                .unwrap()
                .unwrap();
            assert_eq!(cached.sequence, saved.sequence);
            assert_eq!(cached.lifetime, saved.lifetime);
            assert_eq!(cached.response_digest, saved.response_digest);
            assert_eq!(cached.candidates[0].id(), saved.candidates[0].id());
            assert_eq!(cached.cache_state, SearchCacheState::Fresh);
        }
    }

    #[test]
    fn response_policy_delayed_body_commit_does_not_restart_freshness_or_stale_caps() {
        let (node, request) = setup();
        let observed = now() - Duration::seconds(180);
        let saved = commit_policy(
            &node,
            &request,
            &policy(ProviderResponseReuse::Reusable, observed),
        )
        .unwrap();
        assert_eq!(saved.cache_state, SearchCacheState::Observed);
        assert!(node
            .kernel
            .read_cached_search_page(&request, false)
            .unwrap()
            .is_none());
        let stale = node
            .kernel
            .read_cached_search_page(&request, true)
            .unwrap()
            .unwrap();
        assert_eq!(stale.cache_state, SearchCacheState::StaleOnError);
        assert_eq!(stale.lifetime, saved.lifetime);

        let expired_policy = policy(
            ProviderResponseReuse::Reusable,
            now() - Duration::seconds(601),
        );
        commit_policy(&node, &request, &expired_policy).unwrap();
        assert!(node
            .kernel
            .read_cached_search_page(&request, true)
            .unwrap()
            .is_none());
    }

    #[test]
    fn response_policy_no_store_rejects_without_replacing_or_writing_payloads() {
        let (node, request) = setup();
        let saved = commit_policy(
            &node,
            &request,
            &policy(ProviderResponseReuse::Reusable, now()),
        )
        .unwrap();
        let before = source_counts(&node);
        let rejected = commit_policy(
            &node,
            &request,
            &policy(ProviderResponseReuse::NoStore, now()),
        )
        .unwrap_err();
        assert_eq!(rejected.code(), ProblemCode::ValidationFailed);
        assert_eq!(source_counts(&node), before);
        let cached = node
            .kernel
            .read_cached_search_page(&request, false)
            .unwrap()
            .unwrap();
        assert_eq!(cached.sequence, saved.sequence);
        assert_eq!(cached.candidates[0].id(), saved.candidates[0].id());
        assert_eq!(cached.lifetime, saved.lifetime);
    }

    #[test]
    fn response_policy_no_store_purge_removes_only_current_partition_without_new_payloads() {
        let (node, request) = setup();
        let reusable = policy(ProviderResponseReuse::Reusable, now());
        let first = commit_policy(&node, &request, &reusable).unwrap();
        let second = commit_policy(&node, &request, &reusable).unwrap();
        let mut other_query = request.clone();
        other_query.query = SearchProviderQuery::try_new(
            SearchQuery::try_new("Different title").unwrap(),
            request.query.provider().clone(),
            1,
            None,
            None,
            vec![],
        )
        .unwrap();
        let query_page = commit_policy(&node, &other_query, &reusable).unwrap();
        let mut other_profile = request.clone();
        other_profile.access = node
            .add_profile_with_scopes(&[ScopeKey::MetadataSearch])
            .into();
        let profile_page = commit_policy(&node, &other_profile, &reusable).unwrap();
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        let before = source_counts(&node);
        assert_eq!(before.0, 4);
        assert_eq!(before.1, 4);
        assert_eq!(
            commit_policy(
                &node,
                &request,
                &policy(ProviderResponseReuse::NoStore, now()),
            )
            .unwrap_err()
            .code(),
            ProblemCode::ValidationFailed
        );
        assert_eq!(source_counts(&node), before);
        node.kernel
            .discard_cached_search_page(&request, &prepared)
            .unwrap();
        assert_eq!(source_counts(&node), (2, 2, before.2, before.3));
        for upstream_unavailable in [false, true] {
            assert!(node
                .kernel
                .read_cached_search_page(&request, upstream_unavailable)
                .unwrap()
                .is_none());
        }
        for receipt in [first.candidates[0].id(), second.candidates[0].id()] {
            assert!(node
                .kernel
                .read_search_candidate(&details(&request, receipt))
                .unwrap()
                .is_none());
        }
        for (other_request, saved) in [(&other_query, query_page), (&other_profile, profile_page)] {
            let retained = node
                .kernel
                .read_cached_search_page(other_request, false)
                .unwrap()
                .unwrap();
            assert_eq!(retained.sequence, saved.sequence);
            assert_eq!(retained.candidates[0].id(), saved.candidates[0].id());
            assert_eq!(retained.lifetime, saved.lifetime);
        }
        node.kernel
            .discard_cached_search_page(&request, &prepared)
            .unwrap();
        assert_eq!(source_counts(&node), (2, 2, before.2, before.3));
    }

    #[test]
    fn response_policy_purge_rechecks_revocation_and_prepared_partition_before_deletion() {
        for revoke in [false, true] {
            let (node, request) = setup();
            let reusable = policy(ProviderResponseReuse::Reusable, now());
            commit_policy(&node, &request, &reusable).unwrap();
            let mut preparation_request = request.clone();
            if !revoke {
                preparation_request.access = node
                    .add_profile_with_scopes(&[ScopeKey::MetadataSearch])
                    .into();
                commit_policy(&node, &preparation_request, &reusable).unwrap();
            }
            let prepared = node
                .kernel
                .prepare_search_page(&preparation_request)
                .unwrap();
            let before = source_counts(&node);
            if revoke {
                node.kernel
                    .revoke_credential(RevokeCredentialCommand::new(
                        RequestCorrelationId::new_v7(),
                        node.access,
                        node.access.credential_id(),
                    ))
                    .unwrap();
            }
            assert_eq!(
                node.kernel
                    .discard_cached_search_page(&request, &prepared)
                    .unwrap_err()
                    .code(),
                ProblemCode::Forbidden
            );
            assert_eq!(source_counts(&node), before);
        }
    }

    #[test]
    fn response_policy_no_cache_is_observed_but_never_reused_even_on_error() {
        let (node, request) = setup();
        let saved = commit_policy(
            &node,
            &request,
            &policy(ProviderResponseReuse::ValidateEveryReuse, now()),
        )
        .unwrap();
        assert_eq!(saved.cache_state, SearchCacheState::Observed);
        assert_eq!(saved.lifetime.created_at(), saved.lifetime.fresh_until());
        assert_eq!(saved.lifetime.created_at(), saved.lifetime.stale_until());
        for upstream_unavailable in [false, true] {
            assert!(node
                .kernel
                .read_cached_search_page(&request, upstream_unavailable)
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn response_policy_must_revalidate_allows_fresh_but_never_stale_error_reuse() {
        let (node, request) = setup();
        let fresh = commit_policy(
            &node,
            &request,
            &policy(ProviderResponseReuse::ValidateWhenStale, now()),
        )
        .unwrap();
        assert_eq!(fresh.lifetime.fresh_until(), fresh.lifetime.stale_until());
        assert!(node
            .kernel
            .read_cached_search_page(&request, false)
            .unwrap()
            .is_some());
        commit_policy(
            &node,
            &request,
            &policy(
                ProviderResponseReuse::ValidateWhenStale,
                now() - Duration::seconds(180),
            ),
        )
        .unwrap();
        for upstream_unavailable in [false, true] {
            assert!(node
                .kernel
                .read_cached_search_page(&request, upstream_unavailable)
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn response_policy_source_age_and_short_error_grace_shorten_both_deadlines() {
        let (node, request) = setup();
        let observed = now() - Duration::seconds(80);
        let response_policy = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::Reusable,
            observed,
            StdDuration::from_secs(100),
            Some(StdDuration::from_secs(150)),
            Some(StdDuration::from_secs(20)),
        );
        let saved = commit_policy(&node, &request, &response_policy).unwrap();
        assert_eq!(
            saved.lifetime.fresh_until(),
            canonical(observed + Duration::seconds(50))
        );
        assert_eq!(
            saved.lifetime.stale_until(),
            canonical(observed + Duration::seconds(70))
        );
        for upstream_unavailable in [false, true] {
            assert!(node
                .kernel
                .read_cached_search_page(&request, upstream_unavailable)
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn response_policy_submicrosecond_deadlines_floor_after_original_observation_arithmetic() {
        let (node, request) = setup();
        let observed = (now() - Duration::seconds(5))
            .with_nanosecond(123_456_789)
            .unwrap();
        for freshness_nanos in [0, 100, 500] {
            let response_policy = ProviderResponseCachePolicy::new(
                ProviderResponseReuse::ValidateWhenStale,
                observed,
                StdDuration::ZERO,
                Some(StdDuration::from_nanos(freshness_nanos)),
                None,
            );
            let saved = commit_policy(&node, &request, &response_policy).unwrap();
            let expected = canonical(observed + Duration::nanoseconds(freshness_nanos as i64));
            assert_eq!(saved.lifetime.created_at(), canonical(observed));
            assert_eq!(saved.lifetime.fresh_until(), expected);
            assert_eq!(saved.lifetime.stale_until(), expected);
            assert_eq!(saved.cache_state, SearchCacheState::Observed);
            for upstream_unavailable in [false, true] {
                assert!(node
                    .kernel
                    .read_cached_search_page(&request, upstream_unavailable)
                    .unwrap()
                    .is_none());
            }
        }
    }

    #[test]
    fn response_policy_newest_restrictive_snapshot_never_falls_back_to_older_fresh_page() {
        let (node, request) = setup();
        let old = commit_policy(
            &node,
            &request,
            &policy(ProviderResponseReuse::Reusable, now()),
        )
        .unwrap();
        let restrictive = commit_policy(
            &node,
            &request,
            &policy(ProviderResponseReuse::ValidateEveryReuse, now()),
        )
        .unwrap();
        assert!(restrictive.sequence > old.sequence);
        assert_eq!(source_counts(&node).0, 2);
        assert!(old.lifetime.cache_state(now(), false).is_some());
        for upstream_unavailable in [false, true] {
            assert!(node
                .kernel
                .read_cached_search_page(&request, upstream_unavailable)
                .unwrap()
                .is_none());
        }
    }

    #[test]
    fn response_policy_legacy_missing_policy_is_a_miss_without_older_fallback() {
        let (node, request) = setup();
        let response_policy = policy(ProviderResponseReuse::Reusable, now());
        commit_policy(&node, &request, &response_policy).unwrap();
        let legacy = commit_policy(&node, &request, &response_policy).unwrap();
        replace_context(
            &node,
            legacy.sequence,
            &request.query.receipt_context().to_json().unwrap(),
        );
        let before = source_counts(&node);
        for upstream_unavailable in [false, true] {
            assert!(node
                .kernel
                .read_cached_search_page(&request, upstream_unavailable)
                .unwrap()
                .is_none());
        }
        assert_eq!(source_counts(&node), before);
    }

    #[test]
    fn response_policy_corrupt_policy_or_valid_but_inconsistent_lifetime_fails_closed() {
        for corruption in [
            "unknown_policy_field",
            "lifetime",
            "policy_deadline",
            "no_store",
        ] {
            let (node, request) = setup();
            let response_policy = policy(ProviderResponseReuse::Reusable, now());
            let saved = commit_policy(&node, &request, &response_policy).unwrap();
            if corruption == "lifetime" {
                let connection = node.kernel.inner.connection.lock().unwrap();
                connection
                    .execute_batch("DROP TRIGGER search_pages_immutable_update")
                    .unwrap();
                connection
                    .execute(
                        "UPDATE search_pages SET fresh_until = ?1 WHERE sequence = ?2",
                        params![
                            timestamp(saved.lifetime.fresh_until() - Duration::seconds(1)),
                            i64::try_from(saved.sequence).unwrap()
                        ],
                    )
                    .unwrap();
            } else {
                let context = request.query.receipt_context();
                let encoded = match corruption {
                    "unknown_policy_field" => {
                        context.to_response_json(&response_policy).unwrap().replace(
                            "\"response_policy\":{",
                            "\"response_policy\":{\"untrusted\":true,",
                        )
                    }
                    "policy_deadline" => context
                        .to_response_json(&ProviderResponseCachePolicy::new(
                            ProviderResponseReuse::Reusable,
                            response_policy.received_at(),
                            StdDuration::ZERO,
                            Some(StdDuration::from_secs(30)),
                            None,
                        ))
                        .unwrap(),
                    "no_store" => context
                        .to_response_json(&policy(
                            ProviderResponseReuse::NoStore,
                            response_policy.received_at(),
                        ))
                        .unwrap(),
                    _ => unreachable!(),
                };
                replace_context(&node, saved.sequence, &encoded);
            }
            let before = source_counts(&node);
            for upstream_unavailable in [false, true] {
                let rejected = node
                    .kernel
                    .read_cached_search_page(&request, upstream_unavailable)
                    .unwrap_err();
                assert_eq!(
                    rejected.code(),
                    ProblemCode::IntegrityFailed,
                    "{corruption}"
                );
            }
            assert_eq!(source_counts(&node), before);
        }
    }

    #[test]
    fn response_policy_future_observation_and_expired_receipt_are_not_admitted() {
        let (node, request) = setup();
        let before = source_counts(&node);
        for observed in [now() + Duration::hours(1), now() - Duration::hours(25)] {
            let rejected = commit_policy(
                &node,
                &request,
                &policy(ProviderResponseReuse::Reusable, observed),
            )
            .unwrap_err();
            assert_eq!(rejected.code(), ProblemCode::ValidationFailed);
            assert_eq!(source_counts(&node), before);
        }
    }
}
