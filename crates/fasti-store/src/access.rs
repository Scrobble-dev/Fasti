use crate::kernel::{
    authorize_transaction, digest_secret, harden_private_regular_file, load_access_snapshot,
    map_sql, now, problem, random_secret, scope_storage_key, timestamp, verify_digest,
    SqliteKernel,
};
use chrono::{DateTime, Duration, Utc};
use fasti_application::{
    authorize, AccessAdministrationPort, AccessSnapshot, ApplicationResult,
    AuthenticateCredentialQuery, AuthorizationRequirement, CapabilityKey,
    CompleteRecoveryBootstrapOutcome, CompleteRecoveryBootstrapRequest, ConfigureListenerCommand,
    EnrollFirstClientCommand, EnrollFirstClientOutcome, FastiProblem, InitializeNodeCommand,
    InitializeNodeOutcome, ListenerConfiguration, PortabilityFailureReceipt, PortabilityResult,
    PrepareRecoveryBootstrapOutcome, PrepareRecoveryBootstrapRequest, ProblemCode,
    ProfileSelectionOutcome, RequestAccessContext, RevokeCredentialCommand,
    RotateCredentialCommand, RotateCredentialOutcome, ScopeKey, SecretMaterial,
};
use fasti_domain::{ClientId, CredentialId, ProfileGrantId, ProfileId, WorkspaceId};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use std::fs::OpenOptions;
use std::io::{Read, Write};

const INITIALIZATION_LIFETIME_MINUTES: i64 = 10;
pub(crate) const FULL_ADMIN_SCOPES: &[ScopeKey] = &[
    ScopeKey::CapabilityRead,
    ScopeKey::ProfileSelect,
    ScopeKey::CredentialManage,
    ScopeKey::ListenerConfigure,
    ScopeKey::ObservationAccept,
    ScopeKey::ReceiptRead,
    ScopeKey::IdentityWrite,
    ScopeKey::IdentityRead,
    ScopeKey::ProfileStateRead,
    ScopeKey::ProfileStateWrite,
    ScopeKey::ReviewRead,
    ScopeKey::ReviewWrite,
    ScopeKey::CorrectionRead,
    ScopeKey::CorrectionWrite,
    ScopeKey::WorkspaceExport,
    ScopeKey::WorkspaceVerify,
    ScopeKey::BrowserUserManage,
];

pub(crate) const V8_NODE_OWNER_SCOPE_BACKFILL: &[ScopeKey] = &[
    ScopeKey::ProfileStateRead,
    ScopeKey::ProfileStateWrite,
    ScopeKey::ReviewRead,
    ScopeKey::ReviewWrite,
    ScopeKey::CorrectionRead,
    ScopeKey::CorrectionWrite,
    ScopeKey::WorkspaceExport,
    ScopeKey::WorkspaceVerify,
    ScopeKey::BrowserUserManage,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryTransactionError {
    BootstrapClosed,
    Integrity,
    Pending,
    Storage,
    Validation,
}

fn recovery_sql<T>(result: rusqlite::Result<T>) -> Result<T, RecoveryTransactionError> {
    result.map_err(|_| RecoveryTransactionError::Storage)
}

fn recovery_problem(
    error: RecoveryTransactionError,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    let code = match error {
        RecoveryTransactionError::BootstrapClosed => ProblemCode::BootstrapClosed,
        RecoveryTransactionError::Integrity => ProblemCode::IntegrityFailed,
        RecoveryTransactionError::Pending => ProblemCode::RecoveryBootstrapPending,
        RecoveryTransactionError::Storage => ProblemCode::StorageUnavailable,
        RecoveryTransactionError::Validation => ProblemCode::ValidationFailed,
    };
    Box::new(FastiProblem::from_code(
        code,
        CapabilityKey::RestoreWorkspace,
        correlation_id,
    ))
}

fn prepare_recovery_receipt(
    request: &PrepareRecoveryBootstrapRequest,
    error: RecoveryTransactionError,
) -> PortabilityFailureReceipt {
    PortabilityFailureReceipt::try_recovery_bootstrap_prepare(
        request,
        recovery_problem(error, request.correlation_id()),
    )
    .expect("recovery prepare errors are owned by RestoreWorkspace")
}

fn complete_recovery_receipt(
    request: &CompleteRecoveryBootstrapRequest,
    error: RecoveryTransactionError,
) -> PortabilityFailureReceipt {
    PortabilityFailureReceipt::try_recovery_bootstrap_complete(
        request,
        recovery_problem(error, request.correlation_id()),
    )
    .expect("recovery completion errors are owned by RestoreWorkspace")
}

#[derive(Debug)]
struct RecoveryNodeState {
    initialized: i64,
    workspace_id: Option<String>,
    profile_id: Option<String>,
    client_id: Option<String>,
    initialization_digest: Option<String>,
    initialization_expires_at: Option<String>,
    initialization_consumed_at: Option<String>,
    restore_attempt_id: Option<String>,
}

fn load_recovery_node_state(
    transaction: &Transaction<'_>,
    owned_only: bool,
) -> Result<Option<RecoveryNodeState>, RecoveryTransactionError> {
    recovery_sql(
        transaction
            .query_row(
                r#"
                SELECT initialized, workspace_id, profile_id, client_id,
                       initialization_digest, initialization_expires_at,
                       initialization_consumed_at, recovery_restore_attempt_id
                FROM node_state
                WHERE singleton = 1
                  AND (?1 = 0 OR recovery_restore_attempt_id IS NOT NULL)
                "#,
                params![owned_only],
                |row| {
                    Ok(RecoveryNodeState {
                        initialized: row.get(0)?,
                        workspace_id: row.get(1)?,
                        profile_id: row.get(2)?,
                        client_id: row.get(3)?,
                        initialization_digest: row.get(4)?,
                        initialization_expires_at: row.get(5)?,
                        initialization_consumed_at: row.get(6)?,
                        restore_attempt_id: row.get(7)?,
                    })
                },
            )
            .optional(),
    )
}

fn auth_table_counts(
    transaction: &Transaction<'_>,
) -> Result<(i64, i64, i64), RecoveryTransactionError> {
    recovery_sql(transaction.query_row(
        r#"
        SELECT (SELECT COUNT(*) FROM credentials),
               (SELECT COUNT(*) FROM profile_grants),
               (SELECT COUNT(*) FROM grant_scopes)
        "#,
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ))
}

fn profile_belongs_to_workspace(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
) -> Result<bool, RecoveryTransactionError> {
    recovery_sql(transaction.query_row(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM profiles
            WHERE workspace_id = ?1 AND profile_id = ?2
        )
        "#,
        params![workspace_id.to_string(), profile_id.to_string()],
        |row| row.get(0),
    ))
}

fn exact_provisional_auth(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    client_id: ClientId,
    proof_digest: &str,
) -> Result<Option<(String, String, i64)>, RecoveryTransactionError> {
    recovery_sql(
        transaction
            .query_row(
                r#"
                SELECT cr.credential_id, pg.grant_id, cr.epoch
                FROM clients c
                JOIN credentials cr
                  ON cr.workspace_id = c.workspace_id
                 AND cr.client_id = c.client_id
                JOIN profile_grants pg
                  ON pg.workspace_id = c.workspace_id
                 AND pg.client_id = c.client_id
                JOIN grant_scopes gs ON gs.grant_id = pg.grant_id
                WHERE c.workspace_id = ?1
                  AND c.client_id = ?2
                  AND c.status = 'active'
                  AND c.current_credential_epoch = 1
                  AND cr.digest = ?3
                  AND cr.epoch = 1
                  AND cr.status = 'active'
                  AND pg.profile_id = ?4
                  AND pg.status = 'active'
                  AND gs.scope_key = ?5
                  AND (SELECT COUNT(*) FROM credentials) = 1
                  AND (SELECT COUNT(*) FROM profile_grants) = 1
                  AND (SELECT COUNT(*) FROM grant_scopes) = 1
                "#,
                params![
                    workspace_id.to_string(),
                    client_id.to_string(),
                    proof_digest,
                    profile_id.to_string(),
                    scope_storage_key(ScopeKey::ClientEnroll),
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional(),
    )
}

fn insert_recovery_provisional(
    transaction: &Transaction<'_>,
    request: &PrepareRecoveryBootstrapRequest,
    client_id: ClientId,
    proof_digest: &str,
    created_at: &str,
    expires_at: &str,
    replace_pending: bool,
) -> Result<(), RecoveryTransactionError> {
    let credential_id = CredentialId::new_v7();
    let grant_id = ProfileGrantId::new_v7();
    recovery_sql(transaction.execute(
        "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 1, ?3)",
        params![
            client_id.to_string(),
            request.workspace_id().to_string(),
            created_at,
        ],
    ))?;
    recovery_sql(transaction.execute(
        r#"
        INSERT INTO credentials(
            credential_id, workspace_id, client_id, digest, epoch, status, created_at
        ) VALUES (?1, ?2, ?3, ?4, 1, 'active', ?5)
        "#,
        params![
            credential_id.to_string(),
            request.workspace_id().to_string(),
            client_id.to_string(),
            proof_digest,
            created_at,
        ],
    ))?;
    recovery_sql(transaction.execute(
        r#"
        INSERT INTO profile_grants(
            grant_id, workspace_id, profile_id, client_id, status, created_at
        ) VALUES (?1, ?2, ?3, ?4, 'active', ?5)
        "#,
        params![
            grant_id.to_string(),
            request.workspace_id().to_string(),
            request.profile_id().to_string(),
            client_id.to_string(),
            created_at,
        ],
    ))?;
    recovery_sql(transaction.execute(
        "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
        params![
            grant_id.to_string(),
            scope_storage_key(ScopeKey::ClientEnroll),
        ],
    ))?;
    let changed = if replace_pending {
        recovery_sql(transaction.execute(
            r#"
            UPDATE node_state
            SET initialized = 1,
                profile_id = ?1,
                client_id = ?2,
                initialization_digest = ?3,
                initialization_expires_at = ?4,
                initialization_consumed_at = NULL
            WHERE singleton = 1
              AND workspace_id = ?5
              AND recovery_restore_attempt_id = ?6
            "#,
            params![
                request.profile_id().to_string(),
                client_id.to_string(),
                proof_digest,
                expires_at,
                request.workspace_id().to_string(),
                request.restore_attempt_id().to_string(),
            ],
        ))?
    } else {
        recovery_sql(transaction.execute(
            r#"
            INSERT INTO node_state(
                singleton, initialized, workspace_id, profile_id, client_id,
                initialization_digest, initialization_expires_at,
                initialization_consumed_at, created_at,
                recovery_restore_attempt_id
            ) VALUES (1, 1, ?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)
            "#,
            params![
                request.workspace_id().to_string(),
                request.profile_id().to_string(),
                client_id.to_string(),
                proof_digest,
                expires_at,
                created_at,
                request.restore_attempt_id().to_string(),
            ],
        ))?
    };
    if changed != 1 {
        return Err(RecoveryTransactionError::Integrity);
    }
    Ok(())
}

fn sorted_full_admin_scope_keys() -> Vec<&'static str> {
    let mut scopes: Vec<_> = FULL_ADMIN_SCOPES
        .iter()
        .copied()
        .map(scope_storage_key)
        .collect();
    scopes.sort_unstable();
    scopes
}

fn require_one_row(
    changed: usize,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    if changed == 1 {
        Ok(())
    } else {
        Err(problem(
            ProblemCode::IntegrityFailed,
            capability,
            correlation_id,
        ))
    }
}

impl AccessAdministrationPort for SqliteKernel {
    fn ensure_bootstrap_secret(&self) -> ApplicationResult<SecretMaterial> {
        let capability = CapabilityKey::InitializeNode;
        let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
        // Serializes the whole read-validate-recover sequence below: without
        // it, two concurrent callers can each observe the same malformed
        // file, and the loser can delete a secret the winner just published
        // and republish a different one, leaving them disagreeing about the
        // secret's value.
        let _bootstrap_secret_guard = self.lock_bootstrap_secret(capability, correlation_id)?;
        let path = self.data_root().join("bootstrap.secret");
        let unavailable = || {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        };

        // The stored value is always exactly 64 hex characters -- bounded so
        // a corrupted or hostile file can't make this read unbounded.
        let read_existing = || -> std::io::Result<String> {
            let mut contents = String::new();
            std::fs::File::open(&path)?
                .take(128)
                .read_to_string(&mut contents)?;
            Ok(contents)
        };
        let is_valid = |contents: &str| SecretMaterial::try_from_hex(contents.trim()).is_ok();

        let stored_hex = match read_existing() {
            Ok(contents) if is_valid(&contents) => contents,
            // Absent, unreadable, or empty (including a file left
            // zero-length by a prior write that never reached disk before a
            // crash) -- (re)publish through a unique temporary file in the
            // target's directory so concurrent callers don't collide, fsync
            // before publishing so a crash after this point still leaves
            // either a complete file or none, and use no-replace semantics
            // (hard link, not rename, which silently overwrites) so a
            // concurrent legitimate publish is never clobbered. Bounded to
            // two attempts: a hard link can only fail because the
            // destination already exists, and that existing file is either
            // a winning concurrent publish (read and use it) or the exact
            // stale/invalid file that put us in this branch to begin with
            // (remove it and retry once, so this can't loop forever
            // refusing to replace it).
            _ => {
                let parent = path.parent().ok_or_else(unavailable)?;
                let secret = random_secret(capability, correlation_id)?;
                let hex = secret.expose_hex();
                let mut published_hex = None;
                for attempt in 0..2 {
                    let temporary =
                        parent.join(format!("bootstrap.secret.tmp.{correlation_id}.{attempt}"));
                    let published = (|| -> std::io::Result<()> {
                        let mut file = OpenOptions::new()
                            .write(true)
                            .create(true)
                            .truncate(true)
                            .open(&temporary)?;
                        harden_private_regular_file(&temporary).map_err(|_| {
                            std::io::Error::other("hardening the secret file failed")
                        })?;
                        file.write_all(hex.as_bytes())?;
                        file.sync_all()?;
                        drop(file);
                        #[cfg(unix)]
                        {
                            std::fs::hard_link(&temporary, &path)
                        }
                        #[cfg(not(unix))]
                        {
                            std::fs::rename(&temporary, &path)
                        }
                    })();
                    let _ = std::fs::remove_file(&temporary);

                    match published {
                        Ok(()) => {
                            published_hex = Some(hex.clone());
                            break;
                        }
                        Err(_) => match read_existing() {
                            Ok(contents) if is_valid(&contents) => {
                                published_hex = Some(contents);
                                break;
                            }
                            _ => {
                                // Still invalid: a stale leftover, not a
                                // concurrent legitimate publish. Clear it and
                                // let the loop retry the hard link.
                                let _ = std::fs::remove_file(&path);
                            }
                        },
                    }
                }
                published_hex.ok_or_else(unavailable)?
            }
        };

        SecretMaterial::try_from_hex(stored_hex.trim()).map_err(|_| unavailable())
    }

    fn initialize_node(
        &self,
        command: InitializeNodeCommand,
    ) -> ApplicationResult<InitializeNodeOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::InitializeNode;
        let workspace_id = WorkspaceId::new_v7();
        let profile_id = ProfileId::new_v7();
        let client_id = ClientId::new_v7();
        let enrollment_credential_id = CredentialId::new_v7();
        let enrollment_grant_id = ProfileGrantId::new_v7();
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
                r#"
                SELECT EXISTS(SELECT 1 FROM node_state WHERE singleton = 1)
                    OR EXISTS(SELECT 1 FROM workspaces)
                "#,
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
                "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 1, ?3)",
                params![client_id.to_string(), workspace_id.to_string(), created_at_text],
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
                    enrollment_credential_id.to_string(),
                    workspace_id.to_string(),
                    client_id.to_string(),
                    proof_digest,
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
                    enrollment_grant_id.to_string(),
                    workspace_id.to_string(),
                    profile_id.to_string(),
                    client_id.to_string(),
                    created_at_text
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                params![
                    enrollment_grant_id.to_string(),
                    scope_storage_key(ScopeKey::ClientEnroll)
                ],
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
                      AND recovery_restore_attempt_id IS NULL
                    "#,
                    [],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, Option<String>>(4)?,
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
        let (Some(stored_proof_digest), Some(expires_at)) = (stored_proof_digest, expires_at)
        else {
            return Err(Box::new(FastiProblem::bootstrap_closed(correlation_id)));
        };
        let expires_at = DateTime::parse_from_rfc3339(&expires_at)
            .map(|value| value.with_timezone(&Utc))
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        if consumed_at.is_some()
            || expires_at <= created_at
            || !verify_digest(&stored_proof_digest, &presented_proof_digest)
        {
            return Err(Box::new(FastiProblem::bootstrap_closed(correlation_id)));
        }

        let workspace_id = workspace
            .parse::<WorkspaceId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let profile_id = profile
            .parse::<ProfileId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let client_id = client
            .parse::<ClientId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let provisional = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT cr.credential_id, pg.grant_id, cr.epoch
                    FROM credentials cr
                    JOIN clients c
                      ON c.client_id = cr.client_id
                     AND c.workspace_id = cr.workspace_id
                    JOIN profile_grants pg
                      ON pg.client_id = cr.client_id
                     AND pg.workspace_id = cr.workspace_id
                    JOIN profiles p
                      ON p.profile_id = pg.profile_id
                     AND p.workspace_id = cr.workspace_id
                    JOIN grant_scopes gs
                      ON gs.grant_id = pg.grant_id
                     AND gs.scope_key = ?5
                    WHERE cr.digest = ?1
                      AND cr.workspace_id = ?2
                      AND cr.client_id = ?3
                      AND pg.profile_id = ?4
                      AND cr.status = 'active'
                      AND c.status = 'active'
                      AND pg.status = 'active'
                      AND cr.epoch = c.current_credential_epoch
                      AND NOT EXISTS (
                          SELECT 1 FROM grant_scopes extra
                          WHERE extra.grant_id = pg.grant_id
                            AND extra.scope_key <> ?5
                      )
                    "#,
                    params![
                        stored_proof_digest,
                        workspace,
                        client,
                        profile,
                        scope_storage_key(ScopeKey::ClientEnroll)
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                        ))
                    },
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let Some((provisional_credential, grant, epoch)) = provisional else {
            return Err(Box::new(FastiProblem::bootstrap_closed(correlation_id)));
        };
        let provisional_credential_id = provisional_credential
            .parse::<CredentialId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let grant_id = grant
            .parse::<ProfileGrantId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let epoch = u64::try_from(epoch)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let epoch_storage = i64::try_from(epoch)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        let provisional_access = RequestAccessContext::new(
            workspace_id,
            profile_id,
            client_id,
            provisional_credential_id,
            grant_id,
            epoch,
        );
        authorize_transaction(
            &transaction,
            capability,
            &provisional_access,
            correlation_id,
        )?;

        let created_at_text = timestamp(created_at);
        let changed = map_sql(
            transaction.execute(
                r#"
                UPDATE credentials SET status = 'revoked', revoked_at = ?1
                WHERE credential_id = ?2
                  AND workspace_id = ?3
                  AND client_id = ?4
                  AND digest = ?5
                  AND epoch = ?6
                  AND status = 'active'
                "#,
                params![
                    created_at_text,
                    provisional_credential,
                    workspace,
                    client,
                    stored_proof_digest,
                    epoch_storage
                ],
            ),
            capability,
            correlation_id,
        )?;
        require_one_row(changed, capability, correlation_id)?;
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO credentials(
                    credential_id, workspace_id, client_id, digest, epoch, status, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)
                "#,
                params![
                    credential_id.to_string(),
                    workspace,
                    client,
                    credential_digest,
                    epoch_storage,
                    created_at_text
                ],
            ),
            capability,
            correlation_id,
        )?;
        let changed = map_sql(
            transaction.execute(
                "DELETE FROM grant_scopes WHERE grant_id = ?1",
                [grant.as_str()],
            ),
            capability,
            correlation_id,
        )?;
        require_one_row(changed, capability, correlation_id)?;
        for scope in FULL_ADMIN_SCOPES {
            map_sql(
                transaction.execute(
                    "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                    params![grant, scope_storage_key(*scope)],
                ),
                capability,
                correlation_id,
            )?;
        }
        let changed = map_sql(
            transaction.execute(
                r#"
                UPDATE node_state
                SET initialization_consumed_at = ?1,
                    initialization_digest = NULL,
                    initialization_expires_at = NULL
                WHERE singleton = 1
                  AND initialization_consumed_at IS NULL
                  AND initialization_digest = ?2
                  AND initialization_expires_at = ?3
                "#,
                params![created_at_text, stored_proof_digest, timestamp(expires_at)],
            ),
            capability,
            correlation_id,
        )?;
        require_one_row(changed, capability, correlation_id)?;
        map_sql(transaction.commit(), capability, correlation_id)?;

        Ok(EnrollFirstClientOutcome::new(
            RequestAccessContext::new(
                workspace_id,
                profile_id,
                client_id,
                credential_id,
                grant_id,
                epoch,
            ),
            credential,
        ))
    }

    fn authenticate_credential(
        &self,
        query: AuthenticateCredentialQuery,
    ) -> ApplicationResult<RequestAccessContext> {
        let correlation_id = query.correlation_id();
        let capability = query.capability();
        let digest = digest_secret(query.credential());
        let connection = self.lock_connection(capability, correlation_id)?;
        let mut statement = map_sql(
            connection.prepare(
                r#"
                SELECT cr.workspace_id, pg.profile_id, cr.client_id,
                       cr.credential_id, pg.grant_id, cr.epoch
                FROM credentials cr
                JOIN clients c
                  ON c.client_id = cr.client_id
                 AND c.workspace_id = cr.workspace_id
                JOIN profile_grants pg
                  ON pg.client_id = cr.client_id
                 AND pg.workspace_id = cr.workspace_id
                JOIN profiles p
                  ON p.profile_id = pg.profile_id
                 AND p.workspace_id = cr.workspace_id
                WHERE cr.digest = ?1
                  AND cr.status = 'active'
                  AND c.status = 'active'
                  AND pg.status = 'active'
                  AND cr.epoch = c.current_credential_epoch
                ORDER BY pg.grant_id
                LIMIT 2
                "#,
            ),
            capability,
            correlation_id,
        )?;
        let mut rows = map_sql(statement.query([digest]), capability, correlation_id)?;
        let Some(row) = map_sql(rows.next(), capability, correlation_id)? else {
            return Err(Box::new(FastiProblem::authentication_failed(
                capability,
                correlation_id,
            )));
        };
        let values = map_sql(
            (|| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })(),
            capability,
            correlation_id,
        )?;
        if map_sql(rows.next(), capability, correlation_id)?.is_some() {
            return Err(Box::new(FastiProblem::authentication_failed(
                capability,
                correlation_id,
            )));
        }
        drop(rows);
        drop(statement);
        let (workspace, profile, client, credential, grant, epoch) = values;
        let epoch = u64::try_from(epoch)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
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
            epoch,
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
        let new_epoch_storage = i64::try_from(new_epoch)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        let created_at = timestamp(now());

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
                  AND epoch = ?5
                  AND status = 'active'
                "#,
                params![
                    created_at,
                    command.access().credential_id().to_string(),
                    command.access().workspace_id().to_string(),
                    command.access().client_id().to_string(),
                    i64::try_from(command.access().presented_credential_epoch()).map_err(|_| {
                        problem(ProblemCode::IntegrityFailed, capability, correlation_id)
                    })?
                ],
            ),
            capability,
            correlation_id,
        )?;
        require_one_row(changed, capability, correlation_id)?;
        let changed = map_sql(
            transaction.execute(
                r#"
                UPDATE clients SET current_credential_epoch = ?1
                WHERE client_id = ?2
                  AND workspace_id = ?3
                  AND status = 'active'
                  AND current_credential_epoch = ?4
                "#,
                params![
                    new_epoch_storage,
                    command.access().client_id().to_string(),
                    command.access().workspace_id().to_string(),
                    i64::try_from(command.access().presented_credential_epoch()).map_err(|_| {
                        problem(ProblemCode::IntegrityFailed, capability, correlation_id)
                    })?
                ],
            ),
            capability,
            correlation_id,
        )?;
        require_one_row(changed, capability, correlation_id)?;
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
                    new_epoch_storage,
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

impl SqliteKernel {
    /// Private transaction core for recovery bootstrap preparation.
    ///
    /// The private recovery coordinator proves the exact COMPLETE marker
    /// before and after opening this kernel. No raw public
    /// [`fasti_application::RecoveryBootstrapPort`] implementation exposes
    /// this transaction core.
    pub(crate) fn prepare_recovery_bootstrap_after_verified_activation(
        &self,
        request: PrepareRecoveryBootstrapRequest,
    ) -> PortabilityResult<PrepareRecoveryBootstrapOutcome> {
        let result = (|| {
            let capability = CapabilityKey::RestoreWorkspace;
            let proof = random_secret(capability, request.correlation_id())
                .map_err(|_| RecoveryTransactionError::Storage)?;
            let proof_digest = digest_secret(&proof);
            let client_id = ClientId::new_v7();

            let mut connection = self
                .inner
                .connection
                .lock()
                .map_err(|_| RecoveryTransactionError::Storage)?;
            let transaction =
                recovery_sql(connection.transaction_with_behavior(TransactionBehavior::Immediate))?;
            let created_at = now();
            let created_at_text = timestamp(created_at);
            let expires_at =
                timestamp(created_at + Duration::minutes(INITIALIZATION_LIFETIME_MINUTES));
            if !profile_belongs_to_workspace(
                &transaction,
                request.workspace_id(),
                request.profile_id(),
            )? {
                return Err(RecoveryTransactionError::Validation);
            }

            let state = load_recovery_node_state(&transaction, false)?;
            if state.is_none() {
                if request.replace_pending() {
                    return Err(RecoveryTransactionError::Validation);
                }
                if auth_table_counts(&transaction)? != (0, 0, 0) {
                    return Err(RecoveryTransactionError::Integrity);
                }
            } else {
                let state = state.ok_or(RecoveryTransactionError::Integrity)?;
                let expected_restore_attempt_id = request.restore_attempt_id().to_string();
                let expected_workspace_id = request.workspace_id().to_string();
                let expected_profile_id = request.profile_id().to_string();
                if state.initialized != 1
                    || state.restore_attempt_id.as_deref()
                        != Some(expected_restore_attempt_id.as_str())
                    || state.workspace_id.as_deref() != Some(expected_workspace_id.as_str())
                    || state.profile_id.as_deref() != Some(expected_profile_id.as_str())
                    || state.initialization_consumed_at.is_some()
                {
                    return Err(RecoveryTransactionError::Validation);
                }
                let prior_client = state
                    .client_id
                    .as_deref()
                    .ok_or(RecoveryTransactionError::Integrity)?
                    .parse::<ClientId>()
                    .map_err(|_| RecoveryTransactionError::Integrity)?;
                let prior_digest = state
                    .initialization_digest
                    .as_deref()
                    .ok_or(RecoveryTransactionError::Integrity)?;
                if state.initialization_expires_at.is_none() {
                    return Err(RecoveryTransactionError::Integrity);
                }
                let (prior_credential, prior_grant, _) = exact_provisional_auth(
                    &transaction,
                    request.workspace_id(),
                    request.profile_id(),
                    prior_client,
                    prior_digest,
                )?
                .ok_or(RecoveryTransactionError::Integrity)?;
                if !request.replace_pending() {
                    return Err(RecoveryTransactionError::Pending);
                }

                for (sql, value) in [
                    (
                        "DELETE FROM grant_scopes WHERE grant_id = ?1",
                        prior_grant.as_str(),
                    ),
                    (
                        "DELETE FROM profile_grants WHERE grant_id = ?1",
                        prior_grant.as_str(),
                    ),
                    (
                        "DELETE FROM credentials WHERE credential_id = ?1",
                        prior_credential.as_str(),
                    ),
                    (
                        "DELETE FROM clients WHERE client_id = ?1",
                        state
                            .client_id
                            .as_deref()
                            .ok_or(RecoveryTransactionError::Integrity)?,
                    ),
                ] {
                    if recovery_sql(transaction.execute(sql, [value]))? != 1 {
                        return Err(RecoveryTransactionError::Integrity);
                    }
                }
            }

            insert_recovery_provisional(
                &transaction,
                &request,
                client_id,
                &proof_digest,
                &created_at_text,
                &expires_at,
                request.replace_pending(),
            )?;
            recovery_sql(transaction.commit())?;

            Ok(PrepareRecoveryBootstrapOutcome::new(
                request.restore_attempt_id(),
                request.workspace_id(),
                request.profile_id(),
                client_id,
                proof,
            ))
        })();

        result.map_err(|error| prepare_recovery_receipt(&request, error))
    }

    /// Private completion core paired with
    /// [`Self::prepare_recovery_bootstrap_after_verified_activation`].
    pub(crate) fn complete_recovery_bootstrap_transaction(
        &self,
        request: CompleteRecoveryBootstrapRequest,
    ) -> PortabilityResult<CompleteRecoveryBootstrapOutcome> {
        let result = (|| {
            let proof_digest = digest_secret(request.initialization_proof());
            let final_digest = digest_secret(request.credential());
            let replacement_credential_id = CredentialId::new_v7();

            let mut connection = self
                .inner
                .connection
                .lock()
                .map_err(|_| RecoveryTransactionError::Storage)?;
            let transaction =
                recovery_sql(connection.transaction_with_behavior(TransactionBehavior::Immediate))?;
            let completed_at = now();
            let completed_at_text = timestamp(completed_at);
            let state = load_recovery_node_state(&transaction, true)?
                .ok_or(RecoveryTransactionError::BootstrapClosed)?;

            if state.restore_attempt_id.as_deref()
                != Some(request.restore_attempt_id().to_string().as_str())
                || state.workspace_id.as_deref()
                    != Some(request.workspace_id().to_string().as_str())
                || state.profile_id.as_deref() != Some(request.profile_id().to_string().as_str())
            {
                return Err(RecoveryTransactionError::Validation);
            }
            if state.client_id.as_deref() != Some(request.client_id().to_string().as_str()) {
                return Err(RecoveryTransactionError::BootstrapClosed);
            }
            if state.initialized != 1
                || !profile_belongs_to_workspace(
                    &transaction,
                    request.workspace_id(),
                    request.profile_id(),
                )?
            {
                return Err(RecoveryTransactionError::Integrity);
            }

            let (credential_id, grant_id, epoch) = if state.initialization_consumed_at.is_none() {
                let stored_proof_digest = state
                    .initialization_digest
                    .as_deref()
                    .ok_or(RecoveryTransactionError::Integrity)?;
                let expires_at = state
                    .initialization_expires_at
                    .as_deref()
                    .ok_or(RecoveryTransactionError::Integrity)?;
                let expires_at = DateTime::parse_from_rfc3339(expires_at)
                    .map(|value| value.with_timezone(&Utc))
                    .map_err(|_| RecoveryTransactionError::Integrity)?;
                let (provisional_credential, grant, epoch) = exact_provisional_auth(
                    &transaction,
                    request.workspace_id(),
                    request.profile_id(),
                    request.client_id(),
                    stored_proof_digest,
                )?
                .ok_or(RecoveryTransactionError::Integrity)?;
                if expires_at <= completed_at
                    || !verify_digest(stored_proof_digest, &proof_digest)
                    || verify_digest(stored_proof_digest, &final_digest)
                {
                    return Err(RecoveryTransactionError::BootstrapClosed);
                }

                if recovery_sql(transaction.execute(
                    r#"
                        UPDATE credentials SET status = 'revoked', revoked_at = ?1
                        WHERE credential_id = ?2
                          AND digest = ?3
                          AND status = 'active'
                        "#,
                    params![
                        completed_at_text,
                        provisional_credential,
                        stored_proof_digest
                    ],
                ))? != 1
                {
                    return Err(RecoveryTransactionError::Integrity);
                }
                recovery_sql(transaction.execute(
                        r#"
                        INSERT INTO credentials(
                            credential_id, workspace_id, client_id, digest, epoch, status, created_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, 'active', ?6)
                        "#,
                        params![
                            replacement_credential_id.to_string(),
                            request.workspace_id().to_string(),
                            request.client_id().to_string(),
                            final_digest,
                            epoch,
                            completed_at_text,
                        ],
                    ))?;
                if recovery_sql(transaction.execute(
                    "DELETE FROM grant_scopes WHERE grant_id = ?1",
                    [grant.as_str()],
                ))? != 1
                {
                    return Err(RecoveryTransactionError::Integrity);
                }
                for scope in FULL_ADMIN_SCOPES {
                    recovery_sql(transaction.execute(
                        "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                        params![grant, scope_storage_key(*scope)],
                    ))?;
                }
                if recovery_sql(transaction.execute(
                    r#"
                        UPDATE node_state
                        SET initialization_consumed_at = ?1,
                            initialization_digest = NULL,
                            initialization_expires_at = NULL
                        WHERE singleton = 1
                          AND recovery_restore_attempt_id = ?2
                          AND workspace_id = ?3
                          AND profile_id = ?4
                          AND client_id = ?5
                          AND initialization_consumed_at IS NULL
                          AND initialization_digest = ?6
                        "#,
                    params![
                        completed_at_text,
                        request.restore_attempt_id().to_string(),
                        request.workspace_id().to_string(),
                        request.profile_id().to_string(),
                        request.client_id().to_string(),
                        stored_proof_digest,
                    ],
                ))? != 1
                {
                    return Err(RecoveryTransactionError::Integrity);
                }
                (replacement_credential_id.to_string(), grant, epoch)
            } else {
                if state.initialization_digest.is_some()
                    || state.initialization_expires_at.is_some()
                {
                    return Err(RecoveryTransactionError::Integrity);
                }
                let mut credentials = recovery_sql(transaction.prepare(
                    r#"
                            SELECT credential_id, digest, epoch, status
                            FROM credentials
                            WHERE workspace_id = ?1 AND client_id = ?2
                            ORDER BY credential_id
                            "#,
                ))?;
                let rows = recovery_sql(credentials.query_map(
                    params![
                        request.workspace_id().to_string(),
                        request.client_id().to_string(),
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                ))?;
                let rows = rows
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(|_| RecoveryTransactionError::Storage)?;
                let proof = rows
                    .iter()
                    .find(|row| row.3 == "revoked" && verify_digest(&row.1, &proof_digest))
                    .ok_or(RecoveryTransactionError::BootstrapClosed)?;
                let mut completion_credentials = rows
                    .iter()
                    .filter(|row| row.0 != proof.0 && row.2 == proof.2);
                let completion_credential = completion_credentials
                    .next()
                    .ok_or(RecoveryTransactionError::Integrity)?;
                if completion_credentials.next().is_some() {
                    return Err(RecoveryTransactionError::Integrity);
                }
                if !verify_digest(&completion_credential.1, &final_digest)
                    || completion_credential.3 != "active"
                {
                    return Err(RecoveryTransactionError::BootstrapClosed);
                }
                if auth_table_counts(&transaction)?
                    != (
                        2,
                        1,
                        i64::try_from(FULL_ADMIN_SCOPES.len()).unwrap_or(i64::MAX),
                    )
                    || rows.len() != 2
                    || rows.iter().any(|row| row.2 != 1)
                {
                    return Err(RecoveryTransactionError::Integrity);
                }
                let (grant,): (String,) = recovery_sql(transaction.query_row(
                    r#"
                        SELECT grant_id FROM profile_grants
                        WHERE workspace_id = ?1
                          AND profile_id = ?2
                          AND client_id = ?3
                          AND status = 'active'
                        "#,
                    params![
                        request.workspace_id().to_string(),
                        request.profile_id().to_string(),
                        request.client_id().to_string(),
                    ],
                    |row| Ok((row.get(0)?,)),
                ))?;
                let mut scope_statement = recovery_sql(transaction.prepare(
                    "SELECT scope_key FROM grant_scopes WHERE grant_id = ?1 ORDER BY scope_key",
                ))?;
                let scopes = recovery_sql(
                    scope_statement.query_map([grant.as_str()], |row| row.get::<_, String>(0)),
                )?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(|_| RecoveryTransactionError::Storage)?;
                if scopes != sorted_full_admin_scope_keys() {
                    return Err(RecoveryTransactionError::Integrity);
                }
                (
                    completion_credential.0.clone(),
                    grant,
                    completion_credential.2,
                )
            };

            let credential_id = credential_id
                .parse::<CredentialId>()
                .map_err(|_| RecoveryTransactionError::Integrity)?;
            let grant_id = grant_id
                .parse::<ProfileGrantId>()
                .map_err(|_| RecoveryTransactionError::Integrity)?;
            let epoch = u64::try_from(epoch).map_err(|_| RecoveryTransactionError::Integrity)?;
            recovery_sql(transaction.commit())?;

            Ok(CompleteRecoveryBootstrapOutcome::new(
                request.restore_attempt_id(),
                RequestAccessContext::new(
                    request.workspace_id(),
                    request.profile_id(),
                    request.client_id(),
                    credential_id,
                    grant_id,
                    epoch,
                ),
            ))
        })();

        result.map_err(|error| complete_recovery_receipt(&request, error))
    }
}

// Compile-time anchors keep the private transaction seams type-checked in
// production builds without making either phase dispatchable before the
// stopped-node filesystem-marker verifier exists.
const _: fn(
    &SqliteKernel,
    PrepareRecoveryBootstrapRequest,
) -> PortabilityResult<PrepareRecoveryBootstrapOutcome> =
    SqliteKernel::prepare_recovery_bootstrap_after_verified_activation;
const _: fn(
    &SqliteKernel,
    CompleteRecoveryBootstrapRequest,
) -> PortabilityResult<CompleteRecoveryBootstrapOutcome> =
    SqliteKernel::complete_recovery_bootstrap_transaction;

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_application::SecretMaterial;
    use fasti_domain::{RequestCorrelationId, RestoreAttemptId};
    use rusqlite::types::Value;
    use std::sync::{Arc, Barrier};
    use tempfile::TempDir;

    struct TestNode {
        _root: TempDir,
        kernel: SqliteKernel,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        client_id: ClientId,
        grant_id: ProfileGrantId,
        initialization_proof_hex: String,
        credential_hex: String,
    }

    impl TestNode {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("temporary data root");
            let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
            let initialized = kernel
                .initialize_node(InitializeNodeCommand::new(RequestCorrelationId::new_v7()))
                .expect("initialize node");
            let initialization_proof_hex = initialized.initialization_proof().expose_hex();
            let proof = SecretMaterial::try_from_hex(&initialization_proof_hex)
                .expect("copy one-time proof for enrollment");
            let enrolled = kernel
                .enroll_first_client(EnrollFirstClientCommand::new(
                    RequestCorrelationId::new_v7(),
                    proof,
                ))
                .expect("enroll first client");

            Self {
                _root: root,
                kernel,
                workspace_id: enrolled.access().workspace_id(),
                profile_id: enrolled.access().profile_id(),
                client_id: enrolled.access().client_id(),
                grant_id: enrolled.access().grant_id(),
                initialization_proof_hex,
                credential_hex: enrolled.credential().expose_hex(),
            }
        }

        fn authenticate(&self) -> ApplicationResult<RequestAccessContext> {
            let credential = SecretMaterial::try_from_hex(&self.credential_hex)
                .expect("copy credential for authentication");
            self.kernel
                .authenticate_credential(AuthenticateCredentialQuery::new(
                    RequestCorrelationId::new_v7(),
                    CapabilityKey::DiscoverCapabilities,
                    credential,
                ))
        }

        fn insert_profile_and_grant(
            &self,
            workspace_id: WorkspaceId,
            profile_id: ProfileId,
        ) -> ProfileGrantId {
            let grant_id = ProfileGrantId::new_v7();
            let created_at = timestamp(now());
            let connection = self
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                    params![
                        profile_id.to_string(),
                        workspace_id.to_string(),
                        created_at
                    ],
                )
                .expect("insert profile");
            connection
                .execute(
                    r#"
                    INSERT INTO profile_grants(
                        grant_id, workspace_id, profile_id, client_id, status, created_at
                    ) VALUES (?1, ?2, ?3, ?4, 'active', ?5)
                    "#,
                    params![
                        grant_id.to_string(),
                        workspace_id.to_string(),
                        profile_id.to_string(),
                        self.client_id.to_string(),
                        created_at
                    ],
                )
                .expect("insert profile grant");
            connection
                .execute(
                    "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                    params![
                        grant_id.to_string(),
                        scope_storage_key(ScopeKey::CapabilityRead)
                    ],
                )
                .expect("insert capability scope");
            grant_id
        }
    }

    struct RecoveryNode {
        root: TempDir,
        kernel: SqliteKernel,
        restore_attempt_id: RestoreAttemptId,
        workspace_id: WorkspaceId,
        profile_id: ProfileId,
        imported_client_id: ClientId,
    }

    impl RecoveryNode {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("temporary recovery data root");
            let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
            let restore_attempt_id = RestoreAttemptId::new_v7();
            let workspace_id = WorkspaceId::new_v7();
            let profile_id = ProfileId::new_v7();
            let imported_client_id = ClientId::new_v7();
            let created_at = timestamp(now());
            {
                let connection = kernel.inner.connection.lock().expect("SQLite connection");
                connection
                    .execute(
                        "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                        params![workspace_id.to_string(), created_at],
                    )
                    .expect("restored workspace");
                connection
                    .execute(
                        "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                        params![profile_id.to_string(), workspace_id.to_string(), created_at],
                    )
                    .expect("restored profile");
                connection
                    .execute(
                        "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 1, ?3)",
                        params![
                            imported_client_id.to_string(),
                            workspace_id.to_string(),
                            created_at,
                        ],
                    )
                    .expect("non-secret imported client provenance");
            }
            Self {
                root,
                kernel,
                restore_attempt_id,
                workspace_id,
                profile_id,
                imported_client_id,
            }
        }

        fn prepare(&self, replace_pending: bool) -> PrepareRecoveryBootstrapOutcome {
            self.kernel
                .prepare_recovery_bootstrap_after_verified_activation(
                    PrepareRecoveryBootstrapRequest::new(
                        self.restore_attempt_id,
                        RequestCorrelationId::new_v7(),
                        self.workspace_id,
                        self.profile_id,
                        replace_pending,
                    ),
                )
                .expect("prepare recovery bootstrap")
        }

        fn complete_request(
            &self,
            client_id: ClientId,
            proof_hex: &str,
            credential_hex: &str,
        ) -> CompleteRecoveryBootstrapRequest {
            CompleteRecoveryBootstrapRequest::new(
                self.restore_attempt_id,
                RequestCorrelationId::new_v7(),
                self.workspace_id,
                self.profile_id,
                client_id,
                SecretMaterial::try_from_hex(proof_hex).expect("copy recovery proof"),
                SecretMaterial::try_from_hex(credential_hex).expect("copy final credential"),
            )
        }

        fn snapshot(&self) -> RecoveryDatabaseSnapshot {
            let connection = self
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            RecoveryDatabaseSnapshot::read(&connection)
        }
    }

    #[derive(Debug, PartialEq)]
    struct RecoveryDatabaseSnapshot {
        node_state: Vec<Vec<Value>>,
        clients: Vec<Vec<Value>>,
        credentials: Vec<Vec<Value>>,
        profile_grants: Vec<Vec<Value>>,
        grant_scopes: Vec<Vec<Value>>,
    }

    impl RecoveryDatabaseSnapshot {
        fn read(connection: &rusqlite::Connection) -> Self {
            Self {
                node_state: query_values(connection, "SELECT * FROM node_state ORDER BY singleton"),
                clients: query_values(connection, "SELECT * FROM clients ORDER BY client_id"),
                credentials: query_values(
                    connection,
                    "SELECT * FROM credentials ORDER BY credential_id",
                ),
                profile_grants: query_values(
                    connection,
                    "SELECT * FROM profile_grants ORDER BY grant_id",
                ),
                grant_scopes: query_values(
                    connection,
                    "SELECT * FROM grant_scopes ORDER BY grant_id, scope_key",
                ),
            }
        }
    }

    fn query_values(connection: &rusqlite::Connection, sql: &str) -> Vec<Vec<Value>> {
        let mut statement = connection.prepare(sql).expect("prepare snapshot query");
        let column_count = statement.column_count();
        statement
            .query_map([], |row| {
                (0..column_count)
                    .map(|index| row.get::<_, Value>(index))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .expect("query snapshot")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect snapshot")
    }

    fn fixed_secret(byte: u8) -> String {
        SecretMaterial::from_bytes([byte; 32]).expose_hex()
    }

    #[test]
    fn clean_restored_fixture_starts_without_node_state_or_authorization() {
        let node = RecoveryNode::new();
        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let (node_states, credentials, grants, scopes): (i64, i64, i64, i64) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM node_state),
                       (SELECT COUNT(*) FROM credentials),
                       (SELECT COUNT(*) FROM profile_grants),
                       (SELECT COUNT(*) FROM grant_scopes)
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("restored-state counts");
        assert_eq!((node_states, credentials, grants, scopes), (0, 0, 0, 0));
    }

    #[test]
    fn recovery_prepare_rejects_wrong_workspace_and_cross_profile_without_mutation() {
        let node = RecoveryNode::new();
        let before = node.snapshot();
        let wrong_workspace = match node
            .kernel
            .prepare_recovery_bootstrap_after_verified_activation(
                PrepareRecoveryBootstrapRequest::new(
                    node.restore_attempt_id,
                    RequestCorrelationId::new_v7(),
                    WorkspaceId::new_v7(),
                    node.profile_id,
                    false,
                ),
            ) {
            Ok(_) => panic!("wrong workspace must fail"),
            Err(error) => error,
        };
        assert_eq!(
            wrong_workspace.problem().code(),
            ProblemCode::ValidationFailed
        );
        assert_eq!(node.snapshot(), before);

        let foreign_workspace = WorkspaceId::new_v7();
        let foreign_profile = ProfileId::new_v7();
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            let created_at = timestamp(now());
            connection
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![foreign_workspace.to_string(), created_at],
                )
                .expect("foreign workspace");
            connection
                .execute(
                    "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                    params![
                        foreign_profile.to_string(),
                        foreign_workspace.to_string(),
                        created_at,
                    ],
                )
                .expect("foreign profile");
        }
        let before_cross_profile = node.snapshot();
        let cross_profile = match node
            .kernel
            .prepare_recovery_bootstrap_after_verified_activation(
                PrepareRecoveryBootstrapRequest::new(
                    node.restore_attempt_id,
                    RequestCorrelationId::new_v7(),
                    node.workspace_id,
                    foreign_profile,
                    false,
                ),
            ) {
            Ok(_) => panic!("cross-workspace profile must fail"),
            Err(error) => error,
        };
        assert_eq!(
            cross_profile.problem().code(),
            ProblemCode::ValidationFailed
        );
        assert_eq!(node.snapshot(), before_cross_profile);
    }

    #[test]
    fn recovery_prepare_rejects_imported_authorization_without_reuse_or_mutation() {
        let node = RecoveryNode::new();
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            let imported_digest = digest_secret(&SecretMaterial::from_bytes([61; 32]));
            connection
                .execute(
                    r#"
                    INSERT INTO credentials(
                        credential_id, workspace_id, client_id, digest, epoch,
                        status, created_at
                    ) VALUES (?1, ?2, ?3, ?4, 1, 'active', ?5)
                    "#,
                    params![
                        CredentialId::new_v7().to_string(),
                        node.workspace_id.to_string(),
                        node.imported_client_id.to_string(),
                        imported_digest,
                        timestamp(now()),
                    ],
                )
                .expect("seed imported authorization that restore must reject");
        }
        let before = node.snapshot();
        let error = match node
            .kernel
            .prepare_recovery_bootstrap_after_verified_activation(
                PrepareRecoveryBootstrapRequest::new(
                    node.restore_attempt_id,
                    RequestCorrelationId::new_v7(),
                    node.workspace_id,
                    node.profile_id,
                    false,
                ),
            ) {
            Ok(_) => panic!("imported authorization must fail closed"),
            Err(error) => error,
        };
        assert_eq!(error.problem().code(), ProblemCode::IntegrityFailed);
        assert_eq!(node.snapshot(), before);
    }

    #[test]
    fn recovery_prepare_composes_only_one_fresh_provisional_authority() {
        let node = RecoveryNode::new();
        let prepared = node.prepare(false);
        let proof_hex = prepared.initialization_proof().expose_hex();
        assert_ne!(prepared.client_id(), node.imported_client_id);

        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let counts: (i64, i64, i64) = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM credentials),
                       (SELECT COUNT(*) FROM profile_grants),
                       (SELECT COUNT(*) FROM grant_scopes)
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("auth table counts");
        assert_eq!(counts, (1, 1, 1));
        let (client, digest, scope, attempt): (String, String, String, String) = connection
            .query_row(
                r#"
                SELECT ns.client_id, cr.digest, gs.scope_key,
                       ns.recovery_restore_attempt_id
                FROM node_state ns
                JOIN credentials cr ON cr.client_id = ns.client_id
                JOIN profile_grants pg ON pg.client_id = ns.client_id
                JOIN grant_scopes gs ON gs.grant_id = pg.grant_id
                WHERE ns.singleton = 1
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("prepared recovery composition");
        assert_eq!(client, prepared.client_id().to_string());
        assert_eq!(attempt, node.restore_attempt_id.to_string());
        assert_eq!(scope, scope_storage_key(ScopeKey::ClientEnroll));
        assert_ne!(digest, proof_hex);
        assert!(verify_digest(
            &digest,
            &digest_secret(&SecretMaterial::try_from_hex(&proof_hex).expect("copy prepared proof"))
        ));
        let imported_auth: i64 = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM credentials WHERE client_id = ?1)
                     + (SELECT COUNT(*) FROM profile_grants WHERE client_id = ?1)
                "#,
                [node.imported_client_id.to_string()],
                |row| row.get(0),
            )
            .expect("imported authorization count");
        assert_eq!(imported_auth, 0);
    }

    #[test]
    fn recovery_proof_cannot_use_first_client_enrollment() {
        let node = RecoveryNode::new();
        let prepared = node.prepare(false);
        let proof_hex = prepared.initialization_proof().expose_hex();
        let before = node.snapshot();

        let error = match node
            .kernel
            .enroll_first_client(EnrollFirstClientCommand::new(
                RequestCorrelationId::new_v7(),
                SecretMaterial::try_from_hex(&proof_hex).expect("copy recovery proof"),
            )) {
            Ok(_) => panic!("recovery proof must not use initial enrollment"),
            Err(error) => error,
        };

        assert_eq!(error.code(), ProblemCode::BootstrapClosed);
        assert_eq!(node.snapshot(), before);
        node.kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &proof_hex,
                &fixed_secret(62),
            ))
            .expect("owned recovery completion remains available");
    }

    #[test]
    fn explicit_replacement_removes_only_the_unconsumed_recovery_provisional() {
        let node = RecoveryNode::new();
        let first = node.prepare(false);
        let first_client = first.client_id();
        let first_proof = first.initialization_proof().expose_hex();
        let pending = node.snapshot();
        let pending_error = match node
            .kernel
            .prepare_recovery_bootstrap_after_verified_activation(
                PrepareRecoveryBootstrapRequest::new(
                    node.restore_attempt_id,
                    RequestCorrelationId::new_v7(),
                    node.workspace_id,
                    node.profile_id,
                    false,
                ),
            ) {
            Ok(_) => panic!("implicit pending replacement must fail"),
            Err(error) => error,
        };
        assert_eq!(
            pending_error.problem().code(),
            ProblemCode::RecoveryBootstrapPending
        );
        assert_eq!(node.snapshot(), pending);

        let replacement = node.prepare(true);
        let replacement_proof = replacement.initialization_proof().expose_hex();
        assert_ne!(replacement.client_id(), first_client);
        assert_ne!(replacement_proof, first_proof);

        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let removed: i64 = connection
            .query_row(
                r#"
                SELECT (SELECT COUNT(*) FROM clients WHERE client_id = ?1)
                     + (SELECT COUNT(*) FROM credentials WHERE client_id = ?1)
                     + (SELECT COUNT(*) FROM profile_grants WHERE client_id = ?1)
                "#,
                [first_client.to_string()],
                |row| row.get(0),
            )
            .expect("removed provisional count");
        assert_eq!(removed, 0);
        drop(connection);

        let old_completion = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                first_client,
                &first_proof,
                &fixed_secret(91),
            ))
            .expect_err("replaced proof must stay closed");
        assert_eq!(
            old_completion.problem().code(),
            ProblemCode::BootstrapClosed
        );
    }

    #[test]
    fn wrong_and_expired_recovery_pairs_are_closed_without_mutation() {
        let node = RecoveryNode::new();
        let prepared = node.prepare(false);
        let proof_hex = prepared.initialization_proof().expose_hex();
        let before_wrong = node.snapshot();
        let wrong = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &fixed_secret(71),
                &fixed_secret(72),
            ))
            .expect_err("wrong proof must fail");
        assert_eq!(wrong.problem().code(), ProblemCode::BootstrapClosed);
        assert_eq!(node.snapshot(), before_wrong);

        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "UPDATE node_state SET initialization_expires_at = '2000-01-01T00:00:00Z' WHERE singleton = 1",
                    [],
                )
                .expect("expire recovery proof");
        }
        let before_expired = node.snapshot();
        let expired = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &proof_hex,
                &fixed_secret(73),
            ))
            .expect_err("expired proof must fail");
        assert_eq!(expired.problem().code(), ProblemCode::BootstrapClosed);
        assert_eq!(node.snapshot(), before_expired);
    }

    #[test]
    fn recovery_completion_rejects_wrong_attempt_and_profile_without_mutation() {
        let node = RecoveryNode::new();
        let prepared = node.prepare(false);
        let proof_hex = prepared.initialization_proof().expose_hex();
        let before_wrong_attempt = node.snapshot();
        let wrong_attempt = node
            .kernel
            .complete_recovery_bootstrap_transaction(CompleteRecoveryBootstrapRequest::new(
                RestoreAttemptId::new_v7(),
                RequestCorrelationId::new_v7(),
                node.workspace_id,
                node.profile_id,
                prepared.client_id(),
                SecretMaterial::try_from_hex(&proof_hex).expect("copy proof"),
                SecretMaterial::try_from_hex(&fixed_secret(74)).expect("copy credential"),
            ))
            .expect_err("wrong restore attempt must fail");
        assert_eq!(
            wrong_attempt.problem().code(),
            ProblemCode::ValidationFailed
        );
        assert_eq!(node.snapshot(), before_wrong_attempt);

        let other_profile = ProfileId::new_v7();
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                    params![
                        other_profile.to_string(),
                        node.workspace_id.to_string(),
                        timestamp(now()),
                    ],
                )
                .expect("second restored profile");
        }
        let before_cross_profile = node.snapshot();
        let cross_profile = node
            .kernel
            .complete_recovery_bootstrap_transaction(CompleteRecoveryBootstrapRequest::new(
                node.restore_attempt_id,
                RequestCorrelationId::new_v7(),
                node.workspace_id,
                other_profile,
                prepared.client_id(),
                SecretMaterial::try_from_hex(&proof_hex).expect("copy proof"),
                SecretMaterial::try_from_hex(&fixed_secret(75)).expect("copy credential"),
            ))
            .expect_err("cross-profile completion must fail");
        assert_eq!(
            cross_profile.problem().code(),
            ProblemCode::ValidationFailed
        );
        assert_eq!(node.snapshot(), before_cross_profile);
    }

    #[test]
    fn caller_owned_pair_retries_before_and_after_commit_without_secret_reissue() {
        let node = RecoveryNode::new();
        let prepared = node.prepare(false);
        let proof_hex = prepared.initialization_proof().expose_hex();
        let credential_hex = fixed_secret(81);
        let final_digest = digest_secret(
            &SecretMaterial::try_from_hex(&credential_hex).expect("copy final credential"),
        );
        let pending = node.snapshot();
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute_batch(&format!(
                    r#"
                    CREATE TEMP TRIGGER fail_recovery_completion
                    BEFORE INSERT ON credentials
                    WHEN NEW.digest = '{final_digest}'
                    BEGIN
                        SELECT RAISE(ABORT, 'simulated pre-commit failure');
                    END;
                    "#
                ))
                .expect("install pre-commit failure trigger");
        }
        let pre_commit = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &proof_hex,
                &credential_hex,
            ))
            .expect_err("simulated pre-commit failure");
        assert_eq!(pre_commit.problem().code(), ProblemCode::StorageUnavailable);
        assert_eq!(node.snapshot(), pending);
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute_batch("DROP TRIGGER fail_recovery_completion;")
                .expect("remove pre-commit failure trigger");
        }

        let completed = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &proof_hex,
                &credential_hex,
            ))
            .expect("retry same caller-owned pair");
        let committed = node.snapshot();
        let replayed = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &proof_hex,
                &credential_hex,
            ))
            .expect("post-commit retry is idempotent");
        assert_eq!(replayed, completed);
        assert_eq!(node.snapshot(), committed);

        let different_credential = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &proof_hex,
                &fixed_secret(82),
            ))
            .expect_err("different post-commit credential must fail closed");
        assert_eq!(
            different_credential.problem().code(),
            ProblemCode::BootstrapClosed
        );
        assert_eq!(node.snapshot(), committed);
        assert_eq!(completed.access().client_id(), prepared.client_id());
        assert_ne!(completed.access().client_id(), node.imported_client_id);

        let replace_consumed = match node
            .kernel
            .prepare_recovery_bootstrap_after_verified_activation(
                PrepareRecoveryBootstrapRequest::new(
                    node.restore_attempt_id,
                    RequestCorrelationId::new_v7(),
                    node.workspace_id,
                    node.profile_id,
                    true,
                ),
            ) {
            Ok(_) => panic!("consumed recovery state cannot be replaced"),
            Err(error) => error,
        };
        assert_eq!(
            replace_consumed.problem().code(),
            ProblemCode::ValidationFailed
        );
        assert_eq!(node.snapshot(), committed);

        let authenticated = node
            .kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                RequestCorrelationId::new_v7(),
                CapabilityKey::DiscoverCapabilities,
                SecretMaterial::try_from_hex(&credential_hex).expect("authenticate final secret"),
            ))
            .expect("final caller-owned credential authenticates");
        assert_eq!(&authenticated, completed.access());
        assert_plaintext_secrets_absent(&node, &[&proof_hex, &credential_hex]);
    }

    #[test]
    fn recovery_completion_replay_closes_after_credential_rotation() {
        let node = RecoveryNode::new();
        let prepared = node.prepare(false);
        let proof_hex = prepared.initialization_proof().expose_hex();
        let credential_hex = fixed_secret(84);
        let completed = node
            .kernel
            .complete_recovery_bootstrap_transaction(node.complete_request(
                prepared.client_id(),
                &proof_hex,
                &credential_hex,
            ))
            .expect("complete recovery");
        let rotated = node
            .kernel
            .rotate_credential(RotateCredentialCommand::new(
                RequestCorrelationId::new_v7(),
                *completed.access(),
            ))
            .expect("rotate recovered credential");
        let rotated_hex = rotated.credential().expose_hex();
        let after_rotation = node.snapshot();

        for candidate in [&credential_hex, &rotated_hex] {
            let error = node
                .kernel
                .complete_recovery_bootstrap_transaction(node.complete_request(
                    prepared.client_id(),
                    &proof_hex,
                    candidate,
                ))
                .expect_err("rotated recovery pair must not replay");
            assert_eq!(error.problem().code(), ProblemCode::BootstrapClosed);
            assert_eq!(node.snapshot(), after_rotation);
        }
    }

    #[test]
    fn concurrent_recovery_completion_commits_once_and_returns_one_identity() {
        let node = RecoveryNode::new();
        let prepared = node.prepare(false);
        let client_id = prepared.client_id();
        let proof_hex = prepared.initialization_proof().expose_hex();
        let credential_hex = fixed_secret(83);
        let barrier = Arc::new(Barrier::new(3));
        let mut threads = Vec::new();
        for _ in 0..2 {
            let kernel = node.kernel.clone();
            let barrier = Arc::clone(&barrier);
            let proof_hex = proof_hex.clone();
            let credential_hex = credential_hex.clone();
            let request = CompleteRecoveryBootstrapRequest::new(
                node.restore_attempt_id,
                RequestCorrelationId::new_v7(),
                node.workspace_id,
                node.profile_id,
                client_id,
                SecretMaterial::try_from_hex(&proof_hex).expect("copy proof"),
                SecretMaterial::try_from_hex(&credential_hex).expect("copy credential"),
            );
            threads.push(std::thread::spawn(move || {
                barrier.wait();
                kernel.complete_recovery_bootstrap_transaction(request)
            }));
        }
        barrier.wait();
        let first = threads
            .remove(0)
            .join()
            .expect("first completion thread")
            .expect("first completion");
        let second = threads
            .remove(0)
            .join()
            .expect("second completion thread")
            .expect("second completion");
        assert_eq!(first, second);

        let snapshot = node.snapshot();
        assert_eq!(snapshot.credentials.len(), 2);
        assert_eq!(snapshot.profile_grants.len(), 1);
        assert_eq!(snapshot.grant_scopes.len(), FULL_ADMIN_SCOPES.len());
    }

    fn assert_plaintext_secrets_absent(node: &RecoveryNode, secrets: &[&str]) {
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            let _: (i64, i64, i64) = connection
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .expect("checkpoint recovery database");
        }
        for entry in
            std::fs::read_dir(node.root.path().join("current")).expect("read current data root")
        {
            let entry = entry.expect("data-root entry");
            if !entry.file_type().expect("entry type").is_file() {
                continue;
            }
            let bytes = std::fs::read(entry.path()).expect("read data-root file");
            for secret in secrets {
                assert!(
                    !bytes
                        .windows(secret.len())
                        .any(|window| window == secret.as_bytes()),
                    "plaintext secret appeared in {}",
                    entry.path().display()
                );
            }
        }
    }

    #[test]
    fn enrollment_consumes_the_proof_and_replaces_its_only_scope() {
        let node = TestNode::new();
        let replay_proof = SecretMaterial::try_from_hex(&node.initialization_proof_hex)
            .expect("copy consumed proof for replay");
        let error = match node
            .kernel
            .enroll_first_client(EnrollFirstClientCommand::new(
                RequestCorrelationId::new_v7(),
                replay_proof,
            )) {
            Ok(_) => panic!("consumed proof must not enroll again"),
            Err(error) => error,
        };
        assert_eq!(error.code(), ProblemCode::BootstrapClosed);

        let authentication_proof = SecretMaterial::try_from_hex(&node.initialization_proof_hex)
            .expect("copy consumed proof for authentication");
        let authentication_error = node
            .kernel
            .authenticate_credential(AuthenticateCredentialQuery::new(
                RequestCorrelationId::new_v7(),
                CapabilityKey::InspectReview,
                authentication_proof,
            ))
            .expect_err("consumed proof must not authenticate");
        assert_eq!(
            authentication_error.code(),
            ProblemCode::AuthenticationFailed
        );
        assert_eq!(
            authentication_error.capability(),
            CapabilityKey::InspectReview
        );

        let connection = node
            .kernel
            .inner
            .connection
            .lock()
            .expect("SQLite connection");
        let mut statement = connection
            .prepare("SELECT scope_key FROM grant_scopes WHERE grant_id = ?1 ORDER BY scope_key")
            .expect("prepare scope query");
        let scopes = statement
            .query_map([node.grant_id.to_string()], |row| row.get::<_, String>(0))
            .expect("query final scopes")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect final scopes");
        assert_eq!(scopes.len(), FULL_ADMIN_SCOPES.len());
        assert!(!scopes
            .iter()
            .any(|scope| scope == scope_storage_key(ScopeKey::ClientEnroll)));
        for required_scope in FULL_ADMIN_SCOPES {
            assert!(scopes
                .iter()
                .any(|scope| scope == scope_storage_key(*required_scope)));
        }
    }

    #[test]
    fn initialization_rejects_a_restored_workspace_without_node_state() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
        let restored_workspace = WorkspaceId::new_v7();
        {
            let connection = kernel.inner.connection.lock().expect("SQLite connection");
            connection
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![restored_workspace.to_string(), timestamp(now())],
                )
                .expect("insert restored workspace");
        }

        let error = match kernel
            .initialize_node(InitializeNodeCommand::new(RequestCorrelationId::new_v7()))
        {
            Ok(_) => panic!("restored workspace must keep bootstrap closed"),
            Err(error) => error,
        };
        assert_eq!(error.code(), ProblemCode::AlreadyInitialized);

        let connection = kernel.inner.connection.lock().expect("SQLite connection");
        let workspace_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM workspaces", [], |row| row.get(0))
            .expect("workspace count");
        let node_state_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM node_state", [], |row| row.get(0))
            .expect("node-state count");
        assert_eq!(workspace_count, 1);
        assert_eq!(node_state_count, 0);
    }

    #[test]
    fn credential_authentication_accepts_one_active_profile_grant() {
        let node = TestNode::new();
        let access = node.authenticate().expect("single active grant");

        assert_eq!(access.workspace_id(), node.workspace_id);
        assert_eq!(access.profile_id(), node.profile_id);
        assert_eq!(access.client_id(), node.client_id);
        assert_eq!(access.grant_id(), node.grant_id);
    }

    #[test]
    fn credential_authentication_rejects_ambiguous_active_profile_grants() {
        let node = TestNode::new();
        node.insert_profile_and_grant(node.workspace_id, ProfileId::new_v7());

        assert!(node.authenticate().is_err());
    }

    #[test]
    fn bootstrap_secret_recovers_from_a_zero_length_file_left_by_a_crashed_write() {
        let root = tempfile::tempdir().expect("temporary data root");
        // A write that reached open()/create_new() but crashed before
        // write_all() completed -- exactly what the old non-atomic
        // create-then-write left behind, and what would have permanently
        // wedged every future startup without an atomic publish.
        std::fs::write(root.path().join("bootstrap.secret"), b"")
            .expect("seed a zero-length secret file");

        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
        let secret = kernel
            .ensure_bootstrap_secret()
            .expect("a zero-length prior file must not be treated as durably published");

        let republished = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret file readable after recovery");
        assert_eq!(republished.trim(), secret.expose_hex());
        assert!(
            !root.path().join("bootstrap.secret.tmp").exists(),
            "the temporary publish file must not linger after a successful rename"
        );
    }

    #[test]
    fn bootstrap_secret_is_stable_across_repeated_calls() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");

        let first = kernel
            .ensure_bootstrap_secret()
            .expect("first bootstrap secret");
        let second = kernel
            .ensure_bootstrap_secret()
            .expect("second bootstrap secret");

        assert_eq!(first.expose_hex(), second.expose_hex());
    }

    #[test]
    fn concurrent_bootstrap_secret_publishers_agree_on_one_value() {
        // `SqliteKernel::open` takes an exclusive OS-level lock on the data
        // root (StoreOpenError::DataRootLocked), so two *processes* (two
        // separate `open()` calls) can never race to publish this file --
        // only one can ever hold an open kernel against a given data root.
        // The scenario worth guarding is concurrent *callers sharing one
        // already-open kernel* (e.g. multiple threads before the code that
        // primes this at startup is known to run exactly once) -- this
        // exercises that instead of racing separate `open()` calls, which
        // would just deadlock on the data-root lock rather than test
        // anything about the publish logic itself.
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
        let barrier = Barrier::new(8);

        let results: Vec<String> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let kernel = &kernel;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        kernel
                            .ensure_bootstrap_secret()
                            .expect("bootstrap secret")
                            .expose_hex()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("publisher thread panicked"))
                .collect()
        });

        let persisted = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret file readable after concurrent publishing");
        for result in &results {
            assert_eq!(
                result,
                persisted.trim(),
                "every concurrent caller must return the value that was actually persisted, \
                 never a value only it generated but lost the publish race for"
            );
        }
    }

    #[test]
    fn concurrent_recovery_from_a_pre_existing_malformed_secret_agrees_on_one_value() {
        // Same threat model as concurrent_bootstrap_secret_publishers_agree_on_one_value,
        // but starting from a malformed (not missing) file, so every caller
        // enters the "invalid -> remove stale -> republish" recovery branch
        // together, instead of the plain "no file yet" branch. Without
        // bootstrap_secret serializing this sequence, a caller whose
        // validity check ran before a sibling's publish can go on to delete
        // that sibling's freshly-published valid secret and republish a
        // different one, leaving callers disagreeing about which secret is
        // actually persisted.
        let root = tempfile::tempdir().expect("temporary data root");
        std::fs::write(root.path().join("bootstrap.secret"), b"not-a-valid-secret")
            .expect("seed a malformed secret file");
        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
        let barrier = Barrier::new(8);

        let results: Vec<String> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let kernel = &kernel;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        kernel
                            .ensure_bootstrap_secret()
                            .expect("bootstrap secret")
                            .expose_hex()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("publisher thread panicked"))
                .collect()
        });

        let persisted = std::fs::read_to_string(root.path().join("bootstrap.secret"))
            .expect("bootstrap secret file readable after concurrent recovery");
        for result in &results {
            assert_eq!(
                result,
                persisted.trim(),
                "every concurrent caller recovering from the same malformed file must return \
                 the value that was actually persisted, never a value only it generated but \
                 lost the recovery race for"
            );
        }
    }

    #[test]
    fn credential_authentication_rejects_a_cross_workspace_grant() {
        let node = TestNode::new();
        let foreign_workspace = WorkspaceId::new_v7();
        let foreign_profile = ProfileId::new_v7();
        let created_at = timestamp(now());
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![foreign_workspace.to_string(), created_at],
                )
                .expect("insert foreign workspace");
        }
        node.insert_profile_and_grant(foreign_workspace, foreign_profile);
        {
            let connection = node
                .kernel
                .inner
                .connection
                .lock()
                .expect("SQLite connection");
            connection
                .execute(
                    "UPDATE profile_grants SET status = 'revoked', revoked_at = ?1 WHERE grant_id = ?2",
                    params![timestamp(now()), node.grant_id.to_string()],
                )
                .expect("revoke original grant");
        }

        assert!(node.authenticate().is_err());
    }
}
