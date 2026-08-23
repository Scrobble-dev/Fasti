use crate::crypto::{sha256_hex, sha256_reader};
use crate::evidence::{canonical_digest_hex, path_to_storage_value, relative_evidence_path};
use crate::identity::matching_record_ids;
use crate::kernel::{
    authorize_connection, authorize_transaction, map_json, map_sql, now, parse_timestamp,
    reject_unsafe_existing_file, timestamp, SqliteKernel,
};
use fasti_application::{
    AcceptObservationCommand, AcceptObservationOutcome, AcceptObservationReceipt,
    ApplicationResult, CapabilityKey, FastiProblem, ObservationAcceptancePort, ReceiptStreamBatch,
    ReceiptStreamEvent, ReceiptStreamPort, ReplayReceiptQuery, StreamReceiptsQuery,
    MAX_IDENTITY_CLAIMS, MAX_RECEIPT_STREAM_REPLAY,
};
use fasti_domain::{
    ClientId, CommittedAt, EvidenceId, EvidenceReference, ExternalIdentifierClaim, Grain,
    InterpretationId, Observation, ObservationId, ObservationResolution, ObservedAt, OccurredAt,
    OccurrenceId, OperationId, ProfileId, ReceiptId, ReceivedAt, RecordId, ReviewItemId,
    Sha256Digest, WorkspaceId,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::json;
use std::collections::BTreeSet;
use std::fs::File;

impl ObservationAcceptancePort for SqliteKernel {
    fn authorize_and_accept(
        &self,
        command: AcceptObservationCommand,
    ) -> ApplicationResult<AcceptObservationOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::AcceptObservation;
        if command.identity_clues().len() > MAX_IDENTITY_CLAIMS {
            return Err(Box::new(FastiProblem::from_code(
                fasti_application::ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            )));
        }
        let canonical_clues = canonical_identity_clues(command.identity_clues());
        let evidence = validate_prepared_evidence(self, &command)?;
        let semantic_digest = semantic_digest(&command, &canonical_clues)?;

        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        verify_evidence_row(&transaction, &command, capability)?;

        let existing = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT semantic_digest, receipt_id
                    FROM operations
                    WHERE workspace_id = ?1 AND client_id = ?2 AND operation_id = ?3
                    "#,
                    params![
                        command.access().workspace_id().to_string(),
                        command.access().client_id().to_string(),
                        command.operation_id().to_string()
                    ],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        if let Some((stored_digest, receipt_id)) = existing {
            if stored_digest != semantic_digest {
                return Err(Box::new(FastiProblem::idempotency_conflict(
                    capability,
                    correlation_id,
                )));
            }
            let receipt_id = receipt_id.parse::<ReceiptId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?;
            let receipt = load_receipt(
                &transaction,
                receipt_id,
                command.access().workspace_id(),
                command.access().profile_id(),
                command.access().client_id(),
                capability,
                correlation_id,
            )?;
            return Ok(AcceptObservationOutcome::Replayed(receipt));
        }

        let selected_claims = selected_claims(command.target_grain(), &canonical_clues);
        let matches = matching_record_ids(
            &transaction,
            command.access().workspace_id(),
            &selected_claims,
            capability,
            correlation_id,
        )?;
        let (resolution, record_id) = match matches.as_slice() {
            [] => (ObservationResolution::Unresolved, None),
            [record_id] => (ObservationResolution::Resolved, Some(*record_id)),
            _ => (ObservationResolution::Conflicted, None),
        };

        let received_at = ReceivedAt::from_application_clock(now());
        let observation_id = ObservationId::new_v7();
        let occurrence_id = OccurrenceId::new_v7();
        let interpretation_id = InterpretationId::new_v7();
        let review_item_id = matches.len().gt(&1).then(ReviewItemId::new_v7);
        let (observation, _) = Observation::new_unresolved(
            observation_id,
            command.access().workspace_id(),
            command.access().profile_id(),
            command.access().client_id(),
            evidence.clone(),
            command.occurred_at().cloned(),
            command.observed_at().clone(),
            received_at,
        );
        let occurred_json = command
            .occurred_at()
            .map(|value| map_json(serde_json::to_string(value), capability, correlation_id))
            .transpose()?;
        let observed_json = map_json(
            serde_json::to_string(command.observed_at()),
            capability,
            correlation_id,
        )?;
        let created_at = timestamp(now());

        map_sql(
            transaction.execute(
                r#"
                INSERT INTO observations(
                    observation_id, workspace_id, profile_id, source_client_id,
                    evidence_id, occurred_at_json, observed_at_json, received_at, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    observation_id.to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string(),
                    command.access().client_id().to_string(),
                    evidence.evidence_id().to_string(),
                    occurred_json,
                    observed_json,
                    timestamp(received_at.value()),
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        for (ordinal, claim) in canonical_clues.iter().enumerate() {
            map_sql(
                transaction.execute(
                    r#"
                    INSERT INTO observation_clues(
                        observation_id, ordinal, namespace, grain, value
                    ) VALUES (?1, ?2, ?3, ?4, ?5)
                    "#,
                    params![
                        observation_id.to_string(),
                        i64::try_from(ordinal).unwrap_or(i64::MAX),
                        claim.namespace(),
                        claim.grain().as_str(),
                        claim.value()
                    ],
                ),
                capability,
                correlation_id,
            )?;
        }
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO occurrences(
                    occurrence_id, workspace_id, profile_id, observation_id,
                    record_id, occurred_at_json, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    occurrence_id.to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string(),
                    observation_id.to_string(),
                    record_id.map(|value| value.to_string()),
                    occurred_json,
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO interpretations(
                    interpretation_id, observation_id, occurrence_id,
                    prior_interpretation_id, record_id, state, created_at
                ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6)
                "#,
                params![
                    interpretation_id.to_string(),
                    observation_id.to_string(),
                    occurrence_id.to_string(),
                    record_id.map(|value| value.to_string()),
                    resolution_storage_value(resolution),
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        if let Some(review_item_id) = review_item_id {
            map_sql(
                transaction.execute(
                    r#"
                    INSERT INTO review_items(
                        review_item_id, workspace_id, profile_id, observation_id,
                        current_interpretation_id, status, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?6)
                    "#,
                    params![
                        review_item_id.to_string(),
                        command.access().workspace_id().to_string(),
                        command.access().profile_id().to_string(),
                        observation_id.to_string(),
                        interpretation_id.to_string(),
                        created_at
                    ],
                ),
                capability,
                correlation_id,
            )?;
            for candidate in &matches {
                map_sql(
                    transaction.execute(
                        "INSERT INTO review_candidates(review_item_id, record_id) VALUES (?1, ?2)",
                        params![review_item_id.to_string(), candidate.to_string()],
                    ),
                    capability,
                    correlation_id,
                )?;
            }
        }

        let receipt_id = ReceiptId::new_v7();
        let committed_at = CommittedAt::from_durability_boundary(now());
        let receipt = AcceptObservationReceipt::from_committed(
            receipt_id,
            command.operation_id(),
            &observation,
            Some(occurrence_id),
            Some(interpretation_id),
            record_id,
            review_item_id,
            resolution,
            committed_at,
        )
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO receipts(
                    receipt_id, operation_id, workspace_id, profile_id, client_id,
                    capability_key, observation_id, occurrence_id, interpretation_id,
                    record_id, review_item_id, evidence_id, payload_digest,
                    resolution, received_at, committed_at, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'accept_observation', ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
                "#,
                params![
                    receipt_id.to_string(),
                    command.operation_id().to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string(),
                    command.access().client_id().to_string(),
                    observation_id.to_string(),
                    occurrence_id.to_string(),
                    interpretation_id.to_string(),
                    record_id.map(|value| value.to_string()),
                    review_item_id.map(|value| value.to_string()),
                    evidence.evidence_id().to_string(),
                    evidence.digest().to_string(),
                    resolution_storage_value(resolution),
                    timestamp(received_at.value()),
                    timestamp(committed_at.value()),
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO operations(
                    workspace_id, client_id, operation_id, capability_key,
                    semantic_digest, receipt_id, created_at
                ) VALUES (?1, ?2, ?3, 'accept_observation', ?4, ?5, ?6)
                "#,
                params![
                    command.access().workspace_id().to_string(),
                    command.access().client_id().to_string(),
                    command.operation_id().to_string(),
                    semantic_digest,
                    receipt_id.to_string(),
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(AcceptObservationOutcome::Committed(receipt))
    }

    fn authorize_and_replay(
        &self,
        query: ReplayReceiptQuery,
    ) -> ApplicationResult<AcceptObservationReceipt> {
        let capability = CapabilityKey::ReplayReceipt;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;
        let receipt = load_receipt(
            &transaction,
            query.receipt_id(),
            query.access().workspace_id(),
            query.access().profile_id(),
            query.access().client_id(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(receipt)
    }
}

impl ReceiptStreamPort for SqliteKernel {
    fn authorize_and_stream(
        &self,
        query: StreamReceiptsQuery,
    ) -> ApplicationResult<ReceiptStreamBatch> {
        let capability = CapabilityKey::StreamReceipts;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;
        let after_sequence = if let Some(cursor) = query.last_event_id() {
            let receipt_id = cursor
                .parse::<ReceiptId>()
                .map_err(|_| Box::new(FastiProblem::cursor_expired(correlation_id)))?;
            map_sql(
                transaction
                    .query_row(
                        r#"
                        SELECT sequence FROM receipts
                        WHERE receipt_id = ?1 AND workspace_id = ?2
                          AND profile_id = ?3 AND client_id = ?4
                        "#,
                        params![
                            receipt_id.to_string(),
                            query.access().workspace_id().to_string(),
                            query.access().profile_id().to_string(),
                            query.access().client_id().to_string()
                        ],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional(),
                capability,
                correlation_id,
            )?
            .ok_or_else(|| Box::new(FastiProblem::cursor_expired(correlation_id)))?
        } else {
            0
        };
        let receipt_ids = {
            let mut statement = map_sql(
                transaction.prepare(
                    r#"
                    SELECT receipt_id FROM receipts
                    WHERE workspace_id = ?1 AND profile_id = ?2 AND client_id = ?3
                      AND sequence > ?4
                    ORDER BY sequence ASC
                    LIMIT ?5
                    "#,
                ),
                capability,
                correlation_id,
            )?;
            let rows = map_sql(
                statement.query_map(
                    params![
                        query.access().workspace_id().to_string(),
                        query.access().profile_id().to_string(),
                        query.access().client_id().to_string(),
                        after_sequence,
                        i64::try_from(MAX_RECEIPT_STREAM_REPLAY).unwrap_or(100)
                    ],
                    |row| row.get::<_, String>(0),
                ),
                capability,
                correlation_id,
            )?;
            let mut receipt_ids = Vec::new();
            for row in rows {
                receipt_ids.push(map_sql(row, capability, correlation_id)?);
            }
            receipt_ids
        };
        let mut events = Vec::with_capacity(receipt_ids.len());
        for receipt_id in receipt_ids {
            let receipt_id = receipt_id.parse::<ReceiptId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?;
            let receipt = load_receipt(
                &transaction,
                receipt_id,
                query.access().workspace_id(),
                query.access().profile_id(),
                query.access().client_id(),
                capability,
                correlation_id,
            )?;
            events.push(ReceiptStreamEvent::new(correlation_id, receipt));
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(ReceiptStreamBatch::new(events))
    }
}

fn validate_prepared_evidence(
    kernel: &SqliteKernel,
    command: &AcceptObservationCommand,
) -> ApplicationResult<EvidenceReference> {
    let correlation_id = command.correlation_id();
    let capability = CapabilityKey::AcceptObservation;
    let (digest, size, path) = {
        let connection = kernel.lock_connection(capability, correlation_id)?;
        authorize_connection(&connection, capability, command.access(), correlation_id)?;
        map_sql(
            connection
                .query_row(
                    r#"
                    SELECT digest, size_bytes, relative_path FROM evidence
                    WHERE evidence_id = ?1 AND workspace_id = ?2
                    "#,
                    params![
                        command.prepared_evidence().evidence_id().to_string(),
                        command.access().workspace_id().to_string()
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional(),
            capability,
            correlation_id,
        )?
        .ok_or_else(|| Box::new(FastiProblem::evidence_not_found(correlation_id)))?
    };
    let size = u64::try_from(size)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    if digest != command.prepared_evidence().digest().as_str()
        || size != command.prepared_evidence().byte_length()
    {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    let digest_hex = canonical_digest_hex(&digest)
        .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let expected_relative_path = relative_evidence_path(digest_hex);
    if path != path_to_storage_value(&expected_relative_path) {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    let full_path = kernel.inner.current_root.join(expected_relative_path);
    reject_unsafe_existing_file(&full_path)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let file = File::open(full_path)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let (observed_digest, observed_size) = sha256_reader(file)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let observed = format!("sha256:{}", crate::crypto::encode_hex(&observed_digest));
    if observed != digest || observed_size != size {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    Ok(command.prepared_evidence().clone())
}

fn verify_evidence_row(
    connection: &Connection,
    command: &AcceptObservationCommand,
    capability: CapabilityKey,
) -> ApplicationResult<()> {
    let correlation_id = command.correlation_id();
    let exists = map_sql(
        connection.query_row(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM evidence
                WHERE evidence_id = ?1 AND workspace_id = ?2
                  AND digest = ?3 AND size_bytes = ?4
            )
            "#,
            params![
                command.prepared_evidence().evidence_id().to_string(),
                command.access().workspace_id().to_string(),
                command.prepared_evidence().digest().to_string(),
                i64::try_from(command.prepared_evidence().byte_length()).unwrap_or(i64::MAX)
            ],
            |row| row.get::<_, bool>(0),
        ),
        capability,
        correlation_id,
    )?;
    if !exists {
        return Err(Box::new(FastiProblem::evidence_not_found(correlation_id)));
    }
    Ok(())
}

fn canonical_identity_clues(
    identity_clues: &[ExternalIdentifierClaim],
) -> Vec<ExternalIdentifierClaim> {
    let mut claims = identity_clues.to_vec();
    claims.sort_by(|left, right| {
        (left.namespace(), left.grain().as_str(), left.value()).cmp(&(
            right.namespace(),
            right.grain().as_str(),
            right.value(),
        ))
    });
    claims.dedup_by(|left, right| {
        left.namespace() == right.namespace()
            && left.grain() == right.grain()
            && left.value() == right.value()
    });
    claims
}

fn semantic_digest(
    command: &AcceptObservationCommand,
    canonical_clues: &[ExternalIdentifierClaim],
) -> ApplicationResult<String> {
    let capability = CapabilityKey::AcceptObservation;
    let correlation_id = command.correlation_id();
    let clues = canonical_clues
        .iter()
        .map(|claim| {
            json!({
                "namespace": claim.namespace(),
                "grain": claim.grain().as_str(),
                "value": claim.value(),
            })
        })
        .collect::<Vec<_>>();
    let value = json!({
        "capability": "accept_observation",
        "workspace_id": command.access().workspace_id().to_string(),
        "profile_id": command.access().profile_id().to_string(),
        "evidence": command.prepared_evidence().digest().as_str(),
        "occurred_at": command.occurred_at(),
        "observed_at": command.observed_at(),
        "target_grain": command.target_grain().map(Grain::as_str),
        "identity_clues": clues,
    });
    let bytes = map_json(serde_json::to_vec(&value), capability, correlation_id)?;
    Ok(format!("sha256:{}", sha256_hex(&bytes)))
}

fn selected_claims(
    target_grain: Option<Grain>,
    canonical_clues: &[ExternalIdentifierClaim],
) -> Vec<ExternalIdentifierClaim> {
    if let Some(target) = target_grain {
        return canonical_clues
            .iter()
            .filter(|claim| claim.grain() == target)
            .cloned()
            .collect();
    }
    let grains = canonical_clues
        .iter()
        .map(ExternalIdentifierClaim::grain)
        .collect::<BTreeSet<_>>();
    if grains.len() == 1 {
        canonical_clues.to_vec()
    } else {
        Vec::new()
    }
}

pub(crate) fn load_receipt(
    connection: &Connection,
    receipt_id: ReceiptId,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: ClientId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<AcceptObservationReceipt> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT
                    r.operation_id, r.observation_id, r.occurrence_id,
                    r.interpretation_id, r.record_id, r.review_item_id,
                    r.evidence_id, r.payload_digest, r.resolution,
                    r.received_at, r.committed_at,
                    o.occurred_at_json, o.observed_at_json,
                    e.size_bytes
                FROM receipts r
                JOIN observations o
                  ON o.observation_id = r.observation_id
                 AND o.workspace_id = r.workspace_id
                 AND o.profile_id = r.profile_id
                 AND o.source_client_id = r.client_id
                JOIN evidence e
                  ON e.evidence_id = r.evidence_id
                 AND e.workspace_id = r.workspace_id
                WHERE r.receipt_id = ?1 AND r.workspace_id = ?2
                  AND r.profile_id = ?3 AND r.client_id = ?4
                "#,
                params![
                    receipt_id.to_string(),
                    workspace_id.to_string(),
                    profile_id.to_string(),
                    client_id.to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, i64>(13)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((
        operation,
        observation,
        occurrence,
        interpretation,
        record,
        review,
        evidence,
        digest,
        resolution,
        received,
        committed,
        occurred_json,
        observed_json,
        size,
    )) = row
    else {
        return Err(Box::new(FastiProblem::receipt_not_found(
            capability,
            correlation_id,
        )));
    };
    let received_at =
        ReceivedAt::from_application_clock(parse_timestamp(&received, capability, correlation_id)?);
    let occurred_at = occurred_json
        .map(|value| {
            map_json(
                serde_json::from_str::<OccurredAt>(&value),
                capability,
                correlation_id,
            )
        })
        .transpose()?;
    let observed_at = map_json(
        serde_json::from_str::<ObservedAt>(&observed_json),
        capability,
        correlation_id,
    )?;
    let evidence_reference = EvidenceReference::new(
        evidence
            .parse::<EvidenceId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        digest
            .parse::<Sha256Digest>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        u64::try_from(size)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
    );
    let (observation_value, _) = Observation::new_unresolved(
        observation
            .parse::<ObservationId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        workspace_id,
        profile_id,
        client_id,
        evidence_reference,
        occurred_at,
        observed_at,
        received_at,
    );
    AcceptObservationReceipt::from_committed(
        receipt_id,
        operation
            .parse::<OperationId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        &observation_value,
        parse_optional_id::<OccurrenceId>(occurrence, capability, correlation_id)?,
        parse_optional_id::<InterpretationId>(interpretation, capability, correlation_id)?,
        parse_optional_id::<RecordId>(record, capability, correlation_id)?,
        parse_optional_id::<ReviewItemId>(review, capability, correlation_id)?,
        parse_resolution(&resolution, capability, correlation_id)?,
        CommittedAt::from_durability_boundary(parse_timestamp(
            &committed,
            capability,
            correlation_id,
        )?),
    )
    .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

fn parse_optional_id<T: std::str::FromStr>(
    value: Option<String>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Option<T>> {
    value
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
        })
        .transpose()
}

fn parse_resolution(
    value: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<ObservationResolution> {
    match value {
        "unresolved" => Ok(ObservationResolution::Unresolved),
        "resolved" => Ok(ObservationResolution::Resolved),
        "conflicted" => Ok(ObservationResolution::Conflicted),
        _ => Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        ))),
    }
}

fn resolution_storage_value(value: ObservationResolution) -> &'static str {
    match value {
        ObservationResolution::Unresolved => "unresolved",
        ObservationResolution::Resolved => "resolved",
        ObservationResolution::Conflicted => "conflicted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{ProblemCode, ScopeKey};
    use fasti_domain::{
        ClaimedTrust, ExternalIdentifierClaim, ObservedAt, RequestCorrelationId,
    };

    fn observed_at() -> ObservedAt {
        ObservedAt::parse("2026-08-23T10:30:00Z", ClaimedTrust::DeviceObserved)
            .expect("observed time")
    }

    fn claim(namespace: &str, value: &str) -> ExternalIdentifierClaim {
        ExternalIdentifierClaim::try_new(namespace, Grain::Release, value)
            .expect("valid exact identifier")
    }

    #[test]
    fn canonical_clues_drive_replay_and_persist_once() {
        let node = TestNode::new();
        let evidence = node.upload(b"canonical clue evidence");
        let operation_id = OperationId::new_v7();
        let first = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            operation_id,
            None,
            observed_at(),
            evidence.clone(),
        )
        .with_identity_clues(
            vec![
                claim("kitsu", "42"),
                claim("imdb", "tt0903747"),
                claim("kitsu", "42"),
            ],
            Some(Grain::Release),
        );
        let committed = node
            .kernel
            .authorize_and_accept(first)
            .expect("accept canonical clues");
        assert!(!committed.is_replay());

        let replay = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            operation_id,
            None,
            observed_at(),
            evidence,
        )
        .with_identity_clues(
            vec![claim("imdb", "tt0903747"), claim("kitsu", "42")],
            Some(Grain::Release),
        );
        let replayed = node
            .kernel
            .authorize_and_accept(replay)
            .expect("equivalent clues replay");
        assert!(replayed.is_replay());
        assert_eq!(
            replayed.receipt().receipt_id(),
            committed.receipt().receipt_id()
        );

        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let mut statement = connection
            .prepare(
                r#"
                SELECT namespace, value FROM observation_clues
                WHERE observation_id = ?1 ORDER BY ordinal
                "#,
            )
            .expect("prepare clue query");
        let stored = statement
            .query_map([committed.receipt().observation_id().to_string()], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("query clues")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect clues");
        assert_eq!(
            stored,
            vec![
                ("imdb".to_owned(), "tt0903747".to_owned()),
                ("kitsu".to_owned(), "42".to_owned()),
            ]
        );
    }

    #[test]
    fn operation_identity_is_bound_to_the_target_profile() {
        let node = TestNode::new();
        let evidence = node.upload(b"profile-bound operation");
        let operation_id = OperationId::new_v7();
        let first = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            operation_id,
            None,
            observed_at(),
            evidence.clone(),
        );
        node.kernel
            .authorize_and_accept(first)
            .expect("first profile accepts operation");

        let second_access = node.add_profile_with_scopes(&[ScopeKey::ObservationAccept]);
        let second = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            second_access,
            operation_id,
            None,
            observed_at(),
            evidence,
        );
        let error = node
            .kernel
            .authorize_and_accept(second)
            .expect_err("same operation cannot target another profile");
        assert_eq!(error.code(), ProblemCode::IdempotencyConflict);
    }

    #[test]
    fn observation_rejects_unbounded_identity_clues_before_storage_work() {
        let node = TestNode::new();
        let evidence = node.upload(b"bounded clue evidence");
        let clues = (0..=MAX_IDENTITY_CLAIMS)
            .map(|index| claim("tmdb", &index.to_string()))
            .collect();
        let command = AcceptObservationCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            OperationId::new_v7(),
            None,
            observed_at(),
            evidence,
        )
        .with_identity_clues(clues, Some(Grain::Release));

        let error = node
            .kernel
            .authorize_and_accept(command)
            .expect_err("identity-clue limit");
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
    }
}
