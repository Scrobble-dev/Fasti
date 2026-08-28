use crate::kernel::{authorize_transaction, map_sql, now, timestamp, SqliteKernel};
use fasti_application::{
    ApplicationResult, CapabilityKey, ClearNuvioCollectionsCommand, FastiProblem,
    GetNuvioCollectionsQuery, NuvioCollectionsDocument, NuvioCollectionsPort,
    ReplaceNuvioCollectionsCommand,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

impl NuvioCollectionsPort for SqliteKernel {
    fn get_nuvio_collections(
        &self,
        query: GetNuvioCollectionsQuery,
    ) -> ApplicationResult<Option<NuvioCollectionsDocument>> {
        let capability = CapabilityKey::GetNuvioCollections;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;
        let stored = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT document_json
                    FROM profile_nuvio_collections
                    WHERE workspace_id = ?1 AND profile_id = ?2
                    "#,
                    params![
                        query.access().workspace_id().to_string(),
                        query.access().profile_id().to_string()
                    ],
                    |row| row.get::<_, String>(0),
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let document = stored
            .map(|stored| {
                NuvioCollectionsDocument::try_from_canonical_json(&stored).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })
            })
            .transpose()?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(document)
    }

    fn replace_nuvio_collections(
        &self,
        command: ReplaceNuvioCollectionsCommand,
    ) -> ApplicationResult<NuvioCollectionsDocument> {
        let capability = CapabilityKey::ReplaceNuvioCollections;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO profile_nuvio_collections(
                    workspace_id, profile_id, document_json, updated_at
                ) VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(workspace_id, profile_id) DO UPDATE SET
                    document_json = excluded.document_json,
                    updated_at = excluded.updated_at
                WHERE profile_nuvio_collections.document_json <> excluded.document_json
                "#,
                params![
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string(),
                    command.document().canonical_json(),
                    timestamp(now())
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(command.into_document())
    }

    fn clear_nuvio_collections(
        &self,
        command: ClearNuvioCollectionsCommand,
    ) -> ApplicationResult<()> {
        let capability = CapabilityKey::ClearNuvioCollections;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        map_sql(
            transaction.execute(
                r#"
                DELETE FROM profile_nuvio_collections
                WHERE workspace_id = ?1 AND profile_id = ?2
                "#,
                params![
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string()
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{ProblemCode, ScopeKey};
    use fasti_domain::RequestCorrelationId;

    fn document(title: &str) -> NuvioCollectionsDocument {
        NuvioCollectionsDocument::try_from_json(&format!(
            r#"[{{"id":"collection","title":"{title}","folders":[]}}]"#
        ))
        .expect("test document")
    }

    fn get(
        node: &TestNode,
        access: fasti_application::RequestAccessContext,
    ) -> Option<NuvioCollectionsDocument> {
        node.kernel
            .get_nuvio_collections(GetNuvioCollectionsQuery::new(
                RequestCorrelationId::new_v7(),
                access,
            ))
            .expect("get Nuvio Collections")
    }

    #[test]
    fn collections_are_profile_owned_replaceable_and_clearable() {
        let node = TestNode::new();
        node.kernel
            .replace_nuvio_collections(ReplaceNuvioCollectionsCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                document("First profile"),
            ))
            .expect("replace first profile document");

        let second = node
            .add_profile_with_scopes(&[ScopeKey::ProfileStateRead, ScopeKey::ProfileStateWrite]);
        assert!(get(&node, second).is_none());
        node.kernel
            .replace_nuvio_collections(ReplaceNuvioCollectionsCommand::new(
                RequestCorrelationId::new_v7(),
                second,
                document("Second profile"),
            ))
            .expect("replace second profile document");

        assert!(get(&node, node.access)
            .expect("first profile document")
            .canonical_json()
            .contains("First profile"));
        assert!(get(&node, second)
            .expect("second profile document")
            .canonical_json()
            .contains("Second profile"));

        node.kernel
            .clear_nuvio_collections(ClearNuvioCollectionsCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("clear first profile document");
        assert!(get(&node, node.access).is_none());
        assert!(get(&node, second).is_some());
    }

    #[test]
    fn profile_scope_is_required() {
        let node = TestNode::new();
        let denied = node.add_profile_with_scopes(&[]);
        let problem = node
            .kernel
            .get_nuvio_collections(GetNuvioCollectionsQuery::new(
                RequestCorrelationId::new_v7(),
                denied,
            ))
            .expect_err("missing profile-state scope must fail");
        assert_eq!(problem.code(), ProblemCode::Forbidden);
    }

    #[test]
    fn collections_survive_kernel_restart() {
        let node = TestNode::new();
        let expected = node
            .kernel
            .replace_nuvio_collections(ReplaceNuvioCollectionsCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                document("Durable"),
            ))
            .expect("replace document");
        let (root, access) = node.into_stopped();
        let kernel = SqliteKernel::open(root.path()).expect("reopen SQLite kernel");
        let actual = kernel
            .get_nuvio_collections(GetNuvioCollectionsQuery::new(
                RequestCorrelationId::new_v7(),
                access,
            ))
            .expect("get after restart")
            .expect("stored document");
        assert_eq!(actual.canonical_json(), expected.canonical_json());
    }
}
