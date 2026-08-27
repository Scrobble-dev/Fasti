use crate::identity::load_record_grain;
use crate::kernel::{authorize_transaction, map_sql, now, timestamp, SqliteKernel};
use fasti_application::{
    ApplicationResult, CapabilityKey, FastiProblem, ListTrackingDispositionsQuery,
    ProfileRecordStatePort, SetTrackingDispositionCommand, TrackingDispositionView,
};
use fasti_domain::{RecordId, TrackingDisposition};
use rusqlite::{params, TransactionBehavior};
use std::str::FromStr;

const MAX_TRACKING_DISPOSITIONS_PAGE: i64 = 500;

impl ProfileRecordStatePort for SqliteKernel {
    fn list_tracking_dispositions(
        &self,
        query: ListTrackingDispositionsQuery,
    ) -> ApplicationResult<Vec<TrackingDispositionView>> {
        let capability = CapabilityKey::ListTrackingDispositions;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;

        let mut statement = map_sql(
            transaction.prepare(
                r#"
                SELECT record_id, disposition
                FROM profile_record_tracking_dispositions
                WHERE workspace_id = ?1 AND profile_id = ?2
                ORDER BY record_id
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
                    MAX_TRACKING_DISPOSITIONS_PAGE
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            ),
            capability,
            correlation_id,
        )?;
        let mut dispositions = Vec::new();
        for row in rows {
            let (record_id, disposition) = map_sql(row, capability, correlation_id)?;
            dispositions.push(TrackingDispositionView::new(
                record_id.parse::<RecordId>().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                TrackingDisposition::from_str(&disposition).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
            ));
        }
        drop(statement);
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(dispositions)
    }

    fn set_tracking_disposition(
        &self,
        command: SetTrackingDispositionCommand,
    ) -> ApplicationResult<Option<TrackingDispositionView>> {
        let capability = CapabilityKey::SetTrackingDisposition;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        load_record_grain(
            &transaction,
            command.access().workspace_id(),
            command.record_id(),
            capability,
            correlation_id,
        )?;

        match command.disposition() {
            Some(disposition) => {
                map_sql(
                    transaction.execute(
                        r#"
                        INSERT INTO profile_record_tracking_dispositions(
                            workspace_id, profile_id, record_id, disposition, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5)
                        ON CONFLICT(workspace_id, profile_id, record_id) DO UPDATE SET
                            disposition = excluded.disposition,
                            updated_at = excluded.updated_at
                        "#,
                        params![
                            command.access().workspace_id().to_string(),
                            command.access().profile_id().to_string(),
                            command.record_id().to_string(),
                            disposition.as_str(),
                            timestamp(now())
                        ],
                    ),
                    capability,
                    correlation_id,
                )?;
            }
            None => {
                map_sql(
                    transaction.execute(
                        r#"
                        DELETE FROM profile_record_tracking_dispositions
                        WHERE workspace_id = ?1 AND profile_id = ?2 AND record_id = ?3
                        "#,
                        params![
                            command.access().workspace_id().to_string(),
                            command.access().profile_id().to_string(),
                            command.record_id().to_string()
                        ],
                    ),
                    capability,
                    correlation_id,
                )?;
            }
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(command
            .disposition()
            .map(|disposition| TrackingDispositionView::new(command.record_id(), disposition)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{CreateRecordCommand, IdentityPort, ProblemCode, ScopeKey};
    use fasti_domain::{Grain, RequestCorrelationId};

    fn create_record(node: &TestNode) -> RecordId {
        node.kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create record")
            .record_id()
    }

    fn list(
        node: &TestNode,
        access: fasti_application::RequestAccessContext,
    ) -> Vec<TrackingDispositionView> {
        node.kernel
            .list_tracking_dispositions(ListTrackingDispositionsQuery::new(
                RequestCorrelationId::new_v7(),
                access,
            ))
            .expect("list tracking dispositions")
    }

    #[test]
    fn tracking_dispositions_are_profile_owned_and_clearable() {
        let node = TestNode::new();
        let record_id = create_record(&node);
        node.kernel
            .set_tracking_disposition(SetTrackingDispositionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                Some(TrackingDisposition::Watching),
            ))
            .expect("set first profile disposition");

        let second = node
            .add_profile_with_scopes(&[ScopeKey::ProfileStateRead, ScopeKey::ProfileStateWrite]);
        assert!(list(&node, second).is_empty());
        node.kernel
            .set_tracking_disposition(SetTrackingDispositionCommand::new(
                RequestCorrelationId::new_v7(),
                second,
                record_id,
                Some(TrackingDisposition::Dropped),
            ))
            .expect("set second profile disposition");

        assert_eq!(
            list(&node, node.access),
            vec![TrackingDispositionView::new(
                record_id,
                TrackingDisposition::Watching
            )]
        );
        assert_eq!(
            list(&node, second),
            vec![TrackingDispositionView::new(
                record_id,
                TrackingDisposition::Dropped
            )]
        );

        assert_eq!(
            node.kernel
                .set_tracking_disposition(SetTrackingDispositionCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    record_id,
                    None,
                ))
                .expect("clear disposition"),
            None
        );
        assert!(list(&node, node.access).is_empty());
        assert_eq!(list(&node, second).len(), 1);
    }

    #[test]
    fn setting_a_missing_record_returns_the_typed_problem() {
        let node = TestNode::new();
        let problem = node
            .kernel
            .set_tracking_disposition(SetTrackingDispositionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                RecordId::new_v7(),
                Some(TrackingDisposition::OnHold),
            ))
            .expect_err("missing record must fail");
        assert_eq!(problem.code(), ProblemCode::RecordNotFound);
    }
}
