use super::*;
include!("local_search_bounds_tests.rs");
use crate::{
    identity::insert_record,
    metadata::{write_field_claim, write_profile_field_override},
    test_support::TestNode,
};
use fasti_application::{
    ConfigureMetadataProjectionCommand, MetadataOverrideMutation, MetadataProjectionPort,
    ProblemCode, RequestAccessContext, ScopeKey, SearchPersistencePort,
};
use fasti_domain::{
    FieldClaim, FieldKey, MetadataProjectionPolicy, NamespaceKey, ProfileFieldOverride, ReceivedAt,
    RequestCorrelationId, SearchQuery,
};

fn node() -> TestNode {
    let node = TestNode::new();
    assert_eq!(node.kernel.inner.connection.lock().unwrap().query_row(
        "SELECT COUNT(*) FROM grant_scopes WHERE grant_id = ?1 AND scope_key = 'metadata_search'",
        [node.access.grant_id().to_string()],
        |row| row.get::<_, i64>(0),
    ).unwrap(), 1);
    node
}

fn request(access: RequestAccessContext, query: &str) -> LocalSearchRequest {
    LocalSearchRequest {
        correlation_id: RequestCorrelationId::new_v7(),
        access: access.into(),
        query: SearchQuery::try_new(query).unwrap(),
        grains: vec![],
        after: None,
    }
}

fn seed(node: &TestNode, count: usize, title: &str) -> Vec<RecordId> {
    seed_kind(node, count, title, Grain::Film)
}

fn seed_kind(node: &TestNode, count: usize, title: &str, grain: Grain) -> Vec<RecordId> {
    let mut connection = node.kernel.inner.connection.lock().unwrap();
    let transaction = connection.transaction().unwrap();
    let ids = (0..count)
        .map(|_| {
            let id = insert_record(
                &transaction,
                node.access.workspace_id(),
                grain,
                CAPABILITY,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
            let claim = FieldClaim::try_new(
                NamespaceKey::try_new("tmdb").unwrap(),
                title,
                None,
                ReceivedAt::from_application_clock(crate::kernel::now()),
                None,
            )
            .unwrap();
            write_field_claim(
                &transaction,
                node.access.workspace_id(),
                id,
                &FieldKey::try_new(TITLE_FIELD_KEY).unwrap(),
                &claim,
                CAPABILITY,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
            id
        })
        .collect();
    transaction.commit().unwrap();
    ids
}

fn configure(
    node: &TestNode,
    access: RequestAccessContext,
    mutations: Vec<MetadataOverrideMutation>,
) {
    node.kernel
        .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
            RequestCorrelationId::new_v7(),
            access,
            MetadataProjectionPolicy::default_for_profile(access.profile_id()),
            None,
            vec![],
            mutations,
        ))
        .unwrap();
}

fn set(record: RecordId, field: &str, value: &str) -> MetadataOverrideMutation {
    MetadataOverrideMutation::Set {
        record_id: record,
        field_key: FieldKey::try_new(field).unwrap(),
        value: value.into(),
    }
}

fn ids(node: &TestNode, access: RequestAccessContext, query: &str) -> Vec<RecordId> {
    node.kernel
        .search_local_records(&request(access, query))
        .unwrap()
        .records
        .into_iter()
        .map(|record| record.record_id())
        .collect()
}

#[test]
fn local_search_literal_short_punctuation_and_unicode_queries() {
    let node = node();
    let record = seed(&node, 1, "Árbol 東京 🐎 %_ O\"R (NEAR) Straße")[0];
    for query in [
        "á", "ÁR", "東", "東京", "🐎", "%", "%_", "O\"R", "(NEAR)", "straße",
    ] {
        assert_eq!(ids(&node, node.access, query), vec![record], "{query}");
    }
    for query in ["absent", "arbol", "STRASSE", "OR", "*", "[a-z]"] {
        assert!(
            ids(&node, node.access, query).is_empty(),
            "literal negative: {query}"
        );
    }
    let connection = node.kernel.inner.connection.lock().unwrap();
    let provider_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM provider_capability_states",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        provider_count, 0,
        "local Search needs no configured provider"
    );
}

#[test]
fn local_search_postings_are_candidates_not_authoritative_matches() {
    let node = node();
    let record = seed(&node, 1, "needle unrelated")[0];
    assert!(ids(&node, node.access, "needle absent").is_empty());
    configure(
        &node,
        node.access,
        vec![set(record, TITLE_FIELD_KEY, "Replacement")],
    );
    assert!(
        ids(&node, node.access, "needle").is_empty(),
        "hidden old claim cannot match"
    );
    assert_eq!(ids(&node, node.access, "Replacement"), vec![record]);
}

#[test]
fn local_search_private_override_isolation_and_clear_keep_sibling_postings() {
    let node = node();
    let second = node.add_profile_with_scopes(&[ScopeKey::MetadataSearch]);
    let record = seed(&node, 1, "Public title")[0];
    configure(
        &node,
        node.access,
        vec![
            set(record, TITLE_FIELD_KEY, "Private secret"),
            set(record, ORIGINAL_TITLE_FIELD_KEY, "Private sibling"),
        ],
    );
    assert_eq!(ids(&node, node.access, "Private"), vec![record]);
    assert!(ids(&node, second, "Private").is_empty());
    assert_eq!(ids(&node, second, "Public"), vec![record]);
    configure(
        &node,
        node.access,
        vec![MetadataOverrideMutation::Clear {
            record_id: record,
            field_key: FieldKey::try_new(TITLE_FIELD_KEY).unwrap(),
        }],
    );
    assert!(ids(&node, node.access, "secret").is_empty());
    assert_eq!(ids(&node, node.access, "sibling"), vec![record]);
    assert_eq!(ids(&node, node.access, "Public"), vec![record]);
    let connection = node.kernel.inner.connection.lock().unwrap();
    let deleted: i64 = connection.query_row(
        "SELECT COUNT(*) FROM local_search_grams WHERE workspace_id=?1 AND profile_partition=?2 AND record_id=?3 AND gram='sec'",
        params![node.access.workspace_id().to_string(), node.access.profile_id().to_string(), record.to_string()], |row| row.get(0),
    ).unwrap();
    assert_eq!(deleted, 0, "cleared private text is removed from postings");
}

#[test]
fn local_search_stable_pages_deduplicate_shared_and_private_hits_beyond_500() {
    let node = node();
    let mut expected = seed(&node, 605, "Common title");
    configure(
        &node,
        node.access,
        expected
            .iter()
            .take(101)
            .map(|record| set(*record, ORIGINAL_TITLE_FIELD_KEY, "Common private"))
            .collect(),
    );
    expected.sort_by_key(ToString::to_string);
    let mut query = request(node.access, "Common");
    let mut found = Vec::new();
    let mut sizes = Vec::new();
    loop {
        let page = node.kernel.search_local_records(&query).unwrap();
        sizes.push(page.records.len());
        found.extend(page.records.into_iter().map(|record| record.record_id()));
        if page.next.is_none() {
            break;
        }
        query.after = page.next;
        assert!(sizes.len() < 10, "cursor must progress");
    }
    assert_eq!(sizes, vec![100, 100, 100, 100, 100, 100, 5]);
    assert_eq!(found, expected);
}

#[test]
fn local_search_empty_filtered_page_keeps_progress_to_later_matches() {
    let node = node();
    let mut records = seed(&node, 101, "needle absent");
    records.sort_by_key(ToString::to_string);
    configure(
        &node,
        node.access,
        vec![set(
            *records.last().unwrap(),
            TITLE_FIELD_KEY,
            "needle present",
        )],
    );
    let mut query = request(node.access, "needle present");
    let first = node.kernel.search_local_records(&query).unwrap();
    assert!(first.records.is_empty());
    assert!(first.next.is_some());
    query.after = first.next;
    let second = node.kernel.search_local_records(&query).unwrap();
    assert_eq!(second.records.len(), 1);
    assert_eq!(second.records[0].record_id(), *records.last().unwrap());
    assert!(second.next.is_none());
}

#[test]
fn local_search_cursor_binds_query_profile_grain_and_rechecks_revocation() {
    let node = node();
    seed(&node, 101, "Common title");
    let second = node.add_profile_with_scopes(&[ScopeKey::MetadataSearch]);
    let mut query = request(node.access, "Common");
    query.after = node.kernel.search_local_records(&query).unwrap().next;
    assert!(query.after.is_some());
    for changed in [
        LocalSearchRequest {
            query: SearchQuery::try_new("Common title").unwrap(),
            ..query.clone()
        },
        LocalSearchRequest {
            access: second.into(),
            ..query.clone()
        },
        LocalSearchRequest {
            grains: vec![Grain::Film],
            ..query.clone()
        },
    ] {
        assert_eq!(
            node.kernel
                .search_local_records(&changed)
                .err()
                .unwrap()
                .code(),
            ProblemCode::ValidationFailed
        );
    }
    node.kernel
        .inner
        .connection
        .lock()
        .unwrap()
        .execute(
            "DELETE FROM grant_scopes WHERE grant_id=?1 AND scope_key='metadata_search'",
            [node.access.grant_id().to_string()],
        )
        .unwrap();
    assert_eq!(
        node.kernel
            .search_local_records(&query)
            .err()
            .unwrap()
            .code(),
        ProblemCode::Forbidden
    );
}

#[test]
fn local_search_rebuild_matches_sources_and_failed_rebuild_rolls_back() {
    let node = node();
    let record = seed(&node, 1, "Public title")[0];
    configure(
        &node,
        node.access,
        vec![set(record, ORIGINAL_TITLE_FIELD_KEY, "Private original")],
    );
    let before = ids(&node, node.access, "Private");
    {
        let mut connection = node.kernel.inner.connection.lock().unwrap();
        let transaction = connection.transaction().unwrap();
        transaction
            .execute("DELETE FROM local_search_grams", [])
            .unwrap();
        rebuild(&transaction).unwrap();
        transaction.commit().unwrap();
    }
    assert_eq!(ids(&node, node.access, "Private"), before);
    assert_eq!(ids(&node, node.access, "Public"), vec![record]);
    {
        let mut connection = node.kernel.inner.connection.lock().unwrap();
        connection.execute_batch("CREATE TRIGGER reject_search_rebuild BEFORE INSERT ON local_search_grams BEGIN SELECT RAISE(ABORT, 'fixture interruption'); END;").unwrap();
        let transaction = connection.transaction().unwrap();
        assert!(rebuild(&transaction).is_err());
        transaction.rollback().unwrap();
        connection
            .execute_batch("DROP TRIGGER reject_search_rebuild")
            .unwrap();
    }
    assert_eq!(ids(&node, node.access, "Private"), before);
}

#[test]
fn local_search_candidate_plan_uses_posting_keyset_without_full_scan() {
    let node = node();
    seed(&node, 100, "Common title");
    let connection = node.kernel.inner.connection.lock().unwrap();
    let mut plan = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {SELECT_CANDIDATES}"))
        .unwrap();
    let details = plan
        .query_map(
            params![node.access.workspace_id().to_string(), "", "com", ""],
            |row| row.get::<_, String>(3),
        )
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("SEARCH local_search_grams")
                && detail.contains("gram=?")
                && detail.contains("record_id>?")),
        "{details:?}"
    );
    assert!(
        !details
            .iter()
            .any(|detail| detail.contains("SCAN local_search_grams")
                || detail.contains("SCAN record")),
        "{details:?}"
    );
    let mut query = connection.prepare(SELECT_CANDIDATES).unwrap();
    query
        .query_map(
            params![node.access.workspace_id().to_string(), "", "com", ""],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(query.get_status(rusqlite::StatementStatus::FullscanStep), 0);
    let steps = query.get_status(rusqlite::StatementStatus::VmStep);
    assert!(steps < 5000, "candidate VM steps={steps}; plan={details:?}");
}

#[test]
fn local_search_sparse_grain_keeps_bounded_candidate_work_and_continuation() {
    let node = node();
    seed(&node, 1000, "Common film");
    let target = seed_kind(&node, 1, "Common series", Grain::Series)[0];
    let mut query = request(node.access, "Common");
    query.grains = vec![Grain::Series];
    let first = node.kernel.search_local_records(&query).unwrap();
    assert!(first.records.is_empty());
    assert!(first.next.is_some());
    query.after = first.next;
    let mut found = Vec::new();
    let mut pages = 1;
    loop {
        let page = node.kernel.search_local_records(&query).unwrap();
        found.extend(page.records.into_iter().map(|record| record.record_id()));
        pages += 1;
        if page.next.is_none() {
            break;
        }
        query.after = page.next;
        assert!(pages <= 11);
    }
    assert_eq!(found, vec![target]);
    assert_eq!(pages, 11);
    query.after = None;
    query.grains = vec![Grain::Edition];
    let absent = node.kernel.search_local_records(&query).unwrap();
    assert!(absent.records.is_empty());
    assert!(absent.next.is_some());
    let connection = node.kernel.inner.connection.lock().unwrap();
    let mut statement = connection.prepare(SELECT_CANDIDATES).unwrap();
    let rows = statement
        .query_map(
            params![node.access.workspace_id().to_string(), "", "com", ""],
            |row| row.get::<_, String>(0),
        )
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(rows.len(), 101);
    assert_eq!(
        statement.get_status(rusqlite::StatementStatus::FullscanStep),
        0
    );
    let steps = statement.get_status(rusqlite::StatementStatus::VmStep);
    assert!(steps < 5000, "candidate VM steps={steps}");
}

#[test]
#[ignore = "release-only 10,000-Record performance evidence; run explicitly with --release --ignored"]
fn local_search_10000_records_release_latency() {
    #[cfg(debug_assertions)]
    panic!("run this evidence fixture with --release");
    #[cfg(not(debug_assertions))]
    {
        let node = node();
        seed(&node, 10_000, "Common title 東京 %_");
        let mut samples = Vec::new();
        for iteration in 0..105 {
            let query = request(
                node.access,
                ["c", "co", "Common", "東京", "%_", "missing"][iteration % 6],
            );
            let start = std::time::Instant::now();
            let page = node.kernel.search_local_records(&query).unwrap();
            let elapsed = start.elapsed();
            assert!(page.records.len() <= 100);
            if iteration >= 5 {
                samples.push(elapsed);
            }
        }
        samples.sort();
        let p95 = samples[94];
        eprintln!(
            "local_search dataset=10000 samples=100 p50={:?} p95={:?} max={:?}",
            samples[49], p95, samples[99]
        );
        assert!(p95 < std::time::Duration::from_millis(250), "p95={p95:?}");
    }
}

#[test]
fn local_search_direct_metadata_writers_roll_back_source_and_index_without_outer_transaction() {
    let node = node();
    let record = seed(&node, 1, "Public title")[0];
    configure(
        &node,
        node.access,
        vec![set(record, TITLE_FIELD_KEY, "Private title")],
    );
    let connection = node.kernel.inner.connection.lock().unwrap();
    assert!(connection.is_autocommit(), "no enclosing test transaction");
    let snapshot = || {
        [
            "metadata_field_claims",
            "metadata_claims",
            "metadata_claim_provenance",
            "metadata_profile_field_overrides",
            "local_search_grams",
        ]
        .into_iter()
        .map(|table| {
            let mut statement = connection
                .prepare(&format!("SELECT * FROM {table} ORDER BY 1,2,3,4"))
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
        .collect::<Vec<_>>()
    };
    let before = snapshot();
    // The ordered gram writer inserts earlier grams before this rejection, so
    // both partial insert rollback and restoration of deleted private grams matter.
    connection.execute_batch("CREATE TRIGGER reject_search_owner_write BEFORE INSERT ON local_search_grams WHEN NEW.gram = 'z' BEGIN SELECT RAISE(ABORT, 'fixture index write failure'); END;").unwrap();
    let key = FieldKey::try_new(TITLE_FIELD_KEY).unwrap();
    let at =
        ReceivedAt::from_application_clock(crate::kernel::now() + chrono::Duration::seconds(1));
    let claim = FieldClaim::try_new(
        NamespaceKey::try_new("tmdb").unwrap(),
        "abzz claim",
        None,
        at,
        None,
    )
    .unwrap();
    assert!(write_field_claim(
        &connection,
        node.access.workspace_id(),
        record,
        &key,
        &claim,
        CAPABILITY,
        RequestCorrelationId::new_v7()
    )
    .is_err());
    assert!(
        connection.is_autocommit(),
        "claim savepoint is released after rollback"
    );
    assert_eq!(
        snapshot(),
        before,
        "failed direct claim write changed source/index values"
    );
    let override_ =
        ProfileFieldOverride::try_new(node.access.profile_id(), record, key, "abzz override", at)
            .unwrap();
    assert!(write_profile_field_override(
        &connection,
        node.access.workspace_id(),
        &override_,
        CAPABILITY,
        RequestCorrelationId::new_v7()
    )
    .is_err());
    assert!(
        connection.is_autocommit(),
        "override savepoint is released after rollback"
    );
    assert_eq!(
        snapshot(),
        before,
        "failed direct override write changed source/index values"
    );
    connection
        .execute_batch("DROP TRIGGER reject_search_owner_write")
        .unwrap();
    drop(connection);
    assert_eq!(ids(&node, node.access, "Private"), vec![record]);
    assert!(ids(&node, node.access, "abzz").is_empty());
}
