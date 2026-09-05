use crate::kernel::{authorize_application_transaction, map_sql, now, timestamp, SqliteKernel};
use crate::metadata::load_record_metadata_batch;
use fasti_application::{
    ApplicationResult, AttachIdentifierCommand, AttachIdentifierOutcome, CapabilityKey,
    CreateRecordCommand, CreateRecordOutcome, FastiProblem, IdentityPort, ListRecordsQuery,
    ProblemCode, RecordActivity, RecordIdentifier, RecordListView, RecordSummary,
    RegisterNamespaceDefinitionCommand, RegisterNamespaceDefinitionOutcome,
};
use fasti_domain::{
    ExternalIdentifierClaim, ExternalIdentifierId, FieldKey, Grain, InterpretationState,
    NamespaceKey, OccurredAt, ProfileId, RecordId, RecordStatus, WorkspaceId,
    ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY, POSTER_FIELD_KEY, RELEASE_YEAR_FIELD_KEY,
    TITLE_FIELD_KEY,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::{BTreeSet, HashMap};

/// Bound on one `list_records` page. Mirrors `review.rs`'s `MAX_REVIEW_PAGE`
/// pattern: no cursor yet, just a hard cap, matching the size a single local
/// library realistically needs before real pagination earns its keep.
///
/// ponytail: no cursor pagination. Add one if a real library exceeds this.
pub(crate) const MAX_RECORDS_PAGE: i64 = 500;

const SELECT_ACTIVE_RECORD_BY_ID: &str = r#"
    SELECT record_id, grain FROM records
    WHERE workspace_id = ?1 AND record_id = ?2 AND status = 'active'
"#;

pub(crate) fn register_namespace_tx(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    definition: &fasti_domain::NamespaceDefinition,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<bool> {
    let workspace_id = workspace_id.to_string();
    let namespace = definition.namespace().as_str();
    let supported_grains = definition
        .grains()
        .iter()
        .map(|grain| grain.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let existing = map_sql(
        transaction
            .query_row(
                r#"
                SELECT label, supported_grains, id_pattern, normalization, licence_posture
                FROM namespace_definitions
                WHERE workspace_id = ?1 AND namespace = ?2
                "#,
                params![workspace_id, namespace],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let expected = (
        definition.label(),
        supported_grains.as_str(),
        definition.id_pattern(),
        definition.normalization(),
        definition.licence_posture().as_str(),
    );
    let created = match existing {
        Some(existing)
            if (
                existing.0.as_str(),
                existing.1.as_str(),
                existing.2.as_str(),
                existing.3.as_str(),
                existing.4.as_str(),
            ) == expected =>
        {
            false
        }
        Some(_) => {
            return Err(Box::new(FastiProblem::from_code(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            )))
        }
        None => {
            map_sql(
                transaction.execute(
                    r#"
                    INSERT INTO namespace_definitions(
                        workspace_id, namespace, label, supported_grains, id_pattern,
                        normalization, licence_posture, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                    "#,
                    params![
                        workspace_id,
                        namespace,
                        definition.label(),
                        supported_grains,
                        definition.id_pattern(),
                        definition.normalization(),
                        definition.licence_posture().as_str(),
                        timestamp(now())
                    ],
                ),
                capability,
                correlation_id,
            )?;
            true
        }
    };
    Ok(created)
}

impl IdentityPort for SqliteKernel {
    fn register_namespace_definition(
        &self,
        command: RegisterNamespaceDefinitionCommand,
    ) -> ApplicationResult<RegisterNamespaceDefinitionOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::RegisterNamespace;
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            command.access(),
            correlation_id,
        )?;
        let definition = command.definition();
        let created = register_namespace_tx(
            &transaction,
            authorized.workspace_id(),
            definition,
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(RegisterNamespaceDefinitionOutcome::new(
            definition.namespace().clone(),
            created,
        ))
    }

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
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            command.access(),
            correlation_id,
        )?;
        let record_id = insert_record(
            &transaction,
            authorized.workspace_id(),
            command.grain(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(CreateRecordOutcome::new(
            authorized.workspace_id(),
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
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            command.access(),
            correlation_id,
        )?;
        let outcome = attach_identifier_tx(
            &transaction,
            authorized.workspace_id(),
            command.record_id(),
            command.claim(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(outcome)
    }

    fn list_records(&self, query: ListRecordsQuery) -> ApplicationResult<RecordListView> {
        let capability = CapabilityKey::ListRecords;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            query.access(),
            correlation_id,
        )?;

        let workspace_id = authorized.workspace_id();
        let profile_id = authorized.profile_id();
        let selector = query.record_id().map_err(|_| {
            Box::new(
                FastiProblem::validation_failed(
                    capability,
                    correlation_id,
                    vec![fasti_application::Violation::try_new(
                        "invalid_record_selector",
                        "/query/record_id",
                        "Record selector is invalid",
                        "an optional canonical rec_ UUIDv7",
                    )
                    .expect("store-owned violation")],
                )
                .expect("one bounded violation"),
            )
        })?;
        let (sql, selector) = match selector {
            Some(record_id) => (
                SELECT_ACTIVE_RECORD_BY_ID,
                rusqlite::types::Value::Text(record_id.to_string()),
            ),
            None => (
                r#"
                SELECT record_id, grain FROM records
                WHERE workspace_id = ?1 AND status = 'active'
                ORDER BY record_id
                LIMIT ?2
                "#,
                rusqlite::types::Value::Integer(MAX_RECORDS_PAGE + 1),
            ),
        };
        let mut statement = map_sql(transaction.prepare(sql), capability, correlation_id)?;
        let rows = map_sql(
            statement.query_map(params![workspace_id.to_string(), selector], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            }),
            capability,
            correlation_id,
        )?;
        let mut records = Vec::new();
        for row in rows {
            let (record_id, grain) = map_sql(row, capability, correlation_id)?;
            let record_id = record_id.parse::<RecordId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?;
            let grain = grain.parse::<Grain>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?;
            records.push((record_id, grain));
        }
        drop(statement);

        let truncated = records.len() > MAX_RECORDS_PAGE as usize;
        records.truncate(MAX_RECORDS_PAGE as usize);

        let summaries = load_record_summaries(
            &transaction,
            workspace_id,
            profile_id,
            records,
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(RecordListView::new(summaries, truncated))
    }
}

pub(crate) fn load_record_summaries(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: fasti_domain::ProfileId,
    records: Vec<(RecordId, Grain)>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Vec<RecordSummary>> {
    let record_ids: Vec<_> = records.iter().map(|(id, _)| *id).collect();
    let summaries = load_record_summary_fields(
        connection,
        workspace_id,
        profile_id,
        records,
        capability,
        correlation_id,
    )?;
    let (mut identifiers, _) = load_record_identifiers_batch(
        connection,
        workspace_id,
        &record_ids,
        capability,
        correlation_id,
        None,
    )?
    .expect("an unbounded identifier read cannot exhaust a supplied budget");
    Ok(summaries
        .into_iter()
        .map(|summary| {
            let identifiers = identifiers.remove(&summary.record_id()).unwrap_or_default();
            summary.with_identifiers(identifiers)
        })
        .collect())
}

/// Intermediate fields only. Callers must complete identifiers before publishing
/// a RecordSummary; Search can first discard nonmatching title projections.
pub(crate) fn load_record_summary_fields(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: fasti_domain::ProfileId,
    records: Vec<(RecordId, Grain)>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Vec<RecordSummary>> {
    let field_keys = [
        FieldKey::try_new(TITLE_FIELD_KEY).expect("canonical record summary field key"),
        FieldKey::try_new(POSTER_FIELD_KEY).expect("canonical record summary field key"),
        FieldKey::try_new(ORIGINAL_TITLE_FIELD_KEY).expect("canonical record summary field key"),
        FieldKey::try_new(OVERVIEW_FIELD_KEY).expect("canonical record summary field key"),
        FieldKey::try_new(RELEASE_YEAR_FIELD_KEY).expect("canonical record summary field key"),
    ];
    let record_ids: Vec<_> = records.iter().map(|(id, _)| *id).collect();
    let metadata = load_record_metadata_batch(
        connection,
        workspace_id,
        profile_id,
        &record_ids,
        &field_keys,
        capability,
        correlation_id,
    )?;
    let mut activities = load_latest_activities_batch(
        connection,
        workspace_id,
        profile_id,
        &record_ids,
        capability,
        correlation_id,
    )?;

    let mut summaries = Vec::with_capacity(records.len());
    for (record_id, grain) in records {
        summaries.push(RecordSummary::new(
            record_id,
            grain,
            RecordStatus::Active,
            metadata.resolve(record_id, &field_keys[0], capability, correlation_id)?,
            metadata.resolve(record_id, &field_keys[1], capability, correlation_id)?,
            metadata.resolve(record_id, &field_keys[2], capability, correlation_id)?,
            metadata.resolve(record_id, &field_keys[3], capability, correlation_id)?,
            metadata.resolve(record_id, &field_keys[4], capability, correlation_id)?,
            Vec::new(),
            activities.remove(&record_id),
        ));
    }
    Ok(summaries)
}

fn parse_interpretation_state(
    value: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<InterpretationState> {
    match value {
        "unresolved" => Ok(InterpretationState::Unresolved),
        "resolved" => Ok(InterpretationState::Resolved),
        "conflicted" => Ok(InterpretationState::Conflicted),
        _ => Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        ))),
    }
}

pub(crate) fn selected_record_ids_json(
    record_ids: &[RecordId],
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<String> {
    if record_ids.len() > MAX_RECORDS_PAGE as usize {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    serde_json::to_string(record_ids)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

pub(crate) const SELECT_RECORD_IDENTIFIERS: &str = r#"
    WITH page_records AS (
        SELECT record_id FROM records
        WHERE workspace_id = ?1 AND status = 'active'
          AND record_id IN (SELECT value FROM json_each(?2))
    )
    SELECT identifier.record_id, identifier.namespace,
           identifier.grain, identifier.value
    FROM external_identifiers identifier
    JOIN page_records page ON page.record_id = identifier.record_id
    WHERE identifier.workspace_id = ?1
"#;

/// Compact serde_json string length, without allocating or copying SQLite text.
pub(crate) fn json_string_bytes(value: &str) -> usize {
    value.bytes().fold(2usize, |size, byte| {
        size.saturating_add(match byte {
            b'"' | b'\\' | b'\n' | b'\r' | b'\t' | 8 | 12 => 2,
            0..=31 => 6,
            _ => 1,
        })
    })
}

type RecordIdentifierBatch = (HashMap<RecordId, Vec<RecordIdentifier>>, usize);

pub(crate) fn load_record_identifiers_batch(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_ids: &[RecordId],
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
    budget: Option<usize>,
) -> ApplicationResult<Option<RecordIdentifierBatch>> {
    let record_ids_json = selected_record_ids_json(record_ids, capability, correlation_id)?;
    // Bounded readers must see rows before any unbounded SQLite value sort.
    // Establish the same canonical order in memory only after complete admission.
    let sql = if budget.is_some() {
        SELECT_RECORD_IDENTIFIERS.to_owned()
    } else {
        format!("{SELECT_RECORD_IDENTIFIERS} ORDER BY identifier.record_id, identifier.namespace, identifier.grain, identifier.value")
    };
    let mut statement = map_sql(connection.prepare_cached(&sql), capability, correlation_id)?;
    let mut rows = map_sql(
        statement.query(params![workspace_id.to_string(), record_ids_json]),
        capability,
        correlation_id,
    )?;
    let mut identifiers: HashMap<RecordId, Vec<RecordIdentifier>> = HashMap::new();
    let mut bytes = 0usize;
    while let Some(row) = map_sql(rows.next(), capability, correlation_id)? {
        // Borrow SQLite text before allocating, charging actual escaped strings
        // plus object keys/separators and a comma. Do not charge ASCII as six
        // bytes: that incorrectly rejects complete Records which fit the page.
        let size = (1..=3).try_fold(
            r#"{"namespace":,"grain":,"value":},"#.len(),
            |size, column| {
                let value = row.get_ref(column)?.as_str()?;
                Ok::<_, rusqlite::Error>(size.saturating_add(json_string_bytes(value)))
            },
        );
        bytes = bytes.saturating_add(map_sql(size, capability, correlation_id)?);
        if budget.is_some_and(|budget| bytes > budget) {
            return Ok(None);
        }
        let (record_id, namespace, grain, value): (String, String, String, String) = map_sql(
            (|| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))(),
            capability,
            correlation_id,
        )?;
        let record_id = record_id
            .parse::<RecordId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let namespace = NamespaceKey::try_new(namespace)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let grain = grain
            .parse::<Grain>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        identifiers
            .entry(record_id)
            .or_default()
            .push(RecordIdentifier::new(namespace, grain, value));
    }
    if budget.is_some() {
        for values in identifiers.values_mut() {
            values.sort_unstable_by(|a, b| {
                (a.namespace().as_str(), a.grain().as_str(), a.value()).cmp(&(
                    b.namespace().as_str(),
                    b.grain().as_str(),
                    b.value(),
                ))
            });
        }
    }
    Ok(Some((identifiers, bytes)))
}

fn load_latest_activities_batch(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    record_ids: &[RecordId],
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<HashMap<RecordId, RecordActivity>> {
    let record_ids_json = selected_record_ids_json(record_ids, capability, correlation_id)?;
    let mut statement = map_sql(
        connection.prepare(
            r#"
            WITH page_records AS (
                SELECT record_id FROM records
                WHERE workspace_id = ?1 AND status = 'active'
                  AND record_id IN (SELECT value FROM json_each(?2))
            ), latest_occurrences AS (
                SELECT occurrence.record_id, occurrence.occurrence_id,
                       occurrence.occurred_at_json,
                       ROW_NUMBER() OVER (
                           PARTITION BY occurrence.record_id
                           ORDER BY occurrence.created_at DESC,
                                    occurrence.occurrence_id DESC
                       ) AS occurrence_rank
                FROM occurrences occurrence
                JOIN page_records page ON page.record_id = occurrence.record_id
                WHERE occurrence.workspace_id = ?1 AND occurrence.profile_id = ?3
            ), latest_interpretations AS (
                SELECT interpretation.occurrence_id, interpretation.state,
                       ROW_NUMBER() OVER (
                           PARTITION BY interpretation.occurrence_id
                           ORDER BY interpretation.created_at DESC,
                                    interpretation.interpretation_id DESC
                       ) AS interpretation_rank
                FROM interpretations interpretation
                JOIN latest_occurrences occurrence
                  ON occurrence.occurrence_id = interpretation.occurrence_id
                 AND occurrence.occurrence_rank = 1
            )
            SELECT occurrence.record_id, occurrence.occurred_at_json,
                   interpretation.state
            FROM latest_occurrences occurrence
            JOIN latest_interpretations interpretation
              ON interpretation.occurrence_id = occurrence.occurrence_id
             AND interpretation.interpretation_rank = 1
            WHERE occurrence.occurrence_rank = 1
            ORDER BY occurrence.record_id
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                record_ids_json,
                profile_id.to_string()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut activities = HashMap::new();
    for row in rows {
        let (record_id, occurred_at, state) = map_sql(row, capability, correlation_id)?;
        let record_id = record_id
            .parse::<RecordId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let occurred_at = occurred_at
            .map(|value| {
                serde_json::from_str::<OccurredAt>(&value).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })
            })
            .transpose()?;
        let activity = RecordActivity::new(
            occurred_at,
            parse_interpretation_state(&state, capability, correlation_id)?,
        );
        if activities.insert(record_id, activity).is_some() {
            return Err(Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            )));
        }
    }
    Ok(activities)
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
    value
        .parse::<Grain>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
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

    let declared_grains = map_sql(
        transaction
            .query_row(
                r#"
                SELECT supported_grains FROM namespace_definitions
                WHERE workspace_id = ?1 AND namespace = ?2
                "#,
                params![workspace_id.to_string(), claim.namespace()],
                |row| row.get::<_, String>(0),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    if !declared_grains.is_some_and(|grains| {
        grains
            .split(',')
            .any(|grain| grain == claim.grain().as_str())
    }) {
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
        let identifier = identifier
            .parse::<ExternalIdentifierId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let existing_record = existing_record
            .parse::<RecordId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
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
    Ok(AttachIdentifierOutcome::new(identifier_id, record_id, true))
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
            value
                .parse::<RecordId>()
                .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::write_field_claim;
    use crate::test_support::TestNode;
    use chrono::{TimeZone, Utc};
    use fasti_application::{
        provider_identity_mapping, AcceptObservationCommand, CreateProviderRecordCommand,
        IdentityPort, ListRecordsQuery, ObservationAcceptancePort, ProviderMetadataField,
        ProviderMetadataPort, RegisterNamespaceDefinitionCommand, GOOGLE_BOOKS_PROVIDER_ID,
        TMDB_PROVIDER_ID,
    };
    use fasti_domain::{
        ClaimedTrust, FieldClaim, FieldResolutionTier, NamespaceDefinition, NamespaceKey,
        NamespaceLicencePosture, ObservationResolution, ObservedAt, OperationId, ReceivedAt,
        RequestCorrelationId, TITLE_FIELD_KEY,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn definition(namespace: &str, grains: impl IntoIterator<Item = Grain>) -> NamespaceDefinition {
        NamespaceDefinition::try_new(
            namespace,
            format!("{namespace} test namespace"),
            grains,
            ".+",
            "identity",
            NamespaceLicencePosture::Unknown,
        )
        .expect("valid test namespace definition")
    }

    #[test]
    fn attachment_requires_a_workspace_namespace_for_the_claim_grain() {
        let node = TestNode::new();
        let record = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Release,
            ))
            .expect("create record")
            .record_id();
        let claim = ExternalIdentifierClaim::try_new("imdb", Grain::Release, "tt0903747")
            .expect("valid claim syntax");

        let error = node
            .kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                claim.clone(),
            ))
            .expect_err("undeclared namespace must fail");
        assert_eq!(error.code(), ProblemCode::InvalidIdentifier);

        let registered = node
            .kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                definition("imdb", [Grain::Release]),
            ))
            .expect("register namespace");
        assert!(registered.created());
        assert_eq!(registered.namespace().as_str(), "imdb");

        let film_record = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create film record")
            .record_id();
        let film_claim = ExternalIdentifierClaim::try_new("imdb", Grain::Film, "tt0903747")
            .expect("valid claim syntax");
        let error = node
            .kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                film_record,
                film_claim,
            ))
            .expect_err("undeclared grain must fail");
        assert_eq!(error.code(), ProblemCode::InvalidIdentifier);

        node.kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                claim,
            ))
            .expect("attach through registered namespace");
    }

    #[test]
    fn registration_is_idempotent_but_cannot_redefine_a_key() {
        let node = TestNode::new();
        let command = || {
            RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                definition("tmdb_tv", [Grain::Series]),
            )
        };
        assert!(node
            .kernel
            .register_namespace_definition(command())
            .expect("first registration")
            .created());
        assert!(!node
            .kernel
            .register_namespace_definition(command())
            .expect("idempotent registration")
            .created());

        let error = node
            .kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                definition("tmdb_tv", [Grain::Film]),
            ))
            .expect_err("same key cannot change comparison space");
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
    }

    fn list(node: &TestNode) -> Vec<RecordSummary> {
        node.kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("list records")
            .into_records()
    }

    fn exact_record(
        node: &TestNode,
        access: fasti_application::RequestAccessContext,
        record_id: RecordId,
    ) -> ApplicationResult<RecordListView> {
        node.kernel.list_records(
            ListRecordsQuery::new(RequestCorrelationId::new_v7(), access).with_record_id(record_id),
        )
    }

    #[test]
    fn exact_record_selector_enriches_beyond_500_and_preserves_profile_projection() {
        let node = TestNode::new();
        {
            let mut connection = node.kernel.inner.connection.lock().unwrap();
            let tx = connection.transaction().unwrap();
            for _ in 0..MAX_RECORDS_PAGE {
                insert_record(
                    &tx,
                    node.access.workspace_id(),
                    Grain::Film,
                    CapabilityKey::CreateRecord,
                    RequestCorrelationId::new_v7(),
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }
        let (record_id, identifier) = observed_record(&node);
        let other = node.add_profile_with_scopes(&[
            fasti_application::ScopeKey::IdentityRead,
            fasti_application::ScopeKey::ObservationAccept,
        ]);
        let first_time =
            accept_record_activity(&node, node.access, &identifier, "2026-08-23T10:30:00Z");
        let other_time = accept_record_activity(&node, other, &identifier, "2026-08-24T10:30:00Z");
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            let title = FieldKey::try_new(TITLE_FIELD_KEY).unwrap();
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &title,
                &FieldClaim::try_new(ns("tmdb"), "Shared title", None, received(100), None)
                    .unwrap(),
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
                None,
            )
            .unwrap();
            let override_ = fasti_domain::ProfileFieldOverride::try_new(
                other.profile_id(),
                record_id,
                title,
                "Other profile title",
                received(200),
            )
            .unwrap();
            crate::metadata::write_profile_field_override(
                &connection,
                node.access.workspace_id(),
                &override_,
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
        }
        let default = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .unwrap();
        assert!(default.truncated());
        let default = default.into_records();
        assert_eq!(default.len(), MAX_RECORDS_PAGE as usize);
        assert!(!default.iter().any(|record| record.record_id() == record_id));
        for (access, title, time) in [
            (node.access, "Shared title", first_time),
            (other, "Other profile title", other_time),
        ] {
            let selected = exact_record(&node, access, record_id).unwrap();
            assert!(!selected.truncated());
            let selected = selected.into_records();
            assert_eq!(selected.len(), 1);
            let record = &selected[0];
            assert_eq!(record.record_id(), record_id);
            assert_eq!(record.grain(), Grain::Film);
            assert_eq!(record.title().value(), Some(title));
            assert_eq!(record.identifiers().len(), 1);
            assert_eq!(record.identifiers()[0].value(), identifier.value());
            assert_eq!(record.latest_activity().unwrap().occurred_at(), Some(&time));
        }
    }

    #[test]
    fn exact_record_selector_is_nonenumerating_and_keeps_identity_authorization() {
        let node = TestNode::new();
        let local = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .unwrap()
            .record_id();
        let other_node = TestNode::new();
        let foreign = other_node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                other_node.access,
                Grain::Film,
            ))
            .unwrap()
            .record_id();
        // Put a foreign workspace's existing Record in this database so this
        // proves workspace isolation, not merely an unknown identifier.
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            connection
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![
                        other_node.access.workspace_id().to_string(),
                        timestamp(now())
                    ],
                )
                .unwrap();
            connection.execute("INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, 'film', 'active', ?3)",
                params![foreign.to_string(), other_node.access.workspace_id().to_string(), timestamp(now())]).unwrap();
        }
        let selected = exact_record(&node, node.access, local).unwrap();
        assert!(!selected.truncated());
        let selected = selected.into_records();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].title().tier(), FieldResolutionTier::Empty);
        assert!(selected[0].identifiers().is_empty());
        assert!(selected[0].latest_activity().is_none());
        for id in [RecordId::new_v7(), foreign] {
            let selected = exact_record(&node, node.access, id).unwrap();
            assert!(!selected.truncated());
            assert!(selected.into_records().is_empty());
        }
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            // Published Records admit only active status. Inject an invalid
            // historical row solely to prove the selector's active predicate;
            // this does not introduce a new Record lifecycle.
            connection
                .pragma_update(None, "ignore_check_constraints", true)
                .unwrap();
            connection
                .execute(
                    "UPDATE records SET status = 'inactive' WHERE record_id = ?1",
                    [local.to_string()],
                )
                .unwrap();
            connection
                .pragma_update(None, "ignore_check_constraints", false)
                .unwrap();
        }
        let selected = exact_record(&node, node.access, local).unwrap();
        assert!(!selected.truncated());
        assert!(selected.into_records().is_empty());
        let denied = node.add_profile_with_scopes(&[]);
        for (access, expected) in [
            (node.access, ProblemCode::ValidationFailed),
            (denied, ProblemCode::Forbidden),
        ] {
            let error = node
                .kernel
                .list_records(
                    ListRecordsQuery::new(RequestCorrelationId::new_v7(), access)
                        .with_record_selector(Err(fasti_application::InvalidRecordSelector)),
                )
                .err()
                .expect("invalid selector requires authorization first");
            assert_eq!(error.code(), expected);
        }
        for id in [local, foreign, RecordId::new_v7()] {
            assert_eq!(
                exact_record(&node, denied, id)
                    .err()
                    .expect("identity scope required")
                    .code(),
                ProblemCode::Forbidden
            );
        }
    }

    #[test]
    fn exact_record_selector_uses_primary_key_and_constant_cold_select_preparations_at_10000_records(
    ) {
        let node = TestNode::new();
        let selected = RecordId::new_v7();
        let selects = Arc::new(AtomicUsize::new(0));
        {
            let counter = Arc::clone(&selects);
            node.kernel
                .inner
                .connection
                .lock()
                .unwrap()
                .authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
                    if matches!(context.action, rusqlite::hooks::AuthAction::Select) {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                    rusqlite::hooks::Authorization::Allow
                }))
                .unwrap();
        }
        let mut previous = 0;
        let mut counts = Vec::new();
        for total in [0, 100, 10_000] {
            {
                let mut connection = node.kernel.inner.connection.lock().unwrap();
                let tx = connection.transaction().unwrap();
                {
                    let mut insert = tx.prepare("INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, 'film', 'active', ?3)").unwrap();
                    for index in previous..total {
                        let id = if index == 0 {
                            selected
                        } else {
                            RecordId::new_v7()
                        };
                        insert
                            .execute(params![
                                id.to_string(),
                                node.access.workspace_id().to_string(),
                                timestamp(now())
                            ])
                            .unwrap();
                    }
                }
                tx.commit().unwrap();
            }
            previous = total;
            // SQLite's authorizer counts preparation, not execution. Compare
            // cold preparations so statement reuse cannot hide query growth.
            node.kernel
                .inner
                .connection
                .lock()
                .unwrap()
                .flush_prepared_statement_cache();
            selects.store(0, Ordering::Relaxed);
            let page = exact_record(&node, node.access, selected).unwrap();
            assert!(!page.truncated());
            assert_eq!(page.into_records().len(), usize::from(total != 0));
            counts.push(selects.load(Ordering::Relaxed));
        }
        assert!(counts[0] > 0);
        assert_eq!(
            counts,
            vec![counts[0]; 3],
            "exact selection must not add cold per-record SELECT preparations"
        );
        assert!(counts[0] <= 40, "same bounded enrichment owner: {counts:?}");
        let connection = node.kernel.inner.connection.lock().unwrap();
        let mut explain = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {SELECT_ACTIVE_RECORD_BY_ID}"))
            .unwrap();
        let plan = explain
            .query_map(
                params![node.access.workspace_id().to_string(), selected.to_string()],
                |row| row.get::<_, String>(3),
            )
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|step| step.contains("SEARCH records") && step.contains("record_id=?")),
            "indexed exact lookup: {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|step| step.contains("SCAN records") || step.contains("TEMP B-TREE")),
            "no scan or sort: {plan:?}"
        );
        let mut query = connection.prepare(SELECT_ACTIVE_RECORD_BY_ID).unwrap();
        {
            let mut rows = query
                .query(params![
                    node.access.workspace_id().to_string(),
                    selected.to_string()
                ])
                .unwrap();
            assert!(rows.next().unwrap().is_some());
            assert!(rows.next().unwrap().is_none());
        }
        assert_eq!(query.get_status(rusqlite::StatementStatus::FullscanStep), 0);
        assert_eq!(query.get_status(rusqlite::StatementStatus::Sort), 0);
    }

    #[test]
    fn record_page_reports_truncation() {
        let node = TestNode::new();
        let mut connection =
            rusqlite::Connection::open(node.kernel.database_path()).expect("open test database");
        let transaction = connection.transaction().expect("start seed transaction");
        for _ in 0..=MAX_RECORDS_PAGE {
            let record_id = RecordId::new_v7();
            transaction
                .execute(
                    "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, 'film', 'active', '2026-08-28T00:00:00Z')",
                    params![record_id.to_string(), node.access.workspace_id().to_string()],
                )
                .expect("seed record");
        }
        transaction.commit().expect("commit seed transaction");

        let page = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("list records");
        assert!(page.truncated());
        assert_eq!(page.into_records().len(), MAX_RECORDS_PAGE as usize);
    }

    #[test]
    fn record_page_uses_a_constant_number_of_selects() {
        let node = TestNode::new();
        let mut connection =
            rusqlite::Connection::open(node.kernel.database_path()).expect("open test database");
        let transaction = connection.transaction().expect("start seed transaction");
        for _ in 0..MAX_RECORDS_PAGE {
            transaction
                .execute(
                    "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, 'film', 'active', '2026-08-28T00:00:00Z')",
                    params![RecordId::new_v7().to_string(), node.access.workspace_id().to_string()],
                )
                .expect("seed record");
        }
        transaction.commit().expect("commit seed transaction");

        let selects = Arc::new(AtomicUsize::new(0));
        {
            let counter = Arc::clone(&selects);
            node.kernel
                .inner
                .connection
                .lock()
                .expect("kernel connection")
                .authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
                    if matches!(context.action, rusqlite::hooks::AuthAction::Select) {
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                    rusqlite::hooks::Authorization::Allow
                }))
                .expect("install select counter");
        }

        assert_eq!(list(&node).len(), MAX_RECORDS_PAGE as usize);
        let select_count = selects.load(Ordering::Relaxed);
        assert!(
            select_count <= 40,
            "record listing regressed to per-record queries: {select_count} SELECTs"
        );
    }

    fn ns(value: &str) -> NamespaceKey {
        NamespaceKey::try_new(value).expect("valid namespace")
    }

    fn received(seconds: i64) -> ReceivedAt {
        ReceivedAt::from_application_clock(Utc.timestamp_opt(seconds, 0).single().expect("instant"))
    }

    #[test]
    fn a_local_only_record_with_no_claims_is_a_valid_empty_row() {
        let node = TestNode::new();
        let record_id = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create record")
            .record_id();

        let summaries = list(&node);
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.record_id(), record_id);
        assert_eq!(summary.grain(), Grain::Film);
        assert_eq!(summary.title().tier(), FieldResolutionTier::Empty);
        assert_eq!(summary.title().value(), None);
        assert_eq!(summary.poster().tier(), FieldResolutionTier::Empty);
        assert!(summary.latest_activity().is_none());
    }

    #[test]
    fn claims_from_multiple_providers_resolve_to_the_freshest_tier() {
        let node = TestNode::new();
        let record_id = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create record")
            .record_id();
        let key = FieldKey::try_new(TITLE_FIELD_KEY).expect("valid field key");
        let older = FieldClaim::try_new(ns("tvdb"), "Older Title", None, received(100), None)
            .expect("valid claim");
        let newer = FieldClaim::try_new(ns("tmdb"), "Newer Title", None, received(200), None)
            .expect("valid claim");

        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            for claim in [&older, &newer] {
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record_id,
                    &key,
                    claim,
                    CapabilityKey::ListRecords,
                    RequestCorrelationId::new_v7(),
                    None,
                )
                .expect("write claim");
            }
        }

        let summaries = list(&node);
        assert_eq!(summaries.len(), 1);
        let title = summaries[0].title();
        // Neither claim declares an expiry, so both are fresh and the most
        // recently fetched one wins the fallback tier -- exactly what
        // `resolve_field` proves in fasti-domain; this proves the store
        // wiring feeds it the real persisted claim set.
        assert_eq!(title.tier(), FieldResolutionTier::FallbackProviderClaim);
        assert_eq!(title.value(), Some("Newer Title"));
    }

    #[test]
    fn a_record_with_a_resolved_occurrence_reports_its_latest_activity() {
        let node = TestNode::new();
        node.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                definition("tmdb", [Grain::Film]),
            ))
            .expect("register namespace");
        let record_id = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create record")
            .record_id();
        let claim = ExternalIdentifierClaim::try_new("tmdb", Grain::Film, "42")
            .expect("valid claim syntax");
        node.kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                claim.clone(),
            ))
            .expect("attach identifier");

        let evidence = node.upload(b"an observation touching the record above");
        let observed_at = ObservedAt::parse("2026-08-23T10:30:00Z", ClaimedTrust::DeviceObserved)
            .expect("observed_at");
        let command = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            OperationId::new_v7(),
            None,
            observed_at,
            evidence,
        )
        .with_identity_clues(vec![claim], Some(Grain::Film));
        node.kernel
            .authorize_and_accept(command)
            .expect("accept observation resolving to the record");

        let summaries = list(&node);
        assert_eq!(summaries.len(), 1);
        let activity = summaries[0]
            .latest_activity()
            .expect("occurrence produced activity");
        assert_eq!(
            activity.interpretation_state(),
            InterpretationState::Resolved
        );
    }

    fn observed_record(node: &TestNode) -> (RecordId, ExternalIdentifierClaim) {
        node.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                definition("tmdb", [Grain::Film]),
            ))
            .expect("register namespace");
        let record = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create record")
            .record_id();
        let identifier =
            ExternalIdentifierClaim::try_new("tmdb", Grain::Film, "42").expect("identifier");
        node.kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                identifier.clone(),
            ))
            .expect("attach identifier");
        (record, identifier)
    }

    fn accept_record_activity(
        node: &TestNode,
        access: fasti_application::RequestAccessContext,
        identifier: &ExternalIdentifierClaim,
        time: &str,
    ) -> OccurredAt {
        let occurred = OccurredAt::parse(time, ClaimedTrust::SourceClaim).expect("occurred time");
        let evidence = node.upload_for(access, time.as_bytes());
        node.kernel
            .authorize_and_accept(
                AcceptObservationCommand::new(
                    RequestCorrelationId::new_v7(),
                    access,
                    OperationId::new_v7(),
                    Some(occurred.clone()),
                    ObservedAt::parse(time, ClaimedTrust::DeviceObserved).expect("observed time"),
                    evidence,
                )
                .with_identity_clues(vec![identifier.clone()], Some(Grain::Film)),
            )
            .expect("accept activity");
        occurred
    }

    #[test]
    fn record_activity_is_owned_by_the_authorized_profile() {
        let node = TestNode::new();
        let second = node.add_profile_with_scopes(&[
            fasti_application::ScopeKey::IdentityRead,
            fasti_application::ScopeKey::ObservationAccept,
        ]);
        let (record, identifier) = observed_record(&node);
        let first_time =
            accept_record_activity(&node, node.access, &identifier, "2026-08-23T10:30:00Z");
        let page = |access| {
            node.kernel
                .list_records(ListRecordsQuery::new(
                    RequestCorrelationId::new_v7(),
                    access,
                ))
                .expect("authorized record list")
                .into_records()
        };
        assert!(page(second)[0].latest_activity().is_none());
        let second_time =
            accept_record_activity(&node, second, &identifier, "2026-08-24T10:30:00Z");
        for (access, expected) in [(node.access, &first_time), (second, &second_time)] {
            let records = page(access);
            assert_eq!(records[0].record_id(), record);
            assert_eq!(
                records[0]
                    .latest_activity()
                    .expect("profile activity")
                    .occurred_at(),
                Some(expected)
            );
        }
        accept_record_activity(&node, node.access, &identifier, "2026-08-25T10:30:00Z");
        assert_eq!(
            page(second)[0]
                .latest_activity()
                .expect("second profile activity")
                .occurred_at(),
            Some(&second_time)
        );
    }

    #[test]
    fn selected_record_enrichment_keeps_sparse_ids_beyond_the_first_page() {
        let node = TestNode::new();
        {
            let mut connection = node.kernel.inner.connection.lock().expect("connection");
            let transaction = connection.transaction().expect("seed transaction");
            for _ in 0..MAX_RECORDS_PAGE {
                insert_record(
                    &transaction,
                    node.access.workspace_id(),
                    Grain::Film,
                    CapabilityKey::CreateRecord,
                    RequestCorrelationId::new_v7(),
                )
                .expect("seed record");
            }
            transaction.commit().expect("seed Records");
        }
        let (selected, identifier) = observed_record(&node);
        let time = accept_record_activity(&node, node.access, &identifier, "2026-08-23T10:30:00Z");
        assert!(!list(&node)
            .iter()
            .any(|record| record.record_id() == selected));
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();
        let ids = |workspace, records: &[RecordId]| {
            load_record_identifiers_batch(
                &connection,
                workspace,
                records,
                capability,
                correlation_id,
                None,
            )
            .map(|value| value.expect("unbounded fixture read").0)
        };
        let activities = |workspace, records: &[RecordId]| {
            load_latest_activities_batch(
                &connection,
                workspace,
                node.access.profile_id(),
                records,
                capability,
                correlation_id,
            )
        };
        let selection = [selected, selected, RecordId::new_v7()];
        let identifiers =
            ids(node.access.workspace_id(), &selection).expect("selected identifiers");
        assert_eq!(identifiers.len(), 1);
        assert_eq!(identifiers[&selected].len(), 1);
        assert_eq!(identifiers[&selected][0].value(), "42");
        let activity =
            activities(node.access.workspace_id(), &selection).expect("selected activity");
        assert_eq!(activity.len(), 1);
        assert_eq!(activity[&selected].occurred_at(), Some(&time));
        for selection in [&[][..], &selection[..]] {
            assert!(ids(WorkspaceId::new_v7(), selection)
                .expect("foreign workspace")
                .is_empty());
            assert!(activities(WorkspaceId::new_v7(), selection)
                .expect("foreign workspace")
                .is_empty());
        }
        assert!(ids(node.access.workspace_id(), &[])
            .expect("empty IDs")
            .is_empty());
        assert!(activities(node.access.workspace_id(), &[])
            .expect("empty activities")
            .is_empty());
        let oversized = vec![selected; MAX_RECORDS_PAGE as usize + 1];
        assert_eq!(
            ids(node.access.workspace_id(), &oversized)
                .expect_err("bounded IDs")
                .code(),
            ProblemCode::IntegrityFailed
        );
        assert_eq!(
            activities(node.access.workspace_id(), &oversized)
                .expect_err("bounded activities")
                .code(),
            ProblemCode::IntegrityFailed
        );
    }

    #[test]
    fn provider_created_records_reuse_the_same_google_books_and_tmdb_coordinates_as_ingest() {
        let node = TestNode::new();
        let observed_at = ObservedAt::parse("2026-08-23T10:30:00Z", ClaimedTrust::DeviceObserved)
            .expect("observed_at");

        for (provider, kind, value) in [
            (GOOGLE_BOOKS_PROVIDER_ID, "book", "book-1"),
            (TMDB_PROVIDER_ID, "movie", "42"),
            (TMDB_PROVIDER_ID, "show", "42"),
        ] {
            let mapping = provider_identity_mapping(provider, kind).expect("provider mapping");
            node.kernel
                .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    mapping.namespace_definition().expect("provider namespace"),
                ))
                .expect("register provider namespace");
            let source = NamespaceKey::try_new(mapping.namespace()).expect("provider source");
            let fetched = received(100);
            let policy = fasti_application::ProviderResponseCachePolicy::new(
                fasti_application::ProviderResponseReuse::Reusable,
                fetched.value(),
                std::time::Duration::ZERO,
                None,
                None,
            );
            let field = ProviderMetadataField::new(
                FieldKey::try_new(TITLE_FIELD_KEY).expect("title field"),
                FieldClaim::try_new_unbound_provider(
                    fasti_domain::MetadataClaimId::new_v7(),
                    format!("{provider} {kind}"),
                    fasti_domain::FieldClaimProvenance::try_new(
                        fasti_domain::MetadataProviderId::try_new(provider).unwrap(),
                        source,
                        mapping.identifier(value).unwrap().value(),
                        None,
                        None,
                        None,
                        fasti_domain::Sha256Digest::from_bytes(&[7; 32]),
                    )
                    .unwrap(),
                    fetched,
                    Some(
                        fetched.value()
                            + chrono::Duration::seconds(fasti_domain::METADATA_FRESH_SECONDS),
                    ),
                    fasti_domain::FieldClaimStatus::Fresh,
                )
                .expect("provider field"),
            );
            let created = node
                .kernel
                .create_provider_record(CreateProviderRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    mapping.grain(),
                    mapping.identifier(value).expect("provider identifier"),
                    vec![field],
                    policy,
                ))
                .expect("create provider record");
            let evidence = node.upload(format!("{provider}:{kind}:{value}").as_bytes());
            let accepted = node
                .kernel
                .authorize_and_accept(
                    AcceptObservationCommand::new(
                        RequestCorrelationId::new_v7(),
                        node.access,
                        OperationId::new_v7(),
                        None,
                        observed_at.clone(),
                        evidence,
                    )
                    .with_identity_clues(
                        vec![mapping.identifier(value).expect("ingest identifier")],
                        Some(mapping.grain()),
                    ),
                )
                .expect("accept provider-coordinate observation");
            assert_eq!(
                accepted.receipt().resolution(),
                ObservationResolution::Resolved
            );
            assert_eq!(accepted.receipt().record_id(), Some(created.record_id()));
        }
    }
}
