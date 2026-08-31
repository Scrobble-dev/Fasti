use crate::browser_auth::authenticate_session;
use crate::kernel::{map_sql, parse_timestamp, timestamp};
use crate::SqliteKernel;
use fasti_application::{
    c1_first_run_steps, AccessBrowserSessionSummary, AccessCeremonyEvidence, AccessEvidenceKind,
    AccessEvidenceState, AccessMembershipSummary, AccessProfileGrantSummary, AccessProjection,
    AccessProjectionPort, AccessRecentAuthenticationSummary, AccessSessionAuthenticationSummary,
    AccessSubjectSummary, AccessTrailBaseActivationSummary, ApplicationResult, BrowserSessionQuery,
    CapabilityKey, FastiProblem, ProblemCode, SessionPolicy, ACCESS_EVIDENCE_LIMIT,
    ACCESS_PROFILE_GRANT_LIMIT, ACCESS_SESSION_INVENTORY_LIMIT,
};
use fasti_domain::{
    AuthCeremonyFailure, AuthCeremonyState, AuthSubjectId, AuthenticationMethod, BrowserSessionId,
    ClientId, FastiBrowserSession, MembershipId, MembershipLifecycle, OperationId, ProfileGrantId,
    ProfileId, RequestCorrelationId, TrailBaseActivationState, TrailBaseInstanceId, WorkspaceId,
    WorkspaceRole,
};
use rusqlite::{params, Connection, TransactionBehavior};
use std::str::FromStr;

struct SessionProjectionRow {
    browser_session_id: String,
    auth_subject_id: String,
    workspace_id: String,
    selected_profile_grant_id: String,
    created_at: String,
    last_seen_at: String,
    idle_expires_at: String,
    absolute_expires_at: String,
    revoked_at: Option<String>,
    idle_timeout_seconds: i64,
    last_seen_write_interval_seconds: i64,
    auth_epoch: i64,
    authorization_epoch: i64,
    rotation_generation: i64,
}

struct SessionAuthenticationRow {
    trailbase_instance_id: String,
    activation_generation: i64,
    method: String,
    verified_at: String,
    recent_authentication_expires_at: Option<String>,
}

fn problem(code: ProblemCode, correlation_id: RequestCorrelationId) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(
        code,
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    ))
}

fn parse_id<T: FromStr>(value: &str, correlation_id: RequestCorrelationId) -> ApplicationResult<T> {
    value
        .parse()
        .map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))
}

fn parse_u64(value: i64, correlation_id: RequestCorrelationId) -> ApplicationResult<u64> {
    u64::try_from(value).map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))
}

fn parse_session(
    row: SessionProjectionRow,
    current_session_id: BrowserSessionId,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessBrowserSessionSummary> {
    let session = FastiBrowserSession::try_new(
        parse_id(&row.browser_session_id, correlation_id)?,
        parse_id(&row.auth_subject_id, correlation_id)?,
        parse_id(&row.workspace_id, correlation_id)?,
        parse_id(&row.selected_profile_grant_id, correlation_id)?,
        parse_timestamp(
            &row.created_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
        parse_timestamp(
            &row.last_seen_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
        parse_timestamp(
            &row.idle_expires_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
        parse_timestamp(
            &row.absolute_expires_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
        row.revoked_at
            .as_deref()
            .map(|value| {
                parse_timestamp(value, CapabilityKey::ReadAccessProjection, correlation_id)
            })
            .transpose()?,
        parse_u64(row.auth_epoch, correlation_id)?,
        parse_u64(row.authorization_epoch, correlation_id)?,
        parse_u64(row.rotation_generation, correlation_id)?,
    )
    .map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))?;
    Ok(AccessBrowserSessionSummary::new(
        session,
        session.id() == current_session_id,
        parse_u64(row.idle_timeout_seconds, correlation_id)?,
        parse_u64(row.last_seen_write_interval_seconds, correlation_id)?,
    ))
}

fn load_membership(
    connection: &Connection,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessMembershipSummary> {
    let (membership_id, lifecycle, role, created_at, updated_at): (
        String,
        String,
        String,
        String,
        String,
    ) = map_sql(
        connection.query_row(
            r#"
            SELECT membership_id, lifecycle, role, created_at, updated_at
            FROM workspace_memberships
            WHERE auth_subject_id = ?1 AND workspace_id = ?2 AND lifecycle = 'active'
            "#,
            params![subject_id.to_string(), workspace_id.to_string()],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    Ok(AccessMembershipSummary::new(
        parse_id::<MembershipId>(&membership_id, correlation_id)?,
        workspace_id,
        MembershipLifecycle::from_storage(&lifecycle)
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, correlation_id))?,
        WorkspaceRole::from_storage(&role)
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, correlation_id))?,
        parse_timestamp(
            &created_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
        parse_timestamp(
            &updated_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
    ))
}

fn load_sessions(
    connection: &Connection,
    subject: fasti_domain::AuthSubject,
    current_session_id: BrowserSessionId,
    at: chrono::DateTime<chrono::Utc>,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<(Vec<AccessBrowserSessionSummary>, bool)> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT session.browser_session_id, session.auth_subject_id,
                   session.workspace_id, session.selected_profile_grant_id,
                   session.created_at, session.last_seen_at,
                   session.idle_expires_at, session.absolute_expires_at,
                   session.revoked_at, session.idle_timeout_seconds,
                   session.last_seen_write_interval_seconds, session.auth_epoch,
                   session.authorization_epoch, session.rotation_generation
            FROM fasti_browser_sessions session
            JOIN workspace_memberships membership
              ON membership.auth_subject_id = session.auth_subject_id
             AND membership.workspace_id = session.workspace_id
             AND membership.lifecycle = 'active'
            JOIN fasti_browser_session_grants session_grant
              ON session_grant.browser_session_id = session.browser_session_id
             AND session_grant.profile_grant_id = session.selected_profile_grant_id
            JOIN auth_subject_profile_grants subject_grant
              ON subject_grant.auth_subject_id = session.auth_subject_id
             AND subject_grant.profile_grant_id = session.selected_profile_grant_id
            JOIN profile_grants grant
              ON grant.grant_id = session.selected_profile_grant_id
             AND grant.workspace_id = session.workspace_id
             AND grant.status = 'active'
            JOIN clients client
              ON client.client_id = grant.client_id
             AND client.status = 'active'
            WHERE session.auth_subject_id = ?1
              AND session.revoked_at IS NULL
              AND session.idle_expires_at > ?2
              AND session.absolute_expires_at > ?2
              AND session.auth_epoch = ?3
              AND session.authorization_epoch = ?4
            ORDER BY (session.browser_session_id = ?5) DESC,
                     session.last_seen_at DESC, session.browser_session_id
            LIMIT ?6
            "#,
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                subject.id().to_string(),
                timestamp(at),
                i64::try_from(subject.auth_epoch())
                    .map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))?,
                i64::try_from(subject.authorization_epoch())
                    .map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))?,
                current_session_id.to_string(),
                i64::try_from(ACCESS_SESSION_INVENTORY_LIMIT + 1)
                    .map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))?,
            ],
            |row| {
                Ok(SessionProjectionRow {
                    browser_session_id: row.get(0)?,
                    auth_subject_id: row.get(1)?,
                    workspace_id: row.get(2)?,
                    selected_profile_grant_id: row.get(3)?,
                    created_at: row.get(4)?,
                    last_seen_at: row.get(5)?,
                    idle_expires_at: row.get(6)?,
                    absolute_expires_at: row.get(7)?,
                    revoked_at: row.get(8)?,
                    idle_timeout_seconds: row.get(9)?,
                    last_seen_write_interval_seconds: row.get(10)?,
                    auth_epoch: row.get(11)?,
                    authorization_epoch: row.get(12)?,
                    rotation_generation: row.get(13)?,
                })
            },
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    let mut sessions = Vec::with_capacity(ACCESS_SESSION_INVENTORY_LIMIT + 1);
    for row in rows {
        sessions.push(parse_session(
            map_sql(row, CapabilityKey::ReadAccessProjection, correlation_id)?,
            current_session_id,
            correlation_id,
        )?);
    }
    let truncated = sessions.len() > ACCESS_SESSION_INVENTORY_LIMIT;
    sessions.truncate(ACCESS_SESSION_INVENTORY_LIMIT);
    Ok((sessions, truncated))
}

fn load_profile_grants(
    connection: &Connection,
    subject_id: AuthSubjectId,
    current_session_id: BrowserSessionId,
    workspace_id: WorkspaceId,
    selected_grant_id: ProfileGrantId,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<(Vec<AccessProfileGrantSummary>, bool)> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT grant.grant_id, grant.profile_id, grant.client_id
            FROM fasti_browser_session_grants session_grant
            JOIN auth_subject_profile_grants subject_grant
              ON subject_grant.auth_subject_id = ?1
             AND subject_grant.profile_grant_id = session_grant.profile_grant_id
            JOIN profile_grants grant
              ON grant.grant_id = session_grant.profile_grant_id
             AND grant.workspace_id = ?2
             AND grant.status = 'active'
            JOIN clients client
              ON client.client_id = grant.client_id
             AND client.status = 'active'
            WHERE session_grant.browser_session_id = ?3
            ORDER BY grant.profile_id, grant.grant_id
            LIMIT ?4
            "#,
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                subject_id.to_string(),
                workspace_id.to_string(),
                current_session_id.to_string(),
                i64::try_from(ACCESS_PROFILE_GRANT_LIMIT + 1)
                    .map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))?,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    let mut grants = Vec::with_capacity(ACCESS_PROFILE_GRANT_LIMIT + 1);
    for row in rows {
        let (grant_id, profile_id, client_id) =
            map_sql(row, CapabilityKey::ReadAccessProjection, correlation_id)?;
        let grant_id = parse_id::<ProfileGrantId>(&grant_id, correlation_id)?;
        grants.push(AccessProfileGrantSummary::new(
            grant_id,
            parse_id::<ProfileId>(&profile_id, correlation_id)?,
            parse_id::<ClientId>(&client_id, correlation_id)?,
            grant_id == selected_grant_id,
        ));
    }
    let truncated = grants.len() > ACCESS_PROFILE_GRANT_LIMIT;
    grants.truncate(ACCESS_PROFILE_GRANT_LIMIT);
    Ok((grants, truncated))
}

fn load_session_authentication(
    connection: &Connection,
    session_id: BrowserSessionId,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<SessionAuthenticationRow> {
    map_sql(
        connection.query_row(
            r#"
            SELECT trailbase_instance_id, activation_generation, method,
                   verified_at, recent_authentication_expires_at
            FROM fasti_browser_session_authentication
            WHERE browser_session_id = ?1
            "#,
            [session_id.to_string()],
            |row| {
                Ok(SessionAuthenticationRow {
                    trailbase_instance_id: row.get(0)?,
                    activation_generation: row.get(1)?,
                    method: row.get(2)?,
                    verified_at: row.get(3)?,
                    recent_authentication_expires_at: row.get(4)?,
                })
            },
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )
}

fn load_trailbase_activation(
    connection: &Connection,
    authentication: &SessionAuthenticationRow,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessTrailBaseActivationSummary> {
    let (instance_id, state, blocker, generation, updated_at): (
        String,
        String,
        Option<String>,
        i64,
        String,
    ) = map_sql(
        connection.query_row(
            r#"
            SELECT trailbase_instance_id, activation_state, activation_blocker,
                   activation_generation, updated_at
            FROM trailbase_installation
            WHERE trailbase_instance_id = ?1
            "#,
            [&authentication.trailbase_instance_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    let generation = parse_u64(generation, correlation_id)?;
    Ok(AccessTrailBaseActivationSummary::new(
        parse_id::<TrailBaseInstanceId>(&instance_id, correlation_id)?,
        TrailBaseActivationState::from_storage(&state, blocker.as_deref())
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, correlation_id))?,
        generation,
        generation == parse_u64(authentication.activation_generation, correlation_id)?,
        parse_timestamp(
            &updated_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
    ))
}

fn project_session_authentication(
    row: SessionAuthenticationRow,
    activation: AccessTrailBaseActivationSummary,
    at: chrono::DateTime<chrono::Utc>,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessSessionAuthenticationSummary> {
    let recent_expires_at = row
        .recent_authentication_expires_at
        .as_deref()
        .map(|value| parse_timestamp(value, CapabilityKey::ReadAccessProjection, correlation_id))
        .transpose()?;
    let recent_state = match recent_expires_at {
        None => AccessEvidenceState::Unavailable,
        Some(expires_at)
            if expires_at > at && activation.evidence_state() == AccessEvidenceState::Verified =>
        {
            AccessEvidenceState::Verified
        }
        Some(_) => AccessEvidenceState::NeedsAttention,
    };
    Ok(AccessSessionAuthenticationSummary::new(
        AuthenticationMethod::from_storage(&row.method)
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, correlation_id))?,
        parse_timestamp(
            &row.verified_at,
            CapabilityKey::ReadAccessProjection,
            correlation_id,
        )?,
        parse_u64(row.activation_generation, correlation_id)?,
        AccessRecentAuthenticationSummary::new(recent_state, recent_expires_at),
    ))
}

fn load_evidence(
    connection: &Connection,
    subject_id: AuthSubjectId,
    workspace_id: WorkspaceId,
    current_session_id: BrowserSessionId,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<(Vec<AccessCeremonyEvidence>, bool)> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT audit.event_kind, audit.operation_id, audit.correlation_id,
                   audit.occurred_at, ceremony.state, ceremony.failure
            FROM access_audit_events audit
            LEFT JOIN auth_ceremonies ceremony
              ON ceremony.operation_id = audit.operation_id
            WHERE audit.auth_subject_id = ?1
              AND audit.workspace_id = ?2
              AND (
                    (audit.event_kind = 'browser_session_issued'
                        AND audit.browser_session_id = ?3)
                    OR audit.event_kind = 'first_administrator_bootstrapped'
              )
            ORDER BY audit.occurred_at DESC, audit.audit_event_id DESC
            LIMIT ?4
            "#,
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                subject_id.to_string(),
                workspace_id.to_string(),
                current_session_id.to_string(),
                i64::try_from(ACCESS_EVIDENCE_LIMIT + 1)
                    .map_err(|_| problem(ProblemCode::IntegrityFailed, correlation_id))?,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        ),
        CapabilityKey::ReadAccessProjection,
        correlation_id,
    )?;
    let mut evidence = Vec::with_capacity(ACCESS_EVIDENCE_LIMIT + 1);
    for row in rows {
        let (kind, operation_id, evidence_correlation_id, occurred_at, state, failure) =
            map_sql(row, CapabilityKey::ReadAccessProjection, correlation_id)?;
        let ceremony_state = state
            .as_deref()
            .map(|value| {
                AuthCeremonyState::from_storage(value)
                    .ok_or_else(|| problem(ProblemCode::IntegrityFailed, correlation_id))
            })
            .transpose()?;
        let failure = failure
            .as_deref()
            .map(|value| {
                AuthCeremonyFailure::from_storage(value)
                    .ok_or_else(|| problem(ProblemCode::IntegrityFailed, correlation_id))
            })
            .transpose()?;
        if ceremony_state.is_some_and(|state| state != AuthCeremonyState::Completed)
            || failure.is_some()
        {
            return Err(problem(ProblemCode::IntegrityFailed, correlation_id));
        }
        let kind = match kind.as_str() {
            "browser_session_issued" => AccessEvidenceKind::CurrentSessionIssued,
            "first_administrator_bootstrapped" => AccessEvidenceKind::FirstAdministratorBootstrap,
            _ => return Err(problem(ProblemCode::IntegrityFailed, correlation_id)),
        };
        evidence.push(AccessCeremonyEvidence::new(
            kind,
            AccessEvidenceState::Verified,
            parse_id::<OperationId>(&operation_id, correlation_id)?,
            parse_id::<RequestCorrelationId>(&evidence_correlation_id, correlation_id)?,
            ceremony_state,
            failure,
            parse_timestamp(
                &occurred_at,
                CapabilityKey::ReadAccessProjection,
                correlation_id,
            )?,
        ));
    }
    let truncated = evidence.len() > ACCESS_EVIDENCE_LIMIT;
    evidence.truncate(ACCESS_EVIDENCE_LIMIT);
    Ok((evidence, truncated))
}

impl AccessProjectionPort for SqliteKernel {
    fn read_access_projection(
        &self,
        query: BrowserSessionQuery,
    ) -> ApplicationResult<AccessProjection> {
        let capability = CapabilityKey::ReadAccessProjection;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let current = authenticate_session(
            &transaction,
            query.session_secret(),
            None,
            query.now(),
            capability,
            correlation_id,
        )?;
        let subject = current.subject();
        let session = current.session();
        let membership = load_membership(
            &transaction,
            subject.id(),
            session.workspace_id(),
            correlation_id,
        )?;
        let (sessions, sessions_truncated) = load_sessions(
            &transaction,
            subject,
            session.id(),
            query.now(),
            correlation_id,
        )?;
        let current_session = sessions
            .iter()
            .copied()
            .find(|summary| summary.is_current())
            .ok_or_else(|| problem(ProblemCode::IntegrityFailed, correlation_id))?;
        let (profile_grants, profile_grants_truncated) = load_profile_grants(
            &transaction,
            subject.id(),
            session.id(),
            session.workspace_id(),
            session.selected_profile_grant_id(),
            correlation_id,
        )?;
        if !profile_grants.iter().any(|grant| grant.is_selected()) {
            return Err(problem(ProblemCode::IntegrityFailed, correlation_id));
        }
        let authentication_row =
            load_session_authentication(&transaction, session.id(), correlation_id)?;
        let trailbase =
            load_trailbase_activation(&transaction, &authentication_row, correlation_id)?;
        let authentication = project_session_authentication(
            authentication_row,
            trailbase,
            query.now(),
            correlation_id,
        )?;
        let (evidence, evidence_truncated) = load_evidence(
            &transaction,
            subject.id(),
            session.workspace_id(),
            session.id(),
            correlation_id,
        )?;
        let projection = AccessProjection::new(
            query.now(),
            AccessSubjectSummary::new(
                subject.id(),
                subject.lifecycle(),
                subject.created_at(),
                subject.updated_at(),
            ),
            membership,
            current_session,
            sessions,
            sessions_truncated,
            profile_grants,
            profile_grants_truncated,
            SessionPolicy::C1,
            authentication,
            trailbase,
            c1_first_run_steps(),
            evidence,
            evidence_truncated,
        );
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(projection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use chrono::{TimeZone, Utc};
    use fasti_application::{
        BrowserSessionPort, CreateAuthSubjectCommand, CreateBrowserSessionCommand, SecretMaterial,
        SessionPolicy,
    };
    use fasti_domain::{
        AuthSubject, AuthSubjectLifecycle, MembershipId, OperationId, Sha256Digest,
        TrailBaseInstanceId,
    };

    fn at(seconds: i64) -> chrono::DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000 + seconds, 0)
            .single()
            .expect("time")
    }

    #[test]
    fn projection_is_current_bounded_and_does_not_invent_later_access_state() {
        let node = TestNode::new();
        let subject = AuthSubject::try_new(
            AuthSubjectId::new_v7(),
            AuthSubjectLifecycle::Active,
            0,
            0,
            at(0),
            at(0),
        )
        .expect("subject");
        node.kernel
            .create_auth_subject(CreateAuthSubjectCommand::new(
                RequestCorrelationId::new_v7(),
                subject,
            ))
            .expect("persist subject");
        let membership_id = MembershipId::new_v7();
        let instance_id = TrailBaseInstanceId::new_v7();
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    r#"
                    INSERT INTO trailbase_installation(
                        singleton, trailbase_instance_id, physical_root_identity,
                        release_lock_identity,
                        activation_state, activation_blocker, activation_generation,
                        created_at, updated_at
                    ) VALUES (1, ?1, ?2, ?3, 'active', NULL, 1, ?4, ?4)
                    "#,
                    params![
                        instance_id.to_string(),
                        Sha256Digest::from_bytes(&[7; 32]).to_string(),
                        Sha256Digest::from_bytes(&[8; 32]).to_string(),
                        timestamp(at(0)),
                    ],
                )
                .expect("installation");
            connection
                .execute(
                    r#"
                    INSERT INTO workspace_memberships(
                        membership_id, auth_subject_id, workspace_id, lifecycle,
                        role, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, 'active', 'administrator', ?4, ?4)
                    "#,
                    params![
                        membership_id.to_string(),
                        subject.id().to_string(),
                        node.access.workspace_id().to_string(),
                        timestamp(at(0)),
                    ],
                )
                .expect("membership");
            connection
                .execute(
                    "INSERT INTO auth_subject_profile_grants(auth_subject_id, profile_grant_id) VALUES (?1, ?2)",
                    params![subject.id().to_string(), node.access.grant_id().to_string()],
                )
                .expect("subject grant");
        }
        let created = node
            .kernel
            .create_browser_session(
                CreateBrowserSessionCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    subject.id(),
                    node.access.workspace_id(),
                    vec![node.access.grant_id()],
                    node.access.grant_id(),
                    SessionPolicy::C1,
                    false,
                    at(1),
                )
                .expect("command"),
            )
            .expect("session");
        let operation_id = OperationId::new_v7();
        let evidence_correlation_id = RequestCorrelationId::new_v7();
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    r#"
                    INSERT INTO fasti_browser_session_authentication(
                        browser_session_id, trailbase_instance_id, activation_generation,
                        method, verified_at, recent_authentication_expires_at
                    ) VALUES (?1, ?2, 1, 'trailbase_password', ?3, NULL)
                    "#,
                    params![
                        created.session().id().to_string(),
                        instance_id.to_string(),
                        timestamp(at(1)),
                    ],
                )
                .expect("session authentication");
            connection
                .execute(
                    r#"
                    INSERT INTO access_audit_events(
                        event_kind, trailbase_instance_id, auth_subject_id,
                        workspace_id, operation_id, browser_session_id,
                        correlation_id, occurred_at
                    ) VALUES ('browser_session_issued', ?1, ?2, ?3, ?4, ?5, ?6, ?7)
                    "#,
                    params![
                        instance_id.to_string(),
                        subject.id().to_string(),
                        node.access.workspace_id().to_string(),
                        operation_id.to_string(),
                        created.session().id().to_string(),
                        evidence_correlation_id.to_string(),
                        timestamp(at(1)),
                    ],
                )
                .expect("audit evidence");
        }
        let query_secret = SecretMaterial::try_from_hex(&created.session_secret().expose_hex())
            .expect("copy session secret");
        let projection = node
            .kernel
            .read_access_projection(BrowserSessionQuery::new(
                RequestCorrelationId::new_v7(),
                query_secret,
                at(2),
            ))
            .expect("projection");

        assert_eq!(projection.subject().id(), subject.id());
        assert_eq!(projection.membership().role(), WorkspaceRole::Administrator);
        assert_eq!(projection.sessions().len(), 1);
        assert!(projection.current_session().is_current());
        assert!(projection.profile_grants()[0].is_selected());
        assert_eq!(
            projection.authentication().recent_authentication().state(),
            AccessEvidenceState::Unavailable
        );
        assert_eq!(projection.evidence_state(), AccessEvidenceState::Verified);
        assert!(projection.first_run_steps()[1..]
            .iter()
            .all(|step| step.state() == AccessEvidenceState::Unavailable));
    }
}
