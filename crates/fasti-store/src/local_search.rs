//! Rebuildable substring postings. Metadata resolution remains the authority.

use crate::identity::load_record_summaries;
use crate::kernel::{authorize_application_transaction, map_sql};
use crate::SqliteKernel;
use fasti_application::{
    normalize_local_search_text, ApplicationResult, CapabilityKey, FastiProblem, LocalSearchCursor,
    LocalSearchPage, LocalSearchRequest,
};
use fasti_domain::{Grain, RecordId, ORIGINAL_TITLE_FIELD_KEY, TITLE_FIELD_KEY};
use rusqlite::{params, Connection, TransactionBehavior};
use std::collections::{BTreeMap, BTreeSet};

const CAPABILITY: CapabilityKey = CapabilityKey::SearchMetadata;
const PAGE_SIZE: usize = 100;

pub(crate) fn searchable_field(field: &str) -> bool {
    matches!(field, TITLE_FIELD_KEY | ORIGINAL_TITLE_FIELD_KEY)
}

/// One native index handles literal 1/2-character queries as well as substrings.
/// Public postings are a superset of immutable claims, never a second projection.
pub(crate) fn index_text(
    connection: &Connection,
    workspace: &str,
    profile: &str,
    record: &str,
    value: &str,
) -> rusqlite::Result<()> {
    let text: Vec<_> = normalize_local_search_text(value).chars().collect();
    let mut grams = BTreeSet::new();
    for size in 1..=3.min(text.len()) {
        for window in text.windows(size) {
            grams.insert(window.iter().collect::<String>());
        }
    }
    let mut statement = connection.prepare_cached(
        "INSERT OR IGNORE INTO local_search_grams(workspace_id, profile_partition, gram, record_id) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for gram in grams {
        statement.execute(params![workspace, profile, gram, record])?;
    }
    Ok(())
}

/// Called inside the override owner's transaction. Clearing one field retains
/// postings supported by its sibling field, without retaining deleted private text.
pub(crate) fn reindex_overrides(
    connection: &Connection,
    workspace: &str,
    profile: &str,
    record: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM local_search_grams WHERE workspace_id = ?1 AND profile_partition = ?2 AND record_id = ?3",
        params![workspace, profile, record],
    )?;
    let mut statement = connection.prepare(
        "SELECT value FROM metadata_profile_field_overrides WHERE workspace_id = ?1 AND profile_id = ?2 AND record_id = ?3 AND field_key IN ('core.title', 'core.original_title')",
    )?;
    let rows = statement.query_map(params![workspace, profile, record], |row| {
        row.get::<_, String>(0)
    })?;
    for row in rows {
        index_text(connection, workspace, profile, record, &row?)?;
    }
    Ok(())
}

/// Migration/restore callers supply their transaction. No source row or archive
/// stream is changed; a failed rebuild rolls back with its enclosing operation.
pub(crate) fn rebuild(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM local_search_grams", [])?;
    for sql in [
        "SELECT workspace_id, '', record_id, value FROM metadata_field_claims WHERE field_key IN ('core.title', 'core.original_title') ORDER BY record_id, field_key, source, fetched_at",
        "SELECT workspace_id, profile_id, record_id, value FROM metadata_profile_field_overrides WHERE field_key IN ('core.title', 'core.original_title') ORDER BY workspace_id, profile_id, record_id, field_key",
    ] {
        let mut statement = connection.prepare(sql)?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)))?;
        for row in rows {
            let (workspace, profile, record, value) = row?;
            index_text(connection, &workspace, &profile, &record, &value)?;
        }
    }
    Ok(())
}

const SELECT_CANDIDATES: &str = "
    SELECT posting.record_id, record.grain
    FROM (
        SELECT workspace_id, record_id FROM local_search_grams
        WHERE workspace_id = ?1 AND profile_partition = ?2
          AND gram = ?3 AND record_id > ?4
        ORDER BY record_id LIMIT 101
    ) posting
    JOIN records record INDEXED BY records_workspace_record_idx
      ON record.record_id = posting.record_id AND record.workspace_id = posting.workspace_id";

pub(crate) fn search(
    kernel: &SqliteKernel,
    request: &LocalSearchRequest,
) -> ApplicationResult<LocalSearchPage> {
    let id = request.correlation_id;
    let mut connection = kernel.lock_connection(CAPABILITY, id)?;
    let transaction = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Deferred),
        CAPABILITY,
        id,
    )?;
    let access = authorize_application_transaction(&transaction, CAPABILITY, &request.access, id)?;
    if request.grains.len() > Grain::ALL.len() {
        return Err(Box::new(FastiProblem::from_code(
            fasti_application::ProblemCode::ValidationFailed,
            CAPABILITY,
            id,
        )));
    }
    let context = request.context_digest(&access);
    if request
        .after
        .as_ref()
        .is_some_and(|after| after.context_digest != context)
    {
        return Err(Box::new(FastiProblem::from_code(
            fasti_application::ProblemCode::ValidationFailed,
            CAPABILITY,
            id,
        )));
    }
    let needle = normalize_local_search_text(request.query.as_str());
    let anchor: String = needle.chars().take(3).collect();
    let after = request
        .after
        .as_ref()
        .map(|cursor| cursor.last_record_id.to_string())
        .unwrap_or_default();
    let mut candidates = BTreeMap::new();
    let mut statement = map_sql(transaction.prepare(SELECT_CANDIDATES), CAPABILITY, id)?;
    // Two independent indexed ranges avoid a workspace-wide OR/UNION sort.
    for partition in [String::new(), access.profile_id().to_string()] {
        let rows = map_sql(
            statement.query_map(
                params![access.workspace_id().to_string(), partition, anchor, after],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ),
            CAPABILITY,
            id,
        )?;
        for row in rows {
            let (record, grain) = map_sql(row, CAPABILITY, id)?;
            let record_id = record
                .parse::<RecordId>()
                .map_err(|_| Box::new(FastiProblem::integrity_failed(CAPABILITY, id)))?;
            let grain = grain
                .parse::<Grain>()
                .map_err(|_| Box::new(FastiProblem::integrity_failed(CAPABILITY, id)))?;
            candidates.insert(record, (record_id, grain));
        }
    }
    drop(statement);
    let more = candidates.len() > PAGE_SIZE;
    let mut candidates: Vec<_> = candidates.into_values().take(PAGE_SIZE).collect();
    let next = if more {
        candidates.last().map(|(record, _)| LocalSearchCursor {
            last_record_id: *record,
            context_digest: context,
        })
    } else {
        None
    };
    // Filter after the inspected-ID bound; a sparse grain must not scan all postings.
    candidates.retain(|(_, grain)| request.grains.is_empty() || request.grains.contains(grain));
    let mut records = load_record_summaries(
        &transaction,
        access.workspace_id(),
        access.profile_id(),
        candidates,
        CAPABILITY,
        id,
    )?;
    records.retain(|record| {
        [record.title(), record.original_title()]
            .into_iter()
            .any(|field| {
                field
                    .value()
                    .is_some_and(|value| normalize_local_search_text(value).contains(&needle))
            })
    });
    map_sql(transaction.commit(), CAPABILITY, id)?;
    Ok(LocalSearchPage { records, next })
}

#[cfg(test)]
#[path = "local_search_tests.rs"]
mod tests;
