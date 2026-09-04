use crate::kernel::{
    digest_secret, map_sql, parse_timestamp, random_secret, timestamp, verify_digest, SqliteKernel,
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use fasti_application::{
    ApplicationResult, AuthenticatedBrowserSession, BrowserSessionInventory,
    BrowserSessionMutationCommand, BrowserSessionPort, BrowserSessionQuery, BrowserSessionSummary,
    CapabilityKey, CreateAuthSubjectCommand, CreateBrowserSessionCommand, CreatedBrowserSession,
    FastiProblem, ProblemCode, RevokeBrowserSessionOutcome, SelectBrowserSessionProfileCommand,
    SessionPolicy, TargetBrowserSessionCommand,
};
use fasti_domain::{
    AuthSubject, AuthSubjectId, AuthSubjectLifecycle, BrowserSessionId, BrowserSessionState,
    FastiBrowserSession, ProfileGrantId, WorkspaceId,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::time::Duration;

type SubjectRow = (String, String, i64, i64, String, String);
type SessionRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    i64,
    i64,
    i64,
    i64,
    i64,
);

fn problem(
    code: ProblemCode,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(code, capability, correlation_id))
}

fn checked_duration(
    duration: Duration,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<ChronoDuration> {
    ChronoDuration::from_std(duration)
        .map_err(|_| problem(ProblemCode::ValidationFailed, capability, correlation_id))
}

fn duration_seconds(
    duration: Duration,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<i64> {
    i64::try_from(duration.as_secs())
        .map_err(|_| problem(ProblemCode::ValidationFailed, capability, correlation_id))
}

fn checked_add(
    at: DateTime<Utc>,
    duration: Duration,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<DateTime<Utc>> {
    at.checked_add_signed(checked_duration(duration, capability, correlation_id)?)
        .ok_or_else(|| problem(ProblemCode::ValidationFailed, capability, correlation_id))
}

fn parse_subject(
    row: SubjectRow,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<AuthSubject> {
    let (id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at) = row;
    AuthSubject::try_new(
        id.parse::<AuthSubjectId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        AuthSubjectLifecycle::from_storage(&lifecycle)
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        u64::try_from(auth_epoch)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        u64::try_from(authorization_epoch)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        parse_timestamp(&created_at, capability, correlation_id)?,
        parse_timestamp(&updated_at, capability, correlation_id)?,
    )
    .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))
}

fn parse_session(
    row: &SessionRow,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<FastiBrowserSession> {
    FastiBrowserSession::try_new(
        row.0
            .parse::<BrowserSessionId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        row.1
            .parse::<AuthSubjectId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        row.2
            .parse::<WorkspaceId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        row.3
            .parse::<ProfileGrantId>()
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        parse_timestamp(&row.4, capability, correlation_id)?,
        parse_timestamp(&row.5, capability, correlation_id)?,
        parse_timestamp(&row.6, capability, correlation_id)?,
        parse_timestamp(&row.7, capability, correlation_id)?,
        row.8
            .as_deref()
            .map(|value| parse_timestamp(value, capability, correlation_id))
            .transpose()?,
        u64::try_from(row.11)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        u64::try_from(row.12)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        u64::try_from(row.13)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
    )
    .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))
}

fn session_problem(state: BrowserSessionState) -> ProblemCode {
    match state {
        BrowserSessionState::IdleExpired | BrowserSessionState::AbsoluteExpired => {
            ProblemCode::BrowserSessionExpired
        }
        BrowserSessionState::Revoked => ProblemCode::BrowserSessionRevoked,
        BrowserSessionState::SubjectInactive
        | BrowserSessionState::SubjectMismatch
        | BrowserSessionState::PolicyChanged => ProblemCode::SessionPolicyChanged,
        BrowserSessionState::Active => unreachable!("active sessions do not have an error"),
    }
}

fn query_subject(
    connection: &Connection,
    subject_id: AuthSubjectId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<AuthSubject> {
    let row = map_sql(
        connection.query_row(
            "SELECT auth_subject_id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at FROM auth_subjects WHERE auth_subject_id = ?1",
            [subject_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        ),
        capability,
        correlation_id,
    )?;
    parse_subject(row, capability, correlation_id)
}

fn query_session_row(
    connection: &Connection,
    session_digest: &str,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Option<(SessionRow, String)>> {
    map_sql(
        connection
            .query_row(
                r#"
                SELECT browser_session_id, auth_subject_id, workspace_id,
                       selected_profile_grant_id, created_at, last_seen_at,
                       idle_expires_at, absolute_expires_at, revoked_at,
                       idle_timeout_seconds, last_seen_write_interval_seconds,
                       auth_epoch, authorization_epoch, rotation_generation,
                       csrf_digest
                FROM fasti_browser_sessions
                WHERE session_digest = ?1
                "#,
                [session_digest],
                |row| {
                    Ok((
                        (
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                            row.get(7)?,
                            row.get(8)?,
                            row.get(9)?,
                            row.get(10)?,
                            row.get(11)?,
                            row.get(12)?,
                            row.get(13)?,
                        ),
                        row.get(14)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )
}

pub(crate) fn authenticate_session(
    connection: &Connection,
    session_secret: &fasti_application::SecretMaterial,
    csrf_secret: Option<&fasti_application::SecretMaterial>,
    at: DateTime<Utc>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<AuthenticatedBrowserSession> {
    let digest = digest_secret(session_secret);
    let Some((mut row, csrf_digest)) =
        query_session_row(connection, &digest, capability, correlation_id)?
    else {
        return Err(problem(
            ProblemCode::BrowserSessionRevoked,
            capability,
            correlation_id,
        ));
    };
    if let Some(csrf) = csrf_secret {
        if !verify_digest(&csrf_digest, &digest_secret(csrf)) {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
    }
    let subject_id = row
        .1
        .parse::<AuthSubjectId>()
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    let subject = query_subject(connection, subject_id, capability, correlation_id)?;
    let mut session = parse_session(&row, capability, correlation_id)?;
    let state = session.state(&subject, at);
    if !matches!(state, BrowserSessionState::Active) {
        if matches!(
            state,
            BrowserSessionState::IdleExpired | BrowserSessionState::AbsoluteExpired
        ) {
            let _ = connection.execute(
                "UPDATE fasti_browser_sessions SET revoked_at = COALESCE(revoked_at, ?1) WHERE browser_session_id = ?2",
                params![timestamp(at), session.id().to_string()],
            );
        }
        return Err(problem(session_problem(state), capability, correlation_id));
    }
    validate_active_membership(
        connection,
        subject.id(),
        session.workspace_id(),
        ProblemCode::SessionPolicyChanged,
        capability,
        correlation_id,
    )?;
    validate_grants(
        connection,
        subject.id(),
        session.workspace_id(),
        &[session.selected_profile_grant_id()],
        ProblemCode::SessionPolicyChanged,
        capability,
        correlation_id,
    )?;

    let write_interval = ChronoDuration::seconds(row.10);
    if at >= session.last_seen_at() + write_interval {
        let absolute = session.absolute_expires_at();
        let idle = (at + ChronoDuration::seconds(row.9)).min(absolute);
        map_sql(
            connection.execute(
                "UPDATE fasti_browser_sessions SET last_seen_at = ?1, idle_expires_at = ?2 WHERE browser_session_id = ?3 AND revoked_at IS NULL",
                params![timestamp(at), timestamp(idle), session.id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        row.5 = timestamp(at);
        row.6 = timestamp(idle);
        session = parse_session(&row, capability, correlation_id)?;
    }
    Ok(AuthenticatedBrowserSession::new(subject, session))
}

fn validate_active_membership(
    connection: &Connection,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    invalid_code: ProblemCode,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    let exists = map_sql(
        connection.query_row(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM workspace_memberships
                WHERE auth_subject_id = ?1
                  AND workspace_id = ?2
                  AND lifecycle = 'active'
            )
            "#,
            params![subject_id.to_string(), workspace_id.to_string()],
            |row| row.get::<_, i64>(0),
        ),
        capability,
        correlation_id,
    )?;
    if exists != 1 {
        return Err(problem(invalid_code, capability, correlation_id));
    }
    Ok(())
}

fn validate_grants(
    connection: &Connection,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    grants: &[ProfileGrantId],
    invalid_code: ProblemCode,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<()> {
    for grant_id in grants {
        let exists = map_sql(
            connection.query_row(
                r#"
                SELECT EXISTS(
                    SELECT 1
                    FROM auth_subject_profile_grants subject_grant
                    JOIN profile_grants grant
                      ON grant.grant_id = subject_grant.profile_grant_id
                    JOIN clients client ON client.client_id = grant.client_id
                    WHERE subject_grant.auth_subject_id = ?1
                      AND grant.grant_id = ?2
                      AND grant.workspace_id = ?3
                      AND grant.status = 'active'
                      AND client.status = 'active'
                )
                "#,
                params![
                    subject_id.to_string(),
                    grant_id.to_string(),
                    workspace_id.to_string()
                ],
                |row| row.get::<_, i64>(0),
            ),
            capability,
            correlation_id,
        )?;
        if exists != 1 {
            return Err(problem(invalid_code, capability, correlation_id));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_session(
    transaction: &Transaction<'_>,
    subject: AuthSubject,
    workspace_id: WorkspaceId,
    grants: &[ProfileGrantId],
    selected_grant_id: ProfileGrantId,
    policy: SessionPolicy,
    remembered: bool,
    at: DateTime<Utc>,
    rotation_generation: u64,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<CreatedBrowserSession> {
    validate_active_membership(
        transaction,
        subject.id(),
        workspace_id,
        ProblemCode::Forbidden,
        capability,
        correlation_id,
    )?;
    validate_grants(
        transaction,
        subject.id(),
        workspace_id,
        grants,
        ProblemCode::Forbidden,
        capability,
        correlation_id,
    )?;
    let session_secret = random_secret(capability, correlation_id)?;
    let csrf_secret = random_secret(capability, correlation_id)?;
    let session_id = BrowserSessionId::new_v7();
    let absolute_expires_at = checked_add(
        at,
        policy.absolute_lifetime(remembered),
        capability,
        correlation_id,
    )?;
    let idle_expires_at = checked_add(
        at,
        policy.browser_idle_timeout(),
        capability,
        correlation_id,
    )?
    .min(absolute_expires_at);
    let session = FastiBrowserSession::try_new(
        session_id,
        subject.id(),
        workspace_id,
        selected_grant_id,
        at,
        at,
        idle_expires_at,
        absolute_expires_at,
        None,
        subject.auth_epoch(),
        subject.authorization_epoch(),
        rotation_generation,
    )
    .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    map_sql(
        transaction.execute(
            r#"
            INSERT INTO fasti_browser_sessions(
                browser_session_id, session_digest, csrf_digest, auth_subject_id,
                workspace_id, selected_profile_grant_id, created_at, last_seen_at,
                idle_expires_at, absolute_expires_at, idle_timeout_seconds,
                last_seen_write_interval_seconds, revoked_at, auth_epoch,
                authorization_epoch, rotation_generation
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9, ?10, ?11, NULL, ?12, ?13, ?14)
            "#,
            params![
                session.id().to_string(),
                digest_secret(&session_secret),
                digest_secret(&csrf_secret),
                subject.id().to_string(),
                workspace_id.to_string(),
                selected_grant_id.to_string(),
                timestamp(at),
                timestamp(idle_expires_at),
                timestamp(absolute_expires_at),
                duration_seconds(policy.browser_idle_timeout(), capability, correlation_id)?,
                duration_seconds(
                    policy.last_seen_write_interval(),
                    capability,
                    correlation_id
                )?,
                i64::try_from(subject.auth_epoch()).map_err(|_| problem(
                    ProblemCode::ValidationFailed,
                    capability,
                    correlation_id
                ))?,
                i64::try_from(subject.authorization_epoch()).map_err(|_| problem(
                    ProblemCode::ValidationFailed,
                    capability,
                    correlation_id
                ))?,
                i64::try_from(rotation_generation).map_err(|_| problem(
                    ProblemCode::ValidationFailed,
                    capability,
                    correlation_id
                ))?,
            ],
        ),
        capability,
        correlation_id,
    )?;
    for grant_id in grants {
        map_sql(
            transaction.execute(
                "INSERT INTO fasti_browser_session_grants(browser_session_id, profile_grant_id) VALUES (?1, ?2)",
                params![session.id().to_string(), grant_id.to_string()],
            ),
            capability,
            correlation_id,
        )?;
    }
    Ok(CreatedBrowserSession::new(
        session,
        session_secret,
        csrf_secret,
    ))
}

fn session_grants(
    connection: &Connection,
    session_id: BrowserSessionId,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<Vec<ProfileGrantId>> {
    let mut statement = map_sql(
        connection.prepare(
            "SELECT profile_grant_id FROM fasti_browser_session_grants WHERE browser_session_id = ?1 ORDER BY profile_grant_id",
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([session_id.to_string()], |row| row.get::<_, String>(0)),
        capability,
        correlation_id,
    )?;
    let mut grants = Vec::new();
    for row in rows {
        let value = map_sql(row, capability, correlation_id)?;
        grants.push(
            value
                .parse::<ProfileGrantId>()
                .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        );
    }
    Ok(grants)
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
            SELECT COUNT(*)
            FROM workspace_memberships membership
            JOIN auth_subjects subject
              ON subject.auth_subject_id = membership.auth_subject_id
            WHERE membership.workspace_id = ?1
              AND membership.lifecycle = 'active'
              AND membership.role = 'administrator'
              AND subject.lifecycle = 'active'
            "#,
            [workspace_id],
            |row| row.get(0),
        ),
        capability,
        correlation_id,
    )
}

fn rotate_session(
    transaction: &Transaction<'_>,
    current: AuthenticatedBrowserSession,
    selected_grant_id: ProfileGrantId,
    at: DateTime<Utc>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> ApplicationResult<CreatedBrowserSession> {
    let current_session = current.session();
    let grants = session_grants(
        transaction,
        current_session.id(),
        capability,
        correlation_id,
    )?;
    validate_grants(
        transaction,
        current.subject().id(),
        current.session().workspace_id(),
        &grants,
        ProblemCode::Forbidden,
        capability,
        correlation_id,
    )?;
    if !grants.contains(&selected_grant_id) {
        return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
    }
    let (idle_timeout_seconds, write_interval_seconds): (i64, i64) = map_sql(
        transaction.query_row(
            "SELECT idle_timeout_seconds, last_seen_write_interval_seconds FROM fasti_browser_sessions WHERE browser_session_id = ?1",
            [current_session.id().to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ),
        capability,
        correlation_id,
    )?;
    let idle_expires_at = (at + ChronoDuration::seconds(idle_timeout_seconds))
        .min(current_session.absolute_expires_at());
    if idle_expires_at <= at {
        return Err(problem(
            ProblemCode::BrowserSessionExpired,
            capability,
            correlation_id,
        ));
    }
    let session_secret = random_secret(capability, correlation_id)?;
    let csrf_secret = random_secret(capability, correlation_id)?;
    let rotated = FastiBrowserSession::try_new(
        BrowserSessionId::new_v7(),
        current.subject().id(),
        current_session.workspace_id(),
        selected_grant_id,
        at,
        at,
        idle_expires_at,
        current_session.absolute_expires_at(),
        None,
        current.subject().auth_epoch(),
        current.subject().authorization_epoch(),
        current_session
            .rotation_generation()
            .checked_add(1)
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
    )
    .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    map_sql(
        transaction.execute(
            r#"
            INSERT INTO fasti_browser_sessions(
                browser_session_id, session_digest, csrf_digest, auth_subject_id,
                workspace_id, selected_profile_grant_id, created_at, last_seen_at,
                idle_expires_at, absolute_expires_at, idle_timeout_seconds,
                last_seen_write_interval_seconds, revoked_at, auth_epoch,
                authorization_epoch, rotation_generation
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?8, ?9, ?10, ?11, NULL, ?12, ?13, ?14)
            "#,
            params![
                rotated.id().to_string(),
                digest_secret(&session_secret),
                digest_secret(&csrf_secret),
                rotated.subject_id().to_string(),
                rotated.workspace_id().to_string(),
                rotated.selected_profile_grant_id().to_string(),
                timestamp(at),
                timestamp(idle_expires_at),
                timestamp(rotated.absolute_expires_at()),
                idle_timeout_seconds,
                write_interval_seconds,
                i64::try_from(rotated.auth_epoch()).map_err(|_| problem(
                    ProblemCode::IntegrityFailed,
                    capability,
                    correlation_id
                ))?,
                i64::try_from(rotated.authorization_epoch()).map_err(|_| problem(
                    ProblemCode::IntegrityFailed,
                    capability,
                    correlation_id
                ))?,
                i64::try_from(rotated.rotation_generation()).map_err(|_| problem(
                    ProblemCode::IntegrityFailed,
                    capability,
                    correlation_id
                ))?,
            ],
        ),
        capability,
        correlation_id,
    )?;
    for grant_id in grants {
        map_sql(
            transaction.execute(
                "INSERT INTO fasti_browser_session_grants(browser_session_id, profile_grant_id) VALUES (?1, ?2)",
                params![rotated.id().to_string(), grant_id.to_string()],
            ),
            capability,
            correlation_id,
        )?;
    }
    let copied_authentication = map_sql(
        transaction.execute(
            r#"
            INSERT INTO fasti_browser_session_authentication(
                browser_session_id, trailbase_instance_id, activation_generation,
                method, verified_at, recent_authentication_expires_at
            )
            SELECT ?1, trailbase_instance_id, activation_generation,
                   method, verified_at, recent_authentication_expires_at
            FROM fasti_browser_session_authentication
            WHERE browser_session_id = ?2
            "#,
            params![rotated.id().to_string(), current_session.id().to_string()],
        ),
        capability,
        correlation_id,
    )?;
    if copied_authentication != 1 {
        return Err(problem(
            ProblemCode::IntegrityFailed,
            capability,
            correlation_id,
        ));
    }
    map_sql(
        transaction.execute(
            "UPDATE fasti_browser_sessions SET revoked_at = ?1 WHERE browser_session_id = ?2 AND revoked_at IS NULL",
            params![timestamp(at), current_session.id().to_string()],
        ),
        capability,
        correlation_id,
    )?;
    Ok(CreatedBrowserSession::new(
        rotated,
        session_secret,
        csrf_secret,
    ))
}

impl BrowserSessionPort for SqliteKernel {
    fn create_auth_subject(&self, command: CreateAuthSubjectCommand) -> ApplicationResult<()> {
        let capability = CapabilityKey::CreateBrowserSession;
        let correlation_id = command.correlation_id();
        let subject = command.subject();
        let connection = self.lock_connection(capability, correlation_id)?;
        map_sql(
            connection.execute(
                "INSERT INTO auth_subjects(auth_subject_id, lifecycle, auth_epoch, authorization_epoch, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    subject.id().to_string(),
                    subject.lifecycle().as_str(),
                    i64::try_from(subject.auth_epoch()).map_err(|_| problem(ProblemCode::ValidationFailed, capability, correlation_id))?,
                    i64::try_from(subject.authorization_epoch()).map_err(|_| problem(ProblemCode::ValidationFailed, capability, correlation_id))?,
                    timestamp(subject.created_at()),
                    timestamp(subject.updated_at()),
                ],
            ),
            capability,
            correlation_id,
        )?;
        Ok(())
    }

    fn create_browser_session(
        &self,
        command: CreateBrowserSessionCommand,
    ) -> ApplicationResult<CreatedBrowserSession> {
        let capability = CapabilityKey::CreateBrowserSession;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let subject = query_subject(
            &transaction,
            command.subject_id(),
            capability,
            correlation_id,
        )?;
        if !matches!(subject.lifecycle(), AuthSubjectLifecycle::Active) {
            return Err(problem(ProblemCode::Forbidden, capability, correlation_id));
        }
        let created = insert_session(
            &transaction,
            subject,
            command.workspace_id(),
            command.authorized_profile_grants(),
            command.selected_profile_grant_id(),
            command.policy(),
            command.remembered(),
            command.now(),
            0,
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(created)
    }

    fn authenticate_browser_session(
        &self,
        query: BrowserSessionQuery,
    ) -> ApplicationResult<AuthenticatedBrowserSession> {
        let capability = CapabilityKey::ReadBrowserSession;
        let correlation_id = query.correlation_id();
        let connection = self.lock_connection(capability, correlation_id)?;
        authenticate_session(
            &connection,
            query.session_secret(),
            None,
            query.now(),
            capability,
            correlation_id,
        )
    }

    fn list_browser_sessions(
        &self,
        query: BrowserSessionQuery,
    ) -> ApplicationResult<BrowserSessionInventory> {
        let capability = CapabilityKey::ListBrowserSessions;
        let correlation_id = query.correlation_id();
        let connection = self.lock_connection(capability, correlation_id)?;
        let current = authenticate_session(
            &connection,
            query.session_secret(),
            None,
            query.now(),
            capability,
            correlation_id,
        )?;
        let mut statement = map_sql(
            connection.prepare(
                r#"
                SELECT browser_session_id, auth_subject_id, workspace_id,
                       selected_profile_grant_id, created_at, last_seen_at,
                       idle_expires_at, absolute_expires_at, revoked_at,
                       idle_timeout_seconds, last_seen_write_interval_seconds,
                       auth_epoch, authorization_epoch, rotation_generation
                FROM fasti_browser_sessions
                WHERE auth_subject_id = ?1 AND revoked_at IS NULL
                  AND idle_expires_at > ?2 AND absolute_expires_at > ?2
                ORDER BY created_at, browser_session_id
                LIMIT 33
                "#,
            ),
            capability,
            correlation_id,
        )?;
        let rows = map_sql(
            statement.query_map(
                params![current.subject().id().to_string(), timestamp(query.now())],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                        row.get(13)?,
                    ))
                },
            ),
            capability,
            correlation_id,
        )?;
        let mut sessions = Vec::new();
        for row in rows {
            let row = map_sql(row, capability, correlation_id)?;
            let session = parse_session(&row, capability, correlation_id)?;
            sessions.push(BrowserSessionSummary::new(
                session,
                session.id() == current.session().id(),
            ));
        }
        let truncated = sessions.len() > 32;
        sessions.truncate(32);
        Ok(BrowserSessionInventory::new(sessions, truncated))
    }

    fn revoke_current_browser_session(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<bool> {
        let capability = CapabilityKey::EndBrowserSession;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let current = authenticate_session(
            &transaction,
            command.session_secret(),
            Some(command.csrf_secret()),
            command.now(),
            capability,
            correlation_id,
        )?;
        let changed = map_sql(
            transaction.execute(
                "UPDATE fasti_browser_sessions SET revoked_at = ?1 WHERE browser_session_id = ?2 AND revoked_at IS NULL",
                params![timestamp(command.now()), current.session().id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(changed == 1)
    }

    fn revoke_browser_session(
        &self,
        command: TargetBrowserSessionCommand,
    ) -> ApplicationResult<RevokeBrowserSessionOutcome> {
        let capability = CapabilityKey::RevokeBrowserSession;
        let correlation_id = command.proof().correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let current = authenticate_session(
            &transaction,
            command.proof().session_secret(),
            Some(command.proof().csrf_secret()),
            command.proof().now(),
            capability,
            correlation_id,
        )?;
        let changed = map_sql(
            transaction.execute(
                "UPDATE fasti_browser_sessions SET revoked_at = ?1 WHERE browser_session_id = ?2 AND auth_subject_id = ?3 AND revoked_at IS NULL",
                params![
                    timestamp(command.proof().now()),
                    command.target_session_id().to_string(),
                    current.subject().id().to_string(),
                ],
            ),
            capability,
            correlation_id,
        )?;
        let revoked = changed == 1;
        let current_session_revoked =
            revoked && current.session().id() == command.target_session_id();
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(RevokeBrowserSessionOutcome::new(
            revoked,
            current_session_revoked,
        ))
    }

    fn revoke_other_browser_sessions(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<u64> {
        let capability = CapabilityKey::RevokeOtherBrowserSessions;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let current = authenticate_session(
            &transaction,
            command.session_secret(),
            Some(command.csrf_secret()),
            command.now(),
            capability,
            correlation_id,
        )?;
        let changed = map_sql(
            transaction.execute(
                "UPDATE fasti_browser_sessions SET revoked_at = ?1 WHERE auth_subject_id = ?2 AND browser_session_id <> ?3 AND revoked_at IS NULL",
                params![
                    timestamp(command.now()),
                    current.subject().id().to_string(),
                    current.session().id().to_string(),
                ],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        u64::try_from(changed)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))
    }

    fn revoke_all_browser_sessions(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<u64> {
        let capability = CapabilityKey::RevokeAllBrowserSessions;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let current = authenticate_session(
            &transaction,
            command.session_secret(),
            Some(command.csrf_secret()),
            command.now(),
            capability,
            correlation_id,
        )?;
        let changed = map_sql(
            transaction.execute(
                "UPDATE fasti_browser_sessions SET revoked_at = ?1 WHERE auth_subject_id = ?2 AND revoked_at IS NULL",
                params![timestamp(command.now()), current.subject().id().to_string()],
            ),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        u64::try_from(changed)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))
    }

    fn rotate_browser_session(
        &self,
        command: BrowserSessionMutationCommand,
    ) -> ApplicationResult<CreatedBrowserSession> {
        let capability = CapabilityKey::RotateBrowserSession;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let current = authenticate_session(
            &transaction,
            command.session_secret(),
            Some(command.csrf_secret()),
            command.now(),
            capability,
            correlation_id,
        )?;
        let rotated = rotate_session(
            &transaction,
            current,
            current.session().selected_profile_grant_id(),
            command.now(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(rotated)
    }

    fn select_browser_session_profile(
        &self,
        command: SelectBrowserSessionProfileCommand,
    ) -> ApplicationResult<CreatedBrowserSession> {
        let capability = CapabilityKey::SelectBrowserSessionProfile;
        let correlation_id = command.proof().correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let current = authenticate_session(
            &transaction,
            command.proof().session_secret(),
            Some(command.proof().csrf_secret()),
            command.proof().now(),
            capability,
            correlation_id,
        )?;
        let rotated = rotate_session(
            &transaction,
            current,
            command.target_profile_grant_id(),
            command.proof().now(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(rotated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use fasti_application::SecretMaterial;
    use fasti_domain::{ClientId, ProfileId, TrailBaseInstanceId};

    struct Fixture {
        _root: tempfile::TempDir,
        kernel: SqliteKernel,
        subject_id: AuthSubjectId,
        workspace_id: WorkspaceId,
        instance_id: TrailBaseInstanceId,
        grants: [ProfileGrantId; 3],
        created_at: DateTime<Utc>,
    }

    fn secret_copy(secret: &SecretMaterial) -> SecretMaterial {
        SecretMaterial::try_from_hex(&secret.expose_hex()).expect("copy test secret")
    }

    fn mutation_command(
        correlation_id: fasti_domain::RequestCorrelationId,
        session_secret: SecretMaterial,
        csrf_secret: SecretMaterial,
        now: DateTime<Utc>,
    ) -> BrowserSessionMutationCommand {
        let boundary = fasti_application::BrowserRequestBoundaryPolicy::try_new(
            "https://fasti.example",
            "fasti.example",
        )
        .expect("valid boundary policy")
        .validate(Some("https://fasti.example"), Some("fasti.example"))
        .expect("matching request boundary");
        BrowserSessionMutationCommand::new(
            correlation_id,
            session_secret,
            csrf_secret,
            boundary,
            now,
        )
    }

    fn policy() -> SessionPolicy {
        SessionPolicy::try_new(
            Duration::from_secs(30),
            Duration::from_secs(120),
            Duration::from_secs(240),
            Duration::from_secs(10),
        )
        .expect("deterministic policy")
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let workspace_id = WorkspaceId::new_v7();
        let instance_id = TrailBaseInstanceId::new_v7();
        let client_id = ClientId::new_v7();
        let profiles = [
            ProfileId::new_v7(),
            ProfileId::new_v7(),
            ProfileId::new_v7(),
        ];
        let grants = [
            ProfileGrantId::new_v7(),
            ProfileGrantId::new_v7(),
            ProfileGrantId::new_v7(),
        ];
        let created_at = Utc.timestamp_opt(1_700_000_000, 0).single().expect("time");
        {
            let connection = kernel
                .lock_connection(
                    CapabilityKey::CreateBrowserSession,
                    fasti_domain::RequestCorrelationId::new_v7(),
                )
                .expect("connection");
            connection
                .execute(
                    "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                    params![workspace_id.to_string(), timestamp(created_at)],
                )
                .expect("workspace");
            connection
                .execute(
                    "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 1, ?3)",
                    params![client_id.to_string(), workspace_id.to_string(), timestamp(created_at)],
                )
                .expect("client");
            connection
                .execute(
                    r#"
                    INSERT INTO trailbase_installation(
                        singleton, trailbase_instance_id, physical_root_identity,
                        release_lock_identity, activation_state, activation_blocker,
                        activation_generation, created_at, updated_at
                    ) VALUES (1, ?1, ?2, ?3, 'active', NULL, 1, ?4, ?4)
                    "#,
                    params![
                        instance_id.to_string(),
                        format!("sha256:{}", "0".repeat(64)),
                        format!("sha256:{}", "1".repeat(64)),
                        timestamp(created_at),
                    ],
                )
                .expect("TrailBase installation");
            for (profile_id, grant_id) in profiles.into_iter().zip(grants) {
                connection
                    .execute(
                        "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                        params![profile_id.to_string(), workspace_id.to_string(), timestamp(created_at)],
                    )
                    .expect("profile");
                connection
                    .execute(
                        "INSERT INTO profile_grants(grant_id, workspace_id, profile_id, client_id, status, created_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5)",
                        params![grant_id.to_string(), workspace_id.to_string(), profile_id.to_string(), client_id.to_string(), timestamp(created_at)],
                    )
                    .expect("grant");
            }
        }
        let subject_id = AuthSubjectId::new_v7();
        kernel
            .create_auth_subject(CreateAuthSubjectCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                AuthSubject::try_new(
                    subject_id,
                    AuthSubjectLifecycle::Active,
                    1,
                    1,
                    created_at,
                    created_at,
                )
                .expect("valid subject"),
            ))
            .expect("subject");
        {
            let connection = kernel
                .lock_connection(
                    CapabilityKey::CreateBrowserSession,
                    fasti_domain::RequestCorrelationId::new_v7(),
                )
                .expect("connection");
            for grant_id in grants {
                connection
                    .execute(
                        "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                        params![subject_id.to_string(), grant_id.to_string()],
                    )
                    .expect("subject profile grant");
            }
            connection
                .execute(
                    "INSERT INTO workspace_memberships(membership_id, auth_subject_id, workspace_id, lifecycle, role, created_at, updated_at) VALUES (?1, ?2, ?3, 'active', 'member', ?4, ?4)",
                    params![
                        fasti_domain::MembershipId::new_v7().to_string(),
                        subject_id.to_string(),
                        workspace_id.to_string(),
                        timestamp(created_at),
                    ],
                )
                .expect("active membership");
        }
        Fixture {
            _root: root,
            kernel,
            subject_id,
            workspace_id,
            instance_id,
            grants,
            created_at,
        }
    }

    fn create_session(
        fixture: &Fixture,
        grants: &[ProfileGrantId],
        selected: ProfileGrantId,
        offset_seconds: i64,
    ) -> CreatedBrowserSession {
        let created = fixture
            .kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    fixture.subject_id,
                    fixture.workspace_id,
                    grants.to_vec(),
                    selected,
                    policy(),
                    false,
                    fixture.created_at + ChronoDuration::seconds(offset_seconds),
                )
                .expect("session command"),
            )
            .expect("session");
        fixture
            .kernel
            .inner
            .connection
            .lock()
            .expect("connection")
            .execute(
                r#"
                INSERT INTO fasti_browser_session_authentication(
                    browser_session_id, trailbase_instance_id, activation_generation,
                    method, verified_at, recent_authentication_expires_at
                ) VALUES (?1, ?2, 1, 'trailbase_password', ?3, NULL)
                "#,
                params![
                    created.session().id().to_string(),
                    fixture.instance_id.to_string(),
                    timestamp(fixture.created_at + ChronoDuration::seconds(offset_seconds)),
                ],
            )
            .expect("session authentication");
        created
    }

    #[test]
    fn viable_administrators_require_active_subject_membership_and_role() {
        let fixture = fixture();
        let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
        let connection = fixture
            .kernel
            .lock_connection(CapabilityKey::CreateBrowserSession, correlation_id)
            .expect("connection");
        let count = || {
            viable_administrator_count(
                &connection,
                &fixture.workspace_id.to_string(),
                CapabilityKey::CreateBrowserSession,
                correlation_id,
            )
            .expect("administrator count")
        };
        assert_eq!(count(), 0);

        connection
            .execute(
                "UPDATE workspace_memberships SET role = 'administrator' WHERE auth_subject_id = ?1 AND workspace_id = ?2",
                params![
                    fixture.subject_id.to_string(),
                    fixture.workspace_id.to_string(),
                ],
            )
            .expect("administrator membership");
        assert_eq!(count(), 1);

        connection
            .execute(
                "UPDATE auth_subjects SET lifecycle = 'disabled' WHERE auth_subject_id = ?1",
                [fixture.subject_id.to_string()],
            )
            .expect("disable subject");
        assert_eq!(count(), 0);
    }

    fn assert_problem<T>(result: ApplicationResult<T>, expected: ProblemCode) {
        match result {
            Ok(_) => panic!("expected {expected:?}"),
            Err(problem) => assert_eq!(problem.code(), expected),
        }
    }

    #[test]
    fn session_creation_rejects_an_inactive_subject_without_problem_contract_panic() {
        let fixture = fixture();
        {
            let connection = fixture
                .kernel
                .lock_connection(
                    CapabilityKey::CreateBrowserSession,
                    fasti_domain::RequestCorrelationId::new_v7(),
                )
                .expect("connection");
            connection
                .execute(
                    "UPDATE auth_subjects SET lifecycle = 'disabled' WHERE auth_subject_id = ?1",
                    [fixture.subject_id.to_string()],
                )
                .expect("disable subject");
        }
        assert_problem(
            fixture.kernel.create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    fixture.subject_id,
                    fixture.workspace_id,
                    fixture.grants[..1].to_vec(),
                    fixture.grants[0],
                    policy(),
                    false,
                    fixture.created_at,
                )
                .expect("session command"),
            ),
            ProblemCode::Forbidden,
        );
    }

    #[test]
    fn last_seen_write_is_bounded_and_idle_and_absolute_expiry_fail_closed() {
        let fixture = fixture();
        let created = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 0);
        let before_interval = fixture
            .kernel
            .authenticate_browser_session(BrowserSessionQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                secret_copy(created.session_secret()),
                fixture.created_at + ChronoDuration::seconds(9),
            ))
            .expect("active before write interval");
        assert_eq!(before_interval.session().last_seen_at(), fixture.created_at);

        let at_interval = fixture
            .kernel
            .authenticate_browser_session(BrowserSessionQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                secret_copy(created.session_secret()),
                fixture.created_at + ChronoDuration::seconds(10),
            ))
            .expect("touch at interval");
        assert_eq!(
            at_interval.session().last_seen_at(),
            fixture.created_at + ChronoDuration::seconds(10)
        );
        assert_eq!(
            at_interval.session().idle_expires_at(),
            fixture.created_at + ChronoDuration::seconds(40)
        );

        assert_problem(
            fixture
                .kernel
                .authenticate_browser_session(BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(created.session_secret()),
                    fixture.created_at + ChronoDuration::seconds(40),
                )),
            ProblemCode::BrowserSessionExpired,
        );

        let absolute = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 1);
        assert_problem(
            fixture
                .kernel
                .authenticate_browser_session(BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(absolute.session_secret()),
                    fixture.created_at + ChronoDuration::seconds(121),
                )),
            ProblemCode::BrowserSessionExpired,
        );
    }

    #[test]
    fn successful_search_reads_persist_browser_activity_and_read_proof_cannot_commit() {
        use crate::kernel::now;
        use fasti_application::{
            BrowserRequestBoundaryPolicy, BrowserSessionAccessContext, OutboundAccessPolicy,
            ProviderId, ProviderStatePort, SearchPageRequest, SearchPersistencePort,
            SearchProviderQuery,
        };
        let mut fixture = fixture();
        fixture.created_at = now() - ChronoDuration::seconds(15);
        fixture
            .kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, 'metadata_search')",
                [fixture.grants[0].to_string()],
            )
            .unwrap();
        fixture
            .kernel
            .put_provider_capability_state(fixture.workspace_id, crate::search::tests::state(1))
            .unwrap();
        for cache_read in [false, true] {
            let created = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 0);
            let access = BrowserSessionAccessContext::read(
                BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(created.session_secret()),
                    now(),
                ),
                BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example")
                    .unwrap()
                    .validate_read(Some("fasti.example"))
                    .unwrap(),
            );
            let request = SearchPageRequest {
                correlation_id: fasti_domain::RequestCorrelationId::new_v7(),
                access: access.into(),
                query: SearchProviderQuery::try_new(
                    fasti_domain::SearchQuery::try_new("Film").unwrap(),
                    ProviderId::try_new("tmdb").unwrap(),
                    1,
                    None,
                    None,
                    vec![],
                )
                .unwrap(),
                outbound_policy: OutboundAccessPolicy::default(),
                terms_revision: "tmdb-v1".into(),
            };
            if cache_read {
                assert!(fixture
                    .kernel
                    .read_cached_search_page(&request, false)
                    .unwrap()
                    .is_none());
            } else {
                fixture.kernel.prepare_search_page(&request).unwrap();
            }
            let last_seen: String = fixture
                .kernel
                .inner
                .connection
                .lock()
                .unwrap()
                .query_row(
                    "SELECT last_seen_at FROM fasti_browser_sessions WHERE browser_session_id = ?1",
                    [created.session().id().to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(
                parse_timestamp(
                    &last_seen,
                    CapabilityKey::SearchMetadata,
                    request.correlation_id
                )
                .unwrap()
                    > fixture.created_at
            );
            let prepared = fixture.kernel.prepare_search_page(&request).unwrap();
            assert_eq!(
                fixture
                    .kernel
                    .commit_search_page(
                        &request,
                        &prepared,
                        &[],
                        &fasti_domain::Sha256Digest::from_bytes(&[7; 32]),
                        None
                    )
                    .unwrap_err()
                    .code(),
                ProblemCode::Forbidden
            );
        }
    }

    #[test]
    fn rotation_revokes_the_old_secret_without_extending_absolute_expiry() {
        let fixture = fixture();
        let created = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 0);
        let recent_expires_at = fixture.created_at + ChronoDuration::seconds(60);
        fixture
            .kernel
            .inner
            .connection
            .lock()
            .expect("connection")
            .execute(
                "UPDATE fasti_browser_session_authentication SET recent_authentication_expires_at = ?1 WHERE browser_session_id = ?2",
                params![
                    timestamp(recent_expires_at),
                    created.session().id().to_string(),
                ],
            )
            .expect("recent authentication");
        let rotated = fixture
            .kernel
            .rotate_browser_session(mutation_command(
                fasti_domain::RequestCorrelationId::new_v7(),
                secret_copy(created.session_secret()),
                secret_copy(created.csrf_secret()),
                fixture.created_at + ChronoDuration::seconds(1),
            ))
            .expect("rotate");
        let copied_authentication = fixture
            .kernel
            .inner
            .connection
            .lock()
            .expect("connection")
            .query_row(
                r#"
                SELECT trailbase_instance_id, activation_generation, method,
                       verified_at, recent_authentication_expires_at
                FROM fasti_browser_session_authentication
                WHERE browser_session_id = ?1
                "#,
                [rotated.session().id().to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .expect("rotated session authentication");
        assert_eq!(
            copied_authentication,
            (
                fixture.instance_id.to_string(),
                1,
                "trailbase_password".to_owned(),
                timestamp(fixture.created_at),
                Some(timestamp(recent_expires_at)),
            )
        );
        assert_ne!(rotated.session().id(), created.session().id());
        assert_eq!(rotated.session().rotation_generation(), 1);
        assert_eq!(
            rotated.session().absolute_expires_at(),
            created.session().absolute_expires_at()
        );
        assert_problem(
            fixture
                .kernel
                .authenticate_browser_session(BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(created.session_secret()),
                    fixture.created_at + ChronoDuration::seconds(2),
                )),
            ProblemCode::BrowserSessionRevoked,
        );
        fixture
            .kernel
            .authenticate_browser_session(BrowserSessionQuery::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                secret_copy(rotated.session_secret()),
                fixture.created_at + ChronoDuration::seconds(2),
            ))
            .expect("rotated secret is active");
    }

    #[test]
    fn exact_revocation_current_other_and_all_are_subject_scoped() {
        let fixture = fixture();
        let first = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 0);
        let second = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 1);
        let third = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 2);
        assert!(fixture
            .kernel
            .revoke_browser_session(TargetBrowserSessionCommand::new(
                mutation_command(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(first.session_secret()),
                    secret_copy(first.csrf_secret()),
                    fixture.created_at + ChronoDuration::seconds(3),
                ),
                second.session().id(),
            ))
            .expect("revoke exact")
            .revoked());
        assert_problem(
            fixture
                .kernel
                .authenticate_browser_session(BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(second.session_secret()),
                    fixture.created_at + ChronoDuration::seconds(4),
                )),
            ProblemCode::BrowserSessionRevoked,
        );
        assert_eq!(
            fixture
                .kernel
                .revoke_other_browser_sessions(mutation_command(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(first.session_secret()),
                    secret_copy(first.csrf_secret()),
                    fixture.created_at + ChronoDuration::seconds(4),
                ))
                .expect("revoke others"),
            1
        );
        assert_problem(
            fixture
                .kernel
                .authenticate_browser_session(BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(third.session_secret()),
                    fixture.created_at + ChronoDuration::seconds(5),
                )),
            ProblemCode::BrowserSessionRevoked,
        );
        assert_eq!(
            fixture
                .kernel
                .revoke_all_browser_sessions(mutation_command(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(first.session_secret()),
                    secret_copy(first.csrf_secret()),
                    fixture.created_at + ChronoDuration::seconds(5),
                ))
                .expect("revoke all"),
            1
        );
        assert_problem(
            fixture
                .kernel
                .authenticate_browser_session(BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(first.session_secret()),
                    fixture.created_at + ChronoDuration::seconds(6),
                )),
            ProblemCode::BrowserSessionRevoked,
        );
    }

    #[test]
    fn profile_selection_requires_a_pre_authorized_existing_grant_and_rotates() {
        let fixture = fixture();
        let created = create_session(&fixture, &fixture.grants[..2], fixture.grants[0], 0);
        assert_problem(
            fixture
                .kernel
                .select_browser_session_profile(SelectBrowserSessionProfileCommand::new(
                    mutation_command(
                        fasti_domain::RequestCorrelationId::new_v7(),
                        secret_copy(created.session_secret()),
                        secret_copy(created.csrf_secret()),
                        fixture.created_at + ChronoDuration::seconds(1),
                    ),
                    fixture.grants[2],
                )),
            ProblemCode::Forbidden,
        );
        let selected = fixture
            .kernel
            .select_browser_session_profile(SelectBrowserSessionProfileCommand::new(
                mutation_command(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(created.session_secret()),
                    secret_copy(created.csrf_secret()),
                    fixture.created_at + ChronoDuration::seconds(2),
                ),
                fixture.grants[1],
            ))
            .expect("select authorized grant");
        assert_eq!(
            selected.session().selected_profile_grant_id(),
            fixture.grants[1]
        );
        assert_eq!(selected.session().rotation_generation(), 1);
        assert_problem(
            fixture
                .kernel
                .authenticate_browser_session(BrowserSessionQuery::new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    secret_copy(created.session_secret()),
                    fixture.created_at + ChronoDuration::seconds(3),
                )),
            ProblemCode::BrowserSessionRevoked,
        );
    }

    #[test]
    fn session_grants_remain_subject_owned_and_active_at_the_transaction_boundary() {
        let fixture = fixture();
        let other_subject_id = AuthSubjectId::new_v7();
        fixture
            .kernel
            .create_auth_subject(CreateAuthSubjectCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                AuthSubject::try_new(
                    other_subject_id,
                    AuthSubjectLifecycle::Active,
                    1,
                    1,
                    fixture.created_at,
                    fixture.created_at,
                )
                .expect("valid other subject"),
            ))
            .expect("other subject");

        let create_for = |subject_id, workspace_id, grant_id| {
            fixture.kernel.create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    fasti_domain::RequestCorrelationId::new_v7(),
                    subject_id,
                    workspace_id,
                    vec![grant_id],
                    grant_id,
                    policy(),
                    false,
                    fixture.created_at,
                )
                .expect("session command"),
            )
        };

        assert_problem(
            create_for(other_subject_id, fixture.workspace_id, fixture.grants[0]),
            ProblemCode::Forbidden,
        );

        {
            let connection = fixture
                .kernel
                .lock_connection(
                    CapabilityKey::CreateBrowserSession,
                    fasti_domain::RequestCorrelationId::new_v7(),
                )
                .expect("connection");
            connection
                .execute(
                    "UPDATE profile_grants SET status = 'revoked', revoked_at = ?1 WHERE grant_id = ?2",
                    params![timestamp(fixture.created_at), fixture.grants[0].to_string()],
                )
                .expect("revoke grant");
        }
        assert_problem(
            create_for(fixture.subject_id, fixture.workspace_id, fixture.grants[0]),
            ProblemCode::Forbidden,
        );
        {
            let connection = fixture
                .kernel
                .lock_connection(
                    CapabilityKey::CreateBrowserSession,
                    fasti_domain::RequestCorrelationId::new_v7(),
                )
                .expect("connection");
            connection
                .execute(
                    "UPDATE profile_grants SET status = 'active', revoked_at = NULL WHERE grant_id = ?1",
                    [fixture.grants[0].to_string()],
                )
                .expect("reactivate grant");
            connection
                .execute(
                    "UPDATE clients SET status = 'revoked' WHERE workspace_id = ?1",
                    [fixture.workspace_id.to_string()],
                )
                .expect("revoke client");
        }
        assert_problem(
            create_for(fixture.subject_id, fixture.workspace_id, fixture.grants[0]),
            ProblemCode::Forbidden,
        );
        let other_workspace_id = WorkspaceId::new_v7();
        let other_client_id = ClientId::new_v7();
        let other_profile_id = ProfileId::new_v7();
        let other_grant_id = ProfileGrantId::new_v7();
        let connection = fixture
            .kernel
            .lock_connection(
                CapabilityKey::CreateBrowserSession,
                fasti_domain::RequestCorrelationId::new_v7(),
            )
            .expect("connection");
        connection
            .execute(
                "UPDATE clients SET status = 'active' WHERE workspace_id = ?1",
                [fixture.workspace_id.to_string()],
            )
            .expect("reactivate client");
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![
                    other_workspace_id.to_string(),
                    timestamp(fixture.created_at)
                ],
            )
            .expect("other workspace");
        connection
            .execute(
                "INSERT INTO clients(client_id, workspace_id, status, current_credential_epoch, created_at) VALUES (?1, ?2, 'active', 1, ?3)",
                params![other_client_id.to_string(), other_workspace_id.to_string(), timestamp(fixture.created_at)],
            )
            .expect("other client");
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                params![
                    other_profile_id.to_string(),
                    other_workspace_id.to_string(),
                    timestamp(fixture.created_at)
                ],
            )
            .expect("other profile");
        connection
            .execute(
                "INSERT INTO profile_grants(grant_id, workspace_id, profile_id, client_id, status, created_at) VALUES (?1, ?2, ?3, ?4, 'active', ?5)",
                params![other_grant_id.to_string(), other_workspace_id.to_string(), other_profile_id.to_string(), other_client_id.to_string(), timestamp(fixture.created_at)],
            )
            .expect("other grant");
        connection
            .execute(
                "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                params![fixture.subject_id.to_string(), other_grant_id.to_string()],
            )
            .expect("cross-workspace subject grant");
        drop(connection);

        assert_problem(
            create_for(fixture.subject_id, fixture.workspace_id, other_grant_id),
            ProblemCode::Forbidden,
        );
    }

    #[test]
    fn profile_selection_rechecks_revoked_grants() {
        let fixture = fixture();
        let created = create_session(&fixture, &fixture.grants[..2], fixture.grants[0], 0);
        let connection = fixture
            .kernel
            .lock_connection(
                CapabilityKey::SelectBrowserSessionProfile,
                fasti_domain::RequestCorrelationId::new_v7(),
            )
            .expect("connection");
        connection
            .execute(
                "UPDATE profile_grants SET status = 'revoked', revoked_at = ?1 WHERE grant_id = ?2",
                params![timestamp(fixture.created_at), fixture.grants[1].to_string()],
            )
            .expect("revoke grant");
        drop(connection);

        assert_problem(
            fixture
                .kernel
                .select_browser_session_profile(SelectBrowserSessionProfileCommand::new(
                    mutation_command(
                        fasti_domain::RequestCorrelationId::new_v7(),
                        secret_copy(created.session_secret()),
                        secret_copy(created.csrf_secret()),
                        fixture.created_at + ChronoDuration::seconds(1),
                    ),
                    fixture.grants[1],
                )),
            ProblemCode::Forbidden,
        );
    }

    #[test]
    fn session_authentication_rechecks_membership_grant_and_client_state() {
        for revoke in ["membership", "grant", "client"] {
            let fixture = fixture();
            let created = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 0);
            let connection = fixture
                .kernel
                .lock_connection(
                    CapabilityKey::ReadBrowserSession,
                    fasti_domain::RequestCorrelationId::new_v7(),
                )
                .expect("connection");
            if revoke == "membership" {
                connection
                    .execute(
                        "UPDATE workspace_memberships SET lifecycle = 'suspended' WHERE auth_subject_id = ?1 AND workspace_id = ?2",
                        params![
                            fixture.subject_id.to_string(),
                            fixture.workspace_id.to_string(),
                        ],
                    )
                    .expect("suspend membership");
            } else if revoke == "grant" {
                connection
                    .execute(
                        "UPDATE profile_grants SET status = 'revoked', revoked_at = ?1 WHERE grant_id = ?2",
                        params![timestamp(fixture.created_at), fixture.grants[0].to_string()],
                    )
                    .expect("revoke selected grant");
            } else {
                connection
                    .execute(
                        "UPDATE clients SET status = 'revoked' WHERE workspace_id = ?1",
                        [fixture.workspace_id.to_string()],
                    )
                    .expect("revoke grant-owning client");
            }
            drop(connection);

            assert_problem(
                fixture
                    .kernel
                    .authenticate_browser_session(BrowserSessionQuery::new(
                        fasti_domain::RequestCorrelationId::new_v7(),
                        secret_copy(created.session_secret()),
                        fixture.created_at + ChronoDuration::seconds(1),
                    )),
                ProblemCode::SessionPolicyChanged,
            );
        }
    }

    #[test]
    fn concurrent_exact_revocation_has_one_winner_and_exact_ids_cannot_collide() {
        let fixture = fixture();
        let current = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 0);
        let target = create_session(&fixture, &fixture.grants[..1], fixture.grants[0], 1);
        let session_hex = current.session_secret().expose_hex();
        let csrf_hex = current.csrf_secret().expose_hex();
        let target_id = target.session().id();
        let at = fixture.created_at + ChronoDuration::seconds(2);
        let mut threads = Vec::new();
        for _ in 0..2 {
            let kernel = fixture.kernel.clone();
            let session_hex = session_hex.clone();
            let csrf_hex = csrf_hex.clone();
            threads.push(std::thread::spawn(move || {
                kernel
                    .revoke_browser_session(TargetBrowserSessionCommand::new(
                        mutation_command(
                            fasti_domain::RequestCorrelationId::new_v7(),
                            SecretMaterial::try_from_hex(&session_hex).expect("session"),
                            SecretMaterial::try_from_hex(&csrf_hex).expect("csrf"),
                            at,
                        ),
                        target_id,
                    ))
                    .expect("concurrent revoke")
            }));
        }
        let wins = threads
            .into_iter()
            .map(|thread| thread.join().expect("thread"))
            .filter(|outcome| outcome.revoked())
            .count();
        assert_eq!(wins, 1);

        let connection = fixture
            .kernel
            .lock_connection(
                CapabilityKey::CreateBrowserSession,
                fasti_domain::RequestCorrelationId::new_v7(),
            )
            .expect("connection");
        let duplicate = connection.execute(
            r#"
            INSERT INTO fasti_browser_sessions(
                browser_session_id, session_digest, csrf_digest, auth_subject_id,
                workspace_id, selected_profile_grant_id, created_at, last_seen_at,
                idle_expires_at, absolute_expires_at, idle_timeout_seconds,
                last_seen_write_interval_seconds, revoked_at, auth_epoch,
                authorization_epoch, rotation_generation
            )
            SELECT browser_session_id, session_digest || 'x', csrf_digest,
                   auth_subject_id, workspace_id, selected_profile_grant_id,
                   created_at, last_seen_at, idle_expires_at, absolute_expires_at,
                   idle_timeout_seconds, last_seen_write_interval_seconds,
                   revoked_at, auth_epoch, authorization_epoch, rotation_generation
            FROM fasti_browser_sessions WHERE browser_session_id = ?1
            "#,
            [current.session().id().to_string()],
        );
        assert!(
            duplicate.is_err(),
            "exact public session IDs must be unique"
        );
    }
}
