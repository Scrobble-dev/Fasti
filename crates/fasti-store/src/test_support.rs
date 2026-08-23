use crate::kernel::{now, scope_storage_key, timestamp};
use crate::SqliteKernel;
use fasti_application::{
    AccessAdministrationPort, EnrollFirstClientCommand, EvidenceUploadPort, EvidenceUploadRequest,
    InitializeNodeCommand, RequestAccessContext, ScopeKey, SecretMaterial,
};
use fasti_domain::{ProfileGrantId, ProfileId, RequestCorrelationId};
use rusqlite::params;
use tempfile::TempDir;

pub(crate) struct TestNode {
    _root: TempDir,
    pub(crate) kernel: SqliteKernel,
    pub(crate) access: RequestAccessContext,
}

impl TestNode {
    pub(crate) fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary data root");
        let kernel = SqliteKernel::open(root.path()).expect("SQLite kernel");
        let initialized = kernel
            .initialize_node(InitializeNodeCommand::new(RequestCorrelationId::new_v7()))
            .expect("initialize node");
        let proof = SecretMaterial::try_from_hex(&initialized.initialization_proof().expose_hex())
            .expect("copy one-time proof");
        let enrolled = kernel
            .enroll_first_client(EnrollFirstClientCommand::new(
                RequestCorrelationId::new_v7(),
                proof,
            ))
            .expect("enroll first client");

        Self {
            _root: root,
            kernel,
            access: *enrolled.access(),
        }
    }

    pub(crate) fn upload(&self, bytes: &[u8]) -> fasti_domain::EvidenceReference {
        self.upload_for(self.access, bytes)
    }

    pub(crate) fn upload_for(
        &self,
        access: RequestAccessContext,
        bytes: &[u8],
    ) -> fasti_domain::EvidenceReference {
        let mut upload = self
            .kernel
            .begin_evidence_upload(EvidenceUploadRequest::new(
                RequestCorrelationId::new_v7(),
                access,
                Some(bytes.len() as u64),
            ))
            .expect("begin evidence upload");
        upload.write_chunk(bytes).expect("write evidence");
        upload.finish().expect("finish evidence")
    }

    pub(crate) fn add_profile_with_scopes(&self, scopes: &[ScopeKey]) -> RequestAccessContext {
        let profile_id = ProfileId::new_v7();
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
                    self.access.workspace_id().to_string(),
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
                    self.access.workspace_id().to_string(),
                    profile_id.to_string(),
                    self.access.client_id().to_string(),
                    created_at
                ],
            )
            .expect("insert profile grant");
        for scope in scopes {
            connection
                .execute(
                    "INSERT INTO grant_scopes(grant_id, scope_key) VALUES (?1, ?2)",
                    params![grant_id.to_string(), scope_storage_key(*scope)],
                )
                .expect("insert grant scope");
        }
        RequestAccessContext::new(
            self.access.workspace_id(),
            profile_id,
            self.access.client_id(),
            self.access.credential_id(),
            grant_id,
            self.access.presented_credential_epoch(),
        )
    }
}
