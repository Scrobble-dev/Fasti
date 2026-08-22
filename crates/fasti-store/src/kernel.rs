use crate::crypto::{constant_time_eq, sha256_hex};
use crate::schema::{migrate, SCHEMA_VERSION};
use chrono::{DateTime, Utc};
use fasti_application::{
    authorize, AccessSnapshot, ApplicationResult, AuthorizationRequirement, CapabilityKey,
    CredentialStatus, FastiProblem, GrantStatus, ProblemCode, RequestAccessContext, ScopeKey,
};
use fasti_domain::{
    ClientId, CredentialId, ProfileGrantId, ProfileId, RequestCorrelationId, WorkspaceId,
};
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
pub const MAX_EVIDENCE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_TEMP_EVIDENCE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_CONCURRENT_UPLOADS: usize = 4;

#[derive(Debug, Error)]
pub enum StoreOpenError {
    #[error("failed to prepare the Fasti data root: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to open or migrate SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("SQLite returned unsupported journal mode {0:?}; WAL is required")]
    JournalMode(String),
    #[error("SQLite returned synchronous level {0}; FULL is required")]
    SynchronousLevel(i64),
    #[error("SQLite schema version {actual} does not match expected version {expected}")]
    SchemaVersion { expected: i64, actual: i64 },
}

#[derive(Debug, Clone)]
pub struct SqliteKernel {
    pub(crate) inner: Arc<KernelInner>,
}

#[derive(Debug)]
pub(crate) struct KernelInner {
    pub(crate) data_root: PathBuf,
    pub(crate) current_root: PathBuf,
    pub(crate) payload_root: PathBuf,
    pub(crate) scratch_root: PathBuf,
    pub(crate) connection: Mutex<Connection>,
    pub(crate) upload_budget: Mutex<UploadBudget>,
}

#[derive(Debug, Default)]
pub(crate) struct UploadBudget {
    pub(crate) active: usize,
    pub(crate) reserved_bytes: u64,
}

impl SqliteKernel {
    pub fn open(data_root: impl AsRef<Path>) -> Result<Self, StoreOpenError> {
        let data_root = data_root.as_ref().to_path_buf();
        let current_root = data_root.join("current");
        let payload_root = current_root.join("payloads").join("sha256");
        let scratch_root = current_root.join("scratch").join("uploads");
        fs::create_dir_all(&payload_root)?;
        fs::create_dir_all(&scratch_root)?;

        let database_path = current_root.join("fasti.sqlite3");
        let connection = Connection::open(database_path)?;
        connection.busy_timeout(DEFAULT_BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let journal_mode: String =
            connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(StoreOpenError::JournalMode(journal_mode));
        }
        connection.pragma_update(None, "synchronous", "FULL")?;
        let synchronous: i64 = connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        if synchronous != 2 {
            return Err(StoreOpenError::SynchronousLevel(synchronous));
        }
        migrate(&connection)?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            return Err(StoreOpenError::SchemaVersion {
                expected: SCHEMA_VERSION,
                actual: version,
            });
        }

        Ok(Self {
            inner: Arc::new(KernelInner {
                data_root,
                current_root,
                payload_root,
                scratch_root,
                connection: Mutex::new(connection),
                upload_budget: Mutex::new(UploadBudget::default()),
            }),
        })
    }

    pub fn data_root(&self) -> &Path {
        &self.inner.data_root
    }

    pub fn database_path(&self) -> PathBuf {
        self.inner.current_root.join("fasti.sqlite3")
    }

    pub(crate) fn lock_connection(
        &self,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<MutexGuard<'_, Connection>> {
        self.inner.connection.lock().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })
    }

    pub(crate) fn lock_upload_budget(
        &self,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<MutexGuard<'_, UploadBudget>> {
        self.inner.upload_budget.lock().map_err(|_| {
            Box::new(FastiProblem::storage_unavailable(
                capability,
                correlation_id,
            ))
        })
    }
}

pub(crate) fn now() -> DateTime<Utc> {
    Utc::now()
}

pub(crate) fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

pub(crate) fn parse_timestamp(
    value: &str,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

pub(crate) fn map_sql<T>(
    result: rusqlite::Result<T>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<T> {
    result.map_err(|_| {
        Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        ))
    })
}

pub(crate) fn map_json<T>(
    result: serde_json::Result<T>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<T> {
    result.map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

pub(crate) fn random_secret(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<fasti_application::SecretMaterial> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| {
        Box::new(FastiProblem::storage_unavailable(
            capability,
            correlation_id,
        ))
    })?;
    Ok(fasti_application::SecretMaterial::from_bytes(bytes))
}

pub(crate) fn digest_secret(secret: &fasti_application::SecretMaterial) -> String {
    sha256_hex(secret.expose_bytes())
}

pub(crate) fn verify_digest(stored: &str, presented: &str) -> bool {
    constant_time_eq(stored.as_bytes(), presented.as_bytes())
}

pub(crate) fn fsync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

pub(crate) fn scope_storage_key(scope: ScopeKey) -> &'static str {
    match scope {
        ScopeKey::CapabilityRead => "capability_read",
        ScopeKey::ClientEnroll => "client_enroll",
        ScopeKey::ProfileSelect => "profile_select",
        ScopeKey::CredentialManage => "credential_manage",
        ScopeKey::ListenerConfigure => "listener_configure",
        ScopeKey::ObservationAccept => "observation_accept",
        ScopeKey::ReceiptRead => "receipt_read",
        ScopeKey::IdentityWrite => "identity_write",
        ScopeKey::ReviewRead => "review_read",
        ScopeKey::ReviewWrite => "review_write",
        ScopeKey::CorrectionRead => "correction_read",
        ScopeKey::CorrectionWrite => "correction_write",
        ScopeKey::WorkspaceExport => "workspace_export",
        ScopeKey::WorkspaceRestore => "workspace_restore",
        ScopeKey::WorkspaceVerify => "workspace_verify",
    }
}

pub(crate) fn parse_scope(value: &str) -> Option<ScopeKey> {
    ScopeKey::ALL
        .iter()
        .copied()
        .find(|scope| scope_storage_key(*scope) == value)
}

pub(crate) fn load_access_snapshot(
    connection: &Connection,
    access: &RequestAccessContext,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessSnapshot> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT
                    cr.workspace_id,
                    pg.profile_id,
                    cr.client_id,
                    cr.credential_id,
                    pg.grant_id,
                    cr.status,
                    pg.status,
                    c.current_credential_epoch
                FROM credentials cr
                JOIN clients c ON c.client_id = cr.client_id
                JOIN profile_grants pg
                  ON pg.client_id = cr.client_id
                 AND pg.grant_id = ?2
                WHERE cr.credential_id = ?1
                "#,
                rusqlite::params![
                    access.credential_id().to_string(),
                    access.grant_id().to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;

    let Some((
        workspace,
        profile,
        client,
        credential,
        grant,
        credential_status,
        grant_status,
        epoch,
    )) = row
    else {
        return Ok(AccessSnapshot::bootstrap_closed());
    };

    let mut statement = map_sql(
        connection.prepare("SELECT scope_key FROM grant_scopes WHERE grant_id = ?1"),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([grant.as_str()], |row| row.get::<_, String>(0)),
        capability,
        correlation_id,
    )?;
    let mut scopes = Vec::new();
    for value in rows {
        let value = map_sql(value, capability, correlation_id)?;
        let scope = parse_scope(&value)
            .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        scopes.push(scope);
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
    let credential_id = credential
        .parse::<CredentialId>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let grant_id = grant
        .parse::<ProfileGrantId>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;

    Ok(AccessSnapshot::established(
        workspace_id,
        profile_id,
        client_id,
        credential_id,
        grant_id,
        if credential_status == "active" {
            CredentialStatus::Active
        } else {
            CredentialStatus::Revoked
        },
        if grant_status == "active" {
            GrantStatus::Active
        } else {
            GrantStatus::Revoked
        },
        u64::try_from(epoch).unwrap_or(u64::MAX),
        scopes,
    ))
}

pub(crate) fn authorize_connection(
    connection: &Connection,
    capability: CapabilityKey,
    access: &RequestAccessContext,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessSnapshot> {
    let snapshot = load_access_snapshot(connection, access, capability, correlation_id)?;
    authorize(
        &AuthorizationRequirement::for_capability(capability),
        Some(access),
        Some(&snapshot),
    )
    .map_err(|_| Box::new(FastiProblem::forbidden(capability, correlation_id)))?;
    Ok(snapshot)
}

pub(crate) fn authorize_transaction(
    transaction: &Transaction<'_>,
    capability: CapabilityKey,
    access: &RequestAccessContext,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AccessSnapshot> {
    authorize_connection(transaction, capability, access, correlation_id)
}

pub(crate) fn problem(
    code: ProblemCode,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(code, capability, correlation_id))
}
