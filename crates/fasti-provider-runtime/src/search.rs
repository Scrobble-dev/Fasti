use crate::metadata::run_blocking;
use crate::{
    ProviderRuntime, ProviderRuntimeError, ProviderRuntimeErrorKind, ProviderSearchInput,
    ProviderSearchPage,
};
use fasti_application::{
    ApplicationResult, CapabilityKey, ProblemCode, ProviderCapabilityState, ProviderOperationLease,
    SearchPageRequest, SearchPersistencePort, StoredSearchPage,
};
use std::{future::Future, sync::Arc};

/// One source's outcome. Source failure must not discard another source's local
/// or remote results; authorization and persistence failures remain typed errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSearchOutcome {
    Page {
        page: StoredSearchPage,
        upstream_problem: Option<ProblemCode>,
    },
    Unavailable {
        problem: ProblemCode,
    },
}

/// Governed provider-page orchestration. Hosts acquire their existing provider
/// gate before calling; this service neither creates locks nor owns user state.
pub struct ProviderSearchService {
    runtime: Arc<ProviderRuntime>,
    persistence: Arc<dyn SearchPersistencePort>,
}

impl ProviderSearchService {
    pub fn new(runtime: Arc<ProviderRuntime>, persistence: Arc<dyn SearchPersistencePort>) -> Self {
        Self {
            runtime,
            persistence,
        }
    }

    pub async fn search_page(
        &self,
        request: SearchPageRequest,
        offline: bool,
        lease: ProviderOperationLease,
    ) -> ApplicationResult<ProviderSearchOutcome> {
        let runtime = Arc::clone(&self.runtime);
        let query = request.query.clone();
        let policy = request.outbound_policy.clone();
        self.search_page_with(request, offline, lease, move |state| async move {
            runtime
                .search_page(
                    ProviderSearchInput {
                        provider: query.provider().as_str().to_owned(),
                        query: query.query().as_str().to_owned(),
                    },
                    query.page(),
                    query.locale(),
                    &policy,
                    &state,
                )
                .await
        })
        .await
    }

    // Keep the fetch lazy: cache-only paths never resolve DNS or load credentials.
    // The private closure also exercises sequencing without replacing governed egress.
    async fn search_page_with<F, Fut>(
        &self,
        request: SearchPageRequest,
        offline: bool,
        lease: ProviderOperationLease,
        fetch: F,
    ) -> ApplicationResult<ProviderSearchOutcome>
    where
        F: FnOnce(ProviderCapabilityState) -> Fut,
        Fut: Future<Output = Result<ProviderSearchPage, ProviderRuntimeError>>,
    {
        let capability = CapabilityKey::SearchMetadata;
        let id = request.correlation_id;
        let persistence = Arc::clone(&self.persistence);
        let prepare_request = request.clone();
        let prepared = run_blocking(&lease, capability, id, move || {
            persistence.prepare_search_page(&prepare_request)
        })
        .await?;
        if let Some(page) = self.cached(&request, false, &lease).await? {
            return Ok(ProviderSearchOutcome::Page {
                page,
                upstream_problem: None,
            });
        }
        if offline {
            return self
                .fallback(&request, ProblemCode::ProviderUnavailable, &lease)
                .await;
        }
        let fetched = fetch(prepared.provider_state.clone()).await;
        // Errors also expose source state. Recheck authority before returning any
        // post-I/O outcome, then let the final commit recheck atomically again.
        let persistence = Arc::clone(&self.persistence);
        let check_request = request.clone();
        let current = run_blocking(&lease, capability, id, move || {
            persistence.prepare_search_page(&check_request)
        })
        .await?;
        if current.partition != prepared.partition {
            return Err(Box::new(fasti_application::FastiProblem::forbidden(
                capability, id,
            )));
        }
        let fetched = match fetched {
            Ok(page) => page,
            Err(error) => {
                if matches!(
                    error.kind(),
                    ProviderRuntimeErrorKind::Network | ProviderRuntimeErrorKind::Provider
                ) && matches!(
                    error.problem_code(),
                    ProblemCode::ProviderUnavailable | ProblemCode::ProviderRateLimited
                ) {
                    return self.fallback(&request, error.problem_code(), &lease).await;
                }
                // Vault, policy, credential and malformed-response failures cannot
                // rescue an otherwise ineligible page through stale-on-error.
                return Ok(ProviderSearchOutcome::Unavailable {
                    problem: error.problem_code(),
                });
            }
        };
        let mut candidates = Vec::with_capacity(fetched.candidates.len());
        let context = request.query.receipt_context();
        for candidate in &fetched.candidates {
            let candidate = match candidate.search_evidence() {
                Ok(candidate) => candidate,
                Err(error) => {
                    return Ok(ProviderSearchOutcome::Unavailable {
                        problem: error.problem_code(),
                    })
                }
            };
            if candidate.data().provider != request.query.provider().as_str() {
                return Ok(ProviderSearchOutcome::Unavailable {
                    problem: ProblemCode::ProviderResponseInvalid,
                });
            }
            if context.accepts(&candidate) {
                candidates.push(candidate);
            }
        }
        let persistence = Arc::clone(&self.persistence);
        let page = run_blocking(&lease, capability, id, move || {
            persistence.commit_search_page(
                &request,
                &prepared,
                &candidates,
                &fetched.evidence_digest,
                fetched.next_page,
            )
        })
        .await?;
        Ok(ProviderSearchOutcome::Page {
            page,
            upstream_problem: None,
        })
    }

    async fn cached(
        &self,
        request: &SearchPageRequest,
        upstream_unavailable: bool,
        lease: &ProviderOperationLease,
    ) -> ApplicationResult<Option<StoredSearchPage>> {
        let persistence = Arc::clone(&self.persistence);
        let request = request.clone();
        run_blocking(
            lease,
            CapabilityKey::SearchMetadata,
            request.correlation_id,
            move || persistence.read_cached_search_page(&request, upstream_unavailable),
        )
        .await
    }

    async fn fallback(
        &self,
        request: &SearchPageRequest,
        problem: ProblemCode,
        lease: &ProviderOperationLease,
    ) -> ApplicationResult<ProviderSearchOutcome> {
        Ok(match self.cached(request, true, lease).await? {
            Some(page) => ProviderSearchOutcome::Page {
                page,
                upstream_problem: Some(problem),
            },
            None => ProviderSearchOutcome::Unavailable { problem },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_application::*;
    use fasti_domain::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };

    struct Persistence {
        prepared: PreparedSearchPage,
        fresh: Option<StoredSearchPage>,
        stale: Option<StoredSearchPage>,
        calls: Mutex<Vec<&'static str>>,
        deny_commit: AtomicBool,
        deny_prepare: AtomicBool,
        pause_commit: Mutex<
            Option<(
                tokio::sync::oneshot::Sender<()>,
                std::sync::mpsc::Receiver<()>,
            )>,
        >,
    }

    fn page() -> StoredSearchPage {
        let at = chrono::Utc::now();
        StoredSearchPage {
            sequence: 1,
            candidates: vec![],
            next_page: Some(2),
            cache_state: SearchCacheState::Fresh,
            lifetime: SearchReceiptLifetime::try_new(
                at,
                at + chrono::Duration::seconds(120),
                at + chrono::Duration::seconds(600),
                at + chrono::Duration::seconds(86400),
            )
            .unwrap(),
            response_digest: Sha256Digest::from_bytes(&[4; 32]),
        }
    }

    fn setup(
        fresh: Option<StoredSearchPage>,
        stale: Option<StoredSearchPage>,
    ) -> (ProviderSearchService, Arc<Persistence>, SearchPageRequest) {
        setup_with_grains(fresh, stale, vec![])
    }

    fn setup_with_grains(
        fresh: Option<StoredSearchPage>,
        stale: Option<StoredSearchPage>,
        grains: Vec<Grain>,
    ) -> (ProviderSearchService, Arc<Persistence>, SearchPageRequest) {
        let access = RequestAccessContext::new(
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let request = SearchPageRequest {
            correlation_id: RequestCorrelationId::new_v7(),
            access: access.into(),
            query: SearchProviderQuery::try_new(
                SearchQuery::try_new("Fixture").unwrap(),
                ProviderId::try_new("tmdb").unwrap(),
                1,
                None,
                None,
                grains,
            )
            .unwrap(),
            outbound_policy: OutboundAccessPolicy::default(),
            terms_revision: "fixture-terms".into(),
        };
        let prepared = PreparedSearchPage {
            partition: SearchReceiptPartition::try_new(
                AuthorizedApplicationAccess::new(
                    access.workspace_id(),
                    access.profile_id(),
                    access.grant_id(),
                    AuthorizedActor::Credential {
                        presented_client_id: access.client_id(),
                        credential_id: access.credential_id(),
                    },
                ),
                request.query.receipt_context().digest(),
                Sha256Digest::from_bytes(&[1; 32]),
                Sha256Digest::from_bytes(&[2; 32]),
                request.terms_revision.clone(),
            )
            .unwrap(),
            provider_state: ProviderCapabilityState::try_new(
                ProviderId::try_new("tmdb").unwrap(),
                ProviderCapabilityId::try_new("metadata.search").unwrap(),
                ProviderCapabilityStatus::Available,
                1,
                CredentialRequirement::BearerToken,
                Some(CredentialReference::try_new("secret:fixture").unwrap()),
                ProviderCredentialStatus::StoredUnverified,
                ConfigurationDigest::parse("ab".repeat(32)).unwrap(),
                ProviderCheckMetadata::never_run(),
                ProviderCheckMetadata::never_run(),
            )
            .unwrap(),
        };
        let persistence = Arc::new(Persistence {
            prepared,
            fresh,
            stale,
            calls: Mutex::new(vec![]),
            deny_commit: AtomicBool::new(false),
            deny_prepare: AtomicBool::new(false),
            pause_commit: Mutex::new(None),
        });
        let runtime = Arc::new(ProviderRuntime::new(Arc::new(
            crate::PlatformCredentialVault::new("fasti-test", "search-no-vault-access"),
        )));
        (
            ProviderSearchService::new(runtime, persistence.clone()),
            persistence,
            request,
        )
    }

    impl SearchPersistencePort for Persistence {
        fn prepare_search_page(
            &self,
            request: &SearchPageRequest,
        ) -> ApplicationResult<PreparedSearchPage> {
            self.calls.lock().unwrap().push("prepare");
            if self.deny_prepare.load(Ordering::SeqCst) {
                return Err(Box::new(FastiProblem::forbidden(
                    CapabilityKey::SearchMetadata,
                    request.correlation_id,
                )));
            }
            Ok(self.prepared.clone())
        }
        fn read_cached_search_page(
            &self,
            _: &SearchPageRequest,
            stale: bool,
        ) -> ApplicationResult<Option<StoredSearchPage>> {
            self.calls
                .lock()
                .unwrap()
                .push(if stale { "stale" } else { "fresh" });
            Ok(if stale {
                self.stale.clone()
            } else {
                self.fresh.clone()
            })
        }
        fn commit_search_page(
            &self,
            request: &SearchPageRequest,
            prepared: &PreparedSearchPage,
            candidates: &[SearchCandidate],
            digest: &Sha256Digest,
            next_page: Option<u32>,
        ) -> ApplicationResult<StoredSearchPage> {
            self.calls.lock().unwrap().push("commit");
            let pause = self.pause_commit.lock().unwrap().take();
            if let Some((entered, release)) = pause {
                let _ = entered.send(());
                let _ = release.recv();
            }
            if self.deny_commit.load(Ordering::SeqCst) {
                return Err(Box::new(FastiProblem::forbidden(
                    CapabilityKey::SearchMetadata,
                    request.correlation_id,
                )));
            }
            assert_eq!(prepared, &self.prepared);
            let mut page = page();
            page.next_page = next_page;
            page.response_digest = digest.clone();
            page.candidates = candidates
                .iter()
                .map(|candidate| {
                    SearchCandidateReceipt::new(
                        SearchCandidateReceiptId::new_v7(),
                        prepared.partition.clone(),
                        candidate.clone(),
                        digest.clone(),
                        page.lifetime.clone(),
                    )
                })
                .collect();
            Ok(page)
        }
        fn read_search_candidate(
            &self,
            _: &ReadSearchCandidateRequest,
        ) -> ApplicationResult<Option<StoredSearchCandidate>> {
            panic!("page orchestration must not reopen details");
        }
    }

    async fn lease() -> ProviderOperationLease {
        ProviderOperationLease::new(Arc::new(tokio::sync::Mutex::new(())).lock_owned().await)
    }

    #[tokio::test]
    async fn cache_and_offline_paths_never_fetch_even_for_empty_pages() {
        for offline in [false, true] {
            let (service, persistence, request) = setup(Some(page()), None);
            let result = service
                .search_page_with(request, offline, lease().await, |_| async {
                    panic!("cache hit fetched upstream")
                })
                .await
                .unwrap();
            assert!(
                matches!(result, ProviderSearchOutcome::Page { page, upstream_problem: None } if page.candidates.is_empty() && page.cache_state == SearchCacheState::Fresh)
            );
            assert_eq!(*persistence.calls.lock().unwrap(), ["prepare", "fresh"]);
        }
        for cached in [false, true] {
            let mut stale = page();
            stale.cache_state = SearchCacheState::StaleOnError;
            let (service, persistence, request) = setup(None, cached.then_some(stale));
            let result = service
                .search_page_with(request, true, lease().await, |_| async {
                    panic!("offline fetched upstream")
                })
                .await
                .unwrap();
            assert_eq!(matches!(result, ProviderSearchOutcome::Page { .. }), cached);
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare", "fresh", "stale"]
            );
        }
    }

    #[tokio::test]
    async fn search_commits_normalized_receipts_and_filtered_empty_continuations() {
        for filtered in [false, true] {
            let grains = if filtered {
                vec![Grain::Series]
            } else {
                vec![]
            };
            let (service, persistence, request) = setup_with_grains(None, None, grains);
            let upstream = crate::providers::search_page_fixture();
            let digest = upstream.evidence_digest.clone();
            let expected = persistence.prepared.provider_state.clone();
            let result = service
                .search_page_with(request, false, lease().await, move |state| async move {
                    assert_eq!(state, expected);
                    Ok(upstream)
                })
                .await
                .unwrap();
            let ProviderSearchOutcome::Page {
                page,
                upstream_problem: None,
            } = result
            else {
                panic!("committed page")
            };
            assert_eq!(page.candidates.len(), usize::from(!filtered));
            assert_eq!(page.next_page, Some(2));
            assert_eq!(page.response_digest, digest);
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare", "fresh", "prepare", "commit"]
            );
        }
    }

    #[tokio::test]
    async fn only_transient_upstream_failures_allow_stale_fallback() {
        for (error, fallback) in [
            (ProviderRuntimeError::network("offline"), true),
            (ProviderRuntimeError::rate_limited("limit"), true),
            (ProviderRuntimeError::credential("denied"), false),
            (ProviderRuntimeError::configuration("policy"), false),
            (ProviderRuntimeError::vault("locked"), false),
            (ProviderRuntimeError::response_invalid("invalid"), false),
        ] {
            let mut stale = page();
            stale.cache_state = SearchCacheState::StaleOnError;
            let (service, persistence, request) = setup(None, Some(stale));
            let code = error.problem_code();
            let outcome = service
                .search_page_with(request, false, lease().await, |_| async move { Err(error) })
                .await
                .unwrap();
            match outcome {
                ProviderSearchOutcome::Page {
                    upstream_problem, ..
                } => {
                    assert!(fallback);
                    assert_eq!(upstream_problem, Some(code));
                }
                ProviderSearchOutcome::Unavailable { problem } => {
                    assert!(!fallback);
                    assert_eq!(problem, code);
                }
            }
            assert_eq!(
                persistence.calls.lock().unwrap().contains(&"stale"),
                fallback
            );
        }
    }

    #[tokio::test]
    async fn search_commit_reauthorization_is_not_rescued_by_cache() {
        let (service, persistence, request) = setup(None, Some(page()));
        persistence.deny_commit.store(true, Ordering::SeqCst);
        let error = service
            .search_page_with(request, false, lease().await, |_| async {
                Ok(crate::providers::search_page_fixture())
            })
            .await
            .unwrap_err();
        assert_eq!(error.code(), ProblemCode::Forbidden);
        assert_eq!(
            *persistence.calls.lock().unwrap(),
            ["prepare", "fresh", "prepare", "commit"]
        );
    }

    #[tokio::test]
    async fn search_cancellation_releases_network_but_retains_started_commit_lease() {
        for commit in [false, true] {
            let (service, persistence, request) = setup(None, None);
            let gate = Arc::new(tokio::sync::Mutex::new(()));
            let lease = ProviderOperationLease::new(Arc::clone(&gate).lock_owned().await);
            let (entered, started) = tokio::sync::oneshot::channel();
            let (release, finish) = std::sync::mpsc::channel();
            let mut network_entered = Some(entered);
            if commit {
                *persistence.pause_commit.lock().unwrap() =
                    Some((network_entered.take().unwrap(), finish));
            }
            let caller = tokio::spawn(async move {
                service
                    .search_page_with(request, false, lease, |_| async move {
                        if let Some(entered) = network_entered {
                            let _ = entered.send(());
                            std::future::pending::<()>().await;
                        }
                        Ok(crate::providers::search_page_fixture())
                    })
                    .await
            });
            tokio::time::timeout(std::time::Duration::from_secs(5), started)
                .await
                .unwrap()
                .unwrap();
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
            let held = gate.try_lock().is_err();
            let _ = release.send(());
            let _completed = tokio::time::timeout(std::time::Duration::from_secs(5), gate.lock())
                .await
                .unwrap();
            assert_eq!(held, commit);
            assert_eq!(
                persistence.calls.lock().unwrap().contains(&"commit"),
                commit
            );
        }
    }

    #[tokio::test]
    async fn real_outbound_denials_are_not_stale_fallback_errors() {
        for (host, networks, policy) in [
            (
                "127.0.0.1",
                &[NetworkClass::Public][..],
                OutboundAccessPolicy::default(),
            ),
            (
                "93.184.216.34",
                &[NetworkClass::Public][..],
                OutboundAccessPolicy {
                    deny_providers: vec!["tmdb".into()],
                    ..Default::default()
                },
            ),
        ] {
            let (service, persistence, request) = setup(None, Some(page()));
            let outcome = service
                .search_page_with(request, false, lease().await, |_| async move {
                    let declaration = OutboundAccessDeclaration {
                        provider: "tmdb",
                        capabilities: &["metadata.search"],
                        hosts: if host == "127.0.0.1" {
                            &["127.0.0.1"]
                        } else {
                            &["93.184.216.34"]
                        },
                        networks,
                    };
                    let endpoint = format!("https://{host}/").parse().unwrap();
                    crate::GovernedTransport::default()
                        .authorize(declaration, &policy, "metadata.search", &endpoint)
                        .await?;
                    panic!("denied endpoint must not be authorized")
                })
                .await
                .unwrap();
            assert_eq!(
                outcome,
                ProviderSearchOutcome::Unavailable {
                    problem: ProblemCode::ProviderRouteUnavailable
                }
            );
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare", "fresh", "prepare"]
            );
        }
    }

    #[tokio::test]
    async fn search_reauthorizes_before_disclosing_failure_or_result() {
        for success in [false, true] {
            let (service, persistence, request) = setup(None, Some(page()));
            let revoked = Arc::clone(&persistence);
            let error = service
                .search_page_with(request, false, lease().await, |_| async move {
                    revoked.deny_prepare.store(true, Ordering::SeqCst);
                    if success {
                        Ok(crate::providers::search_page_fixture())
                    } else {
                        Err(ProviderRuntimeError::credential("denied"))
                    }
                })
                .await
                .unwrap_err();
            assert_eq!(error.code(), ProblemCode::Forbidden);
            assert_eq!(
                *persistence.calls.lock().unwrap(),
                ["prepare", "fresh", "prepare"]
            );
        }
        let (service, persistence, request) = setup(None, None);
        persistence.deny_prepare.store(true, Ordering::SeqCst);
        assert_eq!(
            service
                .search_page_with(request, false, lease().await, |_| async {
                    panic!("unauthorized fetch")
                })
                .await
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(*persistence.calls.lock().unwrap(), ["prepare"]);
    }
}
