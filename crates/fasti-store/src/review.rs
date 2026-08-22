use crate::identity::{attach_identifier_tx, insert_record, load_record_grain};
use crate::kernel::{authorize_transaction, map_sql, now, timestamp, SqliteKernel};
use fasti_application::{
    ApplicationResult, CapabilityKey, FastiProblem, ResolveReviewCommand, ResolveReviewOutcome,
    ReviewAction, ReviewActionCommand, ReviewItemView, ReviewPort, ReviewQuery,
    ReviewResolutionTarget,
};
use fasti_domain::{InterpretationId, ObservationId, RecordId, ReviewItemId, ReviewStatus};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

const MAX_REVIEW_PAGE: i64 = 100;

impl ReviewPort for SqliteKernel {
    fn inspect_reviews(&self, query: ReviewQuery) -> ApplicationResult<Vec<ReviewItemView>> {
        let capability = CapabilityKey::InspectReview;
        let correlation_id = query.correlation_id();
        let connection = self.lock_connection(capability, correlation_id)?;
        let snapshot = crate::kernel::load_access_snapshot(
            &connection,
            query.access(),
            capability,
            correlation_id,
        )?;
        fasti_application::authorize(
            &fasti_application::AuthorizationRequirement::for_capability(capability),
            Some(query.access()),
            Some(&snapshot),
        )
        .map_err(|_| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;

        let mut values = Vec::new();
        if let Some(review_item_id) = query.review_item_id() {
            if let Some(value) = load_review_view(
                &connection,
                query.access().workspace_id(),
                query.access().profile_id(),
                review_item_id,
                capability,
                correlation_id,
            )? {
                values.push(value);
            }
        } else {
            let mut statement = map_sql(
                connection.prepare(
                    r#"
                    SELECT review_item_id FROM review_items
                    WHERE workspace_id = ?1 AND profile_id = ?2
                      AND status IN ('open', 'deferred')
                    ORDER BY review_item_id
                    LIMIT ?3
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
                        MAX_REVIEW_PAGE
                    ],
                    |row| row.get::<_, String>(0),
                ),
                capability,
                correlation_id,
            )?;
            for row in rows {
                let id = map_sql(row, capability, correlation_id)?
                    .parse::<ReviewItemId>()
                    .map_err(|_| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?;
                if let Some(value) = load_review_view(
                    &connection,
                    query.access().workspace_id(),
                    query.access().profile_id(),
                    id,
                    capability,
                    correlation_id,
                )? {
                    values.push(value);
                }
            }
        }
        Ok(values)
    }

    fn change_review_status(
        &self,
        command: ReviewActionCommand,
    ) -> ApplicationResult<ReviewItemView> {
        let capability = match command.action() {
            ReviewAction::Defer => CapabilityKey::DeferReview,
            ReviewAction::Resume => CapabilityKey::ResumeReview,
        };
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let current = load_review_status(
            &transaction,
            command.access().workspace_id(),
            command.access().profile_id(),
            command.review_item_id(),
            capability,
            correlation_id,
        )?;
        let next = match (current, command.action()) {
            (ReviewStatus::Open | ReviewStatus::Deferred, ReviewAction::Defer) => {
                ReviewStatus::Deferred
            }
            (ReviewStatus::Deferred, ReviewAction::Resume) => ReviewStatus::Open,
            _ => {
                return Err(Box::new(FastiProblem::from_code(
                    fasti_application::ProblemCode::ValidationFailed,
                    capability,
                    correlation_id,
                )))
            }
        };
        map_sql(
            transaction.execute(
                r#"
                UPDATE review_items SET status = ?1, updated_at = ?2
                WHERE review_item_id = ?3 AND workspace_id = ?4 AND profile_id = ?5
                "#,
                params![
                    review_status_value(next),
                    timestamp(now()),
                    command.review_item_id().to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string()
                ],
            ),
            capability,
            correlation_id,
        )?;
        let view = load_review_view(
            &transaction,
            command.access().workspace_id(),
            command.access().profile_id(),
            command.review_item_id(),
            capability,
            correlation_id,
        )?
        .ok_or_else(|| Box::new(FastiProblem::review_not_found(capability, correlation_id)))?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(view)
    }

    fn resolve_review(
        &self,
        command: ResolveReviewCommand,
    ) -> ApplicationResult<ResolveReviewOutcome> {
        let capability = CapabilityKey::ResolveReview;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let row = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT observation_id, current_interpretation_id, status
                    FROM review_items
                    WHERE review_item_id = ?1 AND workspace_id = ?2 AND profile_id = ?3
                    "#,
                    params![
                        command.review_item_id().to_string(),
                        command.access().workspace_id().to_string(),
                        command.access().profile_id().to_string()
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let Some((observation_id, prior_interpretation_id, status)) = row else {
            return Err(Box::new(FastiProblem::review_not_found(
                capability,
                correlation_id,
            )));
        };
        if status == "resolved" {
            return Err(Box::new(FastiProblem::from_code(
                fasti_application::ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            )));
        }

        let record_id = match command.target() {
            ReviewResolutionTarget::Existing(record_id) => {
                load_record_grain(
                    &transaction,
                    command.access().workspace_id(),
                    record_id,
                    capability,
                    correlation_id,
                )?;
                record_id
            }
            ReviewResolutionTarget::New(grain) => insert_record(
                &transaction,
                command.access().workspace_id(),
                grain,
                capability,
                correlation_id,
            )?,
        };
        for claim in command.identifiers() {
            attach_identifier_tx(
                &transaction,
                command.access().workspace_id(),
                record_id,
                claim,
                capability,
                correlation_id,
            )?;
        }

        let occurrence_id = map_sql(
            transaction.query_row(
                "SELECT occurrence_id FROM occurrences WHERE observation_id = ?1",
                [observation_id.as_str()],
                |row| row.get::<_, String>(0),
            ),
            capability,
            correlation_id,
        )?;
        let replacement_interpretation_id = InterpretationId::new_v7();
        let created_at = timestamp(now());
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO interpretations(
                    interpretation_id, observation_id, occurrence_id,
                    prior_interpretation_id, record_id, state, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'resolved', ?6)
                "#,
                params![
                    replacement_interpretation_id.to_string(),
                    observation_id,
                    occurrence_id,
                    prior_interpretation_id,
                    record_id.to_string(),
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "UPDATE occurrences SET record_id = ?1 WHERE occurrence_id = ?2",
                params![record_id.to_string(), occurrence_id],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                UPDATE review_items
                SET current_interpretation_id = ?1, status = 'resolved', updated_at = ?2
                WHERE review_item_id = ?3
                "#,
                params![
                    replacement_interpretation_id.to_string(),
                    created_at,
                    command.review_item_id().to_string()
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(ResolveReviewOutcome::new(
            command.review_item_id(),
            record_id,
            replacement_interpretation_id,
        ))
    }
}

fn load_review_status(
    transaction: &Transaction<'_>,
    workspace_id: fasti_domain::WorkspaceId,
    profile_id: fasti_domain::ProfileId,
    review_item_id: ReviewItemId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<ReviewStatus> {
    let value = map_sql(
        transaction
            .query_row(
                r#"
                SELECT status FROM review_items
                WHERE review_item_id = ?1 AND workspace_id = ?2 AND profile_id = ?3
                "#,
                params![
                    review_item_id.to_string(),
                    workspace_id.to_string(),
                    profile_id.to_string()
                ],
                |row| row.get::<_, String>(0),
            )
            .optional(),
        capability,
        correlation_id,
    )?
    .ok_or_else(|| Box::new(FastiProblem::review_not_found(capability, correlation_id)))?;
    parse_review_status(&value, capability, correlation_id)
}

fn load_review_view(
    connection: &rusqlite::Connection,
    workspace_id: fasti_domain::WorkspaceId,
    profile_id: fasti_domain::ProfileId,
    review_item_id: ReviewItemId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Option<ReviewItemView>> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT observation_id, current_interpretation_id, status
                FROM review_items
                WHERE review_item_id = ?1 AND workspace_id = ?2 AND profile_id = ?3
                "#,
                params![
                    review_item_id.to_string(),
                    workspace_id.to_string(),
                    profile_id.to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((observation, interpretation, status)) = row else {
        return Ok(None);
    };
    let mut statement = map_sql(
        connection.prepare(
            "SELECT record_id FROM review_candidates WHERE review_item_id = ?1 ORDER BY record_id",
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([review_item_id.to_string()], |row| row.get::<_, String>(0)),
        capability,
        correlation_id,
    )?;
    let mut candidates = Vec::new();
    for row in rows {
        candidates.push(
            map_sql(row, capability, correlation_id)?
                .parse::<RecordId>()
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
        );
    }
    Ok(Some(ReviewItemView::new(
        review_item_id,
        observation
            .parse::<ObservationId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        interpretation
            .parse::<InterpretationId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        parse_review_status(&status, capability, correlation_id)?,
        candidates,
    )))
}

fn parse_review_status(
    value: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<ReviewStatus> {
    match value {
        "open" => Ok(ReviewStatus::Open),
        "deferred" => Ok(ReviewStatus::Deferred),
        "resolved" => Ok(ReviewStatus::Resolved),
        _ => Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        ))),
    }
}

fn review_status_value(status: ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Open => "open",
        ReviewStatus::Deferred => "deferred",
        ReviewStatus::Resolved => "resolved",
    }
}
