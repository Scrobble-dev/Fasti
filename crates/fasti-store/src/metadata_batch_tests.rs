//! Differential checks against the existing single-Record metadata owners.
use super::*;
use crate::{identity::insert_record, test_support::TestNode};
use chrono::TimeZone;
use fasti_domain::{
    Grain, ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY, POSTER_FIELD_KEY, RELEASE_YEAR_FIELD_KEY,
    TITLE_FIELD_KEY,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const CAP: CapabilityKey = CapabilityKey::ListRecords;

fn fields() -> [FieldKey; 5] {
    [
        TITLE_FIELD_KEY,
        POSTER_FIELD_KEY,
        ORIGINAL_TITLE_FIELD_KEY,
        OVERVIEW_FIELD_KEY,
        RELEASE_YEAR_FIELD_KEY,
    ]
    .map(|field| FieldKey::try_new(field).unwrap())
}

fn received_at(seconds: i64) -> ReceivedAt {
    ReceivedAt::from_application_clock(
        chrono::Utc
            .timestamp_opt(1_700_000_000 + seconds, 0)
            .single()
            .unwrap(),
    )
}

fn records(node: &TestNode, count: usize) -> Vec<RecordId> {
    let mut connection = node.kernel.inner.connection.lock().unwrap();
    let transaction = connection.transaction().unwrap();
    let result = (0..count)
        .map(|_| {
            insert_record(
                &transaction,
                node.access.workspace_id(),
                Grain::Film,
                CAP,
                RequestCorrelationId::new_v7(),
            )
            .unwrap()
        })
        .collect();
    transaction.commit().unwrap();
    result
}

fn mixed_claims(node: &TestNode, record: RecordId, field: &FieldKey, count: usize) {
    mixed_claim_range(node, record, field, 0..count);
}

fn mixed_claim_range(
    node: &TestNode,
    record: RecordId,
    field: &FieldKey,
    ordinals: std::ops::Range<usize>,
) {
    let mut connection = node.kernel.inner.connection.lock().unwrap();
    let transaction = connection.transaction().unwrap();
    for ordinal in ordinals {
        let (provider, source) = if ordinal % 2 == 0 {
            ("tmdb", "tmdb.movie")
        } else {
            ("google-books", "googlebooks.volume")
        };
        let locale = if ordinal % 3 == 0 { "fr-fr" } else { "en-ie" };
        let fetched = received_at(ordinal as i64);
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new(provider).unwrap(),
            NamespaceKey::try_new(source).unwrap(),
            format!("fixture-{ordinal}"),
            Some(MetadataLocale::try_new(locale).unwrap()),
            None,
            None,
            Sha256Digest::from_bytes(&[7; 32]),
        )
        .unwrap();
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record,
            field.clone(),
            format!("Value {ordinal}"),
            provenance,
            fetched,
            (ordinal % 5 == 0).then(|| fetched.value() + chrono::Duration::seconds(10)),
            FieldClaimStatus::Fresh,
        )
        .unwrap();
        write_field_claim(
            &transaction,
            node.access.workspace_id(),
            record,
            field,
            &claim,
            CAP,
            RequestCorrelationId::new_v7(),
            None,
        )
        .unwrap();
        if ordinal % 7 == 0 {
            let event = FieldClaimLifecycleEvent::try_new(
                claim.claim_id(),
                1,
                FieldClaimStatus::Fresh,
                FieldClaimStatus::Revoked,
                received_at(ordinal as i64 + 1000),
                Some(Sha256Digest::from_bytes(&[9; 32])),
            )
            .unwrap();
            append_field_claim_lifecycle_event(
                &transaction,
                node.access.workspace_id(),
                &event,
                CAP,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
        }
    }
    transaction.commit().unwrap();
}

fn assert_matches_single_record_owner(node: &TestNode, selected: &[RecordId]) {
    let connection = node.kernel.inner.connection.lock().unwrap();
    let fields = fields();
    let id = RequestCorrelationId::new_v7();
    let batch = load_record_metadata_batch(
        &connection,
        node.access.workspace_id(),
        node.access.profile_id(),
        selected,
        &fields,
        CAP,
        id,
    )
    .unwrap();
    let policy = load_projection_policy(
        &connection,
        node.access.workspace_id(),
        node.access.profile_id(),
        CAP,
        id,
    )
    .unwrap();
    for record in selected {
        for field in &fields {
            let claims = load_field_claims(
                &connection,
                node.access.workspace_id(),
                *record,
                field,
                CAP,
                id,
                crate::kernel::now(),
            )
            .unwrap();
            let override_ = load_profile_field_override(
                &connection,
                node.access.workspace_id(),
                node.access.profile_id(),
                *record,
                field,
                CAP,
                id,
            )
            .unwrap();
            let expected =
                resolve_profile_field(override_.as_ref(), &claims, &[], &policy, batch.resolved_at)
                    .unwrap();
            assert_eq!(
                batch.resolve(*record, field, CAP, id).unwrap(),
                expected,
                "record={record}, field={}",
                field.as_str()
            );
        }
    }
}

#[test]
fn metadata_batch_matches_single_record_resolution_at_claim_cap_and_sparse_selection() {
    let node = TestNode::new();
    let records = records(&node, 603);
    let fields = fields();
    for (field, count) in fields.iter().take(3).zip([255, 256, 257]) {
        mixed_claims(&node, records[600], field, count);
    }
    mixed_claims(&node, records[550], &fields[3], 7);
    {
        let connection = node.kernel.inner.connection.lock().unwrap();
        let policy = MetadataProjectionPolicy::new(
            node.access.profile_id(),
            Some(MetadataProviderId::try_new("tmdb").unwrap()),
            Some(MetadataLocale::try_new("fr-fr").unwrap()),
            Some(MetadataLocale::try_new("en-ie").unwrap()),
            true,
            LastKnownGoodPolicy::Allow,
        );
        write_projection_policy(
            &connection,
            node.access.workspace_id(),
            &policy,
            CAP,
            RequestCorrelationId::new_v7(),
        )
        .unwrap();
        let override_ = ProfileFieldOverride::try_new(
            node.access.profile_id(),
            records[601],
            fields[0].clone(),
            "Override only",
            received_at(2000),
        )
        .unwrap();
        write_profile_field_override(
            &connection,
            node.access.workspace_id(),
            &override_,
            CAP,
            RequestCorrelationId::new_v7(),
        )
        .unwrap();
        for (field, expected) in fields.iter().take(3).zip([255, 256, 256]) {
            assert_eq!(
                load_field_claims(
                    &connection,
                    node.access.workspace_id(),
                    records[600],
                    field,
                    CAP,
                    RequestCorrelationId::new_v7(),
                    crate::kernel::now()
                )
                .unwrap()
                .len(),
                expected
            );
        }
    }
    assert_matches_single_record_owner(
        &node,
        &[
            records[601],
            records[600],
            records[550],
            records[602],
            records[600],
        ],
    );
    assert_matches_single_record_owner(&node, &[]);
}

#[test]
fn metadata_batch_rejects_malformed_selected_claim_even_when_override_wins() {
    let node = TestNode::new();
    let records = records(&node, 2);
    let field = fields()[0].clone();
    mixed_claims(&node, records[0], &field, 2);
    let connection = node.kernel.inner.connection.lock().unwrap();
    let override_ = ProfileFieldOverride::try_new(
        node.access.profile_id(),
        records[0],
        field.clone(),
        "Winning override",
        received_at(2000),
    )
    .unwrap();
    write_profile_field_override(
        &connection,
        node.access.workspace_id(),
        &override_,
        CAP,
        RequestCorrelationId::new_v7(),
    )
    .unwrap();
    let trigger: String = connection.query_row("SELECT sql FROM sqlite_master WHERE type='trigger' AND name='metadata_field_claims_immutable_update'", [], |row| row.get(0)).unwrap();
    connection
        .execute_batch("DROP TRIGGER metadata_field_claims_immutable_update")
        .unwrap();
    connection
        .execute(
            "UPDATE metadata_field_claims SET value = char(10) WHERE record_id=?1",
            [records[0].to_string()],
        )
        .unwrap();
    connection.execute_batch(&trigger).unwrap();
    let id = RequestCorrelationId::new_v7();
    assert!(load_field_claims(
        &connection,
        node.access.workspace_id(),
        records[0],
        &field,
        CAP,
        id,
        crate::kernel::now()
    )
    .is_err());
    assert!(
        load_record_metadata_batch(
            &connection,
            node.access.workspace_id(),
            node.access.profile_id(),
            &[records[0]],
            &fields(),
            CAP,
            id
        )
        .is_err(),
        "an override must not hide corrupt selected evidence"
    );
    assert!(
        load_record_metadata_batch(
            &connection,
            node.access.workspace_id(),
            node.access.profile_id(),
            &[records[1]],
            &fields(),
            CAP,
            id
        )
        .is_ok(),
        "unselected malformed evidence is not loaded"
    );
}

#[test]
fn metadata_batch_matches_source_tiebreak_at_the_256_claim_boundary() {
    let node = TestNode::new();
    let record = records(&node, 1)[0];
    let field = fields()[0].clone();
    mixed_claims(&node, record, &field, 255);
    {
        let connection = node.kernel.inner.connection.lock().unwrap();
        for source in ["aa.fixture", "zz.fixture"] {
            let claim = FieldClaim::try_new(
                NamespaceKey::try_new(source).unwrap(),
                source,
                None,
                received_at(-1),
                None,
            )
            .unwrap();
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record,
                &field,
                &claim,
                CAP,
                RequestCorrelationId::new_v7(),
                None,
            )
            .unwrap();
        }
        let selected = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record,
            &field,
            CAP,
            RequestCorrelationId::new_v7(),
            crate::kernel::now(),
        )
        .unwrap();
        assert_eq!(selected.len(), 256);
        assert_eq!(selected.last().unwrap().source().as_str(), "zz.fixture");
        assert!(!selected
            .iter()
            .any(|claim| claim.source().as_str() == "aa.fixture"));
        assert_eq!(
            batch_claim_ids(
                &connection,
                node.access.workspace_id(),
                &[record],
                &fields()
            ),
            selected
                .iter()
                .map(|claim| claim.claim_id().to_string())
                .collect::<Vec<_>>(),
            "compare every selected claim, not only the winning projection",
        );
    }
    assert_matches_single_record_owner(&node, &[record]);
}

fn batch_claim_ids(
    connection: &Connection,
    workspace: WorkspaceId,
    selected: &[RecordId],
    fields: &[FieldKey; 5],
) -> Vec<String> {
    let selected = serde_json::to_string(selected).unwrap();
    let mut statement = connection.prepare(SELECT_RECORD_METADATA).unwrap();
    statement
        .query_map(
            params![
                workspace.to_string(),
                selected,
                fields[0].as_str(),
                fields[1].as_str(),
                fields[2].as_str(),
                fields[3].as_str(),
                fields[4].as_str(),
                MAX_EFFECTIVE_FIELD_CLAIMS
            ],
            |row| row.get::<_, String>(2),
        )
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

#[test]
fn metadata_batch_missing_newest_provenance_does_not_consume_the_claim_cap() {
    let node = TestNode::new();
    let record = records(&node, 1)[0];
    let field = fields()[0].clone();
    mixed_claims(&node, record, &field, 256);
    {
        let connection = node.kernel.inner.connection.lock().unwrap();
        // Historical/incomplete source shape: the newest field row has not
        // acquired provenance, so neither reader may spend a retained slot on it.
        connection.execute("INSERT INTO metadata_field_claims(workspace_id, record_id, field_key, source, value, fetched_at, created_at) VALUES (?1, ?2, ?3, 'zz.fixture', 'Unregistered newest value', ?4, ?4)",
            params![node.access.workspace_id().to_string(), record.to_string(), field.as_str(), timestamp(received_at(10_000).value())]).unwrap();
        let expected = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record,
            &field,
            CAP,
            RequestCorrelationId::new_v7(),
            crate::kernel::now(),
        )
        .unwrap();
        assert_eq!(expected.len(), 256);
        assert_eq!(
            batch_claim_ids(
                &connection,
                node.access.workspace_id(),
                &[record],
                &fields()
            ),
            expected
                .iter()
                .map(|claim| claim.claim_id().to_string())
                .collect::<Vec<_>>()
        );
    }
    assert_matches_single_record_owner(&node, &[record]);
}

#[test]
fn metadata_batch_duplicate_requested_fields_preserve_in_set_semantics() {
    let node = TestNode::new();
    let record = records(&node, 1)[0];
    let fields = fields();
    mixed_claims(&node, record, &fields[0], 256);
    mixed_claims(&node, record, &fields[3], 3);
    let connection = node.kernel.inner.connection.lock().unwrap();
    let duplicate = [
        fields[0].clone(),
        fields[0].clone(),
        fields[3].clone(),
        fields[0].clone(),
        fields[3].clone(),
    ];
    let expected = batch_claim_ids(&connection, node.access.workspace_id(), &[record], &fields);
    assert_eq!(expected.len(), 259);
    assert_eq!(
        batch_claim_ids(
            &connection,
            node.access.workspace_id(),
            &[record],
            &duplicate
        ),
        expected
    );
    let id = RequestCorrelationId::new_v7();
    let batch = load_record_metadata_batch(
        &connection,
        node.access.workspace_id(),
        node.access.profile_id(),
        &[record],
        &duplicate,
        CAP,
        id,
    )
    .unwrap();
    let policy = load_projection_policy(
        &connection,
        node.access.workspace_id(),
        node.access.profile_id(),
        CAP,
        id,
    )
    .unwrap();
    for field in &duplicate {
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record,
            field,
            CAP,
            id,
            crate::kernel::now(),
        )
        .unwrap();
        assert_eq!(
            batch.resolve(record, field, CAP, id).unwrap(),
            resolve_profile_field(None, &claims, &[], &policy, batch.resolved_at).unwrap()
        );
    }
}

#[test]
fn metadata_batch_select_count_does_not_scale_per_record() {
    let node = TestNode::new();
    let records = records(&node, 100);
    mixed_claims(&node, records[99], &fields()[0], 3);
    let connection = node.kernel.inner.connection.lock().unwrap();
    let selects = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&selects);
    connection
        .authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
            if matches!(context.action, rusqlite::hooks::AuthAction::Select) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
            rusqlite::hooks::Authorization::Allow
        }))
        .unwrap();
    load_record_metadata_batch(
        &connection,
        node.access.workspace_id(),
        node.access.profile_id(),
        &records[99..],
        &fields(),
        CAP,
        RequestCorrelationId::new_v7(),
    )
    .unwrap();
    let single = selects.swap(0, Ordering::Relaxed);
    load_record_metadata_batch(
        &connection,
        node.access.workspace_id(),
        node.access.profile_id(),
        &records,
        &fields(),
        CAP,
        RequestCorrelationId::new_v7(),
    )
    .unwrap();
    let hundred = selects.load(Ordering::Relaxed);
    assert_eq!(
        single, hundred,
        "selected batch must not issue per-Record reads"
    );
    assert!(
        hundred <= 40,
        "reuse the existing Record-list SELECT ceiling, got {hundred}"
    );
}

fn bounded_query_metrics(node: &TestNode, selected: &[RecordId]) -> (usize, i32, i32) {
    let connection = node.kernel.inner.connection.lock().unwrap();
    let selected = serde_json::to_string(selected).unwrap();
    let fields = fields();
    let mut plan = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {SELECT_RECORD_METADATA}"))
        .unwrap();
    let plan_rows = plan
        .query_map(
            params![
                node.access.workspace_id().to_string(),
                selected,
                fields[0].as_str(),
                fields[1].as_str(),
                fields[2].as_str(),
                fields[3].as_str(),
                fields[4].as_str(),
                MAX_EFFECTIVE_FIELD_CLAIMS
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let selected_node = plan_rows
        .iter()
        .find(|(_, _, detail)| detail == "CO-ROUTINE selected")
        .expect("bounded selected-key coroutine")
        .0;
    let inside_selected = |mut ancestor: i64| {
        while ancestor != 0 && ancestor != selected_node {
            ancestor = plan_rows
                .iter()
                .find(|(id, _, _)| *id == ancestor)
                .expect("plan parent exists")
                .1;
        }
        ancestor == selected_node
    };
    for (_, parent, detail) in &plan_rows {
        if detail.contains("TEMP B-TREE") {
            assert!(
                inside_selected(*parent),
                "payload rows must not be sorted after selected keys: {plan_rows:?}"
            );
        }
    }
    assert!(
        plan_rows.iter().any(|(_, _, detail)| detail
            .contains("metadata_claim_provenance_recent_idx")
            && detail.contains("workspace_id=?")
            && detail.contains("record_id=?")
            && detail.contains("field_key=?")),
        "recent claims must use the bounded compound range: {plan_rows:?}"
    );
    let (_, inner_parent, inner_lookup) = plan_rows
        .iter()
        .find(|(_, _, detail)| {
            detail.contains(
                "SEARCH claim USING COVERING INDEX metadata_field_claims_record_field_idx",
            )
        })
        .expect("inner claim existence check must avoid fetching wide payloads");
    assert!(
        inside_selected(*inner_parent),
        "covering lookup belongs inside key selection: {plan_rows:?}"
    );
    for predicate in [
        "workspace_id=?",
        "record_id=?",
        "field_key=?",
        "source=?",
        "fetched_at=?",
    ] {
        assert!(
            inner_lookup.contains(predicate),
            "missing covering equality {predicate}: {plan_rows:?}"
        );
    }
    assert!(
        plan_rows
            .iter()
            .any(|(_, parent, detail)| !inside_selected(*parent)
                && detail.contains("SEARCH claim USING PRIMARY KEY")
                && ["record_id=?", "field_key=?", "source=?", "fetched_at=?"]
                    .iter()
                    .all(|predicate| detail.contains(predicate))),
        "selected payloads must be fetched once by complete primary key: {plan_rows:?}"
    );
    let mut statement = connection.prepare(SELECT_RECORD_METADATA).unwrap();
    let mut rows = statement
        .query(params![
            node.access.workspace_id().to_string(),
            selected,
            fields[0].as_str(),
            fields[1].as_str(),
            fields[2].as_str(),
            fields[3].as_str(),
            fields[4].as_str(),
            MAX_EFFECTIVE_FIELD_CLAIMS
        ])
        .unwrap();
    let mut count = 0;
    while let Some(row) = rows.next().unwrap() {
        PersistedFieldClaimRow::read(row)
            .unwrap()
            .decode(CAP, RequestCorrelationId::new_v7())
            .unwrap();
        count += 1;
    }
    drop(rows);
    (
        count,
        statement.get_status(rusqlite::StatementStatus::FullscanStep),
        statement.get_status(rusqlite::StatementStatus::VmStep),
    )
}

fn known_policy_history(
    node: &TestNode,
    record: RecordId,
    field: &FieldKey,
    range: std::ops::Range<i64>,
) {
    let mut connection = node.kernel.inner.connection.lock().unwrap();
    let transaction = connection.transaction().unwrap();
    for ordinal in range {
        let fetched = received_at(ordinal);
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").unwrap(),
            NamespaceKey::try_new("tmdb.movie").unwrap(),
            "same-response-variant",
            Some(MetadataLocale::try_new("en-US").unwrap()),
            None,
            None,
            Sha256Digest::from_bytes(&[7; 32]),
        )
        .unwrap();
        let policy = (ordinal <= 0).then(|| {
            ProviderResponseCachePolicy::new(
                if ordinal == 0 {
                    fasti_application::ProviderResponseReuse::ValidateEveryReuse
                } else {
                    fasti_application::ProviderResponseReuse::Reusable
                },
                fetched.value(),
                std::time::Duration::ZERO,
                Some(std::time::Duration::from_secs(120)),
                None,
            )
            .to_canonical_json()
        });
        let restricted = ordinal == 0;
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record,
            field.clone(),
            "x".repeat(4096),
            provenance,
            fetched,
            (!restricted).then(|| fetched.value() + chrono::Duration::seconds(120)),
            if restricted {
                FieldClaimStatus::Stale
            } else {
                FieldClaimStatus::Fresh
            },
        )
        .unwrap();
        write_field_claim(
            &transaction,
            node.access.workspace_id(),
            record,
            field,
            &claim,
            CAP,
            RequestCorrelationId::new_v7(),
            policy.as_deref(),
        )
        .unwrap();
    }
    transaction.commit().unwrap();
}

fn known_policy_query_metrics(
    node: &TestNode,
    record: RecordId,
    field: &FieldKey,
) -> (usize, i32, i32) {
    let connection = node.kernel.inner.connection.lock().unwrap();
    let record_json = serde_json::to_string(&[record]).unwrap();
    let field_json = serde_json::to_string(&[field.as_str()]).unwrap();
    let mut plan = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {SELECT_KNOWN_FIELD_POLICIES}"))
        .unwrap();
    let plan_rows = plan
        .query_map(
            params![
                node.access.workspace_id().to_string(),
                record_json,
                field_json
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    let key_node = plan_rows
        .iter()
        .find(|(_, _, detail)| detail == "CO-ROUTINE k")
        .expect("policy key coroutine")
        .0;
    let inside_keys = |mut ancestor: i64| {
        while ancestor != 0 && ancestor != key_node {
            ancestor = plan_rows
                .iter()
                .find(|(id, _, _)| *id == ancestor)
                .expect("plan parent")
                .1;
        }
        ancestor == key_node
    };
    for (_, parent, detail) in &plan_rows {
        if detail.contains("TEMP B-TREE") {
            assert!(
                inside_keys(*parent),
                "no policy JSON/payload sort after k: {plan_rows:?}"
            );
        }
    }
    assert!(
        plan_rows
            .iter()
            .any(|(_, parent, detail)| inside_keys(*parent)
                && detail.contains("metadata_claim_provenance_recent_idx")
                && ["workspace_id=?", "record_id=?", "field_key=?"]
                    .iter()
                    .all(|part| detail.contains(part))),
        "history scan must retain exact indexed scope: {plan_rows:?}"
    );
    for alias in ["p", "registered", "c"] {
        assert!(
            plan_rows
                .iter()
                .any(|(_, parent, detail)| !inside_keys(*parent)
                    && detail.contains(&format!("SEARCH {alias} USING PRIMARY KEY"))),
            "selected policy payload lookup must be outside k: alias={alias}, {plan_rows:?}"
        );
    }
    // Inspect the actual SQL's key projections as well as the plan: EQP alone
    // does not enumerate the values carried through temporary sort records.
    let ranked = SELECT_KNOWN_FIELD_POLICIES
        .split("FROM json_each")
        .next()
        .unwrap();
    assert!(!ranked.contains("response_policy_json"));
    assert!(!ranked.contains("c.value"));
    assert!(SELECT_KNOWN_FIELD_POLICIES
        .contains("SELECT claim_id, record_id, field_key FROM ranked_keys"));
    let mut statement = connection.prepare(SELECT_KNOWN_FIELD_POLICIES).unwrap();
    let values = known_metadata_policies(
        &mut statement,
        params![
            node.access.workspace_id().to_string(),
            record_json,
            field_json
        ],
        CAP,
        RequestCorrelationId::new_v7(),
    )
    .unwrap()
    .collect::<ApplicationResult<Vec<_>>>()
    .unwrap();
    assert_eq!(
        values.len(),
        1,
        "latest known observation per complete variant"
    );
    assert_eq!(
        values[0].fetched_at,
        received_at(0).value(),
        "NULL rows cannot bury the restriction"
    );
    assert_eq!(
        values[0].policy,
        ProviderResponseCachePolicy::new(
            fasti_application::ProviderResponseReuse::ValidateEveryReuse,
            received_at(0).value(),
            std::time::Duration::ZERO,
            Some(std::time::Duration::from_secs(120)),
            None
        )
    );
    (
        values.len(),
        statement.get_status(rusqlite::StatementStatus::FullscanStep),
        statement.get_status(rusqlite::StatementStatus::VmStep),
    )
}

#[test]
fn metadata_known_policy_companion_indexes_full_history_and_sorts_only_keys() {
    let node = TestNode::new();
    let record = records(&node, 1)[0];
    let field = fields()[0].clone();
    known_policy_history(&node, record, &field, -1..257);
    let shallow = known_policy_query_metrics(&node, record, &field);
    known_policy_history(&node, record, &field, 257..4097);
    let deep = known_policy_query_metrics(&node, record, &field);
    assert_eq!((shallow.0, deep.0), (1, 1));
    assert!(
        deep.2 > shallow.2,
        "full-history policy discovery must actually inspect deeper indexed history"
    );
    eprintln!("metadata_known_policy sqlite={} null_history=256/4096 rows={}/{} fullscan_steps={}/{} vm_steps={}/{}",
        rusqlite::version(), shallow.0, deep.0, shallow.1, deep.1, shallow.2, deep.2);
}

#[test]
fn metadata_known_policy_stream_rejects_descending_scope_order() {
    let node = TestNode::new();
    let record = records(&node, 1)[0];
    let fields = fields();
    for field in &fields[..2] {
        known_policy_history(&node, record, field, 0..1);
    }
    let connection = node.kernel.inner.connection.lock().unwrap();
    let mut statement = connection
        .prepare(&format!(
            "SELECT * FROM ({SELECT_KNOWN_FIELD_POLICIES}) ORDER BY record_id DESC, field_key DESC"
        ))
        .unwrap();
    let mut stream = known_metadata_policies(
        &mut statement,
        params![
            node.access.workspace_id().to_string(),
            serde_json::to_string(&[record]).unwrap(),
            serde_json::to_string(&fields[..2].iter().map(FieldKey::as_str).collect::<Vec<_>>())
                .unwrap()
        ],
        CAP,
        RequestCorrelationId::new_v7(),
    )
    .unwrap();
    assert!(stream.next().unwrap().is_ok());
    let problem = stream
        .next()
        .unwrap()
        .err()
        .expect("descending scope must fail closed");
    assert_eq!(
        problem.code(),
        fasti_application::ProblemCode::IntegrityFailed
    );
    assert!(stream.next().is_none());
}

#[test]
fn metadata_batch_narrow_query_limits_history_and_sorts_only_selected_keys() {
    let node = TestNode::new();
    let records = records(&node, 603);
    let fields = fields();
    let selected = [records[550], records[602]];
    mixed_claims(&node, selected[0], &fields[3], 256);
    mixed_claims(&node, selected[1], &fields[1], 256);
    let shallow = bounded_query_metrics(&node, &selected);
    mixed_claim_range(&node, selected[0], &fields[3], 256..4096);
    let deep = bounded_query_metrics(&node, &selected);
    assert_eq!(shallow.0, 512);
    assert_eq!(
        deep.0, 512,
        "retain at most256 claims per selected Record/field"
    );
    assert_eq!(
        shallow.1, deep.1,
        "constant five-field relation scans must not grow with history depth"
    );
    eprintln!("metadata_batch source_history=256/4096 retained_rows={} fullscan_steps={}/{} vm_steps={}/{}", deep.0, shallow.1, deep.1, shallow.2, deep.2);
    assert_matches_single_record_owner(&node, &selected);
}

/// Historical source fixture: insert the same validated table shape as restore,
/// omitting unrelated substring-posting rebuild work from this metadata benchmark.
#[cfg(all(target_os = "linux", not(debug_assertions)))]
fn seed_dense_history(node: &TestNode, selected: &[RecordId], mixed_known: bool) {
    let workspace = node.access.workspace_id().to_string();
    let wide_value = "x".repeat(4096);
    let observed = now() - chrono::Duration::seconds(30);
    let digest = Sha256Digest::from_bytes(&[7; 32]);
    let observations = (0..256)
        .map(|ordinal| {
            let fetched = if mixed_known {
                observed + chrono::Duration::microseconds(ordinal)
            } else {
                received_at(ordinal).value()
            };
            let expires = mixed_known.then(|| timestamp(fetched + chrono::Duration::seconds(120)));
            let policy = (mixed_known && ordinal < 255).then(|| {
                ProviderResponseCachePolicy::new(
                    ProviderResponseReuse::Reusable,
                    fetched,
                    std::time::Duration::ZERO,
                    Some(std::time::Duration::from_secs(120)),
                    None,
                )
                .to_canonical_json()
            });
            (
                timestamp(fetched),
                expires,
                policy,
                // Exercise the maximum source-identifier width in key sorting.
                format!("{ordinal:0512}"),
            )
        })
        .collect::<Vec<_>>();
    let mut connection = node.kernel.inner.connection.lock().unwrap();
    let transaction = connection.transaction().unwrap();
    {
        let mut field_insert = transaction.prepare("INSERT INTO metadata_field_claims(workspace_id, record_id, field_key, source, value, locale, fetched_at, expires_at, created_at) VALUES (?1, ?2, ?3, 'tmdb.movie', ?4, NULL, ?5, ?6, ?5)").unwrap();
        let mut registered_insert = transaction.prepare("INSERT INTO metadata_claims(claim_id, workspace_id, record_id, claim_kind, created_at, response_policy_json) VALUES (?1, ?2, ?3, 'field', ?4, ?5)").unwrap();
        let mut provenance_insert = transaction.prepare("INSERT INTO metadata_claim_provenance(claim_id, workspace_id, record_id, field_key, source, fetched_at, provenance_state, initial_status, created_at, provider_id, source_record_id, evidence_digest) VALUES (?1, ?2, ?3, ?4, 'tmdb.movie', ?5, ?6, 'fresh', ?5, ?7, ?8, ?9)").unwrap();
        for record in selected {
            let record = record.to_string();
            for field in fields() {
                for (fetched, expires, policy, variant) in &observations {
                    let claim = MetadataClaimId::new_v7().to_string();
                    field_insert
                        .execute(params![
                            workspace,
                            record,
                            field.as_str(),
                            wide_value,
                            fetched,
                            expires
                        ])
                        .unwrap();
                    registered_insert
                        .execute(params![claim, workspace, record, fetched, policy])
                        .unwrap();
                    provenance_insert
                        .execute(params![
                            claim,
                            workspace,
                            record,
                            field.as_str(),
                            fetched,
                            if mixed_known {
                                "complete"
                            } else {
                                "legacy_incomplete"
                            },
                            mixed_known.then_some("tmdb"),
                            mixed_known.then_some(variant.as_str()),
                            mixed_known.then_some(digest.as_str())
                        ])
                        .unwrap();
                }
            }
        }
    }
    transaction.commit().unwrap();
}

#[cfg(all(target_os = "linux", not(debug_assertions)))]
fn resident_memory_bytes() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let read = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            * 1024
    };
    (read("VmRSS:"), read("VmHWM:"))
}

#[test]
#[ignore = "isolated Linux release dense metadata memory/latency evidence; run explicitly with --release --ignored --test-threads=1"]
fn metadata_batch_dense_release_memory_and_latency() {
    #[cfg(any(not(target_os = "linux"), debug_assertions))]
    panic!("run this isolated evidence fixture on Linux with --release");
    #[cfg(all(target_os = "linux", not(debug_assertions)))]
    {
        let count = match std::env::var("FASTI_METADATA_BATCH_RECORDS") {
            Err(std::env::VarError::NotPresent) => 100,
            Ok(value) if value == "100" => 100,
            Ok(value) if value == "500" => 500,
            other => panic!("FASTI_METADATA_BATCH_RECORDS accepts only 100 or 500: {other:?}"),
        };
        let mixed_known = match std::env::var("FASTI_METADATA_BATCH_MIXED_KNOWN") {
            Err(std::env::VarError::NotPresent) => false,
            Ok(value) if value == "0" => false,
            Ok(value) if value == "1" => true,
            other => panic!("FASTI_METADATA_BATCH_MIXED_KNOWN accepts only 0 or 1: {other:?}"),
        };
        let node = TestNode::new();
        let selected = records(&node, count);
        seed_dense_history(&node, &selected, mixed_known);
        let (rss_before, hwm_before) = resident_memory_bytes();
        let connection = node.kernel.inner.connection.lock().unwrap();
        if mixed_known {
            // Count admission without adding a separate GROUP BY/DISTINCT sorter
            // to the process high-water mark measured for the real reader.
            let counts = connection
                .query_row("SELECT SUM(response_policy_json IS NOT NULL),SUM(response_policy_json IS NULL) FROM metadata_claims", [], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                    ))
                })
                .unwrap();
            let groups = i64::try_from(count * 5).unwrap();
            assert_eq!(
                counts,
                (groups * 255, groups),
                "each generated field retains known policies plus its newest NULL row"
            );
        }
        let fields = fields();
        let mut elapsed = Vec::new();
        for _ in 0..5 {
            let started = std::time::Instant::now();
            let id = RequestCorrelationId::new_v7();
            let batch = load_record_metadata_batch(
                &connection,
                node.access.workspace_id(),
                node.access.profile_id(),
                &selected,
                &fields,
                CAP,
                id,
            )
            .unwrap();
            for record in &selected {
                for field in &fields {
                    assert_eq!(
                        batch
                            .resolve(*record, field, CAP, id)
                            .unwrap()
                            .value()
                            .unwrap()
                            .len(),
                        4096
                    );
                }
            }
            elapsed.push(started.elapsed());
        }
        let (rss_after, hwm_after) = resident_memory_bytes();
        elapsed.sort();
        eprintln!("metadata_batch records={count} mixed_known={mixed_known} fields=5 claims_per_field=256 value_bytes=4096 samples=5 median={:?} max={:?} rss_before={rss_before} rss_after={rss_after} hwm_before={hwm_before} hwm_after={hwm_after}", elapsed[2], elapsed[4]);
        assert!(
            hwm_after <= 192 * 1024 * 1024,
            "dense metadata process exceeded192MiB absolute ceiling: {hwm_after}"
        );
        // This dense-source fixture reports latency, not a replacement for the
        // separately qualified100-result/10,000-Record local Search p95 gate.
    }
}
