use crate::crypto::sha256_bytes;
use crate::kernel::timestamp;
use crate::SqliteKernel;
use chrono::{DateTime, Duration, Utc};
use fasti_application::{
    AccessAdministrationPort, ApplicationResult, BootstrapFirstAdministratorCommand,
    BootstrapFirstAdministratorOutcome, CancelAuthCeremonyCommand, CapabilityKey,
    ClaimAuthCeremonyCommand, FastiProblem, HumanAccessPort, ProblemCode, StartAuthCeremonyCommand,
    VerifyTrailBaseInstallationCommand,
};
use fasti_domain::{
    AccessAuditEventKind, AccessInvariantError, AuthCallbackPath, AuthCeremony,
    AuthCeremonyFailure, AuthCeremonyProtocol, AuthCeremonyPurpose, AuthCeremonyState,
    AuthReturnTarget, AuthSubject, AuthSubjectId, AuthSubjectLifecycle, MembershipId,
    MembershipLifecycle, OperationId, RequestCorrelationId, Sha256Digest, TrailBaseActivationState,
    TrailBaseExternalAnchor, TrailBaseInstallation, TrailBaseInstanceId, TrailBaseSubject,
    WorkspaceId, WorkspaceMembership, WorkspaceRole,
};
use rusqlite::{
    params, Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior,
};
use thiserror::Error;

const CEREMONY_CAPACITY: i64 = 10_000;
const CEREMONY_RETENTION_HOURS: i64 = 24;
const AUDIT_CAPACITY: i64 = 10_000;
const AUDIT_RETENTION_DAYS: i64 = 90;

#[derive(Debug, Error)]
pub(crate) enum HumanAccessStoreError {
    #[error("human Access storage failed")]
    Storage(#[from] rusqlite::Error),
    #[error("human Access persisted state is invalid")]
    Integrity,
    #[error("human Access capacity is exhausted")]
    CapacityExceeded,
    #[error("human Access record was not found")]
    NotFound,
    #[error("human Access record conflicts with existing state")]
    Conflict,
    #[error(transparent)]
    Invariant(#[from] AccessInvariantError),
}

type StoreResult<T> = Result<T, HumanAccessStoreError>;

struct CeremonyRow {
    operation_id: String,
    purpose: String,
    protocol: String,
    trailbase_instance_id: String,
    activation_generation: i64,
    browser_binding_digest: String,
    callback_path: String,
    return_target: String,
    correlation_id: String,
    state: String,
    failure: Option<String>,
    created_at: String,
    expires_at: String,
    claimed_at: Option<String>,
    terminal_at: Option<String>,
}

impl SqliteKernel {
    pub(crate) fn verify_trailbase_installation(
        &self,
        instance_id: TrailBaseInstanceId,
        release_matches: bool,
        declared_restore: bool,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<TrailBaseInstallation> {
        let observed_root =
            Sha256Digest::from_bytes(&sha256_bytes(self.data_root_identity().as_bytes()));
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let current = load_installation(&transaction)?;
        let is_new = current.is_none();
        let mut installation = match current {
            Some(current) if current.id() == instance_id => current,
            Some(_) => return Err(HumanAccessStoreError::Integrity),
            None => TrailBaseInstallation::new(instance_id, observed_root.clone(), at),
        };

        let changed = if declared_restore {
            installation.declare_restore(at)?
        } else {
            installation.verify(&observed_root, release_matches, at)?
        };
        persist_installation(&transaction, &installation, is_new)?;
        if changed {
            let event = match installation.activation_state() {
                TrailBaseActivationState::Active => AccessAuditEventKind::TrailBaseActivated,
                TrailBaseActivationState::Blocked(_) => AccessAuditEventKind::TrailBaseBlocked,
                TrailBaseActivationState::Inactive => return Err(HumanAccessStoreError::Integrity),
            };
            insert_installation_audit(&transaction, event, installation.id(), correlation_id, at)?;
        }
        transaction.commit()?;
        Ok(installation)
    }

    pub(crate) fn maintain_auth_ceremonies(
        &self,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<Vec<OperationId>> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let expired = maintain_ceremonies(&transaction, correlation_id, at)?;
        transaction.commit()?;
        Ok(expired)
    }

    pub(crate) fn insert_auth_ceremony(&self, ceremony: &AuthCeremony) -> StoreResult<()> {
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        maintain_ceremonies(
            &transaction,
            ceremony.correlation_id(),
            ceremony.created_at(),
        )?;
        let count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM auth_ceremonies", [], |row| row.get(0))?;
        if count >= CEREMONY_CAPACITY {
            transaction.commit()?;
            return Err(HumanAccessStoreError::CapacityExceeded);
        }
        require_active_installation(
            &transaction,
            ceremony.trailbase_instance_id(),
            ceremony.activation_generation(),
        )?;
        let result = transaction.execute(
            r#"
            INSERT INTO auth_ceremonies(
                operation_id, purpose, protocol, trailbase_instance_id,
                activation_generation, browser_binding_digest, callback_path,
                return_target, correlation_id, state, failure, created_at,
                expires_at, claimed_at, terminal_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11, ?12, NULL, NULL)
            "#,
            params![
                ceremony.id().to_string(),
                ceremony.purpose().as_str(),
                ceremony.protocol().as_str(),
                ceremony.trailbase_instance_id().to_string(),
                i64::try_from(ceremony.activation_generation())
                    .map_err(|_| HumanAccessStoreError::Integrity)?,
                ceremony.browser_binding_digest().as_str(),
                ceremony.callback_path().as_str(),
                ceremony.return_target().as_str(),
                ceremony.correlation_id().to_string(),
                ceremony.state().as_str(),
                timestamp(ceremony.created_at()),
                timestamp(ceremony.expires_at()),
            ],
        );
        map_insert(result)?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn claim_auth_ceremony(
        &self,
        browser_binding_digest: &Sha256Digest,
        instance_id: TrailBaseInstanceId,
        activation_generation: u64,
        callback_path: &AuthCallbackPath,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<AuthCeremony> {
        self.maintain_auth_ceremonies(correlation_id, at)?;
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        require_active_installation(&transaction, instance_id, activation_generation)?;
        let mut ceremony = load_ceremony_by_binding(&transaction, browser_binding_digest)?
            .ok_or(HumanAccessStoreError::NotFound)?;
        let prior_state = ceremony.state();
        ceremony.claim(
            browser_binding_digest,
            instance_id,
            activation_generation,
            callback_path,
            at,
        )?;
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyClaimed,
            &ceremony,
            correlation_id,
            at,
        )?;
        transaction.commit()?;
        Ok(ceremony)
    }

    pub(crate) fn cancel_auth_ceremony(
        &self,
        operation_id: OperationId,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<AuthCeremony> {
        self.maintain_auth_ceremonies(correlation_id, at)?;
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;
        let mut ceremony =
            load_ceremony(&transaction, operation_id)?.ok_or(HumanAccessStoreError::NotFound)?;
        let prior_state = ceremony.state();
        ceremony.cancel(at)?;
        persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyCancelled,
            &ceremony,
            correlation_id,
            at,
        )?;
        transaction.commit()?;
        Ok(ceremony)
    }

    pub(crate) fn persist_first_administrator(
        &self,
        operation_id: OperationId,
        trailbase_subject: TrailBaseSubject,
        correlation_id: RequestCorrelationId,
        at: DateTime<Utc>,
    ) -> StoreResult<BootstrapFirstAdministratorOutcome> {
        let observed_root =
            Sha256Digest::from_bytes(&sha256_bytes(self.data_root_identity().as_bytes()));
        let subject_id = AuthSubjectId::new_v7();
        let membership_id = MembershipId::new_v7();
        let connection = self
            .inner
            .connection
            .lock()
            .map_err(|_| HumanAccessStoreError::Integrity)?;
        let transaction = Transaction::new_unchecked(&connection, TransactionBehavior::Immediate)?;

        let workspace_id = load_initialized_workspace(&transaction)?;
        let mut ceremony =
            load_ceremony(&transaction, operation_id)?.ok_or(HumanAccessStoreError::NotFound)?;
        if !matches!(
            (
                ceremony.purpose(),
                ceremony.protocol(),
                ceremony.return_target(),
            ),
            (
                AuthCeremonyPurpose::FirstAdministratorBootstrap,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                AuthReturnTarget::FirstRun,
            )
        ) {
            return Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::InvalidCeremonyTransition,
            ));
        }
        let installation = load_installation(&transaction)?
            .filter(|installation| {
                installation.id() == ceremony.trailbase_instance_id()
                    && installation.activation_generation() == ceremony.activation_generation()
                    && installation.physical_root_identity() == &observed_root
                    && matches!(
                        installation.activation_state(),
                        TrailBaseActivationState::Active
                    )
            })
            .ok_or(HumanAccessStoreError::Integrity)?;
        if matches!(ceremony.state(), AuthCeremonyState::Completed) {
            let matches_completed_bootstrap: bool = transaction.query_row(
                r#"
                SELECT EXISTS(
                    SELECT 1
                    FROM access_audit_events audit
                    JOIN trailbase_auth_anchors anchor
                      ON anchor.trailbase_instance_id = audit.trailbase_instance_id
                     AND anchor.auth_subject_id = audit.auth_subject_id
                    JOIN workspace_memberships membership
                      ON membership.membership_id = audit.membership_id
                     AND membership.auth_subject_id = audit.auth_subject_id
                     AND membership.workspace_id = audit.workspace_id
                    WHERE audit.event_kind = 'first_administrator_bootstrapped'
                      AND audit.operation_id = ?1
                      AND audit.trailbase_instance_id = ?2
                      AND anchor.trailbase_subject = ?3
                )
                "#,
                params![
                    operation_id.to_string(),
                    ceremony.trailbase_instance_id().to_string(),
                    trailbase_subject.as_bytes().as_slice(),
                ],
                |row| row.get(0),
            )?;
            if !matches_completed_bootstrap {
                return Err(HumanAccessStoreError::Integrity);
            }
            transaction.commit()?;
            return Ok(BootstrapFirstAdministratorOutcome::AlreadyBootstrapped);
        }
        if !matches!(ceremony.state(), AuthCeremonyState::Claimed) {
            return Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::InvalidCeremonyTransition,
            ));
        }

        let membership_count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM workspace_memberships", [], |row| {
                row.get(0)
            })?;
        if membership_count != 0 {
            transaction.commit()?;
            return Ok(BootstrapFirstAdministratorOutcome::AlreadyBootstrapped);
        }
        let anchor_conflict: bool = transaction.query_row(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM trailbase_auth_anchors
                WHERE (trailbase_instance_id = ?1 AND trailbase_subject = ?2)
                   OR auth_subject_id = ?3
            )
            "#,
            params![
                installation.id().to_string(),
                trailbase_subject.as_bytes().as_slice(),
                subject_id.to_string(),
            ],
            |row| row.get(0),
        )?;
        if anchor_conflict {
            return Err(HumanAccessStoreError::Conflict);
        }

        let subject = AuthSubject::try_new(subject_id, AuthSubjectLifecycle::Active, 0, 0, at, at)?;
        let anchor =
            TrailBaseExternalAnchor::new(installation.id(), trailbase_subject, subject.id(), at);
        let membership = WorkspaceMembership::try_new(
            membership_id,
            subject.id(),
            workspace_id,
            MembershipLifecycle::Active,
            WorkspaceRole::Administrator,
            at,
            at,
        )?;
        let prior_ceremony_state = ceremony.state();
        ceremony.complete(at)?;

        map_insert(transaction.execute(
            r#"
            INSERT INTO auth_subjects(
                auth_subject_id, lifecycle, auth_epoch, authorization_epoch,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                subject.id().to_string(),
                subject.lifecycle().as_str(),
                i64::try_from(subject.auth_epoch()).map_err(|_| HumanAccessStoreError::Integrity)?,
                i64::try_from(subject.authorization_epoch())
                    .map_err(|_| HumanAccessStoreError::Integrity)?,
                timestamp(subject.created_at()),
                timestamp(subject.updated_at()),
            ],
        ))?;
        map_insert(transaction.execute(
            r#"
            INSERT INTO trailbase_auth_anchors(
                trailbase_instance_id, trailbase_subject, auth_subject_id, linked_at
            ) VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                anchor.trailbase_instance_id().to_string(),
                anchor.trailbase_subject().as_bytes().as_slice(),
                anchor.auth_subject_id().to_string(),
                timestamp(anchor.linked_at()),
            ],
        ))?;
        map_insert(transaction.execute(
            r#"
            INSERT INTO workspace_memberships(
                membership_id, auth_subject_id, workspace_id, lifecycle,
                role, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                membership.id().to_string(),
                membership.subject_id().to_string(),
                membership.workspace_id().to_string(),
                membership.lifecycle().as_str(),
                membership.role().as_str(),
                timestamp(membership.created_at()),
                timestamp(membership.updated_at()),
            ],
        ))?;
        persist_ceremony_transition(&transaction, &ceremony, prior_ceremony_state)?;
        insert_anchor_audit(&transaction, &anchor, correlation_id, at)?;
        insert_first_administrator_audit(
            &transaction,
            &installation,
            &subject,
            &membership,
            ceremony.id(),
            correlation_id,
            at,
        )?;
        insert_ceremony_audit(
            &transaction,
            AccessAuditEventKind::CeremonyCompleted,
            &ceremony,
            correlation_id,
            at,
        )?;
        transaction.commit()?;
        Ok(BootstrapFirstAdministratorOutcome::Created {
            subject_id: subject.id(),
            membership_id: membership.id(),
            workspace_id,
        })
    }
}

pub(crate) fn recover_auth_ceremonies_on_open(
    connection: &Connection,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<u64> {
    recover_auth_ceremonies(connection, correlation_id, at)
}

fn recover_auth_ceremonies(
    connection: &Connection,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<u64> {
    let transaction = Transaction::new_unchecked(connection, TransactionBehavior::Immediate)?;
    let ceremonies = load_live_ceremonies(&transaction)?;
    let mut recovered = 0_u64;
    for mut ceremony in ceremonies {
        let prior_state = ceremony.state();
        if ceremony.recover_after_restart(at)? {
            persist_ceremony_transition(&transaction, &ceremony, prior_state)?;
            insert_ceremony_audit(
                &transaction,
                match ceremony.state() {
                    AuthCeremonyState::Failed => AccessAuditEventKind::CeremonyFailed,
                    AuthCeremonyState::CleanupUncertain => {
                        AccessAuditEventKind::CeremonyCleanupUncertain
                    }
                    _ => return Err(HumanAccessStoreError::Integrity),
                },
                &ceremony,
                correlation_id,
                at,
            )?;
            recovered = recovered
                .checked_add(1)
                .ok_or(HumanAccessStoreError::Integrity)?;
        }
    }
    prune_terminal_ceremonies(&transaction, at)?;
    prune_audit_age(&transaction, at)?;
    prune_audit_overflow(&transaction)?;
    transaction.commit()?;
    Ok(recovered)
}

fn maintain_ceremonies(
    transaction: &Transaction<'_>,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<Vec<OperationId>> {
    let ceremonies = load_expired_pending_ceremonies(transaction, at)?;
    let mut expired = Vec::with_capacity(ceremonies.len());
    for mut ceremony in ceremonies {
        let prior_state = ceremony.state();
        if ceremony.expire(at)? {
            persist_ceremony_transition(transaction, &ceremony, prior_state)?;
            insert_ceremony_audit(
                transaction,
                AccessAuditEventKind::CeremonyExpired,
                &ceremony,
                correlation_id,
                at,
            )?;
            expired.push(ceremony.id());
        }
    }
    prune_terminal_ceremonies(transaction, at)?;
    prune_audit_age(transaction, at)?;
    prune_audit_overflow(transaction)?;
    Ok(expired)
}

fn load_installation(transaction: &Transaction<'_>) -> StoreResult<Option<TrailBaseInstallation>> {
    let row = transaction
        .query_row(
            r#"
            SELECT trailbase_instance_id, physical_root_identity, activation_state,
                   activation_blocker, activation_generation, created_at, updated_at
            FROM trailbase_installation WHERE singleton = 1
            "#,
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()?;
    row.map(|(id, root, state, blocker, generation, created, updated)| {
        TrailBaseInstallation::try_from_persisted(
            id.parse().map_err(|_| HumanAccessStoreError::Integrity)?,
            root.parse().map_err(|_| HumanAccessStoreError::Integrity)?,
            TrailBaseActivationState::from_storage(&state, blocker.as_deref())
                .ok_or(HumanAccessStoreError::Integrity)?,
            u64::try_from(generation).map_err(|_| HumanAccessStoreError::Integrity)?,
            parse_time(&created)?,
            parse_time(&updated)?,
        )
        .map_err(|_| HumanAccessStoreError::Integrity)
    })
    .transpose()
}

fn load_initialized_workspace(transaction: &Transaction<'_>) -> StoreResult<WorkspaceId> {
    transaction
        .query_row(
            r#"
            SELECT node.workspace_id
            FROM node_state node
            JOIN workspaces workspace ON workspace.workspace_id = node.workspace_id
            WHERE node.singleton = 1 AND node.initialized = 1
              AND node.recovery_restore_attempt_id IS NULL
            "#,
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(HumanAccessStoreError::Integrity)?
        .parse()
        .map_err(|_| HumanAccessStoreError::Integrity)
}

fn persist_installation(
    transaction: &Transaction<'_>,
    installation: &TrailBaseInstallation,
    insert: bool,
) -> StoreResult<()> {
    let generation = i64::try_from(installation.activation_generation())
        .map_err(|_| HumanAccessStoreError::Integrity)?;
    let (state, blocker) = match installation.activation_state() {
        TrailBaseActivationState::Inactive => ("inactive", None),
        TrailBaseActivationState::Active => ("active", None),
        TrailBaseActivationState::Blocked(blocker) => ("blocked", Some(blocker.as_str())),
    };
    let changed = if insert {
        transaction.execute(
            r#"
            INSERT INTO trailbase_installation(
                singleton, trailbase_instance_id, physical_root_identity,
                activation_state, activation_blocker, activation_generation,
                created_at, updated_at
            ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                installation.id().to_string(),
                installation.physical_root_identity().as_str(),
                state,
                blocker,
                generation,
                timestamp(installation.created_at()),
                timestamp(installation.updated_at()),
            ],
        )?
    } else {
        transaction.execute(
            r#"
            UPDATE trailbase_installation
            SET activation_state = ?1, activation_blocker = ?2,
                activation_generation = ?3, updated_at = ?4
            WHERE singleton = 1 AND trailbase_instance_id = ?5
            "#,
            params![
                state,
                blocker,
                generation,
                timestamp(installation.updated_at()),
                installation.id().to_string(),
            ],
        )?
    };
    if changed == 1 {
        Ok(())
    } else {
        Err(HumanAccessStoreError::Integrity)
    }
}

fn require_active_installation(
    transaction: &Transaction<'_>,
    instance_id: TrailBaseInstanceId,
    generation: u64,
) -> StoreResult<()> {
    let generation = i64::try_from(generation).map_err(|_| HumanAccessStoreError::Integrity)?;
    let active: bool = transaction.query_row(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM trailbase_installation
            WHERE singleton = 1 AND trailbase_instance_id = ?1
              AND activation_state = 'active' AND activation_generation = ?2
        )
        "#,
        params![instance_id.to_string(), generation],
        |row| row.get(0),
    )?;
    if active {
        Ok(())
    } else {
        Err(HumanAccessStoreError::Integrity)
    }
}

fn load_ceremony(
    transaction: &Transaction<'_>,
    operation_id: OperationId,
) -> StoreResult<Option<AuthCeremony>> {
    load_ceremony_where(transaction, "operation_id = ?1", operation_id.to_string())
}

fn load_ceremony_by_binding(
    transaction: &Transaction<'_>,
    digest: &Sha256Digest,
) -> StoreResult<Option<AuthCeremony>> {
    load_ceremony_where(
        transaction,
        "browser_binding_digest = ?1",
        digest.as_str().to_owned(),
    )
}

fn load_ceremony_where(
    transaction: &Transaction<'_>,
    predicate: &str,
    value: String,
) -> StoreResult<Option<AuthCeremony>> {
    let sql = format!(
        "SELECT operation_id, purpose, protocol, trailbase_instance_id, activation_generation, browser_binding_digest, callback_path, return_target, correlation_id, state, failure, created_at, expires_at, claimed_at, terminal_at FROM auth_ceremonies WHERE {predicate}"
    );
    transaction
        .query_row(&sql, [value], read_ceremony_row)
        .optional()?
        .map(parse_ceremony)
        .transpose()
}

fn load_live_ceremonies(transaction: &Transaction<'_>) -> StoreResult<Vec<AuthCeremony>> {
    load_ceremonies(
        transaction,
        "WHERE state IN ('pending', 'claimed') ORDER BY operation_id",
        [],
    )
}

fn load_expired_pending_ceremonies(
    transaction: &Transaction<'_>,
    at: DateTime<Utc>,
) -> StoreResult<Vec<AuthCeremony>> {
    load_ceremonies(
        transaction,
        "WHERE state = 'pending' AND expires_at <= ?1 ORDER BY expires_at, operation_id",
        [timestamp(at)],
    )
}

fn load_ceremonies<P>(
    transaction: &Transaction<'_>,
    suffix: &str,
    params: P,
) -> StoreResult<Vec<AuthCeremony>>
where
    P: rusqlite::Params,
{
    let sql = format!(
        "SELECT operation_id, purpose, protocol, trailbase_instance_id, activation_generation, browser_binding_digest, callback_path, return_target, correlation_id, state, failure, created_at, expires_at, claimed_at, terminal_at FROM auth_ceremonies {suffix}"
    );
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement
        .query_map(params, read_ceremony_row)?
        .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(parse_ceremony).collect()
}

fn read_ceremony_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CeremonyRow> {
    Ok(CeremonyRow {
        operation_id: row.get(0)?,
        purpose: row.get(1)?,
        protocol: row.get(2)?,
        trailbase_instance_id: row.get(3)?,
        activation_generation: row.get(4)?,
        browser_binding_digest: row.get(5)?,
        callback_path: row.get(6)?,
        return_target: row.get(7)?,
        correlation_id: row.get(8)?,
        state: row.get(9)?,
        failure: row.get(10)?,
        created_at: row.get(11)?,
        expires_at: row.get(12)?,
        claimed_at: row.get(13)?,
        terminal_at: row.get(14)?,
    })
}

fn parse_ceremony(row: CeremonyRow) -> StoreResult<AuthCeremony> {
    let failure = row
        .failure
        .as_deref()
        .map(|value| {
            AuthCeremonyFailure::from_storage(value).ok_or(HumanAccessStoreError::Integrity)
        })
        .transpose()?;
    AuthCeremony::try_from_persisted(
        row.operation_id
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        AuthCeremonyPurpose::from_storage(&row.purpose).ok_or(HumanAccessStoreError::Integrity)?,
        AuthCeremonyProtocol::from_storage(&row.protocol)
            .ok_or(HumanAccessStoreError::Integrity)?,
        row.trailbase_instance_id
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        u64::try_from(row.activation_generation).map_err(|_| HumanAccessStoreError::Integrity)?,
        row.browser_binding_digest
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        AuthCallbackPath::parse(row.callback_path).map_err(|_| HumanAccessStoreError::Integrity)?,
        AuthReturnTarget::from_storage(&row.return_target)
            .ok_or(HumanAccessStoreError::Integrity)?,
        row.correlation_id
            .parse()
            .map_err(|_| HumanAccessStoreError::Integrity)?,
        AuthCeremonyState::from_storage(&row.state).ok_or(HumanAccessStoreError::Integrity)?,
        failure,
        parse_time(&row.created_at)?,
        parse_time(&row.expires_at)?,
        row.claimed_at.as_deref().map(parse_time).transpose()?,
        row.terminal_at.as_deref().map(parse_time).transpose()?,
    )
    .map_err(|_| HumanAccessStoreError::Integrity)
}

fn persist_ceremony_transition(
    transaction: &Transaction<'_>,
    ceremony: &AuthCeremony,
    prior_state: AuthCeremonyState,
) -> StoreResult<()> {
    let changed = transaction.execute(
        r#"
        UPDATE auth_ceremonies
        SET state = ?1, failure = ?2, claimed_at = ?3, terminal_at = ?4
        WHERE operation_id = ?5 AND state = ?6
        "#,
        params![
            ceremony.state().as_str(),
            ceremony.failure().map(AuthCeremonyFailure::as_str),
            ceremony.claimed_at().map(timestamp),
            ceremony.terminal_at().map(timestamp),
            ceremony.id().to_string(),
            prior_state.as_str(),
        ],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(HumanAccessStoreError::Conflict)
    }
}

fn insert_installation_audit(
    transaction: &Transaction<'_>,
    event: AccessAuditEventKind,
    instance_id: TrailBaseInstanceId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4)
        "#,
        params![
            event.as_str(),
            instance_id.to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn insert_anchor_audit(
    transaction: &Transaction<'_>,
    anchor: &TrailBaseExternalAnchor,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, auth_subject_id,
            correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            AccessAuditEventKind::AnchorLinked.as_str(),
            anchor.trailbase_instance_id().to_string(),
            anchor.auth_subject_id().to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_first_administrator_audit(
    transaction: &Transaction<'_>,
    installation: &TrailBaseInstallation,
    subject: &AuthSubject,
    membership: &WorkspaceMembership,
    operation_id: OperationId,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, auth_subject_id, workspace_id,
            membership_id, operation_id, correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            AccessAuditEventKind::FirstAdministratorBootstrapped.as_str(),
            installation.id().to_string(),
            subject.id().to_string(),
            membership.workspace_id().to_string(),
            membership.id().to_string(),
            operation_id.to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn insert_ceremony_audit(
    transaction: &Transaction<'_>,
    event: AccessAuditEventKind,
    ceremony: &AuthCeremony,
    correlation_id: RequestCorrelationId,
    at: DateTime<Utc>,
) -> StoreResult<()> {
    prune_audit_age(transaction, at)?;
    transaction.execute(
        r#"
        INSERT INTO access_audit_events(
            event_kind, trailbase_instance_id, operation_id,
            correlation_id, occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![
            event.as_str(),
            ceremony.trailbase_instance_id().to_string(),
            ceremony.id().to_string(),
            correlation_id.to_string(),
            timestamp(at),
        ],
    )?;
    prune_audit_overflow(transaction)?;
    Ok(())
}

fn prune_terminal_ceremonies(transaction: &Transaction<'_>, at: DateTime<Utc>) -> StoreResult<()> {
    let cutoff = at
        .checked_sub_signed(Duration::hours(CEREMONY_RETENTION_HOURS))
        .ok_or(HumanAccessStoreError::Integrity)?;
    transaction.execute(
        "DELETE FROM auth_ceremonies WHERE terminal_at IS NOT NULL AND terminal_at <= ?1",
        [timestamp(cutoff)],
    )?;
    Ok(())
}

fn prune_audit_age(transaction: &Transaction<'_>, at: DateTime<Utc>) -> StoreResult<()> {
    let cutoff = at
        .checked_sub_signed(Duration::days(AUDIT_RETENTION_DAYS))
        .ok_or(HumanAccessStoreError::Integrity)?;
    transaction.execute(
        "DELETE FROM access_audit_events WHERE occurred_at <= ?1",
        [timestamp(cutoff)],
    )?;
    Ok(())
}

fn prune_audit_overflow(transaction: &Transaction<'_>) -> StoreResult<()> {
    transaction.execute(
        r#"
        DELETE FROM access_audit_events
        WHERE audit_event_id IN (
            SELECT audit_event_id FROM access_audit_events
            ORDER BY occurred_at DESC, audit_event_id DESC
            LIMIT -1 OFFSET ?1
        )
        "#,
        [AUDIT_CAPACITY],
    )?;
    Ok(())
}

fn map_insert(result: rusqlite::Result<usize>) -> StoreResult<usize> {
    result.map_err(|error| match &error {
        rusqlite::Error::SqliteFailure(detail, _)
            if detail.code == ErrorCode::ConstraintViolation =>
        {
            HumanAccessStoreError::Conflict
        }
        _ => HumanAccessStoreError::Storage(error),
    })
}

fn parse_time(value: &str) -> StoreResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| HumanAccessStoreError::Integrity)
}

impl HumanAccessPort for SqliteKernel {
    fn verify_trailbase_installation(
        &self,
        command: VerifyTrailBaseInstallationCommand,
    ) -> ApplicationResult<TrailBaseInstallation> {
        SqliteKernel::verify_trailbase_installation(
            self,
            command.instance_id(),
            command.release_matches(),
            command.declared_restore(),
            command.correlation_id(),
            command.at(),
        )
        .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn start_auth_ceremony(&self, command: StartAuthCeremonyCommand) -> ApplicationResult<()> {
        self.insert_auth_ceremony(command.ceremony())
            .map_err(|error| access_problem(error, command.ceremony().correlation_id()))
    }

    fn claim_auth_ceremony(
        &self,
        command: ClaimAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony> {
        SqliteKernel::claim_auth_ceremony(
            self,
            command.browser_binding_digest(),
            command.instance_id(),
            command.activation_generation(),
            command.callback_path(),
            command.correlation_id(),
            command.at(),
        )
        .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn cancel_auth_ceremony(
        &self,
        command: CancelAuthCeremonyCommand,
    ) -> ApplicationResult<AuthCeremony> {
        SqliteKernel::cancel_auth_ceremony(
            self,
            command.operation_id(),
            command.correlation_id(),
            command.at(),
        )
        .map_err(|error| access_problem(error, command.correlation_id()))
    }

    fn bootstrap_first_administrator(
        &self,
        command: BootstrapFirstAdministratorCommand,
    ) -> ApplicationResult<BootstrapFirstAdministratorOutcome> {
        let expected = AccessAdministrationPort::ensure_bootstrap_secret(self).map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                CapabilityKey::CreateBrowserSession,
                command.correlation_id(),
            ))
        })?;
        if !command.bootstrap_secret().constant_time_eq(&expected) {
            return Err(Box::new(FastiProblem::forbidden(
                CapabilityKey::CreateBrowserSession,
                command.correlation_id(),
            )));
        }
        self.persist_first_administrator(
            command.operation_id(),
            command.trailbase_subject(),
            command.correlation_id(),
            command.at(),
        )
        .map_err(|error| access_problem(error, command.correlation_id()))
    }
}

fn access_problem(
    error: HumanAccessStoreError,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    let code = match error {
        HumanAccessStoreError::Storage(_) => ProblemCode::StorageUnavailable,
        HumanAccessStoreError::Integrity
        | HumanAccessStoreError::Invariant(
            AccessInvariantError::TrailBaseInstallationBlocked
            | AccessInvariantError::ActivationGenerationOverflow
            | AccessInvariantError::EpochOverflow,
        ) => ProblemCode::IntegrityFailed,
        HumanAccessStoreError::CapacityExceeded => ProblemCode::CapacityExceeded,
        HumanAccessStoreError::NotFound
        | HumanAccessStoreError::Conflict
        | HumanAccessStoreError::Invariant(
            AccessInvariantError::CeremonyBindingMismatch
            | AccessInvariantError::CeremonyInstallationMismatch
            | AccessInvariantError::CeremonyGenerationMismatch
            | AccessInvariantError::CeremonyCallbackMismatch,
        ) => ProblemCode::Forbidden,
        HumanAccessStoreError::Invariant(_) => ProblemCode::ValidationFailed,
    };
    Box::new(FastiProblem::from_code(
        code,
        CapabilityKey::CreateBrowserSession,
        correlation_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser_auth::viable_administrator_count;
    use crate::test_support::TestNode;
    use fasti_application::SecretMaterial;
    use std::sync::{Arc, Barrier};

    fn at(minutes: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-30T00:00:00Z")
            .expect("fixed time")
            .with_timezone(&Utc)
            + Duration::minutes(minutes)
    }

    fn active_kernel() -> (tempfile::TempDir, SqliteKernel, TrailBaseInstallation) {
        let root = tempfile::tempdir().expect("temporary root");
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let installation = kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("active installation");
        (root, kernel, installation)
    }

    fn ceremony(installation: &TrailBaseInstallation, created: i64, expires: i64) -> AuthCeremony {
        AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::SignIn,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            installation.id(),
            installation.activation_generation(),
            Sha256Digest::from_bytes(&sha256_bytes(OperationId::new_v7().to_string().as_bytes())),
            AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback"),
            AuthReturnTarget::ApplicationHome,
            RequestCorrelationId::new_v7(),
            at(created),
            at(expires),
        )
        .expect("ceremony")
    }

    fn bootstrap_ceremony(installation: &TrailBaseInstallation, binding_byte: u8) -> AuthCeremony {
        AuthCeremony::try_new(
            OperationId::new_v7(),
            AuthCeremonyPurpose::FirstAdministratorBootstrap,
            AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
            installation.id(),
            installation.activation_generation(),
            Sha256Digest::from_bytes(&[binding_byte; 32]),
            AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback"),
            AuthReturnTarget::FirstRun,
            RequestCorrelationId::new_v7(),
            at(1),
            at(10),
        )
        .expect("bootstrap ceremony")
    }

    fn claim_bootstrap(
        kernel: &SqliteKernel,
        installation: &TrailBaseInstallation,
        ceremony: &AuthCeremony,
    ) {
        kernel
            .insert_auth_ceremony(ceremony)
            .expect("insert bootstrap ceremony");
        kernel
            .claim_auth_ceremony(
                ceremony.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path(),
                RequestCorrelationId::new_v7(),
                at(2),
            )
            .expect("claim bootstrap ceremony");
    }

    fn insert_pending_rows(
        kernel: &SqliteKernel,
        installation: &TrailBaseInstallation,
        count: usize,
    ) {
        let connection = kernel.inner.connection.lock().expect("connection");
        let transaction = connection.unchecked_transaction().expect("transaction");
        {
            let mut statement = transaction
                .prepare(
                    r#"
                    INSERT INTO auth_ceremonies(
                        operation_id, purpose, protocol, trailbase_instance_id,
                        activation_generation, browser_binding_digest, callback_path,
                        return_target, correlation_id, state, failure, created_at,
                        expires_at, claimed_at, terminal_at
                    ) VALUES (?1, 'sign_in', 'trailbase_authorization_code_pkce', ?2, ?3, ?4,
                              '/auth/trailbase/callback', 'application_home', ?5, 'pending',
                              NULL, ?6, ?7, NULL, NULL)
                    "#,
                )
                .expect("prepare insert");
            for sequence in 0..count {
                statement
                    .execute(params![
                        OperationId::new_v7().to_string(),
                        installation.id().to_string(),
                        i64::try_from(installation.activation_generation())
                            .expect("generation fits"),
                        Sha256Digest::from_bytes(&sha256_bytes(&sequence.to_be_bytes())).as_str(),
                        RequestCorrelationId::new_v7().to_string(),
                        timestamp(at(1)),
                        timestamp(at(10)),
                    ])
                    .expect("insert pending row");
            }
        }
        transaction.commit().expect("commit pending rows");
    }

    #[test]
    fn installation_is_bound_to_physical_root_and_declared_restore_is_terminal_in_c1() {
        let (_root, kernel, active) = active_kernel();
        assert_eq!(active.activation_state(), TrailBaseActivationState::Active);
        assert_eq!(active.activation_generation(), 1);

        let blocked = kernel
            .verify_trailbase_installation(
                active.id(),
                true,
                true,
                RequestCorrelationId::new_v7(),
                at(1),
            )
            .expect("declare restore");
        assert!(matches!(
            blocked.activation_state(),
            TrailBaseActivationState::Blocked(_)
        ));
        assert_eq!(blocked.activation_generation(), 2);
        assert!(matches!(
            kernel.verify_trailbase_installation(
                active.id(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(2),
            ),
            Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::TrailBaseInstallationBlocked
            ))
        ));
    }

    #[test]
    fn ceremony_claim_has_one_winner_and_cancellation_rejects_claimed_rows() {
        let (_root, kernel, installation) = active_kernel();
        let ceremony = ceremony(&installation, 1, 10);
        kernel
            .insert_auth_ceremony(&ceremony)
            .expect("insert ceremony");
        let claimed = kernel
            .claim_auth_ceremony(
                ceremony.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path(),
                RequestCorrelationId::new_v7(),
                at(2),
            )
            .expect("claim ceremony");
        assert_eq!(claimed.state(), AuthCeremonyState::Claimed);
        assert!(matches!(
            kernel.claim_auth_ceremony(
                ceremony.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                ceremony.callback_path(),
                RequestCorrelationId::new_v7(),
                at(3),
            ),
            Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::InvalidCeremonyTransition
            ))
        ));
        assert!(matches!(
            kernel.cancel_auth_ceremony(ceremony.id(), RequestCorrelationId::new_v7(), at(3),),
            Err(HumanAccessStoreError::Invariant(
                AccessInvariantError::InvalidCeremonyTransition
            ))
        ));
    }

    #[test]
    fn concurrent_claims_grant_exactly_one_exchange_permission() {
        let (_root, kernel, installation) = active_kernel();
        let ceremony = ceremony(&installation, 1, 10);
        kernel
            .insert_auth_ceremony(&ceremony)
            .expect("insert ceremony");
        let barrier = Arc::new(Barrier::new(3));
        let instance_id = installation.id();
        let activation_generation = installation.activation_generation();
        let mut workers = Vec::new();
        for _ in 0..2 {
            let kernel = kernel.clone();
            let barrier = Arc::clone(&barrier);
            let digest = ceremony.browser_binding_digest().clone();
            let callback = ceremony.callback_path().clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                kernel.claim_auth_ceremony(
                    &digest,
                    instance_id,
                    activation_generation,
                    &callback,
                    RequestCorrelationId::new_v7(),
                    at(2),
                )
            }));
        }
        barrier.wait();
        let results: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("claim worker"))
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    matches!(
                        result,
                        Err(HumanAccessStoreError::Invariant(
                            AccessInvariantError::InvalidCeremonyTransition
                        ))
                    )
                })
                .count(),
            1
        );
    }

    #[test]
    fn ceremony_capacity_rejects_the_row_after_the_exact_ten_thousand_bound() {
        let (_root, kernel, installation) = active_kernel();
        insert_pending_rows(&kernel, &installation, 9_999);
        kernel
            .insert_auth_ceremony(&ceremony(&installation, 1, 10))
            .expect("ten-thousandth row");
        assert!(matches!(
            kernel.insert_auth_ceremony(&ceremony(&installation, 1, 10)),
            Err(HumanAccessStoreError::CapacityExceeded)
        ));
        let connection = kernel.inner.connection.lock().expect("connection");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM auth_ceremonies", [], |row| row.get(0))
            .expect("ceremony count");
        assert_eq!(count, CEREMONY_CAPACITY);
    }

    #[test]
    fn terminal_and_audit_retention_use_the_exact_frozen_boundaries() {
        let (_root, kernel, installation) = active_kernel();
        let terminal = ceremony(&installation, 1, 3);
        kernel
            .insert_auth_ceremony(&terminal)
            .expect("insert ceremony");
        kernel
            .cancel_auth_ceremony(terminal.id(), RequestCorrelationId::new_v7(), at(2))
            .expect("cancel ceremony");

        kernel
            .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(2 + 24 * 60 - 1))
            .expect("retain before boundary");
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM auth_ceremonies WHERE operation_id = ?1",
                    [terminal.id().to_string()],
                    |row| row.get(0),
                )
                .expect("terminal count");
            assert_eq!(count, 1);
        }
        kernel
            .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(2 + 24 * 60))
            .expect("prune at boundary");
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM auth_ceremonies WHERE operation_id = ?1",
                    [terminal.id().to_string()],
                    |row| row.get(0),
                )
                .expect("terminal count");
            assert_eq!(count, 0);
        }

        kernel
            .maintain_auth_ceremonies(
                RequestCorrelationId::new_v7(),
                at(AUDIT_RETENTION_DAYS * 24 * 60 - 1),
            )
            .expect("retain audit before boundary");
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM access_audit_events WHERE occurred_at = ?1",
                    [timestamp(at(0))],
                    |row| row.get(0),
                )
                .expect("old audit count");
            assert_eq!(count, 1);
        }
        kernel
            .maintain_auth_ceremonies(
                RequestCorrelationId::new_v7(),
                at(AUDIT_RETENTION_DAYS * 24 * 60),
            )
            .expect("prune audit at boundary");
        let connection = kernel.inner.connection.lock().expect("connection");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE occurred_at = ?1",
                [timestamp(at(0))],
                |row| row.get(0),
            )
            .expect("old audit count");
        assert_eq!(count, 0);
    }

    #[test]
    fn audit_overflow_keeps_the_newest_ten_thousand_events() {
        let (_root, kernel, installation) = active_kernel();
        {
            let connection = kernel.inner.connection.lock().expect("connection");
            let transaction = connection.unchecked_transaction().expect("transaction");
            {
                let mut statement = transaction
                    .prepare(
                        r#"
                        INSERT INTO access_audit_events(
                            event_kind, trailbase_instance_id, correlation_id, occurred_at
                        ) VALUES ('trailbase_activated', ?1, ?2, ?3)
                        "#,
                    )
                    .expect("prepare audit insert");
                for _ in 1..AUDIT_CAPACITY {
                    statement
                        .execute(params![
                            installation.id().to_string(),
                            RequestCorrelationId::new_v7().to_string(),
                            timestamp(at(0)),
                        ])
                        .expect("insert audit");
                }
            }
            transaction.commit().expect("commit audits");
        }
        kernel
            .verify_trailbase_installation(
                installation.id(),
                false,
                false,
                RequestCorrelationId::new_v7(),
                at(1),
            )
            .expect("block mismatched release");
        let connection = kernel.inner.connection.lock().expect("connection");
        let (count, newest_kind, oldest_id): (i64, String, i64) = connection
            .query_row(
                r#"
                SELECT COUNT(*),
                       (SELECT event_kind FROM access_audit_events
                        ORDER BY occurred_at DESC, audit_event_id DESC LIMIT 1),
                       (SELECT MIN(audit_event_id) FROM access_audit_events)
                FROM access_audit_events
                "#,
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("audit inventory");
        assert_eq!(count, AUDIT_CAPACITY);
        assert_eq!(newest_kind, AccessAuditEventKind::TrailBaseBlocked.as_str());
        assert_eq!(oldest_id, 2);
    }

    #[test]
    fn first_administrator_bootstrap_is_atomic_and_adds_no_profile_authority() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let ceremony = bootstrap_ceremony(&installation, 21);
        claim_bootstrap(&node.kernel, &installation, &ceremony);
        let baseline: (i64, i64, i64, i64) = {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM profiles), (SELECT COUNT(*) FROM profile_grants), (SELECT COUNT(*) FROM grant_scopes), (SELECT COUNT(*) FROM clients)",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .expect("baseline")
        };
        let outcome = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                ceremony.id(),
                TrailBaseSubject::from_bytes([7; 16]),
                RequestCorrelationId::new_v7(),
                at(3),
            ),
        )
        .expect("first administrator");
        let BootstrapFirstAdministratorOutcome::Created {
            subject_id,
            membership_id,
            workspace_id,
        } = outcome
        else {
            panic!("first bootstrap must create the administrator");
        };

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let persisted: (String, Vec<u8>, String, String, String, String, String) = connection
            .query_row(
                r#"
                SELECT subject.lifecycle, anchor.trailbase_subject,
                       membership.lifecycle, membership.role, ceremony.state,
                       membership.workspace_id, membership.membership_id
                FROM auth_subjects subject
                JOIN trailbase_auth_anchors anchor
                  ON anchor.auth_subject_id = subject.auth_subject_id
                JOIN workspace_memberships membership
                  ON membership.auth_subject_id = subject.auth_subject_id
                JOIN auth_ceremonies ceremony ON ceremony.operation_id = ?1
                WHERE subject.auth_subject_id = ?2
                "#,
                params![ceremony.id().to_string(), subject_id.to_string()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .expect("persisted bootstrap");
        assert_eq!(persisted.0, "active");
        assert_eq!(persisted.1, vec![7; 16]);
        assert_eq!(persisted.2, "active");
        assert_eq!(persisted.3, "administrator");
        assert_eq!(persisted.4, "completed");
        assert_eq!(persisted.5, workspace_id.to_string());
        assert_eq!(persisted.6, membership_id.to_string());
        assert_eq!(
            viable_administrator_count(
                &connection,
                &workspace_id.to_string(),
                CapabilityKey::CreateBrowserSession,
                RequestCorrelationId::new_v7(),
            )
            .expect("viable admins"),
            1
        );
        let authority_counts: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM profiles), (SELECT COUNT(*) FROM profile_grants), (SELECT COUNT(*) FROM grant_scopes), (SELECT COUNT(*) FROM clients)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("authority counts");
        assert_eq!(authority_counts, baseline);
        let bootstrap_audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('anchor_linked', 'first_administrator_bootstrapped', 'ceremony_completed')",
                [],
                |row| row.get(0),
            )
            .expect("bootstrap audits");
        assert_eq!(bootstrap_audits, 3);
        drop(connection);

        let retry = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                ceremony.id(),
                TrailBaseSubject::from_bytes([7; 16]),
                RequestCorrelationId::new_v7(),
                at(4),
            ),
        )
        .expect("bootstrap retry");
        assert_eq!(
            retry,
            BootstrapFirstAdministratorOutcome::AlreadyBootstrapped
        );
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let retry_counts: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM auth_subjects), (SELECT COUNT(*) FROM trailbase_auth_anchors), (SELECT COUNT(*) FROM workspace_memberships), (SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('anchor_linked', 'first_administrator_bootstrapped', 'ceremony_completed'))",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("retry state");
        assert_eq!(retry_counts, (1, 1, 1, 3));
        connection
            .execute(
                "UPDATE trailbase_installation SET activation_state = 'blocked', activation_blocker = 'release_mismatch', updated_at = ?1",
                [timestamp(at(5))],
            )
            .expect("block installation");
        drop(connection);

        let blocked_retry = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                ceremony.id(),
                TrailBaseSubject::from_bytes([7; 16]),
                RequestCorrelationId::new_v7(),
                at(6),
            ),
        )
        .expect_err("blocked installation must fence a completed retry");
        assert_eq!(blocked_retry.code(), ProblemCode::IntegrityFailed);
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let blocked_retry_audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('anchor_linked', 'first_administrator_bootstrapped', 'ceremony_completed')",
                [],
                |row| row.get(0),
            )
            .expect("blocked retry audits");
        assert_eq!(blocked_retry_audits, 3);
    }

    #[test]
    fn wrong_bootstrap_secret_has_no_database_side_effect() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let ceremony = bootstrap_ceremony(&installation, 22);
        claim_bootstrap(&node.kernel, &installation, &ceremony);
        let wrong = SecretMaterial::from_bytes([0; 32]);
        let actual = node
            .kernel
            .ensure_bootstrap_secret()
            .expect("bootstrap secret");
        assert!(!wrong.constant_time_eq(&actual));

        let problem = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                wrong,
                ceremony.id(),
                TrailBaseSubject::from_bytes([8; 16]),
                RequestCorrelationId::new_v7(),
                at(3),
            ),
        )
        .expect_err("wrong secret must fail");
        assert_eq!(problem.code(), ProblemCode::Forbidden);
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let counts: (i64, i64, i64, String) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM auth_subjects), (SELECT COUNT(*) FROM trailbase_auth_anchors), (SELECT COUNT(*) FROM workspace_memberships), (SELECT state FROM auth_ceremonies WHERE operation_id = ?1)",
                [ceremony.id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("unchanged state");
        assert_eq!(counts, (0, 0, 0, "claimed".to_owned()));
    }

    #[test]
    fn concurrent_first_administrator_bootstrap_has_one_writer() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let first = bootstrap_ceremony(&installation, 23);
        let second = bootstrap_ceremony(&installation, 24);
        claim_bootstrap(&node.kernel, &installation, &first);
        claim_bootstrap(&node.kernel, &installation, &second);
        let commands = [
            (
                first.id(),
                TrailBaseSubject::from_bytes([9; 16]),
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
            (
                second.id(),
                TrailBaseSubject::from_bytes([10; 16]),
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
            ),
        ];
        let barrier = Arc::new(Barrier::new(3));
        let mut workers = Vec::new();
        for (operation_id, subject, secret) in commands {
            let kernel = node.kernel.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                HumanAccessPort::bootstrap_first_administrator(
                    &kernel,
                    BootstrapFirstAdministratorCommand::new(
                        secret,
                        operation_id,
                        subject,
                        RequestCorrelationId::new_v7(),
                        at(3),
                    ),
                )
            }));
        }
        barrier.wait();
        let outcomes: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().expect("bootstrap worker").expect("outcome"))
            .collect();
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    BootstrapFirstAdministratorOutcome::Created { .. }
                ))
                .count(),
            1
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| matches!(
                    outcome,
                    BootstrapFirstAdministratorOutcome::AlreadyBootstrapped
                ))
                .count(),
            1
        );
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let counts: (i64, i64, i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM auth_subjects), (SELECT COUNT(*) FROM trailbase_auth_anchors), (SELECT COUNT(*) FROM workspace_memberships), (SELECT COUNT(*) FROM auth_ceremonies WHERE state = 'completed'), (SELECT COUNT(*) FROM auth_ceremonies WHERE state = 'claimed')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("winner counts");
        assert_eq!(counts, (1, 1, 1, 1, 1));
    }

    #[test]
    fn removed_membership_keeps_first_administrator_bootstrap_closed() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let first = bootstrap_ceremony(&installation, 26);
        claim_bootstrap(&node.kernel, &installation, &first);
        HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                first.id(),
                TrailBaseSubject::from_bytes([12; 16]),
                RequestCorrelationId::new_v7(),
                at(3),
            ),
        )
        .expect("initial administrator");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute("UPDATE workspace_memberships SET lifecycle = 'removed'", [])
                .expect("remove membership");
        }

        let second = bootstrap_ceremony(&installation, 27);
        claim_bootstrap(&node.kernel, &installation, &second);
        let outcome = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                second.id(),
                TrailBaseSubject::from_bytes([13; 16]),
                RequestCorrelationId::new_v7(),
                at(4),
            ),
        )
        .expect("closed bootstrap");
        assert_eq!(
            outcome,
            BootstrapFirstAdministratorOutcome::AlreadyBootstrapped
        );
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let counts: (i64, i64, i64, String) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM auth_subjects), (SELECT COUNT(*) FROM trailbase_auth_anchors), (SELECT COUNT(*) FROM workspace_memberships), (SELECT state FROM auth_ceremonies WHERE operation_id = ?1)",
                [second.id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("closed state");
        assert_eq!(counts, (1, 1, 1, "claimed".to_owned()));
    }

    #[test]
    fn bootstrap_rejects_unclaimed_wrong_purpose_and_blocked_installation_without_writes() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let pending = bootstrap_ceremony(&installation, 28);
        node.kernel
            .insert_auth_ceremony(&pending)
            .expect("pending ceremony");
        let pending_problem = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                pending.id(),
                TrailBaseSubject::from_bytes([14; 16]),
                RequestCorrelationId::new_v7(),
                at(2),
            ),
        )
        .expect_err("pending ceremony must fail");
        assert_eq!(pending_problem.code(), ProblemCode::ValidationFailed);

        let sign_in = ceremony(&installation, 1, 10);
        node.kernel
            .insert_auth_ceremony(&sign_in)
            .expect("sign-in ceremony");
        node.kernel
            .claim_auth_ceremony(
                sign_in.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                sign_in.callback_path(),
                RequestCorrelationId::new_v7(),
                at(2),
            )
            .expect("claim sign-in");
        let purpose_problem = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                sign_in.id(),
                TrailBaseSubject::from_bytes([15; 16]),
                RequestCorrelationId::new_v7(),
                at(3),
            ),
        )
        .expect_err("wrong-purpose ceremony must fail");
        assert_eq!(purpose_problem.code(), ProblemCode::ValidationFailed);

        let blocked = bootstrap_ceremony(&installation, 29);
        claim_bootstrap(&node.kernel, &installation, &blocked);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute(
                    "UPDATE trailbase_installation SET activation_state = 'blocked', activation_blocker = 'release_mismatch', updated_at = ?1",
                    [timestamp(at(3))],
                )
                .expect("block installation");
        }
        let blocked_problem = HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                blocked.id(),
                TrailBaseSubject::from_bytes([16; 16]),
                RequestCorrelationId::new_v7(),
                at(4),
            ),
        )
        .expect_err("blocked installation must fail");
        assert_eq!(blocked_problem.code(), ProblemCode::IntegrityFailed);

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let counts: (i64, i64, i64) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM auth_subjects), (SELECT COUNT(*) FROM trailbase_auth_anchors), (SELECT COUNT(*) FROM workspace_memberships)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("identity state");
        assert_eq!(counts, (0, 0, 0));
    }

    #[test]
    fn bootstrap_audit_failure_rolls_back_identity_and_ceremony() {
        let node = TestNode::new();
        let installation = node
            .kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                at(0),
            )
            .expect("installation");
        let ceremony = bootstrap_ceremony(&installation, 25);
        claim_bootstrap(&node.kernel, &installation, &ceremony);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            connection
                .execute_batch(
                    r#"
                    CREATE TRIGGER reject_first_administrator_audit
                    BEFORE INSERT ON access_audit_events
                    WHEN NEW.event_kind = 'first_administrator_bootstrapped'
                    BEGIN
                        SELECT RAISE(ABORT, 'test audit failure');
                    END;
                    "#,
                )
                .expect("failure trigger");
        }
        assert!(HumanAccessPort::bootstrap_first_administrator(
            &node.kernel,
            BootstrapFirstAdministratorCommand::new(
                node.kernel
                    .ensure_bootstrap_secret()
                    .expect("bootstrap secret"),
                ceremony.id(),
                TrailBaseSubject::from_bytes([11; 16]),
                RequestCorrelationId::new_v7(),
                at(3),
            ),
        )
        .is_err());
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let counts: (i64, i64, i64, String) = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM auth_subjects), (SELECT COUNT(*) FROM trailbase_auth_anchors), (SELECT COUNT(*) FROM workspace_memberships), (SELECT state FROM auth_ceremonies WHERE operation_id = ?1)",
                [ceremony.id().to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("rolled back state");
        assert_eq!(counts, (0, 0, 0, "claimed".to_owned()));
    }

    #[test]
    fn maintenance_expires_pending_only_and_restart_recovers_each_live_state_once() {
        let (_root, kernel, installation) = active_kernel();
        let pending = ceremony(&installation, 1, 2);
        kernel
            .insert_auth_ceremony(&pending)
            .expect("insert pending");
        assert_eq!(
            kernel
                .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(2))
                .expect("maintain"),
            vec![pending.id()]
        );
        assert!(kernel
            .maintain_auth_ceremonies(RequestCorrelationId::new_v7(), at(3))
            .expect("idempotent maintenance")
            .is_empty());
    }

    #[test]
    fn opening_the_kernel_recovers_live_ceremonies_once_before_exposure() {
        let root = tempfile::tempdir().expect("temporary root");
        let started = Utc::now();
        let kernel = SqliteKernel::open(root.path()).expect("kernel");
        let installation = kernel
            .verify_trailbase_installation(
                TrailBaseInstanceId::new_v7(),
                true,
                false,
                RequestCorrelationId::new_v7(),
                started,
            )
            .expect("active installation");
        let make = || {
            AuthCeremony::try_new(
                OperationId::new_v7(),
                AuthCeremonyPurpose::SignIn,
                AuthCeremonyProtocol::TrailBaseAuthorizationCodePkce,
                installation.id(),
                installation.activation_generation(),
                Sha256Digest::from_bytes(&sha256_bytes(
                    OperationId::new_v7().to_string().as_bytes(),
                )),
                AuthCallbackPath::parse("/auth/trailbase/callback").expect("callback"),
                AuthReturnTarget::ApplicationHome,
                RequestCorrelationId::new_v7(),
                started,
                started + Duration::minutes(5),
            )
            .expect("ceremony")
        };
        let pending = make();
        let claimed = make();
        kernel
            .insert_auth_ceremony(&pending)
            .expect("insert pending");
        kernel
            .insert_auth_ceremony(&claimed)
            .expect("insert claimed candidate");
        kernel
            .claim_auth_ceremony(
                claimed.browser_binding_digest(),
                installation.id(),
                installation.activation_generation(),
                claimed.callback_path(),
                RequestCorrelationId::new_v7(),
                started,
            )
            .expect("claim");
        drop(kernel);

        let reopened = SqliteKernel::open(root.path()).expect("reopened kernel");
        let connection = reopened.inner.connection.lock().expect("connection");
        let states: Vec<(String, String)> = {
            let mut statement = connection
                .prepare("SELECT state, failure FROM auth_ceremonies ORDER BY state, operation_id")
                .expect("prepare states");
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("query states")
                .collect::<Result<_, _>>()
                .expect("collect states")
        };
        assert_eq!(
            states,
            vec![
                (
                    "cleanup_uncertain".to_owned(),
                    "exchange_outcome_uncertain".to_owned()
                ),
                ("failed".to_owned(), "verifier_lost_on_restart".to_owned()),
            ]
        );
        let recovery_audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('ceremony_failed', 'ceremony_cleanup_uncertain')",
                [],
                |row| row.get(0),
            )
            .expect("recovery audit count");
        assert_eq!(recovery_audits, 2);
        drop(connection);
        drop(reopened);

        let reopened_again = SqliteKernel::open(root.path()).expect("second reopen");
        let connection = reopened_again.inner.connection.lock().expect("connection");
        let recovery_audits: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM access_audit_events WHERE event_kind IN ('ceremony_failed', 'ceremony_cleanup_uncertain')",
                [],
                |row| row.get(0),
            )
            .expect("recovery audit count");
        assert_eq!(recovery_audits, 2);
    }
}
