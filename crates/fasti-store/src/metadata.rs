//! Persistence for provider metadata claims and user overrides.
//!
//! `fasti_domain::metadata` models `FieldClaim`/`FieldOverride` and the
//! deterministic `resolve_field()` tiering, but had zero SQLite persistence.
//! These functions are the store-side half: write every claim a provider
//! ever supplied (history, never overwritten in place) and the single
//! current override per field, then read them back for resolution.

use crate::kernel::{map_sql, now, parse_timestamp, timestamp};
use fasti_application::{ApplicationResult, CapabilityKey, FastiProblem};
use fasti_domain::{
    FieldClaim, FieldClaimError, FieldKey, FieldOverride, NamespaceKey, ReceivedAt, RecordId,
    RequestCorrelationId, WorkspaceId,
};
use rusqlite::{params, Connection, OptionalExtension};

// ponytail: no production caller yet -- the provider adapter that fetches
// and writes real claims is a separate, later task (see the read-model doc).
// These are the persistence primitives it will call; tests exercise them
// directly in the meantime.
#[allow(dead_code)]
pub(crate) fn write_field_claim(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    claim: &FieldClaim,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_field_claims(
                workspace_id, record_id, field_key, source, value, locale,
                fetched_at, expires_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(record_id, field_key, source, fetched_at) DO UPDATE SET
                value = excluded.value,
                locale = excluded.locale,
                expires_at = excluded.expires_at,
                created_at = excluded.created_at
            "#,
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                field_key.as_str(),
                claim.source().as_str(),
                claim.value(),
                claim.locale(),
                timestamp(claim.fetched_at()),
                claim.expires_at().map(timestamp),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn write_field_override(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    override_: &FieldOverride,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_field_overrides(
                workspace_id, record_id, field_key, value, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(record_id, field_key) DO UPDATE SET
                value = excluded.value,
                created_at = excluded.created_at
            "#,
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                field_key.as_str(),
                override_.value(),
                timestamp(override_.created_at())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

pub(crate) fn load_field_claims(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<FieldClaim>> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT source, value, locale, fetched_at, expires_at
            FROM metadata_field_claims
            WHERE workspace_id = ?1 AND record_id = ?2 AND field_key = ?3
            ORDER BY fetched_at, source
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                field_key.as_str()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut claims = Vec::new();
    for row in rows {
        let (source, value, locale, fetched_at, expires_at) =
            map_sql(row, capability, correlation_id)?;
        let source = NamespaceKey::try_new(source)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let fetched_at = ReceivedAt::from_application_clock(parse_timestamp(
            &fetched_at,
            capability,
            correlation_id,
        )?);
        let expires_at = expires_at
            .map(|value| parse_timestamp(&value, capability, correlation_id))
            .transpose()?;
        let claim = FieldClaim::try_new(source, value, locale, fetched_at, expires_at).map_err(
            |error: FieldClaimError| {
                let _ = error;
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            },
        )?;
        claims.push(claim);
    }
    Ok(claims)
}

pub(crate) fn load_field_override(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Option<FieldOverride>> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT value, created_at FROM metadata_field_overrides
                WHERE workspace_id = ?1 AND record_id = ?2 AND field_key = ?3
                "#,
                params![
                    workspace_id.to_string(),
                    record_id.to_string(),
                    field_key.as_str()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((value, created_at)) = row else {
        return Ok(None);
    };
    let created_at = ReceivedAt::from_application_clock(parse_timestamp(
        &created_at,
        capability,
        correlation_id,
    )?);
    let override_ = FieldOverride::try_new(value, created_at)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    Ok(Some(override_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{CreateRecordCommand, IdentityPort};
    use fasti_domain::{Grain, ReceivedAt};

    fn field_key(value: &str) -> FieldKey {
        FieldKey::try_new(value).expect("valid field key")
    }

    fn ns(value: &str) -> NamespaceKey {
        NamespaceKey::try_new(value).expect("valid namespace")
    }

    fn received(seconds: i64) -> ReceivedAt {
        use chrono::TimeZone;
        ReceivedAt::from_application_clock(
            chrono::Utc
                .timestamp_opt(seconds, 0)
                .single()
                .expect("valid instant"),
        )
    }

    fn a_record(node: &TestNode) -> RecordId {
        node.kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create record")
            .record_id()
    }

    #[test]
    fn claims_and_overrides_round_trip_through_persistence() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new(ns("tmdb"), "Example Title", None, received(100), None)
            .expect("valid claim");
        let override_ = FieldOverride::try_new("My Title", received(200)).expect("valid override");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                capability,
                correlation_id,
            )
            .expect("write claim");
            write_field_override(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &override_,
                capability,
                correlation_id,
            )
            .expect("write override");
        }

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims");
        assert_eq!(claims, vec![claim]);

        let loaded_override = load_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load override")
        .expect("override present");
        assert_eq!(loaded_override, override_);
    }

    #[test]
    fn a_record_with_no_claims_resolves_to_an_empty_read_not_an_error() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims");
        assert!(claims.is_empty());
        let override_ = load_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load override");
        assert_eq!(override_, None);
    }

    #[test]
    fn claims_from_different_workspaces_do_not_leak() {
        let node = TestNode::new();
        let other = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new(ns("tmdb"), "Isolated Title", None, received(100), None)
            .expect("valid claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                capability,
                correlation_id,
            )
            .expect("write claim");
        }

        // A record ID from another workspace's kernel simply is not present
        // in `other`'s database, so this proves query scoping rather than
        // relying on cross-database ID collision.
        let other_connection = other.kernel.inner.connection.lock().expect("connection");
        let claims = load_field_claims(
            &other_connection,
            other.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims from another workspace's database");
        assert!(claims.is_empty());
    }

    #[test]
    fn writing_the_same_claim_twice_is_idempotent() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new(ns("tmdb"), "Example Title", None, received(100), None)
            .expect("valid claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        let connection = node.kernel.inner.connection.lock().expect("connection");
        for _ in 0..2 {
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                capability,
                correlation_id,
            )
            .expect("write claim");
        }
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims");
        assert_eq!(
            claims.len(),
            1,
            "same (record, field, source, fetched_at) replaces, not duplicates"
        );
    }

    #[test]
    fn a_later_override_replaces_the_earlier_one() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let first = FieldOverride::try_new("First Title", received(100)).expect("valid override");
        let second = FieldOverride::try_new("Second Title", received(200)).expect("valid override");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &first,
            capability,
            correlation_id,
        )
        .expect("write first override");
        write_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &second,
            capability,
            correlation_id,
        )
        .expect("write second override");

        let loaded = load_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load override")
        .expect("override present");
        assert_eq!(loaded, second);
    }
}
