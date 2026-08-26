use crate::kernel::{
    authorize_transaction, digest_secret, map_sql, now, problem, random_secret, scope_storage_key,
    timestamp, SqliteKernel,
};
use chrono::{DateTime, Utc};
use fasti_application::{
    ClientCredentialAdministrationPort, ClientCredentialSummary, CreateScopedClientCredentialCommand,
    CreateScopedClientCredentialOutcome, FastiProblem, ListClientCredentialsQuery, ProblemCode,
    RevokeClientCredentialCommand, ScopeKey,
};
use fasti_domain::{ClientId, CredentialId, ProfileGrantId};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::collections::HashSet;

fn parse_utc_timestamp(
    value: &str,
    capability: fasti_application::CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> fasti_application::ApplicationResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

fn scope_from_storage(value: &str) -> Option<ScopeKey> {
    ScopeKey::ALL
        .iter()
        .copied()
        .find(|scope| scope_storage_key(*scope) == value)
}

impl ClientCredentialAdministrationPort for SqliteKernel {
    fn create_scoped_client_credential(
        &self,
        command: CreateScopedClientCredentialCommand,
    ) -> fasti_application::ApplicationResult<CreateScopedClientCredentialOutcome> {
        let capability = fasti_application::CapabilityKey::RotateCredential;
        let correlation_id = command.correlation_id();
        if command.scopes().is_empty() || command.scopes().len() > ScopeKey::ALL.len() {
            return Err(problem(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            ));
        }

        let mut requested = Vec::with_capacity(command.scopes().len());
        let mut unique = HashSet::with_capacity(command.scopes().len());
        for scope in command.scopes() {
            if *scope == ScopeKey::ClientEnroll || !unique.insert(scope_storage_key(*scope)) {
                return Err(problem(
                    ProblemCode::ValidationFailed,
                    capability,
                    correlation_id,
                ));
            }
            requested.push(*scope);
        }
        requested.sort_by_key(|scope| scope_storage_key(*scope));

        let secret = random_secret(capability, correlation_id)?;
        let digest = digest_secret(&secret);
        let client_id = ClientId::new_v7();
        let credential_id = CredentialId::new_v7();
        let grant_id = ProfileGrantId::new_v7();
        let created_at = now();
        let created_at_text = timestamp(created_at);

        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;

        let issuer_scopes: HashSet<String> = {
            let mut statement = map_sql(
                transaction.prepare("SELECT scope_key FROM grant_scopes WHERE grant_id = ?1"),
                capability,
                correlation_id,
            )?;
            let rows = map_sql(
                statement.query_map([command.access().grant_id().to_string()], |row| {
                    row.get::<_, String>(0)
                }),
                capability,
                correlation_id,
            )?;
            let mut scopes = HashSet::new();
            for row in rows {
                scopes.insert(map_sql(row, capability, correlation_id)?);
            }
            scopes
        };
        if requested
            .iter()
            .any(|scope| !issuer_scopes.contains(scope_storage_key(*scope)))
        {
            return Err(Box::new(FastiProblem::forbidden(capability, correlation_id)));
        }

        map_sql(
            transaction.execute(
                "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 1, ?3)",
                params![
                    client_id.to_string(),
                    command.access().workspace_id().to_string(),
                    created_at_text,
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO credentials(
                    credential_id, workspace_id, client_id, digest, epoch, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, 1, 'active', ?5)
                "#,
                params![
                    credential_id.to_string(),
                    command.access().workspace_id().to_string(),
                    client_id.to_string(),
                    digest,
                    created_at_text,
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO profile_grants(
                    grant_id, workspace_id, profile_id, client_id, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, 'active', ?5)
                "#,
                params![
                    grant_id.to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().profile_id().to_string(),
                    client_id.to_string(),
                    created_at_text,
                ],
            ),
            capability,
            correlation_id,
        )?;
        for scope in &requested {
            map_sql(
                transaction.execute(
                    "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                    params![grant_id.to_string(), scope_storage_key(*scope)],
                ),
                capability,
                correlation_id,
            )?;
        }
        map_sql(transaction.commit(), capability, correlation_id)?;

        Ok(CreateScopedClientCredentialOutcome::new(
            client_id,
            credential_id,
            command.access().profile_id(),
            requested,
            secret,
            created_at,
        ))
    }

    fn list_client_credentials(
        &self,
        query: ListClientCredentialsQuery,
    ) -> fasti_application::ApplicationResult<Vec<ClientCredentialSummary>> {
        let capability = fasti_application::CapabilityKey::RotateCredential;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;

        let rows = {
            let mut statement = map_sql(
                transaction.prepare(
                    r#"
                    SELECT c.client_id, cr.credential_id, pg.profile_id,
                           cr.status, cr.created_at, cr.revoked_at, pg.grant_id
                    FROM credentials cr
                    JOIN clients c
                      ON c.workspace_id = cr.workspace_id AND c.client_id = cr.client_id
                    JOIN profile_grants pg
                      ON pg.workspace_id = c.workspace_id AND pg.client_id = c.client_id
                    WHERE cr.workspace_id = ?1
                      AND pg.profile_id = ?2
                      AND cr.credential_id <> ?3
                    ORDER BY cr.created_at DESC, cr.credential_id DESC
                    "#,
                ),
                capability,
                correlation_id,
            )?;
            let mapped = map_sql(
                statement.query_map(
                    params![
                        query.access().workspace_id().to_string(),
                        query.access().profile_id().to_string(),
                        query.access().credential_id().to_string(),
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, String>(6)?,
                        ))
                    },
                ),
                capability,
                correlation_id,
            )?;
            let mut values = Vec::new();
            for row in mapped {
                values.push(map_sql(row, capability, correlation_id)?);
            }
            values
        };

        let mut summaries = Vec::with_capacity(rows.len());
        for (client_id, credential_id, profile_id, status, created_at, revoked_at, grant_id) in rows {
            let scopes = {
                let mut statement = map_sql(
                    transaction.prepare(
                        "SELECT scope_key FROM grant_scopes WHERE grant_id = ?1 ORDER BY scope_key",
                    ),
                    capability,
                    correlation_id,
                )?;
                let mapped = map_sql(
                    statement.query_map([grant_id], |row| row.get::<_, String>(0)),
                    capability,
                    correlation_id,
                )?;
                let mut values = Vec::new();
                for row in mapped {
                    let storage = map_sql(row, capability, correlation_id)?;
                    let scope = scope_from_storage(&storage).ok_or_else(|| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?;
                    values.push(scope);
                }
                values
            };
            summaries.push(ClientCredentialSummary::new(
                client_id.parse().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                credential_id.parse().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                profile_id.parse().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                scopes,
                status == "active",
                parse_utc_timestamp(&created_at, capability, correlation_id)?,
                revoked_at
                    .as_deref()
                    .map(|value| parse_utc_timestamp(value, capability, correlation_id))
                    .transpose()?,
            ));
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(summaries)
    }

    fn revoke_client_credential(
        &self,
        command: RevokeClientCredentialCommand,
    ) -> fasti_application::ApplicationResult<()> {
        let capability = fasti_application::CapabilityKey::RevokeCredential;
        let correlation_id = command.correlation_id();
        if command.credential_id() == command.access().credential_id() {
            return Err(Box::new(FastiProblem::forbidden(capability, correlation_id)));
        }
        let revoked_at = timestamp(now());
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;

        let target = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT cr.client_id, pg.grant_id
                    FROM credentials cr
                    JOIN profile_grants pg
                      ON pg.workspace_id = cr.workspace_id AND pg.client_id = cr.client_id
                    WHERE cr.credential_id = ?1
                      AND cr.workspace_id = ?2
                      AND pg.profile_id = ?3
                    "#,
                    params![
                        command.credential_id().to_string(),
                        command.access().workspace_id().to_string(),
                        command.access().profile_id().to_string(),
                    ],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional(),
            capability,
            correlation_id,
        )?
        .ok_or_else(|| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;

        let credential_changed = map_sql(
            transaction.execute(
                "UPDATE credentials SET status = 'revoked', revoked_at = ?1 WHERE credential_id = ?2 AND status = 'active'",
                params![revoked_at, command.credential_id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        if credential_changed > 1 {
            return Err(Box::new(FastiProblem::integrity_failed(capability, correlation_id)));
        }
        map_sql(
            transaction.execute(
                "UPDATE profile_grants SET status = 'revoked', revoked_at = COALESCE(revoked_at, ?1) WHERE grant_id = ?2 AND status = 'active'",
                params![revoked_at, target.1],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "UPDATE clients SET status = 'revoked' WHERE client_id = ?1 AND workspace_id = ?2",
                params![target.0, command.access().workspace_id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(())
    }
}
