use crate::identity::load_record_grain;
use crate::kernel::{authorize_transaction, map_sql, now, timestamp, SqliteKernel};
use fasti_application::{
    AppendCorrectionCommand, AppendCorrectionOutcome, ApplicationResult, CapabilityKey,
    CorrectionChainView, CorrectionEntryView, CorrectionPort, CorrectionTarget, FastiProblem,
    InspectCorrectionChainQuery, ProblemCode, MAX_CORRECTION_CHAIN_PAGE,
    MAX_CORRECTION_REASON_BYTES,
};
use fasti_domain::{CorrectionId, InterpretationId, ObservationId, OccurrenceId, RecordId};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

impl CorrectionPort for SqliteKernel {
    fn append_correction(
        &self,
        command: AppendCorrectionCommand,
    ) -> ApplicationResult<AppendCorrectionOutcome> {
        let capability = CapabilityKey::AppendCorrection;
        let correlation_id = command.correlation_id();
        let reason = command.reason();
        if reason.trim().is_empty()
            || reason.len() > MAX_CORRECTION_REASON_BYTES
            || reason.contains('\0')
        {
            return Err(Box::new(FastiProblem::from_code(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            )));
        }

        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        require_observation_scope(
            &transaction,
            command.observation_id(),
            command.access().workspace_id(),
            command.access().profile_id(),
            capability,
            correlation_id,
        )?;

        let unresolved_review = map_sql(
            transaction.query_row(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM review_items
                    WHERE observation_id = ?1 AND workspace_id = ?2 AND profile_id = ?3
                      AND status IN ('open', 'deferred')
                )
                "#,
                params![
                    command.observation_id().to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string()
                ],
                |row| row.get::<_, bool>(0),
            ),
            capability,
            correlation_id,
        )?;
        if unresolved_review {
            return Err(Box::new(FastiProblem::from_code(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            )));
        }

        let (prior_interpretation_id, occurrence_id, prior_record_id, prior_state) =
            current_interpretation(
                &transaction,
                command.observation_id(),
                capability,
                correlation_id,
            )?;

        let replacement_record_id = match command.target() {
            CorrectionTarget::Unresolved => {
                if prior_state == "unresolved" && prior_record_id.is_none() {
                    return Err(Box::new(FastiProblem::from_code(
                        ProblemCode::ValidationFailed,
                        capability,
                        correlation_id,
                    )));
                }
                None
            }
            CorrectionTarget::Record(record_id) => {
                let replacement_grain = load_record_grain(
                    &transaction,
                    command.access().workspace_id(),
                    record_id,
                    capability,
                    correlation_id,
                )?;
                if prior_record_id == Some(record_id) && prior_state == "resolved" {
                    return Err(Box::new(FastiProblem::from_code(
                        ProblemCode::ValidationFailed,
                        capability,
                        correlation_id,
                    )));
                }
                if let Some(prior_record_id) = prior_record_id {
                    let prior_grain = load_record_grain(
                        &transaction,
                        command.access().workspace_id(),
                        prior_record_id,
                        capability,
                        correlation_id,
                    )?;
                    if prior_grain != replacement_grain {
                        return Err(Box::new(FastiProblem::from_code(
                            ProblemCode::ValidationFailed,
                            capability,
                            correlation_id,
                        )));
                    }
                }
                Some(record_id)
            }
        };

        let replacement_interpretation_id = InterpretationId::new_v7();
        let correction_id = CorrectionId::new_v7();
        let created_at = timestamp(now());
        let replacement_state = if replacement_record_id.is_some() {
            "resolved"
        } else {
            "unresolved"
        };

        map_sql(
            transaction.execute(
                r#"
                INSERT INTO interpretations(
                    interpretation_id, observation_id, occurrence_id,
                    prior_interpretation_id, record_id, state, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    replacement_interpretation_id.to_string(),
                    command.observation_id().to_string(),
                    occurrence_id.to_string(),
                    prior_interpretation_id.to_string(),
                    replacement_record_id.map(|value| value.to_string()),
                    replacement_state,
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO corrections(
                    correction_id, workspace_id, profile_id, observation_id,
                    prior_interpretation_id, replacement_interpretation_id,
                    actor_client_id, record_id, reason, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                "#,
                params![
                    correction_id.to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string(),
                    command.observation_id().to_string(),
                    prior_interpretation_id.to_string(),
                    replacement_interpretation_id.to_string(),
                    command.access().client_id().to_string(),
                    replacement_record_id.map(|value| value.to_string()),
                    reason,
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                UPDATE review_items
                SET current_interpretation_id = ?1, updated_at = ?2
                WHERE observation_id = ?3 AND workspace_id = ?4 AND profile_id = ?5
                  AND status = 'resolved'
                "#,
                params![
                    replacement_interpretation_id.to_string(),
                    created_at,
                    command.observation_id().to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string()
                ],
            ),
            capability,
            correlation_id,
        )?;

        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(AppendCorrectionOutcome::new(
            correction_id,
            prior_interpretation_id,
            replacement_interpretation_id,
            replacement_record_id,
        ))
    }

    fn inspect_correction_chain(
        &self,
        query: InspectCorrectionChainQuery,
    ) -> ApplicationResult<CorrectionChainView> {
        let capability = CapabilityKey::InspectCorrectionChain;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;
        require_observation_scope(
            &transaction,
            query.observation_id(),
            query.access().workspace_id(),
            query.access().profile_id(),
            capability,
            correlation_id,
        )?;

        let initial_interpretation_id = root_interpretation(
            &transaction,
            query.observation_id(),
            capability,
            correlation_id,
        )?;
        let (current_interpretation_id, _, _, _) = current_interpretation(
            &transaction,
            query.observation_id(),
            capability,
            correlation_id,
        )?;

        let mut statement = map_sql(
            transaction.prepare(
                r#"
                SELECT correction_id, prior_interpretation_id,
                       replacement_interpretation_id, record_id, reason
                FROM corrections
                WHERE workspace_id = ?1 AND profile_id = ?2 AND observation_id = ?3
                ORDER BY created_at, correction_id
                LIMIT ?4
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
                    query.observation_id().to_string(),
                    i64::try_from(MAX_CORRECTION_CHAIN_PAGE + 1).unwrap_or(101)
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            ),
            capability,
            correlation_id,
        )?;
        let mut corrections = Vec::new();
        for row in rows {
            let (correction, prior, replacement, record, reason) =
                map_sql(row, capability, correlation_id)?;
            corrections.push(CorrectionEntryView::new(
                parse_id::<CorrectionId>(&correction, capability, correlation_id)?,
                parse_id::<InterpretationId>(&prior, capability, correlation_id)?,
                parse_id::<InterpretationId>(&replacement, capability, correlation_id)?,
                record
                    .map(|value| parse_id::<RecordId>(&value, capability, correlation_id))
                    .transpose()?,
                reason,
            ));
        }
        let truncated = corrections.len() > MAX_CORRECTION_CHAIN_PAGE;
        corrections.truncate(MAX_CORRECTION_CHAIN_PAGE);
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(CorrectionChainView::new(
            query.observation_id(),
            initial_interpretation_id,
            current_interpretation_id,
            corrections,
            truncated,
        ))
    }
}

fn require_observation_scope(
    transaction: &Transaction<'_>,
    observation_id: ObservationId,
    workspace_id: fasti_domain::WorkspaceId,
    profile_id: fasti_domain::ProfileId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    let exists = map_sql(
        transaction.query_row(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM observations
                WHERE observation_id = ?1 AND workspace_id = ?2 AND profile_id = ?3
            )
            "#,
            params![
                observation_id.to_string(),
                workspace_id.to_string(),
                profile_id.to_string()
            ],
            |row| row.get::<_, bool>(0),
        ),
        capability,
        correlation_id,
    )?;
    if exists {
        Ok(())
    } else {
        Err(Box::new(FastiProblem::from_code(
            ProblemCode::ValidationFailed,
            capability,
            correlation_id,
        )))
    }
}

fn current_interpretation(
    transaction: &Transaction<'_>,
    observation_id: ObservationId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<(InterpretationId, OccurrenceId, Option<RecordId>, String)> {
    let count = map_sql(
        transaction.query_row(
            r#"
            SELECT COUNT(*) FROM interpretations i
            WHERE i.observation_id = ?1
              AND NOT EXISTS(
                  SELECT 1 FROM interpretations child
                  WHERE child.prior_interpretation_id = i.interpretation_id
              )
            "#,
            [observation_id.to_string()],
            |row| row.get::<_, i64>(0),
        ),
        capability,
        correlation_id,
    )?;
    if count != 1 {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    let (interpretation, occurrence, record, state) = map_sql(
        transaction.query_row(
            r#"
            SELECT i.interpretation_id, i.occurrence_id, i.record_id, i.state
            FROM interpretations i
            WHERE i.observation_id = ?1
              AND NOT EXISTS(
                  SELECT 1 FROM interpretations child
                  WHERE child.prior_interpretation_id = i.interpretation_id
              )
            "#,
            [observation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    Ok((
        parse_id::<InterpretationId>(&interpretation, capability, correlation_id)?,
        parse_id::<OccurrenceId>(&occurrence, capability, correlation_id)?,
        record
            .map(|value| parse_id::<RecordId>(&value, capability, correlation_id))
            .transpose()?,
        state,
    ))
}

fn root_interpretation(
    transaction: &Transaction<'_>,
    observation_id: ObservationId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<InterpretationId> {
    let roots = map_sql(
        transaction.query_row(
            "SELECT COUNT(*) FROM interpretations WHERE observation_id = ?1 AND prior_interpretation_id IS NULL",
            [observation_id.to_string()],
            |row| row.get::<_, i64>(0),
        ),
        capability,
        correlation_id,
    )?;
    if roots != 1 {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    let value = map_sql(
        transaction.query_row(
            "SELECT interpretation_id FROM interpretations WHERE observation_id = ?1 AND prior_interpretation_id IS NULL",
            [observation_id.to_string()],
            |row| row.get::<_, String>(0),
        ),
        capability,
        correlation_id,
    )?;
    parse_id::<InterpretationId>(&value, capability, correlation_id)
}

fn parse_id<T: std::str::FromStr>(
    value: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<T> {
    value
        .parse::<T>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{
        AcceptObservationCommand, AttachIdentifierCommand, CorrectionPort, CreateRecordCommand,
        IdentityPort, ObservationAcceptancePort,
    };
    use fasti_domain::{
        ClaimedTrust, ExternalIdentifierClaim, Grain, ObservedAt, OperationId,
        RequestCorrelationId,
    };

    fn observed_at() -> ObservedAt {
        ObservedAt::parse("2026-08-23T20:30:00Z", ClaimedTrust::DeviceObserved)
            .expect("observed time")
    }

    fn create_resolved_observation(node: &TestNode) -> (ObservationId, RecordId) {
        let record = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Release,
            ))
            .expect("create first record")
            .record_id();
        let claim = ExternalIdentifierClaim::try_new("imdb", Grain::Release, "tt0903747")
            .expect("valid identifier");
        node.kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                claim.clone(),
            ))
            .expect("attach identifier");
        let evidence = node.upload(b"original immutable evidence");
        let accepted = node
            .kernel
            .authorize_and_accept(
                AcceptObservationCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    OperationId::new_v7(),
                    None,
                    observed_at(),
                    evidence,
                )
                .with_identity_clues(vec![claim], Some(Grain::Release)),
            )
            .expect("accept resolved observation");
        (accepted.receipt().observation_id(), record)
    }

    #[test]
    fn correction_appends_interpretation_without_rewriting_occurrence() {
        let node = TestNode::new();
        let (observation_id, original_record_id) = create_resolved_observation(&node);
        let replacement_record_id = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Release,
            ))
            .expect("create replacement record")
            .record_id();

        let corrected = node
            .kernel
            .append_correction(AppendCorrectionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                observation_id,
                CorrectionTarget::Record(replacement_record_id),
                "The original interpretation resolved to the wrong release.",
            ))
            .expect("append correction");
        assert_eq!(corrected.record_id(), Some(replacement_record_id));

        let chain = node
            .kernel
            .inspect_correction_chain(InspectCorrectionChainQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                observation_id,
            ))
            .expect("inspect correction chain");
        assert_eq!(chain.corrections().len(), 1);
        assert!(!chain.truncated());
        assert_eq!(
            chain.current_interpretation_id(),
            corrected.replacement_interpretation_id()
        );

        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let occurrence_record = connection
            .query_row(
                "SELECT record_id FROM occurrences WHERE observation_id = ?1",
                [observation_id.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("load occurrence record");
        assert_eq!(occurrence_record, Some(original_record_id.to_string()));
        let interpretation_count = connection
            .query_row(
                "SELECT COUNT(*) FROM interpretations WHERE observation_id = ?1",
                [observation_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .expect("count interpretations");
        assert_eq!(interpretation_count, 2);
    }

    #[test]
    fn correction_rejects_a_cross_grain_replacement() {
        let node = TestNode::new();
        let (observation_id, _) = create_resolved_observation(&node);
        let episode_record = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Episode,
            ))
            .expect("create episode record")
            .record_id();

        let error = node
            .kernel
            .append_correction(AppendCorrectionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                observation_id,
                CorrectionTarget::Record(episode_record),
                "Cross-grain replacement must not be accepted.",
            ))
            .expect_err("cross-grain correction");
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
    }
}
