use super::*;
use crate::identity::{attach_identifier_tx, register_namespace_tx};
use fasti_application::MAX_LOCAL_SEARCH_RESPONSE_BYTES;
use fasti_domain::{
    ExternalIdentifierClaim, NamespaceDefinition, NamespaceLicencePosture, OVERVIEW_FIELD_KEY,
    POSTER_FIELD_KEY,
};
use rusqlite::trace::{TraceEvent, TraceEventCodes};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex, MutexGuard,
};

static TRACE_SERIAL: Mutex<()> = Mutex::new(());
static IDENTIFIER_SELECT_EXECUTIONS: AtomicUsize = AtomicUsize::new(0);

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn trace_identifier_select(event: TraceEvent<'_>) {
    let TraceEvent::Profile(statement, _) = event else {
        return;
    };
    let sql = statement.sql();
    let sql = sql.trim_start();
    if sql.starts_with("WITH page_records AS (")
        && sql.contains("FROM external_identifiers identifier")
    {
        IDENTIFIER_SELECT_EXECUTIONS.fetch_add(1, Ordering::Relaxed);
    }
}

struct IdentifierTraceGuard<'a> {
    connection: &'a Mutex<rusqlite::Connection>,
    _serial: MutexGuard<'static, ()>,
}

impl Drop for IdentifierTraceGuard<'_> {
    fn drop(&mut self) {
        lock_unpoisoned(self.connection).trace_v2(TraceEventCodes::empty(), None);
    }
}

fn count_identifier_selects<T>(node: &TestNode, operation: impl FnOnce() -> T) -> (T, usize) {
    let serial = lock_unpoisoned(&TRACE_SERIAL);
    IDENTIFIER_SELECT_EXECUTIONS.store(0, Ordering::Relaxed);
    node.kernel.inner.connection.lock().unwrap().trace_v2(
        TraceEventCodes::SQLITE_TRACE_PROFILE,
        Some(trace_identifier_select),
    );
    let guard = IdentifierTraceGuard {
        connection: &node.kernel.inner.connection,
        _serial: serial,
    };
    let result = operation();
    let count = IDENTIFIER_SELECT_EXECUTIONS.load(Ordering::Relaxed);
    drop(guard);
    (result, count)
}

fn identifier_value(record: RecordId, ordinal: usize) -> String {
    let prefix = format!("{record}:{ordinal:08}:");
    format!("{prefix}{}", "x".repeat(256 - prefix.len()))
}

fn attach_identifiers(node: &TestNode, records: &[RecordId], per_record: usize) {
    let mut connection = node.kernel.inner.connection.lock().unwrap();
    let transaction = connection.transaction().unwrap();
    let definition = NamespaceDefinition::try_new(
        "query-count",
        "Local Search query-count fixture",
        [Grain::Film],
        ".+",
        "identity",
        NamespaceLicencePosture::Unknown,
    )
    .unwrap();
    register_namespace_tx(
        &transaction,
        node.access.workspace_id(),
        &definition,
        CapabilityKey::RegisterNamespace,
        RequestCorrelationId::new_v7(),
    )
    .unwrap();
    for record in records {
        for ordinal in 0..per_record {
            let claim = ExternalIdentifierClaim::try_new(
                "query-count",
                Grain::Film,
                identifier_value(*record, ordinal),
            )
            .unwrap();
            assert!(attach_identifier_tx(
                &transaction,
                node.access.workspace_id(),
                *record,
                &claim,
                CapabilityKey::AttachIdentifier,
                RequestCorrelationId::new_v7(),
            )
            .unwrap()
            .created());
        }
    }
    transaction.commit().unwrap();
}

#[test]
fn full_identifier_populated_page_executes_one_identifier_select() {
    let node = node();
    let records = seed(&node, 100, "identifier query needle");
    attach_identifiers(&node, &records, 1);

    let (page, identifier_selects) = count_identifier_selects(&node, || {
        node.kernel
            .search_local_records(&request(node.access, "identifier query needle"))
            .unwrap()
    });

    assert_eq!(page.records.len(), 100);
    assert!(page.next.is_none());
    assert!(page
        .records
        .iter()
        .all(|record| record.identifiers().len() == 1));
    assert_eq!(
        identifier_selects, 1,
        "one bounded page must execute one batched identifier statement"
    );

    let (cached_page, cached_identifier_selects) = count_identifier_selects(&node, || {
        node.kernel
            .search_local_records(&request(node.access, "identifier query needle"))
            .unwrap()
    });
    assert_eq!(cached_page.records.len(), 100);
    assert!(cached_page
        .records
        .iter()
        .all(|record| record.identifiers().len() == 1));
    assert_eq!(
        cached_identifier_selects, 1,
        "prepared-cache reuse must still execute the batch exactly once"
    );
}

#[test]
fn empty_local_search_executes_no_identifier_select() {
    let node = node();
    let records = seed(&node, 1, "present title");
    attach_identifiers(&node, &records, 1);

    let (page, identifier_selects) = count_identifier_selects(&node, || {
        node.kernel
            .search_local_records(&request(node.access, "absent title"))
            .unwrap()
    });

    assert!(page.records.is_empty());
    assert!(page.next.is_none());
    assert_eq!(identifier_selects, 0);
}

#[test]
fn identifier_overflow_finds_a_complete_record_boundary_in_at_most_eight_selects() {
    const IDENTIFIERS_PER_RECORD: usize = 128;

    let node = node();
    let mut records = seed(&node, 100, "overflow needle");
    records.sort_by_key(ToString::to_string);
    let large = format!(
        "overflow needle {}",
        "x".repeat(4096 - "overflow needle ".len())
    );
    let fields = [
        TITLE_FIELD_KEY,
        ORIGINAL_TITLE_FIELD_KEY,
        OVERVIEW_FIELD_KEY,
        POSTER_FIELD_KEY,
    ];
    for chunk in records.chunks(20) {
        configure(
            &node,
            node.access,
            chunk
                .iter()
                .flat_map(|record| fields.iter().map(|field| set(*record, field, &large)))
                .collect(),
        );
    }
    attach_identifiers(&node, &records, IDENTIFIERS_PER_RECORD);

    let (page, identifier_selects) = count_identifier_selects(&node, || {
        node.kernel
            .search_local_records(&request(node.access, "overflow needle"))
            .unwrap()
    });

    assert!(
        (1..100).contains(&page.records.len()),
        "fixture must exercise a non-empty overflow boundary"
    );
    assert!(page.next.is_some(), "overflow must retain a continuation");
    assert!(
        page.records
            .iter()
            .all(|record| record.identifiers().len() == IDENTIFIERS_PER_RECORD),
        "the final Record must never contain a partial identifier vector"
    );
    let returned: Vec<_> = page
        .records
        .iter()
        .map(|record| record.record_id())
        .collect();
    assert_eq!(returned, records[..returned.len()]);
    assert_eq!(
        page.next.as_ref().map(|next| next.last_record_id),
        returned.last().copied()
    );
    let response = fasti_contracts::LocalSearchResponseDto {
        records: page.records.into_iter().map(Into::into).collect(),
        next: page.next.map(Into::into),
    };
    let wire = serde_json::to_vec(&response).unwrap();
    assert!(wire.len() <= MAX_LOCAL_SEARCH_RESPONSE_BYTES);
    assert!(
        (2..=8).contains(&identifier_selects),
        "overflow admission must use bounded logarithmic retries, got {identifier_selects}"
    );
}
