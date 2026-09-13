mod response_policy_tests {
    use super::*;
    use fasti_application::{ProviderResponseCachePolicy, ProviderResponseReuse};

    fn observed_policy(reuse: ProviderResponseReuse) -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(
            reuse,
            chrono::Utc::now(),
            std::time::Duration::from_secs(10),
            Some(std::time::Duration::from_secs(60)),
            Some(std::time::Duration::from_secs(30)),
        )
    }

    #[tokio::test]
    async fn no_store_pages_return_live_candidates_without_commit_or_stale_fallback() {
        for (empty, filtered) in [(false, false), (true, false), (false, true)] {
            let grains = if filtered {
                vec![Grain::Series]
            } else {
                vec![]
            };
            // Eligible historical data must not replace a successful live result.
            let (service, persistence, request) = setup_with_grains(None, Some(page()), grains);
            let mut upstream = crate::providers::search_page_fixture()
                .with_response_cache_policy(observed_policy(ProviderResponseReuse::NoStore));
            if empty {
                upstream.candidates.clear();
            }
            let expected: Vec<_> = upstream
                .candidates
                .iter()
                .filter(|_| !filtered)
                .map(|candidate| candidate.search_evidence().unwrap())
                .collect();
            let result = service
                .search_page_with(
                    request,
                    false,
                    lease().await,
                    |_| async move { Ok(upstream) },
                )
                .await
                .unwrap();
            let ProviderSearchOutcome::Live {
                candidates,
                next_page,
            } = result
            else {
                panic!("successful no-store response must remain live-only")
            };
            assert_eq!(candidates, expected);
            assert_eq!(next_page, Some(2));
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare", "fresh", "prepare", "discard"]
            );
        }
    }

    #[tokio::test]
    async fn no_store_terminal_empty_page_does_not_invent_continuation() {
        let (service, persistence, request) = setup(None, None);
        let mut upstream = crate::providers::search_page_fixture()
            .with_response_cache_policy(observed_policy(ProviderResponseReuse::NoStore));
        upstream.candidates.clear();
        upstream.next_page = None;
        let result = service
            .search_page_with(
                request,
                false,
                lease().await,
                |_| async move { Ok(upstream) },
            )
            .await
            .unwrap();
        assert!(
            matches!(result, ProviderSearchOutcome::Live { candidates, next_page: None } if candidates.is_empty())
        );
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "prepare", "discard"]
        );
    }

    #[tokio::test]
    async fn no_store_discard_denial_suppresses_live_payload() {
        let (service, persistence, request) = setup(None, Some(page()));
        persistence.deny_commit.store(true, Ordering::SeqCst);
        let upstream = crate::providers::search_page_fixture()
            .with_response_cache_policy(observed_policy(ProviderResponseReuse::NoStore));
        let error = service
            .search_page_with(
                request,
                false,
                lease().await,
                |_| async move { Ok(upstream) },
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), ProblemCode::Forbidden);
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "prepare", "discard"]
        );
    }

    #[tokio::test]
    async fn no_store_live_response_still_requires_current_post_fetch_authority() {
        let (service, persistence, request) = setup(None, Some(page()));
        let revoke = Arc::clone(&persistence);
        let upstream = crate::providers::search_page_fixture()
            .with_response_cache_policy(observed_policy(ProviderResponseReuse::NoStore));
        let error = service
            .search_page_with(request, false, lease().await, |_| async move {
                revoke.deny_prepare.store(true, Ordering::SeqCst);
                Ok(upstream)
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), ProblemCode::Forbidden);
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "prepare"]
        );
    }

    #[tokio::test]
    async fn no_store_live_candidates_still_require_normalized_evidence() {
        let (service, persistence, request) = setup(None, Some(page()));
        let mut upstream = crate::providers::search_page_fixture()
            .with_response_cache_policy(observed_policy(ProviderResponseReuse::NoStore));
        upstream.candidates[0].title.clear();
        let result = service
            .search_page_with(
                request,
                false,
                lease().await,
                |_| async move { Ok(upstream) },
            )
            .await
            .unwrap();
        assert!(matches!(
            result,
            ProviderSearchOutcome::Unavailable {
                problem: ProblemCode::ProviderResponseInvalid
            }
        ));
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "prepare"]
        );
    }

    #[tokio::test]
    async fn no_store_live_page_rejects_over_bound_count_and_nonforward_continuation() {
        for invalid in ["count", "same_page", "earlier_page", "duplicate_coordinate"] {
            let (service, persistence, request) = setup(None, Some(page()));
            let mut upstream = crate::providers::search_page_fixture();
            match invalid {
                "count" => {
                    upstream.candidates = vec![
                        upstream.candidates[0].clone();
                        fasti_application::MAX_SEARCH_PAGE_CANDIDATES + 1
                    ];
                }
                "same_page" => upstream.next_page = Some(request.query.page()),
                "earlier_page" => upstream.next_page = Some(request.query.page() - 1),
                "duplicate_coordinate" => {
                    upstream.candidates.push(upstream.candidates[0].clone());
                }
                _ => unreachable!(),
            }
            let upstream = upstream
                .with_response_cache_policy(observed_policy(ProviderResponseReuse::NoStore));
            let result = service
                .search_page_with(
                    request,
                    false,
                    lease().await,
                    |_| async move { Ok(upstream) },
                )
                .await
                .unwrap();
            assert!(
                matches!(
                    result,
                    ProviderSearchOutcome::Unavailable {
                        problem: ProblemCode::ProviderResponseInvalid
                    }
                ),
                "invalid live page: {invalid}"
            );
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare", "fresh", "prepare"],
                "invalid live page must not mutate or rescue cache: {invalid}"
            );
        }
    }

    #[tokio::test]
    async fn reusable_response_policy_keeps_existing_durable_commit_path() {
        let (service, persistence, request) = setup(None, None);
        let upstream = crate::providers::search_page_fixture()
            .with_response_cache_policy(observed_policy(ProviderResponseReuse::Reusable));
        let digest = upstream.evidence_digest.clone();
        let result = service
            .search_page_with(
                request,
                false,
                lease().await,
                |_| async move { Ok(upstream) },
            )
            .await
            .unwrap();
        let ProviderSearchOutcome::Page {
            page,
            upstream_problem: None,
        } = result
        else {
            panic!("reusable response must retain durable page behavior")
        };
        assert_eq!(page.candidates.len(), 1);
        assert_eq!(page.response_digest, digest);
        assert_eq!(page.next_page, Some(2));
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "prepare", "commit"]
        );
    }

    #[tokio::test]
    async fn live_result_is_not_an_offline_cache_entry_on_a_later_request() {
        let (service, persistence, request) = setup(None, None);
        let upstream = crate::providers::search_page_fixture()
            .with_response_cache_policy(observed_policy(ProviderResponseReuse::NoStore));
        let online = service
            .search_page_with(request.clone(), false, lease().await, |_| async move {
                Ok(upstream)
            })
            .await
            .unwrap();
        assert!(matches!(online, ProviderSearchOutcome::Live { .. }));
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "prepare", "discard"]
        );
        persistence.calls.lock().unwrap().clear();
        let offline = service
            .search_page_with(request, true, lease().await, |_| async {
                panic!("offline must not refetch a previously live-only result")
            })
            .await
            .unwrap();
        assert!(matches!(
            offline,
            ProviderSearchOutcome::Unavailable {
                problem: ProblemCode::ProviderUnavailable
            }
        ));
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "stale"]
        );
    }
}
