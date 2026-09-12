use crate::browser_auth::{authenticate_session, viable_administrator_count};
use crate::kernel::{
    authorize_transaction, digest_secret, map_sql, now, problem, random_secret, scope_storage_key,
    timestamp, SqliteKernel,
};
use chrono::{DateTime, Utc};
use fasti_application::{
    AccessClientInventory, AccessInventoryPage, AccessInventoryQuery, ApplicationResult,
    CapabilityKey, ClientCredentialAdministrationPort, ClientCredentialSummary,
    CreateScopedClientCredentialCommand, CreateScopedClientCredentialOutcome, FastiProblem,
    ListClientCredentialsQuery, ProblemCode, RevokeClientCredentialCommand, ScopeKey,
};
use fasti_domain::{
    AccessCredentialName, ApplicationClient, ApplicationClientClassification,
    ApplicationClientLifecycle, ApplicationClientPurpose, AuthSubjectId, ClientAuthenticationType,
    ClientId, CredentialId, ProfileGrantId, RequestCorrelationId, WorkspaceId, WorkspaceRole,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::collections::HashSet;

pub(crate) fn list_access_clients(
    kernel: &SqliteKernel,
    query: AccessInventoryQuery<ClientId>,
) -> ApplicationResult<AccessClientInventory> {
    let capability = CapabilityKey::ListAccessClients;
    let request = query.browser_request();
    let correlation_id = request.correlation_id();
    let mut connection = kernel.lock_connection(capability, correlation_id)?;
    // Authentication can refresh activity. Keep that write, current membership
    // and the inventory in one transaction; request-time evidence is not a clock.
    let transaction = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Immediate),
        capability,
        correlation_id,
    )?;
    let current = authenticate_session(
        &transaction,
        request.session_secret(),
        None,
        now(),
        capability,
        correlation_id,
    )?;
    let workspace_id = current.session().workspace_id();
    let subject_id = current.subject().id();
    let role = map_sql(transaction.query_row(
        "SELECT role FROM workspace_memberships WHERE workspace_id = ?1 AND auth_subject_id = ?2 AND lifecycle = 'active'",
        params![workspace_id.to_string(), subject_id.to_string()],
        |row| row.get::<_, String>(0),
    ).optional(), capability, correlation_id)?
        .ok_or_else(|| problem(ProblemCode::SessionPolicyChanged, capability, correlation_id))?;
    let role = WorkspaceRole::from_storage(&role)
        .ok_or_else(|| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    let owner = (role == WorkspaceRole::Member).then_some(subject_id);
    let inventory = read_client_inventory(
        &transaction,
        workspace_id,
        owner,
        query.page(),
        correlation_id,
    )?;
    map_sql(transaction.commit(), capability, correlation_id)?;
    Ok(inventory)
}

fn read_client_inventory(
    connection: &rusqlite::Connection,
    workspace_id: WorkspaceId,
    owner: Option<AuthSubjectId>,
    page: &AccessInventoryPage<ClientId>,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessClientInventory> {
    let capability = CapabilityKey::ListAccessClients;
    let sql = client_inventory_sql(owner.is_some(), page.after().is_some());
    let mut statement = map_sql(connection.prepare(&sql), capability, correlation_id)?;
    let workspace = workspace_id.to_string();
    let owner = owner.map(|value| value.to_string());
    let after = page
        .after()
        .map(|(time, id)| (timestamp(*time), id.to_string()));
    let limit = i64::from(page.limit()) + 1;
    let mut bindings: Vec<(&str, &dyn rusqlite::ToSql)> =
        vec![(":workspace", &workspace), (":limit", &limit)];
    if let Some(owner) = &owner {
        bindings.push((":owner", owner));
    }
    if let Some((time, id)) = &after {
        bindings.push((":after_time", time));
        bindings.push((":after_id", id));
    }
    let mut rows = map_sql(
        statement.query(bindings.as_slice()),
        capability,
        correlation_id,
    )?;
    let mut clients = Vec::with_capacity(usize::from(page.limit()) + 1);
    while let Some(row) = map_sql(rows.next(), capability, correlation_id)? {
        clients.push(client_from_inventory_row(row, correlation_id)?);
    }
    Ok(AccessClientInventory::from_window(clients, page))
}

fn client_inventory_sql(member: bool, continuation: bool) -> String {
    // Only fixed SQL fragments are composed. Separate equality prefixes permit
    // indexed member/admin seeks without an OR over authority or cursor fields.
    let owner_filter = if member {
        "AND owner_subject_id = :owner"
    } else {
        ""
    };
    let cursor_filter = if continuation {
        "AND (inventory_created_year, inventory_created_tail, client_id) < (CAST(:after_time AS INTEGER), substr(:after_time, -23), :after_id)"
    } else {
        ""
    };
    format!(
        "SELECT client_id, workspace_id, owner_subject_id, name, authentication_type, purpose, status, current_credential_epoch,
         CASE WHEN length(CAST(created_at AS BLOB)) BETWEEN 20 AND 30 THEN created_at END
         FROM clients WHERE workspace_id = :workspace {owner_filter} {cursor_filter}
         ORDER BY inventory_created_year DESC, inventory_created_tail DESC, client_id DESC LIMIT :limit"
    )
}

fn client_from_inventory_row(
    row: &rusqlite::Row<'_>,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<ApplicationClient> {
    let capability = CapabilityKey::ListAccessClients;
    let invalid = || problem(ProblemCode::IntegrityFailed, capability, correlation_id);
    let text = |index| row.get::<_, String>(index).map_err(|_| invalid());
    let authentication = match text(4)?.as_str() {
        "first_party" => ClientAuthenticationType::FirstParty,
        "confidential" => ClientAuthenticationType::Confidential,
        _ => return Err(invalid()),
    };
    let purpose = match text(5)?.as_str() {
        "node" => ApplicationClientPurpose::Node,
        "cli" => ApplicationClientPurpose::Cli,
        "device" => ApplicationClientPurpose::Device,
        "integration" => ApplicationClientPurpose::Integration,
        _ => return Err(invalid()),
    };
    let lifecycle = match text(6)?.as_str() {
        "active" => ApplicationClientLifecycle::Active,
        "revoked" => ApplicationClientLifecycle::Revoked,
        _ => return Err(invalid()),
    };
    let owner = row.get::<_, Option<String>>(2).map_err(|_| invalid())?;
    let name = row.get::<_, Option<String>>(3).map_err(|_| invalid())?;
    ApplicationClient::try_from_persisted(
        text(0)?.parse().map_err(|_| invalid())?,
        text(1)?.parse().map_err(|_| invalid())?,
        owner
            .map(|value| value.parse().map_err(|_| invalid()))
            .transpose()?,
        name.map(|value| AccessCredentialName::try_new(&value).map_err(|_| invalid()))
            .transpose()?,
        ApplicationClientClassification::try_from_persisted(authentication, purpose)
            .map_err(|_| invalid())?,
        lifecycle,
        u64::try_from(row.get::<_, i64>(7).map_err(|_| invalid())?).map_err(|_| invalid())?,
        canonical_inventory_timestamp(&text(8)?).ok_or_else(invalid)?,
    )
    .map_err(|_| invalid())
}

pub(crate) fn canonical_inventory_timestamp(value: &str) -> Option<DateTime<Utc>> {
    // FromStr supports the domain's signed/expanded years; exact round-trip
    // equality rejects its extra permissive syntax and preserves leap seconds.
    // Published migration fixtures also retain seconds-only UTC storage bytes.
    if !(20..=30).contains(&value.len()) {
        return None;
    }
    value.parse::<DateTime<Utc>>().ok().filter(|parsed| {
        timestamp(*parsed) == value
            || parsed.to_rfc3339_opts(chrono::SecondsFormat::Secs, true) == value
    })
}

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
            return Err(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )));
        }

        map_sql(
            transaction.execute(
                "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at, authentication_type, purpose) VALUES (?1, ?2, 'active', 1, ?3, 'confidential', 'integration')",
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
        for (client_id, credential_id, profile_id, status, created_at, revoked_at, grant_id) in rows
        {
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
            return Err(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )));
        }
        let revoked_at = timestamp(now());
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let workspace_id = command.access().workspace_id().to_string();
        let viable_administrators_before =
            viable_administrator_count(&transaction, &workspace_id, capability, correlation_id)?;

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
                      AND cr.status = 'active'
                      AND pg.profile_id = ?3
                      AND pg.status = 'active'
                    "#,
                    params![
                        command.credential_id().to_string(),
                        workspace_id,
                        command.access().profile_id().to_string(),
                    ],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional(),
            capability,
            correlation_id,
        )?
        .ok_or_else(|| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;

        if target.0 == command.access().client_id().to_string() {
            return Err(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )));
        }

        let credential_changed = map_sql(
            transaction.execute(
                "UPDATE credentials SET status = 'revoked', revoked_at = ?1 WHERE credential_id = ?2 AND status = 'active'",
                params![revoked_at, command.credential_id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        if credential_changed != 1 {
            return Err(Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            )));
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
                params![target.0, workspace_id],
            ),
            capability,
            correlation_id,
        )?;
        if viable_administrators_before > 0
            && viable_administrator_count(&transaction, &workspace_id, capability, correlation_id)?
                == 0
        {
            return Err(Box::new(FastiProblem::forbidden(
                capability,
                correlation_id,
            )));
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::*;

    fn database() -> (
        rusqlite::Connection,
        WorkspaceId,
        AuthSubjectId,
        AuthSubjectId,
    ) {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .unwrap();
        crate::schema::migrate(&connection).unwrap();
        let (workspace, owner, other) = seed_inventory(&connection);
        (connection, workspace, owner, other)
    }

    fn seed_inventory(
        connection: &rusqlite::Connection,
    ) -> (WorkspaceId, AuthSubjectId, AuthSubjectId) {
        let workspace = WorkspaceId::new_v7();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![workspace.to_string(), timestamp(now())],
            )
            .unwrap();
        let subjects = [AuthSubjectId::new_v7(), AuthSubjectId::new_v7()];
        for subject in subjects {
            connection.execute("INSERT INTO auth_subjects(auth_subject_id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at) VALUES (?1, 'active', 1, 1, ?2, ?2)",
                params![subject.to_string(), timestamp(now())]).unwrap();
        }
        (workspace, subjects[0], subjects[1])
    }

    fn insert(
        connection: &rusqlite::Connection,
        workspace: WorkspaceId,
        owner: Option<AuthSubjectId>,
        time: &str,
    ) -> ClientId {
        let id = ClientId::new_v7();
        connection.execute("INSERT INTO clients(client_id, workspace_id, owner_subject_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, ?3, 'revoked', 0, ?4)",
            params![id.to_string(), workspace.to_string(), owner.map(|id| id.to_string()), time]).unwrap();
        id
    }

    fn collect_pages(
        connection: &rusqlite::Connection,
        workspace: WorkspaceId,
        owner: Option<AuthSubjectId>,
        limit: u16,
    ) -> Vec<ClientId> {
        let mut page = AccessInventoryPage::try_new(Some(limit), None, None).unwrap();
        let mut all = Vec::new();
        loop {
            let result = read_client_inventory(
                connection,
                workspace,
                owner,
                &page,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
            assert!(result.clients().len() <= usize::from(limit));
            for client in result.clients() {
                assert_eq!(client.workspace_id(), workspace);
                assert_eq!(client.lifecycle(), ApplicationClientLifecycle::Revoked);
                assert_eq!(client.current_credential_epoch(), 0);
                if let Some(owner) = owner {
                    assert_eq!(client.owner_subject_id(), Some(owner));
                }
                all.push(client.id());
            }
            let Some((time, id)) = result.next() else {
                break;
            };
            assert_eq!(Some(id), all.last());
            page = AccessInventoryPage::try_new(Some(limit), Some(*time), Some(*id)).unwrap();
            assert!(
                all.len() <= 1000,
                "pagination must terminate without repeated pages"
            );
        }
        all
    }

    #[test]
    fn inventory_mixed_precision_paging_filters_before_limit_and_preserves_bytes() {
        let (connection, workspace, owner, other) = database();
        let other_workspace = WorkspaceId::new_v7();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![other_workspace.to_string(), timestamp(now())],
            )
            .unwrap();
        let mut expected_admin = Vec::new();
        let mut expected_member = Vec::new();
        for index in 0..240 {
            let time = if index % 2 == 0 {
                "2026-08-24T00:00:02Z"
            } else {
                "2026-08-24T00:00:02.000000Z"
            };
            let subject = match index % 3 {
                0 => Some(owner),
                1 => Some(other),
                _ => None,
            };
            let id = insert(&connection, workspace, subject, time);
            expected_admin.push(id);
            if subject == Some(owner) {
                expected_member.push(id);
            }
            insert(&connection, other_workspace, subject, time);
        }
        expected_admin.sort_unstable_by_key(|id| std::cmp::Reverse(id.to_string()));
        expected_member.sort_unstable_by_key(|id| std::cmp::Reverse(id.to_string()));
        for limit in [1, 32, 100] {
            assert_eq!(
                collect_pages(&connection, workspace, None, limit),
                expected_admin
            );
            assert_eq!(
                collect_pages(&connection, workspace, Some(owner), limit),
                expected_member
            );
        }
        let unchanged: i64 = connection
            .query_row(
                "SELECT count(*) FROM clients WHERE created_at = '2026-08-24T00:00:02Z'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unchanged, 240);
    }

    #[test]
    fn inventory_paging_preserves_extreme_years_and_leap_seconds() {
        let (connection, workspace, owner, _) = database();
        let mut expected = Vec::new();
        let times = [
            timestamp(DateTime::<Utc>::MIN_UTC),
            "-0001-12-31T23:59:59Z".into(),
            "0000-01-01T00:00:00.000000Z".into(),
            "2016-12-31T23:59:59.999999Z".into(),
            "2016-12-31T23:59:60Z".into(),
            "2016-12-31T23:59:60.000000Z".into(),
            "2016-12-31T23:59:60.999999Z".into(),
            "2017-01-01T00:00:00Z".into(),
            "9999-12-31T23:59:59Z".into(),
            "+10000-01-01T00:00:00.000000Z".into(),
            timestamp(DateTime::<Utc>::MAX_UTC),
        ];
        for time in times {
            let id = insert(&connection, workspace, Some(owner), &time);
            expected.push((canonical_inventory_timestamp(&time).unwrap(), id));
        }
        expected.sort_unstable_by_key(|(time, id)| std::cmp::Reverse((*time, id.to_string())));
        let ids: Vec<_> = expected.into_iter().map(|(_, id)| id).collect();
        assert_eq!(collect_pages(&connection, workspace, None, 1), ids);
        assert_eq!(collect_pages(&connection, workspace, Some(owner), 1), ids);
    }

    #[test]
    fn inventory_queries_use_full_indexed_seeks_without_sorting() {
        let (connection, _, _, _) = database();
        for member in [false, true] {
            for continuation in [false, true] {
                let sql = format!(
                    "EXPLAIN QUERY PLAN {}",
                    client_inventory_sql(member, continuation)
                );
                let mut statement = connection.prepare(&sql).unwrap();
                let mut rows = statement.raw_query();
                let mut details = String::new();
                while let Some(row) = rows.next().unwrap() {
                    details.push_str(&row.get::<_, String>(3).unwrap());
                }
                let index = if member {
                    "clients_inventory_owner_idx"
                } else {
                    "clients_inventory_workspace_idx"
                };
                assert!(details.contains(index), "{details}");
                assert!(!details.contains("TEMP B-TREE"), "{details}");
                if continuation {
                    assert!(
                        details.contains(
                            "(inventory_created_year,inventory_created_tail,client_id)<(?,?,?)"
                        ),
                        "{details}"
                    );
                }
            }
        }
    }

    #[test]
    fn inventory_rejects_noncanonical_returned_time_instead_of_skipping_it() {
        for time in [
            "not-a-time",
            "2026-08-24T00:00:02.000000000000000000Z",
            "2026-08-24T00:00:02.123Z",
            "2026-08-24T00:00:02+00:00",
            " 2026-08-24T00:00:02Z",
        ] {
            let (connection, workspace, owner, _) = database();
            insert(&connection, workspace, Some(owner), time);
            let error = read_client_inventory(
                &connection,
                workspace,
                Some(owner),
                &AccessInventoryPage::try_new(None, None, None).unwrap(),
                RequestCorrelationId::new_v7(),
            )
            .unwrap_err();
            assert_eq!(error.code(), ProblemCode::IntegrityFailed);
        }
    }

    #[test]
    fn inventory_authorized_demotion_invalidates_old_session_and_limits_new_session() {
        use fasti_application::{
            AccessAdministrationPort, BrowserRequestBoundaryPolicy, BrowserSessionMutationCommand,
            BrowserSessionPort, BrowserSessionQuery, ChangeMembershipRoleCommand,
            CreateBrowserSessionCommand, CreatedBrowserSession, HumanAccessPort, SecretMaterial,
            SessionPolicy, VerifyTrailBaseInstallationCommand,
        };
        use fasti_domain::{MembershipId, ProfileId, Sha256Digest, TrailBaseInstanceId};

        let root = tempfile::tempdir().unwrap();
        let kernel = SqliteKernel::open(root.path()).unwrap();
        let profile = ProfileId::new_v7();
        let grant = ProfileGrantId::new_v7();
        let target_membership = MembershipId::new_v7();
        let (workspace, actor, target, target_client) = {
            let connection = kernel.inner.connection.lock().unwrap();
            let (workspace, actor, target) = seed_inventory(&connection);
            let at = timestamp(now());
            let node = insert(&connection, workspace, None, &at);
            connection
                .execute(
                    "UPDATE clients SET status = 'active' WHERE client_id = ?1",
                    [node.to_string()],
                )
                .unwrap();
            connection.execute("INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)", params![profile.to_string(), workspace.to_string(), at]).unwrap();
            connection.execute("INSERT INTO profile_grants(grant_id, workspace_id, profile_id, client_id, status, created_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5)", params![grant.to_string(), workspace.to_string(), profile.to_string(), node.to_string(), at]).unwrap();
            // Two active administrators preserve administrator continuity.
            for (subject, membership) in
                [(actor, MembershipId::new_v7()), (target, target_membership)]
            {
                connection.execute("INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)", params![subject.to_string(), grant.to_string()]).unwrap();
                connection.execute("INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'administrator', ?4, ?4)", params![membership.to_string(), subject.to_string(), workspace.to_string(), at]).unwrap();
            }
            insert(&connection, workspace, Some(actor), &at);
            let target_client = insert(&connection, workspace, Some(target), &at);
            (workspace, actor, target, target_client)
        };
        let installation = HumanAccessPort::verify_trailbase_installation(
            &kernel,
            VerifyTrailBaseInstallationCommand::new(
                TrailBaseInstanceId::new_v7(),
                Sha256Digest::from_bytes(&[81; 32]),
                Sha256Digest::from_bytes(&[82; 32]),
                false,
                RequestCorrelationId::new_v7(),
                now(),
            ),
        )
        .unwrap();
        let create_session = |subject| {
            BrowserSessionPort::create_browser_session(
                &kernel,
                CreateBrowserSessionCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    subject,
                    workspace,
                    vec![grant],
                    grant,
                    SessionPolicy::C1,
                    false,
                    now(),
                )
                .unwrap(),
            )
            .unwrap()
        };
        let read = |session: &CreatedBrowserSession| {
            AccessInventoryQuery::new(
                BrowserSessionQuery::new(
                    RequestCorrelationId::new_v7(),
                    SecretMaterial::try_from_hex(&session.session_secret().expose_hex()).unwrap(),
                    now(),
                ),
                BrowserRequestBoundaryPolicy::try_new("http://127.0.0.1:8420", "127.0.0.1:8420")
                    .unwrap()
                    .validate_read(Some("127.0.0.1:8420"))
                    .unwrap(),
                AccessInventoryPage::try_new(None, None, None).unwrap(),
            )
        };
        let actor_session = create_session(actor);
        let old_target_session = create_session(target);
        assert_eq!(old_target_session.session().authorization_epoch(), 1);
        assert_eq!(
            kernel
                .list_access_clients(read(&old_target_session))
                .unwrap()
                .clients()
                .len(),
            3
        );
        let proof_at = now();
        {
            let connection = kernel.inner.connection.lock().unwrap();
            // Test-only recent-authentication precondition, matching the existing
            // human_access fixture. This proves the downstream transaction, not
            // a genuine TrailBase fresh-authentication flow.
            connection.execute("INSERT INTO fasti_browser_session_authentication(browser_session_id, trailbase_instance_id, activation_generation, method, verified_at, recent_authentication_expires_at) VALUES (?1, ?2, ?3, 'trailbase_password', ?4, ?5)",
                params![actor_session.session().id().to_string(), installation.id().to_string(),
                    i64::try_from(installation.activation_generation()).unwrap(), timestamp(proof_at),
                    timestamp(proof_at + chrono::TimeDelta::minutes(10))]).unwrap();
        }
        assert!(HumanAccessPort::change_membership_role(
            &kernel,
            ChangeMembershipRoleCommand::new(
                BrowserSessionMutationCommand::new(
                    RequestCorrelationId::new_v7(),
                    SecretMaterial::try_from_hex(&actor_session.session_secret().expose_hex())
                        .unwrap(),
                    SecretMaterial::try_from_hex(&actor_session.csrf_secret().expose_hex())
                        .unwrap(),
                    BrowserRequestBoundaryPolicy::try_new(
                        "http://127.0.0.1:8420",
                        "127.0.0.1:8420"
                    )
                    .unwrap()
                    .validate(Some("http://127.0.0.1:8420"), Some("127.0.0.1:8420"))
                    .unwrap(),
                    proof_at
                ),
                target_membership,
                WorkspaceRole::Member,
            ),
        )
        .unwrap());
        assert_eq!(
            kernel
                .list_access_clients(read(&old_target_session))
                .unwrap_err()
                .code(),
            ProblemCode::SessionPolicyChanged
        );
        let renewed_target_session = create_session(target);
        assert_eq!(renewed_target_session.session().authorization_epoch(), 2);
        let member_inventory = kernel
            .list_access_clients(read(&renewed_target_session))
            .unwrap();
        assert_eq!(member_inventory.clients().len(), 1);
        assert_eq!(member_inventory.clients()[0].id(), target_client);
        assert_eq!(
            member_inventory.clients()[0].owner_subject_id(),
            Some(target)
        );
        assert!(member_inventory.next().is_none());
        assert_eq!(
            kernel
                .list_access_clients(read(&actor_session))
                .unwrap()
                .clients()
                .len(),
            3
        );
    }

    #[test]
    fn inventory_browser_read_rechecks_role_and_execution_time_and_commits_activity() {
        use fasti_application::{
            AccessAdministrationPort, BrowserRequestBoundaryPolicy, BrowserSessionPort,
            BrowserSessionQuery, CreateBrowserSessionCommand, SecretMaterial, SessionPolicy,
        };
        use fasti_domain::{MembershipId, ProfileId};
        let root = tempfile::tempdir().unwrap();
        let kernel = SqliteKernel::open(root.path()).unwrap();
        let created = now() - chrono::TimeDelta::seconds(20);
        let profile = ProfileId::new_v7();
        let grant = ProfileGrantId::new_v7();
        let (workspace, owner, own_client) = {
            let connection = kernel.inner.connection.lock().unwrap();
            let (workspace, owner, other) = seed_inventory(&connection);
            let node = insert(&connection, workspace, None, &timestamp(created));
            connection
                .execute(
                    "UPDATE clients SET status = 'active' WHERE client_id = ?1",
                    [node.to_string()],
                )
                .unwrap();
            connection.execute("INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)", params![profile.to_string(), workspace.to_string(), timestamp(created)]).unwrap();
            connection.execute("INSERT INTO profile_grants(grant_id, workspace_id, profile_id, client_id, status, created_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5)", params![grant.to_string(), workspace.to_string(), profile.to_string(), node.to_string(), timestamp(created)]).unwrap();
            connection.execute("INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)", params![owner.to_string(), grant.to_string()]).unwrap();
            connection.execute("INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'administrator', ?4, ?4)", params![MembershipId::new_v7().to_string(), owner.to_string(), workspace.to_string(), timestamp(created)]).unwrap();
            let own_client = insert(&connection, workspace, Some(owner), &timestamp(created));
            insert(&connection, workspace, Some(other), &timestamp(created));
            (workspace, owner, own_client)
        };
        let session = kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    owner,
                    workspace,
                    vec![grant],
                    grant,
                    SessionPolicy::try_new(
                        std::time::Duration::from_secs(60),
                        std::time::Duration::from_secs(120),
                        std::time::Duration::from_secs(240),
                        std::time::Duration::from_secs(10),
                    )
                    .unwrap(),
                    false,
                    created,
                )
                .unwrap(),
            )
            .unwrap();
        let query = || {
            let boundary =
                BrowserRequestBoundaryPolicy::try_new("http://127.0.0.1:8420", "127.0.0.1:8420")
                    .unwrap()
                    .validate_read(Some("127.0.0.1:8420"))
                    .unwrap();
            AccessInventoryQuery::new(
                BrowserSessionQuery::new(
                    RequestCorrelationId::new_v7(),
                    SecretMaterial::try_from_hex(&session.session_secret().expose_hex()).unwrap(),
                    created,
                ),
                boundary,
                AccessInventoryPage::try_new(None, None, None).unwrap(),
            )
        };
        assert_eq!(
            kernel.list_access_clients(query()).unwrap().clients().len(),
            3
        );
        {
            let connection = kernel.inner.connection.lock().unwrap();
            let seen: String = connection
                .query_row(
                    "SELECT last_seen_at FROM fasti_browser_sessions WHERE browser_session_id = ?1",
                    [session.session().id().to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(canonical_inventory_timestamp(&seen).unwrap() > created);
            // Direct role change isolates the read's role reload. Real demotion
            // also advances the subject epoch and invalidates the old session.
            connection.execute("UPDATE workspace_memberships SET role = 'member' WHERE auth_subject_id = ?1 AND workspace_id = ?2", params![owner.to_string(), workspace.to_string()]).unwrap();
        }
        let member = kernel.list_access_clients(query()).unwrap();
        assert_eq!(member.clients().len(), 1);
        assert_eq!(member.clients()[0].id(), own_client);
        {
            let connection = kernel.inner.connection.lock().unwrap();
            // Valid historical request time cannot revive an expired session.
            connection.execute("UPDATE fasti_browser_sessions SET last_seen_at = ?1, idle_expires_at = ?2 WHERE browser_session_id = ?3", params![timestamp(created), timestamp(created + chrono::TimeDelta::seconds(1)), session.session().id().to_string()]).unwrap();
        }
        assert_eq!(
            kernel.list_access_clients(query()).unwrap_err().code(),
            ProblemCode::BrowserSessionExpired
        );
    }
}
