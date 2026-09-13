use crate::kernel::{authorize_application_transaction, map_sql, now, timestamp, SqliteKernel};
use chrono::{DateTime, SecondsFormat, Utc};
use fasti_application::{
    ApplicationAccessContext, ApplicationResult, CapabilityKey, ConfigurationDigest,
    CredentialReference, CredentialRequirement, FastiProblem, ProblemCode, ProviderCapabilityId,
    ProviderCapabilityState, ProviderCapabilityStatus, ProviderCheckMetadata, ProviderCheckStatus,
    ProviderCredentialStatus, ProviderId, ProviderStatePort, ProviderStatePortError,
    ProviderStateWriteOutcome,
};
use fasti_domain::{RequestCorrelationId, WorkspaceId};
use rusqlite::{
    params, types::Type, Connection, OptionalExtension, Row, Transaction, TransactionBehavior,
};

impl ProviderStatePort for SqliteKernel {
    fn authorize_and_list_provider_capability_states(
        &self,
        correlation_id: RequestCorrelationId,
        access: &ApplicationAccessContext,
    ) -> ApplicationResult<Vec<ProviderCapabilityState>> {
        let capability = CapabilityKey::ListProviders;
        let mut connection = self.inner.connection.lock().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })?;
        let transaction = map_sql(connection.transaction(), capability, correlation_id)?;
        let authority =
            authorize_application_transaction(&transaction, capability, access, correlation_id)?;
        let states = list_states(&transaction, authority.workspace_id()).map_err(|error| {
            Box::new(match map_store_error(error) {
                ProviderStatePortError::Corrupt | ProviderStatePortError::RevisionConflict => {
                    FastiProblem::integrity_failed(capability, correlation_id)
                }
                ProviderStatePortError::Unavailable => {
                    FastiProblem::storage_unavailable(capability, correlation_id)
                }
            })
        })?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(states)
    }

    fn get_provider_capability_state(
        &self,
        workspace_id: WorkspaceId,
        provider_id: &ProviderId,
        capability_id: &ProviderCapabilityId,
    ) -> Result<Option<ProviderCapabilityState>, ProviderStatePortError> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| ProviderStatePortError::Unavailable)?;
        connection
            .query_row(
                &state_select(
                    "WHERE workspace_id = ?1 AND provider_id = ?2 AND capability_id = ?3",
                ),
                params![
                    workspace_id.to_string(),
                    provider_id.as_str(),
                    capability_id.as_str()
                ],
                read_state,
            )
            .optional()
            .map_err(map_store_error)
    }

    fn list_provider_capability_states(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<ProviderCapabilityState>, ProviderStatePortError> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| ProviderStatePortError::Unavailable)?;
        list_states(&connection, workspace_id).map_err(map_store_error)
    }

    fn put_provider_capability_state(
        &self,
        workspace_id: WorkspaceId,
        state: ProviderCapabilityState,
    ) -> Result<ProviderStateWriteOutcome, ProviderStatePortError> {
        let mut connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| ProviderStatePortError::Unavailable)?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_store_error)?;
        let existing = read_existing(&transaction, workspace_id, &state)?;
        let outcome = match existing {
            None => {
                insert_state(&transaction, workspace_id, &state)?;
                ProviderStateWriteOutcome::Created
            }
            Some(existing) if existing == state => ProviderStateWriteOutcome::Unchanged,
            Some(existing) if existing.capability_version() >= state.capability_version() => {
                return Err(ProviderStatePortError::RevisionConflict);
            }
            Some(_) => {
                update_state(&transaction, workspace_id, &state)?;
                ProviderStateWriteOutcome::Replaced
            }
        };
        transaction.commit().map_err(map_store_error)?;
        Ok(outcome)
    }
}

fn list_states(
    connection: &Connection,
    workspace_id: WorkspaceId,
) -> rusqlite::Result<Vec<ProviderCapabilityState>> {
    let mut statement = connection.prepare(&state_select(
        "WHERE workspace_id = ?1 ORDER BY provider_id, capability_id",
    ))?;
    let rows = statement.query_map([workspace_id.to_string()], read_state)?;
    rows.collect()
}

pub(crate) fn state_select(suffix: &str) -> String {
    format!(
        r#"
        SELECT provider_id, capability_id, capability_status, capability_version,
               credential_requirement, credential_reference, credential_status,
               configuration_digest, health_status, health_checked_at,
               health_problem_code, credential_test_status,
               credential_test_checked_at, credential_test_problem_code
        FROM provider_capability_states
        {suffix}
        "#
    )
}

fn read_existing(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    state: &ProviderCapabilityState,
) -> Result<Option<ProviderCapabilityState>, ProviderStatePortError> {
    transaction
        .query_row(
            &state_select("WHERE workspace_id = ?1 AND provider_id = ?2 AND capability_id = ?3"),
            params![
                workspace_id.to_string(),
                state.provider_id().as_str(),
                state.capability_id().as_str()
            ],
            read_state,
        )
        .optional()
        .map_err(map_store_error)
}

fn insert_state(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    state: &ProviderCapabilityState,
) -> Result<(), ProviderStatePortError> {
    let health_checked_at = check_timestamp(state.health());
    let credential_checked_at = check_timestamp(state.credential_test());
    transaction
        .execute(
            r#"
            INSERT INTO provider_capability_states(
                workspace_id, provider_id, capability_id, capability_status,
                capability_version, credential_requirement, credential_reference,
                credential_status, configuration_digest, health_status,
                health_checked_at, health_problem_code, credential_test_status,
                credential_test_checked_at, credential_test_problem_code, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
            )
            "#,
            params![
                workspace_id.to_string(),
                state.provider_id().as_str(),
                state.capability_id().as_str(),
                state.capability_status().as_str(),
                state.capability_version() as i64,
                state.credential_requirement().as_str(),
                state
                    .credential_reference()
                    .map(CredentialReference::as_str),
                state.credential_status().as_str(),
                state.configuration_digest().as_str(),
                state.health().status().as_str(),
                health_checked_at,
                state.health().safe_problem_code().map(ProblemCode::as_str),
                state.credential_test().status().as_str(),
                credential_checked_at,
                state
                    .credential_test()
                    .safe_problem_code()
                    .map(ProblemCode::as_str),
                timestamp(now()),
            ],
        )
        .map_err(map_store_error)?;
    Ok(())
}

fn update_state(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    state: &ProviderCapabilityState,
) -> Result<(), ProviderStatePortError> {
    let health_checked_at = check_timestamp(state.health());
    let credential_checked_at = check_timestamp(state.credential_test());
    let changed = transaction
        .execute(
            r#"
            UPDATE provider_capability_states SET
                capability_status = ?4,
                capability_version = ?5,
                credential_requirement = ?6,
                credential_reference = ?7,
                credential_status = ?8,
                configuration_digest = ?9,
                health_status = ?10,
                health_checked_at = ?11,
                health_problem_code = ?12,
                credential_test_status = ?13,
                credential_test_checked_at = ?14,
                credential_test_problem_code = ?15,
                updated_at = ?16
            WHERE workspace_id = ?1 AND provider_id = ?2 AND capability_id = ?3
            "#,
            params![
                workspace_id.to_string(),
                state.provider_id().as_str(),
                state.capability_id().as_str(),
                state.capability_status().as_str(),
                state.capability_version() as i64,
                state.credential_requirement().as_str(),
                state
                    .credential_reference()
                    .map(CredentialReference::as_str),
                state.credential_status().as_str(),
                state.configuration_digest().as_str(),
                state.health().status().as_str(),
                health_checked_at,
                state.health().safe_problem_code().map(ProblemCode::as_str),
                state.credential_test().status().as_str(),
                credential_checked_at,
                state
                    .credential_test()
                    .safe_problem_code()
                    .map(ProblemCode::as_str),
                timestamp(now()),
            ],
        )
        .map_err(map_store_error)?;
    if changed != 1 {
        return Err(ProviderStatePortError::Corrupt);
    }
    Ok(())
}

fn check_timestamp(check: &ProviderCheckMetadata) -> Option<String> {
    check
        .checked_at()
        .map(|value| value.to_rfc3339_opts(SecondsFormat::Micros, true))
}

pub(crate) fn read_state(row: &Row<'_>) -> rusqlite::Result<ProviderCapabilityState> {
    let provider_id = ProviderId::try_new(row.get::<_, String>(0)?)
        .map_err(|error| conversion_error(0, error))?;
    let capability_id = ProviderCapabilityId::try_new(row.get::<_, String>(1)?)
        .map_err(|error| conversion_error(1, error))?;
    let capability_status = ProviderCapabilityStatus::parse(&row.get::<_, String>(2)?)
        .ok_or_else(|| conversion_message(2, "invalid provider capability status"))?;
    let capability_version = row.get::<_, i64>(3)?;
    let capability_version =
        u64::try_from(capability_version).map_err(|error| conversion_error(3, error))?;
    let credential_requirement = CredentialRequirement::parse(&row.get::<_, String>(4)?)
        .ok_or_else(|| conversion_message(4, "invalid credential requirement"))?;
    let credential_reference = row
        .get::<_, Option<String>>(5)?
        .map(CredentialReference::try_new)
        .transpose()
        .map_err(|error| conversion_error(5, error))?;
    let credential_status = ProviderCredentialStatus::parse(&row.get::<_, String>(6)?)
        .ok_or_else(|| conversion_message(6, "invalid provider credential status"))?;
    let configuration_digest = ConfigurationDigest::parse(row.get::<_, String>(7)?)
        .map_err(|error| conversion_error(7, error))?;
    let health = read_check(row, 8, 9, 10)?;
    let credential_test = read_check(row, 11, 12, 13)?;
    ProviderCapabilityState::try_new(
        provider_id,
        capability_id,
        capability_status,
        capability_version,
        credential_requirement,
        credential_reference,
        credential_status,
        configuration_digest,
        health,
        credential_test,
    )
    .map_err(|error| conversion_error(0, error))
}

fn read_check(
    row: &Row<'_>,
    status_index: usize,
    checked_at_index: usize,
    problem_index: usize,
) -> rusqlite::Result<ProviderCheckMetadata> {
    let status = ProviderCheckStatus::parse(&row.get::<_, String>(status_index)?)
        .ok_or_else(|| conversion_message(status_index, "invalid provider check status"))?;
    let checked_at = row
        .get::<_, Option<String>>(checked_at_index)?
        .map(|value| value.parse::<DateTime<Utc>>())
        .transpose()
        .map_err(|error| conversion_error(checked_at_index, error))?;
    let safe_problem_code = row
        .get::<_, Option<String>>(problem_index)?
        .map(|value| {
            ProblemCode::from_code(&value)
                .ok_or_else(|| conversion_message(problem_index, "unknown safe problem code"))
        })
        .transpose()?;
    ProviderCheckMetadata::try_new(status, checked_at, safe_problem_code)
        .map_err(|error| conversion_error(status_index, error))
}

fn conversion_error(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
}

fn conversion_message(index: usize, message: &'static str) -> rusqlite::Error {
    conversion_error(
        index,
        std::io::Error::new(std::io::ErrorKind::InvalidData, message),
    )
}

fn map_store_error(error: rusqlite::Error) -> ProviderStatePortError {
    match error {
        rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::IntegralValueOutOfRange(..)
        | rusqlite::Error::InvalidColumnType(..) => ProviderStatePortError::Corrupt,
        _ => ProviderStatePortError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;

    fn state(version: u64, digest_byte: &str) -> ProviderCapabilityState {
        ProviderCapabilityState::try_new(
            ProviderId::try_new("tmdb").expect("provider ID"),
            ProviderCapabilityId::try_new("metadata.search").expect("capability ID"),
            ProviderCapabilityStatus::Available,
            version,
            CredentialRequirement::ApiKey,
            Some(
                CredentialReference::try_new("secret:providers/tmdb/api-key")
                    .expect("credential reference"),
            ),
            ProviderCredentialStatus::StoredUnverified,
            ConfigurationDigest::parse(digest_byte.repeat(64)).expect("configuration digest"),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .expect("provider state")
    }

    #[test]
    fn provider_state_is_workspace_owned_and_durable() {
        let node = TestNode::new();
        let expected = state(1, "a");
        assert_eq!(
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), expected.clone()),
            Ok(ProviderStateWriteOutcome::Created)
        );
        assert_eq!(
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), expected.clone()),
            Ok(ProviderStateWriteOutcome::Unchanged)
        );
        assert_eq!(
            node.kernel
                .get_provider_capability_state(
                    WorkspaceId::new_v7(),
                    expected.provider_id(),
                    expected.capability_id(),
                )
                .expect("read another workspace"),
            None
        );

        let (root, access) = node.into_stopped();
        let kernel = SqliteKernel::open(root.path()).expect("reopen SQLite kernel");
        assert_eq!(
            kernel
                .get_provider_capability_state(
                    access.workspace_id(),
                    expected.provider_id(),
                    expected.capability_id(),
                )
                .expect("read provider state"),
            Some(expected)
        );
    }

    #[test]
    fn stale_or_conflicting_revision_is_failure_atomic() {
        let node = TestNode::new();
        let current = state(2, "b");
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), current.clone())
            .expect("store current state");

        for conflicting in [state(1, "a"), state(2, "c")] {
            assert_eq!(
                node.kernel
                    .put_provider_capability_state(node.access.workspace_id(), conflicting,),
                Err(ProviderStatePortError::RevisionConflict)
            );
        }
        assert_eq!(
            node.kernel
                .get_provider_capability_state(
                    node.access.workspace_id(),
                    current.provider_id(),
                    current.capability_id(),
                )
                .expect("read retained state"),
            Some(current)
        );
    }

    #[test]
    fn higher_capability_version_replaces_stored_state() {
        let node = TestNode::new();
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), state(1, "a"))
            .expect("store initial state");
        let replacement = state(2, "b");
        assert_eq!(
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), replacement.clone(),),
            Ok(ProviderStateWriteOutcome::Replaced)
        );
        assert_eq!(
            node.kernel
                .get_provider_capability_state(
                    node.access.workspace_id(),
                    replacement.provider_id(),
                    replacement.capability_id(),
                )
                .expect("read replacement"),
            Some(replacement)
        );
    }

    #[test]
    fn sqlite_surface_contains_no_secret_value_column() {
        let node = TestNode::new();
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), state(1, "a"))
            .expect("store provider state");
        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let columns: Vec<String> = connection
            .prepare("PRAGMA table_info(provider_capability_states)")
            .expect("table info")
            .query_map([], |row| row.get(1))
            .expect("column rows")
            .collect::<Result<_, _>>()
            .expect("columns");
        assert!(columns.contains(&"credential_reference".to_owned()));
        assert!(!columns.iter().any(|column| {
            matches!(
                column.as_str(),
                "secret" | "secret_value" | "credential_value" | "token" | "api_key"
            )
        }));
    }
}
