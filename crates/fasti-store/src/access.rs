use crate::kernel::{
    authorize_transaction, digest_secret, load_access_snapshot, map_sql, now, problem,
    random_secret, scope_storage_key, timestamp, verify_digest, SqliteKernel,
};
use chrono::{DateTime, Duration, Utc};
use fasti_application::{
    authorize, AccessAdministrationPort, AccessSnapshot, ApplicationResult,
    AuthenticateCredentialQuery, AuthorizationRequirement, CapabilityKey, ConfigureListenerCommand,
    EnrollFirstClientCommand, EnrollFirstClientOutcome, FastiProblem, InitializeNodeCommand,
    InitializeNodeOutcome, ListenerConfiguration, ProblemCode, ProfileSelectionOutcome,
    RequestAccessContext, RevokeCredentialCommand, RotateCredentialCommand,
    RotateCredentialOutcome, ScopeKey,
};
use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId, WorkspaceId};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

const INITIALIZATION_LIFETIME_MINUTES: i64 = 10;
const B2_ADMIN_SCOPES: &[ScopeKey] = &[
    ScopeKey::CapabilityRead,
    ScopeKey::ProfileSelect,
    ScopeKey::CredentialManage,
    ScopeKey::ListenerConfigure,
    ScopeKey::ObservationAccept,
    ScopeKey::ReceiptRead,
    ScopeKey::IdentityWrite,
    ScopeKey::ReviewRead,
    ScopeKey::ReviewWrite,
];

impl AccessAdministrationPort for SqliteKernel {
    fn initialize_node(
        &self,
        command: InitializeNodeCommand,
    ) -> ApplicationResult<InitializeNodeOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::InitializeNode;
        let workspace_id = WorkspaceId::new_v7();
        let profile_id = ProfileId::new_v7();
        let client_id = ClientId::new_v7();
        let proof = random_secret(capability, correlation_id)?;
        let proof_digest = digest_secret(&proof);
        let created_at = now();
        let expires_at = created_at + Duration::minutes(INITIALIZATION_LIFETIME_MINUTES);

        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let exists: bool = map_sql(
            transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM node_state WHERE singleton = 1)",
                [],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;
        if exists {
            return Err(Box::new(FastiProblem::already_initialized(correlation_id)));
        }
        authorize(
            &AuthorizationRequirement::for_capability(capability),
            None,
            Some(&AccessSnapshot::bootstrap_open()),
        )
        .map_err(|_| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;

        let created_at_text = timestamp(created_at);
        map_sql(
            transaction.execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![workspace_id.to_string(), created_at_text],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                params![
                    profile_id.to_string(),
                    workspace_id.to_string(),
                    created_at_text
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 0, ?3)",
                params![client_id.to_string(), workspace_id.to_string(), created_at_text],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO node_state(
                    singleton, initialized, workspace_id, profile_id, client_id,
                    initialization_digest, initialization_expires_at, created_at
                ) VALUES (1, 1, ?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    workspace_id.to_string(),
                    profile_id.to_string(),
                    client_id.to_string(),
                    proof_digest,
                    timestamp(expires_at),
                    created_at_text
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;

        Ok(InitializeNodeOutcome::new(
            workspace_id,
            profile_id,
            client_id,
            proof,
        ))
    }

    fn enroll_first_client(
        &self,
        command: EnrollFirstClientCommand,
    ) -> ApplicationResult<EnrollFirstClientOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::EnrollFirstClient;
        let credential = random_secret(capability, correlation_id)?;
        let credential_digest = digest_secret(&credential);
        let presented_proof_digest = digest_secret(command.initialization_proof());
        let credential_id = CredentialId::new_v7();
        let grant_id = ProfileGrantId::new_v7();
        let created_at = now();

        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let state = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT workspace_id, profile_id, client_id,
                           initialization_digest, initialization_expires_at,
                           initialization_consumed_at
                    FROM node_state WHERE singleton = 1
                    "#,
                    [],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<String>>(5)?,
                        ))
                    },
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let Some((workspace, profile, client, stored_proof_digest, expires_at, consumed_at)) =
            state
        else {
            return Err(Box::new(FastiProblem::bootstrap_closed(correlation_id)));
        };
        let expires_at = DateTime::parse_from_rfc3339(&expires_at)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        if consumed_at.is_some()
            || expires_at < created_at
            || !verify_digest(&stored_proof_digest, &presented_proof_digest)
        {
            return Err(Box::new(FastiProblem::bootstrap_closed(correlation_id)));
        }
        authorize(
            &AuthorizationRequirement::for_capability(capability),
            None,
            Some(&AccessSnapshot::bootstrap_open()),
        )
        .map_err(|_| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;

        let workspace_id = workspace
            .parse::<WorkspaceId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let profile_id = profile
            .parse::<ProfileId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let client_id = client
            .parse::<ClientId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let created_at_text = timestamp(created_at);

        map_sql(
            transaction.execute(
                "UPDATE clients SET current_credential_epoch = 1 WHERE client_id = ?1 AND status = 'active'",
                [client.as_str()],
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
                    workspace,
                    client,
                    credential_digest,
                    created_at_text
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
                    workspace,
                    profile,
                    client,
                    created_at_text
                ],
            ),
            capability,
            correlation_id,
        )?;
        for scope in B2_ADMIN_SCOPES {
            map_sql(
                transaction.execute(
                    "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                    params![grant_id.to_string(), scope_storage_key(*scope)],
                ),
                capability,
                correlation_id,
            )?;
        }
        map_sql(
            transaction.execute(
                r#"
                UPDATE node_state
                SET initialization_consumed_at = ?1,
                    initialization_digest = NULL,
                    initialization_expires_at = NULL
                WHERE singleton = 1
                "#,
                [created_at_text.as_str()],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;

        Ok(EnrollFirstClientOutcome::new(
            RequestAccessContext::new(
                workspace_id,
                profile_id,
                client_id,
                credential_id,
                grant_id,
                1,
            ),
            credential,
        ))
    }

    fn authenticate_credential(
        &self,
        query: AuthenticateCredentialQuery,
    ) -> ApplicationResult<RequestAccessContext> {
        let correlation_id = query.correlation_id();
        let capability = CapabilityKey::DiscoverCapabilities;
        let digest = digest_secret(query.credential());
        let connection = self.lock_connection(capability, correlation_id)?;
        let row = map_sql(
            connection
                .query_row(
                    r#"
                    SELECT cr.workspace_id, pg.profile_id, cr.client_id,
                           cr.credential_id, pg.grant_id, cr.epoch
                    FROM credentials cr
                    JOIN clients c ON c.client_id = cr.client_id
                    JOIN profile_grants pg ON pg.client_id = cr.client_id
                    WHERE cr.digest = ?1
                      AND cr.status = 'active'
                      AND c.status = 'active'
                      AND pg.status = 'active'
                      AND cr.epoch = c.current_credential_epoch
                    "#,
                    [digest],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, i64>(5)?,
                        ))
                    },
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let Some((workspace, profile, client, credential, grant, epoch)) = row else {
            return Err(Box::new(FastiProblem::authentication_failed(
                capability,
                correlation_id,
            )));
        };
        let access = RequestAccessContext::new(
            workspace.parse::<WorkspaceId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            profile.parse::<ProfileId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            client.parse::<ClientId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            credential.parse::<CredentialId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            grant.parse::<ProfileGrantId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            u64::try_from(epoch).unwrap_or(u64::MAX),
        );
        let snapshot = load_access_snapshot(&connection, &access, capability, correlation_id)?;
        if !snapshot.is_established() {
            return Err(Box::new(FastiProblem::authentication_failed(
                capability,
                correlation_id,
            )));
        }
        Ok(access)
    }

    fn rotate_credential(
        &self,
        command: RotateCredentialCommand,
    ) -> ApplicationResult<RotateCredentialOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::RotateCredential;
        let replacement = random_secret(capability, correlation_id)?;
        let replacement_digest = digest_secret(&replacement);
        let replacement_id = CredentialId::new_v7();
        let new_epoch = command
            .access()
            .presented_credential_epoch()
            .checked_add(1)
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        let created_at = timestamp(now());

        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        map_sql(
            transaction.execute(
                "UPDATE credentials SET status = 'revoked', revoked_at = ?1 WHERE credential_id = ?2 AND status = 'active'",
                params![created_at, command.access().credential_id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "UPDATE clients SET current_credential_epoch = ?1 WHERE client_id = ?2 AND status = 'active'",
                params![i64::try_from(new_epoch).unwrap_or(i64::MAX), command.access().client_id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO credentials(
                    credential_id, workspace_id, client_id, digest, epoch, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)
                "#,
                params![
                    replacement_id.to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().client_id().to_string(),
                    replacement_digest,
                    i64::try_from(new_epoch).unwrap_or(i64::MAX),
                    created_at
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;

        Ok(RotateCredentialOutcome::new(
            RequestAccessContext::new(
                command.access().workspace_id(),
                command.access().profile_id(),
                command.access().client_id(),
                replacement_id,
                command.access().grant_id(),
                new_epoch,
            ),
            replacement,
        ))
    }

    fn revoke_credential(&self, command: RevokeCredentialCommand) -> ApplicationResult<()> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::RevokeCredential;
        let revoked_at = timestamp(now());
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let changed = map_sql(
            transaction.execute(
                r#"
                UPDATE credentials SET status = 'revoked', revoked_at = ?1
                WHERE credential_id = ?2
                  AND workspace_id = ?3
                  AND client_id = ?4
                  AND status = 'active'
                "#,
                params![
                    revoked_at,
                    command.target_credential_id().to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().client_id().to_string()
                ],
            ),
            capability,
            correlation_id,
        )?;
        if changed != 1 {
            return Err(Box::new(FastiProblem::authentication_failed(
                capability,
                correlation_id,
            )));
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(())
    }

    fn select_profile(
        &self,
        correlation_id: fasti_domain::RequestCorrelationId,
        access: RequestAccessContext,
    ) -> ApplicationResult<ProfileSelectionOutcome> {
        let capability = CapabilityKey::SelectProfile;
        let connection = self.lock_connection(capability, correlation_id)?;
        let snapshot = load_access_snapshot(&connection, &access, capability, correlation_id)?;
        authorize(
            &AuthorizationRequirement::for_capability(capability),
            Some(&access),
            Some(&snapshot),
        )
        .map_err(|_| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;
        Ok(ProfileSelectionOutcome::new(
            access.workspace_id(),
            access.profile_id(),
        ))
    }

    fn configure_listener(
        &self,
        command: ConfigureListenerCommand,
    ) -> ApplicationResult<ListenerConfiguration> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::ConfigureListener;
        if command.loopback_port() == 0 {
            return Err(Box::new(FastiProblem::unsupported_listener(correlation_id)));
        }
        let listen = format!("127.0.0.1:{}", command.loopback_port());
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
                INSERT INTO listener_configuration(singleton, listen, remote_enabled, updated_at)
                VALUES (1, ?1, 0, ?2)
                ON CONFLICT(singleton) DO UPDATE SET
                    listen = excluded.listen,
                    remote_enabled = 0,
                    updated_at = excluded.updated_at
                "#,
                params![listen, timestamp(now())],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(ListenerConfiguration::new(listen, false))
    }
}
