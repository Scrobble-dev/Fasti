use crate::kernel::{authorize_transaction, map_sql, now, timestamp, SqliteKernel};
use fasti_application::{
    ApplicationResult, AttachIdentifierCommand, AttachIdentifierOutcome, CapabilityKey,
    CreateRecordCommand, CreateRecordOutcome, FastiProblem, IdentityPort,
};
use fasti_domain::{
    ExternalIdentifierClaim, ExternalIdentifierId, Grain, RecordId, WorkspaceId,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::BTreeSet;

impl IdentityPort for SqliteKernel {
    fn create_record(
        &self,
        command: CreateRecordCommand,
    ) -> ApplicationResult<CreateRecordOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::CreateRecord;
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let record_id = insert_record(
            &transaction,
            command.access().workspace_id(),
            command.grain(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(CreateRecordOutcome::new(
            command.access().workspace_id(),
            record_id,
            command.grain(),
        ))
    }

    fn attach_identifier(
        &self,
        command: AttachIdentifierCommand,
    ) -> ApplicationResult<AttachIdentifierOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::AttachIdentifier;
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let outcome = attach_identifier_tx(
            &transaction,
            command.access().workspace_id(),
            command.record_id(),
            command.claim(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(outcome)
    }
}

pub(crate) fn insert_record(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    grain: Grain,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<RecordId> {
    let record_id = RecordId::new_v7();
    map_sql(
        transaction.execute(
            r#"
            INSERT INTO records(record_id, workspace_id, grain, status, created_at)
            VALUES (?1, ?2, ?3, 'active', ?4)
            "#,
            params![
                record_id.to_string(),
                workspace_id.to_string(),
                grain.as_str(),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(record_id)
}

pub(crate) fn load_record_grain(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Grain> {
    let value = map_sql(
        connection
            .query_row(
                r#"
                SELECT grain FROM records
                WHERE workspace_id = ?1 AND record_id = ?2 AND status = 'active'
                "#,
                params![workspace_id.to_string(), record_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some(value) = value else {
        return Err(Box::new(FastiProblem::record_not_found(
            capability,
            correlation_id,
        )));
    };
    value.parse::<Grain>().map_err(|_| {
        Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        ))
    })
}

pub(crate) fn attach_identifier_tx(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    claim: &ExternalIdentifierClaim,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<AttachIdentifierOutcome> {
    let record_grain = load_record_grain(
        transaction,
        workspace_id,
        record_id,
        capability,
        correlation_id,
    )?;
    if record_grain != claim.grain() {
        return Err(Box::new(FastiProblem::invalid_identifier(
            capability,
            correlation_id,
        )));
    }

    let existing = map_sql(
        transaction
            .query_row(
                r#"
                SELECT external_identifier_id, record_id
                FROM external_identifiers
                WHERE workspace_id = ?1
                  AND namespace = ?2
                  AND grain = ?3
                  AND value = ?4
                "#,
                params![
                    workspace_id.to_string(),
                    claim.namespace(),
                    claim.grain().as_str(),
                    claim.value()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    if let Some((identifier, existing_record)) = existing {
        let identifier = identifier.parse::<ExternalIdentifierId>().map_err(|_| {
            Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            ))
        })?;
        let existing_record = existing_record.parse::<RecordId>().map_err(|_| {
            Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            ))
        })?;
        if existing_record == record_id {
            return Ok(AttachIdentifierOutcome::new(
                identifier,
                existing_record,
                false,
            ));
        }
        return Err(Box::new(FastiProblem::identity_conflict(
            capability,
            correlation_id,
        )));
    }

    let identifier_id = ExternalIdentifierId::new_v7();
    map_sql(
        transaction.execute(
            r#"
            INSERT INTO external_identifiers(
                external_identifier_id, workspace_id, record_id,
                namespace, grain, value, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                identifier_id.to_string(),
                workspace_id.to_string(),
                record_id.to_string(),
                claim.namespace(),
                claim.grain().as_str(),
                claim.value(),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(AttachIdentifierOutcome::new(
        identifier_id,
        record_id,
        true,
    ))
}

pub(crate) fn matching_record_ids(
    connection: &Connection,
    workspace_id: WorkspaceId,
    claims: &[ExternalIdentifierClaim],
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Vec<RecordId>> {
    let mut records = BTreeSet::new();
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT record_id FROM external_identifiers
            WHERE workspace_id = ?1
              AND namespace = ?2
              AND grain = ?3
              AND value = ?4
            "#,
        ),
        capability,
        correlation_id,
    )?;
    for claim in claims {
        let record = map_sql(
            statement
                .query_row(
                    params![
                        workspace_id.to_string(),
                        claim.namespace(),
                        claim.grain().as_str(),
                        claim.value()
                    ],
                    |row| row.get::<_, String>(0),
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        if let Some(record) = record {
            records.insert(record);
        }
    }
    records
        .into_iter()
        .map(|value| {
            value.parse::<RecordId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(
                    capability,
                    correlation_id,
                ))
            })
        })
        .collect()
}
