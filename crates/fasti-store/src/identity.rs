use crate::kernel::{authorize_transaction, map_sql, now, timestamp, SqliteKernel};
use crate::metadata::{load_field_claims, load_field_override};
use fasti_application::{
    ApplicationResult, AttachIdentifierCommand, AttachIdentifierOutcome, CapabilityKey,
    CreateRecordCommand, CreateRecordOutcome, FastiProblem, IdentityPort, ListRecordsQuery,
    ProblemCode, RecordActivity, RecordIdentifier, RecordListView, RecordSummary,
    RegisterNamespaceDefinitionCommand, RegisterNamespaceDefinitionOutcome,
};
use fasti_domain::{
    resolve_field, ExternalIdentifierClaim, ExternalIdentifierId, FieldKey, Grain,
    InterpretationState, NamespaceKey, OccurredAt, RecordId, RecordStatus, WorkspaceId,
    ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY, POSTER_FIELD_KEY, RELEASE_YEAR_FIELD_KEY,
    TITLE_FIELD_KEY,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::collections::BTreeSet;

/// Bound on one `list_records` page. Mirrors `review.rs`'s `MAX_REVIEW_PAGE`
/// pattern: no cursor yet, just a hard cap, matching the size a single local
/// library realistically needs before real pagination earns its keep.
///
/// ponytail: no cursor pagination. Add one if a real library exceeds this.
const MAX_RECORDS_PAGE: i64 = 500;

impl IdentityPort for SqliteKernel {
    fn register_namespace_definition(
        &self,
        command: RegisterNamespaceDefinitionCommand,
    ) -> ApplicationResult<RegisterNamespaceDefinitionOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::AttachIdentifier;
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let definition = command.definition();
        let workspace_id = command.access().workspace_id().to_string();
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

    fn list_records(&self, query: ListRecordsQuery) -> ApplicationResult<RecordListView> {
        let capability = CapabilityKey::ListRecords;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;

        let workspace_id = query.access().workspace_id();
        let mut statement = map_sql(
            transaction.prepare(
                r#"
                SELECT record_id, grain FROM records
                WHERE workspace_id = ?1 AND status = 'active'
                ORDER BY record_id
                LIMIT ?2
                "#,
            ),
            capability,
            correlation_id,
        )?;
        let rows = map_sql(
            statement.query_map(
                params![workspace_id.to_string(), MAX_RECORDS_PAGE + 1],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ),
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

        let mut summaries = Vec::with_capacity(records.len());
        for (record_id, grain) in records {
            summaries.push(load_record_summary(
                &transaction,
                workspace_id,
                record_id,
                grain,
                capability,
                correlation_id,
            )?);
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(RecordListView::new(summaries, truncated))
    }
}

fn resolved_field(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<fasti_domain::ResolvedField> {
    let claims = load_field_claims(
        connection,
        workspace_id,
        record_id,
        field_key,
        capability,
        correlation_id,
    )?;
    let override_ = load_field_override(
        connection,
        workspace_id,
        record_id,
        field_key,
        capability,
        correlation_id,
    )?;
    // No preferred-provider configuration exists yet (that is a profile
    // preference this task does not build); resolve_field degrades cleanly
    // to fallback/last-known-good/empty tiers without one.
    Ok(resolve_field(
        override_.as_ref(),
        &claims,
        None,
        None,
        now(),
    ))
}

fn load_latest_activity(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Option<RecordActivity>> {
    let occurrence = map_sql(
        connection
            .query_row(
                r#"
                SELECT occurrence_id, occurred_at_json FROM occurrences
                WHERE workspace_id = ?1 AND record_id = ?2
                ORDER BY created_at DESC, occurrence_id DESC
                LIMIT 1
                "#,
                params![workspace_id.to_string(), record_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((occurrence_id, occurred_at_json)) = occurrence else {
        return Ok(None);
    };
    let occurred_at = occurred_at_json
        .map(|value| {
            serde_json::from_str::<OccurredAt>(&value)
                .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
        })
        .transpose()?;
    let state = map_sql(
        connection
            .query_row(
                r#"
                SELECT state FROM interpretations
                WHERE occurrence_id = ?1
                ORDER BY created_at DESC, interpretation_id DESC
                LIMIT 1
                "#,
                [occurrence_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let interpretation_state = match state {
        Some(value) => parse_interpretation_state(&value, capability, correlation_id)?,
        None => return Ok(None),
    };
    Ok(Some(RecordActivity::new(occurred_at, interpretation_state)))
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

fn load_record_summary(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    grain: Grain,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<RecordSummary> {
    let title_key = FieldKey::try_new(TITLE_FIELD_KEY)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let poster_key = FieldKey::try_new(POSTER_FIELD_KEY)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let original_title_key = FieldKey::try_new(ORIGINAL_TITLE_FIELD_KEY)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let overview_key = FieldKey::try_new(OVERVIEW_FIELD_KEY)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let release_year_key = FieldKey::try_new(RELEASE_YEAR_FIELD_KEY)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let title = resolved_field(
        connection,
        workspace_id,
        record_id,
        &title_key,
        capability,
        correlation_id,
    )?;
    let poster = resolved_field(
        connection,
        workspace_id,
        record_id,
        &poster_key,
        capability,
        correlation_id,
    )?;
    let original_title = resolved_field(
        connection,
        workspace_id,
        record_id,
        &original_title_key,
        capability,
        correlation_id,
    )?;
    let overview = resolved_field(
        connection,
        workspace_id,
        record_id,
        &overview_key,
        capability,
        correlation_id,
    )?;
    let release_year = resolved_field(
        connection,
        workspace_id,
        record_id,
        &release_year_key,
        capability,
        correlation_id,
    )?;
    let identifiers = load_record_identifiers(
        connection,
        workspace_id,
        record_id,
        capability,
        correlation_id,
    )?;
    let latest_activity = load_latest_activity(
        connection,
        workspace_id,
        record_id,
        capability,
        correlation_id,
    )?;
    Ok(RecordSummary::new(
        record_id,
        grain,
        RecordStatus::Active,
        title,
        poster,
        original_title,
        overview,
        release_year,
        identifiers,
        latest_activity,
    ))
}

fn load_record_identifiers(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Vec<RecordIdentifier>> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT namespace, grain, value FROM external_identifiers
            WHERE workspace_id = ?1 AND record_id = ?2
            ORDER BY namespace, grain, value
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![workspace_id.to_string(), record_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut identifiers = Vec::new();
    for row in rows {
        let (namespace, grain, value) = map_sql(row, capability, correlation_id)?;
        let namespace = NamespaceKey::try_new(namespace)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let grain = grain
            .parse::<Grain>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        identifiers.push(RecordIdentifier::new(namespace, grain, value));
    }
    Ok(identifiers)
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
            let field = ProviderMetadataField::new(
                FieldKey::try_new(TITLE_FIELD_KEY).expect("title field"),
                FieldClaim::try_new(
                    source,
                    format!("{provider} {kind}"),
                    None,
                    received(100),
                    None,
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
