//! Node-local, immutable Search snapshots. No Record is created by these paths.

use crate::kernel::{authorize_application_transaction, map_sql, now, parse_timestamp, timestamp};
use crate::{providers, SqliteKernel};
use chrono::Duration;
use fasti_application::{
    ApplicationAccessContext, ApplicationResult, AuthorizedActor, CapabilityKey, FastiProblem,
    PreparedSearchPage, ProblemCode, SearchCandidate, SearchCandidateReceipt, SearchPageRequest,
    SearchPersistencePort, SearchReceiptLifetime, SearchReceiptPartition, StoredSearchPage,
    MAX_SEARCH_PAGE_CANDIDATES, SEARCH_FRESH_SECONDS, SEARCH_RECEIPT_SECONDS,
    SEARCH_STALE_ON_ERROR_SECONDS,
};
use fasti_domain::{RequestCorrelationId, SearchCandidateReceiptId, Sha256Digest};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::Serialize;

const CAPABILITY: CapabilityKey = CapabilityKey::SearchMetadata;
const MAX_CACHED_PAGES: i64 = 1024;
const MAX_CACHED_BYTES: i64 = 64 * 1024 * 1024;

fn failure(code: ProblemCode, id: RequestCorrelationId) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(code, CAPABILITY, id))
}

fn json(value: &impl Serialize, id: RequestCorrelationId) -> ApplicationResult<String> {
    serde_json::to_string(value).map_err(|_| failure(ProblemCode::IntegrityFailed, id))
}

fn digest(value: &impl Serialize, id: RequestCorrelationId) -> ApplicationResult<Sha256Digest> {
    format!(
        "sha256:{}",
        crate::crypto::sha256_hex(json(value, id)?.as_bytes())
    )
    .parse()
    .map_err(|_| failure(ProblemCode::IntegrityFailed, id))
}

fn prepare(
    transaction: &Transaction<'_>,
    request: &SearchPageRequest,
) -> ApplicationResult<PreparedSearchPage> {
    let id = request.correlation_id;
    let access = authorize_application_transaction(transaction, CAPABILITY, &request.access, id)?;
    let state = map_sql(transaction.query_row(
        &providers::state_select("WHERE workspace_id = ?1 AND provider_id = ?2 AND capability_id = 'metadata.search'"),
        params![access.workspace_id().to_string(), request.query.provider().as_str()], providers::read_state,
    ).optional(), CAPABILITY, id)?.ok_or_else(|| failure(ProblemCode::CapabilityUnavailable, id))?;
    if !matches!(
        state.capability_status(),
        fasti_application::ProviderCapabilityStatus::Available
            | fasti_application::ProviderCapabilityStatus::Degraded
    ) {
        return Err(failure(ProblemCode::CapabilityUnavailable, id));
    }
    request
        .outbound_policy
        .validate_identifiers()
        .map_err(|_| failure(ProblemCode::Forbidden, id))?;
    let authority_version: i64 = map_sql(transaction.query_row(
        "SELECT authority_version FROM provider_capability_states WHERE workspace_id = ?1 AND provider_id = ?2 AND capability_id = 'metadata.search'",
        params![access.workspace_id().to_string(), request.query.provider().as_str()], |r| r.get(0)
    ), CAPABILITY, id)?;
    let configuration = digest(
        &(
            "fasti.search.provider.v1",
            state.provider_id().as_str(),
            state.capability_id().as_str(),
            authority_version,
            state.configuration_digest().as_str(),
            state.credential_requirement().as_str(),
            state.credential_status().as_str(),
            state
                .credential_reference()
                .map(|reference| reference.as_str()),
            &request.outbound_policy,
        ),
        id,
    )?;
    let mut statement = map_sql(
        transaction
            .prepare("SELECT scope_key FROM grant_scopes WHERE grant_id = ?1 ORDER BY scope_key"),
        CAPABILITY,
        id,
    )?;
    let scopes = map_sql(
        statement.query_map([access.grant_id().to_string()], |r| r.get::<_, String>(0)),
        CAPABILITY,
        id,
    )?
    .collect::<Result<Vec<_>, _>>();
    let scopes = map_sql(scopes, CAPABILITY, id)?;
    let epochs = match access.actor() {
        AuthorizedActor::Credential {
            presented_client_id,
            credential_id,
        } => {
            let epoch: i64 = map_sql(
                transaction.query_row(
                    "SELECT current_credential_epoch FROM clients WHERE client_id = ?1",
                    [presented_client_id.to_string()],
                    |r| r.get(0),
                ),
                CAPABILITY,
                id,
            )?;
            json(&("credential", credential_id, epoch), id)?
        }
        AuthorizedActor::BrowserSession {
            auth_subject_id, ..
        } => {
            let epochs: (i64, i64) = map_sql(transaction.query_row(
                "SELECT auth_epoch, authorization_epoch FROM auth_subjects WHERE auth_subject_id = ?1",
                [auth_subject_id.to_string()], |r| Ok((r.get(0)?, r.get(1)?))), CAPABILITY, id)?;
            json(&("subject", auth_subject_id, epochs), id)?
        }
    };
    let grant = digest(
        &(
            "fasti.search.grant.v1",
            access.workspace_id(),
            access.profile_id(),
            access.grant_id(),
            access.attribution_client_id(),
            scopes,
            epochs,
        ),
        id,
    )?;
    let partition = SearchReceiptPartition::try_new(
        access,
        request.query.digest(),
        grant,
        configuration,
        request.terms_revision.clone(),
    )
    .map_err(|_| failure(ProblemCode::ValidationFailed, id))?;
    Ok(PreparedSearchPage {
        partition,
        provider_state: state,
    })
}

impl SearchPersistencePort for SqliteKernel {
    fn prepare_search_page(
        &self,
        request: &SearchPageRequest,
    ) -> ApplicationResult<PreparedSearchPage> {
        let mut connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| failure(ProblemCode::StorageUnavailable, request.correlation_id))?;
        let transaction = map_sql(connection.transaction(), CAPABILITY, request.correlation_id)?;
        let prepared = prepare(&transaction, request)?;
        map_sql(transaction.commit(), CAPABILITY, request.correlation_id)?;
        Ok(prepared)
    }

    fn commit_search_page(
        &self,
        request: &SearchPageRequest,
        prepared: &PreparedSearchPage,
        candidates: &[SearchCandidate],
        response_digest: &Sha256Digest,
        next_page: Option<u32>,
    ) -> ApplicationResult<StoredSearchPage> {
        let id = request.correlation_id;
        if let ApplicationAccessContext::BrowserSession(access) = &request.access {
            if !access.is_mutation() {
                return Err(failure(ProblemCode::Forbidden, id));
            }
        }
        if candidates.len() > MAX_SEARCH_PAGE_CANDIDATES
            || next_page.is_some_and(|page| page <= request.query.page())
            || candidates
                .iter()
                .any(|candidate| candidate.data().provider != request.query.provider().as_str())
        {
            return Err(failure(ProblemCode::ValidationFailed, id));
        }
        let mut coordinates = std::collections::HashSet::new();
        if candidates.iter().any(|candidate| {
            !coordinates.insert((&candidate.data().kind, &candidate.data().provider_id))
        }) {
            return Err(failure(ProblemCode::ValidationFailed, id));
        }
        let candidate_json = candidates
            .iter()
            .map(|candidate| {
                candidate
                    .to_json()
                    .map_err(|_| failure(ProblemCode::ValidationFailed, id))
            })
            .collect::<ApplicationResult<Vec<_>>>()?;
        let candidate_bytes = candidate_json.iter().map(String::len).sum::<usize>() as i64;
        let mut connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| failure(ProblemCode::StorageUnavailable, id))?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            CAPABILITY,
            id,
        )?;
        let current = prepare(&transaction, request)?;
        if current.partition != prepared.partition {
            return Err(failure(ProblemCode::Forbidden, id));
        }
        let partition = &current.partition;
        let created = parse_timestamp(&timestamp(now()), CAPABILITY, id)?;
        let life = SearchReceiptLifetime::try_new(
            created,
            created + Duration::seconds(SEARCH_FRESH_SECONDS),
            created + Duration::seconds(SEARCH_STALE_ON_ERROR_SECONDS),
            created + Duration::seconds(SEARCH_RECEIPT_SECONDS),
        )
        .map_err(|_| failure(ProblemCode::IntegrityFailed, id))?;
        map_sql(transaction.execute(
            "DELETE FROM search_pages WHERE sequence IN (SELECT sequence FROM search_pages WHERE expires_at <= ?1 ORDER BY expires_at, sequence LIMIT 100)",
            [timestamp(created)]), CAPABILITY, id)?;
        let (pages, bytes): (i64, i64) = map_sql(
            transaction.query_row(
                "SELECT COUNT(*), COALESCE(SUM(candidate_bytes), 0) FROM search_pages",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            ),
            CAPABILITY,
            id,
        )?;
        if pages >= MAX_CACHED_PAGES || candidate_bytes > MAX_CACHED_BYTES - bytes {
            // Expired-only maintenance must advance even when a new snapshot
            // cannot fit. No new page or Record has been written here.
            map_sql(transaction.commit(), CAPABILITY, id)?;
            return Err(failure(ProblemCode::CapacityExceeded, id));
        }
        map_sql(transaction.execute(
            "INSERT INTO search_pages(partition_json, partition_digest, workspace_id, profile_id,
                actor_client_id, actor_subject_id, grant_id, provider_id, upstream_page, next_page,
                candidate_count, response_digest, created_at, fresh_until, stale_until, expires_at, candidate_bytes)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![json(partition, id)?, digest(partition, id)?.as_str(), partition.workspace_id().to_string(),
                partition.profile_id().to_string(), partition.actor_client_id().to_string(),
                partition.actor_subject_id().map(|value| value.to_string()), partition.grant_id().to_string(),
                request.query.provider().as_str(), request.query.page(), next_page, candidates.len() as i64,
                response_digest.as_str(), timestamp(created), timestamp(life.fresh_until()), timestamp(life.stale_until()), timestamp(life.expires_at()), candidate_bytes]
        ), CAPABILITY, id)?;
        let sequence = transaction.last_insert_rowid();
        let mut receipts = Vec::with_capacity(candidates.len());
        for (ordinal, candidate) in candidates.iter().enumerate() {
            let receipt_id = SearchCandidateReceiptId::new_v7();
            map_sql(transaction.execute(
                "INSERT INTO search_candidate_receipts(candidate_receipt_id, page_sequence, ordinal, kind, provider_record_id, candidate_json) VALUES (?1,?2,?3,?4,?5,?6)",
                params![receipt_id.to_string(), sequence, ordinal as i64, candidate.data().kind,
                    candidate.data().provider_id, candidate_json[ordinal]]
            ), CAPABILITY, id)?;
            receipts.push(SearchCandidateReceipt::new(
                receipt_id,
                partition.clone(),
                candidate.clone(),
                response_digest.clone(),
                life.clone(),
            ));
        }
        map_sql(transaction.commit(), CAPABILITY, id)?;
        Ok(StoredSearchPage {
            sequence: sequence as u64,
            candidates: receipts,
            next_page,
        })
    }

    fn read_cached_search_page(
        &self,
        request: &SearchPageRequest,
        upstream_unavailable: bool,
    ) -> ApplicationResult<Option<StoredSearchPage>> {
        let id = request.correlation_id;
        let mut connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| failure(ProblemCode::StorageUnavailable, id))?;
        let transaction = map_sql(connection.transaction(), CAPABILITY, id)?;
        let current = prepare(&transaction, request)?;
        let row = map_sql(transaction.query_row(
            "SELECT sequence, next_page, candidate_count, response_digest, created_at, fresh_until, stale_until, expires_at
             FROM search_pages WHERE partition_digest = ?1 AND partition_json = ?2 ORDER BY sequence DESC LIMIT 1",
            params![digest(&current.partition, id)?.as_str(), json(&current.partition, id)?],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<u32>>(1)?, r.get::<_, u32>(2)?, r.get::<_, String>(3)?,
                r.get::<_, String>(4)?, r.get::<_, String>(5)?, r.get::<_, String>(6)?, r.get::<_, String>(7)?))
        ).optional(), CAPABILITY, id)?;
        let Some((sequence, next_page, count, response, created, fresh, stale, expires)) = row
        else {
            map_sql(transaction.commit(), CAPABILITY, id)?;
            return Ok(None);
        };
        let life = SearchReceiptLifetime::try_new(
            parse_timestamp(&created, CAPABILITY, id)?,
            parse_timestamp(&fresh, CAPABILITY, id)?,
            parse_timestamp(&stale, CAPABILITY, id)?,
            parse_timestamp(&expires, CAPABILITY, id)?,
        )
        .map_err(|_| failure(ProblemCode::IntegrityFailed, id))?;
        if life.cache_state(now(), upstream_unavailable).is_none() {
            map_sql(transaction.commit(), CAPABILITY, id)?;
            return Ok(None);
        }
        let response = response
            .parse()
            .map_err(|_| failure(ProblemCode::IntegrityFailed, id))?;
        let candidates = read_candidates(
            &transaction,
            sequence,
            count as usize,
            &current.partition,
            &response,
            &life,
            id,
        )?;
        map_sql(transaction.commit(), CAPABILITY, id)?;
        Ok(Some(StoredSearchPage {
            sequence: sequence as u64,
            candidates,
            next_page,
        }))
    }
}

fn read_candidates(
    connection: &Connection,
    sequence: i64,
    count: usize,
    partition: &SearchReceiptPartition,
    response: &Sha256Digest,
    life: &SearchReceiptLifetime,
    id: RequestCorrelationId,
) -> ApplicationResult<Vec<SearchCandidateReceipt>> {
    if count > MAX_SEARCH_PAGE_CANDIDATES {
        return Err(failure(ProblemCode::IntegrityFailed, id));
    }
    let mut statement = map_sql(connection.prepare(
        "SELECT candidate_receipt_id, ordinal, candidate_json FROM search_candidate_receipts WHERE page_sequence = ?1 ORDER BY ordinal LIMIT 101"
    ), CAPABILITY, id)?;
    let rows = map_sql(
        statement.query_map([sequence], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, String>(2)?,
            ))
        }),
        CAPABILITY,
        id,
    )?;
    let mut results = Vec::with_capacity(count);
    for row in rows {
        let (receipt, ordinal, data) = map_sql(row, CAPABILITY, id)?;
        if ordinal as usize != results.len() || ordinal as usize >= count {
            return Err(failure(ProblemCode::IntegrityFailed, id));
        }
        results.push(SearchCandidateReceipt::new(
            receipt
                .parse()
                .map_err(|_| failure(ProblemCode::IntegrityFailed, id))?,
            partition.clone(),
            SearchCandidate::from_json(&data)
                .map_err(|_| failure(ProblemCode::IntegrityFailed, id))?,
            response.clone(),
            life.clone(),
        ));
    }
    if results.len() != count {
        return Err(failure(ProblemCode::IntegrityFailed, id));
    }
    Ok(results)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{
        ConfigurationDigest, CredentialReference, CredentialRequirement, OutboundAccessPolicy,
        ProviderCapabilityId, ProviderCapabilityState, ProviderCapabilityStatus,
        ProviderCheckMetadata, ProviderCredentialStatus, ProviderId, ProviderStatePort,
        SearchCandidateData, SearchProviderQuery,
    };
    use fasti_domain::SearchQuery;

    pub(crate) fn state(version: u64) -> ProviderCapabilityState {
        ProviderCapabilityState::try_new(
            ProviderId::try_new("tmdb").unwrap(),
            ProviderCapabilityId::try_new("metadata.search").unwrap(),
            ProviderCapabilityStatus::Available,
            version,
            CredentialRequirement::BearerToken,
            Some(CredentialReference::try_new("secret:tmdb-test").unwrap()),
            ProviderCredentialStatus::StoredUnverified,
            ConfigurationDigest::parse("a".repeat(64)).unwrap(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .unwrap()
    }

    fn setup() -> (TestNode, SearchPageRequest) {
        let node = TestNode::new();
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, 'metadata_search')",
                [node.access.grant_id().to_string()],
            )
            .unwrap();
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), state(1))
            .unwrap();
        let request = SearchPageRequest {
            correlation_id: RequestCorrelationId::new_v7(),
            access: node.access.into(),
            query: SearchProviderQuery::try_new(
                SearchQuery::try_new("Star").unwrap(),
                ProviderId::try_new("tmdb").unwrap(),
                1,
                None,
                None,
                vec![],
            )
            .unwrap(),
            outbound_policy: OutboundAccessPolicy::default(),
            terms_revision: "tmdb-v1".into(),
        };
        (node, request)
    }

    fn candidate(value: &str) -> SearchCandidate {
        SearchCandidate::try_new(SearchCandidateData {
            provider: "tmdb".into(),
            provider_id: value.into(),
            kind: "movie".into(),
            title: format!("Film {value}"),
            original_title: None,
            release_year: None,
            authors: vec![],
            image_url: None,
            overview: None,
        })
        .unwrap()
    }

    fn commit(
        node: &TestNode,
        request: &SearchPageRequest,
        candidates: &[SearchCandidate],
    ) -> StoredSearchPage {
        let prepared = node.kernel.prepare_search_page(request).unwrap();
        node.kernel
            .commit_search_page(
                request,
                &prepared,
                candidates,
                &Sha256Digest::from_bytes(&[7; 32]),
                Some(2),
            )
            .unwrap()
    }

    #[test]
    fn search_snapshots_survive_restart_preserve_order_and_do_not_create_records() {
        let (node, request) = setup();
        let saved = commit(&node, &request, &[candidate("42"), candidate("12")]);
        assert_eq!(
            node.kernel
                .read_cached_search_page(&request, false)
                .unwrap(),
            Some(saved.clone())
        );
        let records: i64 = node
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(records, 0);
        let (root, _) = node.into_stopped();
        let reopened = SqliteKernel::open(root.path()).unwrap();
        assert_eq!(
            reopened.read_cached_search_page(&request, false).unwrap(),
            Some(saved)
        );
    }

    #[test]
    fn search_empty_pages_are_cached_and_refresh_does_not_change_old_receipts() {
        let (node, request) = setup();
        let first = commit(&node, &request, &[candidate("42")]);
        let empty = commit(&node, &request, &[]);
        assert!(empty.sequence > first.sequence);
        assert_eq!(
            node.kernel
                .read_cached_search_page(&request, false)
                .unwrap(),
            Some(empty)
        );
        let count: i64 = node
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM search_candidate_receipts WHERE candidate_receipt_id = ?1",
                [first.candidates[0].id().to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn search_rechecks_grants_and_configuration_between_prepare_and_commit() {
        let (node, request) = setup();
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        commit(&node, &request, &[candidate("42")]);
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), state(2))
            .unwrap();
        assert!(node
            .kernel
            .read_cached_search_page(&request, true)
            .unwrap()
            .is_none());
        assert!(node
            .kernel
            .commit_search_page(
                &request,
                &prepared,
                &[],
                &Sha256Digest::from_bytes(&[7; 32]),
                None
            )
            .is_err());
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
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
        assert!(node.kernel.read_cached_search_page(&request, true).is_err());
        assert!(node
            .kernel
            .commit_search_page(
                &request,
                &prepared,
                &[],
                &Sha256Digest::from_bytes(&[7; 32]),
                None
            )
            .is_err());
    }

    #[test]
    fn search_corrupt_child_count_fails_closed_and_duplicate_page_write_is_atomic() {
        let (node, request) = setup();
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        assert!(node
            .kernel
            .commit_search_page(
                &request,
                &prepared,
                &[candidate("42"), candidate("42")],
                &Sha256Digest::from_bytes(&[7; 32]),
                None
            )
            .is_err());
        assert!(node
            .kernel
            .read_cached_search_page(&request, false)
            .unwrap()
            .is_none());
        let page = commit(&node, &request, &[candidate("42"), candidate("43")]);
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM search_candidate_receipts WHERE candidate_receipt_id = ?1",
                [page.candidates[0].id().to_string()],
            )
            .unwrap();
        assert_eq!(
            node.kernel
                .read_cached_search_page(&request, false)
                .unwrap_err()
                .code(),
            ProblemCode::IntegrityFailed
        );
    }

    #[test]
    fn search_query_policy_and_terms_partitions_do_not_replay_each_other() {
        let (node, request) = setup();
        commit(&node, &request, &[candidate("42")]);
        let mut changed = request.clone();
        changed.terms_revision = "tmdb-v2".into();
        assert!(node
            .kernel
            .read_cached_search_page(&changed, true)
            .unwrap()
            .is_none());
        changed = request.clone();
        changed.outbound_policy.deny_providers.push("tmdb".into());
        assert!(node
            .kernel
            .read_cached_search_page(&changed, true)
            .unwrap()
            .is_none());
        changed = request.clone();
        changed.query = SearchProviderQuery::try_new(
            SearchQuery::try_new("Other").unwrap(),
            ProviderId::try_new("tmdb").unwrap(),
            1,
            None,
            None,
            vec![],
        )
        .unwrap();
        assert!(node
            .kernel
            .read_cached_search_page(&changed, true)
            .unwrap()
            .is_none());
    }

    #[test]
    fn degraded_capability_retains_cache_but_credential_identity_change_does_not() {
        let (node, request) = setup();
        let saved = commit(&node, &request, &[candidate("42")]);
        let base = state(2);
        let degraded = ProviderCapabilityState::try_new(
            base.provider_id().clone(),
            base.capability_id().clone(),
            ProviderCapabilityStatus::Degraded,
            2,
            base.credential_requirement(),
            base.credential_reference().cloned(),
            base.credential_status(),
            base.configuration_digest().clone(),
            ProviderCheckMetadata::try_new(
                fasti_application::ProviderCheckStatus::Unavailable,
                Some(now()),
                Some(ProblemCode::ProviderUnavailable),
            )
            .unwrap(),
            base.credential_test().clone(),
        )
        .unwrap();
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), degraded)
            .unwrap();
        assert_eq!(
            node.kernel.read_cached_search_page(&request, true).unwrap(),
            Some(saved.clone())
        );
        // Corruption/legacy-state fixture: the normal state writer requires a
        // higher version, which already invalidates Search partitions.
        node.kernel.inner.connection.lock().unwrap().execute(
            "UPDATE provider_capability_states SET capability_status = 'degraded' WHERE provider_id = 'tmdb'", []
        ).unwrap();
        assert_eq!(
            node.kernel.read_cached_search_page(&request, true).unwrap(),
            Some(saved)
        );
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        node.kernel.inner.connection.lock().unwrap().execute(
            "UPDATE provider_capability_states SET credential_reference = 'secret:tmdb-other' WHERE provider_id = 'tmdb'", []
        ).unwrap();
        assert!(node
            .kernel
            .read_cached_search_page(&request, true)
            .unwrap()
            .is_none());
        assert!(node
            .kernel
            .commit_search_page(
                &request,
                &prepared,
                &[],
                &Sha256Digest::from_bytes(&[7; 32]),
                None
            )
            .is_err());
    }

    #[test]
    fn search_quota_refuses_new_snapshots_without_evicting_unexpired_results() {
        let (node, request) = setup();
        commit(&node, &request, &[]);
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            for _ in 0..10 {
                connection.execute("INSERT INTO search_pages(partition_json, partition_digest, workspace_id, profile_id, actor_client_id, actor_subject_id, grant_id, provider_id, upstream_page, next_page, candidate_count, candidate_bytes, response_digest, created_at, fresh_until, stale_until, expires_at)
                    SELECT partition_json, partition_digest, workspace_id, profile_id, actor_client_id, actor_subject_id, grant_id, provider_id, upstream_page, next_page, candidate_count, candidate_bytes, response_digest, created_at, fresh_until, stale_until, expires_at FROM search_pages", []).unwrap();
            }
        }
        let before = node
            .kernel
            .read_cached_search_page(&request, false)
            .unwrap();
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        assert_eq!(
            node.kernel
                .commit_search_page(
                    &request,
                    &prepared,
                    &[],
                    &Sha256Digest::from_bytes(&[7; 32]),
                    None
                )
                .unwrap_err()
                .code(),
            ProblemCode::CapacityExceeded
        );
        assert_eq!(
            node.kernel
                .read_cached_search_page(&request, false)
                .unwrap(),
            before
        );
    }

    #[test]
    fn expired_cleanup_advances_even_when_snapshot_admission_is_still_over_quota() {
        let (node, request) = setup();
        commit(&node, &request, &[]);
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            let created = now() - Duration::days(2);
            // Explicit over-quota recovery fixture. Production admission does
            // not create this many pages, but recovery must remain bounded.
            for _ in 0..11 {
                connection.execute("INSERT INTO search_pages(partition_json, partition_digest, workspace_id, profile_id, actor_client_id, actor_subject_id, grant_id, provider_id, upstream_page, next_page, candidate_count, candidate_bytes, response_digest, created_at, fresh_until, stale_until, expires_at)
                    SELECT partition_json, partition_digest, workspace_id, profile_id, actor_client_id, actor_subject_id, grant_id, provider_id, upstream_page, next_page, candidate_count, candidate_bytes, response_digest, ?1, ?2, ?3, ?4 FROM search_pages",
                    params![timestamp(created), timestamp(created + Duration::seconds(120)), timestamp(created + Duration::seconds(600)), timestamp(created + Duration::days(1))]).unwrap();
            }
        }
        let prepared = node.kernel.prepare_search_page(&request).unwrap();
        assert_eq!(
            node.kernel
                .commit_search_page(
                    &request,
                    &prepared,
                    &[],
                    &Sha256Digest::from_bytes(&[7; 32]),
                    None
                )
                .unwrap_err()
                .code(),
            ProblemCode::CapacityExceeded
        );
        let remaining: i64 = node
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM search_pages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 2048 - 100);
        let mut admitted = false;
        for _ in 0..11 {
            if node
                .kernel
                .commit_search_page(
                    &request,
                    &prepared,
                    &[],
                    &Sha256Digest::from_bytes(&[7; 32]),
                    None,
                )
                .is_ok()
            {
                admitted = true;
                break;
            }
        }
        assert!(
            admitted,
            "bounded cleanup must eventually make room without losing progress"
        );
    }
}
