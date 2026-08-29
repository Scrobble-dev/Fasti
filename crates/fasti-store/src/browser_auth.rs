use crate::kernel::{
    digest_secret, load_access_snapshot, map_sql, now, parse_timestamp, random_secret, timestamp,
    verify_digest, SqliteKernel,
};
use argon2::{
    password_hash::{Error as PasswordHashError, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use chrono::Duration;
use fasti_application::{
    authorize, AccessAdministrationPort, ApplicationResult, AuthenticateBrowserSessionQuery,
    AuthenticatedBrowserSession, AuthorizationRequirement, BrowserAccountPort, BrowserPassword,
    BrowserSessionSummary, BrowserUserView, BrowserUsername, CapabilityKey,
    CreateBrowserSessionCommand, CreatedBrowserSession, DeleteBrowserUserCommand,
    EndAllOtherBrowserSessionsCommand, EndBrowserSessionCommand, EndSpecificBrowserSessionCommand,
    EnrollFirstClientCommand, FastiProblem, InitializeNodeCommand, ListBrowserSessionsQuery,
    ListBrowserUsersQuery, ProblemCode, RequestAccessContext, ScopeKey,
    SwitchBrowserSessionProfileCommand, UpdateBrowserUserCommand, Violation,
};
use fasti_domain::{BrowserUserId, ClientId, CredentialId, ProfileGrantId, ProfileId, WorkspaceId};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

const MAX_FAILED_LOGINS: i64 = 5;
const LOCKOUT_MINUTES: i64 = 15;
type BrowserUserRow = (String, String, i64, i64, i64, String, String);

fn problem(
    code: ProblemCode,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(code, capability, correlation_id))
}

fn hash_password(
    password: &BrowserPassword,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<String> {
    let salt_material = random_secret(capability, correlation_id)?;
    let salt = SaltString::encode_b64(salt_material.expose_bytes())
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    Argon2::default()
        .hash_password(password.expose_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))
}

fn verify_password(
    password: &BrowserPassword,
    encoded: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<bool> {
    let hash = PasswordHash::new(encoded)
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    match Argon2::default().verify_password(password.expose_bytes(), &hash) {
        Ok(()) => Ok(true),
        Err(PasswordHashError::Password) => Ok(false),
        Err(_) => Err(problem(
            ProblemCode::IntegrityFailed,
            capability,
            correlation_id,
        )),
    }
}

fn consume_dummy_password_work(
    password: &BrowserPassword,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    let salt = SaltString::encode_b64(b"fasti-browser-auth")
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    Argon2::default()
        .hash_password(password.expose_bytes(), &salt)
        .map(|_| ())
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))
}

fn user_view(
    row: BrowserUserRow,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<BrowserUserView> {
    let (user_id, username, is_admin, is_test_account, active, created_at, updated_at) = row;
    Ok(BrowserUserView::new(
        user_id
            .parse::<BrowserUserId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        username,
        is_admin != 0,
        is_test_account != 0,
        active != 0,
        parse_timestamp(&created_at, capability, correlation_id)?,
        parse_timestamp(&updated_at, capability, correlation_id)?,
    ))
}

pub(crate) fn viable_administrator_count(
    connection: &Connection,
    workspace_id: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<i64> {
    map_sql(
        connection.query_row(
            r#"
            WITH viable_administrators AS (
                SELECT DISTINCT u.user_id
                FROM browser_users u
                JOIN clients c
                  ON c.client_id = u.client_id
                 AND c.workspace_id = ?1
                 AND c.status = 'active'
                JOIN credentials cr
                  ON cr.client_id = c.client_id
                 AND cr.workspace_id = c.workspace_id
                 AND cr.epoch = c.current_credential_epoch
                 AND cr.status = 'active'
                JOIN profile_grants pg
                  ON pg.client_id = c.client_id
                 AND pg.workspace_id = c.workspace_id
                 AND pg.profile_id = u.profile_id
                 AND pg.status = 'active'
                JOIN grant_scopes gs
                  ON gs.grant_id = pg.grant_id
                 AND gs.scope_key = ?2
                WHERE u.is_admin = 1
                  AND u.active = 1
            )
            SELECT COUNT(*) FROM viable_administrators
            "#,
            params![workspace_id, ScopeKey::BrowserUserManage.as_str()],
            |row| row.get(0),
        ),
        capability,
        correlation_id,
    )
}

fn ensure_viable_administrator_remains(
    connection: &Connection,
    workspace_id: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    if viable_administrator_count(connection, workspace_id, capability, correlation_id)? == 0 {
        let violation = Violation::try_new(
            "last_active_administrator_required",
            "/",
            "this change would remove the workspace's last active administrator with browser-user management access",
            "at least one active administrator with browser-user management access must remain",
        )
        .expect("the administrator continuity violation is valid");
        return Err(Box::new(
            FastiProblem::validation_failed(capability, correlation_id, vec![violation])
                .expect("one validation violation is within bounds"),
        ));
    }
    Ok(())
}

fn load_node_access(
    connection: &Connection,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<RequestAccessContext> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT cr.workspace_id, ns.profile_id, ns.client_id,
                       cr.credential_id, pg.grant_id, cr.epoch
                FROM node_state ns
                JOIN clients c ON c.client_id = ns.client_id
                JOIN credentials cr
                  ON cr.client_id = c.client_id
                 AND cr.workspace_id = c.workspace_id
                 AND cr.epoch = c.current_credential_epoch
                 AND cr.status = 'active'
                JOIN profile_grants pg
                  ON pg.client_id = c.client_id
                 AND pg.workspace_id = c.workspace_id
                 AND pg.profile_id = ns.profile_id
                 AND pg.status = 'active'
                WHERE ns.singleton = 1 AND ns.initialized = 1 AND c.status = 'active'
                "#,
                [],
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
    )?
    .ok_or_else(|| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    let (workspace, profile, client, credential, grant, epoch) = row;
    Ok(RequestAccessContext::new(
        workspace
            .parse::<WorkspaceId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        profile
            .parse::<ProfileId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        client
            .parse::<ClientId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        credential
            .parse::<CredentialId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        grant
            .parse::<ProfileGrantId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        u64::try_from(epoch)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
    ))
}

fn authenticate_session(
    connection: &Connection,
    query: &AuthenticateBrowserSessionQuery,
) -> ApplicationResult<AuthenticatedBrowserSession> {
    let capability = query.capability();
    let correlation_id = query.correlation_id();
    let session_digest = digest_secret(query.session());
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT u.user_id, u.username, u.is_admin, u.is_test_account,
                       u.active, u.created_at, u.updated_at,
                       s.csrf_digest, s.expires_at,
                       c.workspace_id, u.profile_id, u.client_id,
                       cr.credential_id, pg.grant_id, cr.epoch
                FROM browser_sessions s
                JOIN browser_users u ON u.user_id = s.user_id
                JOIN clients c ON c.client_id = u.client_id AND c.status = 'active'
                JOIN credentials cr
                  ON cr.client_id = c.client_id
                 AND cr.workspace_id = c.workspace_id
                 AND cr.epoch = c.current_credential_epoch
                 AND cr.status = 'active'
                JOIN profile_grants pg
                  ON pg.client_id = c.client_id
                 AND pg.workspace_id = c.workspace_id
                 AND pg.profile_id = u.profile_id
                 AND pg.status = 'active'
                WHERE s.session_digest = ?1 AND u.active = 1
                "#,
                [session_digest.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, i64>(14)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((
        user_id,
        username,
        is_admin,
        is_test,
        active,
        created_at,
        updated_at,
        csrf_digest,
        expires_at,
        workspace,
        profile,
        client,
        credential,
        grant,
        epoch,
    )) = row
    else {
        return Err(problem(
            ProblemCode::AuthenticationFailed,
            capability,
            correlation_id,
        ));
    };

    let expires_at = parse_timestamp(&expires_at, capability, correlation_id)?;
    if expires_at <= now() {
        let _ = connection.execute(
            "DELETE FROM browser_sessions WHERE session_digest = ?1",
            [session_digest],
        );
        return Err(problem(
            ProblemCode::AuthenticationFailed,
            capability,
            correlation_id,
        ));
    }
    if query.require_csrf() {
        let presented = query.csrf().map(digest_secret);
        if !presented
            .as_deref()
            .is_some_and(|value| verify_digest(&csrf_digest, value))
        {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
    }

    let access = RequestAccessContext::new(
        workspace
            .parse::<WorkspaceId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        profile
            .parse::<ProfileId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        client
            .parse::<ClientId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        credential
            .parse::<CredentialId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        grant
            .parse::<ProfileGrantId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        u64::try_from(epoch)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
    );
    let snapshot = load_access_snapshot(connection, &access, capability, correlation_id)?;
    authorize(
        &AuthorizationRequirement::for_capability(capability),
        Some(&access),
        Some(&snapshot),
    )
    .map_err(|_| problem(ProblemCode::Forbidden, capability, correlation_id))?;
    Ok(AuthenticatedBrowserSession::new(
        user_view(
            (
                user_id, username, is_admin, is_test, active, created_at, updated_at,
            ),
            capability,
            correlation_id,
        )?,
        access,
        expires_at,
    ))
}

impl BrowserAccountPort for SqliteKernel {
    fn ensure_development_browser_user(
        &self,
        username: BrowserUsername,
        password: BrowserPassword,
    ) -> ApplicationResult<()> {
        let capability = CapabilityKey::CreateBrowserSession;
        let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
        {
            let connection = self.lock_connection(capability, correlation_id)?;
            let seeded: bool = map_sql(
                connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM browser_auth_bootstrap WHERE singleton = 1)",
                    [],
                    |row| row.get(0),
                ),
                capability,
                correlation_id,
            )?;
            if seeded {
                return Ok(());
            }
        }

        let initialized = {
            let connection = self.lock_connection(capability, correlation_id)?;
            map_sql(
                connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM node_state WHERE singleton = 1 AND initialized = 1)",
                    [],
                    |row| row.get::<_, bool>(0),
                ),
                capability,
                correlation_id,
            )?
        };
        let access = if initialized {
            let connection = self.lock_connection(capability, correlation_id)?;
            load_node_access(&connection, capability, correlation_id)?
        } else {
            let initialized = self.initialize_node(InitializeNodeCommand::new(correlation_id))?;
            let proof = fasti_application::SecretMaterial::try_from_hex(
                &initialized.initialization_proof().expose_hex(),
            )
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
            *self
                .enroll_first_client(EnrollFirstClientCommand::new(correlation_id, proof))?
                .access()
        };
        let password_hash = hash_password(&password, capability, correlation_id)?;
        let user_id = BrowserUserId::new_v7();
        let created_at = timestamp(now());
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let already_seeded: bool = map_sql(
            transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM browser_auth_bootstrap WHERE singleton = 1)",
                [],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;
        if already_seeded {
            return Ok(());
        }
        map_sql(
            transaction.execute(
                r#"INSERT INTO browser_users(
                    user_id, username, password_hash, client_id, profile_id,
                    is_admin, is_test_account, active, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 1, 1, ?6, ?6)"#,
                params![
                    user_id.to_string(),
                    username.as_str(),
                    password_hash,
                    access.client_id().to_string(),
                    access.profile_id().to_string(),
                    created_at,
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "INSERT INTO browser_auth_bootstrap(singleton, seeded_at) VALUES (1, ?1)",
                [created_at],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)
    }

    fn create_browser_session(
        &self,
        command: CreateBrowserSessionCommand,
    ) -> ApplicationResult<CreatedBrowserSession> {
        let capability = CapabilityKey::CreateBrowserSession;
        let correlation_id = command.correlation_id();
        let session = random_secret(capability, correlation_id)?;
        let csrf = random_secret(capability, correlation_id)?;
        let session_digest = digest_secret(&session);
        let csrf_digest = digest_secret(&csrf);
        let created_at = now();
        let expires_at = created_at + Duration::minutes(i64::from(command.lifetime_minutes()));
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let row = map_sql(
            transaction
                .query_row(
                    r#"SELECT user_id, username, password_hash, is_admin, is_test_account,
                          active, failed_login_count, locked_until, created_at
                   FROM browser_users WHERE username = ?1"#,
                    [command.username().as_str()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5)?,
                            row.get::<_, i64>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, String>(8)?,
                        ))
                    },
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let Some((
            user_id,
            username,
            password_hash,
            is_admin,
            is_test,
            active,
            failed_count,
            locked_until,
            user_created_at,
        )) = row
        else {
            consume_dummy_password_work(command.password(), capability, correlation_id)?;
            return Err(problem(
                ProblemCode::AuthenticationFailed,
                capability,
                correlation_id,
            ));
        };
        let locked = match locked_until.as_deref() {
            Some(value) => parse_timestamp(value, capability, correlation_id)? > created_at,
            None => false,
        };
        let failed_count = if locked_until.is_some() && !locked {
            0
        } else {
            failed_count
        };
        if active == 0 || locked {
            consume_dummy_password_work(command.password(), capability, correlation_id)?;
            return Err(problem(
                ProblemCode::AuthenticationFailed,
                capability,
                correlation_id,
            ));
        }
        if !verify_password(
            command.password(),
            &password_hash,
            capability,
            correlation_id,
        )? {
            let failures = failed_count.saturating_add(1);
            let lock_until = (failures >= MAX_FAILED_LOGINS)
                .then(|| timestamp(created_at + Duration::minutes(LOCKOUT_MINUTES)));
            map_sql(
                transaction.execute(
                    "UPDATE browser_users SET failed_login_count = ?1, locked_until = ?2, updated_at = ?3 WHERE user_id = ?4",
                    params![failures, lock_until, timestamp(created_at), user_id],
                ),
                capability,
                correlation_id,
            )?;
            map_sql(transaction.commit(), capability, correlation_id)?;
            return Err(problem(
                ProblemCode::AuthenticationFailed,
                capability,
                correlation_id,
            ));
        }
        map_sql(
            transaction.execute(
                "UPDATE browser_users SET failed_login_count = 0, locked_until = NULL, updated_at = ?1 WHERE user_id = ?2",
                params![timestamp(created_at), user_id],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "DELETE FROM browser_sessions WHERE expires_at <= ?1",
                [timestamp(created_at)],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(
            transaction.execute(
                "INSERT INTO browser_sessions(session_digest, csrf_digest, user_id, expires_at, created_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![session_digest, csrf_digest, user_id, timestamp(expires_at), timestamp(created_at)],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(CreatedBrowserSession::new(
            user_view(
                (
                    user_id,
                    username,
                    is_admin,
                    is_test,
                    active,
                    user_created_at,
                    timestamp(created_at),
                ),
                capability,
                correlation_id,
            )?,
            session,
            csrf,
            expires_at,
        ))
    }

    fn authenticate_browser_session(
        &self,
        query: AuthenticateBrowserSessionQuery,
    ) -> ApplicationResult<AuthenticatedBrowserSession> {
        let connection = self.lock_connection(query.capability(), query.correlation_id())?;
        authenticate_session(&connection, &query)
    }

    fn end_browser_session(&self, command: EndBrowserSessionCommand) -> ApplicationResult<()> {
        let capability = CapabilityKey::EndBrowserSession;
        let (correlation_id, session, csrf) = command.into_parts();
        let session_digest = digest_secret(&session);
        let connection = self.lock_connection(capability, correlation_id)?;
        authenticate_session(
            &connection,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session,
                Some(csrf),
                true,
            ),
        )?;
        let changed = map_sql(
            connection.execute(
                "DELETE FROM browser_sessions WHERE session_digest = ?1",
                [session_digest],
            ),
            capability,
            correlation_id,
        )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(problem(
                ProblemCode::AuthenticationFailed,
                capability,
                correlation_id,
            ))
        }
    }

    fn list_browser_sessions(
        &self,
        query: ListBrowserSessionsQuery,
    ) -> ApplicationResult<Vec<BrowserSessionSummary>> {
        let capability = CapabilityKey::ReadBrowserSession;
        let (correlation_id, session_secret) = query.into_parts();
        let current_digest = digest_secret(&session_secret);
        let connection = self.lock_connection(capability, correlation_id)?;
        let session = authenticate_session(
            &connection,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session_secret,
                None,
                false,
            ),
        )?;
        let user_id = session.user().user_id().to_string();
        let mut statement = map_sql(
            connection.prepare(
                r#"
                SELECT session_digest, created_at, expires_at, last_seen_at
                FROM browser_sessions
                WHERE user_id = ?1
                ORDER BY created_at DESC
                "#,
            ),
            capability,
            correlation_id,
        )?;
        let rows = map_sql(
            statement.query_map([user_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            }),
            capability,
            correlation_id,
        )?;
        let mut summaries = Vec::new();
        let current_time = now();
        for row in rows {
            let (digest, created_at, expires_at, last_seen_at) =
                map_sql(row, capability, correlation_id)?;
            let parsed_expires = parse_timestamp(&expires_at, capability, correlation_id)?;
            if parsed_expires <= current_time {
                continue;
            }
            let is_curr = verify_digest(&digest, &current_digest);
            let parsed_created = parse_timestamp(&created_at, capability, correlation_id)?;
            let parsed_last_seen = parse_timestamp(&last_seen_at, capability, correlation_id)?;
            let short_id = if digest.len() >= 16 {
                format!("sess_{}", &digest[..12])
            } else {
                format!("sess_{}", &digest)
            };
            summaries.push(BrowserSessionSummary::new(
                short_id,
                parsed_created,
                parsed_expires,
                parsed_last_seen,
                "Local Host (127.0.0.1)".to_string(),
                "Desktop Browser (Workbench)".to_string(),
                is_curr,
            ));
        }
        Ok(summaries)
    }

    fn end_specific_browser_session(
        &self,
        command: EndSpecificBrowserSessionCommand,
    ) -> ApplicationResult<bool> {
        let capability = CapabilityKey::EndBrowserSession;
        let (correlation_id, session_secret, csrf_secret, target_session_id) = command.into_parts();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let caller = authenticate_session(
            &transaction,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session_secret,
                Some(csrf_secret),
                true,
            ),
        )?;
        let user_id = caller.user().user_id().to_string();
        let target_prefix = target_session_id
            .strip_prefix("sess_")
            .unwrap_or(&target_session_id);
        if target_prefix.len() < 8 || !target_prefix.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(problem(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            ));
        }
        let count: i64 = map_sql(
            transaction.query_row(
                "SELECT count(*) FROM browser_sessions WHERE user_id = ?1 AND (session_digest = ?2 OR session_digest LIKE ?2 || '%')",
                params![user_id, target_prefix],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;
        if count == 0 {
            return Ok(false);
        }
        if count > 1 && target_prefix.len() < 64 {
            return Err(problem(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            ));
        }
        let changed = map_sql(
            transaction.execute(
                "DELETE FROM browser_sessions WHERE user_id = ?1 AND (session_digest = ?2 OR (length(?2) >= 8 AND session_digest LIKE ?2 || '%'))",
                params![user_id, target_prefix],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(changed > 0)
    }

    fn end_all_other_browser_sessions(
        &self,
        command: EndAllOtherBrowserSessionsCommand,
    ) -> ApplicationResult<u64> {
        let capability = CapabilityKey::EndBrowserSession;
        let (correlation_id, session_secret, csrf_secret) = command.into_parts();
        let current_digest = digest_secret(&session_secret);
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let caller = authenticate_session(
            &transaction,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session_secret,
                Some(csrf_secret),
                true,
            ),
        )?;
        let user_id = caller.user().user_id().to_string();
        let changed = map_sql(
            transaction.execute(
                "DELETE FROM browser_sessions WHERE user_id = ?1 AND session_digest != ?2",
                params![user_id, current_digest],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(changed as u64)
    }

    fn switch_browser_session_profile(
        &self,
        command: SwitchBrowserSessionProfileCommand,
    ) -> ApplicationResult<AuthenticatedBrowserSession> {
        let capability = CapabilityKey::ReadBrowserSession;
        let (correlation_id, session_secret, csrf_secret, target_profile_id) = command.into_parts();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let caller = authenticate_session(
            &transaction,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session_secret,
                Some(csrf_secret),
                true,
            ),
        )?;
        let workspace_id = caller.access().workspace_id().to_string();
        let client_id = caller.access().client_id().to_string();
        let target_profile_str = target_profile_id.to_string();

        let profile_exists: bool = map_sql(
            transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM profiles WHERE profile_id = ?1 AND workspace_id = ?2)",
                params![target_profile_str, workspace_id],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;
        if !profile_exists {
            return Err(problem(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            ));
        }

        let grant_exists: bool = map_sql(
            transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM profile_grants WHERE client_id = ?1 AND profile_id = ?2 AND workspace_id = ?3 AND status = 'active')",
                params![client_id, target_profile_str, workspace_id],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;

        if !caller.user().is_admin()
            && !grant_exists
            && caller.access().profile_id() != target_profile_id
        {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
        let grant_id = if !grant_exists {
            if !caller.user().is_admin() {
                return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
            }
            let gid = format!("pg_{}", fasti_domain::ProfileGrantId::new_v7());
            let created_at = timestamp(now());
            map_sql(
                transaction.execute(
                    "INSERT INTO profile_grants (grant_id, client_id, workspace_id, profile_id, status, created_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5)",
                    params![gid, client_id, workspace_id, target_profile_str, created_at],
                ),
                capability,
                correlation_id,
            )?;
            gid
        } else {
            let gid: String = map_sql(
                transaction.query_row(
                    "SELECT grant_id FROM profile_grants WHERE client_id = ?1 AND profile_id = ?2 AND workspace_id = ?3 AND status = 'active'",
                    params![client_id, target_profile_str, workspace_id],
                    |row| row.get(0),
                ),
                capability,
                correlation_id,
            )?;
            gid
        };

        let updated_at = timestamp(now());
        map_sql(
            transaction.execute(
                "UPDATE browser_users SET profile_id = ?1, updated_at = ?2 WHERE user_id = ?3",
                params![
                    target_profile_str,
                    updated_at,
                    caller.user().user_id().to_string()
                ],
            ),
            capability,
            correlation_id,
        )?;

        map_sql(transaction.commit(), capability, correlation_id)?;

        let parsed_grant_id = grant_id
            .parse::<ProfileGrantId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        let new_access = RequestAccessContext::new(
            caller.access().workspace_id(),
            target_profile_id,
            caller.access().client_id(),
            caller.access().credential_id(),
            parsed_grant_id,
            caller.access().presented_credential_epoch(),
        );

        Ok(AuthenticatedBrowserSession::new(
            caller.user().clone(),
            new_access,
            caller.expires_at(),
        ))
    }

    fn list_browser_users(
        &self,
        query: ListBrowserUsersQuery,
    ) -> ApplicationResult<Vec<BrowserUserView>> {
        let capability = CapabilityKey::ListBrowserUsers;
        let (correlation_id, session_secret) = query.into_parts();
        let connection = self.lock_connection(capability, correlation_id)?;
        let session = authenticate_session(
            &connection,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session_secret,
                None,
                false,
            ),
        )?;
        if !session.user().is_admin() {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
        let workspace_id = session.access().workspace_id().to_string();
        let mut statement = map_sql(
            connection.prepare(
                r#"
                SELECT u.user_id, u.username, u.is_admin, u.is_test_account,
                       u.active, u.created_at, u.updated_at
                FROM browser_users u
                JOIN clients c ON c.client_id = u.client_id
                WHERE c.workspace_id = ?1
                ORDER BY u.username
                "#,
            ),
            capability,
            correlation_id,
        )?;
        let rows = map_sql(
            statement.query_map([workspace_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            }),
            capability,
            correlation_id,
        )?;
        let mut users = Vec::new();
        for row in rows {
            users.push(user_view(
                map_sql(row, capability, correlation_id)?,
                capability,
                correlation_id,
            )?);
        }
        Ok(users)
    }

    fn update_browser_user(
        &self,
        command: UpdateBrowserUserCommand,
    ) -> ApplicationResult<BrowserUserView> {
        let capability = CapabilityKey::UpdateBrowserUser;
        let (
            correlation_id,
            session,
            csrf,
            target_user_id,
            current_password,
            username,
            password,
            active,
        ) = command.into_parts();
        let password_hash = password
            .as_ref()
            .map(|value| hash_password(value, capability, correlation_id))
            .transpose()?;
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let caller = authenticate_session(
            &transaction,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session,
                Some(csrf),
                true,
            ),
        )?;
        if !caller.user().is_admin() {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
        let workspace_id = caller.access().workspace_id().to_string();
        let caller_hash: String = map_sql(
            transaction.query_row(
                "SELECT password_hash FROM browser_users WHERE user_id = ?1",
                [caller.user().user_id().to_string()],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;
        if !verify_password(&current_password, &caller_hash, capability, correlation_id)? {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
        if let Some(username) = username.as_ref() {
            let conflict: bool = map_sql(transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM browser_users WHERE username = ?1 AND user_id <> ?2)",
                params![username.as_str(), target_user_id.to_string()], |row| row.get(0)), capability, correlation_id)?;
            if conflict {
                return Err(problem(
                    ProblemCode::ValidationFailed,
                    capability,
                    correlation_id,
                ));
            }
        }
        let updated_at = timestamp(now());
        let changed = map_sql(
            transaction.execute(
                r#"UPDATE browser_users
               SET username = COALESCE(?1, username), password_hash = COALESCE(?2, password_hash),
                   active = COALESCE(?3, active), updated_at = ?4
               WHERE user_id = ?5
                 AND client_id IN (
                     SELECT client_id FROM clients WHERE workspace_id = ?6
                 )"#,
                params![
                    username.as_ref().map(BrowserUsername::as_str),
                    password_hash,
                    active.map(i64::from),
                    updated_at,
                    target_user_id.to_string(),
                    workspace_id
                ],
            ),
            capability,
            correlation_id,
        )?;
        if changed != 1 {
            return Err(problem(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            ));
        }
        if active == Some(false) {
            ensure_viable_administrator_remains(
                &transaction,
                &workspace_id,
                capability,
                correlation_id,
            )?;
        }
        if username.is_some() || password.is_some() || active == Some(false) {
            map_sql(
                transaction.execute(
                    "DELETE FROM browser_sessions WHERE user_id = ?1",
                    [target_user_id.to_string()],
                ),
                capability,
                correlation_id,
            )?;
        }
        let row = map_sql(
            transaction.query_row(
                r#"
            SELECT u.user_id, u.username, u.is_admin, u.is_test_account,
                   u.active, u.created_at, u.updated_at
            FROM browser_users u
            JOIN clients c ON c.client_id = u.client_id
            WHERE u.user_id = ?1 AND c.workspace_id = ?2
            "#,
                params![target_user_id.to_string(), workspace_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        user_view(row, capability, correlation_id)
    }

    fn delete_browser_user(&self, command: DeleteBrowserUserCommand) -> ApplicationResult<bool> {
        let capability = CapabilityKey::DeleteBrowserUser;
        let (correlation_id, session, csrf, target_user_id, current_password) =
            command.into_parts();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let caller = authenticate_session(
            &transaction,
            &AuthenticateBrowserSessionQuery::new(
                correlation_id,
                capability,
                session,
                Some(csrf),
                true,
            ),
        )?;
        if !caller.user().is_admin() {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
        let workspace_id = caller.access().workspace_id().to_string();
        let caller_hash: String = map_sql(
            transaction.query_row(
                "SELECT password_hash FROM browser_users WHERE user_id = ?1",
                [caller.user().user_id().to_string()],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;
        if !verify_password(&current_password, &caller_hash, capability, correlation_id)? {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
        let deleted_self = caller.user().user_id() == target_user_id;
        let changed = map_sql(
            transaction.execute(
                r#"
                DELETE FROM browser_users
                WHERE user_id = ?1
                  AND client_id IN (
                      SELECT client_id FROM clients WHERE workspace_id = ?2
                  )
                "#,
                params![target_user_id.to_string(), workspace_id],
            ),
            capability,
            correlation_id,
        )?;
        if changed != 1 {
            return Err(problem(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            ));
        }
        ensure_viable_administrator_remains(
            &transaction,
            &workspace_id,
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(deleted_self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fasti_application::{
        ClientCredentialAdministrationPort, CreateScopedClientCredentialCommand,
        CreateScopedClientCredentialOutcome, RevokeClientCredentialCommand,
    };

    fn login(
        kernel: &SqliteKernel,
        username: &str,
        password: &str,
    ) -> ApplicationResult<CreatedBrowserSession> {
        kernel.create_browser_session(
            CreateBrowserSessionCommand::try_new(
                fasti_domain::RequestCorrelationId::new_v7(),
                BrowserUsername::try_new(username).expect("username"),
                BrowserPassword::try_new(password).expect("password"),
                60,
            )
            .expect("command"),
        )
    }

    fn add_second_administrator(
        kernel: &SqliteKernel,
        first: &CreatedBrowserSession,
    ) -> (RequestAccessContext, CreateScopedClientCredentialOutcome) {
        let access = {
            let connection = Connection::open(kernel.database_path()).expect("database");
            load_node_access(
                &connection,
                CapabilityKey::RotateCredential,
                fasti_domain::RequestCorrelationId::new_v7(),
            )
            .expect("load administrator access")
        };
        let second_access = kernel
            .create_scoped_client_credential(CreateScopedClientCredentialCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                access,
                vec![ScopeKey::BrowserUserManage],
            ))
            .expect("create second administrator access");
        let connection = Connection::open(kernel.database_path()).expect("database");
        connection
            .execute(
                r#"
                INSERT INTO browser_users(
                    user_id, username, password_hash, client_id, profile_id,
                    is_admin, is_test_account, active, created_at, updated_at
                )
                SELECT ?1, 'secondadmin', password_hash, ?2, ?3,
                       1, 0, 1, created_at, updated_at
                FROM browser_users WHERE user_id = ?4
                "#,
                params![
                    BrowserUserId::new_v7().to_string(),
                    second_access.client_id().to_string(),
                    second_access.profile_id().to_string(),
                    first.user().user_id().to_string()
                ],
            )
            .expect("seed second administrator");
        (access, second_access)
    }

    #[test]
    fn expired_lockout_restarts_the_failed_login_count() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let password = BrowserUserId::new_v7().to_string();
        let wrong_password = BrowserUserId::new_v7().to_string();
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("testadmin").expect("username"),
                BrowserPassword::try_new(&password).expect("password"),
            )
            .expect("seed user");
        for _ in 0..MAX_FAILED_LOGINS {
            assert!(login(&kernel, "testadmin", &wrong_password).is_err());
        }

        let connection = Connection::open(kernel.database_path()).expect("database");
        connection
            .execute(
                "UPDATE browser_users SET locked_until = '2020-01-01T00:00:00Z' WHERE username = 'testadmin'",
                [],
            )
            .expect("expire lockout");

        assert!(login(&kernel, "testadmin", &wrong_password).is_err());
        let (failed_count, locked_until): (i64, Option<String>) = connection
            .query_row(
                "SELECT failed_login_count, locked_until FROM browser_users WHERE username = 'testadmin'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read failed login state");
        assert_eq!(failed_count, 1);
        assert_eq!(locked_until, None);
    }

    #[test]
    fn browser_user_administration_is_workspace_scoped() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let password = BrowserUserId::new_v7().to_string();
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("testadmin").expect("username"),
                BrowserPassword::try_new(&password).expect("password"),
            )
            .expect("seed user");
        let login = login(&kernel, "testadmin", &password).expect("login");
        let session = login.session().expose_hex();
        let csrf = login.csrf().expose_hex();
        let foreign_workspace = WorkspaceId::new_v7();
        let foreign_profile = ProfileId::new_v7();
        let foreign_client = ClientId::new_v7();
        let foreign_user = BrowserUserId::new_v7();
        let connection = Connection::open(kernel.database_path()).expect("database");
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, '2026-08-28T00:00:00Z')",
                [foreign_workspace.to_string()],
            )
            .expect("seed foreign workspace");
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, '2026-08-28T00:00:00Z')",
                params![foreign_profile.to_string(), foreign_workspace.to_string()],
            )
            .expect("seed foreign profile");
        connection
            .execute(
                r#"
                INSERT INTO clients(
                    client_id, workspace_id, status, current_credential_epoch, created_at
                ) VALUES (?1, ?2, 'active', 0, '2026-08-28T00:00:00Z')
                "#,
                params![foreign_client.to_string(), foreign_workspace.to_string()],
            )
            .expect("seed foreign client");
        connection
            .execute(
                r#"
                INSERT INTO browser_users(
                    user_id, username, password_hash, client_id, profile_id,
                    is_admin, is_test_account, active, created_at, updated_at
                )
                SELECT ?1, 'foreignadmin', password_hash, ?2, ?3,
                       1, 0, 1, created_at, updated_at
                FROM browser_users WHERE username = 'testadmin'
                "#,
                params![
                    foreign_user.to_string(),
                    foreign_client.to_string(),
                    foreign_profile.to_string()
                ],
            )
            .expect("seed foreign browser user");
        drop(connection);

        let users = kernel
            .list_browser_users(ListBrowserUsersQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
            ))
            .expect("list users");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].username(), "testadmin");

        let update = kernel
            .update_browser_user(
                UpdateBrowserUserCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
                    fasti_application::SecretMaterial::try_from_hex(&csrf).expect("csrf"),
                    foreign_user,
                    BrowserPassword::try_new(&password).expect("current password"),
                    Some(BrowserUsername::try_new("renamedforeign").expect("new username")),
                    None,
                    None,
                )
                .expect("update command"),
            )
            .expect_err("foreign update must fail");
        assert_eq!(update.code(), ProblemCode::ValidationFailed);

        let delete = kernel
            .delete_browser_user(DeleteBrowserUserCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
                fasti_application::SecretMaterial::try_from_hex(&csrf).expect("csrf"),
                foreign_user,
                BrowserPassword::try_new(&password).expect("current password"),
            ))
            .expect_err("foreign delete must fail");
        assert_eq!(delete.code(), ProblemCode::ValidationFailed);
        let connection = Connection::open(kernel.database_path()).expect("database");
        let username: String = connection
            .query_row(
                "SELECT username FROM browser_users WHERE user_id = ?1",
                [foreign_user.to_string()],
                |row| row.get(0),
            )
            .expect("foreign user survives");
        assert_eq!(username, "foreignadmin");
    }

    #[test]
    fn development_user_is_seeded_once_and_the_last_active_admin_is_retained() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("testadmin").expect("username"),
                BrowserPassword::try_new("testadmin").expect("password"),
            )
            .expect("seed user");
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("testadmin").expect("username"),
                BrowserPassword::try_new("testadmin").expect("password"),
            )
            .expect("idempotent seed");
        let login = kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    BrowserUsername::try_new("testadmin").expect("username"),
                    BrowserPassword::try_new("testadmin").expect("password"),
                    60,
                )
                .expect("command"),
            )
            .expect("login");
        let session =
            fasti_application::SecretMaterial::try_from_hex(&login.session().expose_hex())
                .expect("session");
        let csrf = fasti_application::SecretMaterial::try_from_hex(&login.csrf().expose_hex())
            .expect("csrf");
        let user_id = login.user().user_id();
        let updated = kernel
            .update_browser_user(
                UpdateBrowserUserCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    session,
                    csrf,
                    user_id,
                    BrowserPassword::try_new("testadmin").expect("current password"),
                    Some(BrowserUsername::try_new("editedadmin").expect("new username")),
                    None,
                    None,
                )
                .expect("update command"),
            )
            .expect("update user");
        assert_eq!(updated.username(), "editedadmin");
        assert!(kernel
            .authenticate_browser_session(AuthenticateBrowserSessionQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                CapabilityKey::ReadBrowserSession,
                fasti_application::SecretMaterial::try_from_hex(&login.session().expose_hex())
                    .expect("prior session"),
                None,
                false,
            ))
            .is_err());
        let login = kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    BrowserUsername::try_new("editedadmin").expect("username"),
                    BrowserPassword::try_new("testadmin").expect("password"),
                    60,
                )
                .expect("command"),
            )
            .expect("login after username edit");
        let session =
            fasti_application::SecretMaterial::try_from_hex(&login.session().expose_hex())
                .expect("session");
        let csrf = fasti_application::SecretMaterial::try_from_hex(&login.csrf().expose_hex())
            .expect("csrf");
        kernel
            .update_browser_user(
                UpdateBrowserUserCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    session,
                    csrf,
                    user_id,
                    BrowserPassword::try_new("testadmin").expect("current password"),
                    None,
                    Some(BrowserPassword::try_new("editedadmin").expect("new password")),
                    None,
                )
                .expect("password update command"),
            )
            .expect("update password");
        let login = kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    BrowserUsername::try_new("editedadmin").expect("username"),
                    BrowserPassword::try_new("editedadmin").expect("password"),
                    60,
                )
                .expect("command"),
            )
            .expect("login after password edit");
        let session = login.session().expose_hex();
        let csrf = login.csrf().expose_hex();
        let other_user_id = BrowserUserId::new_v7();
        let connection = Connection::open(kernel.database_path()).expect("database");
        connection
            .execute(
                r#"
                INSERT INTO browser_users(
                    user_id, username, password_hash, client_id, profile_id,
                    is_admin, is_test_account, active, created_at, updated_at
                )
                SELECT ?1, 'otheradmin', password_hash, client_id, profile_id,
                       1, 0, 1, created_at, updated_at
                FROM browser_users WHERE user_id = ?2
                "#,
                params![other_user_id.to_string(), user_id.to_string()],
            )
            .expect("seed second administrator");
        drop(connection);
        assert!(!kernel
            .delete_browser_user(DeleteBrowserUserCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
                fasti_application::SecretMaterial::try_from_hex(&csrf).expect("csrf"),
                other_user_id,
                BrowserPassword::try_new("editedadmin").expect("password"),
            ))
            .expect("delete another user"));
        let delete_error = kernel
            .delete_browser_user(DeleteBrowserUserCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
                fasti_application::SecretMaterial::try_from_hex(&csrf).expect("csrf"),
                user_id,
                BrowserPassword::try_new("editedadmin").expect("password"),
            ))
            .expect_err("last active administrator delete must fail");
        assert_eq!(delete_error.code(), ProblemCode::ValidationFailed);
        assert_eq!(
            delete_error.violations()[0].code(),
            "last_active_administrator_required"
        );
        assert_eq!(delete_error.violations()[0].pointer(), "/");
        let deactivate_error = kernel
            .update_browser_user(
                UpdateBrowserUserCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
                    fasti_application::SecretMaterial::try_from_hex(&csrf).expect("csrf"),
                    user_id,
                    BrowserPassword::try_new("editedadmin").expect("password"),
                    None,
                    None,
                    Some(false),
                )
                .expect("deactivation command"),
            )
            .expect_err("last active administrator deactivation must fail");
        assert_eq!(deactivate_error.code(), ProblemCode::ValidationFailed);
        assert_eq!(
            deactivate_error.violations()[0].code(),
            "last_active_administrator_required"
        );
        assert_eq!(deactivate_error.violations()[0].pointer(), "/");
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("testadmin").expect("username"),
                BrowserPassword::try_new("testadmin").expect("password"),
            )
            .expect("marker prevents recreation");
        assert!(kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    BrowserUsername::try_new("editedadmin").expect("username"),
                    BrowserPassword::try_new("editedadmin").expect("password"),
                    60,
                )
                .expect("command")
            )
            .is_ok());
        assert!(kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    BrowserUsername::try_new("testadmin").expect("username"),
                    BrowserPassword::try_new("testadmin").expect("password"),
                    60,
                )
                .expect("command")
            )
            .is_err());
    }

    #[test]
    fn concurrent_admin_deletions_retain_one_active_administrator() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let password = BrowserUserId::new_v7().to_string();
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("firstadmin").expect("username"),
                BrowserPassword::try_new(&password).expect("password"),
            )
            .expect("seed first administrator");
        let first = login(&kernel, "firstadmin", &password).expect("first login");
        let first_user_id = first.user().user_id();
        let second_user_id = BrowserUserId::new_v7();
        let connection = Connection::open(kernel.database_path()).expect("database");
        connection
            .execute(
                r#"
                INSERT INTO browser_users(
                    user_id, username, password_hash, client_id, profile_id,
                    is_admin, is_test_account, active, created_at, updated_at
                )
                SELECT ?1, 'secondadmin', password_hash, client_id, profile_id,
                       1, 0, 1, created_at, updated_at
                FROM browser_users WHERE user_id = ?2
                "#,
                params![second_user_id.to_string(), first_user_id.to_string()],
            )
            .expect("seed second administrator");
        drop(connection);
        let second = login(&kernel, "secondadmin", &password).expect("second login");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

        let spawn_delete = |kernel: SqliteKernel,
                            login: CreatedBrowserSession,
                            user_id: BrowserUserId| {
            let barrier = barrier.clone();
            let password = password.clone();
            std::thread::spawn(move || {
                barrier.wait();
                match kernel.delete_browser_user(DeleteBrowserUserCommand::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    fasti_application::SecretMaterial::try_from_hex(&login.session().expose_hex())
                        .expect("session"),
                    fasti_application::SecretMaterial::try_from_hex(&login.csrf().expose_hex())
                        .expect("csrf"),
                    user_id,
                    BrowserPassword::try_new(&password).expect("password"),
                )) {
                    Ok(true) => "deleted",
                    Err(problem)
                        if problem.code() == ProblemCode::ValidationFailed
                            && problem.violations().iter().any(|violation| {
                                violation.code() == "last_active_administrator_required"
                            }) =>
                    {
                        "retained"
                    }
                    outcome => panic!("unexpected concurrent deletion outcome: {outcome:?}"),
                }
            })
        };

        let first_delete = spawn_delete(kernel.clone(), first, first_user_id);
        let second_delete = spawn_delete(kernel.clone(), second, second_user_id);
        barrier.wait();
        let outcomes = [
            first_delete.join().expect("first deletion thread"),
            second_delete.join().expect("second deletion thread"),
        ];
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == "deleted")
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| **outcome == "retained")
                .count(),
            1
        );

        let connection = Connection::open(kernel.database_path()).expect("database");
        let active_administrators: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM browser_users WHERE is_admin = 1 AND active = 1",
                [],
                |row| row.get(0),
            )
            .expect("count active administrators");
        assert_eq!(active_administrators, 1);
    }

    #[test]
    fn revoked_administrator_access_cannot_satisfy_the_continuity_guard() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let password = BrowserUserId::new_v7().to_string();
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("firstadmin").expect("username"),
                BrowserPassword::try_new(&password).expect("password"),
            )
            .expect("seed first administrator");
        let first = login(&kernel, "firstadmin", &password).expect("first login");
        let (access, second_access) = add_second_administrator(&kernel, &first);
        kernel
            .revoke_client_credential(RevokeClientCredentialCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                access,
                second_access.credential_id(),
            ))
            .expect("revoke second administrator access");

        let session = first.session().expose_hex();
        let csrf = first.csrf().expose_hex();
        let first_user_id = first.user().user_id();
        let delete_error = kernel
            .delete_browser_user(DeleteBrowserUserCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
                fasti_application::SecretMaterial::try_from_hex(&csrf).expect("csrf"),
                first_user_id,
                BrowserPassword::try_new(&password).expect("password"),
            ))
            .expect_err("revoked administrator cannot permit the last usable admin deletion");
        assert!(delete_error
            .violations()
            .iter()
            .any(|violation| { violation.code() == "last_active_administrator_required" }));
        let deactivate_error = kernel
            .update_browser_user(
                UpdateBrowserUserCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    fasti_application::SecretMaterial::try_from_hex(&session).expect("session"),
                    fasti_application::SecretMaterial::try_from_hex(&csrf).expect("csrf"),
                    first_user_id,
                    BrowserPassword::try_new(&password).expect("password"),
                    None,
                    None,
                    Some(false),
                )
                .expect("deactivation command"),
            )
            .expect_err("revoked administrator cannot permit the last usable admin deactivation");
        assert!(deactivate_error
            .violations()
            .iter()
            .any(|violation| { violation.code() == "last_active_administrator_required" }));
    }

    #[test]
    fn removing_one_administrator_cannot_enable_revoking_the_last_other_administrator() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let password = BrowserUserId::new_v7().to_string();
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("firstadmin").expect("username"),
                BrowserPassword::try_new(&password).expect("password"),
            )
            .expect("seed first administrator");
        let first = login(&kernel, "firstadmin", &password).expect("first login");
        let (access, second_access) = add_second_administrator(&kernel, &first);

        assert!(kernel
            .delete_browser_user(DeleteBrowserUserCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                fasti_application::SecretMaterial::try_from_hex(&first.session().expose_hex())
                    .expect("session"),
                fasti_application::SecretMaterial::try_from_hex(&first.csrf().expose_hex())
                    .expect("csrf"),
                first.user().user_id(),
                BrowserPassword::try_new(&password).expect("password"),
            ))
            .expect("remove first administrator"));
        let revoke_error = kernel
            .revoke_client_credential(RevokeClientCredentialCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                access,
                second_access.credential_id(),
            ))
            .expect_err("the remaining administrator access must not be revoked");
        assert_eq!(revoke_error.code(), ProblemCode::Forbidden);
        assert!(login(&kernel, "secondadmin", &password).is_ok());
    }

    #[test]
    fn ordinary_credential_can_be_revoked_without_browser_administrators() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
        let initialized = kernel
            .initialize_node(InitializeNodeCommand::new(correlation_id))
            .expect("initialize node");
        let proof = fasti_application::SecretMaterial::try_from_hex(
            &initialized.initialization_proof().expose_hex(),
        )
        .expect("initialization proof");
        let access = *kernel
            .enroll_first_client(EnrollFirstClientCommand::new(correlation_id, proof))
            .expect("enroll first client")
            .access();
        let ordinary_access = kernel
            .create_scoped_client_credential(CreateScopedClientCredentialCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                access,
                vec![ScopeKey::ObservationAccept],
            ))
            .expect("create ordinary scoped access");

        kernel
            .revoke_client_credential(RevokeClientCredentialCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                access,
                ordinary_access.credential_id(),
            ))
            .expect("revoke ordinary scoped access without browser administrators");
    }

    #[test]
    fn session_management_lists_and_terminates_sessions() {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        kernel
            .ensure_development_browser_user(
                BrowserUsername::try_new("testadmin").expect("username"),
                BrowserPassword::try_new("testadmin").expect("password"),
            )
            .expect("seed user");

        // Login session 1: 30 days timeout (43200 minutes)
        let login1 = kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    BrowserUsername::try_new("testadmin").expect("username"),
                    BrowserPassword::try_new("testadmin").expect("password"),
                    43200,
                )
                .expect("30 day session"),
            )
            .expect("login 1");

        // Login session 2: 60 days timeout (86400 minutes)
        let _login2 = kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    BrowserUsername::try_new("testadmin").expect("username"),
                    BrowserPassword::try_new("testadmin").expect("password"),
                    86400,
                )
                .expect("60 day session"),
            )
            .expect("login 2");

        let session1 =
            fasti_application::SecretMaterial::try_from_hex(&login1.session().expose_hex())
                .expect("session1");
        let csrf1 = fasti_application::SecretMaterial::try_from_hex(&login1.csrf().expose_hex())
            .expect("csrf1");

        // List sessions from session 1 perspective
        let sessions = kernel
            .list_browser_sessions(ListBrowserSessionsQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                session1,
            ))
            .expect("list sessions");

        assert_eq!(sessions.len(), 2);
        let curr_count = sessions.iter().filter(|s| s.is_current()).count();
        assert_eq!(curr_count, 1);

        // Terminate other session
        let other_session = sessions
            .iter()
            .find(|s| !s.is_current())
            .expect("other session");
        let session1_again =
            fasti_application::SecretMaterial::try_from_hex(&login1.session().expose_hex())
                .expect("session1_again");
        let terminated = kernel
            .end_specific_browser_session(EndSpecificBrowserSessionCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                session1_again,
                csrf1,
                other_session.session_id().to_string(),
            ))
            .expect("end specific session");
        assert!(terminated);

        let session1_check =
            fasti_application::SecretMaterial::try_from_hex(&login1.session().expose_hex())
                .expect("session1_check");
        let sessions_after = kernel
            .list_browser_sessions(ListBrowserSessionsQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                session1_check,
            ))
            .expect("list sessions after");
        assert_eq!(sessions_after.len(), 1);
    }
}
